//! The SQLite store, `target/explore/{stub|emerald}/explore.db` (Task C; the DDL and the
//! connection setup are Wave 0).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::model::{FindingKind, GenerationDelta, Macro, NodeId, NodeRow, SeenKind};
use engine::checkpoint::{BuildHash, Checkpoint};
use engine::data::{MapId, ScriptId};
use game_native::trace::Run;
use rusqlite::Connection;
use std::fmt;
use std::path::{Path, PathBuf};

/// The DDL (`CREATE … IF NOT EXISTS`, idempotent).
pub const SCHEMA_SQL: &str = include_str!("schema.sql");
pub const SCHEMA_VERSION: i64 = 1;

/// Sets the pragmas `Db::open` uses (`journal_mode=WAL; synchronous=NORMAL;
/// foreign_keys=ON`) and runs [`SCHEMA_SQL`].
pub fn init_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA_SQL)
}

#[derive(Debug)]
pub enum DbError {
    Sql(rusqlite::Error),
    Schema { found: i64, want: i64 },
    Flavor { db: String, build: String },
    Codec(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sql(e) => write!(f, "sqlite: {e}"),
            DbError::Schema { found, want } => {
                write!(f, "explore.db schema {found}, this build wants {want}")
            }
            DbError::Flavor { db, build } => {
                write!(
                    f,
                    "explore.db is for the {db} content, this build is {build}"
                )
            }
            DbError::Codec(e) => write!(f, "explore.db codec: {e}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DbError::Sql(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sql(e)
    }
}

pub struct OpenState {
    pub fresh: bool,
    pub build_changed: bool,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    /// `<workspace>/target/explore/{stub|emerald}/explore.db`.
    pub fn default_path(stub: bool) -> PathBuf {
        todo!("Task C")
    }
    pub fn open(path: &Path, build: BuildHash, stub: bool) -> Result<(Db, OpenState), DbError> {
        todo!("Task C")
    }
    pub fn open_read_only(path: &Path) -> Result<Db, DbError> {
        todo!("Task C")
    }
    /// One transaction.
    pub fn apply(&mut self, d: &GenerationDelta) -> Result<(), DbError> {
        todo!("Task C")
    }
    /// Id ascending.
    pub fn nodes(&self) -> Result<Vec<NodeRow>, DbError> {
        todo!("Task C")
    }
    pub fn node(&self, id: NodeId) -> Result<Option<NodeRow>, DbError> {
        todo!("Task C")
    }
    /// Root first.
    pub fn lineage(&self, id: NodeId) -> Result<Vec<NodeRow>, DbError> {
        todo!("Task C")
    }
    pub fn seen(&self) -> Result<Vec<(SeenKind, i64, u32)>, DbError> {
        todo!("Task C")
    }
    pub fn coverage(&self, map: Option<MapId>) -> Result<Vec<(MapId, i16, i16, u32)>, DbError> {
        todo!("Task C")
    }
    pub fn findings(&self) -> Result<Vec<FindingRow>, DbError> {
        todo!("Task C")
    }
    pub fn finding(&self, id: i64) -> Result<Option<FindingRow>, DbError> {
        todo!("Task C")
    }
    pub fn put_snapshot(&mut self, id: NodeId, cp: &Checkpoint) -> Result<(), DbError> {
        todo!("Task C")
    }
    pub fn snapshot(&self, id: NodeId, build: BuildHash) -> Result<Option<Checkpoint>, DbError> {
        todo!("Task C")
    }
    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), DbError> {
        todo!("Task C")
    }
    pub fn meta(&self, key: &str) -> Result<Option<String>, DbError> {
        todo!("Task C")
    }
    pub fn mark_all_stale(&mut self) -> Result<u64, DbError> {
        todo!("Task C")
    }
    /// Revalidation results.
    pub fn set_status(&mut self, rows: &[NodeRow]) -> Result<(), DbError> {
        todo!("Task C")
    }
    pub fn record_run(&mut self, r: &RunRow) -> Result<(), DbError> {
        todo!("Task C")
    }
    pub fn stats(&self) -> Result<DbStats, DbError> {
        todo!("Task C")
    }
    /// FNV of the node, finding, seen and coverage tables in key order, without timings or
    /// run rows (determinism tests, `explore digest`).
    pub fn content_digest(&self) -> Result<u64, DbError> {
        todo!("Task C")
    }
}

pub struct FindingRow {
    pub id: i64,
    pub signature: u64,
    pub kind: FindingKind,
    pub node: NodeId,
    pub macro_: Option<Macro>,
    pub tail: Vec<Run>,
    pub total_frame: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    pub message: String,
    pub location: Option<String>,
    pub script: Option<ScriptId>,
    pub log: String,
    pub build: BuildHash,
    pub status: String,
    pub hits: u32,
    pub first_generation: u32,
    pub last_generation: u32,
}

pub struct RunRow {
    pub started: String,
    pub config: String,
    pub threads: u32,
    pub generations: u32,
    pub nodes: u64,
    pub frames: u64,
    pub wall_ms: u64,
}

pub struct DbStats {
    pub nodes: u64,
    pub valid: u64,
    pub stale: u64,
    pub merged: u64,
    pub broken: u64,
    pub findings: u64,
    pub maps: u32,
    pub cells: u64,
    pub scripts: u32,
    pub max_progress: u32,
    pub generation: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(conn: &Connection) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap();
        st.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn schema_is_idempotent_with_the_pragmas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("explore.db");
        for _ in 0..2 {
            let conn = Connection::open(&path).unwrap();
            init_connection(&conn).unwrap();
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "wal");
            let fk: i64 = conn
                .pragma_query_value(None, "foreign_keys", |r| r.get(0))
                .unwrap();
            assert_eq!(fk, 1);
            let sync: i64 = conn
                .pragma_query_value(None, "synchronous", |r| r.get(0))
                .unwrap();
            assert_eq!(sync, 1, "NORMAL");
            assert_eq!(
                tables(&conn),
                [
                    "battle",
                    "coverage",
                    "finding",
                    "meta",
                    "node",
                    "node_touch",
                    "run",
                    "seen",
                    "snapshot"
                ]
            );
        }
    }

    #[test]
    fn strict_tables_and_the_live_key_index() {
        let conn = Connection::open_in_memory().unwrap();
        init_connection(&conn).unwrap();
        let insert = |id: i64, key: i64, status: &str| {
            conn.execute(
                "INSERT INTO node (id, parent, depth, macro, inputs, frames, total_frames, \
                 end_frame, key, region, digest, world_hash, map, x, y, progress, f_novelty, \
                 f_progress, f_bug, f_balance, f_total, generation, status, build_engine, \
                 build_content) VALUES (?1, NULL, 0, 'Settle', '', 0, 0, 0, ?2, 0, \
                 -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ?3, 1, 2)",
                rusqlite::params![id, key, status],
            )
        };
        insert(1, 7, "valid").unwrap();
        assert!(insert(2, 7, "stale").is_err(), "one live node per key");
        insert(3, 7, "merged").unwrap();
        assert!(insert(4, 8, "bogus").is_err(), "status check");
        // STRICT: a text in an INTEGER column is rejected.
        assert!(
            conn.execute(
                "INSERT INTO snapshot (node, build_engine, build_content, bytes) \
                 VALUES (1, 'x', 0, x'00')",
                []
            )
            .is_err()
        );
        // foreign keys are enforced.
        assert!(
            conn.execute(
                "INSERT INTO seen (kind, id, first_node) VALUES (0, 1, 99)",
                []
            )
            .is_err()
        );
        conn.execute(
            "INSERT INTO seen (kind, id, first_node) VALUES (0, 1, 1)",
            [],
        )
        .unwrap();
        // u64 values round trip bit-cast.
        let d: i64 = conn
            .query_row("SELECT digest FROM node WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(d as u64, u64::MAX);
    }

    #[test]
    fn errors_display() {
        let e = DbError::Schema { found: 2, want: 1 };
        assert!(e.to_string().contains('2'));
        let e: DbError = rusqlite::Error::InvalidQuery.into();
        assert!(matches!(e, DbError::Sql(_)));
        assert!(std::error::Error::source(&e).is_some());
    }
}
