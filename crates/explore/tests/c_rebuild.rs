//! Task C: `rebuild::revalidate` after a build change (a hand-made older `BuildHash`, the
//! game unchanged, as `testing::Rebuilt` would give).

mod c_common;

use c_common::{build_lineage, lineage_rows};
use content::Emerald;
use engine::Buttons;
use engine::checkpoint::{BuildHash, build_hash};
use explore::config::Config;
use explore::db::Db;
use explore::rebuild::{RevalidateReport, revalidate};
use explore::{FindingDraft, FindingKind, GenerationDelta, NodeStatus};
use std::sync::atomic::{AtomicU32, Ordering};

/// A db at an older build: lineage 1 -> 2 -> 3 and a sibling 4 of 2 (child of 1), node 2
/// merged into 4 (a merged ancestor of a live node), a finding, an old-build snapshot.
/// `corrupt` changes node 2's stored digest.
fn old_db(path: &std::path::Path, corrupt: bool) -> (Emerald, BuildHash) {
    let game = Emerald::for_shells();
    let now = build_hash(&game);
    let old = BuildHash {
        engine: now.engine ^ 0x55,
        content: now.content,
    };
    let built = build_lineage(&game, &[Buttons::RIGHT, Buttons::DOWN]);
    let mut rows = lineage_rows(&built, old);
    let sib = build_lineage(&game, &[Buttons::LEFT]);
    let mut n4 = lineage_rows(&sib, old).pop().unwrap();
    n4.id = 4;
    n4.parent = Some(1);
    n4.key = explore::AbstractKey(4004);
    rows.push(n4);
    rows[1].status = NodeStatus::Merged;
    rows[1].merged_into = Some(4);
    if corrupt {
        rows[1].digest ^= 1;
    }
    let (mut db, st) = Db::open(path, old, content::STUB).unwrap();
    assert!(st.fresh);
    let mut snap = built[2].cp.clone();
    snap.header.build = old;
    db.apply(&GenerationDelta {
        generation: 1,
        nodes: rows,
        snapshots: vec![(3, std::sync::Arc::new(snap))],
        findings: vec![(
            FindingDraft {
                kind: FindingKind::Softlock,
                message: "trapped".into(),
                location: None,
                macro_: None,
                tail: Vec::new(),
                map: built[2].cp.world.field.map,
                pos: (0, 0),
                script: None,
                log: Vec::new(),
            },
            3,
            0,
        )],
        ..Default::default()
    })
    .unwrap();
    (game, old)
}

fn cfg() -> Config {
    Config {
        snapshot_every_depth: 2,
        ..Config::default()
    }
}

#[test]
fn an_unchanged_game_revalidates_every_node() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (game, old) = old_db(&path, false);
    let now = build_hash(&game);
    let (mut db, st) = Db::open(&path, now, content::STUB).unwrap();
    assert!(st.build_changed);
    let before: Vec<_> = db.nodes().unwrap();
    assert_eq!(db.mark_all_stale().unwrap(), 3);
    let calls = AtomicU32::new(0);
    let repair = |_: &_, _: &_| {
        calls.fetch_add(1, Ordering::Relaxed);
        None
    };
    let r = revalidate(&mut db, &game, &cfg(), &repair).unwrap();
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
    for (a, b) in after.iter().zip(&before) {
        assert_eq!(a.build, now);
        assert_eq!((a.digest, a.key, &a.inputs), (b.digest, b.key, &b.inputs));
        assert_eq!(a.total_frames, b.total_frames);
    }
    let status: Vec<_> = after.iter().map(|n| n.status).collect();
    use NodeStatus::*;
    assert_eq!(status, [Valid, Merged, Valid, Valid]);
    assert_eq!(after[1].merged_into, Some(4));
    assert_eq!(db.meta_build().unwrap(), Some(now));
    assert_eq!(db.findings().unwrap()[0].status, "stale");
    // Old-build snapshots are gone; depth 0 and 2 have new ones.
    assert!(db.snapshot(3, old).unwrap().is_none());
    assert!(db.snapshot(1, now).unwrap().is_some());
    assert!(db.snapshot(3, now).unwrap().is_some());
    assert!(db.snapshot(2, now).unwrap().is_none());
    // Reopening: the build is current.
    drop(db);
    let (_, st) = Db::open(&path, now, content::STUB).unwrap();
    assert!(!st.build_changed);
}

#[test]
fn an_unrepairable_divergence_breaks_the_subtree() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (game, _) = old_db(&path, true);
    let (mut db, _) = Db::open(&path, build_hash(&game), content::STUB).unwrap();
    db.mark_all_stale().unwrap();
    let calls = AtomicU32::new(0);
    let repair = |_: &_, _: &_| {
        calls.fetch_add(1, Ordering::Relaxed);
        None
    };
    let r = revalidate(&mut db, &game, &cfg(), &repair).unwrap();
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
    let status: Vec<_> = db.nodes().unwrap().iter().map(|n| n.status).collect();
    use NodeStatus::*;
    assert_eq!(status, [Valid, Broken, Broken, Valid]);
    assert_eq!(db.stats().unwrap().broken, 2);
}
