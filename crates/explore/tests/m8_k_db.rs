//! Milestone 8 Task K: schema v3 (blob inputs, integer macros, the `key_hit` table, no region
//! column), the key version, and the refusal of older dbs.

mod c_common;

use c_common::row;
use engine::Buttons;
use engine::checkpoint::BuildHash;
use explore::config::{Budget, Config};
use explore::db::{Db, DbError, SCHEMA_VERSION};
use explore::key::KEY_VERSION;
use explore::wire::Explore;
use explore::{AbstractKey, GenerationDelta, NodeStatus, runs};
use game_native::trace::Run;
use rusqlite::Connection;
use std::path::Path;

const B1: BuildHash = BuildHash {
    engine: 7,
    content: 8,
};

#[test]
fn the_schema_is_3_and_fresh_dbs_record_the_key_version() {
    assert_eq!(SCHEMA_VERSION, 3);
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = Db::open(&dir.path().join("explore.db"), B1, content::Flavor::Stub).unwrap();
    assert_eq!(
        db.meta("key_version").unwrap(),
        Some(KEY_VERSION.to_string())
    );
}

#[test]
fn a_schema_2_db_is_refused_with_the_reset_hint() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); \
             INSERT INTO meta VALUES ('schema', '2'), ('flavor', 'stub');",
        )
        .unwrap();
    }
    let e = Db::open(&path, B1, content::Flavor::Stub)
        .err()
        .expect("refused");
    assert!(matches!(e, DbError::Schema { found: 2, want: 3 }), "{e}");
    assert!(e.to_string().contains("explore reset --yes"), "{e}");
    assert!(matches!(
        Db::open_read_only(&path),
        Err(DbError::Schema { found: 2, .. })
    ));
}

#[test]
fn another_key_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (mut db, _) = Db::open(&path, B1, content::Flavor::Stub).unwrap();
    db.set_meta("key_version", "1").unwrap();
    drop(db);
    let e = Db::open(&path, B1, content::Flavor::Stub)
        .err()
        .expect("refused");
    assert!(
        matches!(&e, DbError::KeyVersion { found, want } if found == "1" && *want == KEY_VERSION),
        "{e}"
    );
    assert!(e.to_string().contains("explore reset --yes"), "{e}");
    assert!(matches!(
        Db::open_read_only(&path),
        Err(DbError::KeyVersion { .. })
    ));
}

/// Inputs with a repeated pair of runs (a mashed text box).
fn mashed() -> Vec<Run> {
    let r = |keys, frames| Run { keys, frames };
    let mut v = vec![r(Buttons::UP, 16)];
    for _ in 0..40 {
        v.push(r(Buttons::A, 1));
        v.push(r(Buttons::NONE, 7));
    }
    v.push(r(Buttons::NONE, 300));
    v
}

#[test]
fn inputs_are_blobs_macros_integers_and_light_rows_skip_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let (mut db, _) = Db::open(&path, B1, content::Flavor::Emerald).unwrap();
    let mut n1 = row(1, None, 10, B1);
    n1.inputs = mashed();
    let n2 = row(2, Some(1), 20, B1);
    db.apply(&GenerationDelta {
        generation: 1,
        nodes: vec![n1.clone(), n2.clone()],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(db.nodes().unwrap(), [n1.clone(), n2.clone()]);
    assert_eq!(db.inputs(1).unwrap(), Some(n1.inputs.clone()));
    assert_eq!(db.inputs(3).unwrap(), None);
    let light = db.nodes_light().unwrap();
    for (l, n) in light.iter().zip([&n1, &n2]) {
        assert!(l.inputs.is_empty());
        let mut l = l.clone();
        l.inputs = n.inputs.clone();
        assert_eq!(&l, n);
    }
    drop(db);

    let conn = Connection::open(&path).unwrap();
    let (inputs, macro_, len): (String, String, i64) = conn
        .query_row(
            "SELECT typeof(inputs), typeof(macro), length(inputs) FROM node WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((inputs.as_str(), macro_.as_str()), ("blob", "integer"));
    // The 80 alternating runs are one repeat pair.
    assert!(len < 12, "{len} bytes");
    assert!((len as usize) * 10 < runs::to_text(&n1.inputs).len());
    let cols: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('node')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!cols.iter().any(|c| c == "region"), "{cols:?}");
}

#[test]
fn key_hits_add_up_and_are_content() {
    let (mut db, _) = Db::open_in_memory(B1, content::Flavor::Stub).unwrap();
    db.apply(&GenerationDelta {
        generation: 1,
        nodes: vec![row(1, None, 10, B1)],
        key_hits: vec![(AbstractKey(10), 2), (AbstractKey(99), 1)],
        ..Default::default()
    })
    .unwrap();
    let before = db.content_digest().unwrap();
    db.apply(&GenerationDelta {
        generation: 2,
        key_hits: vec![(AbstractKey(10), 3)],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        db.key_hits().unwrap(),
        [(AbstractKey(10), 5), (AbstractKey(99), 1)]
    );
    assert_ne!(db.content_digest().unwrap(), before);
}

/// Page bytes in use (the file without its free pages).
fn used_bytes(conn: &Connection) -> i64 {
    let q = |p: &str| -> i64 { conn.pragma_query_value(None, p, |r| r.get(0)).unwrap() };
    (q("page_count") - q("freelist_count")) * q("page_size")
}

/// `budget` of the default config on a fresh db at `path`; its rows (with inputs).
fn explore_rows(path: &Path, budget: Budget) -> Vec<explore::NodeRow> {
    let mut ex = Explore::open(content::Emerald::for_shells(), path, Config::default()).unwrap();
    ex.run(budget, |_| {}).unwrap();
    drop(ex);
    Db::open_read_only(path).unwrap().nodes().unwrap()
}

#[test]
fn a_run_keeps_no_childless_duplicate_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    // The stub's 196 cells hold about 32 keys: 30 generations saturate them.
    let nodes = explore_rows(
        &path,
        Budget::Generations(if content::STUB { 30 } else { 3 }),
    );
    // A duplicate's row (merged into an older live node) is kept only as a parent; a
    // superseded row (merged into a newer one) stays, as it was live and written.
    let duplicates = nodes
        .iter()
        .filter(|m| m.status == NodeStatus::Merged)
        .filter(|m| m.merged_into.is_some_and(|into| into < m.id))
        .filter(|m| !nodes.iter().any(|n| n.parent == Some(m.id)))
        .count();
    assert_eq!(duplicates, 0, "a duplicate's row is kept only as a parent");
    let db = Db::open_read_only(&path).unwrap();
    let hits: u32 = db.key_hits().unwrap().iter().map(|&(_, h)| h).sum();
    assert!(
        hits as usize > nodes.len(),
        "duplicates count hits, not rows"
    );
    // Blob inputs against M7's text codec.
    let blob: usize = nodes.iter().map(|n| runs::to_blob(&n.inputs).len()).sum();
    let text: usize = nodes.iter().map(|n| runs::to_text(&n.inputs).len()).sum();
    // The 40% target was set on the stub's and Emerald's runs; Silice's Wave 0 maps (no
    // scripts) measured 43% (SIL1 Task D), so it only has to beat the text codec until its
    // Prologue is built.
    if content::FLAVOR != content::Flavor::Silice {
        assert!(blob * 10 <= text * 4, "blob {blob} B, text {text} B");
    }
    assert!(blob < text, "blob {blob} B, text {text} B");
}

/// The size target (0.3 MB per 1,000 rows; M7: 1.05 MB) on 2,000 inserted nodes of the
/// extracted build. About 85 s of CPU in debug: run it in release,
/// `cargo test --release -p explore --test m8_k_db -- --ignored`.
#[test]
#[ignore = "slow in debug: run in release with --ignored"]
fn the_real_build_keeps_rows_under_300_bytes() {
    if content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let nodes = explore_rows(&path, Budget::Nodes(2000));
    let conn = Connection::open(&path).unwrap();
    let per_row = used_bytes(&conn) / nodes.len() as i64;
    eprintln!("{} rows, {per_row} B per row", nodes.len());
    assert!(nodes.len() >= 1000, "{} rows", nodes.len());
    assert!(per_row <= 300, "{per_row} B per row");
}
