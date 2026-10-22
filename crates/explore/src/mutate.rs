//! Mutation operators (Task D).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::{NodeId, Plan};

pub enum MutationOp {
    Extend,
    ReplaceSuffix { up: u8 },
    FlipOpts,
    Probe,
}

/// The job's start node (`None` = a new game) and plan for the selected node `sel`.
pub fn mutate(g: &Graph, rng: &mut Rng64, sel: NodeId, cfg: &Config) -> (Option<NodeId>, Plan) {
    todo!("Task D")
}
