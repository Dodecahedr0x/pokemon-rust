use super::*;
use crate::battle::test_data::*;
use crate::data::Floor;
use crate::mon::{HoldEffect, Nature, Pokemon};

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

/// The encounter view of `p` (holding nothing the tests' game knows).
fn of(p: &Pokemon) -> Lead {
    Lead::of(Some(p), &TABLES, |_| None)
}

/// A lead with only an ability.
fn with(ability: Ability) -> Lead {
    Lead {
        ability,
        ..Lead::NONE
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
    assert_eq!(encounter_rate(20, Lead::NONE), 320);
    assert_eq!(encounter_rate(180, Lead::NONE), 2880);
    assert_eq!(encounter_rate(255, Lead::NONE), 2880);
    assert_eq!(encounter_rate(0, Lead::NONE), 0);
}

/// Gen3 `MetatileBehavior_IsLandWildEncounter`'s tiles: tall grass and the seven other
/// `TILE_FLAG_HAS_ENCOUNTERS` behaviours that are not water.
const ENCOUNTER_TILES: [Behavior; 8] = [
    Behavior::TallGrass,
    Behavior::Floor(Floor::LongGrass),
    Behavior::Floor(Floor::Unused05),
    Behavior::Floor(Floor::DeepSand),
    Behavior::Floor(Floor::Cave),
    Behavior::Floor(Floor::IndoorEncounter),
    Behavior::Floor(Floor::AshGrass),
    Behavior::Floor(Floor::Footprints),
];

/// Cells without land encounters, among them the two floors only the battle background reads.
const PLAIN_TILES: [Behavior; 6] = [
    Behavior::Normal,
    Behavior::Floor(Floor::Sand),
    Behavior::Floor(Floor::MountainTop),
    Behavior::Counter,
    Behavior::Ledge(crate::data::Dir::Down),
    Behavior::Blocked,
];

#[test]
fn land_encounter_tiles_are_gen3s() {
    for b in ENCOUNTER_TILES {
        assert!(b.land_encounter(), "{b:?}");
    }
    for b in PLAIN_TILES {
        assert!(!b.land_encounter(), "{b:?}");
    }
}

#[test]
fn only_encounter_tiles_draw() {
    let tb = table(20, [ZIGZAGOON; 12]);
    let mut rng = Rng::new(5);
    for prev in ENCOUNTER_TILES.into_iter().chain(PLAIN_TILES) {
        for cur in PLAIN_TILES {
            let r = try_land_encounter(&tb, prev, cur, Lead::NONE, &TABLES, &mut rng);
            assert_eq!(r, None);
        }
    }
    assert_eq!(rng, Rng::new(5));
}

/// Every encounter behaviour draws as tall grass does: the same rolls, the same mon.
#[test]
fn every_encounter_tile_rolls_as_tall_grass() {
    let tb = table(20, [ZIGZAGOON; 12]);
    for cur in ENCOUNTER_TILES {
        let mut hits = 0;
        for seed in 1..2000 {
            for new_cell in [false, true] {
                let (prev, prev_grass) = if new_cell {
                    (Behavior::Normal, Behavior::Normal)
                } else {
                    (cur, Behavior::TallGrass)
                };
                let mut a = Rng::new(seed);
                let mut b = Rng::new(seed);
                let got = try_land_encounter(&tb, prev, cur, Lead::NONE, &TABLES, &mut a);
                let want = try_land_encounter(
                    &tb,
                    prev_grass,
                    Behavior::TallGrass,
                    Lead::NONE,
                    &TABLES,
                    &mut b,
                );
                assert_eq!(got, want, "{cur:?} seed {seed}");
                assert_eq!(a, b, "{cur:?} seed {seed}");
                hits += got.is_some() as u32;
            }
        }
        assert!(hits > 0, "{cur:?}");
    }
}

/// Gen3's `prevMetatileBehavior != curMetatileBehavior` compares raw behaviours: any change of
/// behaviour (tall grass to cave floor, cave floor to long grass, sand to deep sand) draws the
/// 60% new-metatile gate first; the same behaviour again does not.
#[test]
fn new_tile_type_gate_compares_raw_behaviours() {
    let tb = table(180, [ZIGZAGOON; 12]); // the odds always pass
    let rolls = |prev: Behavior, cur: Behavior, seed: u32| {
        let mut rng = Rng::new(seed);
        let got = try_land_encounter(&tb, prev, cur, Lead::NONE, &TABLES, &mut rng);
        let mut n = 0;
        let mut r = Rng::new(seed);
        while r != rng {
            r.random();
            n += 1;
        }
        (got, n)
    };
    for cur in ENCOUNTER_TILES {
        for prev in ENCOUNTER_TILES.into_iter().chain(PLAIN_TILES) {
            for seed in 1..50 {
                let gate = Rng::new(seed).random() % 100;
                let (got, n) = rolls(prev, cur, seed);
                if prev == cur {
                    assert_eq!(n, 3, "{prev:?} -> {cur:?}: odds, slot, level");
                    assert!(got.is_some());
                } else if gate >= 60 {
                    assert_eq!((got, n), (None, 1), "{prev:?} -> {cur:?} seed {seed}");
                } else {
                    assert_eq!(n, 4, "{prev:?} -> {cur:?}: gate, odds, slot, level");
                    assert!(got.is_some());
                }
            }
        }
    }
}

#[test]
fn rate_modifiers_follow_wild_encounter_check() {
    // rate * 16 = 320; Cleanse Tag first (x2/3, floored), then the ability, then the cap.
    let tag = Lead {
        cleanse_tag: true,
        ..Lead::NONE
    };
    assert_eq!(encounter_rate(20, tag), 213);
    assert_eq!(encounter_rate(20, with(Ability::Stench)), 160);
    assert_eq!(encounter_rate(20, with(Ability::WhiteSmoke)), 160);
    assert_eq!(encounter_rate(20, with(Ability::Illuminate)), 640);
    assert_eq!(encounter_rate(20, with(Ability::ArenaTrap)), 640);
    let tag_stench = Lead {
        ability: Ability::Stench,
        cleanse_tag: true,
        ..Lead::NONE
    };
    assert_eq!(encounter_rate(20, tag_stench), 106);
    // The cap comes after the ability.
    assert_eq!(
        encounter_rate(100, with(Ability::Illuminate)),
        MAX_ENCOUNTER_RATE
    );
    assert_eq!(encounter_rate(255, with(Ability::Stench)), 2040);
    // Abilities with no rate effect (Sand Veil's needs a sandstorm: no field weather).
    for a in [
        Ability::KeenEye,
        Ability::Static,
        Ability::SandVeil,
        Ability::SandVeilGen3,
        Ability::Other,
    ] {
        assert_eq!(encounter_rate(20, with(a)), 320, "{a:?}");
    }
    // The odds roll uses the modified rate.
    let tb = table(20, [ZIGZAGOON; 12]);
    for seed in 1..3000 {
        let odds = (Rng::new(seed).random() % 2880) as u32;
        for lead in [with(Ability::Illuminate), with(Ability::Stench), tag] {
            let mut rng = Rng::new(seed);
            let got = try_land_encounter(
                &tb,
                Behavior::Floor(Floor::Cave),
                Behavior::Floor(Floor::Cave),
                lead,
                &TABLES,
                &mut rng,
            );
            assert_eq!(
                got.is_some(),
                odds < encounter_rate(20, lead),
                "seed {seed}"
            );
        }
    }
}

#[test]
fn cleanse_tag_is_read_from_the_lead_s_held_item() {
    let mut p = lead(ZIGZAGOON, 5);
    let tag = crate::data::ItemId(190);
    let hold = |i| (i == tag).then_some(HoldEffect::CleanseTag);
    assert!(!Lead::of(Some(&p), &TABLES, hold).cleanse_tag);
    p.held = Some(crate::data::ItemId(1));
    assert!(!Lead::of(Some(&p), &TABLES, hold).cleanse_tag);
    p.held = Some(tag);
    let l = Lead::of(Some(&p), &TABLES, hold);
    assert!(l.cleanse_tag);
    assert_eq!(l.level, 5);
    assert_eq!(Lead::of(None, &TABLES, hold), Lead::NONE);
}

#[test]
fn magnet_pull_lead_prefers_steel_slots_before_static() {
    let mut species = [ZIGZAGOON; 12];
    species[11] = STEEL; // the 1% slot
    species[10] = SPARK;
    let tb = table(180, species);
    let magnet = lead(MAGNET, 5);
    assert_eq!(magnet.ability(&TABLES), Ability::MagnetPull);
    let mut steel = 0;
    for seed in 1..2000 {
        let mut rng = Rng::new(seed);
        let got = try_land_encounter(
            &tb,
            Behavior::Floor(Floor::Cave),
            Behavior::Floor(Floor::Cave),
            of(&magnet),
            &TABLES,
            &mut rng,
        )
        .unwrap();
        // Rolls: odds, the Magnet Pull coin, then the Steel pick (one valid slot) or the plain
        // slot (no Static coin: the lead is not Static), then the level.
        let mut r = Rng::new(seed);
        r.random();
        let slot = if r.random().is_multiple_of(2) {
            r.random();
            11
        } else {
            land_slot(r.random())
        };
        r.random();
        assert_eq!(got.0, tb.mons[slot].species, "seed {seed}");
        assert_eq!(rng, r, "seed {seed}");
        steel += (got.0 == STEEL) as u32;
    }
    assert!((900..1100).contains(&steel), "{steel}");
}

#[test]
fn hustle_like_leads_get_the_top_level_half_the_time() {
    let mut tb = table(180, [ZIGZAGOON; 12]);
    for m in &mut tb.mons {
        m.min_level = 10;
        m.max_level = 14;
    }
    for ability in [Ability::Hustle, Ability::VitalSpirit, Ability::Pressure] {
        let mut top = 0;
        for seed in 1..2000 {
            let mut rng = Rng::new(seed);
            let got = try_land_encounter(
                &tb,
                Behavior::TallGrass,
                Behavior::TallGrass,
                with(ability),
                &TABLES,
                &mut rng,
            )
            .unwrap();
            let mut r = Rng::new(seed);
            r.random(); // odds
            r.random(); // slot
            let roll = (r.random() % 5) as u8;
            let level = if r.random().is_multiple_of(2) {
                14
            } else {
                10 + roll.saturating_sub(1)
            };
            assert_eq!(got.1, level, "{ability:?} seed {seed}");
            assert_eq!(rng, r);
            top += (got.1 == 14) as u32;
        }
        // Half the time the top level; the other half never (a roll of 4 is lowered to 3).
        assert!((900..1100).contains(&top), "{ability:?} {top}");
    }
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
            let got = try_land_encounter(
                &tb,
                prev,
                Behavior::TallGrass,
                Lead::NONE,
                &TABLES,
                &mut rng,
            );
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
        with += (enc(of(&spark)).unwrap().0 == SPARK) as u32;
        without += (enc(Lead::NONE).unwrap().0 == SPARK) as u32;
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
        of(&spark),
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
                of(l),
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
            of(&lead),
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
