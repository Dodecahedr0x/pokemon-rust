//! The in-memory node graph and the seeded RNG (Task D).
//!
//! The graph mirrors the db's `node` table: every row (any status), the live node of every
//! abstract key (`best`), and how often each key was reached again after its first node
//! (`key_hits`, the selection decay). Everything is a `BTreeMap`, so iteration is in id or key
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
    key_hits: BTreeMap<AbstractKey, u32>,
    /// Rows (any status) per key.
    key_rows: BTreeMap<AbstractKey, u32>,
    /// Rows (any status) whose parent is the key.
    children: BTreeMap<NodeId, u32>,
    next_id: NodeId,
}

impl Graph {
    /// Rebuilds the graph from db rows (any order). `best` holds the live (valid or stale) node
    /// of each key, `key_hits` the rows of each key beyond the first, and `next_id` is the
    /// largest id + 1 (1 for an empty graph, so the root is node 1).
    ///
    /// Duplicate merges (a key reached again without a row) are not in the rows: restore their
    /// hits with [`Graph::set_key_hits`].
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

    /// How often `k` was reached again after its first node (0 for a fresh or unknown key).
    pub fn key_hits(&self, k: AbstractKey) -> u32 {
        self.key_hits.get(&k).copied().unwrap_or(0)
    }

    /// Every key's hits, key ascending (to persist them next to the rows).
    pub fn all_key_hits(&self) -> &BTreeMap<AbstractKey, u32> {
        &self.key_hits
    }

    /// Overrides a key's hits (restoring persisted duplicate counts after `from_rows`).
    pub fn set_key_hits(&mut self, k: AbstractKey, hits: u32) {
        if hits == 0 {
            self.key_hits.remove(&k);
        } else {
            self.key_hits.insert(k, hits);
        }
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
        let rows = self.key_rows.entry(key).or_default();
        if *rows > 0 {
            *self.key_hits.entry(key).or_default() += 1;
        }
        *rows += 1;
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

    /// A key reached again without a new row.
    pub(crate) fn hit(&mut self, k: AbstractKey) {
        *self.key_hits.entry(k).or_default() += 1;
    }
}
