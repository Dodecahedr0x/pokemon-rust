//! The current map's object templates for this visit (scripts move them and change their
//! movement type), with a digest kept up to date on every write, as [`crate::flow::Vars`] and
//! [`crate::bag::Bag`]: the per-frame world hash writes 12 bytes instead of 7 bytes and a
//! movement-type match per template. Also remembers whether any template is a trainer, so trainer
//! sight skips maps without one.

use crate::data::ObjectEvent;
use crate::hash::{Fnv64, splitmix64};
use crate::objects::movement_code;
use alloc::vec::Vec;
use core::ops::Deref;

use super::sight::{TRAINER_TYPE_BURIED, TRAINER_TYPE_NORMAL};

#[derive(Clone, Default)]
pub(crate) struct Templates {
    list: Vec<ObjectEvent>,
    /// Wrapping sum of [`term`] over `list`.
    digest: u64,
    /// Some template is a trainer that looks (`TRAINER_TYPE_NORMAL` or `_BURIED`).
    trainers: bool,
}

/// Template `i`'s share of the digest: what the hash used to write per template.
fn term(i: usize, t: &ObjectEvent) -> u64 {
    let [m, arg] = movement_code(t.movement);
    splitmix64(
        (i as u64) << 56
            | (t.local_id as u64) << 48
            | (t.x as u16 as u64) << 32
            | (t.y as u16 as u64) << 16
            | (m as u64) << 8
            | arg as u64,
    )
}

const fn looks(t: &ObjectEvent) -> bool {
    t.trainer_type == TRAINER_TYPE_NORMAL || t.trainer_type == TRAINER_TYPE_BURIED
}

impl Templates {
    pub fn new(list: Vec<ObjectEvent>) -> Self {
        let mut t = Templates {
            list,
            digest: 0,
            trainers: false,
        };
        t.digest = t.full_digest();
        t.trainers = t.list.iter().any(looks);
        t
    }

    fn full_digest(&self) -> u64 {
        (self.list.iter().enumerate()).fold(0u64, |d, (i, t)| d.wrapping_add(term(i, t)))
    }

    /// Edits template `local` with `f`, keeping the digest (the one writer of templates).
    /// Returns whether the template exists.
    pub fn edit(&mut self, local: u8, f: impl FnOnce(&mut ObjectEvent)) -> bool {
        let Some(i) = self.list.iter().position(|t| t.local_id == local) else {
            return false;
        };
        let t = &mut self.list[i];
        self.digest = self.digest.wrapping_sub(term(i, t));
        f(t);
        self.digest = self.digest.wrapping_add(term(i, t));
        self.trainers = self.list.iter().any(looks);
        true
    }

    /// Whether some template is a trainer that looks for the player.
    pub fn has_trainers(&self) -> bool {
        self.trainers
    }

    /// The count and the digest (O(1)).
    pub fn hash(&self, h: &mut Fnv64) {
        debug_assert_eq!(self.digest, self.full_digest());
        h.write_u32(self.list.len() as u32);
        h.write(&self.digest.to_le_bytes());
    }
}

impl Deref for Templates {
    type Target = [ObjectEvent];
    fn deref(&self) -> &[ObjectEvent] {
        &self.list
    }
}
