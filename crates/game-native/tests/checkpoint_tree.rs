//! Milestone 5 Task G: the route's checkpoint nodes, the real replayer, `materialize`, and the
//! committed tree of the current content flavor (`checkpoints/{stub|emerald|silice}/nodes`), which
//! must verify Valid like the golden hashes: an intended behaviour change needs
//! `cargo xtask checkpoints verify --accept`.

use engine::checkpoint::{decode, encode, state_digest};
use game_native::Session;
use game_native::checkpoints::{ROUTE_PREFIX, materialize, replayer, seed};
use game_native::scenario::{Recording, cached_route};
use game_native::store::{Status, Store};
use game_native::trace::Parent;
use std::path::Path;

mod common;
use common::temp_root;

fn game() -> content::Emerald {
    content::Emerald::for_shells()
}

fn recording() -> &'static Recording {
    cached_route()
}

fn expected_names() -> &'static [&'static str] {
    match content::FLAVOR {
        content::Flavor::Stub => &["stub_start", "stub_after_battle", "stub_end"],
        content::Flavor::Emerald => &["lab", "after_wild", "oldale_center", "route103_after_may"],
        content::Flavor::Silice => &["silice_start", "silice_idle", "silice_end"],
    }
}

#[test]
fn every_route_checkpoint_is_capturable_and_its_trace_replays_valid() {
    let rec = recording();
    let names: Vec<&str> = rec.nodes.iter().map(|n| n.0).collect();
    assert_eq!(names, expected_names());
    println!("nodes {:?}", rec.nodes);
    let mut s = Session::new(game());
    for &(name, frame) in &rec.nodes {
        while s.world().frame() < frame {
            let f = s.world().frame();
            s.step(rec.keys_at(f));
        }
        assert_eq!(s.world().capturable(), Ok(()), "{name} at frame {frame}");
    }
    let root = temp_root("seed");
    let store = Store::open(&root).unwrap();
    let ids = seed(&store, &game(), rec).unwrap();
    assert_eq!(ids.len(), rec.nodes.len());
    let nodes = store.topo_order().unwrap();
    for (n, &(name, frame)) in nodes.iter().zip(&rec.nodes) {
        assert_eq!(n.meta.label, format!("{ROUTE_PREFIX}{name}"));
        assert_eq!(n.meta.frame, frame);
    }
    assert_eq!(nodes[0].trace.parent, Parent::NewGame);
    for w in nodes.windows(2) {
        assert_eq!(w[1].trace.parent, Parent::Checkpoint(w[0].meta.id));
    }
    let verdicts = store.verify(replayer(&game()), false).unwrap();
    assert!(
        verdicts.iter().all(|(_, s)| *s == Status::Valid),
        "{verdicts:?}"
    );
    // Seeding again replaces the route nodes and keeps the others.
    let last = nodes.last().unwrap();
    let mine = store
        .add(
            "mine",
            empty_trace_after(last),
            &store.snapshot(last.meta.id).unwrap().unwrap(),
        )
        .unwrap();
    assert!(
        seed(&store, &game(), rec).is_err(),
        "a node below the route"
    );
    store.remove(mine).unwrap();
    let (trace, cp) = first_node(rec);
    let other = store.add("other", trace, &cp).unwrap();
    // A route that fails to replay (its first node is not capturable) changes nothing.
    let before = store.nodes().unwrap();
    let mut bad = rec.clone();
    bad.nodes[0].1 = 1;
    assert!(seed(&store, &game(), &bad).is_err());
    assert_eq!(store.nodes().unwrap(), before);
    let again = seed(&store, &game(), rec).unwrap();
    assert_eq!(again.len(), rec.nodes.len());
    let labels: Vec<String> = store
        .nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.meta.label)
        .collect();
    assert_eq!(labels.len(), rec.nodes.len() + 1);
    assert!(store.find("other").unwrap().meta.id == other);
    let _ = std::fs::remove_dir_all(root);
}

/// A zero-frame trace from `parent` (a node at the same state).
fn empty_trace_after(parent: &game_native::store::Node) -> game_native::trace::Trace {
    game_native::trace::TraceRecorder::new(parent.meta.frame).trace(
        Parent::Checkpoint(parent.meta.id),
        engine::checkpoint::build_hash(&game()),
    )
}

/// The trace from a new game to the route's first node, and that node's checkpoint.
fn first_node(rec: &Recording) -> (game_native::trace::Trace, engine::checkpoint::Checkpoint) {
    let mut r = game_native::trace::TraceRecorder::new(0);
    let mut w = engine::World::new(&game());
    for f in 0..rec.nodes[0].1 {
        w.step(&game(), rec.keys_at(f));
        r.push(rec.keys_at(f), w.hash());
    }
    let build = engine::checkpoint::build_hash(&game());
    (
        r.trace(Parent::NewGame, build),
        w.checkpoint(&game()).unwrap(),
    )
}

#[test]
fn materialize_replays_the_chain_and_caches_each_snapshot() {
    let rec = recording();
    let root = temp_root("materialize");
    let store = Store::open(&root).unwrap();
    let ids = seed(&store, &game(), rec).unwrap();
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let last = *ids.last().unwrap();
    let m = materialize(&store, &game(), last).unwrap();
    assert!(!m.diverged);
    let cp = m.checkpoint;
    let node = store.find(&last.to_string()).unwrap();
    assert_eq!(state_digest(&cp.world), node.meta.digest);
    assert_eq!(cp.header.world_hash, node.meta.world_hash);
    for &id in &ids {
        assert!(store.snapshot(id).unwrap().is_some(), "{id} cached");
    }
    // From the cache now: the same checkpoint.
    assert_eq!(materialize(&store, &game(), last).unwrap().checkpoint, cp);
    // A cached snapshot of another state (a reseeded tree reusing the id) is not served.
    let (first, second) = (ids[0], ids[1]);
    let wrong = store.snapshot(first).unwrap().unwrap();
    store.put_snapshot(second, &wrong).unwrap();
    let m = materialize(&store, &game(), second).unwrap();
    let want = store.find(&second.to_string()).unwrap().meta.digest;
    assert_eq!(state_digest(&m.checkpoint.world), want);
    assert!(!m.diverged);
    // A node this build no longer reaches is replayed and flagged, not cached.
    let mut n = store.find(&second.to_string()).unwrap();
    n.meta.digest ^= 1;
    std::fs::write(
        root.join(format!("nodes/{second}.ron")),
        ron::to_string(&n).unwrap(),
    )
    .unwrap();
    let m = materialize(&store, &game(), second).unwrap();
    assert!(m.diverged);
    assert_eq!(state_digest(&m.checkpoint.world), want);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn session_restore_round_trips_at_every_route_node() {
    const AFTER: u32 = 120;
    let rec = recording();
    // One live session plays the route; each node's restored session follows it for
    // `AFTER` frames on the same keys.
    let mut live = Session::new(game());
    let mut restored: Vec<(&str, u32, Session<content::Emerald>)> = Vec::new();
    let end = rec.nodes.last().unwrap().1 + AFTER;
    for f in 0..=end {
        assert_eq!(live.world().frame(), f);
        for &(name, frame) in rec.nodes.iter().filter(|n| n.1 == f) {
            let cp = decode(&encode(&live.checkpoint().unwrap())).unwrap();
            let mut s = Session::restore(game(), &cp).unwrap();
            assert_eq!(s.world().hash(), live.world().hash(), "{name}");
            assert_eq!(s.framebuffer(), live.framebuffer(), "{name}: boot frame");
            restored.push((name, frame, s));
        }
        if f == end {
            break;
        }
        live.step(rec.keys_at(f));
        for (name, frame, s) in &mut restored {
            s.step(rec.keys_at(f));
            let after = f + 1 - *frame;
            assert_eq!(s.world().hash(), live.world().hash(), "{name} +{after}");
            if matches!(after, 1 | 2 | 60 | AFTER) {
                assert_eq!(s.framebuffer(), live.framebuffer(), "{name} +{after}");
            }
        }
        restored.retain(|r| f + 1 - r.1 < AFTER);
    }
    assert!(restored.is_empty());
}

/// Copies `nodes/*.ron` of `from` into a fresh store (verify writes snapshots and status; the
/// committed tree's directory is left alone, so tests may run in parallel).
fn copy_nodes(from: &Path, to: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(from.join("nodes")) else {
        return 0;
    };
    std::fs::create_dir_all(to.join("nodes")).unwrap();
    let mut n = 0;
    for e in entries {
        let e = e.unwrap();
        if e.path().extension().is_some_and(|x| x == "ron") {
            std::fs::copy(e.path(), to.join("nodes").join(e.file_name())).unwrap();
            n += 1;
        }
    }
    n
}

#[test]
fn the_committed_tree_verifies_valid() {
    if content::STALE_EXTRACT {
        println!("skipped: {}", content::STALE_EXTRACT_MSG);
        return;
    }
    // This checkout's committed tree: `nodes/` only, never `play`'s saves under `local/`.
    let committed = Store::default_root(content::FLAVOR);
    let root = temp_root("committed");
    if copy_nodes(&committed, &root) == 0 {
        println!("skipped: no committed nodes in {}", committed.display());
        return;
    }
    let store = Store::open(&root).unwrap();
    let verdicts = store.verify(replayer(&game()), false).unwrap();
    // The health `cargo xtask checkpoints verify` (and parity) requires.
    let bad: Vec<_> = verdicts
        .iter()
        .filter(|(_, s)| !Status::healthy([s]))
        .collect();
    assert!(
        Status::healthy(verdicts.iter().map(|(_, s)| s)),
        "{} does not verify: {bad:?}; if the behaviour change is intended, run \
         `cargo xtask checkpoints verify --accept` and commit the nodes",
        committed.display()
    );
    println!("{}: {} nodes valid", committed.display(), verdicts.len());
    let _ = std::fs::remove_dir_all(root);
}
