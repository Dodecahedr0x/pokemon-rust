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
mod tests {
    use super::*;
    use crate::battle::test_data::*;
    use crate::mon::{Nature, Pokemon};

    fn table(rate: u8, species: [SpeciesId; 12]) -> LandEncounters {
        LandEncounters {
            rate,
            mons: species.map(|s| WildMon {
                species: s,
                min_level: 2,
                max_level: 3,
            }),
        }
    }

    fn lead(species: SpeciesId, level: u8) -> Pokemon {
        Pokemon::new(
            &TABLES,
            species,
            level,
            Ivs::Fixed(0),
            0,
            0,
            &mut Rng::new(1),
        )
    }

    #[test]
    fn slot_boundaries_follow_choose_wild_mon_index_land() {
        // ENCOUNTER_CHANCE_LAND_MONS: 20/20/10/10/10/10/5/5/4/4/1/1 of 100.
        let cases = [
            (0, 0),
            (19, 0),
            (20, 1),
            (39, 1),
            (40, 2),
            (49, 2),
            (50, 3),
            (59, 3),
            (60, 4),
            (69, 4),
            (70, 5),
            (79, 5),
            (80, 6),
            (84, 6),
            (85, 7),
            (89, 7),
            (90, 8),
            (93, 8),
            (94, 9),
            (97, 9),
            (98, 10),
            (99, 11),
            // The roll is `Random() % 100`.
            (100, 0),
            (65535, 1),
        ];
        for (roll, slot) in cases {
            assert_eq!(land_slot(roll), slot, "roll {roll}");
        }
    }

    #[test]
    fn rate_gate_is_rate_times_16_of_2880() {
        assert_eq!(encounter_rate(20), 320);
        assert_eq!(encounter_rate(180), 2880);
        assert_eq!(encounter_rate(255), 2880);
        assert_eq!(encounter_rate(0), 0);
    }

    #[test]
    fn only_tall_grass_draws() {
        let tb = table(20, [ZIGZAGOON; 12]);
        let mut rng = Rng::new(5);
        for prev in [Behavior::Normal, Behavior::TallGrass] {
            let r = try_land_encounter(&tb, prev, Behavior::Normal, None, &TABLES, &mut rng);
            assert_eq!(r, None);
        }
        assert_eq!(rng, Rng::new(5));
    }

    /// Replays the Gen3 roll order by hand for many seeds: gate, odds, slot, level.
    #[test]
    fn roll_order_matches_gen3() {
        let mut species = [ZIGZAGOON; 12];
        species[1] = MUDKIP;
        let mut tb = table(20, species);
        tb.mons[1].min_level = 7;
        tb.mons[1].max_level = 4; // reversed: 4..=7
        let mut hits = 0;
        for seed in 1..4000 {
            for new_cell in [false, true] {
                let prev = if new_cell {
                    Behavior::Normal
                } else {
                    Behavior::TallGrass
                };
                let mut rng = Rng::new(seed);
                let got =
                    try_land_encounter(&tb, prev, Behavior::TallGrass, None, &TABLES, &mut rng);
                let mut r = Rng::new(seed);
                let want = (|| {
                    if new_cell && r.random() % 100 >= 60 {
                        return None;
                    }
                    if r.random() % 2880 >= 320 {
                        return None;
                    }
                    // ENCOUNTER_CHANCE_LAND_MONS_SLOT_0..11, cumulative, written out here so
                    // the oracle does not reuse `land_slot`.
                    let roll = r.random() % 100;
                    let slot = [20, 40, 50, 60, 70, 80, 85, 90, 94, 98, 99, 100]
                        .iter()
                        .position(|&t| roll < t)
                        .unwrap();
                    let m = tb.mons[slot];
                    let (lo, hi) = (m.min_level.min(m.max_level), m.min_level.max(m.max_level));
                    Some((m.species, lo + (r.random() % (hi - lo + 1) as u16) as u8))
                })();
                assert_eq!(got, want, "seed {seed}");
                assert_eq!(rng, r, "seed {seed}: same number of rolls");
                hits += got.is_some() as u32;
            }
        }
        // 320/2880 = 11.1% on the same cell, 6.7% on a new one: ~71 per 800 steps.
        assert!((600..900).contains(&hits), "{hits}");
    }

    #[test]
    fn static_lead_prefers_electric_slots() {
        let mut species = [ZIGZAGOON; 12];
        species[11] = SPARK; // the 1% slot
        let tb = table(180, species);
        let spark = lead(SPARK, 5);
        assert_eq!(spark.ability(&TABLES), Ability::Static);
        let (mut with, mut without) = (0, 0);
        for seed in 1..2000 {
            let enc = |l| {
                let mut rng = Rng::new(seed);
                try_land_encounter(
                    &tb,
                    Behavior::TallGrass,
                    Behavior::TallGrass,
                    l,
                    &TABLES,
                    &mut rng,
                )
            };
            with += (enc(Some(&spark)).unwrap().0 == SPARK) as u32;
            without += (enc(None).unwrap().0 == SPARK) as u32;
        }
        // Half the time the only Electric slot, else 1%: ~50.5% vs ~1%.
        assert!((900..1100).contains(&with), "{with}");
        assert!(without < 60, "{without}");
        // All Electric: no index draw, plain slot choice.
        let all = table(180, [SPARK; 12]);
        let mut a = Rng::new(9);
        let mut b = Rng::new(9);
        try_land_encounter(
            &all,
            Behavior::TallGrass,
            Behavior::TallGrass,
            Some(&spark),
            &TABLES,
            &mut a,
        );
        b.random(); // odds
        b.random(); // Static coin
        b.random(); // slot
        b.random(); // level
        assert_eq!(a, b);
    }

    #[test]
    fn keen_eye_skips_weak_encounters_half_the_time() {
        let tb = table(180, [ZIGZAGOON; 12]); // levels 2..=3
        let (mut strong, mut weak, mut low) = (0, 0, 0);
        let hawk20 = lead(HAWK, 20);
        let hawk7 = lead(HAWK, 7);
        let hawk5 = lead(HAWK, 5);
        for seed in 1..2000 {
            let enc = |l: &Pokemon| {
                let mut rng = Rng::new(seed);
                try_land_encounter(
                    &tb,
                    Behavior::TallGrass,
                    Behavior::TallGrass,
                    Some(l),
                    &TABLES,
                    &mut rng,
                )
            };
            weak += enc(&hawk20).is_some() as u32;
            // L7: 3 <= 2 fails for level 3, so only level-2 mons can be skipped.
            low += enc(&hawk7).is_some() as u32;
            // L5: never (playerMonLevel > 5 fails).
            strong += enc(&hawk5).is_some() as u32;
        }
        assert_eq!(strong, 1999);
        assert!((900..1100).contains(&weak), "{weak}");
        assert!((1400..1600).contains(&low), "{low}");
    }

    #[test]
    fn intimidate_filters_like_keen_eye() {
        let tb = table(180, [ZIGZAGOON; 12]); // levels 2..=3
        let lead = lead(GROWLER, 20);
        assert_eq!(lead.ability(&TABLES), Ability::Intimidate);
        let mut seen = 0;
        for seed in 1..2000 {
            let mut rng = Rng::new(seed);
            let got = try_land_encounter(
                &tb,
                Behavior::TallGrass,
                Behavior::TallGrass,
                Some(&lead),
                &TABLES,
                &mut rng,
            );
            // Rolls: odds, slot, level, then the IsAbilityAllowingEncounter coin.
            let mut r = Rng::new(seed);
            r.random();
            let slot = land_slot(r.random());
            let level = 2 + (r.random() % 2) as u8;
            let want = (!r.random().is_multiple_of(2)).then_some((tb.mons[slot].species, level));
            assert_eq!(got, want, "seed {seed}");
            assert_eq!(rng, r);
            seen += got.is_some() as u32;
        }
        assert!((900..1100).contains(&seen), "{seen}");
    }

    #[test]
    fn create_wild_is_deterministic_and_matches_its_rolls() {
        let a = create_wild(&TABLES, ZIGZAGOON, 3, None, 0xABCD_1234, &mut Rng::new(77));
        let b = create_wild(&TABLES, ZIGZAGOON, 3, None, 0xABCD_1234, &mut Rng::new(77));
        assert_eq!(a, b);
        // PickWildMonNature: random() % 25; CreateMonWithNature: random32() until it matches;
        // CreateBoxMon: two IV rolls.
        let mut r = Rng::new(77);
        let nature = r.random() as u32 % 25;
        let p = loop {
            let p = r.random32();
            if p % 25 == nature {
                break p;
            }
        };
        assert_eq!(a.personality, p);
        assert_eq!(a.nature(), Nature(nature as u8));
        let iv = r.random();
        assert_eq!(a.ivs[0], (iv & 31) as u8);
        assert_eq!((a.level, a.ot_id, a.hp), (3, 0xABCD_1234, a.stats[0]));
        let mut rng = Rng::new(77);
        create_wild(&TABLES, ZIGZAGOON, 3, None, 0, &mut rng);
        r.random();
        assert_eq!(rng, r);
        // Different seeds give different mons.
        let c = create_wild(&TABLES, ZIGZAGOON, 3, None, 0xABCD_1234, &mut Rng::new(78));
        assert_ne!(a, c);
    }

    #[test]
    fn synchronize_lead_passes_its_nature_half_the_time() {
        let mut blob = lead(BLOB, 5);
        blob.personality = 7; // NATURE_BRAVE
        assert_eq!(blob.ability(&TABLES), Ability::Synchronize);
        let mut same = 0;
        for seed in 1..1000 {
            let mut rng = Rng::new(seed);
            let coin = Rng::new(seed).random().is_multiple_of(2);
            let m = create_wild(&TABLES, ZIGZAGOON, 3, Some(&blob), 0, &mut rng);
            if coin {
                assert_eq!(m.nature(), Nature(7), "seed {seed}");
            }
            same += (m.nature() == Nature(7)) as u32;
        }
        // 1/2 + 1/2 * 1/25 = 52%.
        assert!((440..600).contains(&same), "{same}");
    }
}
