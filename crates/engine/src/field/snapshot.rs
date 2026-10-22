//! [`Field`] to and from its checkpoint DTO, and the restore upload (Milestone 5 Task B).

use super::Field;
use crate::checkpoint::FieldSnap;
use crate::data::GameData;
use crate::video::Frame;

// Wave 0 stubs: `World::snapshot`/`restore` wire them in Task B.
#[allow(dead_code)]
impl Field {
    pub(crate) fn snapshot(&self) -> FieldSnap {
        todo!("Milestone 5 Task B")
    }

    pub(crate) fn from_snapshot(s: &FieldSnap) -> Field {
        let _ = s;
        todo!("Milestone 5 Task B")
    }

    /// BG1–3 controls, both tilesets' tiles and palettes, every cell of `drawn`, the scroll.
    /// Mutates nothing (the hash is unchanged).
    pub fn upload_all(&self, game: &impl GameData, f: &mut Frame) {
        let _ = (game, f);
        todo!("Milestone 5 Task B")
    }
}
