//! Tournament selection (Task D).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::NodeId;

/// `cfg.tournament` uniform picks from the frontier; the best `selection_score` wins, lower id
/// on ties.
pub fn tournament(g: &Graph, rng: &mut Rng64, generation: u32, cfg: &Config) -> Option<NodeId> {
    todo!("Task D")
}
