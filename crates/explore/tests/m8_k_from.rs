//! Milestone 8 Task K: `run --from` a checkpoint node, `export --to-store`, and the exported
//! node verifying in the store (both flavors; temp stores seeded from the cached route).

use content::Emerald;
use explore::NodeStatus;
use explore::config::{Budget, Config};
use explore::db::Db;
use explore::wire::{self, EXPORT_PREFIX, Explore};
use game_native::checkpoints::{replayer, seed};
use game_native::store::{Status, Store};
use game_native::trace::{CheckpointId, Parent};
use std::path::Path;

/// A committed tree holding the route's first two nodes, and an empty local store over it.
fn stores(dir: &Path) -> Store {
    let base = dir.join("committed");
    let mut rec = game_native::scenario::cached_route().clone();
    rec.nodes.truncate(2);
    seed(&Store::open(&base).unwrap(), &Emerald::for_shells(), &rec).unwrap();
    Store::layered(&base, dir.join("local")).unwrap()
}

fn config() -> Config {
    Config {
        threads: 2,
        batch: 0,
        ..Config::default()
    }
}

#[test]
fn explore_from_a_committed_node_and_export_a_verified_node() {
    let dir = tempfile::tempdir().unwrap();
    let store = stores(dir.path());
    let root = store.find("c00002").unwrap();
    let game = Emerald::for_shells();
    let path = dir.path().join("explore.db");
    let mut ex = Explore::open_from(game, &path, config(), &store, &root.meta.label).unwrap();
    assert_eq!(ex.root.as_ref().map(|r| r.id), Some(CheckpointId(2)));
    ex.run(Budget::Generations(3), |_| {}).unwrap();
    // The root node starts at the checkpoint's frame.
    let first = ex.db.node(1).unwrap().unwrap();
    assert_eq!(first.end_frame, root.meta.frame + first.frames, "{first:?}");
    let digest = ex.db.content_digest().unwrap();
    drop(ex);

    // Reopened without --from it keeps its root; another root is refused.
    let ex = Explore::open(game, &path, config()).unwrap();
    assert_eq!(ex.root.as_ref().map(|r| r.id), Some(CheckpointId(2)));
    assert_eq!(ex.db.content_digest().unwrap(), digest);
    drop(ex);
    let e = Explore::open_from(game, &path, config(), &store, "c00001")
        .err()
        .expect("another root");
    assert!(e.to_string().contains("explores from c00002"), "{e}");

    // The deepest valid node, exported into the local store.
    let db = Db::open_read_only(&path).unwrap();
    assert_eq!(db.meta("root").unwrap().as_deref(), Some("c00002"));
    let node = db
        .nodes_light()
        .unwrap()
        .into_iter()
        .filter(|n| n.status == NodeStatus::Valid)
        .max_by_key(|n| (n.depth, std::cmp::Reverse(n.id)))
        .unwrap();
    assert!(node.depth >= 1, "{node:?}");
    let id = wire::export_to_store(&game, &db, node.id, &store, None).unwrap();
    assert!(!store.is_base(id), "exported into the local store");
    let n = store.find(&id.to_string()).unwrap();
    assert_eq!(n.trace.parent, Parent::Checkpoint(CheckpointId(2)));
    assert_eq!(n.meta.digest, node.digest);
    assert!(n.meta.label.starts_with(EXPORT_PREFIX), "{}", n.meta.label);
    assert!(n.meta.label.contains(":MAP_"), "{}", n.meta.label);
    assert!(
        n.meta.label.ends_with(&format!(":n{}", node.id)),
        "{}",
        n.meta.label
    );
    // The export's replay is its verdict; a full verify agrees.
    assert_eq!(store.statuses().unwrap().get(&id), Some(&Status::Valid));
    let verdicts = store.verify_nodes(replayer(&game), false).unwrap();
    assert_eq!(verdicts.len(), 3);
    assert!(
        verdicts.iter().all(|v| v.status == Status::Valid),
        "{verdicts:?}"
    );
    // A second export of the same node takes the next free label.
    let again = wire::export_to_store(&game, &db, node.id, &store, None).unwrap();
    let label = store.find(&again.to_string()).unwrap().meta.label;
    assert_eq!(label, format!("{} #2", n.meta.label));
}

#[test]
fn a_new_game_db_exports_new_game_traces() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store")).unwrap();
    let game = Emerald::for_shells();
    let path = dir.path().join("explore.db");
    let mut ex = Explore::open(game, &path, config()).unwrap();
    ex.run(Budget::Generations(2), |_| {}).unwrap();
    drop(ex);
    let db = Db::open_read_only(&path).unwrap();
    assert_eq!(db.meta("root").unwrap(), None);
    let id = wire::export_to_store(&game, &db, 2, &store, Some("mine")).unwrap();
    let n = store.find("mine").unwrap();
    assert_eq!((n.meta.id, n.trace.parent), (id, Parent::NewGame));
    let verdicts = store.verify_nodes(replayer(&game), false).unwrap();
    assert_eq!(verdicts.len(), 1);
    assert_eq!(verdicts[0].status, Status::Valid);
}
