use super::*;

fn player_move() -> Entrant {
    let mov = AppealMove {
        appeal: 40,
        jam: 0,
        category: 0,
        id: 1,
    };
    Entrant {
        condition: Condition::default(),
        moves: [Some(mov), None, None, None],
    }
}

#[test]
fn round1_uses_each_categorys_main_and_subs() {
    let c = Condition {
        cool: 10,
        beauty: 4,
        cute: 2,
        smart: 8,
        tough: 6,
        sheen: 2,
    };
    assert_eq!(round1(0, c), 10 + (6 + 4 + 2) / 2);
    assert_eq!(round1(1, c), 4 + (10 + 2 + 2) / 2);
    assert_eq!(round1(2, c), 2 + (4 + 8 + 2) / 2);
    assert_eq!(round1(3, c), 8 + (2 + 6 + 2) / 2);
    assert_eq!(round1(4, c), 6 + (8 + 10 + 2) / 2);
    assert_eq!(round1(9, c), round1(4, c));
}

#[test]
fn standings_rank_by_total_then_round1_then_random() {
    assert_eq!(standings([10, 30, 20, 5], [0; 4], [0; 4]), [2, 0, 1, 3]);
    // Totals tied. Contestant 1 beats 3 on the random (9 > 8); both beat 2 on round 1.
    assert_eq!(standings([10; 4], [1, 5, 3, 5], [1, 9, 2, 8]), [3, 0, 2, 1]);
}

#[test]
fn an_untrained_normal_contest_is_won_by_appeal_40() {
    let o = judge(0, 0, player_move(), &mut Rng::new(1));
    // Five Cool appeals of 40: +10, then repeats −20, −30, −40, −50. Sum 70, round 2 is
    // double, round 1 is 0. Rivals appeal 20 the same way (sum −30, round 2 −60).
    assert_eq!(o.appeal[PLAYER_MON as usize], 70);
    assert_eq!(o.round1[PLAYER_MON as usize], 0);
    assert_eq!(o.total[PLAYER_MON as usize], 140);
    assert_eq!(o.appeal[0], -30);
    assert_eq!(o.total[0], -60);
    assert_eq!(o.place[PLAYER_MON as usize], 0);
}

#[test]
fn a_master_rival_beats_an_untrained_mon() {
    let o = judge(0, 3, player_move(), &mut Rng::new(1));
    assert_eq!(o.total[PLAYER_MON as usize], 140);
    assert_eq!(o.round1[0], 240);
    assert_eq!(o.total[0], 180);
    assert_eq!(o.place[PLAYER_MON as usize], 3);
}

#[test]
fn contest_names_are_the_english_gen3_strings() {
    assert_eq!(name(0), encode(b"COOLNESS CONTEST").as_slice());
    assert_eq!(name(1), encode(b"BEAUTY CONTEST").as_slice());
    assert_eq!(name(2), encode(b"CUTENESS CONTEST").as_slice());
    assert_eq!(name(3), encode(b"SMARTNESS CONTEST").as_slice());
    assert_eq!(name(4), encode(b"TOUGHNESS CONTEST").as_slice());
    assert!(name(5).is_empty());
}
