use super::*;
use crate::battle::test_data::*;

fn new(species: SpeciesId, level: u8, ivs: Ivs, personality: u32) -> Pokemon {
    Pokemon::new(
        &TABLES,
        species,
        level,
        ivs,
        personality,
        0,
        &mut Rng::new(1),
    )
}

#[test]
fn mudkip_l5_adamant_stats() {
    // CalculateMonStats: HP ((2*50+10)*5)/100 + 5 + 10 = 20; Atk (150*5/100 + 5) = 12,
    // *110/100 = 13; Def 110*5/100 + 5 = 10; Spe 90*5/100 + 5 = 9; SpA 10 *90/100 = 9;
    // SpD 10. Personality 3: NATURE_ADAMANT (+Atk -SpA).
    let m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
    assert_eq!(m.nature(), Nature(3));
    assert_eq!(m.stats, [20, 13, 10, 9, 9, 10]);
    assert_eq!(m.hp, 20);
    assert_eq!(m.level, 5);
    assert_eq!(m.ivs, [10; 6]);
    assert_eq!(m.evs, [0; 6]);
    // gExperienceTables[GROWTH_MEDIUM_SLOW][5] = 6*125/5 - 15*25 + 500 - 140 = 135.
    assert_eq!(m.exp, 135);
    assert_eq!(m.friendship, 70);
    assert_eq!(&m.nickname[..7], MUDKIP_NAME);
    assert_eq!(m.nickname[7..], [0xFF; 4]);
    assert_eq!(m.moves, [Some(TACKLE), Some(GROWL), None, None]);
    assert_eq!(m.pp, [35, 40, 0, 0]);
    assert_eq!(m.status, Status::None);
    assert_eq!((m.personality, m.ot_id, m.pp_bonuses), (3, 0, 0));
}

#[test]
fn zigzagoon_l2_hardy_stats() {
    // HP (76*2)/100 + 2 + 10 = 13; Def (82*2)/100 + 5 = 6.
    let m = new(ZIGZAGOON, 2, Ivs::Fixed(0), 0);
    assert_eq!(m.stats[Stat::Hp as usize], 13);
    assert_eq!(m.stats[Stat::Def as usize], 6);
    assert_eq!(m.exp, 8);
    assert_eq!(m.moves, [Some(TACKLE), None, None, None]);
}

#[test]
fn random_ivs_use_two_rolls_of_three_five_bit_fields() {
    // CreateBoxMon: Random() -> HP | Atk << 5 | Def << 10, then Spe | SpA << 5 | SpD << 10.
    let mut r = Rng::new(99);
    let (a, b) = (r.random(), r.random());
    let m = Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Random, 0, 0, &mut Rng::new(99));
    let f = |v: u16, i: u16| ((v >> (5 * i)) & 31) as u8;
    assert_eq!(
        m.ivs,
        [f(a, 0), f(a, 1), f(a, 2), f(b, 0), f(b, 1), f(b, 2)]
    );
    // Fixed IVs draw nothing.
    let mut r = Rng::new(99);
    Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Fixed(31), 0, 0, &mut r);
    assert_eq!(r, Rng::new(99));
    // `fixedIV >= USE_RANDOM_IVS` (32) draws random IVs: the same two rolls.
    let mut r = Rng::new(99);
    let f32 = Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Fixed(32), 0, 0, &mut r);
    assert_eq!(f32.ivs, m.ivs);
    let mut two = Rng::new(99);
    two.random();
    two.random();
    assert_eq!(r, two);
}

#[test]
fn nickname_is_the_species_name_truncated_to_10() {
    // GetSpeciesName copies at most POKEMON_NAME_LENGTH (10) characters, then EOS.
    let m = new(GROWLER, 5, Ivs::Fixed(0), 0);
    assert_eq!(m.nickname[..10], LONG_NAME[..10]);
    assert_eq!(m.nickname[10], 0xFF);
}

#[test]
fn recalc_after_exp_changes_finds_the_level_either_way() {
    // Scanning starts at the current level when reached; a lower exp rescans from 1.
    let mut m = new(MUDKIP, 10, Ivs::Fixed(0), 0);
    m.exp = exp_for_level(GrowthRate::MediumSlow, 30);
    m.recalc_stats(&TABLES);
    assert_eq!(m.level, 30);
    m.exp = exp_for_level(GrowthRate::MediumSlow, 7) + 1;
    m.recalc_stats(&TABLES);
    assert_eq!(m.level, 7);
}

#[test]
fn initial_moveset_shifts_out_the_first_move_when_full() {
    // GiveBoxMonInitialMoveset: L1 Tackle, L1 Growl, L2 Mud-Slap, L3 Water Gun fill the four
    // slots; L4 Bide shifts Tackle out; L5 Tail Whip shifts Growl out; L5 Water Gun is
    // already known (skipped); L9 Growl is above the level.
    let m = new(SPARK, 5, Ivs::Fixed(0), 0);
    assert_eq!(
        m.moves,
        [Some(MUD_SLAP), Some(WATER_GUN), Some(BIDE), Some(TAIL_WHIP)]
    );
    assert_eq!(m.pp, [10, 25, 10, 30]);
    let m = new(SPARK, 4, Ivs::Fixed(0), 0);
    assert_eq!(
        m.moves,
        [Some(GROWL), Some(MUD_SLAP), Some(WATER_GUN), Some(BIDE)]
    );
}

#[test]
fn ability_slot_follows_personality_bit_when_the_species_has_two() {
    let a = new(SPARK, 5, Ivs::Fixed(0), 0);
    assert_eq!((a.ability_slot, a.ability(&TABLES)), (0, Ability::Static));
    let b = new(SPARK, 5, Ivs::Fixed(0), 1);
    assert_eq!(
        (b.ability_slot, b.ability(&TABLES)),
        (1, Ability::Synchronize)
    );
    // No second ability: slot stays 0 (CreateBoxMon only sets it when abilities[1] != 0).
    let c = new(MUDKIP, 5, Ivs::Fixed(0), 1);
    assert_eq!((c.ability_slot, c.ability(&TABLES)), (0, Ability::Torrent));
}

#[test]
fn natures_are_personality_mod_25() {
    let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
    for (p, n) in [(0, 0), (24, 24), (25, 0), (3, 3), (u32::MAX, 20)] {
        m.personality = p;
        assert_eq!(m.nature(), Nature(n), "personality {p:#x}");
    }
}

#[test]
fn gender_thresholds() {
    // GetGenderFromSpeciesAndPersonality: ratio > (p & 0xFF) is female.
    let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
    for (p, g) in [
        (30, Gender::Female),
        (31, Gender::Male),
        (0x100, Gender::Female),
        (0xFF, Gender::Male),
    ] {
        m.personality = p;
        assert_eq!(m.gender(&TABLES), g, "personality {p:#x}");
    }
    let mut z = new(ZIGZAGOON, 5, Ivs::Fixed(0), 126);
    assert_eq!(z.gender(&TABLES), Gender::Female);
    z.personality = 127;
    assert_eq!(z.gender(&TABLES), Gender::Male);
    // MON_MALE (0), MON_FEMALE (254), MON_GENDERLESS (255) ignore the personality.
    assert_eq!(
        new(SPARK, 5, Ivs::Fixed(0), 0).gender(&TABLES),
        Gender::Male
    );
    assert_eq!(
        new(HAWK, 5, Ivs::Fixed(0), 0xFF).gender(&TABLES),
        Gender::Female
    );
    assert_eq!(
        new(BLOB, 5, Ivs::Fixed(0), 0).gender(&TABLES),
        Gender::Genderless
    );
}

#[test]
fn shiny_when_the_xor_of_halves_is_below_8() {
    let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
    // HIHALF(ot) ^ LOHALF(ot) ^ HIHALF(p) ^ LOHALF(p): 0x1234 ^ 0x5678 ^ 0x1234 ^ 0x567F = 7.
    m.ot_id = 0x1234_5678;
    m.personality = 0x1234_567F;
    assert!(m.is_shiny());
    m.personality = 0x1234_5670; // 8
    assert!(!m.is_shiny());
    m.personality = 0x1234_5678; // 0
    assert!(m.is_shiny());
}

#[test]
fn level_up_keeps_the_hp_offset_and_learns_mud_slap() {
    // Mudkip L5 (135 exp) gains 50: 185 >= 179 (L6), so exp = 179, 6 left, then 185 < 236.
    // L6 stats: HP 110*6/100 + 6 + 10 = 22, Atk (150*6/100 + 5) * 110/100 = 15,
    // Def 110*6/100 + 5 = 11, Spe 90*6/100 + 5 = 10, SpA 11 * 90/100 = 9, SpD 11.
    let mut m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
    m.hp = 15;
    let mut exp = 50;
    assert!(m.add_exp_step(&TABLES, &mut exp));
    assert_eq!((m.level, m.exp, exp), (6, 179, 6));
    assert_eq!(m.stats, [22, 15, 11, 10, 9, 11]);
    assert_eq!(m.hp, 17);
    let learned: alloc::vec::Vec<_> = m.moves_at_level(&TABLES, m.level).collect();
    assert_eq!(learned, [MUD_SLAP]);
    assert_eq!(m.try_learn(&TABLES, MUD_SLAP), Learn::Learned(2));
    assert_eq!(m.pp[2], 10);
    assert!(!m.add_exp_step(&TABLES, &mut exp));
    assert_eq!((m.level, m.exp, exp), (6, 185, 0));
    // A fainted mon stays at 0 HP through level-ups (CalculateMonStats).
    m.hp = 0;
    let mut exp = 1000;
    while m.add_exp_step(&TABLES, &mut exp) {}
    assert_eq!(m.hp, 0);
    assert!(m.level > 6);
}

#[test]
fn exp_gain_stops_at_max_level() {
    let mut m = new(ZIGZAGOON, 100, Ivs::Fixed(0), 0);
    assert_eq!(m.exp, 1_000_000);
    let mut exp = 500;
    assert!(!m.add_exp_step(&TABLES, &mut exp));
    assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 0));
    // 99 -> 100: 970299 + 1000000 >= 1000000; 29701 used, the rest is dropped next step.
    let mut m = new(ZIGZAGOON, 99, Ivs::Fixed(0), 0);
    let mut exp = 1_000_000;
    assert!(m.add_exp_step(&TABLES, &mut exp));
    assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 1_000_000 - 29_701));
    assert!(!m.add_exp_step(&TABLES, &mut exp));
    assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 0));
}

#[test]
fn try_learn_and_replace() {
    let mut m = new(SPARK, 5, Ivs::Fixed(0), 0);
    assert_eq!(m.try_learn(&TABLES, BIDE), Learn::AlreadyKnown);
    assert_eq!(m.try_learn(&TABLES, TACKLE), Learn::Full);
    m.pp_bonuses = 0xFF;
    m.pp[1] = 3;
    m.replace_move(&TABLES, 1, TACKLE);
    assert_eq!(m.moves[1], Some(TACKLE));
    assert_eq!(m.pp[1], 35);
    // RemoveMonPPBonus clears the slot's two bits.
    assert_eq!(m.pp_bonuses, 0b1111_0011);
    let mut z = new(ZIGZAGOON, 2, Ivs::Fixed(0), 0);
    assert_eq!(z.try_learn(&TABLES, TACKLE), Learn::AlreadyKnown);
    assert_eq!(z.try_learn(&TABLES, GROWL), Learn::Learned(1));
    assert_eq!(z.pp[1], 40);
}

#[test]
fn heal_restores_hp_pp_with_bonuses_and_status() {
    let mut m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
    m.hp = 1;
    m.pp = [0, 0, 0, 0];
    m.status = Status::Poison;
    // CalculatePPWithBonus: slot 0 has 3 PP Ups: 35 + 35*20*3/100 = 56; slot 1 has 1:
    // 40 + 40*20/100 = 48.
    m.pp_bonuses = 0b0000_0111;
    m.heal(&TABLES);
    assert_eq!(m.hp, 20);
    assert_eq!(m.pp, [56, 48, 0, 0]);
    assert_eq!(m.status, Status::None);
}

#[test]
fn ev_gain_respects_both_caps() {
    // MonGainEVs, BLOB yields 3 HP, 2 Atk.
    let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
    m.gain_evs(&TABLES, BLOB);
    assert_eq!(m.evs, [3, 2, 0, 0, 0, 0]);
    // Per-stat cap 255.
    m.evs = [254, 0, 0, 0, 0, 0];
    m.gain_evs(&TABLES, BLOB);
    assert_eq!(m.evs, [255, 2, 0, 0, 0, 0]);
    // Total cap 510: 509 + min(3, 1) on HP, then the loop stops.
    m.evs = [100, 255, 154, 0, 0, 0];
    m.gain_evs(&TABLES, BLOB);
    assert_eq!(m.evs, [101, 255, 154, 0, 0, 0]);
    m.evs = [255, 255, 0, 0, 0, 0];
    m.gain_evs(&TABLES, BLOB);
    assert_eq!(m.evs, [255, 255, 0, 0, 0, 0]);
    // EVs only count once the stats are recalculated:
    // HP ((100 + 0 + 252/4) * 5)/100 + 15 = 23.
    m.evs = [252, 0, 0, 0, 0, 0];
    let hp = m.stats[0];
    m.recalc_stats(&TABLES);
    assert_eq!((hp, m.stats[0]), (20, 23));
}

#[test]
fn exp_tables_hand_values() {
    use GrowthRate::*;
    // EXP_MEDIUM_SLOW(n) = 6n^3/5 - 15n^2 + 100n - 140.
    assert_eq!(
        [5, 6, 7, 8].map(|l| exp_for_level(MediumSlow, l)),
        [135, 179, 236, 314]
    );
    assert_eq!(
        [2, 3, 5].map(|l| exp_for_level(MediumFast, l)),
        [8, 27, 125]
    );
    // Levels 0 and 1 are 0 and 1 in every table (MediumSlow L1 would be -54).
    for g in [MediumFast, Erratic, Fluctuating, MediumSlow, Fast, Slow] {
        assert_eq!((exp_for_level(g, 0), exp_for_level(g, 1)), (0, 1));
    }
    // EXP_ERRATIC(60) = (150 - 60) * 216000 / 100 = 194400;
    // EXP_ERRATIC(80) = ((1911 - 800) / 3) * 512000 / 500 = 370 * 1024 = 378880.
    assert_eq!(exp_for_level(Erratic, 60), 194_400);
    assert_eq!(exp_for_level(Erratic, 80), 378_880);
    assert_eq!(exp_for_level(Erratic, 100), 600_000);
    // EXP_FLUCTUATING(20) = (20 + 14) * 8000 / 50 = 5440; (10) = (11/3 + 24) * 1000 / 50 = 540.
    assert_eq!(exp_for_level(Fluctuating, 20), 5440);
    assert_eq!(exp_for_level(Fluctuating, 10), 540);
    assert_eq!(exp_for_level(Fluctuating, 100), 1_640_000);
    // EXP_FAST(10) = 4000/5 = 800, EXP_SLOW(10) = 5000/4 = 1250.
    assert_eq!(exp_for_level(Fast, 10), 800);
    assert_eq!(exp_for_level(Slow, 10), 1250);
    assert_eq!(exp_for_level(MediumSlow, 100), 1_059_860);
}

#[test]
fn level_for_exp_inverts_the_table() {
    use GrowthRate::*;
    assert_eq!(level_for_exp(MediumSlow, 134), 4);
    assert_eq!(level_for_exp(MediumSlow, 135), 5);
    assert_eq!(level_for_exp(MediumSlow, 178), 5);
    assert_eq!(level_for_exp(MediumFast, u32::MAX), 100);
    assert_eq!(level_for_exp(MediumFast, 1), 1);
}

/// Every level of every rate against `gExperienceTables`, evaluated from the decomp's
/// `experience_tables.h` by a small C-expression evaluator (skipped without `PKR_DECOMP`).
#[test]
fn exp_tables_match_the_decomp() {
    let Ok(dir) = std::env::var("PKR_DECOMP") else {
        return;
    };
    let src = std::fs::read_to_string(
        std::path::Path::new(&dir).join("src/data/pokemon/experience_tables.h"),
    )
    .unwrap();
    let tables = decomp_exp::tables(&src);
    let rates = [
        GrowthRate::MediumFast,
        GrowthRate::Erratic,
        GrowthRate::Fluctuating,
        GrowthRate::MediumSlow,
        GrowthRate::Fast,
        GrowthRate::Slow,
    ];
    assert!(tables.len() >= 6, "{} rows", tables.len());
    for (g, row) in rates.iter().zip(&tables) {
        assert_eq!(row.len(), 101, "{g:?}");
        for (level, &want) in row.iter().enumerate() {
            assert_eq!(
                exp_for_level(*g, level as u8) as i64,
                want,
                "{g:?} L{level}"
            );
        }
    }
}

/// Test-side oracle: macros (`#define NAME(n) body`, with `\` continuations) and the
/// `gExperienceTables` rows, evaluated with C integer semantics.
mod decomp_exp {
    use std::collections::BTreeMap;
    use std::string::{String, ToString};
    use std::vec::Vec;

    type Macros = BTreeMap<String, (String, String)>;

    pub fn tables(src: &str) -> Vec<Vec<i64>> {
        let joined = src.replace("\\\n", " ");
        let mut macros = Macros::new();
        for line in joined.lines() {
            let Some(rest) = line.trim().strip_prefix("#define ") else {
                continue;
            };
            let open = rest.find('(').unwrap();
            let close = rest.find(')').unwrap();
            let body = rest[close + 1..].split("//").next().unwrap().trim();
            macros.insert(
                rest[..open].into(),
                (rest[open + 1..close].trim().into(), body.into()),
            );
        }
        let start = joined.find("gExperienceTables").unwrap();
        let after = &joined[start..];
        let body = &after[after.find('{').unwrap() + 1..];
        let mut rows = Vec::new();
        for row in body.split('}').take_while(|r| r.contains('{')) {
            let row = &row[row.find('{').unwrap() + 1..];
            let mut vals = Vec::new();
            for line in row.lines() {
                let e = line.split("//").next().unwrap().trim();
                let e = e.trim_end_matches(',').trim();
                if !e.is_empty() {
                    vals.push(eval(e, &macros, &BTreeMap::new()));
                }
            }
            rows.push(vals);
        }
        rows
    }

    fn eval(e: &str, m: &Macros, vars: &BTreeMap<String, i64>) -> i64 {
        let toks = lex(e);
        let mut p = P {
            t: &toks,
            i: 0,
            m,
            vars,
        };
        let v = p.ternary();
        assert_eq!(p.i, toks.len(), "trailing tokens in {e}");
        v
    }

    fn lex(e: &str) -> Vec<String> {
        let mut out = Vec::new();
        let b = e.as_bytes();
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let mut i = 0;
        while i < b.len() {
            if b[i].is_ascii_whitespace() {
                i += 1;
            } else if word(b[i]) {
                let s = i;
                while i < b.len() && word(b[i]) {
                    i += 1;
                }
                out.push(e[s..i].into());
            } else if e[i..].starts_with("<=") {
                out.push("<=".into());
                i += 2;
            } else {
                out.push(e[i..i + 1].into());
                i += 1;
            }
        }
        out
    }

    struct P<'a> {
        t: &'a [String],
        i: usize,
        m: &'a Macros,
        vars: &'a BTreeMap<String, i64>,
    }

    impl P<'_> {
        fn peek(&self) -> &str {
            self.t.get(self.i).map_or("", |s| s.as_str())
        }

        fn eat(&mut self, s: &str) -> bool {
            let hit = self.peek() == s;
            self.i += hit as usize;
            hit
        }

        fn ternary(&mut self) -> i64 {
            let c = self.cmp();
            if self.eat("?") {
                let a = self.ternary();
                assert!(self.eat(":"));
                let b = self.ternary();
                if c != 0 { a } else { b }
            } else {
                c
            }
        }

        fn cmp(&mut self) -> i64 {
            let a = self.add();
            if self.eat("<=") {
                (a <= self.add()) as i64
            } else {
                a
            }
        }

        fn add(&mut self) -> i64 {
            let mut v = self.mul();
            loop {
                if self.eat("+") {
                    v += self.mul();
                } else if self.eat("-") {
                    v -= self.mul();
                } else {
                    return v;
                }
            }
        }

        fn mul(&mut self) -> i64 {
            let mut v = self.atom();
            loop {
                if self.eat("*") {
                    v *= self.atom();
                } else if self.eat("/") {
                    v /= self.atom(); // C truncation
                } else {
                    return v;
                }
            }
        }

        fn atom(&mut self) -> i64 {
            let tok = self.peek().to_string();
            self.i += 1;
            if tok == "(" {
                let v = self.ternary();
                assert!(self.eat(")"));
                return v;
            }
            if let Ok(v) = tok.parse() {
                return v;
            }
            if let Some(v) = self.vars.get(&tok) {
                return *v;
            }
            let (param, body) = self.m.get(&tok).unwrap_or_else(|| panic!("unknown {tok}"));
            assert!(self.eat("("));
            let arg = self.ternary();
            assert!(self.eat(")"));
            let mut vars = BTreeMap::new();
            vars.insert(param.clone(), arg);
            eval(body, self.m, &vars)
        }
    }
}
