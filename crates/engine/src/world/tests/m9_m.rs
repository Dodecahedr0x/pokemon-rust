//! Milestone 9 Task M: the Poké Mart (`shop.c`) on a game without pictures: BUY / SELL /
//! QUIT, the list, "not enough money", the quantity's cap, the confirm, the money taken
//! once "Thank you" prints, the Premier Ball bonus and SELL through the bag.

use super::m9_b::{G, POKE_BALL, POTION, PREMIER_BALL, add, run, tap, top, until, world};
use super::*;
use crate::ui::mart::{BUY, MAX_QUANTITY, Mart, Prompt, QUIT, SELL, max_quantity};
use crate::ui::{BagMode, ScreenKind, ScreenState};

static ITEMS: [ItemId; 2] = [POTION, POKE_BALL];

fn mart(w: &World) -> &Mart {
    match top(w) {
        Some(ScreenState::Mart(m)) => m,
        t => panic!("the mart is not on top: {t:?}"),
    }
}

fn prompt(w: &World) -> Prompt {
    mart(w).prompt()
}

fn mart_on_top(w: &World) -> bool {
    matches!(top(w), Some(ScreenState::Mart(_)))
        && w.screens()
            .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
}

fn open(w: &mut World) {
    let mut f = Frame::default();
    crate::world::screens::open(
        &mut w.s,
        &G,
        ScreenKind::Mart { items: &ITEMS },
        false,
        &mut f,
    );
    until(w, mart_on_top);
}

/// Presses A through messages until `done`.
fn through(w: &mut World, done: impl Fn(&World) -> bool) {
    for _ in 0..600 {
        if done(w) {
            return;
        }
        let msg = matches!(top(w), Some(ScreenState::Mart(m)) if m.prompt() == Prompt::Message);
        tap(w, if msg { Buttons::A } else { Buttons::NONE });
    }
    panic!("never reached: {:?}", top(w));
}

fn is(p: Prompt) -> impl Fn(&World) -> bool {
    move |w| mart_on_top(w) && prompt(w) == p
}

fn list(w: &World) -> bool {
    mart_on_top(w) && matches!(prompt(w), Prompt::List { .. })
}

/// BUY, then row `row` of the list.
fn pick(w: &mut World, row: u16) {
    assert_eq!(prompt(w), Prompt::Menu { cursor: BUY });
    tap(w, Buttons::A);
    until(w, list);
    for _ in 0..row {
        tap(w, Buttons::DOWN);
    }
    let Prompt::List { selected, .. } = prompt(w) else {
        panic!("not on the list");
    };
    assert_eq!(selected, row);
    tap(w, Buttons::A);
}

/// Buys `n` of row `row` (the quantity box starts at 1; UP adds one).
fn buy(w: &mut World, row: u16, n: u16) {
    pick(w, row);
    through(w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    for _ in 1..n {
        tap(w, Buttons::UP);
    }
    assert!(matches!(prompt(w), Prompt::Quantity { value, .. } if value == n));
    tap(w, Buttons::A);
    through(w, is(Prompt::YesNo { cursor: 0 }));
    tap(w, Buttons::A);
}

#[test]
fn the_quantity_is_capped_by_money_and_the_bag_stack() {
    assert_eq!(max_quantity(1000, 300), 3);
    assert_eq!(max_quantity(1_000_000, 1), MAX_QUANTITY);
    assert_eq!(max_quantity(100, 0), MAX_QUANTITY);
}

#[test]
fn buying_takes_the_money_once_thank_you_prints_and_adds_the_items() {
    let mut w = world();
    w.s.money = 1000;
    open(&mut w);
    buy(&mut w, 0, 3);
    through(&mut w, |w| w.s.money != 1000);
    assert_eq!(w.s.money, 1000 - 3 * 300);
    assert_eq!(w.s.bag.count(POTION), 3);
    assert_eq!(prompt(&w), Prompt::Message);
    tap(&mut w, Buttons::B);
    through(&mut w, list);
    // CANCEL (the row after the items) goes back to the menu.
    tap(&mut w, Buttons::DOWN);
    tap(&mut w, Buttons::DOWN);
    assert_eq!(
        prompt(&w),
        Prompt::List {
            selected: 2,
            item: None
        }
    );
    tap(&mut w, Buttons::A);
    through(&mut w, is(Prompt::Menu { cursor: BUY }));
    // QUIT closes the mart.
    tap(&mut w, Buttons::DOWN);
    tap(&mut w, Buttons::DOWN);
    assert_eq!(prompt(&w), Prompt::Menu { cursor: QUIT });
    tap(&mut w, Buttons::A);
    until(&mut w, |w| w.screens_open() == 0);
}

#[test]
fn not_enough_money_refuses_and_returns_to_the_list() {
    let mut w = world();
    w.s.money = 299;
    open(&mut w);
    pick(&mut w, 0);
    through(&mut w, list);
    assert_eq!((w.s.money, w.s.bag.count(POTION)), (299, 0));
}

#[test]
fn the_quantity_box_wraps_at_the_affordable_max() {
    let mut w = world();
    w.s.money = 1000;
    open(&mut w);
    pick(&mut w, 0);
    through(&mut w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    assert_eq!(prompt(&w), Prompt::Quantity { value: 1, max: 3 });
    tap(&mut w, Buttons::DOWN);
    assert_eq!(prompt(&w), Prompt::Quantity { value: 3, max: 3 });
    // B backs out to the list without buying.
    tap(&mut w, Buttons::B);
    through(&mut w, list);
    assert_eq!(w.s.money, 1000);
}

#[test]
fn no_to_the_confirm_buys_nothing() {
    let mut w = world();
    w.s.money = 1000;
    open(&mut w);
    pick(&mut w, 0);
    through(&mut w, |w| matches!(prompt(w), Prompt::Quantity { .. }));
    tap(&mut w, Buttons::A);
    through(&mut w, is(Prompt::YesNo { cursor: 0 }));
    tap(&mut w, Buttons::DOWN);
    tap(&mut w, Buttons::A);
    through(&mut w, list);
    assert_eq!((w.s.money, w.s.bag.count(POTION)), (1000, 0));
}

#[test]
fn ten_poke_balls_earn_a_premier_ball_and_nine_do_not() {
    let mut w = world();
    w.s.money = 10_000;
    open(&mut w);
    buy(&mut w, 1, 9);
    through(&mut w, list);
    assert_eq!(
        (w.s.bag.count(POKE_BALL), w.s.bag.count(PREMIER_BALL)),
        (9, 0)
    );
    tap(&mut w, Buttons::B);
    through(&mut w, is(Prompt::Menu { cursor: BUY }));
    buy(&mut w, 1, 10);
    through(&mut w, list);
    assert_eq!(
        (w.s.bag.count(POKE_BALL), w.s.bag.count(PREMIER_BALL)),
        (19, 1)
    );
    assert_eq!(w.s.money, 10_000 - 19 * 200);
}

#[test]
fn sell_opens_the_bag_and_returns_to_the_menu() {
    let mut w = world();
    add(&mut w, POTION, 2);
    w.s.money = 0;
    open(&mut w);
    tap(&mut w, Buttons::DOWN);
    assert_eq!(prompt(&w), Prompt::Menu { cursor: SELL });
    tap(&mut w, Buttons::A);
    until(&mut w, |w| {
        matches!(top(w), Some(ScreenState::Bag(b)) if b.mode == BagMode::Sell)
            && w.screens()
                .is_some_and(|s| s.host.phase == crate::ui::kit::Phase::Running)
    });
    tap(&mut w, Buttons::B);
    for _ in 0..200 {
        if mart_on_top(&w) {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    through(&mut w, is(Prompt::Menu { cursor: BUY }));
}

/// `pokemart` opens the mart and the script waits until it closes (`CreatePokemartMenu`).
#[test]
fn the_pokemart_request_opens_the_mart_and_waits() {
    const DONE: VarId = VarId(0x4000);
    let mut w = world();
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.mart(&ITEMS).await;
        ctx.set_var(DONE, 1).await;
    }));
    until(&mut w, mart_on_top);
    assert_eq!(w.var(DONE), 0);
    tap(&mut w, Buttons::B);
    until(&mut w, |w| w.screens_open() == 0 && !w.script_running());
    assert_eq!(w.var(DONE), 1);
}
