//! Party, money and coin requests (Milestone 9 Task R owns the file): `givemon` with its
//! extras, `MoneyGet`, the coin requests, `AddFriendship`, `PartyMonSpecies` and the in-game
//! trade swap.
//!
//! `givemon` is the Milestone 4 behaviour routed through `world/caught.rs`, with the Pokédex
//! recording Gen3 `ScriptGiveMon` does, and its extras. The in-game trade swap answers its
//! default (Task J's).

use super::{Host, UNIT};
use crate::data::{GameData, SpeciesId};
use crate::script::{GiveMonExtra, Request, Response};
use crate::world::caught::{Placed, place_mon};

/// Gen3 `givemon` results.
const MON_GIVEN_TO_PARTY: u16 = 0;
const MON_GIVEN_TO_PC: u16 = 1;
const MON_CANT_GIVE: u16 = 2;

/// Gen3 `ScriptGiveMon`: creates the mon, applies the extras (moves replacing the level-up
/// set, with their full PP; the held item; the OT id; the nickname), places it (party, else
/// PC) and, when it was given, sets its Pokédex seen and caught flags. Returns the `givemon`
/// result. The OT name has no field in `Pokemon` and is not kept.
pub(super) fn give_mon<G: GameData>(
    h: &mut Host<'_, G>,
    species: SpeciesId,
    level: u8,
    extra: Option<&'static GiveMonExtra>,
) -> u16 {
    let s = &mut *h.s;
    let mut mon = s.create_mon(h.game, species, level);
    if let Some(e) = extra {
        let t = h.game.battle();
        if let Some(moves) = e.moves {
            for (slot, m) in moves.into_iter().enumerate() {
                mon.moves[slot] = (m.0 != 0).then_some(m);
            }
            for slot in 0..4 {
                mon.pp[slot] = mon.max_pp(t, slot as u8);
            }
        }
        if let Some(item) = e.item {
            mon.held = Some(item);
        }
        if let Some((_, id)) = e.ot {
            mon.ot_id = id;
        }
        if let Some(name) = e.nickname {
            let mut n = [0xFF; crate::mon::NAME_LEN];
            let src = h.game.text(name).iter().take_while(|&&c| c != 0xFF);
            for (d, &c) in n.iter_mut().take(crate::mon::NAME_LEN - 1).zip(src) {
                *d = c;
            }
            mon.nickname = n;
        }
    }
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

/// The money, coin and party reads and changes (Task R).
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let s = &mut *h.s;
    match *r {
        Request::MoneyGet => Some(Response::Word(s.money)),
        Request::CoinsGet => Some(Response::Value(s.coins)),
        Request::CoinsAdd(n) => {
            let max = h.game.special_ids().max_coins;
            let fits = u32::from(s.coins) + u32::from(n) <= u32::from(max);
            if fits {
                s.coins += n;
            }
            Some(Response::Bool(fits))
        }
        Request::CoinsRemove(n) => {
            let had = s.coins >= n;
            if had {
                s.coins -= n;
            }
            Some(Response::Bool(had))
        }
        Request::AddFriendship { slot, n } => {
            let change = |m: &mut crate::mon::Pokemon| {
                m.friendship = (i16::from(m.friendship) + i16::from(n)).clamp(0, 255) as u8;
            };
            if slot == 0xFF {
                for i in 0..s.party.len() {
                    if let Some(m) = s.party.get_mut(i) {
                        change(m);
                    }
                }
            } else if let Some(m) = s.party.get_mut(slot as usize) {
                change(m);
            }
            UNIT
        }
        Request::PartyMonSpecies(slot) => Some(Response::Value(
            s.party.get(slot as usize).map_or(0, |m| m.species.0),
        )),
        _ => UNIT,
    }
}

/// The answer of a request [`begin`] made wait, polled every frame: [`begin`] never waits.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    begin(h, r)
}
