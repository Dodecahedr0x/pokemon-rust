//! Capture and restore of a [`World`] as a [`Checkpoint`] (Milestone 5 Task B).
//!
//! Capture needs an idle field (see [`World::capturable`]); everything else is reached by
//! replaying a trace. Restore rebuilds the semantic state, then builds a boot frame that
//! re-uploads every VRAM byte the state implies without mutating hashed state.

use super::World;
use crate::checkpoint::{Checkpoint, NotCapturable, RestoreError, WorldSnapshot};
use crate::data::GameData;

impl World {
    /// `Ok` when the world can be captured: field mode, no transition, no script or pending
    /// answer, a pristine text box, no fade, and no object running a movement script (checked
    /// in that order).
    pub fn capturable(&self) -> Result<(), NotCapturable> {
        todo!("Milestone 5 Task B")
    }

    /// The semantic state, if [`World::capturable`].
    pub fn snapshot(&self) -> Result<WorldSnapshot, NotCapturable> {
        todo!("Milestone 5 Task B")
    }

    /// [`World::snapshot`] with its header (schema, build, frame, hash).
    pub fn checkpoint(&self, game: &impl GameData) -> Result<Checkpoint, NotCapturable> {
        let _ = game;
        todo!("Milestone 5 Task B")
    }

    /// Rebuilds the state; `initial_frame()` then returns every upload the state implies
    /// (applied with the screen off, like `World::new`'s boot frame). Same build: also checks
    /// `hash() == header.world_hash` (`HashMismatch`).
    pub fn restore(game: &impl GameData, cp: &Checkpoint) -> Result<World, RestoreError> {
        let _ = (game, cp);
        todo!("Milestone 5 Task B")
    }
}
