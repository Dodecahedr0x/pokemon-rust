//! Virtual objects (Wave C): sprites that look like objects but are none (Gen3
//! `CreateVirtualObject` / `TurnVirtualObject`, `createvobject` / `turnvobject`): the contest
//! and Battle Dome audiences. They stand still on a cell facing a direction, never move,
//! collide or run scripts, and last until the next warp load (Gen3 resets the sprites). Not
//! hashed or snapshotted (sprites only; TODO with the checkpoint schema 5).

use super::Objects;
use crate::data::{Dir, GfxId};

/// Gen3 `MAX_SPRITES`: at most this many at once.
pub const VIRTUAL_MAX: usize = 64;

/// One virtual object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VirtualObject {
    /// The script's id (Gen3 `sVirtualObjId`), not a local id.
    pub id: u8,
    pub gfx: GfxId,
    /// Map cell.
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub facing: Dir,
    /// OBJ VRAM tiles (first, count) and what they hold, as an object's.
    pub(super) vram: Option<(u16, u16)>,
    pub(super) uploaded: Option<(usize, u16)>,
}

impl Objects {
    /// Gen3 `CreateVirtualObject`: a new sprite, even when `id` is taken (Gen3 then finds the
    /// first one by id). Ignored past [`VIRTUAL_MAX`].
    pub fn create_virtual(
        &mut self,
        gfx: GfxId,
        id: u8,
        x: i16,
        y: i16,
        elevation: u8,
        facing: Dir,
    ) {
        if self.virtuals.len() < VIRTUAL_MAX {
            self.virtuals.push(VirtualObject {
                id,
                gfx,
                x,
                y,
                elevation,
                facing,
                vram: None,
                uploaded: None,
            });
        }
    }

    /// Gen3 `TurnVirtualObject`: the first virtual object with `id` faces `dir`.
    pub fn turn_virtual(&mut self, id: u8, dir: Dir) {
        if let Some(v) = self.virtuals.iter_mut().find(|v| v.id == id) {
            v.facing = dir;
        }
    }

    /// The virtual objects, in creation order.
    pub fn virtuals(&self) -> &[VirtualObject] {
        &self.virtuals
    }
}
