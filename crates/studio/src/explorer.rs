//! The Explorer tab's core (M7 Task S1, `docs/plans/2026-10-22-milestone-7-explorer.md`): the
//! explorer db read-only ([`ExplorerModel`], [`CoverageGrid`]), the `explore` child process
//! ([`ExplorerProc`]) and a finding as a trace to open in the native pane ([`finding_trace`],
//! then `NativeCmd::OpenTrace`): a NewGame trace, or one from a `--from` db's root node.
//!
//! Task S2 adds the tab's view logic, tested here so the UI stays thin: the run form
//! ([`RunForm`], [`parse_budget`] …), the run slot the UI keeps ([`RunSlot`], [`RunState`]), a
//! poll ([`poll`]), the stats and findings text ([`stats_rows`], [`finding_cells`]) and the
//! per-finding actions ([`open_finding_cmds`], [`export_repro`], [`repro_command`]).
//!
//! The studio never writes the db: `explore run` does, in its own process (a panic there never
//! takes the UI down), and the studio reads it through `Db::open_read_only` while it runs (WAL).

use crate::native::NativeCmd;
use crate::pane::RunMode;
use crate::paths::StudioPaths;
use anyhow::{Context, bail};
use engine::checkpoint::{BuildHash, Checkpoint, build_hash};
use engine::data::{GameData, MapId};
use explore::config::Budget;
use explore::db::{Db, DbStats, FindingRow};
use explore::{goals, wire};
use game_native::trace::Trace;
use std::cmp::Reverse;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The frames "Open at frame before" goes back from a finding's last frame by default.
pub const OPEN_BEFORE: u32 = 120;

/// What the Explorer tab shows of a db: its stats and its findings.
#[derive(Clone, Debug, PartialEq)]
pub struct ExplorerModel {
    pub stats: DbStats,
    /// Open findings first, then by hits (most first), then by id ([`sort_findings`]).
    pub findings: Vec<FindingRow>,
}

impl ExplorerModel {
    /// Reads the db at `path` read-only (works while `explore run` writes it). A missing file
    /// is an error ("no explorer db"), never created.
    pub fn load(path: &Path) -> anyhow::Result<ExplorerModel> {
        let db = open(path)?;
        let stats = db.stats()?;
        let mut findings = db.findings()?;
        sort_findings(&mut findings);
        Ok(ExplorerModel { stats, findings })
    }

    /// The names of the goals reached, in story order (`goals::name` of `0..max_progress`).
    pub fn goals(&self) -> Vec<&'static str> {
        (0..self.stats.max_progress)
            .filter_map(goals::name)
            .collect()
    }
}

/// Open findings first, then by hits (most first), then by id.
pub fn sort_findings(findings: &mut [FindingRow]) {
    findings.sort_by_key(|f| (f.status != "open", Reverse(f.hits), f.id));
}

/// The explorer db at `path`, read-only.
pub fn open(path: &Path) -> anyhow::Result<Db> {
    if !path.is_file() {
        bail!("no explorer db at {}", path.display());
    }
    Db::open_read_only(path).with_context(|| format!("opening {}", path.display()))
}

/// Visits per map cell: `w * h` counts, row-major.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CoverageGrid {
    pub w: u16,
    pub h: u16,
    pub hits: Vec<u32>,
}

impl CoverageGrid {
    /// A `w x h` grid of `(x, y, hits)` rows. Rows outside the map are left out; a cell given
    /// twice adds up (saturating).
    pub fn from_rows(w: u16, h: u16, rows: impl IntoIterator<Item = (i16, i16, u32)>) -> Self {
        let mut hits = vec![0u32; w as usize * h as usize];
        for (x, y, n) in rows {
            if (0..w as i32).contains(&(x as i32)) && (0..h as i32).contains(&(y as i32)) {
                let c = &mut hits[y as usize * w as usize + x as usize];
                *c = c.saturating_add(n);
            }
        }
        CoverageGrid { w, h, hits }
    }

    /// `map`'s coverage in the db at `path`, sized to `game`'s map.
    pub fn load(path: &Path, game: &impl GameData, map: MapId) -> anyhow::Result<CoverageGrid> {
        let db = open(path)?;
        let m = game.map(map);
        let rows = db.coverage(Some(map))?;
        Ok(Self::from_rows(
            m.width,
            m.height,
            rows.into_iter().map(|(_, x, y, n)| (x, y, n)),
        ))
    }

    /// The hits on `(x, y)`, 0 outside the map.
    pub fn get(&self, x: i16, y: i16) -> u32 {
        if x < 0 || y < 0 || x as u16 >= self.w || y as u16 >= self.h {
            return 0;
        }
        self.hits[y as usize * self.w as usize + x as usize]
    }

    /// The visited cells as `(x, y, hits)`, row-major (by y, then x).
    pub fn rows(&self) -> Vec<(i16, i16, u32)> {
        let w = self.w.max(1) as usize;
        (self.hits.iter().enumerate())
            .filter(|&(_, &n)| n > 0)
            .map(|(i, &n)| ((i % w) as i16, (i / w) as i16, n))
            .collect()
    }

    /// How many cells were visited.
    pub fn visited(&self) -> usize {
        self.hits.iter().filter(|&&n| n > 0).count()
    }
}

/// A finding ready to open in the native pane ([`finding_trace`]).
#[derive(Clone, Debug, PartialEq)]
pub struct FindingTrace {
    pub row: FindingRow,
    /// The whole trace from the db's start: the node's lineage inputs, then the tail. Its
    /// parent is a new game, or the root node of a `--from` db.
    pub trace: Trace,
    /// The root node's state for a `--from` db (`None`: a new game).
    pub base: Option<Checkpoint>,
    /// The frame to open it at (`NativeCmd::OpenTrace`).
    pub frames: u32,
    /// Found on another build than this one (`row.build != build_hash(game)`): the replay may
    /// not reach the same state (the UI warns "found on another build").
    pub stale: bool,
}

/// Finding `id` as a trace from the db's start (hashes by replay with
/// `content::Emerald::for_shells()`) and the frame to open it at: its last frame minus
/// `before`, clamped to 0.
///
/// `wire::finding_trace_with_root` with nothing cut, keeping the root's state: a `--from` db's
/// root is materialized here, on the caller's worker thread, by its own `Materializer`
/// (`wire::db_root`) over the root store's disk cache, never by a pane's. `NativeCmd::OpenTrace`
/// stops at the frame itself, so the trace is replayed once here and once in the pane.
pub fn finding_trace(db: &Db, id: i64, before: u32) -> anyhow::Result<FindingTrace> {
    let game = content::Emerald::for_shells();
    let (row, trace, root) = wire::finding_trace_with_root(&game, db, id, 0)?;
    let frames = trace.frames().saturating_sub(before);
    let stale = row.build != build_hash(&game);
    Ok(FindingTrace {
        row,
        trace,
        base: root.map(|r| r.checkpoint.as_ref().clone()),
        frames,
        stale,
    })
}

/// The arguments of `explore run` for the Explorer tab's start button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunArgs {
    pub budget: Budget,
    /// `None` = the explorer's default (rayon's thread count).
    pub threads: Option<usize>,
    /// `None` = the config's seed. A db keeps the seed it was created with.
    pub seed: Option<u64>,
    pub db: PathBuf,
}

impl RunArgs {
    /// `run <budget> [--threads N] [--seed S] --db P`, as `explore`'s CLI parses them.
    pub fn to_args(&self) -> Vec<String> {
        let (flag, n) = match self.budget {
            Budget::Nodes(n) => ("--nodes", n),
            Budget::Frames(n) => ("--frames", n),
            Budget::Generations(n) => ("--generations", u64::from(n)),
            Budget::Secs(n) => ("--secs", n),
        };
        let mut v = vec!["run".to_string(), flag.to_string(), n.to_string()];
        if let Some(t) = self.threads {
            v.extend(["--threads".to_string(), t.to_string()]);
        }
        if let Some(s) = self.seed {
            v.extend(["--seed".to_string(), s.to_string()]);
        }
        v.extend(["--db".to_string(), self.db.display().to_string()]);
        v
    }
}

/// An `explore` child process, its stdout and stderr in a log file. Killed on drop.
///
/// It runs in its own process group ([`crate::proc`]): `stop` kills the whole group, so
/// stopping during the build also ends cargo's rustc processes. Once built, `cargo run`
/// replaces itself with the explorer (exec on unix). The kill is a SIGKILL, not a graceful
/// stop: the db commits one WAL transaction per generation, so a kill loses at most the
/// generation in flight (and the run's `run` row), never the db.
pub struct ExplorerProc {
    child: Child,
    log: PathBuf,
    /// Ended: reaped (`status` holds how), or no longer waitable (`status` is `None`).
    done: bool,
    status: Option<ExitStatus>,
}

impl ExplorerProc {
    /// `cargo run -p explore --release -- <args>` in the workspace ([`explore_command`]),
    /// logging to `log` (truncated; its directory is created).
    pub fn start(args: &[String], log: &Path) -> io::Result<ExplorerProc> {
        Self::spawn(explore_command(args), log)
    }

    /// Spawns `cmd` in a new process group, with no stdin and its output in `log`.
    pub fn spawn(mut cmd: Command, log: &Path) -> io::Result<ExplorerProc> {
        if let Some(dir) = log.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        let out = File::create(log)?;
        crate::proc::new_group(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(out.try_clone()?)
            .stderr(out);
        Ok(ExplorerProc {
            child: cmd.spawn()?,
            log: log.to_path_buf(),
            done: false,
            status: None,
        })
    }

    /// The log file.
    pub fn log(&self) -> &Path {
        &self.log
    }

    /// Whether the process still runs (false once it ended or can no longer be waited on).
    pub fn running(&mut self) -> bool {
        if !self.done {
            match self.child.try_wait() {
                Ok(None) => {}
                Ok(Some(st)) => (self.done, self.status) = (true, Some(st)),
                Err(_) => self.done = true,
            }
        }
        !self.done
    }

    /// How it ended; `None` while it runs, or when it could not be waited on.
    pub fn exit_status(&mut self) -> Option<ExitStatus> {
        self.running();
        self.status
    }

    /// Kills the process group (if the process still runs) and reaps the process.
    pub fn stop(&mut self) {
        if self.running() {
            crate::proc::kill_group(&mut self.child);
            self.status = self.child.wait().ok();
            self.done = true;
        }
    }
}

impl Drop for ExplorerProc {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `cargo run -p explore --release -- <args>` in the workspace (`$CARGO`, else `cargo`).
pub fn explore_command(args: &[String]) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(StudioPaths::detect().workspace)
        .args(["run", "-p", "explore", "--release", "--"])
        .args(args);
    cmd
}

// ---- The Explorer tab's view logic (M7 Task S2): pure helpers the UI calls. ----

/// The budget kinds the Explorer tab offers (`explore run --generations | --secs | --nodes`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BudgetKind {
    #[default]
    Generations,
    Secs,
    Nodes,
}

impl BudgetKind {
    pub const ALL: [BudgetKind; 3] = [BudgetKind::Generations, BudgetKind::Secs, BudgetKind::Nodes];

    /// The `<option>` value.
    pub fn key(self) -> &'static str {
        match self {
            BudgetKind::Generations => "generations",
            BudgetKind::Secs => "secs",
            BudgetKind::Nodes => "nodes",
        }
    }

    /// The `<option>` text.
    pub fn label(self) -> &'static str {
        match self {
            BudgetKind::Generations => "generations",
            BudgetKind::Secs => "seconds",
            BudgetKind::Nodes => "nodes",
        }
    }

    pub fn from_key(key: &str) -> Option<BudgetKind> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }
}

/// The most threads the form accepts.
pub const MAX_THREADS: usize = 256;

/// A positive integer (decimal, or hex with `0x`), surrounding spaces ignored.
fn parse_u64(text: &str) -> Option<u64> {
    let t = text.trim();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => t.parse().ok(),
    }
}

/// The budget field: a positive count of `kind` (generations fit a `u32`).
pub fn parse_budget(kind: BudgetKind, text: &str) -> Result<Budget, String> {
    let n = parse_u64(text)
        .filter(|&n| n > 0)
        .ok_or_else(|| format!("the budget must be a positive number of {}", kind.label()))?;
    Ok(match kind {
        BudgetKind::Generations => {
            Budget::Generations(u32::try_from(n).map_err(|_| "too many generations".to_string())?)
        }
        BudgetKind::Secs => Budget::Secs(n),
        BudgetKind::Nodes => Budget::Nodes(n),
    })
}

/// The seed field: empty for the config's seed, else a `u64` (decimal or `0x` hex).
pub fn parse_seed(text: &str) -> Result<Option<u64>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_u64(text)
        .map(Some)
        .ok_or_else(|| "the seed must be a number (decimal or 0x hex)".into())
}

/// The threads field: empty for the explorer's default, else `1..=MAX_THREADS`.
pub fn parse_threads(text: &str) -> Result<Option<usize>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    match text.trim().parse::<usize>() {
        Ok(n) if (1..=MAX_THREADS).contains(&n) => Ok(Some(n)),
        _ => Err(format!("threads must be 1 to {MAX_THREADS}, or empty")),
    }
}

/// "Open at frame before": frames before the finding's last frame (0 = the last frame).
pub fn parse_before(text: &str) -> Result<u32, String> {
    text.trim()
        .parse()
        .map_err(|_| "frames before must be a whole number".into())
}

/// The Explorer tab's run form, as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunForm {
    pub kind: BudgetKind,
    pub budget: String,
    pub threads: String,
    pub seed: String,
}

impl Default for RunForm {
    /// `--generations 10` (the CLI's default budget), default threads and seed.
    fn default() -> Self {
        RunForm {
            kind: BudgetKind::Generations,
            budget: "10".into(),
            threads: String::new(),
            seed: String::new(),
        }
    }
}

impl RunForm {
    /// The run's arguments on the db at `db`, or the first field error.
    pub fn args(&self, db: &Path) -> Result<RunArgs, String> {
        Ok(RunArgs {
            budget: parse_budget(self.kind, &self.budget)?,
            threads: parse_threads(&self.threads)?,
            seed: parse_seed(&self.seed)?,
            db: db.to_path_buf(),
        })
    }
}

/// Where the studio's `explore run` is ([`RunSlot::state`]).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum RunState {
    /// Never started in this studio.
    #[default]
    Idle,
    /// Cargo is building `explore` (the log's last line starts with `Compiling`).
    Building,
    Running,
    /// Ended with status 0 (its budget is spent).
    Finished,
    /// Ended by Stop.
    Stopped,
    /// Ended otherwise (a build error, a panic, a bad argument): the status and the log's
    /// last [`LOG_TAIL`] lines.
    Failed {
        status: String,
        tail: Vec<String>,
    },
    /// `cargo` could not be started.
    StartFailed(String),
}

impl RunState {
    /// Running, the build included.
    pub fn running(&self) -> bool {
        matches!(self, RunState::Running | RunState::Building)
    }
}

/// The run state's line next to the Start/Stop buttons.
pub fn run_state_text(s: &RunState) -> String {
    match s {
        RunState::Idle => "not running".into(),
        RunState::Building => "building explore…".into(),
        RunState::Running => "running (cargo run -p explore --release)".into(),
        RunState::Finished => "finished".into(),
        RunState::Stopped => "stopped".into(),
        RunState::Failed { status, .. } => format!("failed: {status}"),
        RunState::StartFailed(e) => format!("could not start: {e}"),
    }
}

/// Log lines shown under a failed run.
pub const LOG_TAIL: usize = 30;

/// The last `n` lines of `text`.
pub fn log_tail(text: &str, n: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

/// Whether a running `cargo run`'s log says it is still building: its last line starts with
/// `Compiling` (cargo indents its status lines).
pub fn building(log: &str) -> bool {
    log.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .is_some_and(|l| l.trim_start().starts_with("Compiling"))
}

/// An exit status as text: `exit code N` or `killed by a signal`.
fn status_text(st: Option<ExitStatus>) -> String {
    match st.and_then(|s| s.code()) {
        Some(c) => format!("exit code {c}"),
        None if st.is_some() => "killed by a signal".into(),
        None => "lost (could not wait on it)".into(),
    }
}

#[derive(Default)]
struct Slot {
    proc: Option<ExplorerProc>,
    stopped: bool,
    start_error: Option<String>,
}

/// The studio's `explore run`, shared by the UI and its poll thread. Hold it in the app state:
/// dropping the last clone drops the [`ExplorerProc`], which kills the run. Every method may
/// wait on the process (spawn, kill and reap), so the UI calls them off its thread.
#[derive(Clone, Default)]
pub struct RunSlot(Arc<Mutex<Slot>>);

impl RunSlot {
    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts `explore run <args>` logging to `log`, unless a run is going. A failed start is
    /// kept as the state ([`RunState::StartFailed`]).
    pub fn start(&self, args: &RunArgs, log: &Path) -> Result<(), String> {
        self.start_with(|| ExplorerProc::start(&args.to_args(), log))
    }

    /// [`start`](Self::start) with another process (tests).
    pub fn start_with(
        &self,
        spawn: impl FnOnce() -> io::Result<ExplorerProc>,
    ) -> Result<(), String> {
        let mut s = self.lock();
        if s.proc.as_mut().is_some_and(|p| p.running()) {
            return Err("a run is already going".into());
        }
        // The previous process (ended) is reaped by its drop.
        match spawn() {
            Ok(p) => {
                *s = Slot {
                    proc: Some(p),
                    ..Slot::default()
                };
                Ok(())
            }
            Err(e) => {
                let e = e.to_string();
                *s = Slot {
                    start_error: Some(e.clone()),
                    ..Slot::default()
                };
                Err(e)
            }
        }
    }

    /// Stops the run (kills its process group and reaps it), if one is going.
    pub fn stop(&self) {
        let mut s = self.lock();
        if let Some(p) = s.proc.as_mut()
            && p.running()
        {
            p.stop();
            s.stopped = true;
        }
    }

    /// The run's state; reads the log's tail when it failed.
    pub fn state(&self) -> RunState {
        let (status, log) = {
            let mut s = self.lock();
            if let Some(e) = &s.start_error {
                return RunState::StartFailed(e.clone());
            }
            let stopped = s.stopped;
            let Some(p) = s.proc.as_mut() else {
                return RunState::Idle;
            };
            if p.running() {
                (None, p.log().to_path_buf())
            } else if stopped {
                return RunState::Stopped;
            } else {
                let st = p.exit_status();
                if st.is_some_and(|st| st.success()) {
                    return RunState::Finished;
                }
                (Some(status_text(st)), p.log().to_path_buf())
            }
        };
        // Read outside the lock.
        let text = std::fs::read_to_string(&log);
        match status {
            None if text.as_deref().is_ok_and(building) => RunState::Building,
            None => RunState::Running,
            Some(status) => RunState::Failed {
                status,
                tail: log_tail(
                    &text.unwrap_or_else(|e| format!("(reading {}: {e})", log.display())),
                    LOG_TAIL,
                ),
            },
        }
    }
}

/// One poll of the Explorer tab: the db (or why it cannot be read), the run, and this build.
#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
    pub model: Result<ExplorerModel, String>,
    pub run: RunState,
    pub build: BuildHash,
}

/// Reads the db at `db` and the run's state (blocking: the UI calls it off its thread).
pub fn poll(db: &Path, run: &RunSlot) -> Poll {
    Poll {
        model: ExplorerModel::load(db).map_err(|e| format!("{e:#}")),
        run: run.state(),
        build: build_hash(&content::Emerald::for_shells()),
    }
}

/// The Explorer tab's last poll, with the node rate averaged over the last [`RATE_POLLS`]
/// polls ([`next_poll`]).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Polled {
    /// `None` until the first poll.
    pub poll: Option<Poll>,
    pub rate: Option<f64>,
    /// `(when, nodes)` of the recent polls that read the db, oldest first.
    samples: Vec<(Instant, u64)>,
}

impl Polled {
    pub fn running(&self) -> bool {
        self.poll.as_ref().is_some_and(|p| p.run.running())
    }
}

/// Polls the node rate is averaged over.
pub const RATE_POLLS: usize = 5;

/// `prev` updated with poll `p` taken at `at`, and whether the db's totals changed (the Map
/// tab's coverage then reads the db again). The rate is the nodes added per second over the
/// last [`RATE_POLLS`] polls; the window restarts when the db cannot be read or its node count
/// goes down (a reset db).
pub fn next_poll(prev: &Polled, p: Poll, at: Instant) -> (Polled, bool) {
    let stats = |p: Option<&Poll>| {
        p.and_then(|p| p.model.as_ref().ok())
            .map(|m| m.stats.clone())
    };
    let (before, now) = (stats(prev.poll.as_ref()), stats(Some(&p)));
    let changed = before != now;
    let mut samples = prev.samples.clone();
    match &now {
        Some(n) if samples.last().is_none_or(|&(_, last)| n.nodes >= last) => {
            samples.push((at, n.nodes));
            if samples.len() > RATE_POLLS + 1 {
                samples.remove(0);
            }
        }
        Some(n) => samples = vec![(at, n.nodes)],
        None => samples.clear(),
    }
    let rate = match (samples.first(), samples.last()) {
        (Some(&(t0, n0)), Some(&(t1, n1))) if samples.len() > 1 => {
            nodes_per_sec(n0, n1, t1.saturating_duration_since(t0))
        }
        _ => None,
    };
    let next = Polled {
        poll: Some(p),
        rate,
        samples,
    };
    (next, changed)
}

/// Nodes per second between two polls; `None` without time between them or when the count
/// went down (a reset db).
pub fn nodes_per_sec(prev: u64, now: u64, elapsed: Duration) -> Option<f64> {
    let secs = elapsed.as_secs_f64();
    (secs > 0.0 && now >= prev).then(|| (now - prev) as f64 / secs)
}

/// The stats list: `(label, value)` rows.
pub fn stats_rows(m: &ExplorerModel, rate: Option<f64>) -> Vec<(&'static str, String)> {
    let s = &m.stats;
    let goals = m.goals();
    let open = m.findings.iter().filter(|f| f.status == "open").count();
    vec![
        (
            "nodes",
            format!(
                "{} ({} valid, {} stale, {} merged, {} broken)",
                s.nodes, s.valid, s.stale, s.merged, s.broken
            ),
        ),
        ("nodes/s", rate.map_or("-".into(), |r| format!("{r:.1}"))),
        ("generation", s.generation.to_string()),
        (
            "goals",
            if goals.is_empty() {
                "none yet".into()
            } else {
                format!("{}: {}", goals.len(), goals.join(", "))
            },
        ),
        ("maps", s.maps.to_string()),
        ("cells", s.cells.to_string()),
        ("scripts", s.scripts.to_string()),
        ("findings", format!("{} ({open} open)", s.findings)),
    ]
}

/// What the findings table shows when a finding was recorded on another build.
pub const OTHER_BUILD: &str = "found on another build";

/// A finding's cells in the findings table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindingCells {
    pub id: String,
    pub kind: String,
    pub map: String,
    pub cell: String,
    /// The message's first line.
    pub message: String,
    pub hits: String,
    /// `open`, `stale` or `gone`.
    pub status: String,
    /// "this build" or [`OTHER_BUILD`].
    pub build: String,
    pub other_build: bool,
}

/// A map's name and id, `? (id)` for a map this build does not have.
pub fn map_name(m: MapId) -> String {
    let name = if content::has_map(m) {
        content::Emerald::for_shells().map(m).name
    } else {
        "?"
    };
    format!("{name} ({})", m.0)
}

/// Finding `f`'s cells; `build` is this build ([`Poll::build`]).
pub fn finding_cells(f: &FindingRow, build: BuildHash) -> FindingCells {
    let other_build = f.build != build;
    FindingCells {
        id: f.id.to_string(),
        kind: format!("{:?}", f.kind),
        map: map_name(f.map),
        cell: format!("({}, {})", f.pos.0, f.pos.1),
        message: f.message.lines().next().unwrap_or("").to_string(),
        hits: f.hits.to_string(),
        status: f.status.clone(),
        build: if other_build {
            OTHER_BUILD
        } else {
            "this build"
        }
        .into(),
        other_build,
    }
}

/// "Open at frame before": pause, then open the finding's trace at its frame in the native
/// pane (paused first, so no tick runs past the frame in between), then, for a finding from
/// another build, a note in the inspector's `last` ([`stale_note`]). The UI then shows the
/// Play tab and focuses the native pane.
pub fn open_finding_cmds(ft: FindingTrace) -> Vec<NativeCmd> {
    let note = stale_note(&ft);
    let mut cmds = vec![
        NativeCmd::Mode(RunMode::Pause),
        NativeCmd::OpenTrace {
            trace: ft.trace,
            frames: ft.frames,
            base: ft.base.map(Box::new),
        },
    ];
    cmds.extend(note.map(NativeCmd::Note));
    cmds
}

/// The warning for a finding found on another build, `None` for one from this build.
pub fn stale_note(ft: &FindingTrace) -> Option<String> {
    ft.stale.then(|| {
        format!(
            "finding {}: {OTHER_BUILD}, the replay may not reach it",
            ft.row.id
        )
    })
}

/// The note shown after opening a finding: where it opened, and a warning when it was found
/// on another build.
pub fn opened_text(ft: &FindingTrace) -> String {
    let mut s = format!("opened at frame {} of {}", ft.frames, ft.trace.frames());
    if let Some(n) = stale_note(ft) {
        s.push_str(&format!(" ({n})"));
    }
    s
}

/// The command "Copy" puts on the clipboard.
pub fn repro_command(id: i64) -> String {
    format!("cargo xtask explore repro {id}")
}

/// `explore export <id> --db P`: the finding as a NewGame trace under `target/explore/…/repro/`.
pub fn export_args(id: i64, db: &Path) -> Vec<String> {
    vec![
        "export".into(),
        id.to_string(),
        "--db".into(),
        db.display().to_string(),
    ]
}

/// An export's result: the CLI's last stdout line (`<path> (<n> frames)`), or its error (the
/// last stderr lines).
pub fn export_outcome(success: bool, stdout: &str, stderr: &str) -> Result<String, String> {
    if success {
        Ok(stdout.lines().last().unwrap_or("exported").to_string())
    } else {
        let tail = log_tail(stderr, 4).join("\n");
        Err(if tail.is_empty() {
            "export failed".into()
        } else {
            tail
        })
    }
}

/// Runs `explore export <id> --db P` through cargo in `children` and waits for it (blocking:
/// off the UI thread).
pub fn export_repro(children: &Children, id: i64, db: &Path) -> Result<String, String> {
    let (status, out, err) = children
        .output(explore_command(&export_args(id, db)))
        .map_err(|e| format!("starting cargo: {e}"))?;
    export_outcome(status.is_some_and(|s| s.success()), &out, &err)
}

type ChildList = (u64, Vec<(u64, Child)>);

/// Short-lived child processes the studio waits on (exports), each in its own process group,
/// so that [`Children::stop_all`] (at quit) ends them with what they started.
#[derive(Clone, Default)]
pub struct Children(Arc<Mutex<ChildList>>);

impl Children {
    fn lock(&self) -> std::sync::MutexGuard<'_, ChildList> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `cmd` (no stdin, its own process group) to its end: its status (`None` when it
    /// could not be waited on, or was ended by [`stop_all`](Self::stop_all)), stdout and
    /// stderr.
    pub fn output(&self, mut cmd: Command) -> io::Result<(Option<ExitStatus>, String, String)> {
        crate::proc::new_group(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        fn read(r: Option<impl io::Read + Send + 'static>) -> std::thread::JoinHandle<String> {
            std::thread::spawn(move || {
                let mut s = Vec::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_end(&mut s);
                }
                String::from_utf8_lossy(&s).into_owned()
            })
        }
        let out = read(child.stdout.take());
        let err = read(child.stderr.take());
        let key = {
            let mut g = self.lock();
            g.0 += 1;
            let key = g.0;
            g.1.push((key, child));
            key
        };
        let status = loop {
            {
                let mut g = self.lock();
                let Some(i) = g.1.iter().position(|(k, _)| *k == key) else {
                    break None; // stopped
                };
                match g.1[i].1.try_wait() {
                    Ok(None) => {}
                    Ok(Some(st)) => {
                        g.1.remove(i);
                        break Some(st);
                    }
                    Err(_) => {
                        g.1.remove(i);
                        break None;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let out = out.join().unwrap_or_default();
        let err = err.join().unwrap_or_default();
        Ok((status, out, err))
    }

    /// Kills every running child's process group and reaps it.
    pub fn stop_all(&self) {
        let running = std::mem::take(&mut self.lock().1);
        for (_, mut c) in running {
            crate::proc::kill_group(&mut c);
            let _ = c.wait();
        }
    }

    /// How many children run.
    pub fn len(&self) -> usize {
        self.lock().1.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use explore::FindingKind;

    fn row(id: i64, status: &str, hits: u32) -> FindingRow {
        FindingRow {
            id,
            signature: id as u64,
            kind: FindingKind::Panic,
            node: None,
            macro_: None,
            tail: Vec::new(),
            total_frame: 0,
            map: MapId(0),
            pos: (0, 0),
            message: String::new(),
            location: None,
            script: None,
            log: String::new(),
            build: BuildHash::default(),
            status: status.to_string(),
            hits,
            first_generation: 0,
            last_generation: 0,
        }
    }

    #[test]
    fn findings_are_open_first_then_by_hits_then_by_id() {
        let mut v = vec![
            row(1, "stale", 50),
            row(2, "open", 3),
            row(3, "open", 9),
            row(4, "gone", 1),
            row(5, "open", 3),
        ];
        sort_findings(&mut v);
        let ids: Vec<i64> = v.iter().map(|f| f.id).collect();
        assert_eq!(ids, [3, 2, 5, 1, 4]);
    }

    #[test]
    fn coverage_rows_round_trip_and_drop_cells_outside_the_map() {
        let rows = vec![(0, 0, 1), (3, 0, 7), (1, 2, 100), (2, 2, 12)];
        let g = CoverageGrid::from_rows(4, 3, rows.clone());
        assert_eq!(g.hits.len(), 12);
        assert_eq!(g.rows(), rows);
        assert_eq!(g.visited(), 4);
        assert_eq!((g.get(3, 0), g.get(1, 2), g.get(1, 1)), (7, 100, 0));
        assert_eq!((g.get(-1, 0), g.get(4, 0), g.get(0, 3)), (0, 0, 0));
        let out = CoverageGrid::from_rows(4, 3, [(-1, 0, 5), (4, 1, 5), (0, 3, 5), (0, 0, 2)]);
        assert_eq!(out.rows(), [(0, 0, 2)]);
        let twice = CoverageGrid::from_rows(2, 1, [(1, 0, 2), (1, 0, u32::MAX)]);
        assert_eq!(twice.get(1, 0), u32::MAX);
        assert_eq!(CoverageGrid::from_rows(0, 0, []).rows(), []);
    }

    #[test]
    fn run_args_are_the_cli_flags() {
        let db = PathBuf::from("t/explore.db");
        let a = RunArgs {
            budget: Budget::Secs(600),
            threads: None,
            seed: None,
            db: db.clone(),
        };
        assert_eq!(
            a.to_args(),
            ["run", "--secs", "600", "--db", "t/explore.db"]
        );
        let b = RunArgs {
            budget: Budget::Generations(30),
            threads: Some(4),
            seed: Some(7),
            db,
        };
        assert_eq!(
            b.to_args(),
            [
                "run",
                "--generations",
                "30",
                "--threads",
                "4",
                "--seed",
                "7",
                "--db",
                "t/explore.db"
            ]
        );
        let n = |budget| {
            RunArgs {
                budget,
                ..a.clone()
            }
            .to_args()[1..3]
                .to_vec()
        };
        assert_eq!(n(Budget::Nodes(5)), ["--nodes", "5"]);
        assert_eq!(n(Budget::Frames(9)), ["--frames", "9"]);
    }

    #[test]
    fn a_missing_db_is_an_error_and_is_not_created() {
        let p = std::env::temp_dir().join(format!("pkr-studio-nodb-{}.db", std::process::id()));
        let e = ExplorerModel::load(&p).unwrap_err();
        assert!(e.to_string().contains("no explorer db"), "{e}");
        assert!(!p.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_process_is_stopped_and_killed_on_drop() {
        let dir = std::env::temp_dir().join(format!("pkr-studio-proc-{}", std::process::id()));
        let log = dir.join("sub/explore.log");
        let mut sleep = Command::new("sleep");
        sleep.arg("30");
        let mut p = ExplorerProc::spawn(sleep, &log).unwrap();
        assert!(p.running());
        assert!(log.is_file());
        p.stop();
        assert!(!p.running());
        assert!(!p.exit_status().unwrap().success(), "killed");
        p.stop(); // twice is fine

        let mut sleep = Command::new("sleep");
        sleep.arg("30");
        let p = ExplorerProc::spawn(sleep, &log).unwrap();
        let pid = p.child.id() as i32;
        drop(p);
        assert!(!crate::proc::alive(pid), "pid {pid} still runs");

        // A grandchild (rustc under cargo) dies with the group.
        let mut sh = Command::new("sh");
        sh.args(["-c", "sleep 30 & echo $!; wait"]);
        let mut p = ExplorerProc::spawn(sh, &log).unwrap();
        let grandchild: i32 = loop {
            let text = std::fs::read_to_string(&log).unwrap();
            if let Some(line) = text.lines().next() {
                break line.trim().parse().unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(crate::proc::alive(grandchild));
        p.stop();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::proc::alive(grandchild) {
            assert!(
                std::time::Instant::now() < deadline,
                "the grandchild survived"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let mut echo = Command::new("sh");
        echo.args(["-c", "echo out; echo err >&2"]);
        let mut p = ExplorerProc::spawn(echo, &log).unwrap();
        while p.running() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(p.exit_status().unwrap().success());
        let text = std::fs::read_to_string(p.log()).unwrap();
        assert!(text.contains("out") && text.contains("err"), "{text}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_run_form_parses_and_validates() {
        use BudgetKind::*;
        assert_eq!(
            parse_budget(Generations, " 30 "),
            Ok(Budget::Generations(30))
        );
        assert_eq!(parse_budget(Secs, "600"), Ok(Budget::Secs(600)));
        assert_eq!(parse_budget(Nodes, "0x10"), Ok(Budget::Nodes(16)));
        for bad in ["", "0", "-3", "x", "1.5"] {
            assert!(parse_budget(Secs, bad).is_err(), "{bad:?}");
        }
        assert!(parse_budget(Generations, &(u64::from(u32::MAX) + 1).to_string()).is_err());
        assert_eq!(parse_seed(""), Ok(None));
        assert_eq!(parse_seed("7"), Ok(Some(7)));
        assert_eq!(parse_seed("0xff"), Ok(Some(255)));
        assert!(parse_seed("seven").is_err());
        assert_eq!(parse_threads(" "), Ok(None));
        assert_eq!(parse_threads("4"), Ok(Some(4)));
        for bad in ["0", "-1", "x", &(MAX_THREADS + 1).to_string()] {
            assert!(parse_threads(bad).is_err(), "{bad:?}");
        }
        assert_eq!(parse_before("120"), Ok(120));
        assert_eq!(parse_before("0"), Ok(0));
        assert!(parse_before("-1").is_err());
        for k in BudgetKind::ALL {
            assert_eq!(BudgetKind::from_key(k.key()), Some(k));
        }
        assert_eq!(BudgetKind::from_key("frames"), None);

        let db = Path::new("t/explore.db");
        let form = RunForm::default();
        let a = form.args(db).unwrap();
        assert_eq!(
            a.to_args(),
            ["run", "--generations", "10", "--db", "t/explore.db"]
        );
        let form = RunForm {
            kind: Secs,
            budget: "60".into(),
            threads: "2".into(),
            seed: "9".into(),
        };
        assert_eq!(
            form.args(db).unwrap().to_args(),
            [
                "run",
                "--secs",
                "60",
                "--threads",
                "2",
                "--seed",
                "9",
                "--db",
                "t/explore.db"
            ]
        );
        let bad = RunForm {
            threads: "0".into(),
            ..form.clone()
        };
        assert!(bad.args(db).unwrap_err().contains("threads"));
    }

    #[test]
    fn stats_rows_name_the_goals_and_the_rate() {
        let mut m = ExplorerModel {
            stats: DbStats {
                nodes: 40,
                valid: 30,
                stale: 5,
                merged: 4,
                broken: 1,
                findings: 2,
                maps: 3,
                cells: 77,
                scripts: 6,
                max_progress: 0,
                generation: 9,
            },
            findings: vec![row(1, "open", 2), row(2, "gone", 1)],
        };
        let get =
            |rows: &[(&str, String)], k: &str| rows.iter().find(|r| r.0 == k).unwrap().1.clone();
        let rows = stats_rows(&m, None);
        assert_eq!(
            get(&rows, "nodes"),
            "40 (30 valid, 5 stale, 4 merged, 1 broken)"
        );
        assert_eq!(get(&rows, "nodes/s"), "-");
        assert_eq!(get(&rows, "goals"), "none yet");
        assert_eq!(get(&rows, "cells"), "77");
        assert_eq!(get(&rows, "findings"), "2 (1 open)");
        m.stats.max_progress = 2;
        let rows = stats_rows(&m, Some(12.345));
        assert_eq!(get(&rows, "nodes/s"), "12.3");
        let names: Vec<_> = (0..2).map(|i| goals::name(i).unwrap()).collect();
        assert_eq!(get(&rows, "goals"), format!("2: {}", names.join(", ")));

        let s = Duration::from_secs(2);
        assert_eq!(nodes_per_sec(10, 30, s), Some(10.0));
        assert_eq!(nodes_per_sec(30, 10, s), None, "a reset db");
        assert_eq!(nodes_per_sec(10, 30, Duration::ZERO), None);
    }

    #[test]
    fn finding_cells_mark_another_build() {
        let game = content::Emerald::for_shells();
        let this = build_hash(&game);
        let mut f = row(7, "open", 3);
        f.map = content::data::MAP_IDS[0];
        f.pos = (4, -1);
        f.message = "first line\nsecond".into();
        f.build = this;
        let c = finding_cells(&f, this);
        assert_eq!(c.id, "7");
        assert_eq!(c.kind, "Panic");
        assert_eq!(c.map, format!("{} ({})", game.map(f.map).name, f.map.0));
        assert_eq!(c.cell, "(4, -1)");
        assert_eq!(c.message, "first line");
        assert_eq!((c.hits.as_str(), c.status.as_str()), ("3", "open"));
        assert_eq!((c.build.as_str(), c.other_build), ("this build", false));
        f.build = BuildHash::default();
        let c = finding_cells(&f, this);
        assert_eq!((c.build.as_str(), c.other_build), (OTHER_BUILD, true));
        assert!(map_name(MapId(u16::MAX)).starts_with("? "));
    }

    #[test]
    fn open_export_and_copy_commands() {
        let trace = Trace {
            schema: 0,
            parent: game_native::trace::Parent::NewGame,
            build: BuildHash::default(),
            start_frame: 0,
            inputs: Vec::new(),
            hashes: Vec::new(),
        };
        let ft = FindingTrace {
            row: row(3, "open", 1),
            trace,
            base: None,
            frames: 0,
            stale: true,
        };
        assert!(opened_text(&ft).contains(OTHER_BUILD));
        let note = stale_note(&ft).unwrap();
        assert!(
            note.contains(OTHER_BUILD) && note.contains("finding 3"),
            "{note}"
        );
        // Paused first, then opened, then the stale-build note for the inspector.
        match &open_finding_cmds(ft.clone())[..] {
            [
                NativeCmd::Mode(RunMode::Pause),
                NativeCmd::OpenTrace { frames: 0, .. },
                NativeCmd::Note(n),
            ] => assert_eq!(*n, note),
            c => panic!("{} commands", c.len()),
        }
        let fresh = FindingTrace { stale: false, ..ft };
        assert_eq!(stale_note(&fresh), None);
        assert!(!opened_text(&fresh).contains(OTHER_BUILD));
        assert!(matches!(
            open_finding_cmds(fresh)[..],
            [
                NativeCmd::Mode(RunMode::Pause),
                NativeCmd::OpenTrace { frames: 0, .. }
            ]
        ));
        assert_eq!(repro_command(42), "cargo xtask explore repro 42");
        assert_eq!(
            export_args(42, Path::new("d/explore.db")),
            ["export", "42", "--db", "d/explore.db"]
        );
        assert_eq!(
            export_outcome(true, "x\nrepro/finding-42.ron (90 frames)\n", "Compiling"),
            Ok("repro/finding-42.ron (90 frames)".into())
        );
        assert_eq!(
            export_outcome(false, "", "a\nb\nc\nd\nError: no finding 42\n"),
            Err("b\nc\nd\nError: no finding 42".into())
        );
        assert_eq!(export_outcome(false, "", ""), Err("export failed".into()));
        assert_eq!(log_tail("a\nb\nc", 2), ["b", "c"]);
        assert_eq!(log_tail("a", 5), ["a"]);
        assert!(log_tail("", 5).is_empty());
    }

    fn poll_of(nodes: Option<u64>, cells: u64) -> Poll {
        Poll {
            model: match nodes {
                Some(nodes) => Ok(ExplorerModel {
                    stats: DbStats {
                        nodes,
                        cells,
                        ..DbStats::default()
                    },
                    findings: Vec::new(),
                }),
                None => Err("no explorer db".into()),
            },
            run: RunState::Idle,
            build: BuildHash::default(),
        }
    }

    #[test]
    fn next_poll_averages_the_rate_and_flags_changed_totals() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let (p, changed) = next_poll(&Polled::default(), poll_of(None, 0), at(0));
        assert!(!changed, "no db before or after");
        assert_eq!(p.rate, None);
        let (p, changed) = next_poll(&p, poll_of(Some(0), 0), at(2));
        assert!(changed, "the db appeared");
        assert_eq!(p.rate, None, "one sample");
        // A generation commits every other poll: the average smooths the bursts.
        let (p, _) = next_poll(&p, poll_of(Some(40), 1), at(4));
        assert_eq!(p.rate, Some(20.0));
        let (p, changed) = next_poll(&p, poll_of(Some(40), 1), at(6));
        assert!(!changed, "same totals");
        assert_eq!(p.rate, Some(10.0), "40 nodes over 4 s");
        let mut q = p;
        for (i, n) in [80, 80, 120, 120, 160].into_iter().enumerate() {
            q = next_poll(&q, poll_of(Some(n), 1), at(8 + 2 * i as u64)).0;
        }
        // The window keeps the last RATE_POLLS intervals: from t=6 (40) to t=16 (160).
        assert_eq!(q.samples.len(), RATE_POLLS + 1);
        assert_eq!(q.rate, Some((160.0 - 40.0) / 10.0));
        // A reset db restarts the window; an unreadable db clears it.
        let (r, changed) = next_poll(&q, poll_of(Some(5), 1), at(30));
        assert!(changed);
        assert_eq!((r.rate, r.samples.len()), (None, 1));
        let (r, changed) = next_poll(&r, poll_of(None, 0), at(32));
        assert!(changed);
        assert_eq!((r.rate, r.samples.len()), (None, 0));
        assert!(!r.running());
        assert!(r.poll.is_some());
    }

    #[test]
    fn the_build_shows_in_the_log() {
        assert!(building("   Compiling explore v0.1.0 (/x)\n"));
        assert!(building(
            "   Compiling content v0.1.0\n   Compiling explore\n\n"
        ));
        assert!(!building(
            "   Compiling explore\n    Finished `release`\n     Running `x`\n"
        ));
        assert!(!building("gen 10 nodes 300\n"));
        assert!(!building(""));
        assert_eq!(run_state_text(&RunState::Building), "building explore…");
        assert!(RunState::Building.running() && RunState::Running.running());
        assert!(!RunState::Finished.running());
    }

    #[cfg(unix)]
    #[test]
    fn children_report_their_output_and_stop_all_ends_them() {
        let kids = Children::default();
        let mut c = Command::new("sh");
        c.args(["-c", "echo out; echo err >&2; exit 2"]);
        let (st, out, err) = kids.output(c).unwrap();
        assert_eq!(st.and_then(|s| s.code()), Some(2));
        assert_eq!((out.as_str(), err.as_str()), ("out\n", "err\n"));
        assert!(kids.is_empty());

        // A long child (with a grandchild) is ended by stop_all from another thread.
        let k2 = kids.clone();
        let t = std::thread::spawn(move || {
            let mut c = Command::new("sh");
            c.args(["-c", "sleep 30 & echo $!; wait"]);
            k2.output(c).unwrap()
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while kids.is_empty() {
            assert!(std::time::Instant::now() < deadline, "never started");
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(100)); // the grandchild starts
        kids.stop_all();
        let (st, out, _) = t.join().unwrap();
        assert_eq!(st, None, "stopped");
        assert!(kids.is_empty());
        let grandchild: i32 = out.trim().parse().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while crate::proc::alive(grandchild) {
            assert!(
                std::time::Instant::now() < deadline,
                "the grandchild survived"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_run_slot_reports_running_finished_failed_and_stopped() {
        let dir = std::env::temp_dir().join(format!("pkr-studio-slot-{}", std::process::id()));
        let log = dir.join("explore.log");
        let sh = |script: &str| {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            ExplorerProc::spawn(c, &log)
        };
        let wait_end = |slot: &RunSlot| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while slot.state().running() {
                assert!(std::time::Instant::now() < deadline, "still running");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let slot = RunSlot::default();
        assert_eq!(slot.state(), RunState::Idle);
        slot.start_with(|| sh("sleep 30")).unwrap();
        assert_eq!(slot.state(), RunState::Running);
        assert!(slot.start_with(|| sh("true")).is_err(), "one run at a time");
        let other = slot.clone();
        other.stop();
        assert_eq!(slot.state(), RunState::Stopped);

        slot.start_with(|| sh("true")).unwrap();
        wait_end(&slot);
        assert_eq!(slot.state(), RunState::Finished);

        slot.start_with(|| sh("echo '   Compiling explore v0.1.0'; sleep 30"))
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while slot.state() != RunState::Building {
            assert!(std::time::Instant::now() < deadline, "{:?}", slot.state());
            std::thread::sleep(Duration::from_millis(5));
        }
        slot.stop();
        assert_eq!(slot.state(), RunState::Stopped);

        slot.start_with(|| sh("echo one; echo two >&2; exit 3"))
            .unwrap();
        wait_end(&slot);
        match slot.state() {
            RunState::Failed { status, tail } => {
                assert_eq!(status, "exit code 3");
                assert_eq!(tail, ["one", "two"]);
            }
            s => panic!("{s:?}"),
        }
        assert!(run_state_text(&slot.state()).starts_with("failed: exit code 3"));

        let e = slot
            .start_with(|| Err(io::Error::other("no cargo")))
            .unwrap_err();
        assert_eq!(slot.state(), RunState::StartFailed(e));

        // Dropping the last clone kills the run.
        slot.start_with(|| sh("sleep 30")).unwrap();
        let pid = slot.lock().proc.as_ref().unwrap().child.id() as i32;
        drop((slot, other));
        assert!(!crate::proc::alive(pid), "pid {pid} still runs");
        let _ = std::fs::remove_dir_all(dir);
    }
}
