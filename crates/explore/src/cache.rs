//! Snapshot cache and state reach by replay (Task C).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::db::{Db, DbError};
use crate::model::NodeId;
use engine::checkpoint::Checkpoint;
use engine::data::GameData;
use std::collections::BTreeMap;
use std::sync::Arc;

/// In-memory LRU (by tick) of node end states.
pub struct SnapCache {
    cap: usize,
    tick: u64,
    map: BTreeMap<NodeId, (Arc<Checkpoint>, u64)>,
}

impl SnapCache {
    pub fn new(cap: usize) -> Self {
        todo!("Task C")
    }
    pub fn get(&mut self, id: NodeId) -> Option<Arc<Checkpoint>> {
        todo!("Task C")
    }
    pub fn put(&mut self, id: NodeId, cp: Arc<Checkpoint>) {
        todo!("Task C")
    }
}

#[derive(Debug)]
pub enum ReachError {
    Db(DbError),
    Missing(NodeId),
    Restore(String),
    Diverged { node: NodeId, want: u64, got: u64 },
}

/// Node `id`'s end state: the nearest ancestor in the cache, then in the db (same build), else
/// a new game; replays the inputs down the lineage, checks each digest, caches the result.
pub fn reach<G: GameData + Clone>(
    game: &G,
    db: &Db,
    cache: &mut SnapCache,
    id: NodeId,
) -> Result<Arc<Checkpoint>, ReachError> {
    todo!("Task C")
}
