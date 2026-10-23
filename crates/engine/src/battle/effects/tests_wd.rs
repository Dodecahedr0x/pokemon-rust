//! Wave D move effects on the synthetic tables (`battle::test_data`), one test per mechanism,
//! and the Wave C deviations fixed with them. Event lists use the core tests' compact format
//! (`core::tests::fmt`).

use super::tests::{has, wild};
use crate::battle::core::tests::{Run, trainer};
use crate::battle::test_data::*;
use crate::battle::{BattleKind, Choice, Need, Side};
use crate::data::{ItemId, MoveId};
use crate::mon::{Ability, HoldEffect, Pokemon, Stat, Status, Type};
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;
const ITEM: ItemId = ItemId(77);

/// L5, 100 HP, Atk 13, Def 10, Speed 9: moves first.
fn me(moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [100, 13, 10, 9, 9, 10], moves)
}

/// L5, 100 HP, Atk 10, Def 10, Speed 5.
fn foe(moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [100, 10, 10, 5, 10, 10], moves)
}

/// L5, 100 HP, Speed 1: moves last.
fn slow(moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [100, 13, 10, 1, 9, 10], moves)
}

fn wild_kind() -> BattleKind {
    BattleKind::Wild {
        first_battle: false,
    }
}

fn starts(l: &[String], p: &str) -> bool {
    l.iter().any(|x| x.starts_with(p))
}

fn pos(l: &[String], p: &str) -> usize {
    l.iter()
        .position(|x| x.starts_with(p))
        .unwrap_or_else(|| panic!("no {p} in {l:?}"))
}

/// Chooses `c` for up to `turns` answers, until `stop` holds of the lines so far.
fn until(r: &mut Run, c: Choice, turns: usize, stop: impl Fn(&[String]) -> bool) -> Vec<String> {
    let mut all = Vec::new();
    for _ in 0..turns {
        if matches!(r.b.need, Need::Ended(_)) {
            break;
        }
        all.extend(r.choose(c));
        if stop(&all) {
            break;
        }
    }
    all
}

/// `side`'s battler (party slot 0) holds `ITEM` with `hold`.
fn hold(r: &mut Run, side: Side, hold: Option<HoldEffect>) {
    let m = r.b.battler_mut(side);
    m.x.item = Some(ITEM);
    m.x.hold = hold;
    r.b.x.holds[side as usize][0] = hold;
}

/// `Cmd_setsubstitute`: a quarter of the max HP; the substitute takes the hits
/// (`Cmd_datahpupdate`) and fades at 0 (`BattleScript_SubstituteFade`, cleared at
/// `MOVEEND_SUBSTITUTE`); a second one fails, and so does one at a quarter of the HP or less.
#[test]
fn substitute_takes_the_hits() {
    let mut r = wild(me(&[SUBSTITUTE, REAL_SPLASH]), foe(&[POUND]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnMadeSubstitute@P ~64"), "{l:?}");
    assert!(starts(&l, "SubstituteDamaged"), "{l:?}");
    assert_eq!(r.b.mon(P).hp, 75);
    assert!(r.b.battler(P).fx.substitute);
    assert!(r.b.battler(P).fx.sub_hp < 25);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnHasSubstitute@P ~64"), "{l:?}");
    let l = until(&mut r, Choice::Move(1), 10, |l| {
        starts(l, "PkmnSubstituteFaded")
    });
    assert!(starts(&l, "PkmnSubstituteFaded"), "{l:?}");
    assert_eq!(r.b.mon(P).hp, 75);
    assert!(!r.b.battler(P).fx.substitute);

    let mut r = wild(me(&[SUBSTITUTE]), foe(&[REAL_SPLASH]), 1);
    r.b.battler_mut(P).mon.hp = 25;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "TooWeakForSubstitute@P ~64"), "{l:?}");
    assert!(!r.b.battler(P).fx.substitute);
}

/// `SetMoveEffect` does nothing behind a substitute (the 100% poison of the hit), and the
/// status scripts' `jumpifstatus2 STATUS2_SUBSTITUTE` stops Toxic, Leech Seed and Screech.
#[test]
fn substitute_blocks_effects() {
    for mov in [POISON_ALL, TOXIC, LEECH_SEED, SCREECH] {
        let mut r = wild(me(&[SUBSTITUTE, REAL_SPLASH]), foe(&[mov]), 1);
        let mut l = r.choose(Choice::Move(0));
        l.extend(r.choose(Choice::Move(1)));
        l.extend(r.choose(Choice::Move(1)));
        assert!(r.b.battler(P).fx.substitute, "{l:?}");
        assert_eq!(r.b.mon(P).status, Status::None, "{l:?}");
        assert!(!r.b.battler(P).seeded, "{l:?}");
        assert_eq!(r.b.battler(P).stages[Stat::Def as usize], 0, "{l:?}");
    }
}

/// `BattleScript_EffectBatonPass`: the player picks the next mon (`openpartyscreen`), which
/// keeps the stat stages and the substitute (`SwitchInClearSetData`); alone, it fails.
#[test]
fn baton_pass_passes_stages() {
    let mut r = Run::new(
        wild_kind(),
        &[me(&[SNATCH_DANCE, BATON_PASS, SUBSTITUTE]), me(&[POUND])],
        &[foe(&[REAL_SPLASH])],
        1,
    );
    r.choose(Choice::Move(0));
    r.choose(Choice::Move(2));
    let sub = r.b.battler(P).fx.sub_hp;
    r.choose(Choice::Move(1));
    assert_eq!(r.b.need, Need::Switch);
    let l = r.choose(Choice::Switch(1));
    assert!(has(&l, "SendOut P1"), "{l:?}");
    let m = r.b.battler(P);
    assert_eq!(m.mon.party, 1);
    assert_eq!(m.stages[Stat::Atk as usize], 2);
    assert!(m.fx.substitute && m.fx.sub_hp == sub);

    let mut r = wild(me(&[BATON_PASS]), foe(&[REAL_SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");

    // The opponent sends its next mon at once.
    let mut r = Run::new(
        trainer(),
        &[slow(&[REAL_SPLASH])],
        &[foe(&[BATON_PASS]), foe(&[REAL_SPLASH])],
        1,
    );
    r.b.battler_mut(O).stages[Stat::Def as usize] = 3;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "SendOut O1"), "{l:?}");
    assert_eq!(r.b.battler(O).mon.party, 1);
    assert_eq!(r.b.battler(O).stages[Stat::Def as usize], 3);
}

/// `Cmd_transformdataexecution`: species, stats, moves (5 PP), types; the party mon keeps its
/// own moves and PP.
#[test]
fn transform_copies_the_target() {
    let target = mon(GHOST, 5, [100, 20, 30, 5, 10, 10], &[POUND, TACKLE]);
    let mut r = wild(me(&[TRANSFORM]), target, 1);
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "PkmnTransformedInto@P"), "{l:?}");
    let m = r.b.battler(P);
    assert!(m.fx.transformed);
    assert_eq!(m.mon.species, GHOST);
    assert_eq!(m.mon.moves[..2], [Some(POUND), Some(TACKLE)]);
    assert_eq!(m.mon.pp[..2], [5, 5]);
    assert_eq!(m.stats[2], 30);
    assert_eq!(m.types, r.b.battler(O).types);
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).pp[0], 4);
    let p = r.b.parties[0].get(0).unwrap();
    assert_eq!((p.moves[0], p.pp[0]), (Some(TRANSFORM), 9));

    // A transformed target cannot be copied.
    let mut r = wild(me(&[TRANSFORM]), foe(&[REAL_SPLASH]), 1);
    r.b.battler_mut(O).fx.transformed = true;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
}

/// `Cmd_mimicattackcopy`: no last move fails; then the foe's last move replaces Mimic for the
/// battle (5 PP), not in the party.
#[test]
fn mimic_copies_the_last_move() {
    let mut r = wild(me(&[MIMIC]), foe(&[POUND]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "PkmnLearnedMove2@P"), "{l:?}");
    assert_eq!(r.b.mon(P).moves[0], Some(POUND));
    assert_eq!(r.b.mon(P).pp[0], 5);
    assert!(r.b.battler(P).fx.mimicked & 1 != 0);
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).pp[0], 4);
    assert_eq!(r.b.parties[0].get(0).unwrap().moves[0], Some(MIMIC));
}

/// `Cmd_copymovepermanently`: the target's last printed move replaces Sketch in the party,
/// with its full PP.
#[test]
fn sketch_copies_for_good() {
    let mut r = wild(me(&[SKETCH]), foe(&[POUND]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
    // Sketch has 1 PP: give it back.
    r.b.battler_mut(P).mon.pp[0] = 1;
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "PkmnSketchedMove@P"), "{l:?}");
    assert_eq!(r.b.mon(P).moves[0], Some(POUND));
    assert_eq!(r.b.mon(P).pp[0], 10);
    let p = r.b.parties[0].get(0).unwrap();
    assert_eq!((p.moves[0], p.pp[0]), (Some(POUND), 10));
}

/// `Cmd_tryrecycleitem`: the berry eaten earlier (`usedHeldItems`) comes back.
#[test]
fn recycle_restores_the_berry() {
    let mut r = wild(me(&[RECYCLE, REAL_SPLASH]), foe(&[REAL_SPLASH]), 1);
    hold(&mut r, P, Some(HoldEffect::RestoreHp(30)));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
    r.b.battler_mut(P).mon.hp = 30;
    let l = r.choose(Choice::Move(1));
    assert_eq!(r.b.battler(P).x.item, None, "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "XFoundOneY@P"), "{l:?}");
    assert_eq!(r.b.battler(P).x.item, Some(ITEM));
    assert_eq!(r.b.battler(P).x.hold, Some(HoldEffect::RestoreHp(30)));
    assert_eq!(r.b.parties[0].get(0).unwrap().held, Some(ITEM));
}

/// `Cmd_settypetorandomresistance`: a type resisting the last move that landed on the user;
/// nothing landed yet fails.
#[test]
fn conversion_2_resists_the_last_hit() {
    let mut r = wild(me(&[CONVERSION_2]), foe(&[POUND]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert!(starts(&l, "PkmnChangedType@P"), "{l:?}");
    let t = r.b.battler(P).types[0];
    assert!(matches!(t, Type::Rock | Type::Steel | Type::Ghost), "{t:?}");
}

/// `Cmd_trysetsnatch` + the attack canceler: the foe's Swords Dance goes to the snatcher; last
/// in the turn, Snatch fails.
#[test]
fn snatch_steals_a_stat_move() {
    let mut r = wild(slow(&[SNATCH]), foe(&[SNATCH_DANCE]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnWaitsForTarget@P ~64"), "{l:?}");
    assert!(starts(&l, "PkmnSnatchedMove@O"), "{l:?}");
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 2);
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
    assert!(!r.b.battler(P).fx.snatch);

    let mut r = wild(me(&[SNATCH]), foe(&[SNATCH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@O ~64"), "{l:?}");
}

/// `Cmd_trydobeatup`: one hit per party mon with HP and no status.
#[test]
fn beat_up_hits_per_party_mon() {
    let mut poisoned = me(&[POUND]);
    poisoned.status = Status::Poison;
    let mut fainted = me(&[POUND]);
    fainted.hp = 0;
    let mut r = Run::new(
        wild_kind(),
        &[me(&[BEAT_UP]), poisoned, me(&[POUND]), fainted],
        &[foe(&[REAL_SPLASH])],
        1,
    );
    let l = r.choose(Choice::Move(0));
    let hits = l.iter().filter(|x| x.starts_with("PkmnAttack@P")).count();
    assert_eq!(hits, 2, "{l:?}");
    assert_eq!(
        l.iter().filter(|x| x.starts_with("Hp O")).count(),
        2,
        "{l:?}"
    );
}

/// `EFFECT_PAY_DAY`: level x 5 per hit, given after the win (`PlayerPickedUpMoney`).
#[test]
fn pay_day_scatters_coins() {
    let weak = mon(NORMAL, 5, [10, 10, 10, 5, 10, 10], &[REAL_SPLASH]);
    let mut r = wild(me(&[PAY_DAY]), weak, 1);
    let l = until(&mut r, Choice::Move(0), 6, |_| false);
    assert!(starts(&l, "CoinsScattered"), "{l:?}");
    assert!(starts(&l, "PlayerPickedUpMoney"), "{l:?}");
    let hits = l.iter().filter(|x| x.starts_with("CoinsScattered")).count() as u32;
    assert_eq!(r.b.pay_day(), Some(25 * hits));
}

/// `Cmd_tryswapitems`: Thief takes an item, but not Mail nor the Enigma Berry
/// (`HOLD_EFFECT_NO_SWAP` here); Trick refuses them too.
#[test]
fn thief_and_trick_refuse_mail() {
    let mut r = wild(me(&[THIEF]), foe(&[REAL_SPLASH]), 1);
    hold(&mut r, O, None);
    let l = until(&mut r, Choice::Move(0), 4, |l| starts(l, "Hp O"));
    assert!(starts(&l, "PkmnStoleItem"), "{l:?}");
    assert_eq!(r.b.battler(P).x.item, Some(ITEM));

    let mut r = wild(me(&[THIEF]), foe(&[REAL_SPLASH]), 1);
    hold(&mut r, O, Some(HoldEffect::NoSwap));
    let l = until(&mut r, Choice::Move(0), 4, |l| starts(l, "Hp O"));
    assert!(!starts(&l, "PkmnStoleItem"), "{l:?}");
    assert_eq!(r.b.battler(O).x.item, Some(ITEM));

    let mut r = wild(me(&[TRICK]), foe(&[REAL_SPLASH]), 1);
    hold(&mut r, O, Some(HoldEffect::NoSwap));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "ButItFailed@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(O).x.item, Some(ITEM));
}

/// `HandleWishPerishSongOnTurnEnd` runs after `HandleFaintedMonActions`: a Future Sight due
/// on the turn its target faints hits the replacement.
#[test]
fn future_sight_after_faints() {
    let mut r = Run::new(
        trainer(),
        &[me(&[FUTURE_SIGHT, REAL_SPLASH, MEGA_HIT])],
        &[foe(&[REAL_SPLASH]), foe(&[REAL_SPLASH])],
        1,
    );
    r.choose(Choice::Move(0));
    r.choose(Choice::Move(1));
    r.b.battler_mut(O).mon.hp = 10;
    let l = r.choose(Choice::Move(2));
    assert!(pos(&l, "SendOut O1") < pos(&l, "PkmnTookAttack@O"), "{l:?}");
    assert_eq!(r.b.battler(O).mon.party, 1);
    assert!(r.b.mon(O).hp < 100, "{l:?}");
}

/// `PressurePPLose`: one more PP at any HP; Snatch pays it only when it steals from a
/// Pressure user.
#[test]
fn pressure_exemptions() {
    let mut r = wild(me(&[POUND]), foe(&[REAL_SPLASH]), 1);
    r.b.battler_mut(O).ability = Ability::Pressure;
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).pp[0], 8);

    let mut r = wild(slow(&[SNATCH]), foe(&[SNATCH_DANCE]), 1);
    r.b.battler_mut(O).ability = Ability::Pressure;
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).pp[0], 8);

    let mut r = wild(slow(&[SNATCH]), foe(&[REAL_SPLASH]), 1);
    r.b.battler_mut(O).ability = Ability::Pressure;
    r.choose(Choice::Move(0));
    assert_eq!(r.b.mon(P).pp[0], 9);
}

/// `sProtectSuccessRates` has 4 entries; Gen3 reads `protectUses` past them into
/// `sMovesForbiddenToCopy` (Metronome's id, Struggle's, Sketch's, Mimic's, then 0xFFFE).
#[test]
fn protect_rate_reads_past_the_table() {
    use super::mech::protect_rate;
    assert_eq!(protect_rate(&TABLES, 3), 0x1FFF);
    assert_eq!(protect_rate(&TABLES, 4), METRONOME.0);
    assert_eq!(protect_rate(&TABLES, 5), TABLES.struggle.0);
    assert_eq!(protect_rate(&TABLES, 6), SKETCH.0);
    assert_eq!(protect_rate(&TABLES, 7), MIMIC.0);
    assert_eq!(protect_rate(&TABLES, 8), 0xFFFE);
}
