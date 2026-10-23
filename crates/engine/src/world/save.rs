//! The in-game save (Gen3 `SaveGame` → `TrySavingData`; Milestone 9 Task S owns the file).
//!
//! Task S: [`save_now`] takes the state without the capture checks
//! (`World::snapshot_unchecked`, `world/checkpoint.rs`), normalises that snapshot to "field
//! idle" (the runner dropped, the text box closed, fades and screen effects cleared, objects
//! unfrozen, movement scripts ended on their destination cells) and checks that it restores
//! and captures as it is. It is called by `world/screens.rs` on `ScreenOut::Save` and by
//! [`World::save_snapshot`]. The shells persist the checkpoint on `Event::Saved { ok: true }`.
//!
//! Wave 0: the stub answers `Err(SaveError::Unsupported)`.

use super::{State, World};
use crate::checkpoint::{Checkpoint, SaveError};
use crate::data::GameData;

/// Gen3 `TrySavingData` on a normalised copy of the world (Task S). It takes the World, not
/// only its `State`, because a capture needs the frame and the runner too.
pub(super) fn save_now(w: &World, game: &impl GameData) -> Result<Checkpoint, SaveError> {
    let _ = (w, game);
    Err(SaveError::Unsupported)
}

/// The shell could not write the save (Gen3's "Save error" path, `SaveErrorTimer`): the
/// dialog answers "not saved" on the next frame (Task S).
pub(super) fn save_failed(s: &mut State) {
    let _ = s;
}
