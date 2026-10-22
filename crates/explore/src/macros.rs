//! Macro-action candidates of a capturable state (Task A).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::model::Macro;
use crate::path::Reach;
use engine::checkpoint::WorldSnapshot;
use engine::data::GameData;

/// Every applicable macro of a capturable state, deterministic order, deduplicated, default
/// `Opts`. Order: Enter, Cross, Talk, Inspect, Grass, then WalkTo (coord-event cells first,
/// then one cell per coarse block, nearest its centre).
pub fn candidates<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach) -> Vec<Macro> {
    todo!("Task A")
}

/// Whether `m` is still applicable to `snap` (mutations replaying a sibling's macro).
pub fn applicable<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach, m: &Macro) -> bool {
    todo!("Task A")
}
