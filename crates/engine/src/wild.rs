//! Wild encounters, as Gen3 `wild_encounter.c` (land only: no water, rocks, fishing, repel,
//! flutes, Cleanse Tag, roamers, mass outbreaks or Cute Charm).
//!
//! Every `Random()` of the Gen3 path is one `rng.random()` here, in the same order. The lead is
//! `gPlayerParty[0]` (fainted or not, as Gen3); `None` (an empty party) has no ability. Lead
//! abilities only matter when the engine's `Ability` enum has them: Static (electric slots),
//! Keen Eye / Intimidate (weak encounters skipped), Synchronize (nature). The rate modifiers
//! (Stench, Illuminate, White Smoke, Arena Trap, Sand Veil) and the level ones (Hustle, Vital
//! Spirit, Pressure) map to `Ability::Other` and have no effect.
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
use crate::mon::{Ability, BattleTables, Ivs, LandEncounters, Pokemon, Type, WildMon};
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

/// Gen3 `WildEncounterCheck`'s odds out of `MAX_ENCOUNTER_RATE`: `rate * 16`, capped (no bike,
/// flute or Cleanse Tag; no lead ability in the engine's set changes it).
pub fn encounter_rate(rate: u8) -> u32 {
    (rate as u32 * 16).min(MAX_ENCOUNTER_RATE)
}

/// One land step: the new-metatile 60% gate (`AllowWildCheckOnNewMetatile`, when `prev` !=
/// `cur`), `WildEncounterCheck` (`random() % 2880 < rate * 16`), the slot (Static lead,
/// then `ChooseWildMonIndex_Land`), the level (`ChooseWildMonLevel`) and the Keen Eye /
/// Intimidate filter (`IsAbilityAllowingEncounter`). Only `TallGrass` cells have land
/// encounters (`MetatileBehavior_IsLandWildEncounter`); other cells draw nothing.
pub fn try_land_encounter(
    table: &LandEncounters,
    prev: Behavior,
    cur: Behavior,
    lead: Option<&Pokemon>,
    t: &BattleTables,
    rng: &mut Rng,
) -> Option<(SpeciesId, u8)> {
    if cur != Behavior::TallGrass {
        return None;
    }
    // Gen3 compares raw `MB_*` values; we compare the coarse `Behavior`. This is the same test
    // because only `MB_TALL_GRASS` maps to `TallGrass` (content's classification): any other
    // land-encounter behavior (long grass, cave, sand) would need its own variant first.
    if prev != cur && rng.random() % 100 >= 60 {
        return None;
    }
    if rng.random() as u32 % MAX_ENCOUNTER_RATE >= encounter_rate(table.rate) {
        return None;
    }
    let ability = lead.map_or(Ability::None, |p| p.ability(t));
    let slot = match static_slot(table, ability, t, rng) {
        Some(i) => i,
        None => land_slot(rng.random()),
    };
    let mon = table.mons[slot];
    let level = wild_level(&mon, rng);
    if let Some(lead) = lead
        && matches!(ability, Ability::KeenEye | Ability::Intimidate)
        && lead.level > 5
        && level <= lead.level - 5
        && rng.random().is_multiple_of(2)
    {
        return None;
    }
    Some((mon.species, level))
}

/// Gen3 `TryGetAbilityInfluencedWildMonIndex(…, TYPE_ELECTRIC, ABILITY_STATIC, …)`: a Static
/// lead picks an Electric slot half the time, uniformly, unless none or all are Electric.
fn static_slot(
    table: &LandEncounters,
    ability: Ability,
    t: &BattleTables,
    rng: &mut Rng,
) -> Option<usize> {
    if ability != Ability::Static || !rng.random().is_multiple_of(2) {
        return None;
    }
    let mut valid = [0usize; 12];
    let mut count = 0;
    for (i, m) in table.mons.iter().enumerate() {
        if t.species_info(m.species).types.contains(&Type::Electric) {
            valid[count] = i;
            count += 1;
        }
    }
    if count == 0 || count == table.mons.len() {
        return None;
    }
    Some(valid[rng.random() as usize % count])
}

/// Gen3 `ChooseWildMonLevel`: `min + random() % (max - min + 1)`, bounds swapped if reversed.
fn wild_level(mon: &WildMon, rng: &mut Rng) -> u8 {
    let (min, max) = if mon.max_level >= mon.min_level {
        (mon.min_level, mon.max_level)
    } else {
        (mon.max_level, mon.min_level)
    };
    // u8 as Gen3: 0..=255 wraps the range to 0, where Gen3's `% 0` is undefined behavior.
    // `max(1)` turns that case into "always min" instead of a division-by-zero panic.
    let range = (max - min).wrapping_add(1);
    min + (rng.random() % range.max(1) as u16) as u8
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

#[cfg(test)]
mod tests;
