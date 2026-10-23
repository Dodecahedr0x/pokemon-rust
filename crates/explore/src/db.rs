//! The SQLite store, `target/explore/{stub|emerald|silice}/explore.db`: the last spine run, its
//! positions, the branches, the novelty archive, the coverage heatmap and the findings
//! (`docs/plans/2026-10-23-explorer-spine.md` §4).

use crate::branch::BranchResult;
use crate::model::{FindingDraft, FindingKind, Macro, SeenKind};
use crate::runs;
use crate::spine::{Coverage, SpineRun};
use engine::checkpoint::{BuildHash, state_digest};
use engine::data::{MapId, ScriptId};
use game_native::trace::Run;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// The DDL (`CREATE … IF NOT EXISTS`, idempotent).
pub const SCHEMA_SQL: &str = include_str!("schema.sql");
/// 4: the spine and its branches (the evolutionary node tables are gone). Older dbs are
/// refused with the reset hint.
pub const SCHEMA_VERSION: i64 = 4;

/// How long a connection waits on a lock held by another (writer or reader).
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Sets the pragmas `Db::open` uses (`journal_mode=WAL; synchronous=NORMAL`) and runs
/// [`SCHEMA_SQL`].
pub fn init_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
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
            DbError::Flavor { db, build } if db.is_empty() => write!(
                f,
                "explore.db records no content flavour, this build is the {build} content \
                 (explore dbs are caches: run `explore reset --yes`)"
            ),
            DbError::Flavor { db, build } => write!(
                f,
                "explore.db was made by the {db} content, this build is the {build} content \
                 (each flavour has its own db, `target/explore/<flavour>/explore.db`: pass this \
                 build's with --db, or build the {db} flavour{})",
                match db.as_str() {
                    "silice" => " with PKR_GAME=silice (`cargo xtask explore --game silice`)",
                    "stub" => " with PKR_EXTRACTED_DIR=/nonexistent",
                    _ => " with PKR_GAME unset (`cargo xtask explore --game emerald`)",
                }
            ),
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

/// The explorer database: one writer, any number of read-only readers (`open_read_only`, WAL).
pub struct Db {
    conn: Connection,
    /// The build this process runs (`open`), or the db's recorded build (`open_read_only`).
    /// New rows are stamped with it.
    build: BuildHash,
}

/// A coverage row: `(map, x, y, spine, hits)` ([`Db::coverage`]).
pub type CoverageRow = (MapId, i16, i16, bool, u32);

/// The last spine run (`explore spine`).
#[derive(Clone, PartialEq, Debug)]
pub struct SpineRow {
    pub steps: u32,
    pub done: u32,
    pub frames: u64,
    pub inputs: Vec<Run>,
    pub digest: u64,
    pub error: Option<String>,
    pub wall_ms: u64,
    pub build: BuildHash,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PositionRow {
    pub idx: u32,
    pub label: Option<String>,
    pub segment: Option<String>,
    pub frame: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    pub digest: u64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct BranchRow {
    pub id: i64,
    pub origin: u32,
    /// `branch::Action` RON.
    pub action: String,
    pub kind: String,
    pub outcome: String,
    pub explored: Option<String>,
    pub macros: Vec<Macro>,
    pub explore_frames: u64,
    pub recover_frames: u64,
    pub window_frames: u64,
    pub new_maps: u32,
    pub new_scripts: u32,
    pub new_flags: u32,
    pub new_vars: u32,
    pub new_cells: u32,
    pub points: i64,
    pub first_found: u32,
    pub message: String,
    pub run: i64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FindingRow {
    pub id: i64,
    pub signature: u64,
    pub kind: FindingKind,
    /// The branch it was found on; `None` = the spine.
    pub branch: Option<i64>,
    /// The spine position the tail starts at; `None` = a new game.
    pub origin: Option<u32>,
    pub macro_: Option<Macro>,
    /// Every input from a new game to the finding's frame (inclusive).
    pub inputs: Vec<Run>,
    /// The last frames of `inputs` that are the job's tail (the rest is the spine's prefix).
    pub tail_frames: u64,
    pub total_frame: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    pub message: String,
    pub location: Option<String>,
    pub script: Option<ScriptId>,
    /// The draft's log lines joined by `\n`.
    pub log: String,
    pub build: BuildHash,
    /// `open`, or `stale` (found on another build).
    pub status: String,
    pub hits: u32,
    pub first_run: i64,
    pub last_run: i64,
}

impl FindingRow {
    /// The tail: the last `tail_frames` of the inputs.
    pub fn tail(&self) -> Vec<Run> {
        let total = u64::from(runs::frames(&self.inputs));
        runs::split_at(&self.inputs, total.saturating_sub(self.tail_frames)).1
    }

    /// The draft back (the signature's parts and the tail), for `findings::reproduce`.
    pub fn draft(&self) -> FindingDraft {
        FindingDraft {
            kind: self.kind,
            message: self.message.clone(),
            location: self.location.clone(),
            macro_: self.macro_,
            tail: self.tail(),
            map: self.map,
            pos: self.pos,
            script: self.script,
            log: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct RunRow {
    pub started: String,
    /// `spine` or `branch`.
    pub command: String,
    pub config: String,
    pub threads: u32,
    pub branches: u64,
    pub frames: u64,
    pub wall_ms: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DbStats {
    /// Steps of the spine list, and how many ran (0 and 0 without a spine run).
    pub spine_steps: u32,
    pub spine_done: u32,
    pub spine_frames: u64,
    pub spine_error: Option<String>,
    pub positions: u64,
    pub branches: u64,
    /// Branches per `Outcome::name`, every name present.
    pub outcomes: BTreeMap<String, u64>,
    /// Every finding row, the report-only ones included ([`Db::report_count`]).
    pub findings: u64,
    /// Novelty rows (what branches found the spine never touched), per [`SeenKind`] number.
    pub seen: BTreeMap<i64, u64>,
    /// Cells stood on, by the spine or a branch.
    pub cells: u64,
    pub spine_cells: u64,
}

const FINDING_KINDS: [FindingKind; 9] = [
    FindingKind::Panic,
    FindingKind::Softlock,
    FindingKind::RoundTrip,
    FindingKind::CapturableStuck,
    FindingKind::PlannerDivergence,
    FindingKind::SlowFrame,
    FindingKind::Unsupported,
    FindingKind::Divergence,
    FindingKind::Parity,
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

fn parse_macro(s: &str) -> Result<Macro, DbError> {
    Macro::from_text(s).ok_or_else(|| DbError::Codec(format!("bad macro {s:?}")))
}

fn macros_text(ms: &[Macro]) -> String {
    ron::to_string(ms).expect("macros serialize")
}

fn parse_macros(s: &str) -> Result<Vec<Macro>, DbError> {
    ron::from_str(s).map_err(|e| DbError::Codec(format!("macros {s:?}: {e}")))
}

fn blob(r: &Row, i: usize) -> Result<Vec<Run>, DbError> {
    match r.get_ref(i)? {
        ValueRef::Blob(b) => runs::from_blob(b).map_err(DbError::Codec),
        v => Err(DbError::Codec(format!("inputs: {:?}", v.data_type()))),
    }
}

/// `(map << 32) | (x as u16) << 16 | y as u16` (`SeenKind::Cell`).
pub fn cell_id(map: MapId, x: i16, y: i16) -> i64 {
    (i64::from(map.0) << 32) | (i64::from(x as u16) << 16) | i64::from(y as u16)
}

/// The novelty items of `c` that `base` does not hold, in kind then id order.
pub fn novel_items(c: &Coverage, base: &Coverage) -> Vec<(SeenKind, i64)> {
    let mut out = Vec::new();
    for f in c.flags.difference(&base.flags) {
        out.push((SeenKind::Flag, i64::from(*f)));
    }
    for (v, x) in c.var_values.difference(&base.var_values) {
        out.push((SeenKind::VarValue, (i64::from(*v) << 16) | i64::from(*x)));
    }
    for m in c.maps.difference(&base.maps) {
        out.push((SeenKind::Map, i64::from(m.0)));
    }
    for s in c.scripts.difference(&base.scripts) {
        out.push((SeenKind::Script, i64::from(s.0)));
    }
    for &(m, x, y) in c.cells.difference(&base.cells) {
        out.push((SeenKind::Cell, cell_id(m, x, y)));
    }
    out
}

const FINDING_COLS: &str = "id, signature, kind, branch, origin, macro, inputs, tail_frames, \
     total_frame, map, x, y, message, location, script, log, build_engine, build_content, \
     status, hits, first_run, last_run";

fn finding_from_row(r: &Row) -> Result<FindingRow, DbError> {
    let kind: String = r.get(2)?;
    let macro_: Option<String> = r.get(5)?;
    Ok(FindingRow {
        id: r.get(0)?,
        signature: r.get::<_, i64>(1)? as u64,
        kind: parse_kind(&kind)?,
        branch: r.get(3)?,
        origin: r.get(4)?,
        macro_: macro_.as_deref().map(parse_macro).transpose()?,
        inputs: blob(r, 6)?,
        tail_frames: r.get::<_, i64>(7)? as u64,
        total_frame: r.get::<_, i64>(8)? as u64,
        map: MapId(r.get(9)?),
        pos: (r.get(10)?, r.get(11)?),
        message: r.get(12)?,
        location: r.get(13)?,
        script: r.get::<_, Option<u16>>(14)?.map(ScriptId),
        log: r.get(15)?,
        build: BuildHash {
            engine: r.get::<_, i64>(16)? as u64,
            content: r.get::<_, i64>(17)? as u64,
        },
        status: r.get(18)?,
        hits: r.get(19)?,
        first_run: r.get(20)?,
        last_run: r.get(21)?,
    })
}

/// A finding to store: the draft, where its tail starts, and the inputs before it.
pub struct NewFinding<'a> {
    pub draft: &'a FindingDraft,
    pub branch: Option<i64>,
    pub origin: Option<u32>,
    /// The spine's inputs up to the origin (empty from a new game).
    pub prefix: &'a [Run],
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

    /// Opens (creating the parent dirs and the schema) the db for `build` and `flavor` (the
    /// shells pass `content::FLAVOR`); a db of another flavour ([`DbError::Flavor`]) or schema
    /// ([`DbError::Schema`]) is refused before any write. `build_changed` is reported, never
    /// acted on.
    pub fn open(
        path: &Path,
        build: BuildHash,
        flavor: content::Flavor,
    ) -> Result<(Db, OpenState), DbError> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)
                .map_err(|e| DbError::Codec(format!("creating {}: {e}", dir.display())))?;
        }
        Self::init(Connection::open(path)?, build, flavor)
    }

    /// An in-memory db, as [`Db::open`] (tests).
    pub fn open_in_memory(
        build: BuildHash,
        flavor: content::Flavor,
    ) -> Result<(Db, OpenState), DbError> {
        Self::init(Connection::open_in_memory()?, build, flavor)
    }

    fn init(
        conn: Connection,
        build: BuildHash,
        flavor: content::Flavor,
    ) -> Result<(Db, OpenState), DbError> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let flavor = flavor.name();
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
                db.set_metas(&[
                    ("schema", SCHEMA_VERSION.to_string()),
                    ("flavor", flavor.to_string()),
                    ("build_engine", build.engine.to_string()),
                    ("build_content", build.content.to_string()),
                    ("key_version", crate::key::KEY_VERSION.to_string()),
                ])?;
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
        let has_meta: bool = db.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'meta')",
            [],
            |r| r.get(0),
        )?;
        match if has_meta { db.meta("schema")? } else { None } {
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

    /// The build new rows are stamped with.
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

    /// Records a run; returns its id.
    pub fn record_run(&mut self, r: &RunRow) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO run (started, command, config, threads, branches, frames, wall_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                r.started,
                r.command,
                r.config,
                r.threads,
                r.branches as i64,
                r.frames as i64,
                r.wall_ms as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// The runs, oldest first.
    pub fn runs(&self) -> Result<Vec<RunRow>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT started, command, config, threads, branches, frames, wall_ms FROM run \
             ORDER BY id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(RunRow {
                started: r.get(0)?,
                command: r.get(1)?,
                config: r.get(2)?,
                threads: r.get(3)?,
                branches: r.get::<_, i64>(4)? as u64,
                frames: r.get::<_, i64>(5)? as u64,
                wall_ms: r.get::<_, i64>(6)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Stores a spine run (one transaction): the spine row, its positions, the spine column of
    /// the coverage, and its findings and reports. A spine whose positions differ from the
    /// stored ones (another route or build) clears the branches, the novelty archive and the
    /// branch hits, which were relative to the old one. `meta.build_*` becomes this build, and
    /// open findings of other builds become `stale`.
    pub fn put_spine(&mut self, s: &SpineRun, wall_ms: u64, run: i64) -> Result<(), DbError> {
        let build = self.build;
        let digests: Vec<u64> = s
            .positions
            .iter()
            .map(|p| state_digest(&p.end.world))
            .collect();
        let old: Vec<u64> = self.positions()?.into_iter().map(|p| p.digest).collect();
        let tx = self.conn.transaction()?;
        if old != digests {
            tx.execute_batch(
                "DELETE FROM branch; DELETE FROM seen; UPDATE coverage SET hits = 0; \
                 DELETE FROM position;",
            )?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO spine (id, steps, done, frames, inputs, digest, error, \
             wall_ms, build_engine, build_content) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                s.steps.len() as i64,
                s.positions.len().saturating_sub(1) as i64,
                s.frames as i64,
                runs::to_blob(&s.inputs),
                digests.last().copied().unwrap_or(0) as i64,
                s.error.as_ref().map(|e| e.to_string()),
                wall_ms as i64,
                build.engine as i64,
                build.content as i64,
            ],
        )?;
        {
            let mut st = tx.prepare(
                "INSERT OR REPLACE INTO position (idx, label, segment, frame, map, x, y, digest) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for (p, d) in s.positions.iter().zip(&digests) {
                let (x, y) = p.pos();
                st.execute(params![
                    p.index as i64,
                    p.label,
                    p.segment,
                    p.frame as i64,
                    p.map().0,
                    x,
                    y,
                    *d as i64
                ])?;
            }
        }
        tx.execute("UPDATE coverage SET spine = 0", [])?;
        tx.execute("DELETE FROM coverage WHERE spine = 0 AND hits = 0", [])?;
        {
            let mut st = tx.prepare(
                "INSERT INTO coverage (map, x, y, spine, hits) VALUES (?1, ?2, ?3, 1, 0) \
                 ON CONFLICT (map, x, y) DO UPDATE SET spine = 1",
            )?;
            for &(m, x, y) in &s.coverage.cells {
                st.execute(params![m.0, x, y])?;
            }
        }
        let error = s.error.as_ref().and_then(|e| e.finding.as_ref());
        for f in error.into_iter().chain(&s.reports) {
            put_finding(
                &tx,
                build,
                run,
                &NewFinding {
                    draft: f,
                    branch: None,
                    origin: None,
                    prefix: &[],
                },
            )?;
        }
        for (k, v) in [
            ("build_engine", build.engine.to_string()),
            ("build_content", build.content.to_string()),
        ] {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        tx.execute(
            "UPDATE finding SET status = 'stale' WHERE status = 'open' \
             AND (build_engine != ?1 OR build_content != ?2)",
            params![build.engine as i64, build.content as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Stores branch results in their order (one transaction): each branch row (replacing the
    /// one of the same origin and action), its novelty against the spine (`seen`, the first
    /// branch keeps a row), its cells and its findings and reports (tails prefixed with the
    /// spine's inputs up to the origin).
    pub fn put_branches(
        &mut self,
        spine: &SpineRun,
        results: &[BranchResult],
        run: i64,
    ) -> Result<(), DbError> {
        let build = self.build;
        let tx = self.conn.transaction()?;
        for r in results {
            let origin = &spine.positions[r.target.origin];
            let action = r.target.action.to_text();
            tx.execute(
                "DELETE FROM branch WHERE origin = ?1 AND action = ?2",
                params![origin.index as i64, action],
            )?;
            let novel = novel_items(&r.coverage, &spine.coverage);
            tx.execute(
                "INSERT INTO branch (origin, action, kind, outcome, explored, macros, \
                 explore_frames, recover_frames, window_frames, new_maps, new_scripts, \
                 new_flags, new_vars, new_cells, points, first_found, message, run) VALUES \
                 (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0, ?16, ?17)",
                params![
                    origin.index as i64,
                    action,
                    r.target.kind.name(),
                    r.outcome.name(),
                    r.explored.map(explored_name),
                    macros_text(&r.macros),
                    r.explore_frames as i64,
                    r.recover_frames as i64,
                    r.window_frames as i64,
                    r.gain.maps,
                    r.gain.scripts,
                    r.gain.flags,
                    r.gain.var_values,
                    r.gain.cells,
                    r.points,
                    r.message,
                    run,
                ],
            )?;
            let id = tx.last_insert_rowid();
            let mut first = 0u32;
            {
                let mut st = tx.prepare(
                    "INSERT INTO seen (kind, id, first_branch, hits) VALUES (?1, ?2, ?3, 1) \
                     ON CONFLICT (kind, id) DO UPDATE SET hits = hits + 1",
                )?;
                let mut has =
                    tx.prepare("SELECT EXISTS (SELECT 1 FROM seen WHERE kind = ?1 AND id = ?2)")?;
                for (k, x) in &novel {
                    let had: bool = has.query_row(params![*k as i64, x], |r| r.get(0))?;
                    if !had {
                        first += 1;
                    }
                    st.execute(params![*k as i64, x, id])?;
                }
                let mut cov = tx.prepare(
                    "INSERT INTO coverage (map, x, y, spine, hits) VALUES (?1, ?2, ?3, 0, 1) \
                     ON CONFLICT (map, x, y) DO UPDATE SET hits = hits + 1",
                )?;
                for &(m, x, y) in &r.coverage.cells {
                    cov.execute(params![m.0, x, y])?;
                }
            }
            tx.execute(
                "UPDATE branch SET first_found = ?2 WHERE id = ?1",
                params![id, first],
            )?;
            let prefix = runs::split_at(&spine.inputs, origin.frame).0;
            for f in r.findings.iter().chain(&r.reports) {
                put_finding(
                    &tx,
                    build,
                    run,
                    &NewFinding {
                        draft: f,
                        branch: Some(id),
                        origin: Some(origin.index as u32),
                        prefix: &prefix,
                    },
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Stores the vanilla oracle's `Parity` findings (one transaction): `(origin, draft)` with
    /// `origin` the spine position the diff was taken at (its inputs from a new game are the
    /// repro on our side), and `branch` the branch row the diff ended, if any.
    pub fn put_parity(
        &mut self,
        spine: &SpineRun,
        findings: &[(usize, Option<i64>, FindingDraft)],
        run: i64,
    ) -> Result<(), DbError> {
        let build = self.build;
        let tx = self.conn.transaction()?;
        for (origin, branch, f) in findings {
            let frame = spine.positions.get(*origin).map_or(0, |p| p.frame);
            let prefix = runs::split_at(&spine.inputs, frame).0;
            put_finding(
                &tx,
                build,
                run,
                &NewFinding {
                    draft: f,
                    branch: *branch,
                    origin: Some(*origin as u32),
                    prefix: &prefix,
                },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The last spine run.
    pub fn spine(&self) -> Result<Option<SpineRow>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT steps, done, frames, inputs, digest, error, wall_ms, build_engine, \
             build_content FROM spine WHERE id = 1",
        )?;
        let mut rows = st.query([])?;
        let Some(r) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(SpineRow {
            steps: r.get(0)?,
            done: r.get(1)?,
            frames: r.get::<_, i64>(2)? as u64,
            inputs: blob(r, 3)?,
            digest: r.get::<_, i64>(4)? as u64,
            error: r.get(5)?,
            wall_ms: r.get::<_, i64>(6)? as u64,
            build: BuildHash {
                engine: r.get::<_, i64>(7)? as u64,
                content: r.get::<_, i64>(8)? as u64,
            },
        }))
    }

    /// The spine's positions, in order.
    pub fn positions(&self) -> Result<Vec<PositionRow>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT idx, label, segment, frame, map, x, y, digest FROM position ORDER BY idx",
        )?;
        let rows = st.query_map([], |r| {
            Ok(PositionRow {
                idx: r.get(0)?,
                label: r.get(1)?,
                segment: r.get(2)?,
                frame: r.get::<_, i64>(3)? as u64,
                map: MapId(r.get(4)?),
                pos: (r.get(5)?, r.get(6)?),
                digest: r.get::<_, i64>(7)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The branches by id.
    pub fn branches(&self) -> Result<Vec<BranchRow>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT id, origin, action, kind, outcome, explored, macros, explore_frames, \
             recover_frames, window_frames, new_maps, new_scripts, new_flags, new_vars, \
             new_cells, points, first_found, message, run FROM branch ORDER BY id",
        )?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let macros: String = r.get(6)?;
            out.push(BranchRow {
                id: r.get(0)?,
                origin: r.get(1)?,
                action: r.get(2)?,
                kind: r.get(3)?,
                outcome: r.get(4)?,
                explored: r.get(5)?,
                macros: parse_macros(&macros)?,
                explore_frames: r.get::<_, i64>(7)? as u64,
                recover_frames: r.get::<_, i64>(8)? as u64,
                window_frames: r.get::<_, i64>(9)? as u64,
                new_maps: r.get(10)?,
                new_scripts: r.get(11)?,
                new_flags: r.get(12)?,
                new_vars: r.get(13)?,
                new_cells: r.get(14)?,
                points: r.get(15)?,
                first_found: r.get(16)?,
                message: r.get(17)?,
                run: r.get(18)?,
            });
        }
        Ok(out)
    }

    pub fn branch(&self, id: i64) -> Result<Option<BranchRow>, DbError> {
        Ok(self.branches()?.into_iter().find(|b| b.id == id))
    }

    /// `(kind, id, first_branch, hits)` by kind, id.
    pub fn seen(&self) -> Result<Vec<(SeenKind, i64, i64, u32)>, DbError> {
        let mut st = self
            .conn
            .prepare("SELECT kind, id, first_branch, hits FROM seen ORDER BY kind, id")?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let k: i64 = r.get(0)?;
            let k = SeenKind::from_i64(k).ok_or_else(|| DbError::Codec(format!("seen {k}")))?;
            out.push((k, r.get(1)?, r.get(2)?, r.get(3)?));
        }
        Ok(out)
    }

    /// `(map, x, y, spine, hits)` by map, x, y; one map or all. `spine`: the spine stood there;
    /// `hits`: branches that did.
    pub fn coverage(&self, map: Option<MapId>) -> Result<Vec<CoverageRow>, DbError> {
        let mut st = self.conn.prepare(
            "SELECT map, x, y, spine, hits FROM coverage WHERE ?1 IS NULL OR map = ?1 \
             ORDER BY map, x, y",
        )?;
        let rows = st.query_map([map.map(|m| m.0)], |r| {
            Ok((
                MapId(r.get(0)?),
                r.get(1)?,
                r.get(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get(4)?,
            ))
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

    /// Report-only finding rows (`FindingKind::report_only`), counted apart from the bugs.
    pub fn report_count(&self) -> Result<u64, DbError> {
        let mut n = 0;
        for k in FINDING_KINDS.into_iter().filter(|k| k.report_only()) {
            n += self.conn.query_row(
                "SELECT COUNT(*) FROM finding WHERE kind = ?1",
                [kind_str(k)],
                |r| r.get::<_, i64>(0),
            )? as u64;
        }
        Ok(n)
    }

    /// Id ascending.
    pub fn findings(&self) -> Result<Vec<FindingRow>, DbError> {
        self.query_findings("", [])
    }

    pub fn finding(&self, id: i64) -> Result<Option<FindingRow>, DbError> {
        Ok(self.query_findings("WHERE id = ?1", [id])?.pop())
    }

    pub fn stats(&self) -> Result<DbStats, DbError> {
        let count = |sql: &str| -> Result<u64, DbError> {
            Ok(self.conn.query_row(sql, [], |r| r.get::<_, i64>(0))? as u64)
        };
        let mut outcomes: BTreeMap<String, u64> = crate::branch::Outcome::NAMES
            .iter()
            .map(|n| (n.to_string(), 0))
            .collect();
        {
            let mut st = self
                .conn
                .prepare("SELECT outcome, COUNT(*) FROM branch GROUP BY outcome")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                outcomes.insert(r.get(0)?, r.get::<_, i64>(1)? as u64);
            }
        }
        let mut seen = BTreeMap::new();
        {
            let mut st = self
                .conn
                .prepare("SELECT kind, COUNT(*) FROM seen GROUP BY kind")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                seen.insert(r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64);
            }
        }
        let spine = self.spine()?;
        Ok(DbStats {
            spine_steps: spine.as_ref().map_or(0, |s| s.steps),
            spine_done: spine.as_ref().map_or(0, |s| s.done),
            spine_frames: spine.as_ref().map_or(0, |s| s.frames),
            spine_error: spine.and_then(|s| s.error),
            positions: count("SELECT COUNT(*) FROM position")?,
            branches: count("SELECT COUNT(*) FROM branch")?,
            outcomes,
            findings: count("SELECT COUNT(*) FROM finding")?,
            seen,
            cells: count("SELECT COUNT(*) FROM coverage")?,
            spine_cells: count("SELECT COUNT(*) FROM coverage WHERE spine = 1")?,
        })
    }

    /// FNV of the spine (without its wall time), position, branch (without its run id),
    /// finding (without its run ids), seen and coverage tables in key order (determinism
    /// tests, `explore digest`). `meta` and `run` (timings) are left out. Changing the column
    /// lists changes every recorded digest.
    pub fn content_digest(&self) -> Result<u64, DbError> {
        let mut h = engine::hash::Fnv64::new();
        for (table, cols, order) in [
            (
                "spine",
                "steps, done, frames, inputs, digest, error, build_engine, build_content",
                "id",
            ),
            ("position", "*", "idx"),
            (
                "branch",
                "id, origin, action, kind, outcome, explored, macros, explore_frames, \
                 recover_frames, window_frames, new_maps, new_scripts, new_flags, new_vars, \
                 new_cells, points, first_found, message",
                "id",
            ),
            (
                "finding",
                "id, signature, kind, branch, origin, macro, inputs, tail_frames, total_frame, \
                 map, x, y, message, location, script, log, build_engine, build_content, \
                 status, hits",
                "id",
            ),
            ("seen", "*", "kind, id"),
            ("coverage", "*", "map, x, y"),
        ] {
            h.write(table.as_bytes());
            h.write(&[0xFF]);
            let mut st = self
                .conn
                .prepare(&format!("SELECT {cols} FROM {table} ORDER BY {order}"))?;
            let n = st.column_count();
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                h.write(&[0xFE]);
                for i in 0..n {
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

/// `branch.explored`.
pub fn explored_name(e: crate::branch::Explored) -> &'static str {
    match e {
        crate::branch::Explored::Exhausted => "exhausted",
        crate::branch::Explored::LowNovelty => "low_novelty",
        crate::branch::Explored::Budget => "budget",
    }
}

/// Inserts a finding, or counts a hit on the row of the same signature (its `last_run`, and
/// `open` again on this build).
fn put_finding(
    tx: &Transaction<'_>,
    build: BuildHash,
    run: i64,
    f: &NewFinding<'_>,
) -> Result<(), DbError> {
    let d = f.draft;
    let sig = crate::findings::signature(d) as i64;
    let hit = tx.execute(
        "UPDATE finding SET hits = hits + 1, last_run = ?2, status = 'open', \
         build_engine = ?3, build_content = ?4 WHERE signature = ?1",
        params![sig, run, build.engine as i64, build.content as i64],
    )?;
    if hit > 0 {
        return Ok(());
    }
    let mut inputs = f.prefix.to_vec();
    inputs.extend_from_slice(&d.tail);
    let inputs = game_native::trace::rle(runs::keys(&inputs));
    let tail_frames = u64::from(runs::frames(&d.tail));
    tx.execute(
        "INSERT INTO finding (signature, kind, branch, origin, macro, inputs, tail_frames, \
         total_frame, map, x, y, message, location, script, log, build_engine, build_content, \
         status, hits, first_run, last_run) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
         ?11, ?12, ?13, ?14, ?15, ?16, ?17, 'open', 1, ?18, ?18)",
        params![
            sig,
            kind_str(d.kind),
            f.branch,
            f.origin,
            d.macro_.map(|m| m.to_text()),
            runs::to_blob(&inputs),
            tail_frames as i64,
            i64::from(runs::frames(&inputs)),
            d.map.0,
            d.pos.0,
            d.pos.1,
            d.message,
            d.location,
            d.script.map(|s| s.0),
            d.log.join("\n"),
            build.engine as i64,
            build.content as i64,
            run,
        ],
    )?;
    Ok(())
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
            assert_eq!(
                tables(&conn),
                [
                    "branch", "coverage", "finding", "meta", "position", "run", "seen", "spine"
                ]
            );
        }
    }

    #[test]
    fn strict_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_connection(&conn).unwrap();
        assert!(
            conn.execute(
                "INSERT INTO position (idx, frame, map, x, y, digest) VALUES (0, 'x', 0, 0, 0, 0)",
                []
            )
            .is_err()
        );
        conn.execute(
            "INSERT INTO position (idx, frame, map, x, y, digest) VALUES (0, 1, 0, 0, 0, -1)",
            [],
        )
        .unwrap();
        let d: i64 = conn
            .query_row("SELECT digest FROM position WHERE idx = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(d as u64, u64::MAX);
    }

    #[test]
    fn an_older_schema_is_refused_with_the_reset_hint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("explore.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); \
                 INSERT INTO meta VALUES ('schema', '3'), ('flavor', 'stub');",
            )
            .unwrap();
        }
        let e = Db::open(&path, BuildHash::default(), content::Flavor::Stub)
            .err()
            .expect("refused");
        assert!(matches!(e, DbError::Schema { found: 3, want: 4 }), "{e}");
        assert!(e.to_string().contains("explore reset --yes"), "{e}");
        let e = Db::open_read_only(&path).err().expect("refused");
        assert!(matches!(e, DbError::Schema { found: 3, .. }), "{e}");
    }

    #[test]
    fn errors_display() {
        let e = DbError::Schema { found: 2, want: 1 };
        assert!(e.to_string().contains('2'));
        let e: DbError = rusqlite::Error::InvalidQuery.into();
        assert!(matches!(e, DbError::Sql(_)));
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn cell_ids_are_distinct() {
        assert_ne!(cell_id(MapId(1), 0, 2), cell_id(MapId(1), 2, 0));
        assert_ne!(cell_id(MapId(1), -1, 0), cell_id(MapId(2), -1, 0));
    }
}
