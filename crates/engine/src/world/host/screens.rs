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
use crate::ui::{PartyMode, ScreenKind};
use crate::world::screens::{ANSWER_CANCEL, open};

/// Opens the request's screen; its result answers it in [`resume`].
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let kind = match *r {
        Request::ChoosePartyMon(mode) => ScreenKind::Party(mode),
        // `pokemart` (`CreatePokemartMenu`): the script waits until the shop closes.
        Request::Mart { items } => ScreenKind::Mart { items },
        // `special SaveGame` (Task S): the save dialog over the field.
        Request::SaveDialog => ScreenKind::SaveDialog,
        // `special GameClear`'s record and credits (Task S); the save waits for the field.
        Request::HallOfFame => ScreenKind::HallOfFame,
        // The evolution scene (Task E's screen): until it lands, the species change of the
        // scene without it (`Pokemon::evolve`), reported as the bag's item evolutions are.
        Request::Evolve { slot, into, .. } => {
            let t = h.game.battle();
            let Some(mon) = h.s.party.get_mut(slot as usize) else {
                return Some(Response::Bool(false));
            };
            mon.evolve(t, into);
            h.s.pokedex.set_caught(into);
            h.s.report(crate::script::Report::Degraded("evolution scene"));
            return Some(Response::Bool(true));
        }
        _ => return Some(default_answer(r)),
    };
    if h.immediate {
        h.s.log("screen request in a map script");
        return Some(default_answer(r));
    }
    h.s.answer = None;
    if kind == ScreenKind::HallOfFame {
        h.s.save.pending = true;
    }
    open(h.s, h.game, kind, true, h.f);
    None
}

/// The answer once the screen has closed.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let v = h.s.answer.take()?;
    Some(match r {
        // `CB2_ReturnFromChooseHalfParty`: the order is kept (`gSelectedOrderFromParty`), the
        // answer is whether a mon was chosen.
        Request::ChoosePartyMon(PartyMode::ChooseHalf { .. }) => {
            let order = if v == ANSWER_CANCEL {
                [0; super::stash::MAX_ORDER]
            } else {
                super::stash::unpack_order(v)
            };
            h.s.stash.order = order;
            Response::Value((order[0] != 0) as u16)
        }
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
        Request::ChoosePartyMon(PartyMode::ChooseHalf { .. }) => Response::Value(0),
        Request::ChoosePartyMon(_) => Response::Value(PARTY_NOTHING_CHOSEN),
        Request::SaveDialog | Request::Evolve { .. } => Response::Bool(false),
        _ => Response::Unit,
    }
}
