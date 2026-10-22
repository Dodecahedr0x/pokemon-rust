//! The checkpoint tree on disk (Milestone 5 Task D):
//!
//! ```text
//! nodes/c00001.ron       Node { meta, trace }            committed (no ROM data)
//! snapshots/c00001.ron   Checkpoint (RON, pretty)        git-ignored cache (ROM-derived nicknames)
//! status.ron             BTreeMap<CheckpointId, Status>  git-ignored, written by verify
//! ```

use crate::trace::{CheckpointId, HashMismatch, ReplayReport, Trace};
use engine::checkpoint::{BuildHash, Checkpoint};
use engine::data::MapId;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NodeMeta {
    pub id: CheckpointId,
    pub label: String,
    pub schema: u16,
    pub build: BuildHash,
    pub frame: u32,
    pub map: MapId,
    /// `state_digest` of the node's snapshot.
    pub digest: u64,
    pub world_hash: u64,
}

/// `nodes/c00042.ron` (committed).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Node {
    pub meta: NodeMeta,
    pub trace: Trace,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Status {
    Valid,
    Stale {
        schema: u16,
    },
    Diverged {
        hash: Option<HashMismatch>,
        parent_diverged: bool,
    },
    Broken(String),
}

/// A node's state rebuilt by replaying its trace.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Replayed {
    pub checkpoint: Checkpoint,
    pub report: ReplayReport,
}

/// A checkpoint tree rooted at a directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// `$PKR_CHECKPOINTS`, else `<workspace>/checkpoints/{stub|emerald}`.
    pub fn default_root(stub: bool) -> PathBuf {
        let _ = stub;
        todo!("Milestone 5 Task D")
    }

    pub fn open(root: impl Into<PathBuf>) -> io::Result<Store> {
        let _ = root.into();
        todo!("Milestone 5 Task D")
    }

    /// Every node, by id.
    pub fn nodes(&self) -> io::Result<Vec<Node>> {
        todo!("Milestone 5 Task D")
    }

    /// By `c00012`, `12` or label.
    pub fn find(&self, id_or_label: &str) -> io::Result<Node> {
        let _ = id_or_label;
        todo!("Milestone 5 Task D")
    }

    pub fn add(&self, label: &str, trace: Trace, cp: &Checkpoint) -> io::Result<CheckpointId> {
        let _ = (label, trace, cp);
        todo!("Milestone 5 Task D")
    }

    pub fn relabel(&self, id: CheckpointId, label: &str) -> io::Result<()> {
        let _ = (id, label);
        todo!("Milestone 5 Task D")
    }

    /// Refuses a node with children.
    pub fn remove(&self, id: CheckpointId) -> io::Result<()> {
        let _ = id;
        todo!("Milestone 5 Task D")
    }

    /// The cached snapshot; `None` if absent or of another schema.
    pub fn snapshot(&self, id: CheckpointId) -> io::Result<Option<Checkpoint>> {
        let _ = id;
        todo!("Milestone 5 Task D")
    }

    pub fn put_snapshot(&self, id: CheckpointId, cp: &Checkpoint) -> io::Result<()> {
        let _ = (id, cp);
        todo!("Milestone 5 Task D")
    }

    /// Parents first; a missing parent or a cycle is an error.
    pub fn topo_order(&self) -> io::Result<Vec<Node>> {
        todo!("Milestone 5 Task D")
    }

    /// Replays every node in topological order and classifies it; also writes `status.ron`.
    pub fn verify(
        &self,
        replay: impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String>,
        accept: bool,
    ) -> io::Result<Vec<(CheckpointId, Status)>> {
        let _ = (replay, accept, &self.root);
        todo!("Milestone 5 Task D")
    }
}
