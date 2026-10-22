//! Multichoice menus (owned by Milestone 8 Task H): Gen3 `multichoice`, `ShowMultichoice`
//! (`script_menu.c`).

use super::Host;
use crate::data::GameData;
use crate::script::{MULTI_B_PRESSED, Request, Response};

/// Wave 0: answers [`MULTI_B_PRESSED`] at once (as if B was pressed).
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = (&h.s, r);
    Some(Response::Value(MULTI_B_PRESSED))
}

/// Whether a started menu is done (Wave 0 never starts one).
pub(super) fn done<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> bool {
    let _ = (&h.s, r);
    true
}

/// The answer of a started menu, once done.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    done(h, r).then_some(Response::Value(MULTI_B_PRESSED))
}
