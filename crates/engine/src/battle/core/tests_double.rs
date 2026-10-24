//! Double battles on the synthetic tables (`battle::test_data`): targeting
//! (`HandleAction_UseMove`), the spread moves (`MOVEEND_NEXT_TARGET`, the
//! `MOVE_TARGET_FOES_AND_ALLY` loops, `CalculateBaseDamage`'s halving), Helping Hand,
//! Follow Me, Lightning Rod, the turn order and the faint replacements. Lines use the core
//! tests' format, with `p` / `o` for the right battlers.

use super::tests::{Run, before_setup_draws, trainer};
use super::*;
use crate::battle::test_data::*;
use crate::battle::{Choice, Double, Environment, Need, Side};
use crate::mon::{Ability, Pokemon};
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;
const PR: Side = Side::PlayerRight;
const OR: Side = Side::OpponentRight;

/// A started double battle against one trainer (`trainerbattle_double`).
fn double(player: &[Pokemon], foes: &[Pokemon], seed: u32) -> Run {
    let mut rng = Rng::new(before_setup_draws(seed));
    let setup = BattleSetup {
        kind: trainer(),
        opponents: party(foes),
        environment: Environment::Grass,
        double: Some(Double::default()),
        rules: Default::default(),
    };
    let mut b = Battle::new(&Game, setup, &party(player), &mut rng);
    let mut out = Vec::new();
    b.start(&Game, &mut rng, &mut out);
    Run { b, rng, out }
}

/// L5, 200 HP, Atk 20, Def 10, Speed `spe`: a Normal hit of power 50 deals 8 before the
/// halving, the random factor and STAB.
fn hitter(spe: u16, moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [200, 20, 10, spe, 10, 10], moves)
}

/// L5, 200 HP, Atk 10, Def 10, Speed `spe`.
fn foe(spe: u16, moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [200, 10, 10, spe, 10, 10], moves)
}

fn has(l: &[String], p: &str) -> bool {
    l.iter().any(|x| x.starts_with(p))
}

fn pos(l: &[String], p: &str) -> usize {
    l.iter()
        .position(|x| x.starts_with(p))
        .unwrap_or_else(|| panic!("no {p} in {l:?}"))
}

/// The HP `side` lost this turn.
fn lost(r: &Run, side: Side, before: u16) -> u16 {
    before - r.b.mon(side).hp
}

/// Both player battlers choose: the left one `l`, then the right one `r`.
fn turn(r: &mut Run, l: Choice, rc: Choice) -> Vec<String> {
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(r.b.choosing(), P);
    let mut out = r.choose(l);
    assert_eq!(r.b.choosing(), PR, "{out:?}");
    out.extend(r.choose(rc));
    out
}

#[test]
fn four_battlers_act_by_speed() {
    let mut r = double(
        &[hitter(9, &[REAL_SPLASH]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    assert_eq!(r.b.count(), 4);
    let l = turn(&mut r, Choice::Move(0), Choice::Move(0));
    let used: Vec<char> = l
        .iter()
        .filter(|x| x.starts_with("UsedMove@"))
        .map(|x| x.chars().nth("UsedMove@".len()).unwrap())
        .collect();
    assert_eq!(used, ['P', 'O', 'o', 'p'], "{l:?}");
}

/// `MOVE_TARGET_BOTH`: one attack string, then the partner of the target
/// (`MOVEEND_NEXT_TARGET`); with both foes out the damage is halved.
#[test]
fn spread_move_hits_both_foes_at_half() {
    let mut r = double(
        &[hitter(9, &[SPREAD_HIT]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    let l = turn(&mut r, Choice::Move(0), Choice::Move(0));
    assert_eq!(
        l.iter().filter(|x| x.starts_with("UsedMove@P")).count(),
        1,
        "{l:?}"
    );
    assert!(pos(&l, "Hp O") < pos(&l, "Hp o"), "{l:?}");
    // 8 / 2 = 4, + 2, STAB: 9, the random factor: 7..=9 (unhalved: 12..=15).
    for s in [O, OR] {
        let d = lost(&r, s, 200);
        assert!((7..=9).contains(&d), "{s:?} lost {d}: {l:?}");
    }
}

/// With the right foe absent, a `MOVE_TARGET_BOTH` move is not halved and hits only the left.
#[test]
fn spread_move_alone_is_not_halved() {
    let mut r = double(
        &[hitter(9, &[SPREAD_HIT]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH])],
        1,
    );
    assert!(r.b.is_absent(OR));
    let l = turn(&mut r, Choice::Move(0), Choice::Move(0));
    let d = lost(&r, O, 200);
    assert!((12..=15).contains(&d), "lost {d}: {l:?}");
    assert!(!has(&l, "Hp o"), "{l:?}");
}

/// `MOVE_TARGET_FOES_AND_ALLY` (Earthquake): every battler but the user, in battler order,
/// not halved.
#[test]
fn quake_hits_foes_and_ally() {
    let mut r = double(
        &[hitter(9, &[QUAKE]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    let l = turn(&mut r, Choice::Move(0), Choice::Move(0));
    let (a, b, c) = (pos(&l, "Hp O"), pos(&l, "Hp p"), pos(&l, "Hp o"));
    assert!(a < b && b < c, "{l:?}");
    assert!(!has(&l, "Hp P"), "{l:?}");
    for s in [O, PR, OR] {
        assert!(lost(&r, s, 200) > 0, "{l:?}");
    }
}

/// An absent target is replaced by its partner (`gAbsentBattlerFlags`).
#[test]
fn absent_foe_redirects_to_its_partner() {
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH])],
        1,
    );
    let at = Choice::MoveAt {
        slot: 0,
        target: OR,
    };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(has(&l, "Hp O"), "{l:?}");
}

/// The picked target is hit (`gBattleStruct->moveTarget`).
#[test]
fn picked_target_is_hit() {
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    let at = Choice::MoveAt {
        slot: 0,
        target: OR,
    };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(has(&l, "Hp o"), "{l:?}");
    assert!(!has(&l, "Hp O"), "{l:?}");
}

/// Follow Me (`setforcedtarget`): the other side's selected moves go to its user this turn.
#[test]
fn follow_me_draws_the_attacks() {
    let mut r = double(
        &[hitter(3, &[TACKLE]), hitter(2, &[REAL_SPLASH])],
        &[foe(7, &[FOLLOW_ME]), foe(5, &[REAL_SPLASH])],
        1,
    );
    let at = Choice::MoveAt {
        slot: 0,
        target: OR,
    };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(has(&l, "PkmnCenterAttention@O"), "{l:?}");
    assert!(has(&l, "Hp O"), "{l:?}");
    assert!(!has(&l, "Hp o"), "{l:?}");
    assert_eq!(r.b.fx.follow_me, [None; 2]);
}

/// Helping Hand (`trysethelpinghand`): the partner's move × 1.5 this turn.
#[test]
fn helping_hand_boosts_the_partner() {
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[HELPING_HAND])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    let l = turn(&mut r, Choice::Move(0), Choice::Move(0));
    assert!(pos(&l, "PkmnReadyToHelp@p") < pos(&l, "Hp O"), "{l:?}");
    assert_eq!(r.b.fx.helping_hand, 0);
    // Tackle 35: 4 * 20 * 35 / 10 / 50 = 5 (+ 2), STAB 10, random 8..=10, Helping Hand 12..=15.
    let d = lost(&r, O, 200);
    assert!((12..=15).contains(&d), "lost {d}: {l:?}");

    // Alone, it fails.
    let mut r = double(
        &[hitter(3, &[HELPING_HAND])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    assert!(r.b.is_absent(PR));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P"), "{l:?}");
}

/// Lightning Rod (`HandleAction_UseMove`): an Electric move goes to it, with
/// `BattleScript_TookAttack`.
#[test]
fn lightning_rod_draws_electric_moves() {
    let mut r = double(
        &[hitter(9, &[THUNDER_SHOCK]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    r.b.battler_mut(OR).ability = Ability::LightningRodGen3;
    let at = Choice::MoveAt { slot: 0, target: O };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(
        pos(&l, "UsedMove@P") < pos(&l, "PkmnsXTookAttack@o"),
        "{l:?}"
    );
    assert!(has(&l, "Hp o"), "{l:?}");
    assert!(!has(&l, "Hp O"), "{l:?}");
}

/// A fainted foe with a mon left is replaced; with none, its battler is absent and the
/// battle goes on against the other one.
#[test]
fn fainted_foe_is_replaced_or_absent() {
    let mut weak = foe(5, &[REAL_SPLASH]);
    weak.hp = 1;
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), weak, foe(1, &[REAL_SPLASH])],
        1,
    );
    let at = Choice::MoveAt {
        slot: 0,
        target: OR,
    };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(has(&l, "Faint o"), "{l:?}");
    assert!(has(&l, "SendOut o2"), "{l:?}");
    assert!(!r.b.is_absent(OR));
    assert_eq!(r.b.battler(OR).mon.party, 2);
    assert_eq!(r.b.need(), Need::Action);
}

/// Both battlers that faced a fainted foe share its exp (`gSentPokesToOpponent`).
#[test]
fn exp_goes_to_both_battlers() {
    let mut weak = foe(5, &[REAL_SPLASH]);
    weak.hp = 1;
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[REAL_SPLASH])],
        &[weak, foe(1, &[REAL_SPLASH]), foe(1, &[REAL_SPLASH])],
        1,
    );
    let at = Choice::MoveAt { slot: 0, target: O };
    let l = turn(&mut r, at, Choice::Move(0));
    assert!(has(&l, "Exp 0"), "{l:?}");
    assert!(has(&l, "Exp 1"), "{l:?}");
}

/// The right battler's menu can go back to the left one's choice (`B_ACTION_CANCEL_PARTNER`).
#[test]
fn right_battler_cancels_to_the_left() {
    let mut r = double(
        &[hitter(9, &[TACKLE]), hitter(3, &[REAL_SPLASH])],
        &[foe(7, &[REAL_SPLASH]), foe(5, &[REAL_SPLASH])],
        1,
    );
    r.choose(Choice::Move(0));
    assert_eq!(r.b.choosing(), PR);
    r.choose(Choice::Cancel);
    assert_eq!(r.b.choosing(), P);
    assert_eq!(r.b.actions[0], None);
}

/// Singles never see the doubles state.
#[test]
fn singles_have_no_doubles_state() {
    let mut r = Run::wild(
        &[hitter(9, &[FOLLOW_ME, HELPING_HAND])],
        foe(5, &[REAL_SPLASH]),
        1,
    );
    r.choose(Choice::Move(0));
    r.choose(Choice::Move(1));
    assert_eq!(r.b.fx.follow_me, [None; 2]);
    assert_eq!(r.b.fx.helping_hand, 0);
}
