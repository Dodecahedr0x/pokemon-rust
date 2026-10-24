//! Emerald's `TryStartStepCountScript` checks (`field_control_avatar.c`), answered through
//! `GameData::step_count_script` once per finished player step (Wave E).
//!
//! In Gen3's order, after the engine's friendship counter:
//! - `UpdateFarawayIslandStepCounter` (`faraway_island.c`);
//! - `UpdatePoisonStepCounter` / `DoPoisonFieldEffect` (`field_poison.c`): every 4th step each
//!   poisoned mon loses 1 HP; when one is at 0 (`FLDPSN_FNT`), `EventScript_FieldPoison` starts
//!   (its `TryFieldPoisonWhiteOut` whites out once every mon fainted). `FaintFromFieldPoison`
//!   (status cleared, `FRIENDSHIP_EVENT_FAINT_FIELD_PSN`), which Gen3 runs in that special's
//!   task, runs here with the step: the special cannot change a mon's status;
//! - the Match Call registrations `ShouldDoWallyCall`, `ShouldDoScottFortreeCall`,
//!   `ShouldDoScottBattleFrontierCall`, `ShouldDoRoxanneCall`, `ShouldDoRivalRayquazaCall`
//!   (`field_specials.c`): while the call's flag is set, a step on a town, city, route or ocean
//!   route counts its var up, and at its threshold the registration script starts (it swaps the
//!   flags, so the count stops).
//!
//! Not modelled (no such state in the engine yet): `IncrementRematchStepCounter` (the save's
//! rematch counter), egg hatching (`ShouldEggHatch`: no eggs), `AbnormalWeatherHasExpired`, the
//! Regice braille walk (`ShouldDoBrailleRegicePuzzle`), `SafariZoneTakeStep`, `CountSSTidalStep`
//! and the trainers' Match Calls (`TryStartMatchCall`). Without `EventScript_FieldPoison` (an
//! extract older than 5.10) a fainted mon starts no script. The secret-base exception of the
//! poison counter is moot (no secret bases). The screen flash of `FldEffPoison_Start` is not
//! drawn.

use crate::consts::*;
use engine::data::{GameData, MapType, ScriptId, StepCount};
use engine::mon::Status;

/// `ShouldDo*Call`: (the flag that arms the call, its step var, the steps, the script's index in
/// [`crate::data::SCRIPT_STEP_CALLS`]), in `TryStartStepCountScript`'s order.
const CALLS: [(engine::data::FlagId, engine::data::VarId, u16, usize); 5] = [
    (
        FLAG_ENABLE_FIRST_WALLY_POKENAV_CALL,
        VAR_WALLY_CALL_STEP_COUNTER,
        250,
        1,
    ),
    (
        FLAG_SCOTT_CALL_FORTREE_GYM,
        VAR_SCOTT_FORTREE_CALL_STEP_COUNTER,
        10,
        2,
    ),
    (
        FLAG_SCOTT_CALL_BATTLE_FRONTIER,
        VAR_SCOTT_BF_CALL_STEP_COUNTER,
        10,
        3,
    ),
    (
        FLAG_ENABLE_ROXANNE_FIRST_CALL,
        VAR_ROXANNE_CALL_STEP_COUNTER,
        250,
        0,
    ),
    (
        FLAG_DEFEATED_MAGMA_SPACE_CENTER,
        VAR_RIVAL_RAYQUAZA_CALL_STEP_COUNTER,
        250,
        4,
    ),
];

/// `UpdateFarawayIslandStepCounter`: the count wraps to 0 at 9,999.
const FARAWAY_STEPS_MAX: u16 = 9999;
/// `UpdatePoisonStepCounter`: poison hurts every 4th step.
const POISON_STEP_PERIOD: u16 = 4;
/// `sFriendshipEventModifiers[FRIENDSHIP_EVENT_FAINT_FIELD_PSN]`, by friendship level.
const FAINT_FIELD_PSN: [i16; 3] = [-5, -5, -10];

/// Gen3 `TryStartStepCountScript`'s checks (see the module doc).
pub fn step_count_script(game: &impl GameData, s: StepCount<'_>) -> Option<ScriptId> {
    if s.map == MAP_FARAWAY_ISLAND_INTERIOR {
        let n = s.vars.var(VAR_FARAWAY_ISLAND_STEP_COUNTER) + 1;
        let n = if n >= FARAWAY_STEPS_MAX { 0 } else { n };
        s.vars.set_var(VAR_FARAWAY_ISLAND_STEP_COUNTER, n);
    }
    let poison = (s.vars.var(VAR_POISON_STEP_COUNTER) + 1) % POISON_STEP_PERIOD;
    s.vars.set_var(VAR_POISON_STEP_COUNTER, poison);
    if poison == 0
        && poison_field_effect(s.party)
        && let Some(script) = crate::data::SCRIPT_FIELD_POISON
    {
        return Some(script);
    }
    let outdoors = matches!(
        game.map_type(s.map),
        MapType::Outdoor | MapType::OutdoorSand | MapType::OceanRoute
    );
    for (flag, var, steps, script) in CALLS {
        if !s.vars.flag(flag) || !outdoors {
            continue;
        }
        let n = s.vars.var(var).wrapping_add(1);
        s.vars.set_var(var, n);
        if n >= steps {
            return crate::data::SCRIPT_STEP_CALLS[script];
        }
    }
    None
}

/// `DoPoisonFieldEffect` with `FaintFromFieldPoison` for the mons at 0 HP: whether one is
/// (`FLDPSN_FNT`).
fn poison_field_effect(party: &mut engine::mon::Party) -> bool {
    let mut fainted = false;
    for i in 0..engine::mon::PARTY_SIZE {
        let Some(mon) = party.get_mut(i) else {
            continue;
        };
        if !matches!(mon.status, Status::Poison | Status::Toxic(_)) || mon.species.0 == 0 {
            continue;
        }
        mon.hp = mon.hp.saturating_sub(1);
        if mon.hp == 0 {
            fainted = true;
            mon.status = Status::None;
            let level = usize::from(mon.friendship > 99) + usize::from(mon.friendship > 199);
            mon.friendship = (i16::from(mon.friendship) + FAINT_FIELD_PSN[level]).max(0) as u8;
        }
    }
    fainted
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::flow::Vars;
    use engine::mon::Party;

    fn step(vars: &mut Vars, party: &mut Party, map: engine::data::MapId) -> Option<ScriptId> {
        let game = crate::Game::default();
        step_count_script(
            &game,
            StepCount {
                map,
                cell: (0, 0),
                vars,
                party,
            },
        )
    }

    #[test]
    fn roxanne_call_starts_after_250_outdoor_steps() {
        let (mut vars, mut party) = (Vars::new(), Party::default());
        let route = MAP_ROUTE101;
        assert_eq!(step(&mut vars, &mut party, route), None);
        assert_eq!(vars.var(VAR_ROXANNE_CALL_STEP_COUNTER), 0, "unarmed");
        vars.set_flag(FLAG_ENABLE_ROXANNE_FIRST_CALL, true);
        for _ in 0..249 {
            assert_eq!(step(&mut vars, &mut party, route), None);
        }
        assert_eq!(vars.var(VAR_ROXANNE_CALL_STEP_COUNTER), 249);
        assert_eq!(
            step(&mut vars, &mut party, route),
            crate::data::SCRIPT_STEP_CALLS[0]
        );
        // The full import (it has `EventScript_UseSurf`) has every registration too.
        assert!(
            crate::data::SCRIPT_USE_SURF.is_none()
                || crate::data::SCRIPT_STEP_CALLS.iter().all(Option::is_some)
        );
    }

    #[test]
    fn poison_hurts_every_fourth_step() {
        let (mut vars, mut party) = (Vars::new(), Party::default());
        let mon = engine::mon::Pokemon {
            species: engine::data::SpeciesId(1),
            personality: 0,
            ot_id: 0,
            nickname: [0xFF; engine::mon::NAME_LEN],
            level: 5,
            exp: 0,
            ivs: [0; 6],
            evs: [0; 6],
            moves: [None; 4],
            pp: [0; 4],
            pp_bonuses: 0,
            friendship: 70,
            ability_slot: 0,
            status: Status::Poison,
            hp: 2,
            stats: [20; 6],
            held: None,
        };
        let _ = party.push(mon);
        for _ in 0..3 {
            step(&mut vars, &mut party, MAP_ROUTE101);
        }
        assert_eq!(party.get(0).unwrap().hp, 2);
        step(&mut vars, &mut party, MAP_ROUTE101);
        assert_eq!(party.get(0).unwrap().hp, 1);
        for _ in 0..3 {
            step(&mut vars, &mut party, MAP_ROUTE101);
        }
        // The fourth step faints it: `EventScript_FieldPoison` (when extracted) starts.
        let started = step(&mut vars, &mut party, MAP_ROUTE101);
        assert_eq!(started, crate::data::SCRIPT_FIELD_POISON);
        let m = party.get(0).unwrap();
        assert_eq!((m.hp, m.status), (0, Status::None));
        // Fainted and cured: no more poison steps, no script.
        for _ in 0..4 {
            assert_eq!(step(&mut vars, &mut party, MAP_ROUTE101), None);
        }
    }
}
