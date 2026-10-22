//! The in-memory node graph and the seeded RNG (Task D).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::model::{AbstractKey, NodeId, NodeRow};
use std::collections::BTreeMap;

/// splitmix64.
pub struct Rng64(pub u64);

impl Rng64 {
    pub fn next_u64(&mut self) -> u64 {
        todo!("Task D")
    }
    /// Uniform in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        todo!("Task D")
    }
}

pub struct Graph {
    nodes: BTreeMap<NodeId, NodeRow>,
    best: BTreeMap<AbstractKey, NodeId>,
    key_hits: BTreeMap<AbstractKey, u32>,
    next_id: NodeId,
}

impl Graph {
    pub fn from_rows(rows: Vec<NodeRow>) -> Graph {
        todo!("Task D")
    }
    pub fn node(&self, id: NodeId) -> Option<&NodeRow> {
        todo!("Task D")
    }
    /// Valid, not merged, id ascending.
    pub fn frontier(&self) -> impl Iterator<Item = &NodeRow> {
        todo!("Task D");
        #[allow(unreachable_code)]
        std::iter::empty()
    }
    pub fn best(&self, k: AbstractKey) -> Option<NodeId> {
        todo!("Task D")
    }
    pub fn len(&self) -> usize {
        todo!("Task D")
    }
    pub fn is_empty(&self) -> bool {
        todo!("Task D")
    }
}
