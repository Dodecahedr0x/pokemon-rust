//! [`Request::Query`](crate::script::Request::Query) (Wave C): the reads of the player's state
//! that Gen3 specials make (`GetMonData`, `GetSetPokedexFlag`, `IsBagPocketNonEmpty`, …).
//! Every query answers at once and changes nothing.

use super::Host;
use crate::data::GameData;
use crate::pokedex::DexFlag;
use crate::script::{PARTY_SIZE, Query};

pub(super) fn answer<G: GameData>(h: &Host<'_, G>, q: Query) -> u16 {
    let s = &*h.s;
    match q {
        Query::DexCount { caught, among } => {
            let flag = if caught {
                DexFlag::Caught
            } else {
                DexFlag::Seen
            };
            if among.is_empty() {
                s.pokedex.count(flag)
            } else {
                among.iter().filter(|&&sp| s.pokedex.get(flag, sp)).count() as u16
            }
        }
        Query::PartySpecies(slot) => s.party.get(slot as usize).map_or(0, |m| m.species.0),
        Query::PartyMoveSlot(m) => s
            .party
            .iter()
            .position(|mon| mon.moves.contains(&Some(m)))
            .map_or(PARTY_SIZE, |i| i as u16),
        Query::PartyEvTotal(slot) => s
            .party
            .get(slot as usize)
            .map_or(0, |m| m.evs.iter().map(|&e| e as u16).sum()),
        Query::PartyFriendship(slot) => s
            .party
            .get(slot as usize)
            .map_or(0, |m| m.friendship.into()),
        Query::PartyHp(slot) => s.party.get(slot as usize).map_or(0, |m| m.hp),
        Query::PartyPersonalityLow(slot) => s
            .party
            .get(slot as usize)
            .map_or(0, |m| m.personality as u16),
        Query::PlayerIdLow => s.player_id as u16,
        Query::PocketNonEmpty(p) => s.bag.pocket(p).iter().any(|sl| sl.qty > 0) as u16,
    }
}
