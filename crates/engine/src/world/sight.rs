//! Trainer sight (owned by Milestone 8 Task I): after the player's step-end triggers, a trainer
//! whose line of sight holds the player walks up and starts its script (Gen3
//! `CheckForTrainersWantingBattle`, `trainer_see.c`).

use super::{State, World};
use crate::data::{GameData, ScriptId};

/// A trainer that saw the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Sighting {
    /// Local id of the trainer object.
    pub obj: u8,
    /// Its object script.
    pub script: ScriptId,
}

/// The trainer that sees the player now, if any. Wave 0: none.
pub(super) fn check(game: &impl GameData, s: &State) -> Option<Sighting> {
    let _ = (game, s);
    None
}

impl World {
    /// Starts a sighting: the trainer's object script, as if the player had talked to it.
    pub(super) fn start_sighting(&mut self, game: &impl GameData, sighting: Sighting) {
        let facing = self.s.objects.player().facing;
        self.start_script(game, sighting.script, Some((sighting.obj, facing, true)));
    }
}
