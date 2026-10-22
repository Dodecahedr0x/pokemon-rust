//! The player's bag (Milestone 8; `add`, `remove` and `has_space` are Task H's): Gen3 Emerald
//! pockets with their capacities (`include/constants/global.h`), at most [`MAX_PER_SLOT`] of an
//! item per slot.
//!
//! As [`crate::flow::Vars`], the bag keeps a digest of its contents up to date on every write,
//! so [`Bag::hash`] is O(1) and never walks the 186 slots.

use crate::checkpoint::{BagSlotSnap, BagSnap};
use crate::data::{ItemId, Pocket};
use crate::hash::{Fnv64, splitmix64};
use alloc::boxed::Box;
use alloc::vec;

/// Slots per pocket, in [`Pocket::ALL`] order (Gen3 `BAG_ITEMS_COUNT`, `BAG_KEYITEMS_COUNT`,
/// `BAG_POKEBALLS_COUNT`, `BAG_TMHM_COUNT`, `BAG_BERRIES_COUNT`).
pub const POCKET_CAPACITY: [usize; 5] = [30, 30, 16, 64, 46];
/// Gen3 `MAX_BAG_ITEM_CAPACITY`.
pub const MAX_PER_SLOT: u16 = 99;
/// Every slot of every pocket.
pub const BAG_SLOTS: usize = 30 + 30 + 16 + 64 + 46;

/// One bag slot; `qty` 0 is an empty slot (its item is then `ItemId(0)`, Gen3 `ITEM_NONE`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot {
    pub item: ItemId,
    pub qty: u16,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            item: ItemId(0),
            qty: 0,
        }
    }
}

/// The bag. Heap-backed so `World` stays small on the GBA stack.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bag {
    /// Every pocket's slots, pockets in [`Pocket::ALL`] order.
    slots: Box<[Slot]>,
    digest: u64,
}

impl Default for Bag {
    fn default() -> Self {
        Self::new()
    }
}

/// First slot of `pocket` in `Bag::slots`.
const fn pocket_start(pocket: Pocket) -> usize {
    let mut start = 0;
    let mut i = 0;
    while i < pocket as usize {
        start += POCKET_CAPACITY[i];
        i += 1;
    }
    start
}

/// Digest contribution of slot `i` (0 for an empty slot).
const fn slot_term(i: usize, s: Slot) -> u64 {
    if s.qty == 0 {
        0
    } else {
        splitmix64(3 << 48 | (i as u64) << 32 | (s.item.0 as u64) << 16 | s.qty as u64)
    }
}

impl Bag {
    /// An empty bag.
    pub fn new() -> Self {
        Bag {
            slots: vec![Slot::default(); BAG_SLOTS].into_boxed_slice(),
            digest: 0,
        }
    }

    /// The slots of `pocket` (empty ones included).
    pub fn pocket(&self, pocket: Pocket) -> &[Slot] {
        let start = pocket_start(pocket);
        &self.slots[start..start + POCKET_CAPACITY[pocket as usize]]
    }

    /// How many of `item` the bag holds, over every slot.
    pub fn count(&self, item: ItemId) -> u32 {
        (self.slots.iter())
            .filter(|s| s.qty > 0 && s.item == item)
            .map(|s| s.qty as u32)
            .sum()
    }

    /// Sets slot `index` of `pocket`, keeping the digest (the one writer of `slots`).
    pub fn set_slot(&mut self, pocket: Pocket, index: usize, slot: Slot) {
        assert!(
            index < POCKET_CAPACITY[pocket as usize],
            "bag slot out of range"
        );
        let slot = if slot.qty == 0 { Slot::default() } else { slot };
        let i = pocket_start(pocket) + index;
        let old = self.slots[i];
        self.digest =
            (self.digest.wrapping_sub(slot_term(i, old))).wrapping_add(slot_term(i, slot));
        self.slots[i] = slot;
    }

    /// Gen3 `AddBagItem`: adds `qty` of `item` to `pocket` if it all fits. Task H.
    pub fn add(&mut self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        let _ = (pocket, item, qty);
        todo!("Milestone 8 Task H: Bag::add")
    }

    /// Gen3 `RemoveBagItem`: removes `qty` of `item` if the bag holds that many. Task H.
    pub fn remove(&mut self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        let _ = (pocket, item, qty);
        todo!("Milestone 8 Task H: Bag::remove")
    }

    /// Gen3 `CheckBagHasSpace`. Task H.
    pub fn has_space(&self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        let _ = (pocket, item, qty);
        todo!("Milestone 8 Task H: Bag::has_space")
    }

    /// The digest of every slot, recomputed from scratch (what `digest` tracks).
    fn full_digest(&self) -> u64 {
        (self.slots.iter().enumerate()).fold(0u64, |d, (i, &s)| d.wrapping_add(slot_term(i, s)))
    }

    /// Feeds the contents digest (O(1)); equal contents give equal hashes.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        debug_assert!(cfg!(not(test)) || self.digest == self.full_digest());
        h.write(&self.digest.to_le_bytes());
    }

    /// The non-empty slots, in pocket then slot order.
    pub fn snapshot(&self) -> BagSnap {
        let mut slots = alloc::vec::Vec::new();
        for pocket in Pocket::ALL {
            for (index, s) in self.pocket(pocket).iter().enumerate() {
                if s.qty > 0 {
                    slots.push(BagSlotSnap {
                        pocket,
                        index: index as u8,
                        item: s.item,
                        qty: s.qty,
                    });
                }
            }
        }
        BagSnap { slots }
    }

    /// Rebuilt through [`Bag::set_slot`], so the digest matches the contents. Refuses slots out
    /// of range, quantities outside `1..=MAX_PER_SLOT`, `ITEM_NONE` and repeated slots.
    pub fn from_snapshot(s: &BagSnap) -> Result<Bag, &'static str> {
        let mut bag = Bag::new();
        for b in &s.slots {
            let index = b.index as usize;
            if index >= POCKET_CAPACITY[b.pocket as usize] {
                return Err("bag slot out of range");
            }
            if b.qty == 0 || b.qty > MAX_PER_SLOT || b.item.0 == 0 {
                return Err("bad bag slot");
            }
            if bag.pocket(b.pocket)[index].qty > 0 {
                return Err("bag slot given twice");
            }
            bag.set_slot(
                b.pocket,
                index,
                Slot {
                    item: b.item,
                    qty: b.qty,
                },
            );
        }
        Ok(bag)
    }
}

#[cfg(test)]
mod tests;
