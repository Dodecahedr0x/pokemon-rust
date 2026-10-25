//! One Battle Tower singles challenge, fought and recorded (`frontier_util.c`, `battle_tower.c`).

use super::battles::mash;
use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{self, FIRE, NORMAL, WATER, WATER_GUN};
use crate::frontier::{Args, Call, FACILITY_TOWER, MODE_SINGLES, STATUS_NONE};
use crate::mon::Pokemon;

// Saved vars (not the temp range, which a map change clears). 0x7000 is not stored.
const V_BAD: VarId = VarId(0x40F0);
const V_RIBBON: VarId = VarId(0x40F1);
const V_POINTS: VarId = VarId(0x40F2);
const V_STATUS: VarId = VarId(0x40F3);
const V_CHAL: VarId = VarId(0x40F4);
const V_STREAK: VarId = VarId(0x40F5);
const V_LOST: VarId = VarId(0x40F6);
const V_LINK: VarId = VarId(0x40F7);

fn args(func: u16, arg: u16, value: u16) -> Args {
    Args {
        func,
        arg,
        value,
        facility: FACILITY_TOWER,
        mode: MODE_SINGLES,
        result: 0,
    }
}

fn level_capped(species: SpeciesId) -> Pokemon {
    test_data::mon(species, 40, [80; 6], &[WATER_GUN])
}

fn ace(species: SpeciesId) -> Pokemon {
    test_data::mon(species, 100, [500; 6], &[WATER_GUN])
}

#[test]
fn one_singles_challenge_is_fought_and_recorded() {
    let mut w = started();
    for species in [WATER, NORMAL, FIRE] {
        w.s.party.push(level_capped(species)).unwrap();
    }
    w.s.stash.order = [1, 2, 3, 0];

    start(&mut w, |ctx| {
        Box::pin(async move {
            // Level mode 50 (`VAR_RESULT` 0): three distinct mons at level 40 are eligible.
            let bad = ctx.frontier(Call::Util(args(15, 0, 0))).await;
            ctx.set_var(V_BAD, bad).await;
            ctx.frontier(Call::Util(args(2, 1, 0))).await;
            ctx.frontier(Call::Tower(args(0, 0, 0))).await;
            ctx.frontier(Call::Util(args(2, 4, 0))).await;
            ctx.frontier(Call::Util(args(3, 3, 0))).await;
        })
    });
    until_done(&mut w, 30);
    assert_eq!(w.var(V_BAD), 0, "the party can enter a level-50 challenge");
    assert_eq!(w.s.party.len(), 3);

    for (i, species) in [WATER, NORMAL, FIRE].into_iter().enumerate() {
        *w.s.party.get_mut(i).unwrap() = ace(species);
    }

    assert!(w.s.party.first_usable().is_some(), "the aces can battle");
    let mut probe = crate::rng::Rng::new(1);
    let foes = w.s.frontier.opponent_party(&test_data::TABLES, &mut probe);
    assert_eq!(foes.len(), 3, "a tower opponent brings three Pokémon");
    assert!(foes.iter().all(|m| m.hp > 0), "the opponents are conscious");

    start(&mut w, |ctx| {
        Box::pin(async move {
            for _ in 0..7 {
                ctx.frontier(Call::Tower(args(3, 0, 0))).await;
                let outcome = ctx.frontier(Call::Battle).await;
                if outcome != BattleOutcome::Won as u16 {
                    ctx.set_var(V_LOST, outcome).await;
                }
                ctx.frontier(Call::Util(args(18, 0, 0))).await;
                ctx.frontier(Call::Tower(args(4, 0, 0))).await;
                ctx.heal_party().await;
            }
            ctx.frontier(Call::Util(args(2, 0, 3))).await;
            let ribbons = ctx.frontier(Call::Tower(args(5, 0, 0))).await;
            ctx.set_var(V_RIBBON, ribbons).await;
            let points = ctx.frontier(Call::Util(args(11, 0, 0))).await;
            ctx.set_var(V_POINTS, points).await;
            ctx.frontier(Call::Util(args(8, 0, 0))).await;
            ctx.frontier(Call::Tower(args(6, 0, 0))).await;
            let status = ctx.frontier(Call::Util(args(0, 0, 0))).await;
            ctx.set_var(V_STATUS, status).await;
            let chal = ctx.frontier(Call::Util(args(1, 0, 0))).await;
            ctx.set_var(V_CHAL, chal).await;
            let streak = ctx.frontier(Call::Tower(args(1, 1, 0))).await;
            ctx.set_var(V_STREAK, streak).await;
            let link = ctx.frontier(Call::Tower(args(12, 0, 0))).await;
            ctx.set_var(V_LINK, link).await;
        })
    });
    let fought = core::cell::Cell::new(false);
    let frames = mash(&mut w, 80_000, |w| {
        if w.in_battle() {
            fought.set(true);
        }
        !w.script_running()
    });
    assert!(fought.get(), "the seven sets were fought ({frames} frames)");

    assert_eq!(
        (
            w.var(V_LOST),
            w.var(V_RIBBON),
            w.var(V_POINTS),
            w.var(V_STATUS),
            w.var(V_CHAL),
            w.var(V_STREAK),
            w.var(V_LINK),
            frames > 100,
        ),
        (0, 0, 1, STATUS_NONE, 0, 7, 6, true),
        "{frames} {:?}",
        w.s.frontier
    );
    assert_eq!(w.s.frontier.battle_points(), 1);
    assert_eq!(w.s.frontier.saved_streak(), 7);
    assert_eq!(
        ctx_singles(&w),
        7,
        "GET_STATUS cleared the challenge; the singles stat did not"
    );
}

fn ctx_singles(w: &World) -> u16 {
    let mut record = w.s.frontier.clone();
    let mut rng = crate::rng::Rng::new(1);
    match record.apply(Call::SinglesStreak, &w.s.party, &[0; 4], &mut rng) {
        crate::frontier::Step::Value(v) => v,
        other => panic!("singles streak: {other:?}"),
    }
}
