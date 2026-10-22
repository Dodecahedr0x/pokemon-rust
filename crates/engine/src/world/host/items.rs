//! Items and money (owned by Milestone 8 Task H): `additem`, `removeitem`, `checkitem`,
//! `checkitemspace`, `addmoney`, `removemoney`, `checkmoney` (Gen3 `item.c`, `money.c`).
//!
//! An item resolves to its pocket through [`GameData::item`]. `ITEM_NONE` (0) answers false, as
//! Gen3's `POCKET_NONE` does; an item the game does not know also answers false and raises
//! [`Report::Degraded`] (Gen3 would read past `gItems`).

use super::{Host, UNIT};
use crate::data::{GameData, ItemId, Pocket};
use crate::script::{Report, Request, Response};
use crate::world::{Event, State};

/// Gen3 `MAX_MONEY`.
pub(crate) const MAX_MONEY: u32 = 999_999;

/// Logs `r` and raises it as an event, as `Request::Report` does (shared by Task H's handlers).
pub(super) fn report(s: &mut State, r: Report) {
    let (Report::Unsupported(what) | Report::Degraded(what)) = r;
    s.log(what);
    s.event(Event::Report(r));
}

/// The pocket of `item`, or `None` (reported when the game does not know it).
fn pocket<G: GameData>(h: &mut Host<'_, G>, item: ItemId, what: &'static str) -> Option<Pocket> {
    if item.0 == 0 {
        return None;
    }
    let p = h.game.item(item).map(|i| i.pocket);
    if p.is_none() {
        report(h.s, Report::Degraded(what));
    }
    p
}

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let ok = match *r {
        Request::ItemAdd { item, qty } => {
            pocket(h, item, "additem: unknown item").is_some_and(|p| h.s.bag.add(p, item, qty))
        }
        Request::ItemRemove { item, qty } => pocket(h, item, "removeitem: unknown item")
            .is_some_and(|p| h.s.bag.remove(p, item, qty)),
        Request::ItemCheck { item, qty } => {
            pocket(h, item, "checkitem: unknown item").is_some_and(|p| h.s.bag.has(p, item, qty))
        }
        Request::ItemSpace { item, qty } => pocket(h, item, "checkitemspace: unknown item")
            .is_some_and(|p| h.s.bag.has_space(p, item, qty)),
        // Gen3 `AddMoney` caps at `MAX_MONEY` (it overflows past it only for amounts over
        // `MAX_MONEY`, which no script gives; capped here too).
        Request::MoneyAdd(n) => {
            h.s.money = h.s.money.saturating_add(n).min(MAX_MONEY);
            return UNIT;
        }
        // Gen3 `RemoveMoney` stops at 0.
        Request::MoneyRemove(n) => {
            h.s.money = h.s.money.saturating_sub(n);
            return UNIT;
        }
        // Gen3 `IsEnoughMoney`.
        Request::MoneyCheck(n) => h.s.money >= n,
        _ => return UNIT,
    };
    Some(Response::Bool(ok))
}
