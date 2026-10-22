//! Task C: `rebuild::revalidate` after a build change (a hand-made older `BuildHash`, the
//! game unchanged, as `testing::Rebuilt` would give).
//!
//! The tree: root 1 -> 2 -> 3, and 4 a child of 1 (a sibling of 2). Tests corrupt rows (inputs,
//! digests, totals) to force divergences; the repair closure returns the true expansion of a
//! macro, and the key function maps true end states to their keys.

mod c_common;

use c_common::{Built, build_lineage, lineage_rows, walk};
use content::Emerald;
use engine::Buttons;
use engine::checkpoint::{BuildHash, Checkpoint, WorldSnapshot, build_hash, state_digest};
use explore::config::Config;
use explore::db::Db;
use explore::rebuild::{RevalidateReport, revalidate, revalidate_with_key};
use explore::{
    AbstractKey, FindingDraft, FindingKind, GenerationDelta, Macro, NodeRow, NodeStatus,
};
use game_native::trace::{Run, rle};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use NodeStatus::*;

struct Fixture {
    game: Emerald,
    old: BuildHash,
    now: BuildHash,
    /// Nodes 1..=4 in id order.
    built: Vec<Built>,
    /// The true rows (keys 1001, 1002, 1003, 4004).
    rows: Vec<NodeRow>,
}

impl Fixture {
    fn new() -> Fixture {
        let game = Emerald::for_shells();
        let now = build_hash(&game);
        let old = BuildHash {
            engine: now.engine ^ 0x55,
            content: now.content,
        };
        let mut built = build_lineage(&game, &[Buttons::RIGHT, Buttons::DOWN]);
        let mut rows = lineage_rows(&built, old);
        let sib = build_lineage(&game, &[Buttons::LEFT]).pop().unwrap();
        let mut n4 = lineage_rows(
            &[
                Built {
                    keys: Vec::new(),
                    cp: built[0].cp.clone(),
                },
                Built {
                    keys: sib.keys.clone(),
                    cp: sib.cp.clone(),
                },
            ],
            old,
        )
        .pop()
        .unwrap();
        n4.id = 4;
        n4.parent = Some(1);
        n4.key = AbstractKey(4004);
        n4.macro_ = walk(9, 9);
        n4.total_frames = rows[0].total_frames + u64::from(n4.frames);
        rows.push(n4);
        built.push(sib);
        Fixture {
            game,
            old,
            now,
            built,
            rows,
        }
    }

    /// A db at the old build with `rows` (a finding, an old-build snapshot of node 3), reopened
    /// at the current build with every node marked stale.
    fn db(&self, path: &Path, rows: Vec<NodeRow>) -> Db {
        let (mut db, st) = Db::open(path, self.old, content::STUB).unwrap();
        assert!(st.fresh);
        let mut snap = self.built[2].cp.clone();
        snap.header.build = self.old;
        db.apply(&GenerationDelta {
            generation: 1,
            nodes: rows,
            snapshots: vec![(3, Arc::new(snap))],
            findings: vec![(
                FindingDraft {
                    kind: FindingKind::Softlock,
                    message: "trapped".into(),
                    location: None,
                    macro_: None,
                    tail: Vec::new(),
                    map: self.built[2].cp.world.field.map,
                    pos: (0, 0),
                    script: None,
                    log: Vec::new(),
                },
                Some(3),
                0,
            )],
            ..Default::default()
        })
        .unwrap();
        drop(db);
        let (mut db, st) = Db::open(path, self.now, content::STUB).unwrap();
        assert!(st.build_changed);
        db.mark_all_stale().unwrap();
        db
    }

    /// The true expansion of each macro, counting calls; `fail` macros return `None`.
    fn repair<'a>(
        &'a self,
        calls: &'a AtomicU32,
        fail: &'a [Macro],
    ) -> impl Fn(Option<&Checkpoint>, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync + 'a {
        move |start, m| {
            calls.fetch_add(1, Ordering::Relaxed);
            if fail.contains(m) {
                return None;
            }
            let i = self.rows.iter().position(|n| n.macro_ == *m).unwrap();
            // The root re-expands from a new game, every other node from its parent's state.
            assert_eq!(start.is_none(), i == 0);
            if let (Some(s), Some(p)) = (start, self.rows[i].parent) {
                assert_eq!(state_digest(&s.world), self.rows[p as usize - 1].digest);
            }
            Some((
                rle(self.built[i].keys.iter().copied()),
                self.built[i].cp.clone(),
            ))
        }
    }

    /// True end states -> their row's key, with `over` (row index -> key) applied.
    fn keys(&self, over: &[(usize, u64)]) -> impl Fn(&WorldSnapshot) -> AbstractKey {
        let mut m: BTreeMap<u64, AbstractKey> =
            self.rows.iter().map(|n| (n.digest, n.key)).collect();
        for &(i, k) in over {
            m.insert(self.rows[i].digest, AbstractKey(k));
        }
        move |w| m[&state_digest(w)]
    }
}

fn cfg() -> Config {
    Config {
        snapshot_every_depth: 2,
        ..Config::default()
    }
}

/// Wrong inputs (and totals), right digest: the replay diverges, the repair restores it.
fn corrupt_inputs(n: &mut NodeRow) {
    n.inputs = rle([Buttons::NONE; 3]);
    n.frames = 3;
    n.total_frames += 1000;
}

fn statuses(db: &Db) -> Vec<NodeStatus> {
    db.nodes().unwrap().iter().map(|n| n.status).collect()
}

#[test]
fn an_unchanged_game_revalidates_every_node() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    // Node 2 merged into 4: a merged ancestor of a live node.
    let mut rows = f.rows.clone();
    rows[1].status = Merged;
    rows[1].merged_into = Some(4);
    rows[1].key = rows[3].key;
    let mut db = f.db(&path, rows.clone());
    let calls = AtomicU32::new(0);
    let r = revalidate(&mut db, &f.game, &cfg(), &f.repair(&calls, &[])).unwrap();
    assert_eq!(
        r,
        RevalidateReport {
            valid: 4,
            repaired: 0,
            broken: 0
        }
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let after = db.nodes().unwrap();
    for (a, b) in after.iter().zip(&rows) {
        assert_eq!(a.build, f.now);
        assert_eq!((a.digest, a.key, &a.inputs), (b.digest, b.key, &b.inputs));
        assert_eq!(a.total_frames, b.total_frames);
    }
    assert_eq!(statuses(&db), [Valid, Merged, Valid, Valid]);
    assert_eq!(after[1].merged_into, Some(4));
    assert_eq!(db.meta_build().unwrap(), Some(f.now));
    assert_eq!(db.findings().unwrap()[0].status, "stale");
    // Old-build snapshots are gone; depths 0 and 2 have new ones.
    assert!(db.snapshot(3, f.old).unwrap().is_none());
    assert!(db.snapshot(1, f.now).unwrap().is_some());
    assert!(db.snapshot(3, f.now).unwrap().is_some());
    assert!(db.snapshot(2, f.now).unwrap().is_none());
    drop(db);
    let (_, st) = Db::open(&path, f.now, content::STUB).unwrap();
    assert!(!st.build_changed);
}

#[test]
fn an_unrepairable_divergence_breaks_the_subtree() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    rows[1].digest ^= 1;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate(&mut db, &f.game, &cfg(), &f.repair(&calls, &[walk(0, 1)])).unwrap();
    assert_eq!(
        r,
        RevalidateReport {
            valid: 2,
            repaired: 0,
            broken: 2
        }
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "only node 2 is re-expanded"
    );
    assert_eq!(statuses(&db), [Valid, Broken, Broken, Valid]);
    assert_eq!(db.stats().unwrap().broken, 2);
}

#[test]
fn a_repair_recomputes_total_frames_down_the_tree() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    corrupt_inputs(&mut rows[1]);
    rows[2].total_frames += 1000;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[]),
        &f.keys(&[]),
    )
    .unwrap();
    assert_eq!(
        r,
        RevalidateReport {
            valid: 3,
            repaired: 1,
            broken: 0
        }
    );
    // Back to the true rows (status and build aside).
    let after = db.nodes().unwrap();
    for (a, t) in after.iter().zip(&f.rows) {
        assert_eq!((a.status, a.build), (Valid, f.now));
        assert_eq!(
            (&a.inputs, a.frames, a.total_frames),
            (&t.inputs, t.frames, t.total_frames)
        );
        assert_eq!(
            (a.digest, a.key, a.map, a.pos),
            (t.digest, t.key, t.map, t.pos)
        );
    }
}

#[test]
fn a_diverged_root_re_expands_from_a_new_game() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    rows[0].inputs = rle([Buttons::A; 2]);
    rows[0].frames = 2;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[]),
        &f.keys(&[]),
    )
    .unwrap();
    assert_eq!((r.valid, r.repaired, r.broken), (3, 1, 0));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let root = db.node(1).unwrap().unwrap();
    assert_eq!((&root.inputs, root.status), (&f.rows[0].inputs, Valid));
    assert_eq!(statuses(&db), [Valid; 4]);
}

#[test]
fn a_repaired_key_waits_for_the_deeper_holder_then_merges() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    // Node 4 (depth 1) is repaired onto node 3's key (depth 2, not yet checked); node 3 then
    // keeps it, and the one with fewer total frames stays live.
    let mut rows = f.rows.clone();
    corrupt_inputs(&mut rows[3]);
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[]),
        &f.keys(&[(3, 1003)]),
    )
    .unwrap();
    assert_eq!((r.valid, r.repaired, r.broken), (3, 1, 0));
    let (t3, t4) = (f.rows[2].total_frames, f.rows[3].total_frames);
    let (win, lose) = if (t4, 4) < (t3, 3) { (4, 3) } else { (3, 4) };
    let w = db.node(win).unwrap().unwrap();
    let l = db.node(lose).unwrap().unwrap();
    assert_eq!(
        (w.status, w.merged_into, w.key),
        (Valid, None, AbstractKey(1003))
    );
    assert_eq!(
        (l.status, l.merged_into, l.key),
        (Merged, Some(win), AbstractKey(1003))
    );
}

#[test]
fn more_goals_keep_the_key_against_fewer_frames() {
    // As `Runner::merge` (Task G; `d_runner::more_goals_supersede_even_with_more_frames`):
    // node 4 is repaired onto node 3's key; of the two, the one with more total frames has
    // reached more goals, and it keeps the key.
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    corrupt_inputs(&mut rows[3]);
    let (t3, t4) = (f.rows[2].total_frames, f.rows[3].total_frames);
    assert_ne!(t3, t4);
    let (win, lose) = if t4 > t3 { (4, 3) } else { (3, 4) };
    rows[win as usize - 1].progress = 10;
    rows[lose as usize - 1].progress = 0;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[]),
        &f.keys(&[(3, 1003)]),
    )
    .unwrap();
    assert_eq!((r.valid, r.repaired, r.broken), (3, 1, 0));
    let w = db.node(win).unwrap().unwrap();
    let l = db.node(lose).unwrap().unwrap();
    assert!(w.total_frames > l.total_frames && w.progress > l.progress);
    assert_eq!(
        (w.status, w.merged_into, w.key),
        (Valid, None, AbstractKey(1003))
    );
    assert_eq!(
        (l.status, l.merged_into, l.key),
        (Merged, Some(win), AbstractKey(1003))
    );
}

#[test]
fn a_waiting_node_takes_the_key_when_the_holder_breaks() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    corrupt_inputs(&mut rows[3]);
    rows[2].digest ^= 1;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[walk(0, 2)]),
        &f.keys(&[(3, 1003)]),
    )
    .unwrap();
    assert_eq!((r.valid, r.repaired, r.broken), (2, 1, 1));
    assert_eq!(statuses(&db), [Valid, Valid, Broken, Valid]);
    let n4 = db.node(4).unwrap().unwrap();
    assert_eq!((n4.key, n4.merged_into), (AbstractKey(1003), None));
}

#[test]
fn repaired_siblings_swap_keys() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let mut rows = f.rows.clone();
    corrupt_inputs(&mut rows[1]);
    corrupt_inputs(&mut rows[3]);
    rows[2].total_frames += 1000;
    let mut db = f.db(&dir.path().join("explore.db"), rows);
    let calls = AtomicU32::new(0);
    let r = revalidate_with_key(
        &mut db,
        &f.game,
        &cfg(),
        None,
        &f.repair(&calls, &[]),
        &f.keys(&[(1, 4004), (3, 1002)]),
    )
    .unwrap();
    assert_eq!((r.valid, r.repaired, r.broken), (2, 2, 0));
    assert_eq!(statuses(&db), [Valid; 4]);
    assert_eq!(db.node(2).unwrap().unwrap().key, AbstractKey(4004));
    assert_eq!(db.node(4).unwrap().unwrap().key, AbstractKey(1002));
}

#[test]
fn the_result_does_not_depend_on_the_pool_size() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let digests: Vec<u64> = [1, 4]
        .into_iter()
        .map(|threads| {
            let mut rows = f.rows.clone();
            corrupt_inputs(&mut rows[1]);
            corrupt_inputs(&mut rows[3]);
            let mut db = f.db(&dir.path().join(format!("{threads}.db")), rows);
            let calls = AtomicU32::new(0);
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                revalidate_with_key(
                    &mut db,
                    &f.game,
                    &cfg(),
                    None,
                    &f.repair(&calls, &[]),
                    &f.keys(&[(1, 4004), (3, 1002)]),
                )
            })
            .unwrap();
            db.content_digest().unwrap()
        })
        .collect();
    assert_eq!(digests[0], digests[1]);
}
