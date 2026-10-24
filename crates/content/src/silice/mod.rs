//! Pokémon Silice (SIL1, `cfg(pkr_silice)`): the game root over the generated Silice tables.
//!
//! The build (`build/silice/`) compiles `games/silice` on the extract's base assets into the
//! same generated modules as Emerald (`crate::{consts, data, battle, scripts}`). The scripts are
//! `src/silice/scripts/<module>.rs`, each `pub async fn <Name>(ctx: Ctx)` a script
//! (`content::scripts::<module>::<Name>`, `ScriptId`s in name order), written against
//! [`prelude`] and naming texts, flags and maps by their generated constants only.
//!
//! **Task E** (this file, [`prelude`] and [`new_game`]): the new game, the system ids and the
//! heal locations come from `games/silice/game.ron` (`build/silice/game.rs`, as the `SIL_*` and
//! `HEAL_LOCATION_SIL_*` constants of `crate::consts`):
//! - the new game on `game.ron`'s start cell and facing (`data::NEW_GAME`), with its player
//!   name (`data::PLAYER_NAME`), running [`new_game::new_game`] (its flags and vars);
//! - running on `SIL_RUNNING_FLAG` (`FLAG_SIL_RUNNING_SHOES`), the map-change flags of
//!   `SIL_FLAGS_CLEARED_ON_MAP_CHANGE`, the respawn `SIL_RESPAWN`;
//! - heal location `n` is `SIL_HEAL_LOCATIONS[n - 1]`;
//! - no whiteout script and no wild-set var yet.
//!
//! **Task A** (`build/silice/{text,data,merge}.rs`): the texts of `text/fr.ron`, Silice's
//! trainers, wild encounters (by Silice map), items (with their base items' hold effects) and
//! marts (`data::MARTS`), the map headers and `LOCALID_*`, through the shared `game_data_tables!`.
//!
//! **SIL3 battle mechanics**: the Mega and Primal forms (`battle/forms.ron`,
//! `data::FORM_CHANGES`), the trainers' AI item bags (`data::trainer_items`), and the field
//! rules, per-trainer rules and mid-battle lines of [`rules`].

pub mod mallette;
pub mod new_game;
pub mod prelude;
pub mod rules;

#[cfg(test)]
mod game_tests;
#[cfg(test)]
mod tests;

use crate::consts::*;
use crate::{NEW_GAME_SCRIPT, data};
use engine::battle::{BattleRules, FieldRule, TrainerLine};
use engine::data::{
    FieldMove, FieldMoveGate, FormChange, GameData, ItemId, MapId, ScriptId, SpecialIds, TrainerId,
};
use engine::script::{Ctx, Script};

/// Ch12's « une fois par jour » flags (§8 `FLAG_SIL_DAILY_*`): cleared by the engine on the
/// first map load of each new day (`SpecialIds::daily_flags`).
pub const SIL_DAILY_FLAGS: &[engine::data::FlagId] = &[
    FLAG_SIL_DAILY_JULIEN,
    FLAG_SIL_DAILY_POUSSIN,
    FLAG_SIL_DAILY_TOURNOI,
    FLAG_SIL_DAILY_CROW,
    FLAG_SIL_DAILY_LOIC,
    FLAG_SIL_DAILY_YOHAN,
    FLAG_SIL_DAILY_TAUBER_HOCSEAT,
    FLAG_SIL_DAILY_SAMUEL,
    FLAG_SIL_DAILY_SKYCKER,
    FLAG_SIL_DAILY_TAK,
    FLAG_SIL_DAILY_REVERSE,
    FLAG_SIL_DAILY_LOULPIX_DORIS,
    FLAG_SIL_DAILY_ANTENNE_ALPHA,
    FLAG_SIL_DAILY_ANTENNE_GAMMA,
    FLAG_SIL_DAILY_ANTENNE_ETA,
];

/// Silice's content root.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Game;

impl Game {
    /// What the shells play (the same as `Game::default()`).
    pub const fn for_shells() -> Game {
        Game
    }
}

impl GameData for Game {
    crate::game_data_tables!();

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            flag_running: Some(SIL_RUNNING_FLAG),
            flags_cleared_on_map_change: SIL_FLAGS_CLEARED_ON_MAP_CHANGE,
            respawn_default: SIL_RESPAWN,
            var_wild_set: None,
            script_whiteout: None,
            // Reverse-City's inverted controls and its emitters' idle timer (Ch7).
            var_input_map: Some(VAR_SIL_INPUT_MAP),
            var_idle_frames: Some(VAR_SIL_IDLE_FRAMES),
            // Ch12's daily flags (§9 « Drapeaux quotidiens »): Gen3's `VAR_DAYS` keeps the day.
            var_days: Some(VAR_DAYS),
            daily_flags: SIL_DAILY_FLAGS,
            ..crate::gen3_special_ids()
        }
    }

    /// The Mallette's modules ([`mallette::field_move_gate`]).
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        mallette::field_move_gate(m)
    }

    fn field_move_label(&self, m: FieldMove) -> Option<&'static [u8]> {
        mallette::field_move_label(self.battle(), m)
    }

    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        mallette::field_move_script(m)
    }

    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        Some(data::script_ids::Mallette_CannotUseWaterfall)
    }

    fn dive_emerge_script(&self) -> Option<ScriptId> {
        Some(data::script_ids::Mallette_UseDiveUnderwater)
    }

    /// The new game: `game.ron`'s start, running [`new_game::new_game`].
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == NEW_GAME_SCRIPT {
            return alloc::boxed::Box::pin(new_game::new_game(ctx));
        }
        data::script(id.0, ctx)
    }

    /// `game.ron`'s heal location `id` (numbered from 1).
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        SIL_HEAL_LOCATIONS
            .get(usize::from(id).checked_sub(1)?)
            .copied()
    }

    /// The map's dive or emerge connection, keeping the cell, unless that cell is outside the
    /// connected map (Route S9's conduit runs on south of the lake: its exit's arrow warps
    /// would surface the player off Route S9; Gen3 would warp there); else [`DIVE_WARPS`].
    fn dive_warp(
        &self,
        map: MapId,
        emerge: bool,
        (x, y): (i16, i16),
    ) -> Option<(MapId, Option<(i16, i16)>)> {
        if let Some(to) = data::dive_connection(map.0, emerge) {
            let m = self.map(to);
            let inside = (0..i32::from(m.width)).contains(&i32::from(x))
                && (0..i32::from(m.height)).contains(&i32::from(y));
            return inside.then_some((to, None));
        }
        let &(.., to, tx, ty) = DIVE_WARPS.iter().find(|w| {
            let (from, up, (x0, y0), (x1, y1)) = (w.0, w.1, w.2, w.3);
            from == map && up == emerge && (x0..=x1).contains(&x) && (y0..=y1).contains(&y)
        })?;
        Some((to, Some((tx, ty))))
    }

    /// `battle/forms.ron`'s Mega and Primal forms.
    fn form_changes(&self) -> &'static [FormChange] {
        &data::FORM_CHANGES
    }

    fn battle_rule(&self, id: u8) -> Option<&'static FieldRule> {
        rules::battle_rule(id)
    }

    fn trainer_rules(&self, id: TrainerId) -> Option<&'static BattleRules> {
        rules::trainer_rules(id)
    }

    fn trainer_lines(&self, id: TrainerId) -> &'static [TrainerLine] {
        rules::trainer_lines(id)
    }

    /// `battle/trainers.ron`'s AI item bags.
    fn trainer_items(&self, id: TrainerId) -> &'static [ItemId] {
        data::trainer_items(id.0)
    }
}

/// The fixed dive warps (Gen3 `setdivewarp` in a map's `on_dive_warp` script, for a map without
/// a dive connection): (map, emerge, the cells' corners, inclusive, destination map and cell).
/// The maps' `on_dive_warp` scripts (`Nebula_OnDiveWarp`, `Nebula_TourHall_B1F_OnDiveWarp`)
/// point here; the engine asks [`GameData::dive_warp`] with the player's cell.
pub const DIVE_WARPS: &[(MapId, bool, (i16, i16), (i16, i16), MapId, i16, i16)] = &[
    // Ch9: the Bassin de Captage dives under the Tower, into B1F's cuve.
    (
        crate::consts::MAP_NEBULA,
        false,
        (50, 10),
        (52, 13),
        crate::consts::MAP_NEBULA_TOUR_HALL_B1F,
        6,
        10,
    ),
    // Ch9: the quai's deep water dives back into the conduit's exit (the way back to Route S9).
    (
        crate::consts::MAP_NEBULA,
        false,
        (29, 54),
        (30, 55),
        crate::consts::MAP_ROUTE_S09_UNDERWATER,
        39,
        72,
    ),
    // Ch9: B1F's cuve dives back to the Bassin (the way back to the town).
    (
        crate::consts::MAP_NEBULA_TOUR_HALL_B1F,
        false,
        (5, 9),
        (7, 11),
        crate::consts::MAP_NEBULA,
        51,
        11,
    ),
];
