//! Task C: the SQLite store (temp dirs and in-memory dbs, synthetic rows).

mod c_common;

use c_common::{row, walk};
use engine::Buttons;
use engine::checkpoint::BuildHash;
use engine::data::{MapId, ScriptId};
use explore::db::{Db, DbError, RunRow, SCHEMA_VERSION};
use explore::{
    AbstractKey, BattleSummary, FindingDraft, FindingKind, GenerationDelta, NodeStatus, SeenKind,
    Touch,
};
use game_native::trace::rle;

const B1: BuildHash = BuildHash {
    engine: 0xDEAD_BEEF_0000_0001,
    content: u64::MAX,
};
const B2: BuildHash = BuildHash {
    engine: 2,
    content: 3,
};

fn draft(msg: &str) -> FindingDraft {
    FindingDraft {
        kind: FindingKind::Panic,
        message: msg.to_string(),
        location: Some("src/x.rs:1:2".to_string()),
        macro_: Some(walk(3, 4)),
        tail: rle([Buttons::A, Buttons::NONE, Buttons::NONE]),
        map: MapId(1),
        pos: (3, 4),
        script: Some(ScriptId(9)),
        log: vec!["one", "two"],
    }
}

fn gen1() -> GenerationDelta {
    let mut n2 = row(2, Some(1), 20, B1);
    n2.depth = 1;
    let mut n3 = row(3, Some(2), 30, B1);
    n3.depth = 2;
    GenerationDelta {
        generation: 1,
        nodes: vec![row(1, None, 10, B1), n2, n3],
        touches: vec![
            (1, Touch::Map(MapId(1))),
            (2, Touch::Script(ScriptId(5))),
            (2, Touch::Script(ScriptId(5))),
        ],
        seen: vec![
            (SeenKind::Map, 1, 1),
            (SeenKind::Script, 5, 2),
            (SeenKind::Flag, 0x50, 2),
        ],
        cells: vec![
            (MapId(1), 1, -1, 1),
            (MapId(1), 2, -1, 2),
            (MapId(2), 0, 0, 3),
        ],
        battles: vec![(
            3,
            BattleSummary {
                kind: 0,
                trainer: None,
                outcome: Some(1),
                frames: 400,
                player_level: 5,
                opponent_level: 3,
                hp_lost_permille: 250,
            },
        )],
        findings: vec![(draft("boom 12"), 3, 9)],
        ..Default::default()
    }
}

#[test]
fn open_creates_dirs_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/explore.db");
    let (db, st) = Db::open(&path, B1, true).unwrap();
    assert!(st.fresh && !st.build_changed);
    assert_eq!(db.meta("schema").unwrap(), Some(SCHEMA_VERSION.to_string()));
    assert_eq!(db.meta("flavor").unwrap().as_deref(), Some("stub"));
    assert_eq!(db.meta_build().unwrap(), Some(B1));
    assert_eq!(db.meta("generation").unwrap().as_deref(), Some("0"));
    drop(db);
    let (_, st) = Db::open(&path, B1, true).unwrap();
    assert!(!st.fresh && !st.build_changed);
    let (db, st) = Db::open(&path, B2, true).unwrap();
    assert!(!st.fresh && st.build_changed);
    // Opening does not move the recorded build.
    assert_eq!(db.meta_build().unwrap(), Some(B1));
    assert_eq!(db.build(), B2);
}

#[test]
fn flavor_and_schema_mismatches() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (mut db, _) = Db::open(&path, B1, true).unwrap();
    drop(Db::open(&path, B1, true).unwrap());
    assert!(matches!(
        Db::open(&path, B1, false),
        Err(DbError::Flavor { db, build }) if db == "stub" && build == "emerald"
    ));
    db.set_meta("schema", "99").unwrap();
    assert!(matches!(
        Db::open(&path, B1, true),
        Err(DbError::Schema {
            found: 99,
            want: SCHEMA_VERSION
        })
    ));
    assert!(matches!(
        Db::open_read_only(&path),
        Err(DbError::Schema { found: 99, .. })
    ));
}

#[test]
fn apply_round_trip() {
    let (mut db, _) = Db::open_in_memory(B1, false).unwrap();
    let d = gen1();
    db.apply(&d).unwrap();
    assert_eq!(db.nodes().unwrap(), d.nodes);
    assert_eq!(db.node(2).unwrap().as_ref(), Some(&d.nodes[1]));
    assert_eq!(db.node(9).unwrap(), None);
    let lineage: Vec<_> = db.lineage(3).unwrap().iter().map(|n| n.id).collect();
    assert_eq!(lineage, [1, 2, 3]);
    assert!(db.lineage(9).unwrap().is_empty());
    assert_eq!(
        db.seen().unwrap(),
        [
            (SeenKind::Flag, 0x50, 1),
            (SeenKind::Map, 1, 1),
            (SeenKind::Script, 5, 1)
        ]
    );
    assert_eq!(
        db.coverage(None).unwrap(),
        [
            (MapId(1), 1, -1, 1),
            (MapId(1), 2, -1, 1),
            (MapId(2), 0, 0, 1)
        ]
    );
    assert_eq!(db.coverage(Some(MapId(2))).unwrap(), [(MapId(2), 0, 0, 1)]);
    let f = db.findings().unwrap();
    assert_eq!(f.len(), 1);
    let f = &f[0];
    assert_eq!(db.finding(f.id).unwrap().as_ref(), Some(f));
    let want = draft("boom 12");
    assert_eq!(
        (f.kind, f.node, f.macro_, &f.tail, f.total_frame),
        (want.kind, 3, want.macro_, &want.tail, 9)
    );
    assert_eq!((f.map, f.pos, f.script), (want.map, want.pos, want.script));
    assert_eq!(
        (f.message.as_str(), f.location.as_deref()),
        ("boom 12", want.location.as_deref())
    );
    assert_eq!(f.log, "one\ntwo");
    assert_eq!((f.build, f.status.as_str(), f.hits), (B1, "open", 1));
    assert_eq!((f.first_generation, f.last_generation), (1, 1));
    let s = db.stats().unwrap();
    assert_eq!(
        (s.nodes, s.valid, s.stale, s.merged, s.broken),
        (3, 3, 0, 0, 0)
    );
    assert_eq!((s.findings, s.maps, s.cells, s.scripts), (1, 1, 3, 1));
    assert_eq!((s.max_progress, s.generation), (3, 1));
}

#[test]
fn merged_before_the_live_key_insert() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    db.apply(&gen1()).unwrap();
    // Same key as node 2 without the merge: the live-key index refuses, nothing is applied.
    let mut n4 = row(4, Some(1), 20, B1);
    n4.generation = 2;
    let mut d = GenerationDelta {
        generation: 2,
        nodes: vec![n4.clone()],
        expanded: vec![(1, 2)],
        ..Default::default()
    };
    assert!(db.apply(&d).is_err());
    assert_eq!(db.nodes().unwrap().len(), 3);
    assert_eq!(db.node(1).unwrap().unwrap().expanded, 0);
    assert_eq!(db.meta("generation").unwrap().as_deref(), Some("1"));
    d.merged = vec![(2, 4)];
    db.apply(&d).unwrap();
    let n2 = db.node(2).unwrap().unwrap();
    assert_eq!((n2.status, n2.merged_into), (NodeStatus::Merged, Some(4)));
    assert_eq!(db.node(4).unwrap(), Some(n4));
    assert_eq!(db.node(1).unwrap().unwrap().expanded, 2);
    assert_eq!(db.stats().unwrap().merged, 1);
}

#[test]
fn upserts_count_hits() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    db.apply(&gen1()).unwrap();
    // The same finding (digits differ in the message, same location) and sightings again.
    let d = GenerationDelta {
        generation: 4,
        nodes: vec![row(5, Some(1), 50, B1)],
        seen: vec![(SeenKind::Map, 1, 5)],
        cells: vec![(MapId(1), 1, -1, 5)],
        findings: vec![(draft("boom 13"), 5, 11)],
        ..Default::default()
    };
    db.apply(&d).unwrap();
    let f = &db.findings().unwrap()[0];
    assert_eq!((f.hits, f.first_generation, f.last_generation), (2, 1, 4));
    // An open finding keeps its first repro.
    assert_eq!(
        (f.node, f.total_frame, f.message.as_str()),
        (3, 9, "boom 12")
    );
    assert!(db.seen().unwrap().contains(&(SeenKind::Map, 1, 2)));
    assert_eq!(
        db.coverage(Some(MapId(1))).unwrap()[0],
        (MapId(1), 1, -1, 2)
    );

    // Another build: open findings of other builds go stale, and a new sighting reopens one
    // with the new repro.
    assert_eq!(db.set_build(B2).unwrap(), 1);
    assert_eq!(db.findings().unwrap()[0].status, "stale");
    let d = GenerationDelta {
        generation: 5,
        findings: vec![(draft("boom 14"), 5, 12)],
        ..Default::default()
    };
    db.apply(&d).unwrap();
    let f = &db.findings().unwrap()[0];
    assert_eq!((f.status.as_str(), f.hits, f.build), ("open", 3, B2));
    assert_eq!(
        (f.node, f.total_frame, f.message.as_str()),
        (5, 12, "boom 14")
    );
}

#[test]
fn content_digest_ignores_run_rows() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    let empty = db.content_digest().unwrap();
    db.apply(&gen1()).unwrap();
    let a = db.content_digest().unwrap();
    assert_ne!(a, empty);
    db.record_run(&RunRow {
        started: "now".into(),
        config: "()".into(),
        threads: 4,
        generations: 1,
        nodes: 3,
        frames: 9,
        wall_ms: 12,
    })
    .unwrap();
    db.set_meta("seed", "1").unwrap();
    assert_eq!(db.content_digest().unwrap(), a);
    // A second db with the same content has the same digest.
    let (mut db2, _) = Db::open_in_memory(B1, true).unwrap();
    db2.apply(&gen1()).unwrap();
    assert_eq!(db2.content_digest().unwrap(), a);
    db2.apply(&GenerationDelta {
        generation: 2,
        expanded: vec![(3, 1)],
        ..Default::default()
    })
    .unwrap();
    assert_ne!(db2.content_digest().unwrap(), a);
}

#[test]
fn status_updates_and_mark_all_stale() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    db.apply(&gen1()).unwrap();
    assert_eq!(db.mark_all_stale().unwrap(), 3);
    assert_eq!(db.stats().unwrap().stale, 3);
    // Node 3 breaks, node 2 takes node 3's key in the same batch (written after it).
    let mut n2 = db.node(2).unwrap().unwrap();
    let mut n3 = db.node(3).unwrap().unwrap();
    n3.status = NodeStatus::Broken;
    n2.status = NodeStatus::Valid;
    n2.key = n3.key;
    n2.inputs = rle([Buttons::B; 5]);
    n2.frames = 5;
    n2.build = B2;
    db.set_status(&[n2.clone(), n3.clone()]).unwrap();
    assert_eq!(db.node(2).unwrap(), Some(n2));
    assert_eq!(db.node(3).unwrap(), Some(n3));
}

#[test]
fn read_only_while_a_writer_holds_the_db() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (mut w, _) = Db::open(&path, B1, false).unwrap();
    w.apply(&gen1()).unwrap();
    let mut r = Db::open_read_only(&path).unwrap();
    assert_eq!(r.build(), B1);
    assert_eq!(r.nodes().unwrap().len(), 3);
    // The writer keeps writing; the reader sees the commit.
    w.apply(&GenerationDelta {
        generation: 2,
        nodes: vec![row(4, Some(1), 40, B1)],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(r.stats().unwrap().nodes, 4);
    assert!(r.set_meta("x", "y").is_err(), "read-only");
}

#[test]
fn default_path_is_under_target_explore() {
    let p = Db::default_path(true);
    assert!(
        p.ends_with("target/explore/stub/explore.db"),
        "{}",
        p.display()
    );
    assert!(Db::default_path(false).ends_with("target/explore/emerald/explore.db"));
}

#[test]
fn a_forward_merged_into_inside_one_delta() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    // Node 2 is inserted already merged into node 3, which comes later in the same delta.
    let mut n2 = row(2, Some(1), 20, B1);
    n2.status = NodeStatus::Merged;
    n2.merged_into = Some(3);
    let d = GenerationDelta {
        generation: 1,
        nodes: vec![row(1, None, 10, B1), n2, row(3, Some(1), 20, B1)],
        ..Default::default()
    };
    db.apply(&d).unwrap();
    assert_eq!(db.nodes().unwrap(), d.nodes);
    // A dangling reference is still refused at commit.
    let mut n4 = row(4, Some(1), 40, B1);
    n4.merged_into = Some(99);
    let bad = GenerationDelta {
        generation: 2,
        nodes: vec![n4],
        ..Default::default()
    };
    assert!(db.apply(&bad).is_err());
    assert_eq!(db.nodes().unwrap().len(), 3);
}

#[test]
fn key_hits_persist() {
    let (mut db, _) = Db::open_in_memory(B1, true).unwrap();
    db.apply(&gen1()).unwrap();
    let a = db.content_digest().unwrap();
    assert!(db.key_hits().unwrap().is_empty());
    let hits = [
        (AbstractKey(5), 2),
        (AbstractKey(u64::MAX), 7),
        (AbstractKey(9), 0),
    ];
    db.set_key_hits(&hits).unwrap();
    assert_eq!(
        db.key_hits().unwrap(),
        [(AbstractKey(5), 2), (AbstractKey(u64::MAX), 7)]
    );
    assert_ne!(db.content_digest().unwrap(), a, "key hits are search state");
    // A replacement, not an accumulation.
    db.set_key_hits(&[(AbstractKey(5), 3)]).unwrap();
    assert_eq!(db.key_hits().unwrap(), [(AbstractKey(5), 3)]);
}
