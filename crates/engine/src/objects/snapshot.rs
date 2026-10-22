//! [`Objects`] to and from its checkpoint DTO (Milestone 5 Task B).

use super::Objects;
use crate::checkpoint::ObjectsSnap;

// Wave 0 stubs: `World::snapshot`/`restore` wire them in Task B.
#[allow(dead_code)]
impl Objects {
    pub(crate) fn snapshot(&self) -> ObjectsSnap {
        todo!("Milestone 5 Task B")
    }

    /// `vram`, `uploaded` and `banks` start empty; the boot frame's `draw` refills them.
    pub(crate) fn from_snapshot(s: &ObjectsSnap) -> Objects {
        let _ = s;
        todo!("Milestone 5 Task B")
    }

    /// First object (local id) with a live movement script.
    pub fn any_moves(&self) -> Option<u8> {
        todo!("Milestone 5 Task B")
    }
}
