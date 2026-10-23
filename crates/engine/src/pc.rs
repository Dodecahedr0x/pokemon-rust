//! PC storage: the item storage and the Pokémon boxes (Gen3 `gSaveBlock1Ptr->pcItems`,
//! `gPokemonStoragePtr`; Milestone 9). Wave 0 declares the state (with its digest, hash and
//! snapshot); Task M adds the storage logic (Gen3 `SendMonToPC`, box order, withdraw).
//!
//! Box names and wallpapers are not kept (the list UI shows "BOX n"). The boxes are allocated
//! as they are first written, so a game that never uses the PC keeps its heap.
//!
//! As the bag, the storage keeps a digest of its contents on every write, so [`PcStorage::hash`]
//! is O(1).

use crate::bag::{MAX_PER_SLOT, Slot};
use crate::data::ItemId;
use crate::hash::{Fnv64, splitmix64};
use crate::mon::BoxedMon;
use alloc::vec;
use alloc::vec::Vec;

/// Mons per box (Gen3 `IN_BOX_COUNT`).
pub const BOX_SLOTS: usize = 30;

/// One box.
pub type PcBox = [Option<BoxedMon>; BOX_SLOTS];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PcStorage {
    items: Vec<Slot>,
    /// Boxes `0..boxes.len()`; the others are empty.
    boxes: Vec<PcBox>,
    /// The game's box count (`SpecialIds::pc_boxes`).
    box_count: u8,
    current_box: u8,
    digest: u64,
}

const fn item_term(i: usize, s: Slot) -> u64 {
    if s.qty == 0 {
        0
    } else {
        splitmix64(5 << 48 | (i as u64) << 32 | (s.item.0 as u64) << 16 | s.qty as u64)
    }
}

fn mon_term(b: usize, slot: usize, m: &Option<BoxedMon>) -> u64 {
    match m {
        None => 0,
        Some(m) => {
            let mut h = Fnv64::new();
            h.write(&[6, b as u8, slot as u8]);
            m.hash(&mut h);
            splitmix64(h.finish())
        }
    }
}

impl PcStorage {
    /// An empty storage with `item_slots` item slots and `boxes` boxes
    /// (`SpecialIds::pc_item_slots`, `pc_boxes`).
    pub fn new(item_slots: u8, boxes: u8) -> Self {
        PcStorage {
            items: vec![Slot::default(); item_slots as usize],
            boxes: Vec::new(),
            box_count: boxes,
            current_box: 0,
            digest: 0,
        }
    }

    /// Whether the storage is as `new` left it (the hash and the snapshot leave it out).
    pub fn is_empty(&self) -> bool {
        self.digest == 0 && self.current_box == 0
    }

    pub fn items(&self) -> &[Slot] {
        &self.items
    }

    /// Sets item slot `i` (the one writer of the items). Panics past the slot count.
    pub fn set_item(&mut self, i: usize, slot: Slot) {
        assert!(i < self.items.len(), "pc item slot out of range");
        let slot = if slot.qty == 0 { Slot::default() } else { slot };
        let old = self.items[i];
        self.digest = self
            .digest
            .wrapping_sub(item_term(i, old))
            .wrapping_add(item_term(i, slot));
        self.items[i] = slot;
    }

    pub fn box_count(&self) -> u8 {
        self.box_count
    }

    /// The mon in `slot` of box `b`.
    pub fn mon(&self, b: usize, slot: usize) -> Option<&BoxedMon> {
        self.boxes.get(b)?.get(slot)?.as_ref()
    }

    /// Box `b`'s slots (all `None` for a box never written).
    pub fn pc_box(&self, b: usize) -> Option<&PcBox> {
        self.boxes.get(b)
    }

    /// Sets `slot` of box `b` (the one writer of the boxes). Panics past the box count.
    pub fn set_mon(&mut self, b: usize, slot: usize, mon: Option<BoxedMon>) {
        assert!(
            b < self.box_count as usize && slot < BOX_SLOTS,
            "pc box slot out of range"
        );
        if self.boxes.len() <= b {
            if mon.is_none() {
                return;
            }
            self.boxes.resize(b + 1, [None; BOX_SLOTS]);
        }
        let old = &self.boxes[b][slot];
        self.digest = self
            .digest
            .wrapping_sub(mon_term(b, slot, old))
            .wrapping_add(mon_term(b, slot, &mon));
        self.boxes[b][slot] = mon;
    }

    /// Gen3 `StorageGetCurrentBox`.
    pub fn current_box(&self) -> u8 {
        self.current_box
    }

    pub fn set_current_box(&mut self, b: u8) {
        if b < self.box_count {
            self.current_box = b;
        }
    }

    /// Feeds the digest and the current box into `h`, only when not empty.
    pub fn hash(&self, h: &mut Fnv64) {
        if !self.is_empty() {
            h.write(&[0xA4, self.current_box]);
            h.write(&self.digest.to_le_bytes());
        }
    }

    /// The non-empty item slots, ascending: `(slot, item, qty)`.
    pub fn item_entries(&self) -> impl Iterator<Item = (u8, ItemId, u16)> + '_ {
        (self.items.iter().enumerate())
            .filter(|(_, s)| s.qty > 0)
            .map(|(i, s)| (i as u8, s.item, s.qty))
    }

    /// Whether `qty` fits one slot (Gen3 `MAX_PC_ITEM_CAPACITY`, 999).
    pub const fn qty_valid(qty: u16) -> bool {
        qty > 0 && qty <= MAX_PC_ITEM_CAPACITY
    }

    /// The boxes written so far (trailing empty boxes may be absent).
    pub fn boxes(&self) -> &[PcBox] {
        &self.boxes
    }
}

/// Gen3 `MAX_PC_ITEM_CAPACITY`.
pub const MAX_PC_ITEM_CAPACITY: u16 = 999;
const _: () = assert!(MAX_PER_SLOT <= MAX_PC_ITEM_CAPACITY);

#[cfg(test)]
mod tests;
