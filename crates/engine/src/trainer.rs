//! Trainer parties, as Gen3 `CreateNPCTrainerParty` (`battle_main.c`).

use crate::mon::{
    BattleTables, Ivs, PARTY_SIZE, Party, Pokemon, TrainerData, is_shiny_ot_personality,
};
use crate::rng::Rng;

/// Gen3 `MAX_PER_STAT_IVS`.
const MAX_IV: u32 = 31;

/// Gen3 `CreateNPCTrainerParty` (held items ignored):
/// - personality 0x80 (double battle), 0x78 (female trainer) or 0x88, plus `nameHash << 8`,
///   where `nameHash` keeps growing across the party: each mon adds the trainer name's bytes
///   and its species name's bytes (up to EOS)
/// - `fixedIV = iv * 31 / 255`
/// - OT id `random32()` until the mon is not shiny (`OT_ID_RANDOM_NO_SHINY`)
/// - the level-up moveset, replaced by the custom one when given (base PP)
pub fn create_trainer_party(t: &BattleTables, trainer: &TrainerData, rng: &mut Rng) -> Party {
    let base: u32 = if trainer.double {
        0x80
    } else if trainer.female {
        0x78
    } else {
        0x88
    };
    let mut name_hash: u32 = 0;
    let mut party = Party::default();
    for mon in trainer.party.iter().take(PARTY_SIZE) {
        name_hash = name_hash
            .wrapping_add(byte_sum(trainer.name))
            .wrapping_add(byte_sum(t.species_info(mon.species).name));
        let personality = base.wrapping_add(name_hash << 8);
        let iv = (mon.iv as u32 * MAX_IV / 255) as u8;
        let ot_id = loop {
            let v = rng.random32();
            if !is_shiny_ot_personality(v, personality) {
                break v;
            }
        };
        let mut p = Pokemon::new(
            t,
            mon.species,
            mon.level,
            Ivs::Fixed(iv),
            personality,
            ot_id,
            rng,
        );
        if let Some(moves) = mon.moves {
            for (j, m) in moves.into_iter().enumerate() {
                p.moves[j] = m;
                p.pp[j] = m.map_or(0, |m| t.move_info(m).pp);
            }
        }
        // `take(PARTY_SIZE)` keeps the push in bounds.
        let _ = party.push(p);
    }
    party
}

/// Sum of a Gen3 string's bytes up to EOS (0xFF).
fn byte_sum(s: &[u8]) -> u32 {
    s.iter()
        .take_while(|&&c| c != 0xFF)
        .map(|&c| c as u32)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mon::test_tables::*;
    use crate::mon::{Gender, TrainerMon, TrainerPic};

    static PIC: TrainerPic = TrainerPic {
        tiles: &[],
        palette: &[0; 16],
        y: 8,
    };

    /// Trainer "AB" (0xBB + 0xBC = 375): a Mudkip L5 (IV 255) and a Zigzagoon L3 (IV 128, Bide).
    static PARTY: [TrainerMon; 2] = [
        TrainerMon {
            species: MUDKIP,
            level: 5,
            iv: 255,
            moves: None,
        },
        TrainerMon {
            species: ZIGZAGOON,
            level: 3,
            iv: 128,
            moves: Some([Some(BIDE), None, None, None]),
        },
    ];

    fn trainer(female: bool, double: bool) -> TrainerData {
        TrainerData {
            class_name: &[0xFF],
            name: &[0xBB, 0xBC, 0xFF, 0xBB],
            pic: &PIC,
            female,
            ai: 0,
            double,
            party: &PARTY,
        }
    }

    #[test]
    fn personality_comes_from_the_running_name_hash() {
        // nameHash: mon 0 = 375 + "MUDKIP" (1190) = 1565; mon 1 = 1565 + 375 + "ZIGZAGOON"
        // (1794) = 3734. Personality = base + (nameHash << 8).
        let cases = [
            (false, false, 0x88),
            (true, false, 0x78),
            (true, true, 0x80),
        ];
        for (female, double, base) in cases {
            let p = create_trainer_party(&TABLES, &trainer(female, double), &mut Rng::new(3));
            assert_eq!(p.len(), 2);
            assert_eq!(p.get(0).unwrap().personality, base + (1565 << 8));
            assert_eq!(p.get(1).unwrap().personality, base + (3734 << 8));
        }
        // 0x88 + 1565 << 8: low byte 0x88 = 136 > 31 -> male Mudkip; 0x78 = 120 -> male too
        // (ratio 31). Zigzagoon (127): 0x78 is female, 0x88 male.
        let m = create_trainer_party(&TABLES, &trainer(false, false), &mut Rng::new(3));
        let f = create_trainer_party(&TABLES, &trainer(true, false), &mut Rng::new(3));
        assert_eq!(m.get(1).unwrap().gender(&TABLES), Gender::Male);
        assert_eq!(f.get(1).unwrap().gender(&TABLES), Gender::Female);
    }

    #[test]
    fn ivs_scale_to_31_and_custom_moves_replace_the_learnset() {
        let p = create_trainer_party(&TABLES, &trainer(false, false), &mut Rng::new(3));
        let (a, b) = (p.get(0).unwrap(), p.get(1).unwrap());
        // 255 * 31 / 255 = 31; 128 * 31 / 255 = 15.
        assert_eq!(a.ivs, [31; 6]);
        assert_eq!(b.ivs, [15; 6]);
        assert_eq!(a.moves, [Some(TACKLE), Some(GROWL), None, None]);
        assert_eq!(b.moves, [Some(BIDE), None, None, None]);
        assert_eq!(b.pp, [10, 0, 0, 0]);
        assert_eq!((a.level, b.level), (5, 3));
        assert_eq!(a.hp, a.stats[0]);
        // IV 0 -> 0.
        static ZERO: [TrainerMon; 1] = [TrainerMon {
            species: MUDKIP,
            level: 5,
            iv: 0,
            moves: None,
        }];
        let t = TrainerData {
            party: &ZERO,
            ..trainer(false, false)
        };
        let p = create_trainer_party(&TABLES, &t, &mut Rng::new(3));
        assert_eq!(p.get(0).unwrap().ivs, [0; 6]);
    }

    #[test]
    fn ot_ids_are_random32_and_never_shiny() {
        let mut rng = Rng::new(3);
        let p = create_trainer_party(&TABLES, &trainer(false, false), &mut rng);
        let mut r = Rng::new(3);
        for i in 0..2 {
            let mon = p.get(i).unwrap();
            let ot = loop {
                let v = r.random32();
                if !is_shiny_ot_personality(v, mon.personality) {
                    break v;
                }
            };
            assert_eq!(mon.ot_id, ot);
            assert!(!mon.is_shiny());
        }
        // Fixed IVs: nothing else is drawn.
        assert_eq!(rng, r);
        // Same seed, same party.
        let q = create_trainer_party(&TABLES, &trainer(false, false), &mut Rng::new(3));
        assert_eq!(p, q);
    }
}
