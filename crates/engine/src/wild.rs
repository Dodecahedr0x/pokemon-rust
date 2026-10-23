//! Wild encounters, as Gen3 `wild_encounter.c` (land only: no water, rocks or fishing; no
//! roamers or mass outbreaks, which only exist after the Elite Four).
//!
//! Every `Random()` of the Gen3 path is one `rng.random()` here, in the same order. The lead is
//! `gPlayerParty[0]` (fainted or not, as Gen3; the engine has no eggs, so Gen3's egg tests never
//! pass); `None` (an empty party) has no ability and holds nothing.
//!
//! What changes the odds or the slot, and what does not (see [`Lead`]):
//! - The rate (`WildEncounterCheck`): `rate * 16`, then the Cleanse Tag (x2/3), then the lead's
//!   Stench or White Smoke (/2) or Illuminate or Arena Trap (x2), capped at 2880. Not modelled:
//!   the bikes (x80/100: no bikes), the White and Black Flutes (`FLAG_SYS_ENC_UP_ITEM` /
//!   `DOWN_ITEM`: no field item use) and Sand Veil (/2 in a sandstorm: no field weather).
//! - The slot: a Magnet Pull lead, then a Static lead, picks a Steel, then an Electric, slot half
//!   the time (`TryGetAbilityInfluencedWildMonIndex`).
//! - The filters after the level: the Repel (`IsWildLevelAllowedByRepel`) is not modelled (no
//!   field item use sets `VAR_REPEL_STEP_COUNT`), and draws nothing in Gen3 either; then Keen
//!   Eye / Intimidate skip weak encounters half the time (`IsAbilityAllowingEncounter`).
//! - The level (`ChooseWildMonLevel`): a Hustle, Vital Spirit or Pressure lead gets the
//!   table's top level half the time.
//!
//! Caller contract (the World, Task J):
//! - Keep Gen3's 4-step encounter immunity: after a battle and after a map load
//!   (`RestartWildEncounterImmunitySteps`), the first 4 completed steps do not call
//!   `try_land_encounter` (`CheckStandardWildEncounter`).
//! - Update `prev` to the cell's behavior on every completed step, encounter checked or not
//!   (Gen3 `sPrevMetatileBehavior`).
//! - After a `Some` from `try_land_encounter`, call `create_wild` immediately, with no other
//!   RNG draw in between (Gen3 `TryGenerateWildMon` → `CreateWildMon`).

use crate::data::{Behavior, SpeciesId};
use crate::mon::{Ability, BattleTables, HoldEffect, Ivs, LandEncounters, Pokemon, Type, WildMon};
use crate::rng::Rng;

/// Gen3 `MAX_ENCOUNTER_RATE`: the odds denominator of `WildEncounterCheck`.
pub const MAX_ENCOUNTER_RATE: u32 = 2880;

/// Cumulative `ENCOUNTER_CHANCE_LAND_MONS_SLOT_*` thresholds (20/20/10/10/10/10/5/5/4/4/1/1).
const LAND_SLOT_THRESHOLDS: [u8; 12] = [20, 40, 50, 60, 70, 80, 85, 90, 94, 98, 99, 100];

/// Gen3 `ChooseWildMonIndex_Land` for one `Random()` value.
pub fn land_slot(roll: u16) -> usize {
    let r = (roll % 100) as u8;
    LAND_SLOT_THRESHOLDS
        .iter()
        .position(|&t| r < t)
        .unwrap_or(11)
}

/// What the lead (`gPlayerParty[0]`) brings to an encounter: its ability, its level (Keen Eye,
/// Intimidate) and whether it holds a Cleanse Tag. [`Lead::of`] reads them; tests build it
/// directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lead {
    pub ability: Ability,
    pub level: u8,
    pub cleanse_tag: bool,
}

impl Lead {
    /// No lead (an empty party): no ability, nothing held.
    pub const NONE: Lead = Lead {
        ability: Ability::None,
        level: 0,
        cleanse_tag: false,
    };

    /// The lead's ability and held item; `hold` is `GameData::hold_effect`.
    pub fn of(
        lead: Option<&Pokemon>,
        t: &BattleTables,
        hold: impl Fn(crate::data::ItemId) -> Option<HoldEffect>,
    ) -> Lead {
        lead.map_or(Lead::NONE, |p| Lead {
            ability: p.ability(t),
            level: p.level,
            cleanse_tag: p.held.and_then(hold) == Some(HoldEffect::CleanseTag),
        })
    }
}

/// Gen3 `WildEncounterCheck`'s odds out of `MAX_ENCOUNTER_RATE` for a table's `rate` and the
/// lead (no bike, flute or sandstorm: see the module doc). Integer steps as Gen3 (`u32`).
pub fn encounter_rate(rate: u8, lead: Lead) -> u32 {
    let mut r = rate as u32 * 16;
    // ApplyCleanseTagEncounterRateMod.
    if lead.cleanse_tag {
        r = r * 2 / 3;
    }
    match lead.ability {
        Ability::Stench | Ability::WhiteSmoke => r /= 2,
        Ability::Illuminate | Ability::ArenaTrap => r *= 2,
        _ => {}
    }
    r.min(MAX_ENCOUNTER_RATE)
}

/// One land step (Gen3 `StandardWildEncounter`'s land branch): only on a
/// [`Behavior::land_encounter`] cell (`MetatileBehavior_IsLandWildEncounter`; other cells draw
/// nothing); the new-metatile 60% gate (`AllowWildCheckOnNewMetatile`, when `prev` != `cur`:
/// [`Behavior`] keeps every encounter behaviour distinct, so this is Gen3's raw comparison),
/// `WildEncounterCheck` (`random() % 2880 < `[`encounter_rate`]), the slot (Magnet Pull, then
/// Static, then `ChooseWildMonIndex_Land`), the level (`ChooseWildMonLevel`) and the Keen Eye /
/// Intimidate filter (`IsAbilityAllowingEncounter`).
pub fn try_land_encounter(
    table: &LandEncounters,
    prev: Behavior,
    cur: Behavior,
    lead: Lead,
    t: &BattleTables,
    rng: &mut Rng,
) -> Option<(SpeciesId, u8)> {
    try_land_encounter_repel(table, prev, cur, lead, t, rng, |_| false)
}

/// [`try_land_encounter`] with Gen3's repel check (`IsWildLevelAllowedByRepel`, between the
/// level and the Keen Eye filter, as `TryGenerateWildMon`): `repel(level)` true drops the
/// encounter. The World passes [`repel_blocks`].
pub fn try_land_encounter_repel(
    table: &LandEncounters,
    prev: Behavior,
    cur: Behavior,
    lead: Lead,
    t: &BattleTables,
    rng: &mut Rng,
    repel: impl FnOnce(u8) -> bool,
) -> Option<(SpeciesId, u8)> {
    if !cur.land_encounter() {
        return None;
    }
    if prev != cur && rng.random() % 100 >= 60 {
        return None;
    }
    if rng.random() as u32 % MAX_ENCOUNTER_RATE >= encounter_rate(table.rate, lead) {
        return None;
    }
    let ability = lead.ability;
    let slot = match ability_slot(table, ability, Ability::MagnetPull, Type::Steel, t, rng)
        .or_else(|| ability_slot(table, ability, Ability::Static, Type::Electric, t, rng))
    {
        Some(i) => i,
        None => land_slot(rng.random()),
    };
    let mon = table.mons[slot];
    let level = wild_level(&mon, ability, rng);
    if repel(level) {
        return None;
    }
    if matches!(ability, Ability::KeenEye | Ability::Intimidate)
        && lead.level > 5
        && level <= lead.level - 5
        && rng.random().is_multiple_of(2)
    {
        return None;
    }
    Some((mon.species, level))
}

/// Gen3 `TryGetAbilityInfluencedWildMonIndex(…, ty, want, …)`: a lead with `want` picks a slot
/// of type `ty` half the time, uniformly, unless none or all are of that type. A lead without
/// `want` draws nothing.
fn ability_slot(
    table: &LandEncounters,
    ability: Ability,
    want: Ability,
    ty: Type,
    t: &BattleTables,
    rng: &mut Rng,
) -> Option<usize> {
    if ability != want || !rng.random().is_multiple_of(2) {
        return None;
    }
    let mut valid = [0usize; 12];
    let mut count = 0;
    for (i, m) in table.mons.iter().enumerate() {
        if t.species_info(m.species).types.contains(&ty) {
            valid[count] = i;
            count += 1;
        }
    }
    if count == 0 || count == table.mons.len() {
        return None;
    }
    Some(valid[rng.random() as usize % count])
}

/// Gen3 `ChooseWildMonLevel`: `min + random() % (max - min + 1)`, bounds swapped if reversed;
/// then a Hustle, Vital Spirit or Pressure lead returns `max` on a second roll's even value, or
/// else lowers the first roll by one (not below 0).
fn wild_level(mon: &WildMon, ability: Ability, rng: &mut Rng) -> u8 {
    let (min, max) = if mon.max_level >= mon.min_level {
        (mon.min_level, mon.max_level)
    } else {
        (mon.max_level, mon.min_level)
    };
    // u8 as Gen3: 0..=255 wraps the range to 0, where Gen3's `% 0` is undefined behavior.
    // `max(1)` turns that case into "always min" instead of a division-by-zero panic.
    let range = (max - min).wrapping_add(1);
    let mut roll = (rng.random() % range.max(1) as u16) as u8;
    if matches!(
        ability,
        Ability::Hustle | Ability::VitalSpirit | Ability::Pressure
    ) {
        if rng.random().is_multiple_of(2) {
            return max;
        }
        roll = roll.saturating_sub(1);
    }
    min + roll
}

/// Gen3 `CreateWildMon`: `PickWildMonNature` (a Synchronize lead passes its nature half the
/// time), then `CreateMonWithNature` (`random32()` until `personality % 25 == nature`), random
/// IVs, the player as OT.
pub fn create_wild(
    t: &BattleTables,
    species: SpeciesId,
    level: u8,
    lead: Option<&Pokemon>,
    ot_id: u32,
    rng: &mut Rng,
) -> Pokemon {
    let nature = match lead {
        Some(p) if p.ability(t) == Ability::Synchronize && rng.random().is_multiple_of(2) => {
            p.personality % 25
        }
        _ => rng.random() as u32 % 25,
    };
    let personality = loop {
        let p = rng.random32();
        if p % 25 == nature {
            break p;
        }
    };
    Pokemon::new(t, species, level, Ivs::Random, personality, ot_id, rng)
}

/// Gen3 `IsWildLevelAllowedByRepel`, negated: a repel is active (`SpecialIds::var_repel_steps`
/// non-zero) and the wild `level` is below the first healthy party mon's (Milestone 9 Task B).
/// Wave 0: never blocks.
pub(crate) fn repel_blocks(
    level: u8,
    vars: &crate::flow::Vars,
    party: &crate::mon::Party,
    ids: &crate::data::SpecialIds,
) -> bool {
    let _ = (level, vars, party, ids);
    false
}

#[cfg(test)]
mod tests;
