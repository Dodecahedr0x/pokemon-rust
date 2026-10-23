//! Milestone 8 Task N, after review: what incremental revalidation selects beyond the scripts
//! started and the maps entered. A changed map (and its connected neighbour), a trainer
//! script that sight reads on its map unstarted, a node whose engine events were dropped, and
//! the touches a revalidation rewrites for the nodes it replays or repairs.
//!
//! As in `m8_n_incremental.rs`, the "old build" is this build with edited digests.

use content::Emerald;
use engine::checkpoint::{Checkpoint, build_hash};
use engine::data::{GameData, MapId, ObjectEvent, ScriptId};
use explore::config::{Budget, Config};
use explore::db::Db;
use explore::incremental::{self, Digests, Marked};
use explore::model::{Macro, NodeId, NodeStatus, SimSummary, Touch};
use explore::rebuild::{self, RevalidateReport};
use explore::testing::TrainerOn;
use explore::wire::{self, Explore};
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

fn open(path: &Path) -> Db {
    Db::open(path, build_hash(&Emerald::for_shells()), content::STUB)
        .unwrap()
        .0
}

/// A copy of the db at `from` (with its WAL, if any) at `to`.
fn copy_db(from: &Path, to: &Path) {
    std::fs::copy(from, to).unwrap();
    for ext in ["-wal", "-shm"] {
        let f = format!("{}{ext}", from.display());
        if Path::new(&f).exists() {
            std::fs::copy(&f, format!("{}{ext}", to.display())).unwrap();
        }
    }
}

/// Moves the db at `path` to an older content build whose digests are this build's edited by
/// `edit`, then reopens it on this build: the revalidation report.
fn reopen_after(path: &Path, edit: impl FnOnce(&mut Digests)) -> RevalidateReport {
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
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        [incremental::META_KEY.to_string(), old.encode()],
    )
    .unwrap();
    drop(conn);
    let ex = Explore::open(game, path, config()).unwrap();
    ex.revalidated.expect("a build change revalidates")
}

/// `incremental::mark` on the db at `path` (explored on this build) as if its digests were
/// this build's edited by `edit`, with `game`'s maps: the result and the nodes marked Stale.
fn mark_after<G: GameData>(
    path: &Path,
    game: &G,
    edit: impl FnOnce(&mut Digests),
) -> (Marked, BTreeSet<NodeId>) {
    let mut old = Digests::current();
    edit(&mut old);
    let build = build_hash(&Emerald::for_shells());
    let mut db = open(path);
    db.set_meta(incremental::META_KEY, &old.encode()).unwrap();
    let start = Emerald::for_shells().new_game().map;
    let m = incremental::mark(&mut db, game, &Digests::current(), build, start)
        .unwrap()
        .expect("a script or map change marks incrementally");
    let stale = db
        .nodes_light()
        .unwrap()
        .into_iter()
        .filter(|n| n.status == NodeStatus::Stale)
        .map(|n| n.id)
        .collect();
    (m, stale)
}

/// Valid and merged nodes (the ones a revalidation checks).
fn checked(db: &Db) -> BTreeSet<NodeId> {
    db.nodes_light()
        .unwrap()
        .into_iter()
        .filter(|n| matches!(n.status, NodeStatus::Valid | NodeStatus::Merged))
        .map(|n| n.id)
        .collect()
}

/// The valid nodes among `ids`.
fn valid(db: &Db, ids: &BTreeSet<NodeId>) -> BTreeSet<NodeId> {
    db.nodes_light()
        .unwrap()
        .into_iter()
        .filter(|n| n.status == NodeStatus::Valid && ids.contains(&n.id))
        .map(|n| n.id)
        .collect()
}

/// The checked nodes whose lineage touches `t`.
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

/// `id` and its descendants.
fn subtree(db: &Db, id: NodeId) -> BTreeSet<NodeId> {
    let mut out = BTreeSet::from([id]);
    for n in db.nodes_light().unwrap() {
        if n.parent.is_some_and(|p| out.contains(&p)) {
            out.insert(n.id);
        }
    }
    out
}

/// A script no node started (the last such id). `None` in the stub world (no scripts).
fn untouched_script(db: &Db) -> Option<ScriptId> {
    let s = (0..content::data::SCRIPT_COUNT)
        .rev()
        .map(|i| ScriptId(i as u16))
        .find(|&s| db.touching(Touch::Script(s)).unwrap().is_empty());
    if s.is_none() && !content::STUB {
        panic!("every script was started");
    }
    s
}

/// A valid node other than a root.
fn inner_node(db: &Db) -> NodeId {
    db.nodes_light()
        .unwrap()
        .into_iter()
        .find(|n| n.parent.is_some() && n.status == NodeStatus::Valid)
        .expect("a valid non-root node")
        .id
}

/// A map other than the new game's that some checked nodes but not all are under.
fn partly_touched_map(db: &Db) -> Option<(MapId, BTreeSet<NodeId>)> {
    let start = Emerald::for_shells().new_game().map;
    let all = checked(db).len();
    content::data::MAP_IDS
        .iter()
        .copied()
        .filter(|&m| m != start)
        .map(|m| (m, under(db, Touch::Map(m))))
        .find(|(_, u)| !u.is_empty() && u.len() < all)
}

/// Every `node_touch` row, in order.
fn touch_rows(path: &Path) -> Vec<(i64, i64, i64)> {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut st = conn
        .prepare("SELECT node, kind, id FROM node_touch ORDER BY node, kind, id")
        .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_changed_map_replays_only_the_nodes_under_it_and_its_neighbour() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.db");
    explored(&path);
    let game = Emerald::for_shells();
    let start = game.new_game().map;
    let (maps, want, all) = {
        let db = open(&path);
        let Some((m, mut u)) = partly_touched_map(&db) else {
            // The stub world is one map.
            if !content::STUB {
                panic!("no map some nodes are under");
            }
            return;
        };
        // A map's digest covers its connected maps' layouts (Task E): an edit of the
        // neighbour's layout changes both digests.
        let next = game
            .map(m)
            .connections
            .iter()
            .map(|c| c.map)
            .find(|&c| c != start);
        if let Some(c) = next {
            u.extend(under(&db, Touch::Map(c)));
        }
        let maps: Vec<MapId> = [m].into_iter().chain(next).collect();
        (maps, u, checked(&db).len())
    };
    assert!(want.len() < all, "the neighbour is under every node");
    let r = reopen_after(&path, |d| {
        for m in d.maps.iter_mut().filter(|m| maps.contains(&m.0)) {
            m.2 ^= 1;
        }
    });
    assert_eq!(r.valid as usize, want.len(), "{r:?}");
    assert_eq!(r.kept as usize, all - want.len(), "{r:?}");
    assert_eq!((r.repaired, r.broken), (0, 0));
}

#[test]
fn a_changed_trainer_script_reaches_the_maps_its_trainer_sights_on() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.db");
    explored(&base);
    let game = Emerald::for_shells();
    let (script, map, want) = {
        let db = open(&base);
        let Some(s) = untouched_script(&db) else {
            return;
        };
        let (m, u) = partly_touched_map(&db).expect("a map some nodes are under");
        (s, m, valid(&db, &u))
    };
    assert!(!want.is_empty());
    // The real game has trainers whose script only sight reads on their map.
    let maps: Vec<MapId> = content::data::MAP_IDS.to_vec();
    assert!(!incremental::sighting_maps(&game, &maps).is_empty());
    let template = maps
        .iter()
        .find_map(|&m| game.map(m).objects.first().copied())
        .expect("an object");
    let trainer = TrainerOn {
        inner: game,
        map,
        trainer: ObjectEvent {
            script: Some(script),
            trainer_type: 1,
            ..template
        },
    };
    assert!(incremental::sighting_maps(&trainer, &[map])[&script].contains(&map));
    let edit = |d: &mut Digests| d.scripts[usize::from(script.0)].1 ^= 1;
    // Without the trainer no node is under the script.
    let plain = dir.path().join("plain.db");
    copy_db(&base, &plain);
    let (m, stale) = mark_after(&plain, &game, edit);
    assert_eq!((m.stale, stale.len()), (0, 0), "{m:?}");
    // With it, every valid node under its map.
    let sighted = dir.path().join("sighted.db");
    copy_db(&base, &sighted);
    let (m, stale) = mark_after(&sighted, &trainer, edit);
    assert_eq!(stale, want);
    assert_eq!(m.stale as usize, want.len());
}

#[test]
fn a_node_whose_events_were_dropped_is_under_any_change() {
    let summary = |dropped| SimSummary {
        events_dropped: dropped,
        ..SimSummary::default()
    };
    let map = MapId(1);
    assert!(!incremental::touches(&summary(0), map).contains(&Touch::EventsDropped));
    assert!(incremental::touches(&summary(3), map).contains(&Touch::EventsDropped));

    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.db");
    explored(&base);
    let game = Emerald::for_shells();
    let (script, node, want) = {
        let db = open(&base);
        let Some(s) = untouched_script(&db) else {
            return;
        };
        let n = inner_node(&db);
        (s, n, valid(&db, &subtree(&db, n)))
    };
    let edit = |d: &mut Digests| d.scripts[usize::from(script.0)].1 ^= 1;
    let plain = dir.path().join("plain.db");
    copy_db(&base, &plain);
    assert!(mark_after(&plain, &game, edit).1.is_empty());
    let dropped = dir.path().join("dropped.db");
    copy_db(&base, &dropped);
    rusqlite::Connection::open(&dropped)
        .unwrap()
        .execute(
            "INSERT INTO node_touch (node, kind, id) VALUES (?1, 2, 0)",
            [node],
        )
        .unwrap();
    assert_eq!(mark_after(&dropped, &game, edit).1, want);
}

#[test]
fn a_revalidation_rewrites_the_touches_of_the_nodes_it_checks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.db");
    explored(&path);
    let game = Emerald::for_shells();
    let cfg = config();
    let before = touch_rows(&path);
    // A replay gives the touches the expansion recorded.
    {
        let mut db = open(&path);
        db.mark_all_stale().unwrap();
        let fix = |start: Option<&Checkpoint>, m: &Macro| wire::repair(&game, start, m, &cfg);
        let r = rebuild::revalidate(&mut db, &game, &cfg, &fix).unwrap();
        assert_eq!((r.repaired, r.broken), (0, 0), "{r:?}");
    }
    assert_eq!(touch_rows(&path), before);

    // A repaired node gets the touches of its new path: here one more script, as a path that
    // now starts it.
    let (script, node, want) = {
        let db = open(&path);
        let Some(s) = untouched_script(&db) else {
            return;
        };
        let n = inner_node(&db);
        let sub = subtree(&db, n);
        (s, n, checked(&db).intersection(&sub).count())
    };
    // Its stored digest no longer matches: the replay diverges and the repair runs.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE node SET digest = ~digest, status = 'stale' WHERE id = ?1",
            [node],
        )
        .unwrap();
    {
        let mut db = open(&path);
        let fix = |start: Option<&Checkpoint>, m: &Macro| {
            wire::repair(&game, start, m, &cfg).map(|mut r| {
                r.touches.push(Touch::Script(script));
                r
            })
        };
        let r = rebuild::revalidate(&mut db, &game, &cfg, &fix).unwrap();
        assert_eq!((r.repaired, r.broken), (1, 0), "{r:?}");
        assert_eq!(db.touching(Touch::Script(script)).unwrap(), [node]);
        db.set_meta(incremental::META_KEY, &Digests::current().encode())
            .unwrap();
    }
    // A later change to that script reaches the repaired node and its subtree.
    let r = reopen_after(&path, |d| d.scripts[usize::from(script.0)].1 ^= 1);
    assert_eq!(r.valid as usize, want, "{r:?}");
}
