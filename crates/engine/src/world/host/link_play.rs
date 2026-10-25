//! `Request::Link`: the cable specials, answered at once from the attached partner.

use super::Host;
use crate::data::GameData;
use crate::link::{self, LinkSide};
use crate::script::LinkCall;
use crate::world::State;

pub(super) fn answer<G: GameData>(host: &mut Host<'_, G>, call: LinkCall) -> u16 {
    match call {
        LinkCall::BecomeLeader => {
            let ok = host.s.link_partner.is_some();
            host.s.link_open = ok;
            u16::from(ok)
        }
        LinkCall::Close => {
            if host.s.link_open && host.s.link_partner.is_some() {
                let _ = link::ready_close();
            }
            host.s.link_open = false;
            host.s.union_ready = false;
            0
        }
        LinkCall::InitUnion => {
            host.s.union_ready = host.s.link_partner.is_some();
            0
        }
        LinkCall::Battle => {
            let game = host.game;
            battle(game, host.s)
        }
        LinkCall::UnionRoom => union_room(host.s),
    }
}

fn battle<G: GameData>(game: &G, s: &mut State) -> u16 {
    let Some(partner) = s.link_partner.clone() else {
        return 0;
    };
    if !s.link_open {
        return 0;
    }
    let local = local_side(s);
    match link::single_battle(game, &mut s.rng, &local, &partner) {
        Ok(done) => {
            s.party = done.player;
            done.outcome as u16
        }
        Err(_) => 0,
    }
}

fn union_room(s: &mut State) -> u16 {
    let Some(partner) = s.link_partner.clone() else {
        return 0;
    };
    if !s.link_open || !s.union_ready {
        return 0;
    }
    let local = local_side(s);
    match link::union_room(&local, &partner) {
        Ok(ex) if ex.accepted => {
            s.str_vars.set(0, &ex.partner_name);
            s.union_ready = false;
            1
        }
        _ => {
            s.union_ready = false;
            0
        }
    }
}

fn local_side(s: &State) -> LinkSide {
    LinkSide {
        name: s.player_name,
        trainer_id: s.player_id,
        gender: 0,
        party: s.party.clone(),
        move_slot: 0,
    }
}
