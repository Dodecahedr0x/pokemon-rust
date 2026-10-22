//! Task G: the move effects and abilities of Act 1 (new game → the first badge). Same format
//! as [`super::tests`]; hand goldens cite the decomp script or function.

use super::tests::{draws, foe, has, me, nth, seed_where, wild};
use crate::battle::core::tests::{Run, lines, trainer};
use crate::battle::core::{Action, Battle, Env};
use crate::battle::test_data::*;
use crate::battle::{BattleOutcome, Choice, Need, Side};
use crate::mon::{Gender, Pokemon, Stat, Status};
use crate::rng::Rng;
use alloc::string::String;
use alloc::vec::Vec;

const P: Side = Side::Player;
const O: Side = Side::Opponent;

/// `f` on the battle with `rng` = `Rng::new(seed)`: its result, the RNG after it and the
/// events it pushed.
fn run_env<T>(
    r: &mut Run,
    seed: u32,
    f: impl FnOnce(&mut Battle, &mut Env) -> T,
) -> (T, Rng, Vec<String>) {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    let mut env = Env::new(&TABLES, &mut rng, &mut out);
    let t = f(&mut r.b, &mut env);
    (t, rng, lines(&out))
}

/// `side` uses its move slot 0 (`HandleAction_UseMove`, canceler included).
fn use_slot0(r: &mut Run, seed: u32, side: Side) -> (Rng, Vec<String>) {
    let ((), rng, l) = run_env(r, seed, |b, env| b.use_move(env, side, Action::Move(0)));
    (rng, l)
}

/// A male test mon (`personality & 0xFF` ≥ the 127 gender ratio); `mon` is female.
fn male(mut p: Pokemon) -> Pokemon {
    p.personality = 0xFF;
    p
}

// ---- Attract, Cute Charm, Oblivious ----

#[test]
fn attract_needs_opposite_genders() {
    // BattleScript_EffectAttract: attackstring, accuracycheck (one roll), tryinfatuating.
    let hit = 1;
    let mut r = wild(me(WATER, &[ATTRACT]), male(foe(NORMAL, &[SPLASH])), 1);
    assert_eq!(r.b.mon(P).gender, Gender::Female);
    let (rng, l) = use_slot0(&mut r, hit, P);
    assert_eq!(
        l,
        [
            "UsedMove@P move:42",
            "Anim P move:42",
            "PkmnFellInLove@O ~64"
        ]
    );
    assert_eq!(rng, draws(hit, 1));
    assert!(r.b.battler(O).v.infatuated);
    // Already infatuated, same gender, genderless: "But it failed!".
    let (_, l) = use_slot0(&mut r, hit, P);
    assert_eq!(l[1], "ButItFailed@P ~64");
    for f in [foe(NORMAL, &[SPLASH]), foe(BLOB, &[SPLASH])] {
        let mut r = wild(me(WATER, &[ATTRACT]), f, 1);
        let (_, l) = use_slot0(&mut r, hit, P);
        assert_eq!(l[1], "ButItFailed@P ~64");
        assert!(!r.b.battler(O).v.infatuated);
    }
    // Oblivious: BattleScript_ObliviousPreventsAttraction names the target's ability.
    let mut r = wild(me(WATER, &[ATTRACT]), male(foe(OBLIVIOUS, &[SPLASH])), 1);
    let (_, l) = use_slot0(&mut r, hit, P);
    assert_eq!(l[1], "PkmnPreventsRomanceWith@O AbilityName(Oblivious) ~64");
}

#[test]
fn love_canceler_rolls_and_source_switch_ends_it() {
    // CANCELER_IN_LOVE (after paralysis): `Random() & 1`: 1 attacks after "X is in love with
    // Y!" (`B_SCR_ACTIVE` = the loved battler), 0 adds "immobilized by love" and cancels.
    for attacks in [true, false] {
        let seed = seed_where(|s| (nth(s, 0) & 1 == 1) == attacks);
        let mut r = wild(me(WATER, &[REAL_SPLASH]), male(foe(NORMAL, &[SPLASH])), 1);
        r.b.battler_mut(P).v.infatuated = true;
        let (rng, l) = use_slot0(&mut r, seed, P);
        assert_eq!(l[0], "PkmnInLove@O ~64");
        if attacks {
            assert_eq!(l[1], "UsedMove@P move:37");
            assert_eq!(rng, draws(seed, 1));
        } else {
            assert_eq!(l, ["PkmnInLove@O ~64", "PkmnImmobilizedByLove@P ~64"]);
            assert!(r.b.battler(P).v.unable);
        }
    }
    // SwitchInClearSetData / FaintClearSetData: the source leaving ends the infatuation.
    let mut r = Run::wild(&[me(WATER, &[TACKLE])], foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(P).v.infatuated = true;
    r.b.battler_mut(P).v.escape_prevented = true;
    r.b.clear_bonds(O);
    assert!(!r.b.battler(P).v.infatuated && !r.b.battler(P).v.escape_prevented);
}

#[test]
fn cute_charm_infatuates_a_contact_attacker() {
    // ABILITYEFFECT_ON_DAMAGE, Cute Charm: `Random() % 3 == 0`, other gender, not Oblivious.
    for proc in [true, false] {
        let seed = seed_where(|s| nth(s, 0).is_multiple_of(3) == proc);
        let mut r = wild(male(me(WATER, &[TACKLE])), foe(CHARMER, &[SPLASH]), 1);
        let (_, rng, l) = run_env(&mut r, seed, |b, env| {
            let u = b.move_use(env, P, TACKLE, Some(0));
            super::abilities::move_end(b, env, &u, super::tests::contact_hit());
        });
        assert_eq!(rng, draws(seed, 1));
        assert_eq!(r.b.battler(P).v.infatuated, proc);
        if proc {
            assert_eq!(l, ["PkmnSXInfatuatedY@O AbilityName(CuteCharm) ~64"]);
        }
    }
    // Same gender: the roll is drawn, nothing happens.
    let seed = seed_where(|s| nth(s, 0).is_multiple_of(3));
    let mut r = wild(me(WATER, &[TACKLE]), foe(CHARMER, &[SPLASH]), 1);
    let (_, rng, l) = run_env(&mut r, seed, |b, env| {
        let u = b.move_use(env, P, TACKLE, Some(0));
        super::abilities::move_end(b, env, &u, super::tests::contact_hit());
    });
    assert_eq!((rng, l.len()), (draws(seed, 1), 0));
}

// ---- Mean Look, Magnet Pull, Teleport ----

#[test]
fn block_traps_without_an_accuracy_roll() {
    // BattleScript_EffectMeanLook: `accuracycheck NO_ACC_CALC` draws nothing.
    let mut r = wild(me(WATER, &[BLOCK]), foe(NORMAL, &[SPLASH]), 1);
    let (rng, l) = use_slot0(&mut r, 5, P);
    assert_eq!(
        l,
        [
            "UsedMove@P move:43",
            "Anim P move:43",
            "TargetCantEscapeNow@O ~64"
        ]
    );
    assert_eq!(rng, Rng::new(5));
    let (_, l) = use_slot0(&mut r, 5, P);
    assert_eq!(l[1], "ButItFailed@P ~64");
}

#[test]
fn trapped_player_cannot_run_or_switch() {
    // IsRunningFromBattleImpossible: STATUS2_ESCAPE_PREVENTION → B_MSG_CANT_ESCAPE (a
    // selection string); the party menu refuses the switch.
    let mut r = Run::wild(
        &[me(WATER, &[TACKLE]), me(FIRE, &[TACKLE])],
        foe(NORMAL, &[SPLASH]),
        1,
    );
    r.b.battler_mut(P).v.escape_prevented = true;
    assert_eq!(r.choose(Choice::Run), ["CantEscape@P"]);
    assert_eq!(r.choose(Choice::Switch(1)), Vec::<String>::new());
    assert_eq!(r.b.need(), Need::Action);
    // Magnet Pull holds a Steel battler: B_MSG_PREVENTS_ESCAPE names the foe's ability.
    let mut r = wild(me(STEEL, &[TACKLE]), foe(MAGNET, &[SPLASH]), 1);
    assert_eq!(
        r.choose(Choice::Run),
        ["PreventsEscape@O AbilityName(MagnetPull)"]
    );
    // Run Away still escapes.
    let mut r = wild(me(DARK, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(P).v.escape_prevented = true;
    assert!(has(&r.choose(Choice::Run), "GotAwaySafely@P !"));
}

#[test]
fn teleport_ends_a_wild_battle() {
    // BattleScript_EffectTeleport → setoutcomeonteleport: B_OUTCOME_PLAYER_TELEPORTED, no
    // closing message (HandleEndTurn_FinishBattle).
    let mut r = wild(me(WATER, &[TELEPORT]), foe(NORMAL, &[SPLASH]), 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l,
        [
            "UsedMove@P move:46",
            "Anim P move:46",
            "PkmnFledFromBattle@P ~64",
            "End(PlayerTeleported)"
        ]
    );
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::PlayerTeleported));
    // The wild mon's Teleport: B_OUTCOME_MON_TELEPORTED.
    let slow = crate::battle::test_data::mon(WATER, 5, [20, 13, 10, 1, 9, 10], &[REAL_SPLASH]);
    let fast = crate::battle::test_data::mon(NORMAL, 5, [20, 10, 10, 9, 10, 10], &[TELEPORT]);
    let mut r = wild(slow, fast, 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l.last().unwrap(), "End(MonTeleported)");
    assert!(!l.iter().any(|x| x.starts_with("UsedMove@P")), "{l:?}");
    // A trainer battle, a trapped user: "But it failed!"; Magnet Pull "made it ineffective".
    let mut r = Run::new(
        trainer(),
        &[me(WATER, &[TELEPORT])],
        &[foe(NORMAL, &[SPLASH])],
        1,
    );
    assert_eq!(use_slot0(&mut r, 1, P).1[1], "ButItFailed@P ~64");
    let mut r = wild(me(WATER, &[TELEPORT]), foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(P).v.escape_prevented = true;
    assert_eq!(use_slot0(&mut r, 1, P).1[1], "ButItFailed@P ~64");
    let mut r = wild(me(STEEL, &[TELEPORT]), foe(MAGNET, &[SPLASH]), 1);
    assert_eq!(
        use_slot0(&mut r, 1, P).1[1],
        "PkmnSXMadeItIneffective@O AbilityName(MagnetPull) ~64"
    );
}

// ---- Mud Sport, Water Sport ----

#[test]
fn sports_are_set_once_and_end_with_the_user() {
    let mut r = wild(
        me(WATER, &[MUD_SPORT, WATER_SPORT]),
        foe(NORMAL, &[SPLASH]),
        1,
    );
    let (rng, l) = use_slot0(&mut r, 3, P);
    assert_eq!(
        l,
        [
            "UsedMove@P move:44",
            "Anim P move:44",
            "ElectricityWeakened@P ~64"
        ]
    );
    assert_eq!(rng, Rng::new(3));
    assert!(r.b.field().mud_sport && !r.b.field().water_sport);
    assert_eq!(use_slot0(&mut r, 3, P).1[1], "ButItFailed@P ~64");
    let ((), _, l) = run_env(&mut r, 3, |b, env| b.use_move(env, P, Action::Move(1)));
    assert_eq!(l[2], "FireWeakened@P ~64");
    // FaintClearSetData clears STATUS3.
    r.b.battler_mut(P).mon.hp = 0;
    run_env(&mut r, 3, |b, env| b.try_faint(env, P));
    assert_eq!(r.b.field(), crate::battle::calc::Field::default());
}

#[test]
fn mud_sport_halves_electric_damage() {
    // Same rolls with and without Mud Sport: 40-power Thunder Shock (SpA 9 vs SpD 10, L5):
    // 9*40*4/10/50 + 2 = 4, halved power: 9*20*4/10/50 + 2 = 3 (×100/100 at roll 0 % 16).
    // Draws: accuracy, crit (`% 16 != 0`: none), damage (`% 16 == 0`: the top roll).
    let seed = seed_where(|s| nth(s, 2).is_multiple_of(16) && !nth(s, 1).is_multiple_of(16));
    let mut hp = [0; 2];
    for (i, sport) in [false, true].into_iter().enumerate() {
        let mut r = wild(me(WATER, &[THUNDER_SHOCK]), foe(NORMAL, &[SPLASH]), 1);
        r.b.battler_mut(O).v.mud_sport = sport;
        use_slot0(&mut r, seed, P);
        hp[i] = 20 - r.b.mon(O).hp;
    }
    assert_eq!(hp, [4, 3]);
}

// ---- Uproar ----

#[test]
fn uproar_locks_two_to_five_turns_and_wakes_sleepers() {
    // MOVE_EFFECT_UPROAR (seteffectwithchance, chance 100: its roll first) draws
    // `(Random() & 3) + 2` turns; ENDTURN_UPROAR wakes every sleeper (not Soundproof) then
    // counts down: "making an UPROAR" while it lasts, "calmed down" at 0. Locked turns use no
    // PP and nobody is asked.
    let seed = seed_where(|s| nth(s, 4) & 3 == 0);
    let mut sleeper = foe(NORMAL, &[SPLASH]);
    sleeper.status = Status::Sleep(5);
    let mut r = wild(me(WATER, &[UPROAR]), sleeper, 1);
    let (rng, l) = use_slot0(&mut r, seed, P);
    assert_eq!(rng, draws(seed, 5), "accuracy, crit, damage, chance, turns");
    assert_eq!(l.last().unwrap(), "PkmnCausedUproar@P ~64");
    assert_eq!(r.b.battler(P).v.uproar, 2);
    assert_eq!(r.b.mon(P).pp[0], 9);
    // A full battle: one choice runs both turns (the player is locked on the second).
    let mut r = wild(
        me(WATER, &[UPROAR]),
        {
            let mut f = foe(NORMAL, &[SPLASH]);
            f.status = Status::Sleep(5);
            f
        },
        1,
    );
    r.b.battler_mut(O).mon.hp = 999;
    r.b.battler_mut(O).mon.max_hp = 999;
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnWokeUpInUproar@O ~64"), "{l:?}");
    let caused = l.iter().filter(|x| *x == "PkmnCausedUproar@P ~64").count();
    assert_eq!(caused, 1, "{l:?}");
    let making = l.iter().filter(|x| *x == "PkmnMakingUproar@P ~64").count();
    let used = l.iter().filter(|x| *x == "UsedMove@P move:47").count();
    assert_eq!(used, making + 1, "{l:?}");
    assert!(has(&l, "PkmnCalmedDown@P ~64"), "{l:?}");
    assert_eq!(r.b.mon(P).pp[0], 9, "PP only on the first turn");
    assert_eq!(r.b.battler(P).v.uproar, 0);
    assert_eq!(r.b.need(), Need::Action);
}

#[test]
fn uproar_keeps_targets_awake() {
    // BattleScript_EffectSleep → jumpifcantmakeasleep: gUproarAwakeStringIds, by whether the
    // target is the uproarer; Insomnia "stayed awake using".
    let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(O).v.uproar = 2;
    assert_eq!(use_slot0(&mut r, 1, P).1[1], "PkmnCantSleepInUproar@O ~64");
    let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(P).v.uproar = 2;
    assert_eq!(use_slot0(&mut r, 1, P).1[1], "UproarKeptPkmnAwake@O ~64");
    // Soundproof sleeps through it.
    let seed = seed_where(|s| nth(s, 0) % 100 < 75);
    let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(SOUNDPROOF, &[SPLASH]), 1);
    r.b.battler_mut(P).v.uproar = 2;
    use_slot0(&mut r, seed, P);
    assert!(matches!(r.b.mon(O).status, Status::Sleep(_)));
    let mut r = wild(me(WATER, &[SLEEP_POWDER]), foe(INSOMNIAC, &[SPLASH]), 1);
    assert_eq!(
        use_slot0(&mut r, 1, P).1[1],
        "PkmnStayedAwakeUsing@O AbilityName(Insomnia) ~64"
    );
    // A sleeping attacker wakes in an uproar (CANCELER_ASLEEP, B_MSG_WOKE_UP_UPROAR).
    let mut m = me(WATER, &[REAL_SPLASH]);
    m.status = Status::Sleep(4);
    let mut r = wild(m, foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(O).v.uproar = 2;
    let (_, l) = use_slot0(&mut r, 1, P);
    assert_eq!(
        l[..2],
        [
            "PkmnWokeUpInUproar@P ~64",
            "StatusIcon { side: Player, status: None }"
        ]
    );
    assert_eq!(l[2], "UsedMove@P move:37");
}

#[test]
fn soundproof_blocks_sound_moves() {
    // ABILITYEFFECT_MOVES_BLOCK → BattleScript_SoundproofProtected: no accuracy roll.
    let mut r = wild(me(WATER, &[SOUND_GROWL]), foe(SOUNDPROOF, &[SPLASH]), 1);
    let (rng, l) = use_slot0(&mut r, 9, P);
    assert_eq!(
        l,
        [
            "UsedMove@P move:53",
            "PkmnSXBlocksY@O AbilityName(Soundproof) ~64"
        ]
    );
    assert_eq!(rng, Rng::new(9));
    assert_eq!(r.b.mon(P).pp[0], 39);
}

// ---- Yawn ----

#[test]
fn yawn_sleeps_at_the_end_of_the_next_turn() {
    // BattleScript_EffectYawn: no accuracy roll; STATUS3_YAWN_TURN(2). ENDTURN_YAWN: at 0,
    // `(Random() & 3) + 2` turns of sleep and STRINGID_PKMNFELLASLEEP (B_EFF = the target).
    let mut r = wild(
        me(WATER, &[YAWN, REAL_SPLASH]),
        foe(NORMAL, &[REAL_SPLASH]),
        1,
    );
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "PkmnWasMadeDrowsy@O ~64"), "{l:?}");
    assert!(!has(&l, "PkmnFellAsleep@O ~64"), "{l:?}");
    assert_eq!(r.b.battler(O).v.yawn, 1);
    let l = r.choose(Choice::Move(1));
    let i = l
        .iter()
        .position(|x| x == "PkmnFellAsleep@O ~64")
        .expect("asleep");
    assert!(
        l[i + 1].starts_with("StatusIcon { side: Opponent, status: Sleep("),
        "{l:?}"
    );
    // Insomnia "made it ineffective"; a statused or drowsy target: "But it failed!".
    let mut r = wild(me(WATER, &[YAWN]), foe(INSOMNIAC, &[SPLASH]), 1);
    assert_eq!(
        use_slot0(&mut r, 1, P).1[1],
        "PkmnSXMadeItIneffective@O AbilityName(Insomnia) ~64"
    );
    let mut f = foe(NORMAL, &[SPLASH]);
    f.status = Status::Poison;
    let mut r = wild(me(WATER, &[YAWN]), f, 1);
    assert_eq!(use_slot0(&mut r, 1, P).1[1], "ButItFailed@P ~64");
}

// ---- Double Kick, Foresight, Fury Cutter, Pursuit ----

#[test]
fn double_kick_hits_twice_without_a_count_roll() {
    // BattleScript_EffectDoubleHit: `setmultihitcounter 2`: accuracy, (crit, damage) × 2, then
    // the secondary roll.
    let seed = 1;
    let mut r = wild(me(WATER, &[DOUBLE_KICK]), foe(NORMAL, &[SPLASH]), 1);
    let (rng, l) = use_slot0(&mut r, seed, P);
    assert_eq!(rng, draws(seed, 6));
    assert_eq!(l.iter().filter(|x| *x == "Hit O Normal").count(), 2);
    assert!(has(&l, "HitXTimes@P 2 ~64"), "{l:?}");
}

#[test]
fn foresight_identifies_a_ghost() {
    let seed = 1;
    let mut r = wild(me(WATER, &[FORESIGHT, TACKLE]), foe(GHOST, &[SPLASH]), 1);
    let (rng, l) = use_slot0(&mut r, seed, P);
    assert_eq!(
        l,
        [
            "UsedMove@P move:50",
            "Anim P move:50",
            "PkmnIdentified@O ~64"
        ]
    );
    assert_eq!(rng, draws(seed, 1));
    assert!(r.b.battler(O).foresight);
    let (immune, _, _) = run_env(&mut r, 1, |b, env| {
        let u = b.move_use(env, P, TACKLE, Some(1));
        b.immune(env, &u)
    });
    assert!(!immune);
}

#[test]
fn fury_cutter_doubles_until_a_miss() {
    // Cmd_furycuttercalc: the counter rises to 5 per hit (power 10 << (n - 1)); a miss resets
    // it. Attack string first, then the accuracy roll.
    let hit = seed_where(|s| nth(s, 0) % 100 < 95);
    let miss = seed_where(|s| nth(s, 0) % 100 >= 95);
    let mut r = wild(me(BUG, &[FURY_CUTTER]), foe(NORMAL, &[SPLASH]), 1);
    r.b.battler_mut(O).mon.hp = 999;
    r.b.battler_mut(O).mon.max_hp = 999;
    for n in [1, 2, 3, 4, 5, 5] {
        use_slot0(&mut r, hit, P);
        assert_eq!(r.b.battler(P).v.fury_cutter, n);
    }
    let (_, l) = use_slot0(&mut r, miss, P);
    assert_eq!(l, ["UsedMove@P move:51", "AttackMissed@P ~64"]);
    assert_eq!(r.b.battler(P).v.fury_cutter, 0);
    // The power golden: Atk 13, Def 10, L5, Bug STAB: 13*10*4/10/50 + 2 = 3 → 4 (×1.5) at
    // counter 1; 13*160*4/10/50 + 2 = 18 → 27 at counter 5 (roll 0 % 16, no crit).
    let seed = seed_where(|s| {
        nth(s, 0) % 100 < 95 && !nth(s, 1).is_multiple_of(16) && nth(s, 2).is_multiple_of(16)
    });
    for (counter, dealt) in [(0, 4), (4, 27)] {
        let mut r = wild(me(BUG, &[FURY_CUTTER]), foe(NORMAL, &[SPLASH]), 1);
        r.b.battler_mut(O).mon.hp = 999;
        r.b.battler_mut(P).v.fury_cutter = counter;
        use_slot0(&mut r, seed, P);
        assert_eq!(999 - r.b.mon(O).hp, dealt);
    }
}

#[test]
fn pursuit_strikes_a_switching_foe_first_at_double_damage() {
    // BattleScript_ActionSwitch → jumpifnopursuitswitchdmg: the foe's Pursuit action is
    // cancelled and hits the switching battler first, sDMG_MULTIPLIER 2, no accuracy roll.
    // Foe Atk 10 vs Def 10, L5, 40 power: 4×40×10/10/50 + 2 = 5, ×2 = 10, Dark STAB 15, then
    // seed 3's rolls: no crit, and 15 × (100 - Random() % 16) / 100 = 13.
    let mut r = Run::wild(
        &[me(WATER, &[TACKLE]), me(FIRE, &[TACKLE])],
        foe(DARK, &[PURSUIT]),
        3,
    );
    let l = r.choose(Choice::Switch(1));
    assert_eq!(
        l[..7],
        [
            "UsedMove@O move:52",
            "Anim O move:52",
            "Hit P Normal",
            "Hp P 20->7",
            "Recall P",
            "GoPkmn@P nick:P1",
            "SendOut P1",
        ],
        "{l:?}"
    );
    assert_eq!(r.b.party(P).get(0).unwrap().hp, 7);
    let used = l.iter().filter(|x| x.starts_with("UsedMove@O")).count();
    assert_eq!(used, 1, "{l:?}");
    assert_eq!(r.b.mon(P).party, 1);
}

#[test]
fn pursuit_ko_gives_exp_and_the_chosen_mon_still_comes_in() {
    // A trainer's mon switching out under the player's Pursuit faints: `getexp` in
    // BattleScript_PursuitDmgOnSwitchOut, no `returnatktoball` for a fainted battler, then the
    // chosen mon is sent out (BattleScript_DoSwitchOut).
    let make = |s: u32| {
        let lead = mon(NORMAL, 5, [20, 10, 10, 5, 10, 10], &[TACKLE]);
        let ghost = mon(GHOST, 5, [20, 10, 10, 5, 10, 10], &[ABSORB]);
        let mut r = Run::new(
            trainer(),
            &[me(WATER, &[TACKLE, PURSUIT])],
            &[lead, ghost],
            s,
        );
        r.b.trainer_ai = 0;
        r.choose(Choice::Move(0));
        r
    };
    // ShouldSwitch after the landed Tackle: the Ghost is immune to it (one `Random() % 2`).
    let seed = seed_where(|s| {
        let r = make(s);
        r.b.mon(O).hp > 0 && r.b.actions[O as usize] == Some(Action::Switch(1))
    });
    let mut r = make(seed);
    r.b.battler_mut(O).mon.hp = 1;
    let l = r.choose(Choice::Move(1));
    assert_eq!(l[0], "UsedMove@P move:52", "{l:?}");
    let faint = l.iter().position(|x| x == "Faint O").expect("faint");
    let exp = l
        .iter()
        .position(|x| x.starts_with("PkmnGainedExp@P"))
        .expect("exp");
    let sent = l.iter().position(|x| x == "SendOut O1").expect("send out");
    assert!(faint < exp && exp < sent, "{l:?}");
    assert!(!has(&l, "Recall O"), "{l:?}");
    assert_eq!(r.b.mon(O).party, 1);
    assert_eq!(r.b.need(), Need::Action);
    // The player's switching mon knocked out by the foe's Pursuit: no Recall, the chosen mon
    // comes in and the battle goes on.
    let mut m = me(WATER, &[TACKLE]);
    m.hp = 1;
    let mut r = Run::wild(&[m, me(FIRE, &[TACKLE])], foe(DARK, &[PURSUIT]), 1);
    let l = r.choose(Choice::Switch(1));
    assert!(has(&l, "Faint P"), "{l:?}");
    assert!(!has(&l, "Recall P"), "{l:?}");
    assert!(has(&l, "SendOut P1"), "{l:?}");
    assert_eq!(r.b.mon(P).party, 1);
    assert_eq!(r.b.need(), Need::Action);
}

#[test]
fn pursuit_needs_an_able_user() {
    // jumpifnopursuitswitchdmg: an asleep (or frozen) or loafing Pursuit user does not strike;
    // the switch goes first as usual.
    let mut f = foe(DARK, &[PURSUIT]);
    f.status = Status::Sleep(3);
    let mut r = Run::wild(&[me(WATER, &[TACKLE]), me(FIRE, &[TACKLE])], f, 1);
    let l = r.choose(Choice::Switch(1));
    assert_eq!(
        l[..3],
        ["Recall P", "GoPkmn@P nick:P1", "SendOut P1"],
        "{l:?}"
    );
    let mut r = Run::wild(
        &[me(WATER, &[TACKLE]), me(FIRE, &[TACKLE])],
        foe(TRUANT, &[PURSUIT]),
        1,
    );
    r.b.battler_mut(O).v.truant = true;
    let l = r.choose(Choice::Switch(1));
    assert_eq!(
        l[..3],
        ["Recall P", "GoPkmn@P nick:P1", "SendOut P1"],
        "{l:?}"
    );
    assert!(has(&l, "PkmnLoafing@O ~64"), "{l:?}");
    assert_eq!(r.b.party(P).get(0).unwrap().hp, 20);
}

// ---- abilities ----

#[test]
fn truant_loafs_every_other_turn() {
    // CANCELER_TRUANT, then ABILITYEFFECT_ENDTURN flips the counter (0 at the battle's start).
    let mut r = wild(me(TRUANT, &[REAL_SPLASH]), foe(NORMAL, &[REAL_SPLASH]), 1);
    let turns: Vec<bool> = (0..4)
        .map(|_| has(&r.choose(Choice::Move(0)), "PkmnLoafing@P ~64"))
        .collect();
    assert_eq!(turns, [false, true, false, true]);
}

#[test]
fn shed_skin_rolls_mod_three_at_the_end_of_the_turn() {
    for proc in [true, false] {
        let seed = seed_where(|s| nth(s, 0).is_multiple_of(3) == proc);
        let mut f = foe(SHEDDER, &[SPLASH]);
        f.status = Status::Paralysis;
        let mut r = wild(me(WATER, &[TACKLE]), f, 1);
        let (_, rng, l) = run_env(&mut r, seed, |b, env| super::abilities::end_turn(b, env, O));
        assert_eq!(rng, draws(seed, 1));
        assert_eq!(r.b.mon(O).status == Status::None, proc);
        if proc {
            assert_eq!(
                l,
                [
                    "PkmnSXCuredYProblem@O AbilityName(ShedSkin) ~64",
                    "Report(Degraded(\"shed skin status name\"))",
                    "StatusIcon { side: Opponent, status: None }"
                ]
            );
        }
    }
}

#[test]
fn inner_focus_names_itself_for_a_certain_flinch_only() {
    // SetMoveEffect(MOVE_EFFECT_FLINCH): BattleScript_FlinchPrevention when the effect is
    // primary or MOVE_EFFECT_CERTAIN (Fake Out's); a secondary flinch, even at 100%, is
    // blocked silently.
    let mut r = wild(me(WATER, &[FLINCH_ALL]), foe(FOCUSED, &[TACKLE]), 3);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "Hit O Normal"), "{l:?}");
    assert!(!l.iter().any(|x| x.contains("Flinch")), "{l:?}");
    let ((), _, l) = run_env(&mut r, 1, |b, env| {
        let u = b.move_use(env, P, FLINCH_ALL, Some(0));
        super::status::flinch(b, env, &u, O, super::status::Cause::SECONDARY, true);
    });
    assert_eq!(l, ["PkmnSXPreventsFlinching@O AbilityName(InnerFocus) ~64"]);
}

#[test]
fn early_bird_wakes_twice_as_fast() {
    // CANCELER_ASLEEP: `toSub` 2 with Early Bird; 2 turns left wake at once.
    let mut m = me(EARLY, &[REAL_SPLASH]);
    m.status = Status::Sleep(2);
    let mut r = wild(m, foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, 1, P);
    assert_eq!(l[0], "PkmnWokeUp@P ~64");
    let mut m = me(EARLY, &[REAL_SPLASH]);
    m.status = Status::Sleep(3);
    let mut r = wild(m, foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, 1, P);
    assert_eq!(l, ["PkmnFastAsleep@P ~64"]);
    assert_eq!(r.b.mon(P).status, Status::Sleep(1));
}

#[test]
fn rock_head_has_no_recoil_but_struggle_does() {
    let seed = seed_where(|s| nth(s, 0) % 100 < 85);
    let mut r = wild(me(ROCKHEAD, &[TAKE_DOWN]), foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, seed, P);
    assert!(l.iter().any(|x| x == "Hit O Normal"), "{l:?}");
    assert!(
        !l.iter().any(|x| x.starts_with("PkmnHitWithRecoil")),
        "{l:?}"
    );
    assert_eq!(r.b.mon(P).hp, 20);
    let mut r = wild(me(ROCKHEAD, &[TAKE_DOWN]), foe(NORMAL, &[SPLASH]), 1);
    let ((), _, l) = run_env(&mut r, seed, |b, env| b.use_move(env, P, Action::Struggle));
    assert!(has(&l, "PkmnHitWithRecoil@P ~64"), "{l:?}");
}

#[test]
fn compound_eyes_raises_accuracy() {
    // Cmd_accuracycheck: 95 × 130/100 = 123: a 99 roll still hits.
    let seed = seed_where(|s| nth(s, 0) % 100 == 99);
    let mut r = wild(me(EYES, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, seed, P);
    assert!(has(&l, "Hit O Normal"), "{l:?}");
    let mut r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, seed, P);
    assert!(has(&l, "AttackMissed@P ~64"), "{l:?}");
}

#[test]
fn volatiles_hash_only_when_set() {
    let r = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    let base = crate::battle::core::tests::hash(&r.b);
    let mut r2 = wild(me(WATER, &[TACKLE]), foe(NORMAL, &[SPLASH]), 1);
    assert_eq!(crate::battle::core::tests::hash(&r2.b), base);
    r2.b.battler_mut(O).v.yawn = 1;
    assert_ne!(crate::battle::core::tests::hash(&r2.b), base);
}

#[test]
fn clear_body_keeps_its_stats() {
    // ChangeStatBuffs: a status move's drop → BattleScript_AbilityNoStatLoss; a secondary
    // drop fails silently.
    let mut r = wild(me(WATER, &[GROWL]), foe(CLEAR, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, 1, P);
    assert_eq!(
        l[1..],
        ["PkmnPreventsStatLossWith@O AbilityName(ClearBody) ~64"]
    );
    assert_eq!(r.b.battler(O).stages[Stat::Atk as usize], 0);
    let seed = seed_where(|s| nth(s, 0) % 100 < 95);
    let mut r = wild(me(WATER, &[ICY_WIND]), foe(CLEAR, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, seed, P);
    assert!(!l.iter().any(|x| x.contains("ClearBody")), "{l:?}");
    assert_eq!(r.b.battler(O).stages[Stat::Speed as usize], 0);
    // BattleScript_IntimidatePrevented: "X's CLEAR BODY prevented Y's INTIMIDATE from
    // working!", before any stage check.
    let r = wild(me(CLEAR, &[TACKLE]), foe(INTIM, &[REAL_SPLASH]), 3);
    assert!(
        has(
            &r.intro(),
            "PreventedFromWorking@O AbilityName(Intimidate) AbilityName(ClearBody) ~64"
        ),
        "{:?}",
        r.intro()
    );
    assert_eq!(r.b.battler(P).stages[Stat::Atk as usize], 0);
}

#[test]
fn liquid_ooze_hurts_the_drainer() {
    // BattleScript_AbsorbLiquidOoze: `manipulatedamage DMG_CHANGE_SIGN`, B_MSG_ABSORB_OOZE.
    // Absorb (Grass) is super effective on the Water foe: the attacker loses max(1, dealt / 2).
    let mut r = wild(me(WATER, &[ABSORB]), foe(OOZE, &[REAL_SPLASH]), 3);
    let l = r.choose(Choice::Move(0));
    let dealt = 20 - r.b.mon(O).hp;
    let lost = (dealt / 2).max(1);
    assert_eq!(
        l[5..7],
        [
            alloc::format!("Hp P 20->{}", 20 - lost),
            "ItSuckedLiquidOoze@O ~64".into()
        ]
    );
    // BattleScript_LeechSeedTurnDrain: the seeded Liquid Ooze mon's lost HP hurt the seeder.
    let mut r = wild(me(WATER, &[REAL_SPLASH]), foe(OOZE, &[REAL_SPLASH]), 3);
    r.b.battler_mut(O).seeded = true;
    let l = r.choose(Choice::Move(0));
    let n = l.len();
    assert_eq!(
        l[n - 3..],
        ["Hp O 20->18", "Hp P 20->18", "ItSuckedLiquidOoze@O ~64"]
    );
}

#[test]
fn natural_cure_heals_on_switching_out() {
    // Cmd_switchoutabilities: status1 = 0, silently.
    let mut m = me(CURER, &[TACKLE]);
    m.status = Status::Paralysis;
    let mut r = Run::wild(&[m, me(WATER, &[TACKLE])], foe(NORMAL, &[SPLASH]), 1);
    r.choose(Choice::Switch(1));
    assert_eq!(r.b.party(P).get(0).unwrap().status, Status::None);
}

#[test]
fn trapped_run_away_mon_cannot_switch() {
    // HandleTurnActionSelectionState, B_ACTION_SWITCH: escape prevention gives
    // PARTY_ACTION_CANT_SWITCH whatever the ability (Run Away only helps running), and Magnet
    // Pull holds a Steel battler.
    let mut r = Run::wild(
        &[me(DARK, &[TACKLE]), me(FIRE, &[TACKLE])],
        foe(NORMAL, &[SPLASH]),
        1,
    );
    r.b.battler_mut(P).v.escape_prevented = true;
    assert_eq!(r.choose(Choice::Switch(1)), Vec::<String>::new());
    assert_eq!(r.b.need(), Need::Action);
    assert_eq!(r.b.mon(P).party, 0);
    let mut r = Run::wild(
        &[me(STEEL, &[TACKLE]), me(FIRE, &[TACKLE])],
        foe(MAGNET, &[SPLASH]),
        1,
    );
    assert_eq!(r.choose(Choice::Switch(1)), Vec::<String>::new());
    assert_eq!(r.b.mon(P).party, 0);
}

/// Wins a wild battle with one Tackle against a 1 HP foe: the RNG after it and the events.
fn win_with(party: &[Pokemon], seed: u32) -> (Rng, Vec<String>) {
    let mut f = foe(NORMAL, &[SPLASH]);
    f.hp = 1;
    let mut r = Run::wild(party, f, seed);
    let l = r.choose(Choice::Move(0));
    assert!(has(&l, "End(Won)"), "{l:?}");
    (r.rng, l)
}

#[test]
fn pickup_draws_after_a_win() {
    // Cmd_pickup (BattleScript_PayDayMoneyAndPickUpItems): per party mon with Pickup and no
    // held item (the engine's mons hold nothing), `Random() % 10`, and on 0 `Random() % 100`
    // for the item, which the engine does not give (reported). PICKER is NORMAL with Pickup.
    for found in [false, true] {
        // The draws after the battle's own: compare with a non-Pickup twin.
        let seed = seed_where(|s| {
            let (mut after, _) = win_with(&[me(NORMAL, &[TACKLE])], s);
            after.random().is_multiple_of(10) == found
        });
        let (mut expect, _) = win_with(&[me(NORMAL, &[TACKLE])], seed);
        let (rng, l) = win_with(&[me(PICKER, &[TACKLE])], seed);
        expect.random();
        if found {
            expect.random();
        }
        assert_eq!(rng, expect, "found {found}");
        assert_eq!(has(&l, "Report(Degraded(\"pickup item\"))"), found, "{l:?}");
    }
    // A fainted Pickup mon rolls too (no HP check).
    let mut picker = me(PICKER, &[TACKLE]);
    picker.hp = 0;
    let (mut expect, _) = win_with(&[me(NORMAL, &[TACKLE]), me(NORMAL, &[TACKLE])], 1);
    let (rng, _) = win_with(&[me(NORMAL, &[TACKLE]), picker], 1);
    expect.random();
    assert!(
        rng == expect || {
            expect.random();
            rng == expect
        }
    );
}

#[test]
fn low_kick_power_follows_the_target_weight() {
    // weightdamagecalculation (sWeightToDamageTable, hectograms): the first bracket whose
    // bound is above the weight, else 120. Real weights from pokedex_entries.h.
    use crate::battle::calc::low_kick_power;
    for (weight, power) in [
        (0, 20),
        (36, 20),  // Wurmple
        (100, 40), // a bound is exclusive
        (175, 40), // Zigzagoon
        (200, 40), // Geodude
        (864, 80), // Makuhita
        (970, 80), // Nosepass
        (1000, 100),
        (2000, 120),
        (4600, 120), // Snorlax
    ] {
        assert_eq!(low_kick_power(weight), power, "{weight}");
    }
}

#[test]
fn low_kick_hits_by_weight_and_reports_a_missing_one() {
    // BattleScript_EffectLowKick: attackstring, ppreduce, weightdamagecalculation, then the
    // accuracy roll and BattleScript_HitFromCritCalc (crit, damage, the secondary roll).
    // HEAVY (Rock) weighs 200, as Geodude: 40 power. Atk 13, Def 10, L5:
    // 4×40×13/10/50 + 2 = 6 at the top roll (the test type chart has no Fighting on Rock).
    let seed = seed_where(|s| !nth(s, 1).is_multiple_of(16) && nth(s, 2).is_multiple_of(16));
    let mut r = wild(me(WATER, &[LOW_KICK]), foe(HEAVY, &[SPLASH]), 1);
    let (rng, l) = use_slot0(&mut r, seed, P);
    assert_eq!(rng, draws(seed, 4), "accuracy, crit, damage, secondary");
    assert_eq!(20 - r.b.mon(O).hp, 6, "{l:?}");
    assert!(!l.iter().any(|x| x.starts_with("Report")), "{l:?}");
    // Weight 0 (content without weights yet): Gen3's lowest bracket (20), reported.
    // 4×20×13/10/50 + 2 = 4 at the top roll.
    let mut r = wild(me(WATER, &[LOW_KICK]), foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, seed, P);
    assert!(has(&l, "Report(Degraded(\"low kick weight\"))"), "{l:?}");
    assert_eq!(20 - r.b.mon(O).hp, 4, "{l:?}");
}

#[test]
fn unsupported_effects_are_reported() {
    // Degradation, not silence: the core's fallback names the raw EFFECT_* number.
    let mut r = wild(me(WATER, &[SPLASH]), foe(NORMAL, &[SPLASH]), 1);
    let (_, l) = use_slot0(&mut r, 1, P);
    assert!(has(&l, "Report(UnsupportedEffect(85))"), "{l:?}");
}

#[test]
fn trainer_ai_scoring_pursuit_is_reported() {
    // AI_CV_Pursuit reads `isFirstTurn` and a predicted switch the engine does not model.
    let foes = [mon(DARK, 5, [20, 10, 10, 5, 10, 10], &[PURSUIT])];
    let mut r = Run::new(trainer(), &[me(WATER, &[TACKLE])], &foes, 1);
    r.b.trainer_ai = crate::battle::ai::AI_SCRIPT_CHECK_VIABILITY;
    let l = r.choose(Choice::Move(0));
    assert!(
        has(&l, "Report(Degraded(\"ai pursuit viability\"))"),
        "{l:?}"
    );
    let mut r = Run::new(trainer(), &[me(WATER, &[TACKLE])], &foes, 1);
    r.b.trainer_ai = 0;
    let l = r.choose(Choice::Move(0));
    assert!(!l.iter().any(|x| x.starts_with("Report")), "{l:?}");
}
