//! Requests that open a screen (Milestone 9 Task K owns the file): `pokemart`,
//! `ChoosePartyMon`, the PCs, `special SaveGame` and the evolution scene.
//!
//! Wave 0: every request answers its default at once and opens nothing.

use super::Host;
use crate::data::GameData;
use crate::script::{PARTY_NOTHING_CHOSEN, Request, Response};

/// Opens the request's screen (Task K); the screen's result answers it in [`resume`].
/// Wave 0: the defaults (`Unit`, `PARTY_NOTHING_CHOSEN`, "not saved", "not evolved").
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = &h.s;
    Some(default_answer(r))
}

/// The answer once the screen has closed (Task K). Wave 0: never waits.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = &h.s;
    Some(default_answer(r))
}

fn default_answer(r: &Request) -> Response {
    match r {
        Request::ChoosePartyMon(_) => Response::Value(PARTY_NOTHING_CHOSEN),
        Request::SaveDialog | Request::Evolve { .. } => Response::Bool(false),
        _ => Response::Unit,
    }
}
