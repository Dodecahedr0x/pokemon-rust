//! One contest from a script: choose a mon, link-transfer, judge, results, the contest name,
//! and the artist painting.

use super::{Game, run, started};
use crate::Buttons;
use crate::battle::test_data::{NORMAL, TACKLE};
use crate::contest::{self, name};
use crate::data::{GameData, VarId};
use crate::mon::{Ivs, Pokemon};
use crate::rng::Rng;
use crate::script::BufferSrc;
use crate::ui::ScreenKindTag;
use crate::ui::kit::Phase;
use crate::world::Event;
use alloc::boxed::Box;

#[test]
fn a_script_enters_judges_and_finishes_a_normal_contest() {
    let mut w = started();
    for _ in 0..32 {
        if !w.script_running() {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    assert!(!w.script_running());
    let t = Game.battle();
    let mut rng = Rng::new(1);
    let mut mon = Pokemon::new(t, NORMAL, 5, Ivs::Fixed(0), 1, 1, &mut rng);
    mon.moves = [Some(TACKLE), None, None, None];
    w.s.party.push(mon).unwrap();

    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.set_var(VarId(contest::VAR_RANK), 0).await;
        ctx.set_var(VarId(contest::VAR_CATEGORY), 0).await;
        let slot = ctx.choose_contest_mon().await;
        ctx.set_var(VarId(0x8005), slot).await;
        ctx.contest_link_transfer().await;
        ctx.start_contest().await;
        ctx.show_contest_results().await;
        ctx.buffer(0, BufferSrc::ContestName(0)).await;
        ctx.show_contest_painting(0).await;
    }));

    let mut opened_party = 0;
    let mut opened_contest = 0;
    for _ in 0..800 {
        if !w.script_running() && w.screens_open() == 0 {
            break;
        }
        let press = w.screens().is_some_and(|s| s.host.phase == Phase::Running);
        let keys = if press { Buttons::A } else { Buttons::NONE };
        w.step(&Game, keys).validate().unwrap();
        for e in w.events() {
            match e {
                Event::ScreenOpened(ScreenKindTag::Party) => opened_party += 1,
                Event::ScreenOpened(ScreenKindTag::Contest) => opened_contest += 1,
                _ => {}
            }
        }
    }
    assert!(!w.script_running(), "the contest script did not finish");
    assert_eq!(w.screens_open(), 0);
    assert_eq!(opened_party, 1, "choosecontestmon opens the party menu");
    assert_eq!(
        opened_contest, 3,
        "appeals, results, and the painting each open"
    );
    assert_eq!(w.var(VarId(0x8005)), 0, "the chosen party slot");
    assert_eq!(
        w.var(VarId(contest::VAR_CHOSEN)),
        0,
        "solo categories match"
    );
    let c = &w.s.contest;
    assert!(c.judged);
    assert_eq!(c.place[contest::PLAYER_MON as usize], 0);
    assert_eq!(c.appeal[contest::PLAYER_MON as usize], 70);
    assert_eq!(c.total[contest::PLAYER_MON as usize], 140);
    assert_eq!(c.total[0], -60);
    let artist = c.artist.expect("results record the winner");
    assert!(artist.player);
    assert_eq!(artist.species, NORMAL.0);
    assert_eq!(artist.category, 0);
    let got = w.s.str_vars.get(0);
    assert!(got.starts_with(name(0)));
    assert_eq!(got[name(0).len()], 0xFF);
}

#[test]
fn link_transfer_without_a_mon_says_the_categories_differ() {
    let mut w = started();
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.contest_link_transfer().await;
    }));
    for _ in 0..8 {
        if !w.script_running() {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    assert!(!w.script_running());
    assert_eq!(w.var(VarId(contest::VAR_CHOSEN)), 1);
}
