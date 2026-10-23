//! The SQLite store, `target/explore/{stub|emerald}/explore.db` (Task C; the DDL and the
//! connection setup are Wave 0).

use crate::model::{
    AbstractKey, FindingKind, Fitness, GenerationDelta, Macro, NodeId, NodeRow, NodeStatus,
    SeenKind, Touch,
};
use crate::runs;
use engine::checkpoint::{self, BuildHash, Checkpoint};
use engine::data::{MapId, ScriptId};
use game_native::trace::Run;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// The DDL (`CREATE … IF NOT EXISTS`, idempotent).
pub const SCHEMA_SQL: &str = include_str!("schema.sql");
/// 2: `finding.node` is nullable (a tail from a new game). 3 (Milestone 8): `node.inputs` is a
/// binary blob (`runs::to_blob`), `node.macro` an integer (`Macro::code`), `node.region` is
/// gone, `key_hit` counts the duplicates that leave no row, and `meta` records `key_version`,
/// `act` and `root`.
pub const SCHEMA_VERSION: i64 = 3;

/// How long a connection waits on a lock held by another (writer or reader).
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

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
    Schema {
        found: i64,
        want: i64,
    },
    /// The db's keys were folded by another `key::KEY_VERSION`.
    KeyVersion {
        found: String,
        want: u32,
    },
    Flavor {
        db: String,
        build: String,
    },
    Codec(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sql(e) => write!(f, "sqlite: {e}"),
            DbError::Schema { found, want } => {
                write!(
                    f,
                    "explore.db schema {found}, this build wants {want} (explore dbs are caches: \
                     run `explore reset --yes`)"
                )
            }
            DbError::KeyVersion { found, want } => write!(
                f,
                "explore.db keys are of key version {found}, this build folds version {want} \
                 (explore dbs are caches: run `explore reset --yes`)"
            ),
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

/// The explorer database: one writer (the explorer's main thread), any number of read-only
/// readers (`open_read_only`, WAL).
pub struct Db {
    conn: Connection,
    /// The build this process runs (`open`), or the db's recorded build (`open_read_only`).
    /// New findings are stamped with it.
    build: BuildHash,
}

const NODE_COLS: &str = "id, parent, depth, macro, inputs, frames, total_frames, end_frame, \
     key, digest, world_hash, map, x, y, progress, f_novelty, f_progress, f_bug, f_balance, \
     f_total, generation, expanded, status, merged_into, build_engine, build_content";

/// [`NODE_COLS`] with an empty blob for the inputs (rows without inputs, `Db::nodes_light`).
const NODE_COLS_LIGHT: &str = "id, parent, depth, macro, x'', frames, total_frames, end_frame, \
     key, digest, world_hash, map, x, y, progress, f_novelty, f_progress, f_bug, f_balance, \
     f_total, generation, expanded, status, merged_into, build_engine, build_content";

const FINDING_COLS: &str = "id, signature, kind, node, macro, tail, total_frame, map, x, y, \
     message, location, script, log, build_engine, build_content, status, hits, \
     first_generation, last_generation";

fn flavor_name(stub: bool) -> &'static str {
    if stub { "stub" } else { "emerald" }
}

fn status_str(s: NodeStatus) -> &'static str {
    match s {
        NodeStatus::Valid => "valid",
        NodeStatus::Stale => "stale",
        NodeStatus::Merged => "merged",
        NodeStatus::Broken => "broken",
    }
}

fn parse_status(s: &str) -> Result<NodeStatus, DbError> {
    Ok(match s {
        "valid" => NodeStatus::Valid,
        "stale" => NodeStatus::Stale,
        "merged" => NodeStatus::Merged,
        "broken" => NodeStatus::Broken,
        _ => return Err(DbError::Codec(format!("bad node status {s:?}"))),
    })
}

const FINDING_KINDS: [FindingKind; 7] = [
    FindingKind::Panic,
    FindingKind::Softlock,
    FindingKind::RoundTrip,
    FindingKind::CapturableStuck,
    FindingKind::PlannerDivergence,
    FindingKind::SlowFrame,
    FindingKind::Unsupported,
];

fn kind_str(k: FindingKind) -> String {
    format!("{k:?}")
}

fn parse_kind(s: &str) -> Result<FindingKind, DbError> {
    FINDING_KINDS
        .into_iter()
        .find(|k| kind_str(*k) == s)
        .ok_or_else(|| DbError::Codec(format!("bad finding kind {s:?}")))
}

fn parse_seen(k: i64) -> Result<SeenKind, DbError> {
    Ok(match k {
        0 => SeenKind::Flag,
        1 => SeenKind::VarValue,
        2 => SeenKind::Map,
        3 => SeenKind::Script,
        _ => return Err(DbError::Codec(format!("bad seen kind {k}"))),
    })
}

fn macro_text(m: &Macro) -> Result<String, DbError> {
    ron::to_string(m).map_err(|e| DbError::Codec(format!("macro: {e}")))
}

fn parse_macro(s: &str) -> Result<Macro, DbError> {
    ron::from_str(s).map_err(|e| DbError::Codec(format!("macro {s:?}: {e}")))
}

fn parse_runs(s: &str) -> Result<Vec<Run>, DbError> {
    runs::from_text(s).map_err(DbError::Codec)
}

fn parse_blob(b: &[u8]) -> Result<Vec<Run>, DbError> {
    runs::from_blob(b).map_err(DbError::Codec)
}

fn touch_cols(t: &Touch) -> (i64, i64) {
    match t {
        Touch::Map(m) => (0, i64::from(m.0)),
        Touch::Script(s) => (1, i64::from(s.0)),
        Touch::EventsDropped => (2, 0),
    }
}

fn node_from_row(r: &Row) -> Result<NodeRow, DbError> {
    let code: i64 = r.get(3)?;
    let status: String = r.get(22)?;
    Ok(NodeRow {
        id: r.get(0)?,
        parent: r.get(1)?,
        depth: r.get(2)?,
        macro_: Macro::from_code(code)
            .ok_or_else(|| DbError::Codec(format!("bad macro code {code:#x}")))?,
        inputs: match r.get_ref(4)? {
            ValueRef::Blob(b) => parse_blob(b)?,
            v => return Err(DbError::Codec(format!("node inputs: {:?}", v.data_type()))),
        },
        frames: r.get(5)?,
        total_frames: r.get::<_, i64>(6)? as u64,
        end_frame: r.get(7)?,
        key: AbstractKey(r.get::<_, i64>(8)? as u64),
        digest: r.get::<_, i64>(9)? as u64,
        world_hash: r.get::<_, i64>(10)? as u64,
        map: MapId(r.get(11)?),
        pos: (r.get(12)?, r.get(13)?),
        progress: r.get(14)?,
        fitness: Fitness {
            novelty: r.get(15)?,
            progress: r.get(16)?,
            bug: r.get(17)?,
            balance: r.get(18)?,
            total: r.get(19)?,
        },
        generation: r.get(20)?,
        expanded: r.get(21)?,
        status: parse_status(&status)?,
        merged_into: r.get(23)?,
        build: BuildHash {
            engine: r.get::<_, i64>(24)? as u64,
            content: r.get::<_, i64>(25)? as u64,
        },
    })
}

fn finding_from_row(r: &Row) -> Result<FindingRow, DbError> {
    let kind: String = r.get(2)?;
    let macro_: Option<String> = r.get(4)?;
    let tail: String = r.get(5)?;
    Ok(FindingRow {
        id: r.get(0)?,
        signature: r.get::<_, i64>(1)? as u64,
        kind: parse_kind(&kind)?,
        node: r.get(3)?,
        macro_: macro_.as_deref().map(parse_macro).transpose()?,
        tail: parse_runs(&tail)?,
        total_frame: r.get::<_, i64>(6)? as u64,
        map: MapId(r.get(7)?),
        pos: (r.get(8)?, r.get(9)?),
        message: r.get(10)?,
        location: r.get(11)?,
        script: r.get::<_, Option<u16>>(12)?.map(ScriptId),
        log: r.get(13)?,
        build: BuildHash {
            engine: r.get::<_, i64>(14)? as u64,
            content: r.get::<_, i64>(15)? as u64,
        },
        status: r.get(16)?,
        hits: r.get(17)?,
        first_generation: r.get(18)?,
        last_generation: r.get(19)?,
    })
}

impl Db {
    /// `<workspace>/target/explore/{stub|emerald|silice}/explore.db` (`content::Flavor::name`;
    /// the shells pass `content::FLAVOR`).
    pub fn default_path(flavor: content::Flavor) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/explore is two levels below the workspace");
        workspace
            .join("target")
            .join("explore")
            .join(flavor.name())
            .join("explore.db")
    }

    /// Opens (creating the parent dirs and the schema) the db for `build` and the `stub` or
    /// emerald flavor. A fresh db gets `meta` schema, flavor and build; `generation`
    /// appears with the first `apply` (the caller records `seed` with `set_meta`). `build_changed` is reported, never acted on.
    pub fn open(path: &Path, build: BuildHash, stub: bool) -> Result<(Db, OpenState), DbError> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)
                .map_err(|e| DbError::Codec(format!("creating {}: {e}", dir.display())))?;
        }
        Self::init(Connection::open(path)?, build, stub)
    }

    /// An in-memory db, as [`Db::open`] (tests).
    pub fn open_in_memory(build: BuildHash, stub: bool) -> Result<(Db, OpenState), DbError> {
        Self::init(Connection::open_in_memory()?, build, stub)
    }

    fn init(conn: Connection, build: BuildHash, stub: bool) -> Result<(Db, OpenState), DbError> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let flavor = flavor_name(stub);
        // Refuse another schema or flavor before touching the file (no DDL, no WAL switch).
        let has_meta: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'meta')",
            [],
            |r| r.get(0),
        )?;
        let mut db = Db { conn, build };
        let existing = if has_meta { db.meta("schema")? } else { None };
        if let Some(v) = &existing {
            check_schema(v)?;
            let db_flavor = db.meta("flavor")?.unwrap_or_default();
            if db_flavor != flavor {
                return Err(DbError::Flavor {
                    db: db_flavor,
                    build: flavor.to_string(),
                });
            }
            check_key_version(db.meta("key_version")?)?;
        }
        init_connection(&db.conn)?;
        let state = match existing {
            None => {
                let tx = db.conn.transaction()?;
                for (k, v) in [
                    ("schema", SCHEMA_VERSION.to_string()),
                    ("flavor", flavor.to_string()),
                    ("build_engine", build.engine.to_string()),
                    ("build_content", build.content.to_string()),
                    ("key_version", crate::key::KEY_VERSION.to_string()),
                ] {
                    tx.execute(
                        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                        params![k, v],
                    )?;
                }
                tx.commit()?;
                OpenState {
                    fresh: true,
                    build_changed: false,
                }
            }
            Some(_) => OpenState {
                fresh: false,
                build_changed: db.meta_build()? != Some(build),
            },
        };
        Ok((db, state))
    }

    /// A reader (`SQLITE_OPEN_READ_ONLY`) that works while a writer holds the db (WAL). Its
    /// `build` is the db's recorded build.
    pub fn open_read_only(path: &Path) -> Result<Db, DbError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let mut db = Db {
            conn,
            build: BuildHash::default(),
        };
        match db.meta("schema")? {
            None => {
                return Err(DbError::Schema {
                    found: 0,
                    want: SCHEMA_VERSION,
                });
            }
            Some(v) => check_schema(&v)?,
        }
        check_key_version(db.meta("key_version")?)?;
        db.build = db.meta_build()?.unwrap_or_default();
        Ok(db)
    }

    /// The build new findings are stamped with.
    pub fn build(&self) -> BuildHash {
        self.build
    }

    /// `meta.build_*`.
    pub fn meta_build(&self) -> Result<Option<BuildHash>, DbError> {
        let parse = |k: &str| -> Result<Option<u64>, DbError> {
            self.meta(k)?
                .map(|v| {
                    v.parse::<u64>()
                        .map_err(|_| DbError::Codec(format!("meta {k} = {v:?}")))
                })
                .transpose()
        };
        Ok(match (parse("build_engine")?, parse("build_content")?) {
            (Some(engine), Some(content)) => Some(BuildHash { engine, content }),
            _ => None,
        })
    }

    /// One transaction: merged, expanded, nodes, snapshots, touches, seen, coverage, battles,
    /// findings, `meta.generation`.
    pub fn apply(&mut self, d: &GenerationDelta) -> Result<(), DbError> {
        let build = self.build;
        let tx = self.conn.transaction()?;
        // Foreign keys stay immediate (Task G). With deferred keys, a `merged_into` naming a
        // later row of the delta left the deferred-violation counter non-zero, and while it is,
        // SQLite scans the child tables of every row inserted into `node` (`seen`,
        // `coverage`, `battle` and `node.merged_into` have no index on their node column):
        // a cost that grew with the db. So the `merged_into`s naming a row of this delta are
        // written after the inserts, and no violation is ever pending.
        let mut late_merged_into: Vec<(NodeId, NodeId)> = Vec::new();
        {
            // The old live nodes leave the live key index before the inserts take their keys.
            let mut st = tx.prepare("UPDATE node SET status = 'merged' WHERE id = ?1")?;
            for (old, new) in &d.merged {
                if st.execute(params![old])? != 1 {
                    return Err(DbError::Codec(format!("merged: no node {old}")));
                }
                late_merged_into.push((*old, *new));
            }
            let mut st = tx.prepare("UPDATE node SET expanded = expanded + ?2 WHERE id = ?1")?;
            for (id, n) in &d.expanded {
                if st.execute(params![id, n])? != 1 {
                    return Err(DbError::Codec(format!("expanded: no node {id}")));
                }
            }
            let mut st = tx.prepare(
                "INSERT INTO key_hit (key, hits) VALUES (?1, ?2) \
                 ON CONFLICT (key) DO UPDATE SET hits = hits + excluded.hits",
            )?;
            for (k, n) in &d.key_hits {
                st.execute(params![k.0 as i64, n])?;
            }
            let mut st = tx.prepare(&format!(
                "INSERT INTO node ({NODE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
                 ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, \
                 ?25, ?26)"
            ))?;
            let in_delta: BTreeSet<NodeId> = d.nodes.iter().map(|n| n.id).collect();
            for n in &d.nodes {
                let merged_into = match n.merged_into {
                    Some(m) if in_delta.contains(&m) => {
                        late_merged_into.push((n.id, m));
                        None
                    }
                    m => m,
                };
                st.execute(params![
                    n.id,
                    n.parent,
                    n.depth,
                    n.macro_.code(),
                    runs::to_blob(&n.inputs),
                    n.frames,
                    n.total_frames as i64,
                    n.end_frame,
                    n.key.0 as i64,
                    n.digest as i64,
                    n.world_hash as i64,
                    n.map.0,
                    n.pos.0,
                    n.pos.1,
                    n.progress,
                    n.fitness.novelty,
                    n.fitness.progress,
                    n.fitness.bug,
                    n.fitness.balance,
                    n.fitness.total,
                    n.generation,
                    n.expanded,
                    status_str(n.status),
                    merged_into,
                    n.build.engine as i64,
                    n.build.content as i64,
                ])?;
            }
            let mut st = tx.prepare("UPDATE node SET merged_into = ?2 WHERE id = ?1")?;
            for (id, into) in &late_merged_into {
                st.execute(params![id, into])?;
            }
            for (id, cp) in &d.snapshots {
                put_snapshot(&tx, *id, cp)?;
            }
            let mut st = tx
                .prepare("INSERT OR IGNORE INTO node_touch (node, kind, id) VALUES (?1, ?2, ?3)")?;
            for (node, t) in &d.touches {
                let (k, id) = touch_cols(t);
                st.execute(params![node, k, id])?;
            }
            let mut st = tx.prepare(
                "INSERT INTO seen (kind, id, first_node) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (kind, id) DO UPDATE SET hits = hits + 1",
            )?;
            for (k, id, node) in &d.seen {
                st.execute(params![*k as i64, id, node])?;
            }
            let mut st = tx.prepare(
                "INSERT INTO coverage (map, x, y, first_node) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (map, x, y) DO UPDATE SET hits = hits + 1",
            )?;
            for (map, x, y, node) in &d.cells {
                st.execute(params![map.0, x, y, node])?;
            }
            let mut st = tx.prepare(
                "INSERT INTO battle (node, kind, trainer, outcome, frames, player_level, \
                 opponent_level, hp_lost_permille) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for (node, b) in &d.battles {
                st.execute(params![
                    node,
                    b.kind,
                    b.trainer,
                    b.outcome,
                    b.frames,
                    b.player_level,
                    b.opponent_level,
                    b.hp_lost_permille,
                ])?;
            }
            // A finding seen again counts a hit; one that was stale or gone reopens with the
            // new repro.
            let keep =
                |c: &str| format!("{c} = CASE WHEN status = 'open' THEN {c} ELSE excluded.{c} END");
            let reopen = [
                "kind",
                "node",
                "macro",
                "tail",
                "total_frame",
                "map",
                "x",
                "y",
                "message",
                "location",
                "script",
                "log",
                "build_engine",
                "build_content",
            ]
            .map(keep)
            .join(", ");
            let mut st = tx.prepare(&format!(
                "INSERT INTO finding (signature, kind, node, macro, tail, total_frame, map, x, y, \
                 message, location, script, log, build_engine, build_content, first_generation, \
                 last_generation) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, \
                 ?13, ?14, ?15, ?16, ?16) \
                 ON CONFLICT (signature) DO UPDATE SET {reopen}, hits = hits + 1, \
                 last_generation = excluded.last_generation, status = 'open'"
            ))?;
            for (f, node, total) in &d.findings {
                st.execute(params![
                    crate::findings::signature(f) as i64,
                    kind_str(f.kind),
                    node,
                    f.macro_.as_ref().map(macro_text).transpose()?,
                    runs::to_text(&f.tail),
                    *total as i64,
                    f.map.0,
                    f.pos.0,
                    f.pos.1,
                    f.message,
                    f.location,
                    f.script.map(|s| s.0),
                    f.log.join("\n"),
                    build.engine as i64,
                    build.content as i64,
                    d.generation,
                ])?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('generation', ?1)",
            params![d.generation.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn query_nodes(&self, filter: &str, p: impl rusqlite::Params) -> Result<Vec<NodeRow>, DbError> {
        self.query_cols(NODE_COLS, filter, p)
    }

    fn query_cols(
        &self,
        cols: &str,
        filter: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<NodeRow>, DbError> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {cols} FROM node {filter} ORDER BY id"))?;
        let mut rows = st.query(p)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(node_from_row(r)?);
        }
        Ok(out)
    }

    /// Id ascending, with their inputs.
    pub fn nodes(&self) -> Result<Vec<NodeRow>, DbError> {
        self.query_nodes("", [])
    }

    /// Id ascending, without their inputs (`inputs` empty): what the runner and revalidation
    /// keep in memory (schema v3: inputs are not resident).
    pub fn nodes_light(&self) -> Result<Vec<NodeRow>, DbError> {
        self.query_cols(NODE_COLS_LIGHT, "", [])
    }

    /// Node `id`'s inputs (`None`: no such node).
    pub fn inputs(&self, id: NodeId) -> Result<Option<Vec<Run>>, DbError> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT inputs FROM node WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        blob.map(|b| parse_blob(&b)).transpose()
    }

    /// `(key, hits)` of the `key_hit` table, key order.
    pub fn key_hits(&self) -> Result<Vec<(AbstractKey, u32)>, DbError> {
        let mut st = self
            .conn
            .prepare("SELECT key, hits FROM key_hit ORDER BY key")?;
        let rows = st.query_map([], |r| {
            Ok((AbstractKey(r.get::<_, i64>(0)? as u64), r.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn node(&self, id: NodeId) -> Result<Option<NodeRow>, DbError> {
        Ok(self.query_nodes("WHERE id = ?1", [id])?.pop())
    }

    /// Root first, with their inputs; empty when `id` does not exist.
    pub fn lineage(&self, id: NodeId) -> Result<Vec<NodeRow>, DbError> {
        let mut out = Vec::new();
        let mut next = Some(id);
        while let Some(id) = next {
            let Some(n) = self.node(id)? else { break };
            next = n.parent;
            out.push(n);
        }
        out.reverse();
        Ok(out)
    }

    /// `(kind, id, hits)` by kind then id.
    pub fn seen(&self) -> Result<Vec<(SeenKind, i64, u32)>, DbError> {
        let mut st = self
            .conn
            .prepare("SELECT kind, id, hits FROM seen ORDER BY kind, id")?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push((parse_seen(r.get(0)?)?, r.get(1)?, r.get(2)?));
        }
        Ok(out)
    }

    /// `(map, x, y, hits)` by map, x, y; one map or all.
    pub fn coverage(&self, map: Option<MapId>) -> Result<Vec<(MapId, i16, i16, u32)>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT map, x, y, hits FROM coverage WHERE ?1 IS NULL OR map = ?1 \
             ORDER BY map, x, y",
        )?;
        let rows = st.query_map([map.map(|m| m.0)], |r| {
            Ok((MapId(r.get(0)?), r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn query_findings(
        &self,
        filter: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<FindingRow>, DbError> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {FINDING_COLS} FROM finding {filter} ORDER BY id"
        ))?;
        let mut rows = st.query(p)?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(finding_from_row(r)?);
        }
        Ok(out)
    }

    /// Id ascending.
    /// Report-only finding rows (`FindingKind::report_only`), counted apart from the bugs.
    pub fn report_count(&self) -> Result<u64, DbError> {
        let kinds = FINDING_KINDS.into_iter().filter(|k| k.report_only());
        let mut n = 0;
        for k in kinds {
            n += self.conn.query_row(
                "SELECT COUNT(*) FROM finding WHERE kind = ?1",
                [kind_str(k)],
                |r| r.get::<_, i64>(0),
            )? as u64;
        }
        Ok(n)
    }

    pub fn findings(&self) -> Result<Vec<FindingRow>, DbError> {
        self.query_findings("", [])
    }

    pub fn finding(&self, id: i64) -> Result<Option<FindingRow>, DbError> {
        Ok(self.query_findings("WHERE id = ?1", [id])?.pop())
    }

    /// Stores `cp` (its header's build) as node `id`'s end state, replacing any other.
    pub fn put_snapshot(&mut self, id: NodeId, cp: &Checkpoint) -> Result<(), DbError> {
        put_snapshot(&self.conn, id, cp)
    }

    /// Node `id`'s stored end state, if it was captured by `build`.
    pub fn snapshot(&self, id: NodeId, build: BuildHash) -> Result<Option<Checkpoint>, DbError> {
        let bytes: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT bytes FROM snapshot WHERE node = ?1 AND build_engine = ?2 \
                 AND build_content = ?3",
                params![id, build.engine as i64, build.content as i64],
                |r| r.get(0),
            )
            .optional()?;
        bytes
            .map(|b| {
                checkpoint::decode(&b).map_err(|e| DbError::Codec(format!("snapshot {id}: {e:?}")))
            })
            .transpose()
    }

    /// Removes the snapshots of other builds; returns how many.
    pub fn drop_snapshots_except(&mut self, build: BuildHash) -> Result<u64, DbError> {
        let n = self.conn.execute(
            "DELETE FROM snapshot WHERE build_engine != ?1 OR build_content != ?2",
            params![build.engine as i64, build.content as i64],
        )?;
        Ok(n as u64)
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Several meta values in one transaction (all or none).
    pub fn set_metas(&mut self, kv: &[(&str, String)]) -> Result<(), DbError> {
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)")?;
            for (k, v) in kv {
                st.execute(params![k, v])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>, DbError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// Every `valid` node becomes `stale` (merged and broken ones keep their status); returns
    /// how many.
    pub fn mark_all_stale(&mut self) -> Result<u64, DbError> {
        let n = self.conn.execute(
            "UPDATE node SET status = 'stale' WHERE status = 'valid'",
            [],
        )?;
        Ok(n as u64)
    }

    /// Revalidation results, in one transaction: each row's inputs, frames, total frames, end
    /// frame, key, digest, world hash, map, pos, progress, status, `merged_into` and build
    /// (parent, depth, macro, fitness, generation and `expanded` are kept). The rows leave the
    /// live set first, so keys can move between them in any order.
    ///
    /// Rows must carry their inputs: a row whose inputs do not add up to its `frames` (a light
    /// row of [`Db::nodes_light`]) is refused before any write, so its stored inputs are never
    /// wiped. Light rows go through [`Db::set_status_and_snapshots`].
    pub fn set_status(&mut self, rows: &[NodeRow]) -> Result<(), DbError> {
        if let Some(n) = rows.iter().find(|n| runs::frames(&n.inputs) != n.frames) {
            return Err(DbError::Codec(format!(
                "set_status: node {} has {} input frames for {} frames (a light row?)",
                n.id,
                runs::frames(&n.inputs),
                n.frames
            )));
        }
        let inputs: Vec<(NodeId, &[Run])> =
            rows.iter().map(|n| (n.id, n.inputs.as_slice())).collect();
        self.set_status_and_snapshots(rows, &inputs, &[], &[])
    }

    /// [`Db::set_status`] without the rows' inputs (light rows), the new inputs of the
    /// `inputs` nodes (repaired ones), [`Db::put_snapshot`]s, and the new `node_touch` rows of
    /// the `touches` nodes (each node's old rows are replaced: the replay or repair is what
    /// the node now touches), in one transaction (a revalidated level).
    pub fn set_status_and_snapshots(
        &mut self,
        rows: &[NodeRow],
        inputs: &[(NodeId, &[Run])],
        snapshots: &[(NodeId, &Checkpoint)],
        touches: &[(NodeId, &[Touch])],
    ) -> Result<(), DbError> {
        let tx = self.conn.transaction()?;
        tx.pragma_update(None, "defer_foreign_keys", "ON")?;
        {
            let mut out = tx.prepare("UPDATE node SET status = 'broken' WHERE id = ?1")?;
            for n in rows {
                if out.execute([n.id])? != 1 {
                    return Err(DbError::Codec(format!("set_status: no node {}", n.id)));
                }
            }
            let mut st = tx.prepare("UPDATE node SET inputs = ?2 WHERE id = ?1")?;
            for (id, runs) in inputs {
                if st.execute(params![id, runs::to_blob(runs)])? != 1 {
                    return Err(DbError::Codec(format!("set_status: no node {id}")));
                }
            }
            let mut st = tx.prepare(
                "UPDATE node SET frames = ?2, total_frames = ?3, end_frame = ?4, key = ?5, \
                 digest = ?6, world_hash = ?7, map = ?8, x = ?9, y = ?10, progress = ?11, \
                 status = ?12, merged_into = ?13, build_engine = ?14, build_content = ?15 \
                 WHERE id = ?1",
            )?;
            for n in rows {
                st.execute(params![
                    n.id,
                    n.frames,
                    n.total_frames as i64,
                    n.end_frame,
                    n.key.0 as i64,
                    n.digest as i64,
                    n.world_hash as i64,
                    n.map.0,
                    n.pos.0,
                    n.pos.1,
                    n.progress,
                    status_str(n.status),
                    n.merged_into,
                    n.build.engine as i64,
                    n.build.content as i64,
                ])?;
            }
            for (id, cp) in snapshots {
                put_snapshot(&tx, *id, cp)?;
            }
            let mut del = tx.prepare("DELETE FROM node_touch WHERE node = ?1")?;
            let mut ins = tx
                .prepare("INSERT OR IGNORE INTO node_touch (node, kind, id) VALUES (?1, ?2, ?3)")?;
            for (id, ts) in touches {
                del.execute([id])?;
                for t in ts.iter() {
                    let (k, x) = touch_cols(t);
                    ins.execute(params![id, k, x])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// After a revalidation on `build`: `meta.build_*` = `build`, and open findings of other
    /// builds become `stale`. Returns the number of findings marked.
    pub fn set_build(&mut self, build: BuildHash) -> Result<u64, DbError> {
        let tx = self.conn.transaction()?;
        for (k, v) in [
            ("build_engine", build.engine.to_string()),
            ("build_content", build.content.to_string()),
        ] {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        let n = tx.execute(
            "UPDATE finding SET status = 'stale' WHERE status = 'open' \
             AND (build_engine != ?1 OR build_content != ?2)",
            params![build.engine as i64, build.content as i64],
        )?;
        tx.commit()?;
        self.build = build;
        Ok(n as u64)
    }

    /// The nodes whose `node_touch` names `t` (incremental revalidation, Task N).
    pub fn touching(&self, t: Touch) -> Result<Vec<NodeId>, DbError> {
        let (kind, id) = touch_cols(&t);
        let mut st = self
            .conn
            .prepare("SELECT node FROM node_touch WHERE kind = ?1 AND id = ?2")?;
        let rows = st.query_map(params![kind, id], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// In one transaction: the `kept` nodes and their snapshots move to `build` (their state is
    /// the same on it; a snapshot's header keeps the capturing build, which is informational),
    /// and the valid ones of `stale` become stale (incremental revalidation, Task N).
    pub fn retag(
        &mut self,
        kept: &[NodeId],
        stale: &[NodeId],
        build: BuildHash,
    ) -> Result<(), DbError> {
        let (e, c) = (build.engine as i64, build.content as i64);
        let tx = self.conn.transaction()?;
        {
            let mut node =
                tx.prepare("UPDATE node SET build_engine = ?2, build_content = ?3 WHERE id = ?1")?;
            let mut snap = tx.prepare(
                "UPDATE snapshot SET build_engine = ?2, build_content = ?3 WHERE node = ?1",
            )?;
            for id in kept {
                node.execute(params![id, e, c])?;
                snap.execute(params![id, e, c])?;
            }
            let mut st =
                tx.prepare("UPDATE node SET status = 'stale' WHERE id = ?1 AND status = 'valid'")?;
            for id in stale {
                st.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The config (RON) of the last `run` row, if any.
    pub fn last_run_config(&self) -> Result<Option<String>, DbError> {
        Ok(self
            .conn
            .query_row("SELECT config FROM run ORDER BY id DESC LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn record_run(&mut self, r: &RunRow) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO run (started, config, threads, generations, nodes, frames, wall_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                r.started,
                r.config,
                r.threads,
                r.generations,
                r.nodes as i64,
                r.frames as i64,
                r.wall_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn stats(&self) -> Result<DbStats, DbError> {
        let count =
            |sql: &str| -> Result<i64, DbError> { Ok(self.conn.query_row(sql, [], |r| r.get(0))?) };
        let status = |s: &str| -> Result<u64, DbError> {
            Ok(self
                .conn
                .query_row("SELECT COUNT(*) FROM node WHERE status = ?1", [s], |r| {
                    r.get::<_, i64>(0)
                })? as u64)
        };
        let generation = match self.meta("generation")? {
            Some(v) => v
                .parse()
                .map_err(|_| DbError::Codec(format!("meta generation = {v:?}")))?,
            None => 0,
        };
        Ok(DbStats {
            nodes: count("SELECT COUNT(*) FROM node")? as u64,
            valid: status("valid")?,
            stale: status("stale")?,
            merged: status("merged")?,
            broken: status("broken")?,
            findings: count("SELECT COUNT(*) FROM finding")? as u64,
            maps: count("SELECT COUNT(*) FROM seen WHERE kind = 2")? as u32,
            cells: count("SELECT COUNT(*) FROM coverage")? as u64,
            scripts: count("SELECT COUNT(*) FROM seen WHERE kind = 3")? as u32,
            max_progress: count(
                "SELECT COALESCE(MAX(progress), 0) FROM node WHERE status != 'broken'",
            )? as u32,
            generation,
        })
    }

    /// FNV of the node, key_hit, finding, seen and coverage tables in key order, without
    /// timings or run rows (determinism tests, `explore digest`).
    ///
    /// `battle` and `node_touch` are left out on purpose: both are derived from the nodes'
    /// expansions (the balance record and the re-simulation index), written in the same
    /// transaction as the node rows, so they add nothing the node table does not already pin.
    /// `snapshot` (a cache) and `meta` are left out too. Changing the table list changes every
    /// recorded digest.
    pub fn content_digest(&self) -> Result<u64, DbError> {
        let mut h = engine::hash::Fnv64::new();
        for (table, order) in [
            ("node", "id"),
            ("key_hit", "key"),
            ("finding", "id"),
            ("seen", "kind, id"),
            ("coverage", "map, x, y"),
        ] {
            h.write(table.as_bytes());
            h.write(&[0xFF]);
            let mut st = self
                .conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))?;
            let cols = st.column_count();
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                h.write(&[0xFE]);
                for i in 0..cols {
                    match r.get_ref(i)? {
                        ValueRef::Null => h.write(&[0]),
                        ValueRef::Integer(v) => {
                            h.write(&[1]);
                            h.write(&v.to_le_bytes());
                        }
                        ValueRef::Real(v) => {
                            h.write(&[2]);
                            h.write(&v.to_bits().to_le_bytes());
                        }
                        ValueRef::Text(b) | ValueRef::Blob(b) => {
                            h.write(&[3]);
                            h.write(&(b.len() as u64).to_le_bytes());
                            h.write(b);
                        }
                    }
                }
            }
        }
        Ok(h.finish())
    }
}

fn check_schema(v: &str) -> Result<(), DbError> {
    let found = v
        .parse::<i64>()
        .map_err(|_| DbError::Codec(format!("meta schema = {v:?}")))?;
    if found != SCHEMA_VERSION {
        return Err(DbError::Schema {
            found,
            want: SCHEMA_VERSION,
        });
    }
    Ok(())
}

/// A db without `key_version` predates it (schema 2, refused before this is reached).
fn check_key_version(v: Option<String>) -> Result<(), DbError> {
    let want = crate::key::KEY_VERSION;
    match v {
        Some(v) if v == want.to_string() => Ok(()),
        v => Err(DbError::KeyVersion {
            found: v.unwrap_or_else(|| "none".into()),
            want,
        }),
    }
}

fn put_snapshot(conn: &Connection, id: NodeId, cp: &Checkpoint) -> Result<(), DbError> {
    let b = cp.header.build;
    conn.execute(
        "INSERT OR REPLACE INTO snapshot (node, build_engine, build_content, bytes) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            id,
            b.engine as i64,
            b.content as i64,
            checkpoint::encode(cp)
        ],
    )?;
    Ok(())
}

#[derive(Clone, PartialEq, Debug)]
pub struct FindingRow {
    pub id: i64,
    pub signature: u64,
    pub kind: FindingKind,
    /// The node whose end state the tail starts at; `None` = a new game.
    pub node: Option<NodeId>,
    pub macro_: Option<Macro>,
    pub tail: Vec<Run>,
    pub total_frame: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    pub message: String,
    pub location: Option<String>,
    pub script: Option<ScriptId>,
    /// The draft's log lines joined by `\n`.
    pub log: String,
    pub build: BuildHash,
    /// `open`, `stale` (another build; set by revalidation) or `gone`.
    pub status: String,
    pub hits: u32,
    pub first_generation: u32,
    pub last_generation: u32,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RunRow {
    pub started: String,
    pub config: String,
    pub threads: u32,
    pub generations: u32,
    pub nodes: u64,
    pub frames: u64,
    pub wall_ms: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DbStats {
    pub nodes: u64,
    pub valid: u64,
    pub stale: u64,
    pub merged: u64,
    pub broken: u64,
    /// Every finding row, the report-only ones included ([`Db::report_count`]).
    pub findings: u64,
    pub maps: u32,
    pub cells: u64,
    pub scripts: u32,
    /// Over every node but broken ones.
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
                    "key_hit",
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
                 end_frame, key, digest, world_hash, map, x, y, progress, f_novelty, \
                 f_progress, f_bug, f_balance, f_total, generation, status, build_engine, \
                 build_content) VALUES (?1, NULL, 0, 0, x'', 0, 0, 0, ?2, \
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
