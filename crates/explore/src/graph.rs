//! The in-memory node graph and the seeded RNG (Task D).
//!
//! The graph mirrors the db's `node` table: every row (any status), the live node of every
//! abstract key (`best`), and the rows per key (`key_hits` = rows - 1, the selection decay).
//! It is a pure function of the rows, so a run resumed from the db has the same graph. Everything is a `BTreeMap`, so iteration is in id or key
//! order and never depends on hashing.

use crate::model::{AbstractKey, NodeId, NodeRow, NodeStatus};
use std::collections::BTreeMap;

/// splitmix64.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng64(pub u64);

impl Rng64 {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in `0..n` (`0` when `n == 0`). Rejection sampling, so there is no modulo bias.
    pub fn below(&mut self, n: u64) -> u64 {
        if n <= 1 {
            return 0;
        }
        // The largest multiple of n that fits: values at or above it are redrawn.
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }
}

/// The splitmix64 finalizer: a stable, platform-independent 64-bit mix.
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Whether a status holds its key in the db's `node_live_key` index.
pub fn is_live(s: NodeStatus) -> bool {
    matches!(s, NodeStatus::Valid | NodeStatus::Stale)
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Graph {
    nodes: BTreeMap<NodeId, NodeRow>,
    best: BTreeMap<AbstractKey, NodeId>,
    /// Rows (any status) per key.
    key_rows: BTreeMap<AbstractKey, u32>,
    /// Rows (any status) whose parent is the key.
    children: BTreeMap<NodeId, u32>,
    next_id: NodeId,
}

impl Graph {
    /// Rebuilds the graph from db rows (any order). `best` holds the live (valid or stale) node
    /// of each key, the rows per key, and `next_id` is the largest id + 1 (1 for an empty
    /// graph, so the root is node 1).
    pub fn from_rows(rows: Vec<NodeRow>) -> Graph {
        let mut g = Graph {
            next_id: 1,
            ..Graph::default()
        };
        for row in rows {
            g.insert(row);
        }
        g
    }

    pub fn node(&self, id: NodeId) -> Option<&NodeRow> {
        self.nodes.get(&id)
    }

    /// Valid, not merged, id ascending.
    pub fn frontier(&self) -> impl Iterator<Item = &NodeRow> {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Valid && n.merged_into.is_none())
    }

    pub fn best(&self, k: AbstractKey) -> Option<NodeId> {
        self.best.get(&k).copied()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every row, id ascending.
    pub fn nodes(&self) -> impl Iterator<Item = &NodeRow> {
        self.nodes.values()
    }

    /// How often `k` was reached again after its first node: its rows (any status) minus one,
    /// so `from_rows` rebuilds it exactly (0 for a fresh or unknown key). Duplicates that leave
    /// no row do not count.
    pub fn key_hits(&self, k: AbstractKey) -> u32 {
        self.key_rows.get(&k).map_or(0, |n| n.saturating_sub(1))
    }

    /// The number of rows (any status) whose parent is `id`.
    pub fn children(&self, id: NodeId) -> u32 {
        self.children.get(&id).copied().unwrap_or(0)
    }

    /// The id the next inserted node gets.
    pub fn next_id(&self) -> NodeId {
        self.next_id
    }

    /// `id`'s ancestor `up` levels up, stopping at the root (`up == 0` is `id`).
    pub fn ancestor(&self, id: NodeId, up: u32) -> NodeId {
        let mut at = id;
        for _ in 0..up {
            match self.nodes.get(&at).and_then(|n| n.parent) {
                Some(p) => at = p,
                None => break,
            }
        }
        at
    }

    /// Adds a row as it is (the merge assigns ids from [`Graph::next_id`]).
    pub(crate) fn insert(&mut self, row: NodeRow) {
        let (id, key) = (row.id, row.key);
        *self.key_rows.entry(key).or_default() += 1;
        if is_live(row.status) {
            self.best.insert(key, id);
        }
        if let Some(p) = row.parent {
            *self.children.entry(p).or_default() += 1;
        }
        self.next_id = self.next_id.max(id + 1);
        self.nodes.insert(id, row);
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> Option<&mut NodeRow> {
        self.nodes.get_mut(&id)
    }

    /// `old` loses its key to `new` (`old -> Merged`).
    pub(crate) fn supersede(&mut self, old: NodeId, new: NodeId) {
        if let Some(n) = self.nodes.get_mut(&old) {
            n.status = NodeStatus::Merged;
            n.merged_into = Some(new);
            if self.best.get(&n.key) == Some(&old) {
                self.best.remove(&n.key);
            }
        }
    }
}
