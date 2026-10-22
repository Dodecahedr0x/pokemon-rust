//! Multichoice menus (owned by Milestone 8 Task H): Gen3 `multichoice`, `multichoicedefault`
//! (`ScrCmd_multichoice`, `ScriptMenu_Multichoice`, `script_menu.c`). The window itself is
//! [`crate::menu::Multichoice`], shown and updated by the field text box.
//!
//! A list the game does not have (or an empty one) answers [`MULTI_B_PRESSED`] at once and
//! raises [`Report::Degraded`]; so does a multichoice in a map script (Gen3 would stop the
//! immediate script's context).

use super::Host;
use super::items::report;
use crate::data::GameData;
use crate::menu::Multichoice;
use crate::script::{MULTI_B_PRESSED, Report, Request, Response};

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let Request::Multichoice {
        list,
        x,
        y,
        default,
        cancel,
    } = *r
    else {
        return Some(Response::Value(MULTI_B_PRESSED));
    };
    let s = &mut *h.s;
    if h.immediate {
        report(s, Report::Degraded("multichoice in a map script"));
        return Some(Response::Value(MULTI_B_PRESSED));
    }
    match h.game.multichoice(list) {
        Some(items) if !items.is_empty() => {
            let font = h.game.font();
            let menu = Multichoice::new(font, list, items, (x, y), default, cancel);
            s.text.start_multichoice(h.game, menu, h.f);
            None
        }
        _ => {
            report(s, Report::Degraded("multichoice: unknown list"));
            Some(Response::Value(MULTI_B_PRESSED))
        }
    }
}

/// Whether the started menu has been answered.
pub(super) fn done<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> bool {
    let _ = r;
    !h.s.text.is_multichoice_active()
}

/// The answer, once the menu has closed (the index, or [`MULTI_B_PRESSED`]). A text box closed
/// under the menu (nothing does that today) answers [`MULTI_B_PRESSED`].
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    if !done(h, r) {
        return None;
    }
    let answer =
        h.s.text
            .take_multichoice_result()
            .unwrap_or(MULTI_B_PRESSED);
    Some(Response::Value(answer))
}
