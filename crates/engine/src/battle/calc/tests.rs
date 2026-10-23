use super::*;

/// A battler with explicit stats (HP, Atk, Def, Speed, SpAtk, SpDef).
fn fighter(level: u8, stats: [u16; 6], types: [Type; 2]) -> Fighter {
    Fighter {
        level,
        hp: stats[0],
        max_hp: stats[0],
        stats,
        stages: [0; 8],
        types,
        ability: Ability::None,
        status: Status::None,
        hold: None,
    }
}

const NORMAL: [Type; 2] = [Type::Normal, Type::Normal];

// Mudkip L5 (Atk 13) vs Zigzagoon L2 (Def 6); Tackle: Normal, power 35.
fn mudkip() -> Fighter {
    fighter(5, [20, 13, 10, 9, 9, 10], [Type::Water; 2])
}
fn zigzagoon() -> Fighter {
    fighter(2, [13, 6, 6, 8, 5, 5], NORMAL)
}

#[test]
fn tackle_golden() {
    // CalculateBaseDamage: 13 * 35 * (2*5/5 + 2) / 6 / 50 + 2 = 1820 / 6 = 303 / 50 = 6 + 2.
    let base = base_damage(&mudkip(), &zigzagoon(), 35, Type::Normal, false);
    assert_eq!(base, 8);
    // ApplyRandomDmgMultiplier: r=0 → ×100/100, r=15 → ×85/100.
    assert_eq!(random_damage(base, 0), 8);
    assert_eq!(random_damage(base, 15), 6);
    assert_eq!(random_damage(base, 16), 8, "roll % 16");
    // Cmd_damagecalc: × gCritMultiplier (2) before typecalc/random.
    let crit = base_damage(&mudkip(), &zigzagoon(), 35, Type::Normal, true) * 2;
    assert_eq!(crit, 16);
    assert_eq!(random_damage(crit, 0), 16);
    assert_eq!(random_damage(crit, 15), 13);
}

#[test]
fn ember_stab_super_effective_golden() {
    // Torchic L5 SpA 13 (base 70, IV 31, Hardy) Ember (Fire, 40) vs Wurmple L3 SpD 7 (base 30,
    // IV 7: 67 * 3 / 100 + 5; IV 0 would give 6).
    let torchic = fighter(5, [20, 12, 9, 10, 13, 10], [Type::Fire; 2]);
    let wurmple = fighter(3, [15, 7, 7, 6, 5, 7], [Type::Bug; 2]);
    // 13 * 40 * 4 / 7 / 50 + 2 = 2080 / 7 = 297 / 50 = 5 + 2 = 7.
    let base = base_damage(&torchic, &wurmple, 40, Type::Fire, false);
    assert_eq!(base, 7);
    let d = stab(base);
    assert_eq!(d, 10);
    let chart = [TypeMatchup {
        attack: Type::Fire,
        defend: Type::Bug,
        multiplier: 20,
    }];
    let r = type_effect(&chart, 1, false, Type::Fire, wurmple.types, 40, d);
    assert_eq!(
        r,
        TypeResult {
            damage: 20,
            eff: Effectiveness::Super
        }
    );
    assert_eq!(random_damage(r.damage, 0), 20);
    assert_eq!(random_damage(r.damage, 15), 17);
}

#[test]
fn stages_change_base_damage_and_crit_ignores_bad_ones() {
    let mut a = mudkip();
    a.stages[Stat::Atk as usize] = -1;
    // Growl -1: Atk 13 * 10 / 15 = 8; 8 * 35 * 4 / 6 / 50 + 2 = 1120 / 6 = 186 / 50 = 3 + 2.
    assert_eq!(apply_stage(13, -1), 8);
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 5);
    // A crit with a -1 attacker uses the raw stat: same base as no stages.
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, true), 8);
    // +2 attack is kept on a crit: 13 * 20 / 10 = 26; 26*35*4 = 3640 / 6 = 606 / 50 = 12 + 2.
    a.stages[Stat::Atk as usize] = 2;
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, true), 14);
    // Defender +1 Def: ignored on a crit, applied otherwise (6 * 15 / 10 = 9).
    let mut d = zigzagoon();
    d.stages[Stat::Def as usize] = 1;
    // 1820 / 9 = 202 / 50 = 4 + 2.
    assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, false), 6);
    assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, true), 8);
    // Defender -1 Def is kept on a crit: 6 * 10 / 15 = 4; 1820 / 4 = 455 / 50 = 9 + 2.
    d.stages[Stat::Def as usize] = -1;
    assert_eq!(base_damage(&mudkip(), &d, 35, Type::Normal, true), 11);
}

#[test]
fn burn_guts_pinch_and_minimum() {
    let mut a = mudkip();
    a.status = Status::Burn;
    // 6 / 2 = 3 + 2.
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 5);
    // Guts: no burn halving, Atk ×1.5 = 19: 19*35*4 = 2660 / 6 = 443 / 50 = 8 + 2.
    a.ability = Ability::Guts;
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 10);
    // Physical minimum 1: a weak hit is 1 + 2; special has no minimum (0 + 2).
    let weak = fighter(1, [10, 1, 1, 1, 1, 1], NORMAL);
    let wall = fighter(50, [200, 200, 200, 200, 200, 200], NORMAL);
    assert_eq!(base_damage(&weak, &wall, 10, Type::Normal, false), 3);
    assert_eq!(base_damage(&weak, &wall, 10, Type::Water, false), 2);
    // Mystery skips both branches.
    assert_eq!(
        base_damage(&mudkip(), &zigzagoon(), 35, Type::Mystery, false),
        2
    );
    // Torrent at HP <= max/3: power 40 → 60 for Water. SpA 9: 9*60*4 = 2160 / 5 = 432 / 50
    // = 8 + 2; at full HP: 9*40*4 = 1440 / 5 = 288 / 50 = 5 + 2.
    let mut t = mudkip();
    t.ability = Ability::Torrent;
    assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 7);
    t.hp = 7; // 7 > 20 / 3
    assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 7);
    t.hp = 6; // 6 <= 20 / 3
    assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Water, false), 10);
    assert_eq!(base_damage(&t, &zigzagoon(), 40, Type::Fire, false), 7);
}

#[test]
fn effectiveness_rounds_per_entry_in_table_order() {
    // gTypeEffectiveness lists FIRE→WATER (×0.5) before FIRE→GRASS (×2). On a Water/Grass
    // defender with 5 damage: 5*5/10 = 2, then 2*20/10 = 4. The other order gives 10 → 5.
    let water_first = [
        TypeMatchup {
            attack: Type::Fire,
            defend: Type::Water,
            multiplier: 5,
        },
        TypeMatchup {
            attack: Type::Fire,
            defend: Type::Grass,
            multiplier: 20,
        },
    ];
    let grass_first = [water_first[1], water_first[0]];
    let def = [Type::Water, Type::Grass];
    let a = type_effect(&water_first, 2, false, Type::Fire, def, 40, 5);
    let b = type_effect(&grass_first, 2, false, Type::Fire, def, 40, 5);
    assert_eq!(
        a,
        TypeResult {
            damage: 4,
            eff: Effectiveness::Normal
        }
    );
    assert_eq!(
        b,
        TypeResult {
            damage: 5,
            eff: Effectiveness::Normal
        }
    );
    // Both flags on the same type: a mono-type defender is matched once.
    let r = type_effect(&water_first, 2, false, Type::Fire, [Type::Water; 2], 40, 5);
    assert_eq!(r.damage, 2);
    assert_eq!(r.eff, Effectiveness::NotVery);
    // Minimum 1 unless ×0; no flags for power 0.
    let r = type_effect(&water_first, 2, false, Type::Fire, [Type::Water; 2], 0, 1);
    assert_eq!(
        r,
        TypeResult {
            damage: 1,
            eff: Effectiveness::Normal
        }
    );
}

#[test]
fn immunity_and_foresight_split() {
    let chart = [
        TypeMatchup {
            attack: Type::Normal,
            defend: Type::Rock,
            multiplier: 5,
        },
        // TYPE_FORESIGHT marker here: entries from index 1 on are skipped under Foresight.
        TypeMatchup {
            attack: Type::Normal,
            defend: Type::Ghost,
            multiplier: 0,
        },
    ];
    let ghost = [Type::Ghost, Type::Ghost];
    let r = type_effect(&chart, 1, false, Type::Normal, ghost, 35, 8);
    assert_eq!(
        r,
        TypeResult {
            damage: 0,
            eff: Effectiveness::NoEffect
        }
    );
    let r = type_effect(&chart, 1, true, Type::Normal, ghost, 35, 8);
    assert_eq!(r.damage, 8);
    assert_eq!(r.eff, Effectiveness::Normal);
    assert_eq!(random_damage(0, 3), 0);
}

#[test]
fn crit_chance_table() {
    assert_eq!(crit_chance_stage(false), 0);
    assert_eq!(crit_chance_stage(true), 2);
    assert!(is_crit(0, 0));
    assert!(is_crit(0, 32));
    assert!(!is_crit(0, 15));
    assert!(is_crit(2, 4));
    assert!(!is_crit(2, 5));
    assert!(is_crit(9, 2), "stages clamp to the last entry (1 in 2)");
}

#[test]
fn accuracy_stage_ratios() {
    // 95% at stage 0: roll % 100 + 1 > 95 misses.
    assert!(accuracy_hits(95, 0, 0, 94));
    assert!(!accuracy_hits(95, 0, 0, 95));
    assert!(accuracy_hits(100, 0, 0, 99));
    // -1 accuracy: 75 * 100 / 100 = 75.
    assert!(accuracy_hits(100, -1, 0, 74));
    assert!(!accuracy_hits(100, -1, 0, 75));
    // +1 evasion is the same stage as -1 accuracy.
    assert!(!accuracy_hits(100, 0, 1, 75));
    // +6 accuracy vs -6 evasion clamps to +6: 3/1 * 50 = 150 always hits.
    assert!(accuracy_hits(50, 6, -6, 99));
    // Accuracy 0 never hits (Gen3 uses an effect to skip the check).
    assert!(!accuracy_hits(0, 0, 0, 0));
}

#[test]
fn exp_goldens() {
    // Cmd_getexp: wild Zigzagoon (expYield 60) L2 → 60 * 2 / 7 = 17.
    assert_eq!(exp_yield(60, 2, false, 1), 17);
    // Trainer Poochyena (expYield 55) L5 → 55 * 5 / 7 = 39, ×150/100 = 58.
    assert_eq!(exp_yield(55, 5, false, 1), 39);
    assert_eq!(exp_yield(55, 5, true, 1), 58);
    // Split among two participants (17 / 2 = 8), minimum 1.
    assert_eq!(exp_yield(60, 2, false, 2), 8);
    assert_eq!(exp_yield(1, 1, false, 1), 1);
    assert_eq!(exp_yield(1, 1, true, 1), 1);
}

#[test]
fn run_goldens() {
    // TryRunFromBattle: 9 * 128 / 10 = 115.
    assert_eq!(run_threshold(9, 10, 0), Some(115));
    assert!(run_succeeds(9, 10, 0, 114));
    assert!(!run_succeeds(9, 10, 0, 115));
    assert!(run_succeeds(9, 10, 0, 0x100), "roll & 0xFF");
    // 5 tries: (115 + 150) mod 256 = 9.
    assert_eq!(run_threshold(9, 10, 5), Some(9));
    // Faster or equal always flees, without a roll.
    assert_eq!(run_threshold(10, 10, 0), None);
    assert_eq!(run_threshold(11, 10, 0), None);
    assert!(run_succeeds(10, 10, 0, 0xFF));
}

#[test]
fn catch_goldens() {
    // Zigzagoon catch rate 255, 13/13 HP: 255 * (39 - 26) / 39 = 85.
    let odds = catch_odds(255, 13, 13, Status::None);
    assert_eq!(odds, 85);
    // 16711680 / 85 = 196608; Sqrt → 443; Sqrt → 21; 1048560 / 21 = 49931.
    assert_eq!(isqrt(196_608), 443);
    assert_eq!(isqrt(443), 21);
    assert_eq!(shake_threshold(odds), 49931);
    // Rolls below the threshold shake; the first roll at or above stops.
    let mut rolls = [49930u16, 0, 49931, 0].into_iter();
    assert_eq!(catch_shakes(odds, || rolls.next().unwrap()), 2);
    assert_eq!(catch_shakes(odds, || 0), 4);
    assert_eq!(catch_shakes(255, || panic!("no roll above 254")), 4);
    // Status multipliers.
    assert_eq!(catch_odds(255, 13, 13, Status::Sleep(2)), 170);
    assert_eq!(catch_odds(255, 13, 13, Status::Paralysis), 127);
    // 1 HP of 13: 255 * (39 - 2) / 39 = 241.
    assert_eq!(catch_odds(255, 1, 13, Status::None), 241);
}

#[test]
fn isqrt_floors() {
    for n in [0u32, 1, 2, 3, 4, 15, 16, 17, 99, 100, 101, u32::MAX] {
        let r = isqrt(n) as u64;
        assert!(r * r <= n as u64 && (r + 1) * (r + 1) > n as u64, "{n}");
    }
}

#[test]
fn effective_speed_quarters_paralysis() {
    let mut f = mudkip();
    assert_eq!(effective_speed(&f), 9);
    f.stages[Stat::Speed as usize] = 1;
    assert_eq!(effective_speed(&f), 13);
    f.status = Status::Paralysis;
    assert_eq!(effective_speed(&f), 3);
}

#[test]
fn power_and_defense_abilities_golden() {
    // CalculateBaseDamage, in its order: Huge Power ×2 attack first (13 → 26):
    // 26*35*4 = 3640 / 6 = 606 / 50 = 12 + 2.
    let mut a = mudkip();
    a.ability = Ability::HugePower;
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 14);
    a.ability = Ability::PurePower;
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 14);
    // Hustle ×150/100 attack (13 → 19): 19*35*4 = 2660 / 6 = 443 / 50 = 8 + 2.
    a.ability = Ability::Hustle;
    assert_eq!(base_damage(&a, &zigzagoon(), 35, Type::Normal, false), 10);
    // Thick Fat (defender) halves the special attack for Fire and Ice (13 → 6):
    // 6*40*4 = 960 / 5 = 192 / 50 = 3 + 2; 13*40*4 = 2080 / 5 = 416 / 50 = 8 + 2.
    let torchic = fighter(5, [20, 12, 9, 10, 13, 10], [Type::Fire; 2]);
    let mut fat = zigzagoon();
    fat.ability = Ability::ThickFat;
    assert_eq!(base_damage(&torchic, &fat, 40, Type::Fire, false), 5);
    assert_eq!(base_damage(&torchic, &fat, 40, Type::Ice, false), 5);
    assert_eq!(base_damage(&torchic, &fat, 40, Type::Water, false), 10);
    // Marvel Scale ×150/100 defense with a status (6 → 9): 1820 / 9 = 202 / 50 = 4 + 2.
    let mut scale = zigzagoon();
    scale.ability = Ability::MarvelScale;
    assert_eq!(base_damage(&mudkip(), &scale, 35, Type::Normal, false), 8);
    scale.status = Status::Poison;
    assert_eq!(base_damage(&mudkip(), &scale, 35, Type::Normal, false), 6);
}

#[test]
fn sports_halve_power_before_the_pinch_abilities() {
    // Mud Sport: Electric power / 2 (40 → 20): SpA 9: 9*20*4 = 720 / 5 = 144 / 50 = 2 + 2.
    let a = mudkip();
    let mud = Field {
        mud_sport: true,
        water_sport: false,
    };
    let water = Field {
        mud_sport: false,
        water_sport: true,
    };
    let z = zigzagoon();
    assert_eq!(base_damage(&a, &z, 40, Type::Electric, false), 7);
    assert_eq!(base_damage_in(&a, &z, 40, Type::Electric, false, mud), 4);
    assert_eq!(base_damage_in(&a, &z, 40, Type::Fire, false, mud), 7);
    // Water Sport: Fire power / 2, before Blaze's ×150/100 at low HP: 43 / 2 = 21 → 31
    // (the other order gives 64 / 2 = 32). SpA 100: 100*31*4 = 12400 / 5 = 2480 / 50 = 49 + 2.
    let mut b = fighter(5, [20, 13, 10, 9, 100, 10], [Type::Water; 2]);
    b.ability = Ability::Blaze;
    b.hp = 1;
    assert_eq!(base_damage_in(&b, &z, 43, Type::Fire, false, water), 51);
    // Not Electric: 100*43*4 = 17200 / 5 = 3440 / 50 = 68 + 2.
    assert_eq!(base_damage_in(&b, &z, 43, Type::Electric, false, water), 70);
}

#[test]
fn accuracy_abilities() {
    // Compound Eyes ×130/100 after the stage ratio: 70 → 91.
    assert!(accuracy_hits_by(70, 0, 0, Ability::CompoundEyes, false, 90));
    assert!(!accuracy_hits_by(
        70,
        0,
        0,
        Ability::CompoundEyes,
        false,
        91
    ));
    // Hustle ×80/100 on physical moves only: 100 → 80.
    assert!(accuracy_hits_by(100, 0, 0, Ability::Hustle, true, 79));
    assert!(!accuracy_hits_by(100, 0, 0, Ability::Hustle, true, 80));
    assert!(accuracy_hits_by(100, 0, 0, Ability::Hustle, false, 99));
    assert!(!accuracy_hits_by(70, 0, 0, Ability::None, false, 70));
}
