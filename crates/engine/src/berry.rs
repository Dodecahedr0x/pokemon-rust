//! Berry trees (Gen3 `gSaveBlock1Ptr->berryTrees`, `struct BerryTree`; Wave E): saved
//! per-tree state, read and written by the game's scripts through
//! [`Request::BerryTree`](crate::script::Request::BerryTree) and
//! [`Request::SetBerryTree`](crate::script::Request::SetBerryTree). The engine only stores,
//! hashes and snapshots them: planting, picking, watering and the yields are the game's
//! (Gen3 `berry.c`, in the content). A game without berry trees has none
//! (`SpecialIds::berry_trees` 0).
//!
//! Not modelled: growth over time (Gen3 `BerryTreeTimeUpdate` from the RTC minutes): a tree
//! stays at the stage it was set to.

use crate::hash::Fnv64;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

/// One tree (Gen3 `struct BerryTree`). A blank tree (`berry` 0, every field 0) has no berry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct BerryTree {
    /// Gen3 berry type (`ITEM_TO_BERRY`), 0 for none.
    pub berry: u8,
    /// `BERRY_STAGE_*`.
    pub stage: u8,
    /// Growth stopped until the player has seen the tree (`stopGrowth`).
    pub stop_growth: bool,
    pub minutes_until_next_stage: u16,
    pub berry_yield: u8,
    pub regrowth_count: u8,
    /// `watered1..4` as bits 0..3.
    pub watered: u8,
}

impl BerryTree {
    pub fn is_blank(&self) -> bool {
        *self == BerryTree::default()
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.berry,
            self.stage,
            self.stop_growth as u8,
            self.berry_yield,
            self.regrowth_count,
            self.watered,
        ]);
        h.write_u16(self.minutes_until_next_stage);
    }
}

/// Every tree of the game, by Gen3 tree id.
#[derive(Clone, Debug, Default)]
pub struct BerryTrees {
    trees: Vec<BerryTree>,
    /// The trees' digest for [`BerryTrees::hash`] (`Some(None)`: all blank), cleared by
    /// [`BerryTrees::set`]: the world hash runs every frame, and on the GBA hashing ~90 trees
    /// byte by byte cost about 4 scanlines over the `World::hash` gate.
    digest: Cell<Option<Option<u64>>>,
}

impl PartialEq for BerryTrees {
    fn eq(&self, other: &Self) -> bool {
        self.trees == other.trees
    }
}

impl Eq for BerryTrees {}

impl BerryTrees {
    /// `count` blank trees (`SpecialIds::berry_trees`).
    pub fn new(count: u8) -> Self {
        BerryTrees {
            trees: vec![BerryTree::default(); count as usize],
            digest: Cell::new(None),
        }
    }

    /// Tree `id`; a blank tree for an id the game does not have.
    pub fn get(&self, id: u8) -> BerryTree {
        self.trees.get(id as usize).copied().unwrap_or_default()
    }

    /// Sets tree `id`; nothing for an id the game does not have.
    pub fn set(&mut self, id: u8, tree: BerryTree) {
        if let Some(t) = self.trees.get_mut(id as usize) {
            *t = tree;
            self.digest.set(None);
        }
    }

    /// The non-blank trees, ascending by id.
    pub fn entries(&self) -> impl Iterator<Item = (u8, BerryTree)> + '_ {
        (self.trees.iter().enumerate())
            .filter(|(_, t)| !t.is_blank())
            .map(|(i, t)| (i as u8, *t))
    }

    /// Writes nothing while every tree is blank (so worlds without trees keep their hashes).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        let digest = self.digest.get().unwrap_or_else(|| {
            let mut d = Fnv64::new();
            let mut any = false;
            for (id, t) in self.entries() {
                any = true;
                d.write(&[0xB7, id]);
                t.hash(&mut d);
            }
            let digest = any.then(|| d.finish());
            self.digest.set(Some(digest));
            digest
        });
        if let Some(d) = digest {
            h.write(&[0xB7]);
            h.write(&d.to_le_bytes());
        }
    }

    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}

#[cfg(test)]
mod tests;
