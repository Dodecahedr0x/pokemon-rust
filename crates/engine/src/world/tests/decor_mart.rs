//! A decoration mart buys one decoration into the PC and fills the name buffer
//! (`pokemartdecoration`, `bufferdecorationname`).

use super::m9_b::{tap, top, until, world};
use super::*;
use crate::data::{DecorId, DecorShop};
use crate::decor::DecorPc;
use crate::script::BufferSrc;
use crate::ui::ScreenState;
use crate::ui::mart::{BUY, Prompt};

const DONE: VarId = VarId(0x4000);
const DESK: DecorId = DecorId(1);
static STOCK: [DecorId; 1] = [DESK];

fn mart_on_top(w: &World) -> bool {
    matches!(top(w), Some(ScreenState::Mart(_)))
        && w.screens()
            .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
}

fn prompt(w: &World) -> Prompt {
    match top(w) {
        Some(ScreenState::Mart(m)) => m.prompt(),
        t => panic!("the mart is not on top: {t:?}"),
    }
}

fn through(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..600 {
        if done(w) {
            return;
        }
        let msg = matches!(prompt(w), Prompt::Message);
        tap(w, if msg { Buttons::A } else { Buttons::NONE });
    }
    panic!("never reached: {:?}", prompt(w));
}

fn list(w: &World) -> bool {
    mart_on_top(w) && matches!(prompt(w), Prompt::List { .. })
}

fn open(w: &mut World, shop: DecorShop) {
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.decoration_mart(shop, &STOCK).await;
        ctx.buffer(0, BufferSrc::Decoration(DESK)).await;
        ctx.set_var(DONE, 1).await;
    }));
    until(w, mart_on_top);
    assert_eq!(w.var(DONE), 0);
}

fn confirm_and_yes(w: &mut World) {
    assert_eq!(prompt(w), Prompt::Menu { cursor: BUY });
    tap(w, Buttons::A);
    until(w, list);
    tap(w, Buttons::A);
    through(w, |w| matches!(prompt(w), Prompt::YesNo { cursor: 0 }));
    tap(w, Buttons::A);
}

fn leave(w: &mut World) {
    tap(w, Buttons::B);
    through(w, list);
    tap(w, Buttons::B);
    through(w, |w| prompt(w) == Prompt::Menu { cursor: BUY });
    tap(w, Buttons::B);
    until(w, |w| w.screens_open() == 0 && !w.script_running());
}

#[test]
fn buying_a_decoration_puts_it_in_the_pc_and_buffers_its_name() {
    let mut w = world();
    w.s.money = 3000;
    open(&mut w, DecorShop::Shop);
    confirm_and_yes(&mut w);
    through(&mut w, |w| w.money() != 3000);
    assert_eq!(w.money(), 0);
    assert_eq!(w.s.decor.count(DESK), 1);
    assert_eq!(prompt(&w), Prompt::Message);
    leave(&mut w);
    assert_eq!(w.var(DONE), 1);
    let name = b"SMALL DESK\xFF";
    assert_eq!(&w.str_var(0)[..name.len()], name);
    // No Premier Ball: a decoration purchase is one piece, then the money.
    assert_eq!(w.bag().count(ItemId(12)), 0);
}

#[test]
fn a_full_category_keeps_the_money() {
    let mut w = world();
    w.s.money = 3000;
    for _ in 0..10 {
        assert!(w.s.decor.add(DESK, 0));
    }
    open(&mut w, DecorShop::Shop);
    confirm_and_yes(&mut w);
    through(&mut w, list);
    assert_eq!(w.money(), 3000);
    assert_eq!(w.s.decor.count(DESK), 10);
}

#[test]
fn the_first_empty_slot_takes_the_decoration_until_the_category_is_full() {
    let mut pc = DecorPc::new();
    assert!(!pc.add(DecorId(0), 0));
    assert!(!pc.add(DecorId(1), 8));
    for _ in 0..10 {
        assert!(pc.add(DecorId(1), 0));
    }
    assert!(!pc.add(DecorId(2), 0));
    assert_eq!(pc.count(DecorId(1)), 10);
    assert!(pc.add(DecorId(2), 1));
    assert_eq!(pc.count(DecorId(2)), 1);
}
