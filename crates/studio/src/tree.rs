//! The checkpoint tree view's model (Milestone 6 Task B): one layered [`Store`] (the committed
//! tree as a read-only base, personal saves in the local root; a local node may start from a
//! base node), read through [`Store::index`] and `status.ron`.
//!
//! [`Origin`] is derived per node ([`Index::is_base`]): it is what the view shows and what it
//! may change. Base nodes are read-only from the studio ([`StoreError::ReadOnly`]); the
//! committed tree changes only through `cargo xtask checkpoints` (decisions 7, 8 and 14).

use content::Emerald;
use engine::data::{GameData, MapId};
use game_native::store::{Index, Status, Store, StoreError};
use game_native::trace::{CheckpointId, Parent};
use std::collections::BTreeMap;

/// Where a node lives.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Origin {
    /// The committed tree (the layered store's read-only base).
    Shared,
    /// The personal, git-ignored local root.
    Local,
}

impl Origin {
    pub fn of(index: &Index, id: CheckpointId) -> Origin {
        if index.is_base(id) {
            Origin::Shared
        } else {
            Origin::Local
        }
    }
}

/// One line of the tree view.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    pub origin: Origin,
    pub id: CheckpointId,
    pub label: String,
    /// 0 for a node starting from a new game.
    pub depth: u16,
    pub parent: Parent,
    pub frame: u32,
    pub trace_frames: u32,
    pub map: MapId,
    pub map_name: &'static str,
    /// The last verdict (`status.ron`), if any.
    pub status: Option<Status>,
    pub children: u32,
}

impl TreeRow {
    /// The studio relabels local nodes only (the committed tree is read-only).
    pub fn can_label(&self) -> bool {
        self.origin == Origin::Local
    }

    /// The studio removes local nodes without children only.
    pub fn can_remove(&self) -> bool {
        self.can_label() && self.children == 0
    }
}

/// The rows in display order: depth first from the new-game roots, siblings shared before
/// local, then by id. So the shared nodes come first, and a local node starting from a shared
/// one follows that node's shared children.
#[derive(Clone, Debug, Default)]
pub struct TreeModel {
    rows: Vec<TreeRow>,
    at: BTreeMap<CheckpointId, usize>,
}

impl TreeModel {
    /// The store's index and statuses.
    pub fn load(store: &Store) -> Result<TreeModel, StoreError> {
        let index = store.index()?;
        // A corrupt status.ron is a cache to rebuild (the next verify replaces it), not an
        // error here; the CLI stays strict.
        let statuses = match store.statuses() {
            Err(e) if e.is_corrupt() => BTreeMap::new(),
            r => r?,
        };
        Ok(Self::build(&index, &statuses))
    }

    fn build(index: &Index, statuses: &BTreeMap<CheckpointId, Status>) -> TreeModel {
        let game = Emerald::for_shells();
        let mut model = TreeModel::default();
        // The children of `p`, last row first: shared before local, then by id.
        let pending = |p: Parent| {
            let mut ids = index.children(p).to_vec();
            ids.sort_by_key(|&id| std::cmp::Reverse((Origin::of(index, id), id)));
            ids
        };
        // (id, depth), the next row on top.
        let mut stack: Vec<(CheckpointId, u16)> = pending(Parent::NewGame)
            .into_iter()
            .map(|id| (id, 0))
            .collect();
        while let Some((id, depth)) = stack.pop() {
            let node = index.node(id).expect("indexed");
            let children = index.children(Parent::Checkpoint(id));
            stack.extend(
                pending(Parent::Checkpoint(id))
                    .into_iter()
                    .map(|c| (c, depth + 1)),
            );
            model.at.insert(id, model.rows.len());
            model.rows.push(TreeRow {
                origin: Origin::of(index, id),
                id,
                label: node.meta.label.clone(),
                depth,
                parent: node.trace.parent,
                frame: node.meta.frame,
                trace_frames: node.trace.frames(),
                map: node.meta.map,
                map_name: game.map(node.meta.map).name,
                status: statuses.get(&id).cloned(),
                children: children.len() as u32,
            });
        }
        model
    }

    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    pub fn row(&self, id: CheckpointId) -> Option<&TreeRow> {
        self.at.get(&id).map(|&i| &self.rows[i])
    }

    /// Sets the verdicts of these nodes (a background verify's); unknown ids are ignored.
    pub fn set_statuses(&mut self, verdicts: &[(CheckpointId, Status)]) {
        for (id, status) in verdicts {
            if let Some(&i) = self.at.get(id) {
                self.rows[i].status = Some(status.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_native::checkpoints::{replayer, seed};
    use game_native::scenario::cached_route;
    use game_native::trace::TraceRecorder;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh directory (`std::env::temp_dir()` + pid + counter); a passing test removes it.
    fn temp_root(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pkr-studio-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn kind(e: StoreError) -> &'static str {
        match e {
            StoreError::Label(_) => "Label",
            StoreError::ReadOnly(_) => "ReadOnly",
            StoreError::HasChildren(_) => "HasChildren",
            StoreError::NotFound(_) => "NotFound",
            _ => "other",
        }
    }

    /// The route's first two nodes in a base store, and a layered store over it with two local
    /// zero-frame branches: `x` from the first node and `y` from `x`.
    ///
    /// ```text
    /// r1 (shared) ─ r2 (shared)
    ///             └ x (local) ─ y (local)
    /// ```
    fn layered() -> Option<(PathBuf, PathBuf, Store, [CheckpointId; 4])> {
        if content::STALE_EXTRACT {
            println!("skipped: {}", content::STALE_EXTRACT_MSG);
            return None;
        }
        let game = Emerald::for_shells();
        let (base_root, local_root) = (temp_root("base"), temp_root("local"));
        let mut rec = cached_route().clone();
        rec.nodes.truncate(2);
        let [r1, r2] = seed(&Store::open(&base_root).unwrap(), &game, &rec).unwrap()[..] else {
            panic!("two route nodes")
        };
        let store = Store::layered(&base_root, &local_root).unwrap();
        let build = engine::checkpoint::build_hash(&game);
        let branch = |label: &str, parent: CheckpointId| {
            let p = store.find(&parent.to_string()).unwrap();
            let cp = store.snapshot(parent).unwrap().unwrap();
            let trace = TraceRecorder::new(p.meta.frame).trace(Parent::Checkpoint(parent), build);
            store.add(label, trace, &cp).unwrap()
        };
        let x = branch("x", r1);
        let y = branch("y", x);
        Some((base_root, local_root, store, [r1, r2, x, y]))
    }

    #[test]
    fn rows_are_depth_first_shared_then_local_with_statuses() {
        let Some((base_root, local_root, store, [r1, r2, x, y])) = layered() else {
            return;
        };
        let model = TreeModel::load(&store).unwrap();
        let rows = model.rows();
        let line = |r: &TreeRow| (r.id, r.origin, r.depth, r.children, r.parent);
        assert_eq!(
            rows.iter().map(line).collect::<Vec<_>>(),
            [
                (r1, Origin::Shared, 0, 2, Parent::NewGame),
                (r2, Origin::Shared, 1, 0, Parent::Checkpoint(r1)),
                (x, Origin::Local, 1, 1, Parent::Checkpoint(r1)),
                (y, Origin::Local, 2, 0, Parent::Checkpoint(x)),
            ]
        );
        let n1 = store.find(&r1.to_string()).unwrap();
        let row = model.row(r1).unwrap();
        assert_eq!(row.label, n1.meta.label);
        assert_eq!((row.frame, row.map), (n1.meta.frame, n1.meta.map));
        assert_eq!(row.trace_frames, n1.trace.frames());
        assert!(!row.map_name.is_empty());
        assert_eq!(model.row(x).unwrap().trace_frames, 0);
        assert!(rows.iter().all(|r| r.status.is_none()), "never verified");
        // Statuses: set in place, and read back from status.ron after a verify.
        let mut model = model;
        model.set_statuses(&[
            (r2, Status::Stale { schema: 1 }),
            (CheckpointId(999), Status::Valid),
        ]);
        assert_eq!(
            model.row(r2).unwrap().status,
            Some(Status::Stale { schema: 1 })
        );
        // The studio's Verify (on a worker thread there).
        let verdicts = store
            .verify(replayer(&Emerald::for_shells()), false)
            .unwrap();
        assert_eq!(verdicts.len(), 4);
        assert!(
            Status::healthy(verdicts.iter().map(|(_, s)| s)),
            "{verdicts:?}"
        );
        let model = TreeModel::load(&store).unwrap();
        assert!(model.rows().iter().all(|r| r.status == Some(Status::Valid)));
        assert!(
            !base_root.join("status.ron").exists(),
            "the base is not written"
        );
        // A corrupt status.ron reads as no verdicts here (the CLI refuses it).
        std::fs::write(local_root.join("status.ron"), "garbage").unwrap();
        assert!(store.statuses().is_err());
        let model = TreeModel::load(&store).unwrap();
        assert!(model.rows().iter().all(|r| r.status.is_none()));
        std::fs::remove_dir_all(base_root).unwrap();
        std::fs::remove_dir_all(local_root).unwrap();
    }

    #[test]
    fn relabel_and_remove_refuse_shared_nodes_taken_labels_and_parents() {
        let Some((base_root, local_root, store, [r1, r2, x, y])) = layered() else {
            return;
        };
        // What the view offers: label local nodes, remove local leaves.
        let model = TreeModel::load(&store).unwrap();
        let can = |id| {
            let r = model.row(id).unwrap();
            (r.can_label(), r.can_remove())
        };
        assert_eq!(
            [r1, r2, x, y].map(can),
            [(false, false), (false, false), (true, false), (true, true)]
        );
        assert_eq!(kind(store.relabel(x, "y").unwrap_err()), "Label");
        let r2_label = store.find(&r2.to_string()).unwrap().meta.label;
        assert_eq!(kind(store.relabel(x, &r2_label).unwrap_err()), "Label");
        assert_eq!(kind(store.relabel(r1, "mine").unwrap_err()), "ReadOnly");
        store.relabel(x, "x2").unwrap();
        assert_eq!(TreeModel::load(&store).unwrap().row(x).unwrap().label, "x2");
        assert_eq!(kind(store.remove(x).unwrap_err()), "HasChildren");
        assert_eq!(kind(store.remove(r2).unwrap_err()), "ReadOnly");
        store.remove(y).unwrap();
        store.remove(x).unwrap();
        assert_eq!(kind(store.remove(x).unwrap_err()), "NotFound");
        assert_eq!(TreeModel::load(&store).unwrap().rows().len(), 2);
        std::fs::remove_dir_all(base_root).unwrap();
        std::fs::remove_dir_all(local_root).unwrap();
    }

    #[test]
    fn the_committed_tree_verifies_valid_in_the_background() {
        if content::STALE_EXTRACT {
            println!("skipped: {}", content::STALE_EXTRACT_MSG);
            return;
        }
        let committed = Store::default_root(content::STUB);
        let local = temp_root("committed");
        // The committed tree as the base: verify writes only the local status and snapshots.
        let store = Store::layered(&committed, &local).unwrap();
        let ids = store.index().unwrap();
        if ids.is_empty() {
            println!("skipped: no committed nodes in {}", committed.display());
            return;
        }
        // The studio's Verify (on a worker thread there).
        let verdicts = store
            .verify(replayer(&Emerald::for_shells()), false)
            .unwrap();
        assert_eq!(verdicts.len(), ids.len());
        println!("{}: {} nodes valid", committed.display(), verdicts.len());
        assert!(
            verdicts.iter().all(|(_, s)| *s == Status::Valid),
            "{verdicts:?}"
        );
        let model = TreeModel::load(&store).unwrap();
        assert!(
            model
                .rows()
                .iter()
                .all(|r| r.origin == Origin::Shared && r.status == Some(Status::Valid))
        );
        std::fs::remove_dir_all(local).unwrap();
    }
}
