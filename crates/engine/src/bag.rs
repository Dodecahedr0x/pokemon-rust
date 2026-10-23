//! The player's bag (Milestone 8): Gen3 pockets with the game's slot counts
//! (`SpecialIds::pocket_slots`), at most [`slot_capacity`] of an item per slot (99, berries
//! 999), and Gen3 `item.c`'s rules for adding, removing and checking items.
//!
//! Every write goes through [`Bag::set_slot`], whose bounds `assert!` is unreachable from the
//! other methods: they only index slots of the pocket they iterate.
//!
//! As [`crate::flow::Vars`], the bag keeps a digest of its contents up to date on every write,
//! so [`Bag::hash`] is O(1) and never walks the 186 slots.

use crate::checkpoint::{BagSlotSnap, BagSnap};
use crate::data::{ItemId, Pocket};
use crate::hash::{Fnv64, splitmix64};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

/// Gen3 `MAX_BAG_ITEM_CAPACITY`: per slot, in every pocket but the berries one.
pub const MAX_PER_SLOT: u16 = 99;
/// Gen3 `MAX_BERRY_CAPACITY`: per slot in the berries pocket.
pub const MAX_BERRIES_PER_SLOT: u16 = 999;

/// How many of an item one slot of `pocket` holds (Gen3 `AddBagItem`'s `slotCapacity`).
pub const fn slot_capacity(pocket: Pocket) -> u16 {
    match pocket {
        Pocket::Berries => MAX_BERRIES_PER_SLOT,
        _ => MAX_PER_SLOT,
    }
}

/// Gen3 `TMHM_POCKET` and `BERRIES_POCKET`: an item never takes a second slot there.
const fn single_slot(pocket: Pocket) -> bool {
    matches!(pocket, Pocket::TmHm | Pocket::Berries)
}

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
    /// Slots per pocket (`SpecialIds::pocket_slots`).
    sizes: [u8; 5],
}

impl Bag {
    /// First slot of `pocket` in `slots`.
    fn start(&self, pocket: Pocket) -> usize {
        (self.sizes[..pocket as usize].iter())
            .map(|&n| n as usize)
            .sum()
    }

    /// Slots of `pocket`.
    pub fn pocket_len(&self, pocket: Pocket) -> usize {
        self.sizes[pocket as usize] as usize
    }
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
    /// An empty bag with `sizes` slots per pocket, in [`Pocket::ALL`] order
    /// (`SpecialIds::pocket_slots`).
    pub fn new(sizes: [u8; 5]) -> Self {
        let n = sizes.iter().map(|&n| n as usize).sum();
        Bag {
            slots: vec![Slot::default(); n].into_boxed_slice(),
            digest: 0,
            sizes,
        }
    }

    /// The slots of `pocket` (empty ones included).
    pub fn pocket(&self, pocket: Pocket) -> &[Slot] {
        let start = self.start(pocket);
        &self.slots[start..start + self.pocket_len(pocket)]
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
        assert!(index < self.pocket_len(pocket), "bag slot out of range");
        let slot = if slot.qty == 0 { Slot::default() } else { slot };
        let i = self.start(pocket) + index;
        let old = self.slots[i];
        self.digest =
            (self.digest.wrapping_sub(slot_term(i, old))).wrapping_add(slot_term(i, slot));
        self.slots[i] = slot;
    }

    /// Gen3 `CheckBagHasItem`: whether the slots of `pocket` hold at least `qty` of `item`
    /// together (`qty` 0 needs one slot of it).
    pub fn has(&self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        let mut count = qty;
        for s in self.pocket(pocket) {
            if s.qty > 0 && s.item == item {
                if s.qty >= count {
                    return true;
                }
                count -= s.qty;
            }
        }
        false
    }

    /// Gen3 `AddBagItem`: adds `qty` of `item` to `pocket` if it all fits, else changes nothing.
    /// Existing stacks fill first, in slot order, then the first empty slots; in the TM/HM and
    /// berries pockets an item never takes a second slot. `qty` 0 always succeeds.
    pub fn add(&mut self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        if item.0 == 0 {
            return false;
        }
        let cap = slot_capacity(pocket);
        // Gen3 works on a copy (`newItems`) and keeps it only on success.
        let mut new: Vec<Slot> = self.pocket(pocket).to_vec();
        let mut count = qty;
        for s in new.iter_mut() {
            if s.qty > 0 && s.item == item {
                let owned = s.qty;
                if owned as u32 + count as u32 <= cap as u32 {
                    s.qty = owned + count;
                    count = 0;
                    break;
                }
                if single_slot(pocket) {
                    return false;
                }
                count -= cap - owned;
                s.qty = cap;
                if count == 0 {
                    break;
                }
            }
        }
        if count > 0 {
            for s in new.iter_mut() {
                if s.qty == 0 {
                    s.item = item;
                    if count > cap {
                        if single_slot(pocket) {
                            return false;
                        }
                        count -= cap;
                        s.qty = cap;
                    } else {
                        s.qty = count;
                        count = 0;
                        break;
                    }
                }
            }
            if count > 0 {
                return false;
            }
        }
        for (i, s) in new.iter().enumerate() {
            if *s != self.pocket(pocket)[i] {
                self.set_slot(pocket, i, *s);
            }
        }
        true
    }

    /// Gen3 `RemoveBagItem`: removes `qty` of `item` from `pocket`, slot by slot in order, if
    /// the pocket holds that many together; else changes nothing. Emptied slots stay where they
    /// are (Gen3 compacts pockets only when the bag menu opens). Gen3 first tries the slot under
    /// the bag menu's cursor, which is the first slot without a bag menu.
    pub fn remove(&mut self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        if item.0 == 0 {
            return false;
        }
        let total: u32 = (self.pocket(pocket).iter())
            .filter(|s| s.qty > 0 && s.item == item)
            .map(|s| s.qty as u32)
            .sum();
        if total < qty as u32 {
            return false;
        }
        let mut count = qty;
        for i in 0..self.pocket_len(pocket) {
            let s = self.pocket(pocket)[i];
            if s.qty > 0 && s.item == item {
                let take = s.qty.min(count);
                self.set_slot(
                    pocket,
                    i,
                    Slot {
                        item,
                        qty: s.qty - take,
                    },
                );
                count -= take;
                if count == 0 {
                    break;
                }
            }
        }
        true
    }

    /// Gen3 `CheckBagHasSpace`: whether [`Bag::add`] of the same arguments would succeed.
    pub fn has_space(&self, pocket: Pocket, item: ItemId, qty: u16) -> bool {
        if item.0 == 0 {
            return false;
        }
        let cap = slot_capacity(pocket);
        let slots = self.pocket(pocket);
        let mut count = qty;
        for s in slots {
            if s.qty > 0 && s.item == item {
                if s.qty as u32 + count as u32 <= cap as u32 {
                    return true;
                }
                if single_slot(pocket) {
                    return false;
                }
                count -= cap - s.qty;
                if count == 0 {
                    break;
                }
            }
        }
        if count > 0 {
            for s in slots {
                if s.qty == 0 {
                    if count > cap {
                        if single_slot(pocket) {
                            return false;
                        }
                        count -= cap;
                    } else {
                        return true;
                    }
                }
            }
            return false;
        }
        true
    }

    /// Gen3 `CompactItemsInBagPocket` (the bag menu, when it opens): the non-empty slots move to
    /// the front in their order; with `sort`, ordered by item id (`SortBerriesOrTMHMs`, the
    /// TM/HM and berries pockets).
    pub fn compact(&mut self, pocket: Pocket, sort: bool) {
        let mut full: Vec<Slot> = (self.pocket(pocket).iter())
            .filter(|s| s.qty > 0)
            .copied()
            .collect();
        if sort {
            full.sort_by_key(|s| s.item.0);
        }
        for i in 0..self.pocket_len(pocket) {
            let s = full.get(i).copied().unwrap_or_default();
            if self.pocket(pocket)[i] != s {
                self.set_slot(pocket, i, s);
            }
        }
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

    /// Rebuilt with `sizes` slots per pocket ([`Bag::new`]) through [`Bag::set_slot`], so the
    /// digest matches the contents. Refuses slots out of range, quantities outside
    /// `1..=slot_capacity(pocket)`, `ITEM_NONE` and repeated slots.
    pub fn from_snapshot(s: &BagSnap, sizes: [u8; 5]) -> Result<Bag, &'static str> {
        let mut bag = Bag::new(sizes);
        for b in &s.slots {
            let index = b.index as usize;
            if index >= bag.pocket_len(b.pocket) {
                return Err("bag slot out of range");
            }
            if b.qty == 0 || b.qty > slot_capacity(b.pocket) || b.item.0 == 0 {
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
