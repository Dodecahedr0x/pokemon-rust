//! Items and money (owned by Milestone 8 Task H): `additem`, `removeitem`, `checkitem`,
//! `checkitemspace`, `addmoney`, `removemoney`, `checkmoney` (Gen3 `item.c`, `money.c`).

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

/// Wave 0: nothing changes; checks answer false.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = &h.s;
    match r {
        Request::MoneyAdd(_) | Request::MoneyRemove(_) => UNIT,
        _ => Some(Response::Bool(false)),
    }
}
