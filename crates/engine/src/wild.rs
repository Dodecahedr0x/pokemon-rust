//! Wild encounters, as Gen3 `wild_encounter.c`. **Task C.**

use crate::data::{Behavior, SpeciesId};
use crate::mon::{BattleTables, LandEncounters, Pokemon};
use crate::rng::Rng;

/// One land step: the new-metatile 60% gate (`AllowWildCheckOnNewMetatile`, `prev` -> `cur`),
/// `WildEncounterCheck` (rate x 16, lead ability modifiers, cap 2880, `random() % 2880`), the
/// slot (`ChooseWildMonIndex_Land`) and the level (`ChooseWildMonLevel`). No repel, flute or
/// roamer. Task C.
#[allow(unused_variables)]
pub fn try_land_encounter(
    table: &LandEncounters,
    prev: Behavior,
    cur: Behavior,
    lead: Option<&Pokemon>,
    t: &BattleTables,
    rng: &mut Rng,
) -> Option<(SpeciesId, u8)> {
    todo!("Task C: WildEncounterCheck + ChooseWildMonIndex_Land + ChooseWildMonLevel")
}

/// Gen3 `CreateWildMon`: `PickWildMonNature` (Synchronize lead), then
/// `CreateMonWithNature` (rerolls `random32()` until `personality % 25 == nature`), random IVs.
/// Task C.
#[allow(unused_variables)]
pub fn create_wild(
    t: &BattleTables,
    species: SpeciesId,
    level: u8,
    lead: Option<&Pokemon>,
    ot_id: u32,
    rng: &mut Rng,
) -> Pokemon {
    todo!("Task C: CreateWildMon")
}
