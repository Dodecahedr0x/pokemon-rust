//! AI tests: score-model choices in hand-checked situations, and the first battle's flee.

use super::*;
use crate::battle::core::tests::{Run, trainer};
use crate::battle::test_data::*;
use crate::battle::{BattleKind, BattleOutcome, Need};
use crate::data::MoveId;
use crate::mon::Pokemon;
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
    let mut r = Run::new(first, &[target(WATER, 4)], &[foe], 1);
    let l = r.choose(Choice::Move(0));
    // A fleeing battler acts first (B_ACTION_RUN), so the player's move never runs. The
    // name is `B_BUFF_MON_NICK` (HandleEndTurn_MonFled: PREPARE_MON_NICK_BUFFER, no tag).
    assert_eq!(l, ["WildPkmnFled@O nick:O0 !", "End(MonFled)"]);
    assert_eq!(r.b.need(), Need::Ended(BattleOutcome::MonFled));
    // 5 / 20 = 25%: no flee.
    let mut r = Run::new(first, &[target(WATER, 5)], &[foe], 1);
    let l = r.choose(Choice::Move(0));
    assert_eq!(l[0], "UsedMove@P move:1");
}
