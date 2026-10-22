//! Milestone 4: the party requests, scripted and field battles, the starter screen and the
//! whiteout, on the synthetic game (Task D's battle tables, empty battle graphics and text).

use super::*;
use crate::battle::scene::OpenMenu;
use crate::battle::test_data::{NORMAL, WATER};
use crate::battle::{BattleOutcome, Environment};

/// Alternates A and nothing (a new A press every other frame: it picks FIGHT and move slot 0
/// and dismisses messages) until `done`; returns the frames taken. Panics after `max`.
fn mash(w: &mut World, max: u32, done: impl Fn(&World) -> bool) -> u32 {
    for n in 0..max {
        if done(w) {
            return n;
        }
        run(
            w,
            if n % 2 == 0 {
                Buttons::A
            } else {
                Buttons::NONE
            },
            1,
        );
    }
    panic!(
        "not done after {max} frames (stage {:?})",
        w.battle().map(|b| b.0)
    );
}

/// A script giving `species` at `level`, then battling `foe` (level `foe_level`); FLAG_DONE is
/// set to whether the outcome is `want`.
fn give_and_battle(
    w: &mut World,
    species: SpeciesId,
    level: u8,
    foe: SpeciesId,
    foe_level: u8,
    want: BattleOutcome,
) {
    start(w, move |ctx| {
        Box::pin(async move {
            ctx.give_mon(species, level).await;
            let o = ctx.battle_wild(foe, foe_level, false).await;
            ctx.set_flag(FLAG_DONE, o == want).await;
        })
    });
}

#[test]
fn party_requests_give_count_heal_and_set_respawn() {
    let mut w = started();
    assert_eq!(w.respawn(), RESPAWN, "the game's default heal location");
    start(&mut w, |ctx| {
        Box::pin(async move {
            let a = ctx.give_mon(WATER, 5).await;
            let b = ctx.give_mon(NORMAL, 3).await;
            let n = ctx.party_size().await;
            ctx.heal_party().await;
            ctx.set_respawn(7).await;
            ctx.set_flag(FLAG_DONE, (a, b, n) == (0, 0, 2)).await;
        })
    });
    assert_eq!(until_done(&mut w, 5), 1, "all answered in one frame");
    assert!(w.flag(FLAG_DONE));
    assert_eq!(w.respawn(), 7);
    let p = w.party().get(0).unwrap();
    assert_eq!((p.species, p.level, p.ot_id), (WATER, 5, w.player_id()));
    assert_eq!(p.hp, p.stats[0]);
}

#[test]
fn a_full_party_cannot_be_given_more() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            for _ in 0..6 {
                ctx.give_mon(NORMAL, 2).await;
            }
            let r = ctx.give_mon(NORMAL, 2).await;
            ctx.set_flag(FLAG_DONE, r == 2).await;
        })
    });
    until_done(&mut w, 5);
    assert!(w.flag(FLAG_DONE));
    assert_eq!(w.party().len(), 6);
}

#[test]
fn a_scripted_wild_battle_runs_and_answers_after_the_field_is_back() {
    let mut w = started();
    give_and_battle(&mut w, WATER, 30, NORMAL, 2, BattleOutcome::Won);
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    assert_eq!(w.battle().unwrap().0, Stage::Transition);
    // The transition ends black, the scene loads on black frames.
    let mut saw = (false, false);
    let mut frames = 0;
    while w.battle().is_some_and(|b| b.0 != Stage::Running) {
        let f = run(&mut w, Buttons::NONE, 1);
        if w.battle().is_some_and(|b| b.0 == Stage::Loading) {
            saw.0 = true;
            assert!(f.is_black());
        }
        frames += 1;
        assert!(frames < 400);
    }
    saw.1 = true;
    assert_eq!(saw, (true, true));
    let exp_before = w.party().get(0).unwrap().exp;
    mash(&mut w, 5000, |w| {
        w.battle().is_some_and(|b| {
            b.0 == Stage::Running && matches!(b.2.open_menu(), Some(OpenMenu::Action(_)))
        })
    });
    // Back on the field only once the fade-in is over; the script resumes after that.
    mash(&mut w, 5000, |w| !w.in_battle());
    assert!(w.script_running(), "not answered during the fade-in");
    run(&mut w, Buttons::NONE, 2);
    assert!(!w.script_running());
    assert!(w.flag(FLAG_DONE), "answered Won");
    assert!(
        w.party().get(0).unwrap().exp > exp_before,
        "exp written back"
    );
    assert!(!w.text().is_open());
    assert!(
        w.objects().iter().all(|o| o.frozen),
        "a script's battle leaves unfreezing to the script"
    );
}

#[test]
fn losing_whites_out_to_the_respawn_and_drops_the_script() {
    let mut w = started();
    give_and_battle(&mut w, NORMAL, 2, WATER, 60, BattleOutcome::Lost);
    run(&mut w, Buttons::NONE, 1);
    mash(&mut w, 20_000, |w| {
        w.battle()
            .is_some_and(|b| matches!(b.0, Stage::Whiteout(_)))
    });
    // Gen3 `CB2_WhiteOut` waits on black.
    for _ in 0..WHITEOUT_FRAMES - 1 {
        assert!(run(&mut w, Buttons::NONE, 1).is_black());
    }
    mash(&mut w, 200, |w| !w.in_battle());
    assert!(!w.script_running(), "the script was aborted");
    assert!(!w.flag(FLAG_DONE));
    assert_eq!((w.map(), pos(&w)), (HOUSE, (3, 3)));
    assert_eq!(w.objects().player().facing, Dir::Down);
    let p = w.party().get(0).unwrap();
    assert_eq!(p.hp, p.stats[0], "healed");
    assert!(w.objects().iter().all(|o| !o.frozen));
}

#[test]
fn trainer_battles_answer_won_and_unknown_trainers_are_skipped() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let skipped = ctx.battle_trainer(TrainerId(9), TextId(0)).await;
            ctx.give_mon(WATER, 30).await;
            let o = ctx.battle_trainer(TRAINER, TextId(0)).await;
            let ok = skipped == BattleOutcome::Won && o == BattleOutcome::Won;
            ctx.set_flag(FLAG_DONE, ok).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    mash(&mut w, 20_000, |w| !w.script_running());
    assert!(w.flag(FLAG_DONE));
}

#[test]
fn battles_without_a_mon_or_in_map_scripts_answer_at_once() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let o = ctx.battle_wild(NORMAL, 2, false).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    until_done(&mut w, 3);
    assert!(w.flag(FLAG_DONE));
    assert!(!w.in_battle());
}

/// Walks the route's grass up and down until a wild battle starts.
fn walk_grass_until_battle(w: &mut World) {
    for dir in [Dir::Up, Dir::Down].into_iter().cycle().take(40) {
        if w.in_battle() {
            return;
        }
        walk(w, dir, 1);
    }
    panic!("no encounter");
}

#[test]
fn grass_steps_start_wild_battles_after_the_immunity_steps() {
    let mut w = started();
    // No party: grass never starts a battle.
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 16);
    assert_eq!(w.map(), ROUTE);
    walk(&mut w, Dir::Up, 6);
    assert_eq!(pos(&w), (5, 3));
    walk(&mut w, Dir::Up, 1);
    walk(&mut w, Dir::Down, 1);
    assert!(!w.in_battle());
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 30).await;
        })
    });
    until_done(&mut w, 3);
    // Rate 255 caps at 2880: every check after the 60% new-cell gate passes, and both cells
    // are grass, so the first checked step battles.
    walk_grass_until_battle(&mut w);
    assert_eq!(
        w.battle().unwrap().1.setup().environment,
        Environment::Grass
    );
    mash(&mut w, 20_000, |w| !w.in_battle());
    assert!(
        w.objects().iter().all(|o| !o.frozen),
        "field battles unfreeze"
    );
    // The battle restarted the immunity: 4 free steps.
    assert_eq!(w.s.encounter_immunity, 0);
    for (i, dir) in [Dir::Up, Dir::Down, Dir::Up, Dir::Down]
        .into_iter()
        .enumerate()
    {
        walk(&mut w, dir, 1);
        assert!(!w.in_battle());
        assert_eq!(w.s.encounter_immunity, i as u8 + 1);
    }
    assert_eq!(w.s.encounter_immunity, ENCOUNTER_IMMUNITY_STEPS);
}

#[test]
fn the_immunity_counts_steps_since_the_last_reset() {
    let mut w = started();
    assert_eq!(w.s.encounter_immunity, 0, "reset on the map load");
    walk(&mut w, Dir::Up, 3);
    assert_eq!(w.s.encounter_immunity, 3);
    walk(&mut w, Dir::Up, 2);
    assert_eq!(w.s.encounter_immunity, 4);
    run(&mut w, Buttons::UP, 1);
    assert_eq!(w.s.encounter_immunity, 0, "reset on a crossing");
}

#[test]
fn the_starter_screen_answers_and_the_first_battle_runs_over_it() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let i = ctx.choose_starter([WATER, NORMAL, WATER]).await;
            ctx.give_mon([WATER, NORMAL, WATER][i as usize], 30).await;
            let o = ctx.battle_wild(NORMAL, 2, true).await;
            ctx.set_var(VAR_HOUSE, i as u16).await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.starter().is_some());
    // Loading, then the choice: left, A, then yes.
    run(&mut w, Buttons::NONE, 30);
    tap(&mut w, Buttons::LEFT, 4);
    assert_eq!(w.starter().unwrap().selection(), 0);
    let mut n = 0;
    while w.starter().is_some() {
        run(
            &mut w,
            if n % 2 == 0 {
                Buttons::A
            } else {
                Buttons::NONE
            },
            1,
        );
        n += 1;
        assert!(n < 2000);
    }
    let (stage, battle, _) = w.battle().expect("the first battle starts at once");
    assert_eq!(stage, Stage::Transition);
    assert_eq!(
        battle.setup().kind,
        crate::battle::BattleKind::Wild { first_battle: true }
    );
    assert_eq!(w.party().len(), 1);
    mash(&mut w, 20_000, |w| !w.script_running());
    assert!(w.flag(FLAG_DONE));
    assert_eq!(w.var(VAR_HOUSE), 0);
}

#[test]
fn a_starter_without_a_battle_fades_back_to_the_field() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let i = ctx.choose_starter([WATER, NORMAL, WATER]).await;
            ctx.set_var(VAR_HOUSE, i as u16 + 1).await;
        })
    });
    run(&mut w, Buttons::NONE, 31);
    mash(&mut w, 2000, |w| w.starter().is_none());
    assert_eq!(w.var(VAR_HOUSE), 2, "the middle ball");
    let mut n = 0;
    while !matches!(w.s.mode, Mode::Field) {
        run(&mut w, Buttons::NONE, 1);
        n += 1;
        assert!(n < 40);
    }
    run(&mut w, Buttons::NONE, 20);
    assert_eq!(w.s.fader.level(), 0);
}

#[test]
fn battle_state_is_hashed() {
    let mut a = started();
    give_and_battle(&mut a, WATER, 30, NORMAL, 2, BattleOutcome::Won);
    run(&mut a, Buttons::NONE, 1);
    let h = a.hash();
    run(&mut a, Buttons::NONE, 1);
    assert_ne!(a.hash(), h, "the transition advanced");
}
