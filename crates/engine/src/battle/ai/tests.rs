//! AI tests: score-model choices in hand-checked situations, and the first battle's flee.

use super::*;
use crate::battle::core::tests::{Run, trainer};
use crate::battle::test_data::*;
use crate::battle::{BattleKind, BattleOutcome, Need};
use crate::data::MoveId;
use crate::mon::Pokemon;
use alloc::string::String;
use alloc::vec::Vec;

/// A trainer battle with the given AI flags: the player's mon against `foe_moves`.
fn setup(player: Pokemon, foe_moves: &[MoveId], flags: u32) -> Run {
    let foe = mon(NORMAL, 5, [20, 10, 10, 5, 10, 10], foe_moves);
    let mut r = Run::new(trainer(), &[player], &[foe], 1);
    r.b.trainer_ai = flags;
    r
}

/// The AI's choices over seeds 1..40.
fn choices(r: &Run) -> Vec<u8> {
    (1..40)
        .map(|s| match choose(&r.b, &TABLES, &mut Rng::new(s)) {
            Choice::Move(m) => m,
            c => panic!("{c:?}"),
        })
        .collect()
}

fn target(species: crate::data::SpeciesId, hp: u16) -> Pokemon {
    let mut m = mon(species, 5, [20, 13, 10, 9, 9, 10], &[TACKLE]);
    m.hp = hp;
    m
}

#[test]
fn equal_scores_are_broken_by_random() {
    // No flags: every usable slot keeps 100; `Random() % 2` picks among slots 0 and 1. The
    // empty slots 2–3 are skipped (slot 0 is always a candidate).
    let r = setup(target(WATER, 20), &[TACKLE, GROWL], 0);
    let c = choices(&r);
    assert!(c.contains(&0) && c.contains(&1) && c.iter().all(|&m| m < 2));
}

#[test]
fn setup_draws_four_rolls_then_the_pick() {
    // BattleAI_SetupAIData: `simulatedRNG[i] = 100 - Random() % 16` for each slot, then
    // ChooseMoveOrAction_Singles' `Random() % numOfBestMoves`.
    let r = setup(target(WATER, 20), &[TACKLE, GROWL], 0);
    let mut rng = Rng::new(9);
    choose(&r.b, &TABLES, &mut rng);
    let mut expect = Rng::new(9);
    for _ in 0..5 {
        expect.random();
    }
    assert_eq!(rng, expect);
}

#[test]
fn check_bad_move_avoids_immune_targets() {
    // AI_CheckBadMove: Tackle (Normal) on a Ghost is x0 → -10; Growl (no power) is not
    // checked for type → 100 > 90.
    let r = setup(
        target(GHOST, 20),
        &[TACKLE, GROWL],
        AI_SCRIPT_CHECK_BAD_MOVE,
    );
    assert!(choices(&r).iter().all(|&m| m == 1));
    // Without the flag, both are picked.
    let r = setup(target(GHOST, 20), &[TACKLE, GROWL], 0);
    assert!(choices(&r).contains(&0));
}

#[test]
fn check_bad_move_status_branches() {
    // Sleep Powder on a statused target, Leech Seed on a Grass target, Growl at -6: -10.
    let mut statused = target(WATER, 20);
    statused.status = Status::Poison;
    let r = setup(statused, &[SLEEP_POWDER, TACKLE], AI_SCRIPT_CHECK_BAD_MOVE);
    assert!(choices(&r).iter().all(|&m| m == 1));
    let r = setup(
        target(SPORE, 20),
        &[LEECH_SEED, TACKLE],
        AI_SCRIPT_CHECK_BAD_MOVE,
    );
    assert!(choices(&r).iter().all(|&m| m == 1));
    let mut r = setup(
        target(WATER, 20),
        &[GROWL, TACKLE],
        AI_SCRIPT_CHECK_BAD_MOVE,
    );
    r.b.battler_mut(Side::Player).stages[Stat::Atk as usize] = -6;
    assert!(choices(&r).iter().all(|&m| m == 1));
    // AI_CBM_Confuse on a confused target is -5 (not -10): 95 vs 100.
    let mut r = setup(
        target(WATER, 20),
        &[SUPERSONIC, TACKLE],
        AI_SCRIPT_CHECK_BAD_MOVE,
    );
    r.b.battler_mut(Side::Player).confusion = 2;
    let mut rng = Rng::new(1);
    let mut ai = Ai::setup(&r.b, &TABLES, &mut rng);
    ai.check_bad_move(0, SUPERSONIC);
    assert_eq!(ai.score[0], 95);
}

#[test]
fn try_to_faint_prefers_a_quick_ko() {
    // AI_TryToFaint: both moves KO a 1-HP target; Quick Attack scores +6, Tackle +4.
    let r = setup(
        target(WATER, 1),
        &[TACKLE, QUICK_ATTACK],
        AI_SCRIPT_TRY_TO_FAINT,
    );
    assert!(choices(&r).iter().all(|&m| m == 1));
    // No KO: the weaker move is "not the most powerful" (-1). Def 20: Mega Hit
    // 10×250×4/20/50 + 2 = 12 ×1.5 = 18 (15..=18 after simulatedRNG), Tackle 3 ×1.5 = 4.
    let r = setup(
        mon(WATER, 50, [200, 13, 20, 9, 9, 20], &[TACKLE]),
        &[TACKLE, MEGA_HIT],
        AI_SCRIPT_TRY_TO_FAINT,
    );
    assert!(choices(&r).iter().all(|&m| m == 1));
}

#[test]
fn ai_damage_matches_the_gen3_formula() {
    // AI_CalcDmg + TypeCalc: Tackle by Atk 10 L5 on Def 10: 10×35×4/10/50 + 2 = 4, STAB
    // (Normal user) ×1.5 = 6, × simulatedRNG / 100 (85..=100) → 5 or 6.
    let r = setup(target(WATER, 20), &[TACKLE], 0);
    let mut rng = Rng::new(1);
    let ai = Ai::setup(&r.b, &TABLES, &mut rng);
    let d = ai.damage(0, TACKLE);
    assert_eq!(d, 6 * ai.sim[0] / 100);
    // Effectiveness: neutral STAB stays 60; a Ghost target is x0.
    assert_eq!(ai.effectiveness(TACKLE), 60);
    let r = setup(target(GHOST, 20), &[TACKLE], 0);
    let mut rng = Rng::new(1);
    let ai = Ai::setup(&r.b, &TABLES, &mut rng);
    assert_eq!(ai.effectiveness(TACKLE), 0);
}

#[test]
fn check_viability_bide_and_poison() {
    // AI_CV_Bide: -2 at ≤ 90% HP (no roll). AI_CV_Poison: -1 when the target is at ≤ 50%.
    let mut r = setup(
        target(WATER, 20),
        &[BIDE, POISON_POWDER],
        AI_SCRIPT_CHECK_VIABILITY,
    );
    r.b.battler_mut(Side::Opponent).mon.hp = 18;
    let mut rng = Rng::new(1);
    let mut ai = Ai::setup(&r.b, &TABLES, &mut rng);
    ai.check_viability(0, BIDE);
    assert_eq!(ai.score[0], 98);
    ai.check_viability(1, POISON_POWDER);
    assert_eq!(ai.score[1], 100);
    let r = setup(
        target(WATER, 10),
        &[POISON_POWDER],
        AI_SCRIPT_CHECK_VIABILITY,
    );
    let mut ai = Ai::setup(&r.b, &TABLES, &mut rng);
    ai.check_viability(0, POISON_POWDER);
    assert_eq!(ai.score[0], 99);
}

#[test]
fn first_battle_flees_at_a_fifth_of_the_target_hp() {
    // AI_FirstBattle: `if_hp_equal 20` / `if_hp_less_than 20` (100 × hp / maxHP) → flee.
    let first = BattleKind::Wild { first_battle: true };
    let foe = mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[TACKLE, GROWL]);
    // A slower player (Speed 7 < 8): the foe's B_ACTION_RUN goes through GetWhoStrikesFirst
    // like a priority-0 move and wins, so the player's move never runs. The name is
    // `B_BUFF_MON_NICK` (HandleEndTurn_MonFled: PREPARE_MON_NICK_BUFFER, no tag).
    let slow = |hp| {
        let mut m = mon(WATER, 5, [20, 13, 10, 7, 9, 10], &[TACKLE]);
        m.hp = hp;
        m
    };
    let mut r = Run::new(first, &[slow(4)], &[foe], 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l, ["WildPkmnFled@O nick:O0 !", "End(MonFled)"]);
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::MonFled));
    // 5 / 20 = 25%: no flee; the faster foe attacks first.
    let mut r = Run::new(first, &[slow(5)], &[foe], 1);
    let l = r.choose(Choice::Move(0));
    assert!(l[0].starts_with("UsedMove@O"));
    assert!(l.contains(&"UsedMove@P move:1".to_string()));
    assert!(!l.iter().any(|e| e.starts_with("WildPkmnFled")));
}

#[test]
fn a_faster_player_moves_before_the_foe_flees() {
    // SetActionsAndBattlersTurnOrder puts B_ACTION_RUN first for the player's battlers only:
    // the player (Speed 9 > 8) attacks, then the foe flees.
    let first = BattleKind::Wild { first_battle: true };
    let foe = mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[TACKLE, GROWL]);
    let mut r = Run::new(first, &[target(WATER, 4)], &[foe], 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(
        l,
        [
            "UsedMove@P move:1",
            "Anim P move:1",
            "Hit O Normal",
            "Hp O 13->6",
            "WildPkmnFled@O nick:O0 !",
            "End(MonFled)"
        ]
    );
}

#[test]
fn first_battle_flee_threshold_is_twenty_percent() {
    // 100 × 21 / 100 = 21: kept; 100 × 20 / 100 = 20: `if_hp_equal 20` flees.
    let first = BattleKind::Wild { first_battle: true };
    let foe = mon(NORMAL, 2, [13, 6, 6, 8, 5, 5], &[TACKLE, GROWL]);
    let player = |hp| {
        let mut m = mon(WATER, 5, [100, 13, 10, 9, 9, 10], &[TACKLE]);
        m.hp = hp;
        m
    };
    let ai = |hp| {
        let r = Run::new(first, &[player(hp)], &[foe], 1);
        choose(&r.b, &TABLES, &mut Rng::new(5))
    };
    assert_eq!(ai(20), Choice::Run);
    assert!(matches!(ai(21), Choice::Move(_)));
}

/// `n` draws from `Rng::new(seed)`.
fn after(seed: u32, n: usize) -> Rng {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        r.random();
    }
    r
}

#[test]
fn score_wraps_as_s8_then_floors_at_zero() {
    // Cmd_score: `score[i] += d` on an s8, then `if (score[i] < 0) score[i] = 0`.
    let r = setup(target(WATER, 20), &[TACKLE], 0);
    let mut rng = Rng::new(1);
    let mut ai = Ai::setup(&r.b, &TABLES, &mut rng);
    ai.score[0] = 125;
    ai.add(0, 2);
    assert_eq!(ai.score[0], 127);
    ai.add(0, 1);
    assert_eq!(ai.score[0], 0, "127 + 1 wraps to -128");
    ai.score[0] = 100;
    ai.add(0, -110);
    assert_eq!(ai.score[0], 0);
}

#[test]
fn setup_first_turn_rolls_for_setup_moves_on_turn_zero() {
    // AI_SetupFirstTurn: Growl (EFFECT_ATTACK_DOWN) draws `if_random_less_than 80` on turn 0
    // only; Tackle draws nothing. 4 simulatedRNG + Growl's roll + the pick.
    let mut r = setup(
        target(WATER, 20),
        &[TACKLE, GROWL],
        AI_SCRIPT_SETUP_FIRST_TURN,
    );
    let mut rng = Rng::new(9);
    choose(&r.b, &TABLES, &mut rng);
    assert_eq!(rng, after(9, 6));
    r.b.turn = 1;
    let mut rng = Rng::new(9);
    choose(&r.b, &TABLES, &mut rng);
    assert_eq!(rng, after(9, 5));
    // +2 unless the roll is below 80.
    for seed in 1..40 {
        let mut r = setup(target(WATER, 20), &[GROWL], AI_SCRIPT_SETUP_FIRST_TURN);
        r.b.turn = 0;
        let mut rng = Rng::new(seed);
        let mut ai = Ai::setup(&r.b, &TABLES, &mut rng);
        let roll = {
            let mut x = *ai.rng;
            x.random() % 256
        };
        ai.setup_first_turn(0, GROWL);
        assert_eq!(
            ai.score[0],
            if roll < 80 { 100 } else { 102 },
            "seed {seed}"
        );
    }
}

/// A trainer battle: the player's Mudkip-like mon against `lead` then a Ghost with Absorb
/// (Grass, super effective on Water).
fn two_mon_trainer(lead: &[MoveId], seed: u32) -> Run {
    let foe = mon(NORMAL, 5, [20, 10, 10, 5, 10, 10], lead);
    let ghost = mon(GHOST, 5, [20, 10, 10, 5, 10, 10], &[ABSORB]);
    let mut r = Run::new(trainer(), &[target(WATER, 20)], &[foe, ghost], seed);
    r.b.trainer_ai = 0;
    r
}

#[test]
fn should_switch_draws_and_decision() {
    // Nothing landed on the AI's battler yet: no switch, no draw.
    let r = two_mon_trainer(&[TACKLE], 1);
    let mut rng = Rng::new(3);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, Rng::new(3));
    // Tackle landed: FindMonWithFlagsAndSuperEffective(DOESNT_AFFECT_FOE, 2) finds the Ghost
    // (Normal does nothing to it) with Absorb super effective on the player: one
    // `Random() % 2` decides. On a miss, the (NOT_VERY_EFFECTIVE, 3) pass finds nothing.
    for go in [true, false] {
        let seed = (1..)
            .find(|&s| Rng::new(s).random().is_multiple_of(2) == go)
            .unwrap();
        let mut r = two_mon_trainer(&[TACKLE], 1);
        r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
        let mut rng = Rng::new(seed);
        let got = switch(&r.b, &TABLES, &mut rng);
        assert_eq!(got, go.then_some(1));
        assert_eq!(rng, after(seed, 1));
    }
    // A super effective move of its own: FindMonThatAbsorbsOpponentsMove's `Random() % 3`,
    // then HasSuperEffectiveMoveAgainstOpponents' `Random() % 10 != 0` keeps it in.
    let seed = (1..)
        .find(|&s| !after(s, 1).random().is_multiple_of(10))
        .unwrap();
    let mut r = two_mon_trainer(&[ABSORB], 1);
    r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
    let mut rng = Rng::new(seed);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, after(seed, 2));
    // AreStatsRaised (more than 3 stages up) keeps it in, without a draw.
    let mut r = two_mon_trainer(&[TACKLE], 1);
    r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
    r.b.battler_mut(Side::Opponent).stages[Stat::Atk as usize] = 4;
    let mut rng = Rng::new(3);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, Rng::new(3));
    // Wild battles and one-mon trainers never ask.
    let mut r = setup(target(WATER, 20), &[TACKLE], 0);
    r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, Rng::new(3));
}

#[test]
fn the_trainer_switches_before_choosing_a_move() {
    // Turn 1: the player's Tackle lands on the Normal lead. Turn 2 starts with
    // AI_TrySwitchOrUseItem: on some seeds the Ghost comes in (B_ACTION_SWITCH goes first).
    let mut switched = 0;
    for seed in 1..30 {
        let mut r = two_mon_trainer(&[TACKLE], seed);
        r.choose(Choice::Move(0));
        if r.b.battler(Side::Opponent).last_landed != Some(TACKLE) {
            continue;
        }
        let l = r.choose(Choice::Move(0));
        if l.first().map(String::as_str) == Some("Recall O") {
            assert!(l.contains(&"SendOut O1".to_string()), "{l:?}");
            assert_eq!(r.b.mon(Side::Opponent).party, 1);
            switched += 1;
        }
    }
    assert!(switched > 0);
}

// ---- Task G ----

fn male(mut p: Pokemon) -> Pokemon {
    p.personality = 0xFF;
    p
}

#[test]
fn check_bad_move_task_g_branches() {
    let f = AI_SCRIPT_CHECK_BAD_MOVE;
    let only_slot_1 = |r: &Run| choices(r).iter().all(|&m| m == 1);
    // AI_CheckBadMove_CheckSoundproof: the named sound moves on a Soundproof target.
    assert!(only_slot_1(&setup(
        target(SOUNDPROOF, 20),
        &[SOUND_GROWL, GROWL],
        f
    )));
    // CheckIfAbilityBlocksStatChange: Clear Body.
    assert!(only_slot_1(&setup(target(CLEAR, 20), &[GROWL, TACKLE], f)));
    // AI_CBM_Sleep: Insomnia.
    assert!(only_slot_1(&setup(
        target(INSOMNIAC, 20),
        &[SLEEP_POWDER, GROWL],
        f
    )));
    // AI_CBM_CantEscape, AI_CBM_Foresight.
    let mut r = setup(target(WATER, 20), &[BLOCK, GROWL], f);
    assert!(choices(&r).contains(&0));
    r.b.battler_mut(Side::Player).v.escape_prevented = true;
    assert!(only_slot_1(&r));
    let mut r = setup(target(WATER, 20), &[FORESIGHT, GROWL], f);
    r.b.battler_mut(Side::Player).foresight = true;
    assert!(only_slot_1(&r));
    // AI_CBM_Attract: same gender (both female), Oblivious; a male target is fine.
    assert!(only_slot_1(&setup(target(WATER, 20), &[ATTRACT, GROWL], f)));
    assert!(only_slot_1(&setup(
        male(target(OBLIVIOUS, 20)),
        &[ATTRACT, GROWL],
        f
    )));
    assert!(choices(&setup(male(target(WATER, 20)), &[ATTRACT, GROWL], f)).contains(&0));
    // Teleport: always -10. AI_CBM_MudSport: the user's own Mud Sport.
    assert!(only_slot_1(&setup(
        target(WATER, 20),
        &[TELEPORT, GROWL],
        f
    )));
    let mut r = setup(target(WATER, 20), &[MUD_SPORT, GROWL], f);
    assert!(choices(&r).contains(&0));
    r.b.battler_mut(Side::Opponent).v.mud_sport = true;
    assert!(only_slot_1(&r));
}

#[test]
fn check_viability_task_g_branches() {
    let f = AI_SCRIPT_CHECK_VIABILITY;
    // AI_CV_MudSport: -1 unless the target is Electric (+1); no roll.
    let r = setup(target(WATER, 20), &[MUD_SPORT, TACKLE], f);
    assert!(choices(&r).iter().all(|&m| m == 1));
    let r = setup(target(ELECTRIC, 20), &[MUD_SPORT, TACKLE], f);
    assert!(choices(&r).iter().all(|&m| m == 0));
    // AI_CV_Foresight (non-BUGFIX: the user's types and evasion): -2 without a roll.
    let r = setup(target(GHOST, 20), &[FORESIGHT, TACKLE], f);
    assert!(choices(&r).iter().all(|&m| m == 1));
    // Draw counts: 4 simulatedRNG + the script's rolls + the pick.
    let draws = |r: &Run| {
        let mut rng = Rng::new(7);
        choose(&r.b, &TABLES, &mut rng);
        (0..10).find(|&n| after(7, n) == rng).unwrap()
    };
    // AI_CV_Trap: one roll when the target is badly poisoned or infatuated.
    let mut r = setup(target(WATER, 20), &[BLOCK], f);
    assert_eq!(draws(&r), 5);
    r.b.battler_mut(Side::Player).v.infatuated = true;
    assert_eq!(draws(&r), 6);
    // AI_CV_Foresight3: user evasion above 8 rolls once.
    let mut r = setup(target(WATER, 20), &[FORESIGHT], f);
    assert_eq!(draws(&r), 5);
    r.b.battler_mut(Side::Opponent).stages[Stat::Evasion as usize] = 3;
    assert_eq!(draws(&r), 6);
    // AI_CV_SpeedDownFromChance: the 100% Speed-drop hits (Icy Wind, Rock Tomb, Mud Shot)
    // run AI_CV_SpeedDown (the faster target: one roll).
    assert_eq!(draws(&setup(target(WATER, 20), &[ICY_WIND], f)), 6);
}

#[test]
fn a_trapped_trainer_mon_never_switches() {
    // ShouldSwitch: STATUS2_ESCAPE_PREVENTION, or Magnet Pull on the field and a Steel
    // battler: FALSE before any draw.
    let mut r = two_mon_trainer(&[TACKLE], 1);
    r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
    r.b.battler_mut(Side::Opponent).v.escape_prevented = true;
    let mut rng = Rng::new(3);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, Rng::new(3));
    let mut r = two_mon_trainer(&[TACKLE], 1);
    r.b.battler_mut(Side::Opponent).last_landed = Some(TACKLE);
    r.b.battler_mut(Side::Opponent).types = [Type::Steel, Type::Steel];
    r.b.battler_mut(Side::Player).ability = Ability::MagnetPull;
    let mut rng = Rng::new(3);
    assert_eq!(switch(&r.b, &TABLES, &mut rng), None);
    assert_eq!(rng, Rng::new(3));
}
