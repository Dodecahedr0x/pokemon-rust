//! Wave C move effects on the synthetic tables (`battle::test_data`), one test per mechanism
//! group. Event lists use the core tests' compact format (`core::tests::fmt`).

use super::tests::{has, wild};
use crate::battle::core::Weather;
use crate::battle::core::tests::Run;
use crate::battle::test_data::*;
use crate::battle::{BattleOutcome, Choice, Need, Side};
use crate::data::MoveId;
use crate::mon::{Pokemon, Stat, Status};
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;

/// L5, 100 HP, Atk 13, Def 10, Speed 9: moves first.
fn me(moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [100, 13, 10, 9, 9, 10], moves)
}

/// L5, 100 HP, Atk 10, Def 10, Speed 5.
fn foe(moves: &[MoveId]) -> Pokemon {
    mon(NORMAL, 5, [100, 10, 10, 5, 10, 10], moves)
}

fn count(l: &[String], x: &str) -> usize {
    l.iter().filter(|y| *y == x).count()
}

fn starts(l: &[String], p: &str) -> bool {
    l.iter().any(|x| x.starts_with(p))
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

/// Multi-turn moves: Rollout and Thrash lock the user (`gLockedMoves`, not asked again),
/// Thrash ends in fatigue confusion, Hyper Beam costs the next turn, Solar Beam charges
/// (`STRINGID_EMPTYSTRING3`: no "used" on the first turn) and Fly hides its user.
#[test]
fn multi_turn_locks_and_charges() {
    let mut r = wild(me(&[ROLLOUT]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(count(&l, "UsedMove@P move:57") >= 2, "{l:?}");

    let mut r = wild(me(&[THRASH]), foe(&[POUND]), 3);
    let l = until(&mut r, Choice::Move(0), 4, |l| {
        has(l, "PkmnFatigueConfusion@P ~64")
    });
    assert!(count(&l, "UsedMove@P move:58") >= 2, "{l:?}");
    assert!(has(&l, "PkmnFatigueConfusion@P ~64"), "{l:?}");
    assert!(r.b.battler(P).confusion > 0);

    let mut r = wild(me(&[HYPER_BEAM]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnMustRecharge@P ~64"), "{l:?}");
    assert_eq!(count(&l, "UsedMove@P move:59"), 1, "{l:?}");

    let mut r = wild(me(&[SOLAR_BEAM]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[..2], ["Anim P move:60", "PkmnTookSunlight@P ~64"]);
    assert!(has(&l, "UsedMove@P move:60"), "{l:?}");
    // One PP for both turns.
    assert_eq!(r.b.mon(P).pp[0], 9);

    let mut r = wild(me(&[FLY]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..4],
        [
            "Anim P move:61",
            "PkmnFlewHigh@P ~64",
            "UsedMove@O move:56",
            "AttackMissed@O ~64"
        ]
    );
}

/// Fixed damage: Super Fang halves, Dragon Rage deals 40, Seismic Toss the level; Counter
/// returns twice the physical damage taken this turn.
#[test]
fn fixed_damage() {
    for (mv, after) in [(SUPER_FANG, 50), (DRAGON_RAGE, 60), (SEISMIC_TOSS, 95)] {
        let mut r = wild(me(&[mv]), foe(&[SPLASH]), 1);
        let l = until(&mut r, Choice::Move(0), 4, |l| starts(l, "Hp O"));
        assert!(has(&l, &alloc::format!("Hp O 100->{after}")), "{l:?}");
    }
    let mut r = wild(me(&[COUNTER]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    let taken = l
        .iter()
        .find_map(|x| x.strip_prefix("Hp P 100->"))
        .map(|x| 100 - x.parse::<i32>().unwrap())
        .unwrap();
    assert!(
        has(&l, &alloc::format!("Hp O 100->{}", 100 - 2 * taken)),
        "{l:?}"
    );
}

/// The status-like moves: Curse (not a Ghost), Swagger, Belly Drum, Haze, Rest, Recover, Pain
/// Split.
#[test]
fn stat_and_healing_moves() {
    let mut r = wild(me(&[CURSE]), foe(&[SPLASH]), 1);
    r.choose(Choice::Move(0));
    let s = r.b.battler(P).stages;
    assert_eq!((s[Stat::Speed as usize], s[Stat::Atk as usize]), (-1, 1));
    assert_eq!(s[Stat::Def as usize], 1);

    let mut r = wild(me(&[SWAGGER]), foe(&[SPLASH]), 1);
    let l = until(&mut r, Choice::Move(0), 4, |l| {
        has(l, "PkmnBecameConfused@O ~64")
    });
    assert!(
        has(&l, "DefenderStatRose@O Atk StatSharplyRose ~64"),
        "{l:?}"
    );

    let mut r = wild(me(&[BELLY_DRUM, HAZE]), foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnCutHpMaxedAttack@P ~64"), "{l:?}");
    assert_eq!(r.b.mon(P).hp, 50);
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 6);
    let l = r.choose(Choice::Move(1));
    assert!(has(&l, "StatChangesGone@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 0);

    let mut r = wild(me(&[REST, RECOVER, PAIN_SPLIT]), foe(&[SPLASH]), 1);
    r.b.battler_mut(P).mon.hp = 40;
    let l = r.choose(Choice::Move(1));
    assert!(has(&l, "PkmnRegainedHealth@P ~64"), "{l:?}");
    assert_eq!(r.b.mon(P).hp, 90);
    let l = r.choose(Choice::Move(2));
    assert!(has(&l, "SharedPain@P ~64"), "{l:?}");
    assert_eq!((r.b.mon(P).hp, r.b.mon(O).hp), (95, 95));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnWentToSleep@P ~64"), "{l:?}");
    assert_eq!(r.b.mon(P).hp, 100);
    assert!(matches!(r.b.mon(P).status, Status::Sleep(_)));
}

/// `TrySetCantSelectMoveBattleScript`: a disabled, tormented-repeat or taunted status move is
/// refused at selection (the player is asked again); Encore forces the move.
#[test]
fn selection_limits() {
    // Disable: the foe disables the move the player used last.
    let mut r = wild(me(&[POUND, TACKLE]), foe(&[DISABLE]), 1);
    let l = until(&mut r, Choice::Move(0), 10, |l| {
        starts(l, "PkmnMoveWasDisabled")
    });
    assert!(starts(&l, "PkmnMoveWasDisabled"), "{l:?}");
    assert_eq!(r.b.battler(P).fx.disabled, Some(POUND));
    // What a driver asks before choosing (`Battle::move_selectable`).
    let t = crate::data::GameData::battle(&Game);
    assert_eq!(
        (0..4)
            .map(|i| r.b.move_selectable(t, P, i))
            .collect::<Vec<_>>(),
        [false, true, false, false]
    );
    let l = r.choose(Choice::Move(0));
    assert_eq!(l, ["PkmnMoveIsDisabled@P"]);
    assert_eq!(r.b.need, Need::Action);
    assert!(starts(&r.choose(Choice::Move(1)), "UsedMove@P move:1"));

    // Taunt: status moves are refused.
    let mut r = wild(me(&[GROWL, POUND]), foe(&[TAUNT]), 1);
    let l = until(&mut r, Choice::Move(1), 6, |l| {
        starts(l, "PkmnFellForTaunt")
    });
    assert!(starts(&l, "PkmnFellForTaunt"), "{l:?}");
    assert!(!r.b.move_selectable(t, P, 0) && r.b.move_selectable(t, P, 1));
    assert_eq!(r.choose(Choice::Move(0)), ["PkmnCantUseMoveTaunt@P"]);

    // Torment: not the same move twice in a row.
    let mut r = wild(me(&[POUND, TACKLE]), foe(&[TORMENT]), 1);
    let l = until(&mut r, Choice::Move(1), 6, |l| {
        starts(l, "PkmnSubjectedToTorment")
    });
    assert!(starts(&l, "PkmnSubjectedToTorment"), "{l:?}");
    r.choose(Choice::Move(0));
    assert!(!r.b.move_selectable(t, P, 0), "Pound, used last");
    assert!(r.b.move_selectable(t, P, 1));
    assert_eq!(r.choose(Choice::Move(0)), ["PkmnCantUseMoveTorment@P"]);

    // Encore: the encored move is used whatever the choice.
    let mut r = wild(me(&[POUND, GROWL]), foe(&[ENCORE]), 1);
    let l = until(&mut r, Choice::Move(1), 6, |l| starts(l, "PkmnGotEncore"));
    assert!(starts(&l, "PkmnGotEncore"), "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "UsedMove@P move:2"), "{l:?}");
}

/// Side and field effects: Protect, Safeguard, Mist, Spikes on a switch-in, timed weather,
/// Helping Hand (fails in singles: no partner) and Follow Me.
#[test]
fn field_and_side_effects() {
    let mut r = wild(me(&[PROTECT]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnProtectedItself@P ~64"), "{l:?}");
    assert!(!starts(&l, "Hp P"), "{l:?}");

    let mut r = wild(me(&[SAFEGUARD]), foe(&[STUN_SPORE]), 1);
    let l = until(&mut r, Choice::Move(0), 6, |l| {
        has(l, "PkmnUsedSafeguard@P ~64")
    });
    assert!(has(&l, "PkmnCoveredByVeil@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnUsedSafeguard@P ~64"), "{l:?}");
    assert_eq!(r.b.mon(P).status, Status::None);

    let mut r = wild(me(&[MIST]), foe(&[GROWL]), 1);
    let l = until(&mut r, Choice::Move(0), 6, |l| {
        has(l, "PkmnProtectedByMist@P ~64")
    });
    assert!(has(&l, "PkmnShroudedInMist@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnProtectedByMist@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 0);

    // Reflect halves the foe's physical damage.
    let taken = |first| {
        let mut r = wild(me(&[first]), foe(&[POUND]), 3);
        let l = r.choose(Choice::Move(0));
        let hp = l.iter().find_map(|x| x.strip_prefix("Hp P 100->")).unwrap();
        100 - hp.parse::<i32>().unwrap()
    };
    let (plain, screened) = (taken(REAL_SPLASH), taken(REFLECT));
    assert!(
        screened <= plain / 2 + 1 && screened < plain,
        "{plain} {screened}"
    );

    // Spikes: 1/8 of the max HP on entry with one layer.
    let mut r = Run::wild(&[me(&[POUND]), me(&[POUND])], foe(&[SPIKES]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "SpikesScattered@O ~64"), "{l:?}");
    let l = r.choose(Choice::Switch(1));
    assert!(has(&l, "PkmnHurtBySpikes@P ~64"), "{l:?}");
    assert!(has(&l, "Hp P 100->88"), "{l:?}");

    // Rain Dance: five turns, then "The rain stopped.".
    let mut r = wild(me(&[RAIN_DANCE, SPLASH]), foe(&[SPLASH]), 1);
    r.choose(Choice::Move(0));
    assert_eq!(r.b.weather(), Weather::Rain);
    let l = until(&mut r, Choice::Move(1), 6, |l| has(l, "RainStopped@P ~64"));
    assert_eq!(count(&l, "RainContinues@P ~64"), 3, "{l:?}");
    assert!(has(&l, "RainStopped@P ~64"), "{l:?}");
    assert_eq!(r.b.weather(), Weather::None);

    // Hail pelts both non-Ice battlers.
    let mut r = wild(me(&[HAIL]), foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnPeltedByHail@P ~64"), "{l:?}");
    assert!(has(&l, "Hp O 100->94"), "{l:?}");

    let mut r = wild(me(&[HELPING_HAND, FOLLOW_ME]), foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[..2], ["UsedMove@P move:87", "ButItFailed@P ~64"]);
    let l = r.choose(Choice::Move(1));
    assert!(has(&l, "PkmnCenterAttention@P ~64"), "{l:?}");
}

/// The moves that use another move: Metronome, Mirror Move, Sleep Talk.
#[test]
fn calling_moves() {
    let mut r = wild(me(&[METRONOME]), foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[..2], ["UsedMove@P move:84", "Anim P move:84"]);
    assert!(l[2].starts_with("UsedMove@P move:") && l[2] != "UsedMove@P move:84");
    assert_eq!(r.b.mon(P).pp[0], 9);

    let mut r = wild(me(&[MIRROR_MOVE]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "MirrorMoveFailed@P ~64"), "{l:?}");
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[..2], ["UsedMove@P move:86", "UsedMove@P move:56"]);

    let mut r = wild(me(&[SLEEP_TALK, POUND]), foe(&[SPLASH]), 1);
    r.b.battler_mut(P).mon.status = Status::Sleep(4);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l[..3],
        [
            "PkmnFastAsleep@P ~64",
            "UsedMove@P move:85",
            "Anim P move:85"
        ]
    );
    assert!(has(&l, "UsedMove@P move:56"), "{l:?}");
}

/// End-of-turn effects: Wrap (and no escape), Perish Song, Future Sight, Ingrain, Wish,
/// Destiny Bond.
#[test]
fn end_of_turn_effects() {
    let mut r = wild(me(&[WRAP]), foe(&[SPLASH]), 1);
    let l = until(&mut r, Choice::Move(0), 6, |l| {
        has(l, "PkmnWrappedBy@O ~64")
    });
    assert!(has(&l, "PkmnWrappedBy@O ~64"), "{l:?}");
    assert!(has(&l, "PkmnHurtBy@O move:91 ~64"), "{l:?}");
    assert!(super::run_block(&r.b, O).is_some());

    let mut r = wild(me(&[PERISH_SONG, SPLASH]), foe(&[SPLASH]), 1);
    let mut l = r.choose(Choice::Move(0));
    for _ in 0..3 {
        l.extend(r.choose(Choice::Move(1)));
    }
    for n in [3, 2, 1, 0] {
        assert!(
            has(&l, &alloc::format!("PkmnPerishCountFell@P {n} ~64")),
            "{l:?}"
        );
    }
    assert_eq!(r.b.mon(P).hp, 0);

    let mut r = wild(me(&[FUTURE_SIGHT, SPLASH]), foe(&[SPLASH]), 1);
    let mut l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnForesawAttack@P ~64"), "{l:?}");
    l.extend(r.choose(Choice::Move(1)));
    assert!(!starts(&l, "PkmnTookAttack"), "{l:?}");
    l.extend(r.choose(Choice::Move(1)));
    assert!(has(&l, "PkmnTookAttack@O move:90 ~64"), "{l:?}");

    let mut r = wild(me(&[INGRAIN, WISH]), foe(&[SPLASH]), 1);
    r.b.battler_mut(P).mon.hp = 20;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnPlantedRoots@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnAbsorbedNutrients@P ~64"), "{l:?}");
    assert!(super::switch_block(&r.b, P));
    let mut l = r.choose(Choice::Move(1));
    l.extend(r.choose(Choice::Move(1)));
    assert!(starts(&l, "PkmnWishCameTrue"), "{l:?}");

    let weak = mon(NORMAL, 5, [10, 13, 10, 9, 9, 10], &[DESTINY_BOND]);
    let mut r = wild(weak, foe(&[MEGA_HIT]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnTryingToTakeFoe@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnTookFoe@P ~64"), "{l:?}");
    assert_eq!((r.b.mon(P).hp, r.b.mon(O).hp), (0, 0));
}

/// Hit variants: Facade doubles under a status, Rage builds, an OHKO move fails on a higher
/// level, Roar ends a wild battle.
#[test]
fn hit_variants() {
    let facade = |status| {
        let mut r = wild(me(&[FACADE]), foe(&[SPLASH]), 1);
        r.b.battler_mut(P).mon.status = status;
        let l = until(&mut r, Choice::Move(0), 4, |l| starts(l, "Hp O"));
        let hp = l.iter().find_map(|x| x.strip_prefix("Hp O 100->")).unwrap();
        100 - hp.parse::<i32>().unwrap()
    };
    let (plain, burned) = (facade(Status::None), facade(Status::Paralysis));
    assert!(burned > plain * 3 / 2, "{plain} {burned}");

    let mut r = wild(me(&[RAGE]), foe(&[POUND]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnRageBuilding@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 1);

    let mut r = wild(me(&[KNOCK_OFF]), foe(&[SPLASH]), 1);
    r.b.battler_mut(O).x.item = Some(crate::data::ItemId(1));
    let l = until(&mut r, Choice::Move(0), 4, |l| starts(l, "Hp O"));
    assert!(starts(&l, "PkmnKnockedOff@P"), "{l:?}");
    assert_eq!(r.b.battler(O).x.item, None);

    let low = mon(NORMAL, 4, [100, 13, 10, 9, 9, 10], &[OHKO]);
    let mut r = wild(low, foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnUnaffected@O ~64"), "{l:?}");

    let mut r = wild(me(&[ROAR]), foe(&[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "End(PlayerTeleported)"), "{l:?}");
    assert_eq!(r.b.need, Need::Ended(BattleOutcome::PlayerTeleported));
}

/// Role Play, Skill Swap, Grudge, Imprison, Magic Coat, Trick.
#[test]
fn ability_item_and_seal_moves() {
    use crate::mon::Ability;
    let mut r = wild(me(&[ROLE_PLAY, SKILL_SWAP]), foe(&[SPLASH]), 1);
    r.b.battler_mut(O).ability = Ability::Guts;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnCopiedFoe@P AbilityName(Guts) ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).ability, Ability::Guts);
    r.b.battler_mut(P).ability = Ability::RunAway;
    let l = r.choose(Choice::Move(1));
    assert!(has(&l, "PkmnSwappedAbilities@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(O).ability, Ability::RunAway);

    // Grudge: the move that knocks the user out loses its PP.
    let weak = mon(NORMAL, 5, [10, 13, 10, 9, 9, 10], &[GRUDGE]);
    let mut r = wild(weak, foe(&[MEGA_HIT]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnWantsGrudge@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnLostPPGrudge@O move:11 ~64"), "{l:?}");
    assert_eq!(r.b.mon(O).pp[0], 0);

    // Imprison: the foe cannot select a shared move.
    let mut r = wild(me(&[IMPRISON, POUND]), foe(&[POUND, TACKLE]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnSealedOpponentMove@P ~64"), "{l:?}");
    for _ in 0..4 {
        assert!(!has(&r.choose(Choice::Move(1)), "UsedMove@O move:56"));
    }

    // Magic Coat sends Torment back.
    let mut r = wild(me(&[MAGIC_COAT]), foe(&[TORMENT]), 1);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnShroudedItself@P ~64"), "{l:?}");
    assert!(has(&l, "PkmnMoveBounced@O ~64"), "{l:?}");
    assert!(r.b.battler(O).fx.tormented && !r.b.battler(P).fx.tormented);

    // Trick swaps the items (the player's only).
    let mut r = wild(me(&[TRICK]), foe(&[SPLASH]), 1);
    r.b.battler_mut(O).x.item = Some(crate::data::ItemId(1));
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnSwitchedItems@P ~64"), "{l:?}");
    assert_eq!(r.b.battler(P).x.item, Some(crate::data::ItemId(1)));
    assert_eq!(r.b.battler(O).x.item, None);
}
