//! Party, money and coin requests (Milestone 9 Task R owns the file): `givemon` with its
//! extras, `MoneyGet`, the coin requests, `AddFriendship`, `PartyMonSpecies` and the in-game
//! trade swap.
//!
//! Wave 0: `givemon` is the Milestone 4 behaviour routed through `world/caught.rs`, with the
//! Pokédex recording Gen3 `ScriptGiveMon` does; the other requests answer their defaults.

use super::{Host, UNIT};
use crate::data::{GameData, SpeciesId};
use crate::script::{GiveMonExtra, Request, Response};
use crate::world::caught::{Placed, place_mon};

/// Gen3 `givemon` results.
const MON_GIVEN_TO_PARTY: u16 = 0;
const MON_GIVEN_TO_PC: u16 = 1;
const MON_CANT_GIVE: u16 = 2;

/// Gen3 `ScriptGiveMon`: creates the mon, places it (party, else PC) and, when it was given,
/// sets its Pokédex seen and caught flags. Returns the `givemon` result. The extras (moves,
/// held item, OT, nickname) are Task R's; Wave 0 ignores them.
pub(super) fn give_mon<G: GameData>(
    h: &mut Host<'_, G>,
    species: SpeciesId,
    level: u8,
    extra: Option<&'static GiveMonExtra>,
) -> u16 {
    let _ = extra;
    let s = &mut *h.s;
    let mon = s.create_mon(h.game, species, level);
    match place_mon(h.game, s, mon) {
        Placed::NoRoom => {
            s.log("givemon: party full and no PC");
            MON_CANT_GIVE
        }
        placed => {
            s.pokedex.set_caught(species);
            if matches!(placed, Placed::Party(_)) {
                MON_GIVEN_TO_PARTY
            } else {
                MON_GIVEN_TO_PC
            }
        }
    }
}

/// The other party and money requests (Task R). Wave 0: the defaults.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = &h.s;
    match r {
        Request::MoneyGet => Some(Response::Word(0)),
        Request::CoinsGet | Request::PartyMonSpecies(_) => Some(Response::Value(0)),
        Request::CoinsAdd(_) | Request::CoinsRemove(_) => Some(Response::Bool(false)),
        _ => UNIT,
    }
}
