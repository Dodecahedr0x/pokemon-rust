use super::*;
use crate::battle::test_data::*;
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
        item: None,
    },
    TrainerMon {
        species: ZIGZAGOON,
        level: 3,
        iv: 128,
        moves: Some([Some(BIDE), None, None, None]),
        item: None,
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
        item: None,
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

#[test]
fn a_trainer_mon_item_is_held_without_drawing() {
    static HOLDS: [TrainerMon; 1] = [TrainerMon {
        species: MUDKIP,
        level: 5,
        iv: 0,
        moves: None,
        item: Some(crate::data::ItemId(139)),
    }];
    static PLAIN: [TrainerMon; 1] = [TrainerMon {
        item: None,
        ..HOLDS[0]
    }];
    let (mut a, mut b) = (Rng::new(3), Rng::new(3));
    let held = create_trainer_party(
        &TABLES,
        &TrainerData {
            party: &HOLDS,
            ..trainer(false, false)
        },
        &mut a,
    );
    let plain = create_trainer_party(
        &TABLES,
        &TrainerData {
            party: &PLAIN,
            ..trainer(false, false)
        },
        &mut b,
    );
    assert_eq!(held.get(0).unwrap().held, Some(crate::data::ItemId(139)));
    assert_eq!(plain.get(0).unwrap().held, None);
    assert_eq!(a.state(), b.state());
    assert_eq!(
        Pokemon {
            held: None,
            ..*held.get(0).unwrap()
        },
        *plain.get(0).unwrap()
    );
}
