//! Where a new mon goes: the party, else the PC (Gen3 `GiveMonToPlayer` → `SendMonToPC`;
//! Milestone 9 Task M owns the file). Used by the catch (`world/battle.rs`) and by `givemon`
//! (`world/host/party.rs`).
//!
//! Wave 0's body is the behaviour before the PC: the party, else nowhere.

use super::State;
use crate::data::GameData;
use crate::mon::Pokemon;

/// Where [`place_mon`] put the mon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code, reason = "Milestone 9 Wave 0: Task M places mons in the PC")]
pub(crate) enum Placed {
    /// Party slot.
    Party(u8),
    /// Box and slot (Gen3 `MON_GIVEN_TO_PC`).
    Pc { pc_box: u8, slot: u8 },
    /// Party and PC full (Gen3 `MON_CANT_GIVE`): the mon is dropped.
    NoRoom,
}

/// Gen3 `GiveMonToPlayer`: the first free party slot, else the PC (Task M).
pub(crate) fn place_mon(game: &impl GameData, s: &mut State, mon: Pokemon) -> Placed {
    let _ = game;
    match s.party.push(mon) {
        Ok(slot) => Placed::Party(slot),
        Err(_) => Placed::NoRoom,
    }
}
