//! Milestone 8 Task N: incremental revalidation. After a content-only change, only the nodes
//! whose own or any ancestor's touches name a changed script or map are replayed; the others
//! move to the new build without a replay. An engine, `GLOBAL_DIGEST` (the prelude, every
//! non-generated content source) or name-list change revalidates everything.
//!
//! The "old build" is simulated on a db explored on this build: its nodes, snapshots and
//! `meta.build_content` move to another content id, and `meta.digests` to edited digests. The
//! states are those of this build, so every replayed node stays valid.

use content::Emerald;
use engine::checkpoint::build_hash;
use engine::data::{GameData, ScriptId};
use explore::config::{Budget, Config};
use explore::db::Db;
use explore::incremental::{self, Digests};
use explore::model::{NodeId, NodeStatus, Touch};
use explore::rebuild::RevalidateReport;
use explore::wire::Explore;
use std::collections::BTreeSet;
use std::path::Path;

fn config() -> Config {
    Config {
        threads: 2,
        batch: 16,
        ..Config::default()
    }
}

/// A db explored for a few generations on this build.
fn explored(path: &Path) {
    let mut ex = Explore::open(Emerald::for_shells(), path, config()).unwrap();
    ex.run(Budget::Generations(6), |_| {}).unwrap();
}

/// Turns the db at `path` into one of an older content build whose digests are this build's
/// edited by `edit`, then reopens it on this build: the revalidation report and the db.
fn reopen_after(path: &Path, edit: impl FnOnce(&mut Digests)) -> (RevalidateReport, Db) {
    let mut old = Digests::current();
    edit(&mut old);
    let game = Emerald::for_shells();
    let fake = (build_hash(&game).content ^ 0x5a5a_5a5a) as i64;
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute("UPDATE node SET build_content = ?1", [fake])
        .unwrap();
    conn.execute("UPDATE snapshot SET build_content = ?1", [fake])
        .unwrap();
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'build_content'",
        [(fake as u64).to_string()],
    )
    .unwrap();
    if old == Digests::default() {
        conn.execute("DELETE FROM meta WHERE key = ?1", [incremental::META_KEY])
            .unwrap();
    } else {
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = ?2",
            [old.encode(), incremental::META_KEY.to_string()],
        )
        .unwrap();
    }
    drop(conn);
    let ex = Explore::open(game, path, config()).unwrap();
    let r = ex.revalidated.expect("a build change revalidates");
    (r, ex.db)
}

/// Valid and merged nodes (the ones a revalidation checks).
fn checked(db: &Db) -> Vec<NodeId> {
    db.nodes_light()
        .unwrap()
        .into_iter()
        .filter(|n| matches!(n.status, NodeStatus::Valid | NodeStatus::Merged))
        .map(|n| n.id)
        .collect()
}

/// The nodes whose lineage touches `t`.
fn under(db: &Db, t: Touch) -> BTreeSet<NodeId> {
    let touched: BTreeSet<NodeId> = db.touching(t).unwrap().into_iter().collect();
    checked(db)
        .into_iter()
        .filter(|&id| {
            db.lineage(id)
                .unwrap()
                .iter()
                .any(|n| touched.contains(&n.id))
        })
        .collect()
}

fn everything_replayed(r: RevalidateReport, all: usize) {
    assert_eq!(r.kept, 0, "{r:?}");
    assert_eq!(r.valid as usize, all, "{r:?}");
    assert_eq!((r.repaired, r.broken), (0, 0), "{r:?}");
}

/// A Littleroot script started by some nodes but not by the root job.
fn littleroot_script(db: &Db) -> Option<(ScriptId, BTreeSet<NodeId>)> {
    let root: Vec<NodeId> = db
        .nodes_light()
        .unwrap()
        .iter()
        .filter(|n| n.parent.is_none())
        .map(|n| n.id)
        .collect();
    content::data::SCRIPT_NAMES
        .iter()
        .enumerate()
        .filter(|(_, n)| n.starts_with("LittlerootTown_"))
        .map(|(i, _)| ScriptId(i as u16))
        .find_map(|s| {
            let u = under(db, Touch::Script(s));
            (!u.is_empty() && !root.iter().any(|r| u.contains(r))).then_some((s, u))
        })
}

#[test]
fn a_changed_script_replays_only_the_nodes_under_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.db");
    explored(&path);
    let (before, script, stale, all) = {
        let (db, _) = Db::open(&path, build_hash(&Emerald::for_shells()), content::FLAVOR).unwrap();
        let Some((s, stale)) = littleroot_script(&db) else {
            // The stub world has no scripts, Silice no Littleroot (Emerald only).
            if content::FLAVOR == content::Flavor::Emerald {
                panic!("no Littleroot script touched below the root");
            }
            return;
        };
        (db.content_digest().unwrap(), s, stale, checked(&db).len())
    };
    // An override of that one script changes its digest only.
    let (r, db) = reopen_after(&path, |d| d.scripts[usize::from(script.0)].1 ^= 1);
    assert_eq!(r.valid as usize, stale.len(), "{r:?}");
    assert_eq!(r.kept as usize, all - stale.len(), "{r:?}");
    assert!(r.kept > 0, "{r:?}");
    assert_eq!((r.repaired, r.broken), (0, 0));
    // Same rows, statuses and build as before the simulated change.
    assert_eq!(db.content_digest().unwrap(), before);
    assert_eq!(
        Digests::decode(&db.meta(incremental::META_KEY).unwrap().unwrap()),
        Some(Digests::current())
    );
}

#[test]
fn a_changed_std_script_reaches_the_nodes_of_its_callers() {
    // A script's digest covers its call-graph closure (`extract::codegen::lower`, tested by
    // `extract/tests/codegen.rs`): a changed std script changes every caller's digest. No node
    // starts a std script by itself, so only the callers' digests select nodes.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.db");
    explored(&path);
    let (caller, stale, all, std_script) = {
        let (db, _) = Db::open(&path, build_hash(&Emerald::for_shells()), content::FLAVOR).unwrap();
        let Some((s, stale)) = littleroot_script(&db) else {
            if content::FLAVOR == content::Flavor::Emerald {
                panic!("no Littleroot script touched below the root");
            }
            return;
        };
        let std_script = content::data::SCRIPT_NAMES
            .iter()
            .position(|n| n.starts_with("Std_") || n.starts_with("Common_EventScript_"))
            .map(|i| ScriptId(i as u16))
            .filter(|&x| db.touching(Touch::Script(x)).unwrap().is_empty())
            .expect("a std script no node started");
        (s, stale, checked(&db).len(), std_script)
    };
    // The std script alone: no node is under it.
    let (r, _) = reopen_after(&path, |d| d.scripts[usize::from(std_script.0)].1 ^= 1);
    assert_eq!((r.valid, r.kept as usize), (0, all), "{r:?}");
    // With its caller, as the content build digests them.
    let (r, _) = reopen_after(&path, |d| {
        d.scripts[usize::from(std_script.0)].1 ^= 1;
        d.scripts[usize::from(caller.0)].1 ^= 1;
    });
    assert_eq!(r.valid as usize, stale.len(), "{r:?}");
    assert_eq!(r.kept as usize, all - stale.len(), "{r:?}");
}

#[test]
fn global_engine_name_and_root_map_changes_revalidate_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.db");
    explored(&path);
    let all = {
        let (db, _) = Db::open(&path, build_hash(&Emerald::for_shells()), content::FLAVOR).unwrap();
        checked(&db).len()
    };
    // The prelude and every non-generated content source are in GLOBAL_DIGEST.
    let (r, _) = reopen_after(&path, |d| d.global ^= 1);
    everything_replayed(r, all);
    let (r, _) = reopen_after(&path, |d| d.engine ^= 1);
    everything_replayed(r, all);
    // A map renamed (one name in the old build, missing in the new one).
    let (r, _) = reopen_after(&path, |d| d.maps[0].1.push('X'));
    everything_replayed(r, all);
    // The new game's map: every node starts under it.
    let start = Emerald::for_shells().new_game().map;
    let (r, _) = reopen_after(&path, |d| {
        let m = d.maps.iter_mut().find(|m| m.0 == start).unwrap();
        m.2 ^= 1;
    });
    everything_replayed(r, all);
    // No digests recorded (a db of an older explorer).
    let (r, _) = reopen_after(&path, |d| *d = Digests::default());
    everything_replayed(r, all);
}

#[test]
fn digests_round_trip() {
    let d = Digests::current();
    assert_eq!(Digests::decode(&d.encode()), Some(d.clone()));
    assert_eq!(d.scripts.len(), content::data::SCRIPT_COUNT);
    assert_eq!(Digests::decode("x"), None);
    assert_eq!(
        incremental::changes(&d, &d).map(|c| c.scripts.len()),
        Some(0)
    );
}
