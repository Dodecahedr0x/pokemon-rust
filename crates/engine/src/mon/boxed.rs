//! A boxed Pokémon (Gen3 `struct BoxPokemon`), the compact DTO the PC and the save keep
//! (Milestone 9 Wave 0; Task M owns the file afterwards).
//!
//! Derived fields are dropped, as a Gen3 `BoxPokemon` drops them: the level (recomputed from
//! the exp), the stats and the current HP (recalculated on withdraw, Gen3 `BoxMonToMon` +
//! `CalculateMonStats`: a withdrawn mon comes back at full HP) and the status (cleared). The
//! IVs are packed in one `u32` (5 bits each, Gen3 order HP, Atk, Def, Speed, SpAtk, SpDef), and
//! empty move slots are move 0 (Gen3 `MOVE_NONE`), so a boxed mon encodes in at most about 55
//! postcard bytes.

use super::{BattleTables, NAME_LEN, Pokemon, Status, level_for_exp};
use crate::data::{ItemId, MoveId, SpeciesId};
use crate::hash::Fnv64;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BoxedMon {
    pub species: SpeciesId,
    pub personality: u32,
    pub ot_id: u32,
    pub nickname: [u8; NAME_LEN],
    pub exp: u32,
    /// Five bits per IV, HP in the low bits.
    pub ivs: u32,
    pub evs: [u8; 6],
    /// Move ids, 0 for an empty slot.
    pub moves: [u16; 4],
    pub pp: [u8; 4],
    pub pp_bonuses: u8,
    pub friendship: u8,
    pub ability_slot: u8,
    pub held: Option<ItemId>,
}

impl Pokemon {
    /// The boxed form (Gen3 `SendMonToPC` copies `mon->box`): the level, stats, HP and status
    /// are dropped.
    pub fn to_boxed(&self) -> BoxedMon {
        let mut ivs = 0u32;
        for (i, &v) in self.ivs.iter().enumerate() {
            ivs |= ((v & 31) as u32) << (5 * i);
        }
        BoxedMon {
            species: self.species,
            personality: self.personality,
            ot_id: self.ot_id,
            nickname: self.nickname,
            exp: self.exp,
            ivs,
            evs: self.evs,
            moves: self.moves.map(|m| m.map_or(0, |m| m.0)),
            pp: self.pp,
            pp_bonuses: self.pp_bonuses,
            friendship: self.friendship,
            ability_slot: self.ability_slot,
            held: self.held,
        }
    }

    /// Gen3 `BoxMonToMon`: the level from the exp, the stats recalculated, full HP, no status.
    pub fn from_boxed(t: &BattleTables, b: &BoxedMon) -> Pokemon {
        let info = t.species_info(b.species);
        let mut p = Pokemon {
            species: b.species,
            personality: b.personality,
            ot_id: b.ot_id,
            nickname: b.nickname,
            level: level_for_exp(info.growth, b.exp),
            exp: b.exp,
            ivs: core::array::from_fn(|i| ((b.ivs >> (5 * i)) & 31) as u8),
            evs: b.evs,
            moves: b.moves.map(|m| (m != 0).then_some(MoveId(m))),
            pp: b.pp,
            pp_bonuses: b.pp_bonuses,
            friendship: b.friendship,
            ability_slot: b.ability_slot,
            status: Status::None,
            hp: 0,
            stats: [0; 6],
            held: b.held,
        };
        // Both 0: `recalc_stats` starts a new mon at full HP.
        p.recalc_stats(t);
        p
    }
}

impl BoxedMon {
    /// Feeds every field into `h`.
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.species.0);
        h.write_u32(self.personality);
        h.write_u32(self.ot_id);
        h.write(&self.nickname);
        h.write_u32(self.exp);
        h.write_u32(self.ivs);
        h.write(&self.evs);
        for m in self.moves {
            h.write_u16(m);
        }
        h.write(&self.pp);
        h.write(&[self.pp_bonuses, self.friendship, self.ability_slot]);
        h.write_u16(self.held.map_or(0, |i| i.0));
    }
}
