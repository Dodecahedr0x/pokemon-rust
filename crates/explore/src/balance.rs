//! Battle records (Task F). Balance is recorded, weight 0 in v0.
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::model::BattleSummary;
use engine::World;

/// Tracks one battle from its first frame to the field's return.
pub struct BattleTracker {
    start_frame: u32,
    player_level: u8,
    opponent_level: u8,
    hp_start: u16,
}

impl BattleTracker {
    pub fn start(w: &World) -> Option<Self> {
        todo!("Task F")
    }
    pub fn finish(self, w: &World, outcome: Option<u8>) -> BattleSummary {
        todo!("Task F")
    }
}
