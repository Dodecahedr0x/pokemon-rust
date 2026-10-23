//! Requests that open a screen (Milestone 9 Task K owns the file): `pokemart`,
//! `ChoosePartyMon`, the PCs, `special SaveGame` and the evolution scene.
//!
//! A request opens its screen (`world/screens.rs::open`, marked waiting) and is answered once
//! the last screen has closed, from `State::answer`. Only the screens that exist open: the
//! party menu (`ChoosePartyMon` / `ChooseMonForMoveTutor`, Task PT) and the mart (Task M).
//! The others (PC,
//! save dialog, evolution scene) answer their default at once until their owners land, so a
//! script never waits on a stub. A map script (immediate) never opens a screen: it gets the
//! default and a log line, as M8 does for waiting requests there.

use super::Host;
use crate::data::GameData;
use crate::script::{PARTY_NOTHING_CHOSEN, Request, Response};
use crate::ui::ScreenKind;
use crate::world::screens::{ANSWER_CANCEL, open};

/// Opens the request's screen; its result answers it in [`resume`].
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let kind = match *r {
        Request::ChoosePartyMon(mode) => ScreenKind::Party(mode),
        // `pokemart` (`CreatePokemartMenu`): the script waits until the shop closes.
        Request::Mart { items } => ScreenKind::Mart { items },
        _ => return Some(default_answer(r)),
    };
    if h.immediate {
        h.s.log("screen request in a map script");
        return Some(default_answer(r));
    }
    h.s.answer = None;
    open(h.s, h.game, kind, true, h.f);
    None
}

/// The answer once the screen has closed.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let v = h.s.answer.take()?;
    Some(match r {
        Request::ChoosePartyMon(_) => Response::Value(if v == ANSWER_CANCEL {
            PARTY_NOTHING_CHOSEN
        } else {
            v
        }),
        Request::SaveDialog | Request::Evolve { .. } => Response::Bool(v == 1),
        _ => Response::Unit,
    })
}

fn default_answer(r: &Request) -> Response {
    match r {
        Request::ChoosePartyMon(_) => Response::Value(PARTY_NOTHING_CHOSEN),
        Request::SaveDialog | Request::Evolve { .. } => Response::Bool(false),
        _ => Response::Unit,
    }
}
