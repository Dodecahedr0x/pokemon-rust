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
            let got = try_land_encounter(&tb, prev, Behavior::TallGrass, None, &TABLES, &mut rng);
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
