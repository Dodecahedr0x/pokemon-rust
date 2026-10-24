//! The party's backup around a battle fought with part of the party (Wave F): Gen3
//! `SavePlayerParty` / `LoadPlayerParty` (`load_save.c`), `ReducePlayerPartyToSelectedMons`
//! (`script_pokemon_util.c`), `SaveSelectedParty` (`frontier_util.c`, `frontier_saveparty`) and
//! `gSelectedOrderFromParty`, which the choose-half party menu answers (`party_menu.c`).
//!
//! Gen3 keeps the backup in the save block and the order in RAM; here both live in [`Stash`],
//! which only a running script uses (Steven's multi battle): it is neither snapshotted nor
//! hashed while empty, and it is cleared when the script ends.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::mon::{PARTY_SIZE, Party};
use crate::script::{PartyStash, Response};

/// Gen3 `MAX_FRONTIER_PARTY_SIZE`: the most mons the choose-half menu orders.
pub use crate::ui::party::MAX_ORDER;

/// The party's backup and the chosen order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(in crate::world) struct Stash {
    /// Gen3 `gSaveBlock1Ptr->playerParty` (written by `SavePlayerParty`).
    pub saved: Option<Party>,
    /// Gen3 `gSelectedOrderFromParty`: party slots + 1 in the chosen order, 0 past the last.
    pub order: [u8; MAX_ORDER],
    /// The player's mons past the first [`crate::battle::MULTI_PARTY_SIZE`] while an in-game
    /// partner's take their slots (a game's partner battle on a full party; Emerald's script
    /// reduces the party first, so its bench stays empty): they come back after the battle.
    pub bench: Party,
}

impl Stash {
    pub fn is_empty(&self) -> bool {
        self.saved.is_none() && self.order == [0; MAX_ORDER] && self.bench.is_empty()
    }

    /// Feeds the stash into `h` when it holds anything.
    pub fn hash(&self, h: &mut Fnv64) {
        if self.is_empty() {
            return;
        }
        h.write(&[0xB8]);
        h.write(&self.order);
        match &self.saved {
            None => h.write(&[0]),
            Some(p) => {
                h.write(&[1]);
                p.hash(h);
            }
        }
        // Only when set: Emerald's stash hashes as before.
        if !self.bench.is_empty() {
            h.write(&[0xB9]);
            self.bench.hash(h);
        }
    }

    /// Runs `op` on `party`.
    pub fn run(&mut self, party: &mut Party, op: PartyStash) {
        match op {
            PartyStash::Save => self.saved = Some(party.clone()),
            PartyStash::Load => {
                if let Some(p) = self.saved.take() {
                    *party = p;
                }
            }
            PartyStash::Reduce => {
                let mut out = Party::default();
                for &o in self.order.iter().take_while(|&&o| o != 0) {
                    if let Some(m) = party.get(o as usize - 1) {
                        let _ = out.push(*m);
                    }
                }
                *party = out;
            }
            PartyStash::SaveSelected => {
                let Some(saved) = &mut self.saved else {
                    return;
                };
                for (i, &o) in self.order.iter().enumerate() {
                    let slot = (o as usize).wrapping_sub(1);
                    if slot < PARTY_SIZE
                        && let (Some(dst), Some(src)) = (saved.get_mut(slot), party.get(i))
                    {
                        *dst = *src;
                    }
                }
            }
        }
    }
}

/// The choose-half menu's answer (`ScreenResult::Value`, [`crate::ui::party::pack_order`]) as
/// the order.
pub(in crate::world) fn unpack_order(v: u16) -> [u8; MAX_ORDER] {
    core::array::from_fn(|i| ((v >> (4 * i)) & 0xF) as u8)
}

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, op: PartyStash) -> Option<Response> {
    let s = &mut *h.s;
    s.stash.run(&mut s.party, op);
    UNIT
}
