//! Decoration PC inventory (Gen3 `DecorationAdd` in `decoration_inventory.c`).
//!
//! Eight categories, in save-block order: desks, chairs, plants, ornaments, mats, posters,
//! dolls, cushions. `add` puts the id in the first empty slot of its category. Id 0
//! (`DECOR_NONE`) is refused. Duplicates are kept until the category is full.
//!
//! Live only. It is not snapshotted and not part of [`crate::world::World::hash`]: a save
//! drops it, and hashing it without a snapshot would make that save fail to restore.

use crate::data::DecorId;
use alloc::vec;
use alloc::vec::Vec;

/// Slots per category (`include/global.h` `SaveBlock1`).
const SLOTS: [usize; 8] = [10, 10, 10, 30, 30, 10, 40, 10];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DecorPc {
    slots: [Vec<DecorId>; 8],
}

impl DecorPc {
    pub fn new() -> Self {
        Self {
            slots: SLOTS.map(|n| vec![DecorId(0); n]),
        }
    }

    /// Gen3 `DecorationAdd`: false for id 0, a bad category, or a full category.
    pub fn add(&mut self, id: DecorId, category: u8) -> bool {
        if id.0 == 0 {
            return false;
        }
        let Some(slots) = self.slots.get_mut(usize::from(category)) else {
            return false;
        };
        let Some(slot) = slots.iter_mut().find(|s| s.0 == 0) else {
            return false;
        };
        *slot = id;
        true
    }

    pub fn count(&self, id: DecorId) -> u16 {
        self.slots.iter().flatten().filter(|s| **s == id).count() as u16
    }
}
