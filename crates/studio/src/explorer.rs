//! The Explorer tab's core (M7 Task S1, reworked for the spine and its branches,
//! `docs/plans/2026-10-23-explorer-spine.md`): the explorer db read-only ([`ExplorerModel`]: the
//! spine's checkpoints, the branches, the findings; [`CoverageGrid`]), the `explore` child
//! process ([`ExplorerProc`]) and a finding as a NewGame trace to open in the native pane
//! ([`finding_trace`], then `NativeCmd::OpenTrace`).
//!
//! The tab's view logic is tested here so the UI stays thin: the run form ([`RunForm`],
//! [`parse_budget`] …), the run slot the UI keeps ([`RunSlot`], [`RunState`]), a poll
//! ([`poll`]), the stats, spine, branch and findings text ([`stats_rows`], [`checkpoint_cells`],
//! [`branch_cells`], [`finding_cells`]) and the per-finding actions ([`open_finding_cmds`],
//! [`export_repro`], [`repro_command`]).
//!
//! The studio never writes the db: `explore spine` and `explore branch` do, in their own process
//! (a panic there never takes the UI down), and the studio reads it through
//! `Db::open_read_only` while they run (WAL).

use crate::native::NativeCmd;
use crate::pane::RunMode;
use crate::paths::StudioPaths;
use anyhow::{Context, bail};
use engine::checkpoint::{BuildHash, Checkpoint, build_hash};
use engine::data::{GameData, MapId};
use explore::branch::Outcome;
use explore::db::{BranchRow, Db, DbStats, FindingRow, PositionRow};
use explore::findings;
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

/// What the Explorer tab shows of a db: its stats, the spine's checkpoints, the branches and
/// the findings.
#[derive(Clone, Debug, PartialEq)]
pub struct ExplorerModel {
    pub stats: DbStats,
    /// The spine's labelled positions, in spine order.
    pub checkpoints: Vec<PositionRow>,
    /// The branches the tab lists ([`sort_branches`], at most [`BRANCH_ROWS`]).
    pub branches: Vec<BranchRow>,
    /// Open findings first, then by hits (most first), then by id ([`sort_findings`]).
    pub findings: Vec<FindingRow>,
}

/// The most branches the tab lists.
pub const BRANCH_ROWS: usize = 100;

impl ExplorerModel {
    /// Reads the db at `path` read-only (works while `explore` writes it). A missing file is an
    /// error ("no explorer db"), never created.
    pub fn load(path: &Path) -> anyhow::Result<ExplorerModel> {
        let db = open(path)?;
        let stats = db.stats()?;
        let checkpoints = (db.positions()?.into_iter())
            .filter(|p| p.label.is_some())
            .collect();
        let mut branches = db.branches()?;
        sort_branches(&mut branches);
        branches.truncate(BRANCH_ROWS);
        let mut findings = db.findings()?;
        sort_findings(&mut findings);
        Ok(ExplorerModel {
            stats,
            checkpoints,
            branches,
            findings,
        })
    }

    /// The spine's checkpoint labels, in spine order.
    pub fn labels(&self) -> Vec<&str> {
        (self.checkpoints.iter())
            .filter_map(|p| p.label.as_deref())
            .collect()
    }
}

/// Branches that need a look first (bugs, unexpected divergences, stranded), then the others in
/// [`Outcome::NAMES`] order, then by novelty points (most first), then by id.
pub fn sort_branches(branches: &mut [BranchRow]) {
    let rank = |o: &str| match o {
        "bug" => 0,
        "diverged" => 1,
        "stranded" => 2,
        _ => 3 + Outcome::NAMES.iter().position(|n| *n == o).unwrap_or(9),
    };
    branches.sort_by_key(|b| (rank(&b.outcome), Reverse(b.points), b.id));
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

    /// `map`'s coverage in the db at `path`, sized to `game`'s map: per cell, the branches that
    /// stood on it, plus one when the spine did.
    pub fn load(path: &Path, game: &impl GameData, map: MapId) -> anyhow::Result<CoverageGrid> {
        let db = open(path)?;
        let m = game.map(map);
        let rows = db.coverage(Some(map))?;
        Ok(Self::from_rows(
            m.width,
            m.height,
            rows.into_iter()
                .map(|(_, x, y, spine, n)| (x, y, n.saturating_add(u32::from(spine)))),
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
    /// The whole trace from a new game: the spine's inputs to the branch's origin, then the
    /// tail.
    pub trace: Trace,
    /// The trace's start state (`None`: a new game, the only start the spine has).
    pub base: Option<Checkpoint>,
    /// The frame to open it at (`NativeCmd::OpenTrace`).
    pub frames: u32,
    /// Found on another build than this one (`row.build != build_hash(game)`): the replay may
    /// not reach the same state (the UI warns "found on another build").
    pub stale: bool,
}

/// Finding `id` as a NewGame trace (hashes by replay with `content::Emerald::for_shells()`)
/// and the frame to open it at: its last frame minus `before`, clamped to 0.
/// `NativeCmd::OpenTrace` stops at the frame itself, so the trace is replayed once here and once
/// in the pane.
pub fn finding_trace(db: &Db, id: i64, before: u32) -> anyhow::Result<FindingTrace> {
    let game = content::Emerald::for_shells();
    let row = db
        .finding(id)?
        .ok_or_else(|| anyhow::anyhow!("no finding {id}"))?;
    // A trace that panics before its end stops at the panic (the pane shows that frame).
    let trace = match findings::repro_trace(&game, &[], &row.inputs) {
        Ok(t) | Err((t, _)) => t,
    };
    let frames = trace.frames().saturating_sub(before);
    let stale = row.build != build_hash(&game);
    Ok(FindingTrace {
        row,
        trace,
        base: None,
        frames,
        stale,
    })
}

/// The arguments of `explore spine` or `explore branch` for the Explorer tab's start button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunArgs {
    pub kind: RunKind,
    /// A branch's frame budget; `None` = the config's.
    pub budget: Option<u32>,
    /// Branch workers; `None` = the explorer's default (`explore::workers::default_threads`).
    pub jobs: Option<usize>,
    /// Branch from this checkpoint's segment only; `None` = every origin.
    pub from: Option<String>,
    pub db: PathBuf,
}

impl RunArgs {
    /// `spine --db P`, or `branch [--budget F] [--jobs N] [--from L] --db P`, as `explore`'s
    /// CLI parses them.
    pub fn to_args(&self) -> Vec<String> {
        let mut v = vec![self.kind.key().to_string()];
        if self.kind == RunKind::Branch {
            if let Some(b) = self.budget {
                v.extend(["--budget".to_string(), b.to_string()]);
            }
            if let Some(j) = self.jobs {
                v.extend(["--jobs".to_string(), j.to_string()]);
            }
            if let Some(f) = &self.from {
                v.extend(["--from".to_string(), f.clone()]);
            }
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
/// stop: the db commits the spine, then all the branches, each in one WAL transaction, so a
/// kill loses at most the branches in flight, never the db.
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

/// What the Explorer tab's start button runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunKind {
    /// `explore spine`: the spine from a new game.
    Spine,
    /// `explore branch`: the spine, then branches off it.
    #[default]
    Branch,
}

impl RunKind {
    pub const ALL: [RunKind; 2] = [RunKind::Spine, RunKind::Branch];

    /// The `<option>` value (the `explore` command).
    pub fn key(self) -> &'static str {
        match self {
            RunKind::Spine => "spine",
            RunKind::Branch => "branch",
        }
    }

    /// The `<option>` text.
    pub fn label(self) -> &'static str {
        match self {
            RunKind::Spine => "spine only",
            RunKind::Branch => "spine + branches",
        }
    }

    pub fn from_key(key: &str) -> Option<RunKind> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }
}

/// The most branch workers the form accepts.
pub const MAX_JOBS: usize = 256;

/// A positive integer (decimal, or hex with `0x`), surrounding spaces ignored.
fn parse_u64(text: &str) -> Option<u64> {
    let t = text.trim();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => t.parse().ok(),
    }
}

/// The budget field: empty for the config's, else a positive number of frames per branch.
pub fn parse_budget(text: &str) -> Result<Option<u32>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_u64(text)
        .filter(|&n| n > 0)
        .and_then(|n| u32::try_from(n).ok())
        .map(Some)
        .ok_or_else(|| "the budget must be a positive number of frames, or empty".into())
}

/// The jobs field: empty for the explorer's default, else `1..=MAX_JOBS`.
pub fn parse_jobs(text: &str) -> Result<Option<usize>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    match text.trim().parse::<usize>() {
        Ok(n) if (1..=MAX_JOBS).contains(&n) => Ok(Some(n)),
        _ => Err(format!("jobs must be 1 to {MAX_JOBS}, or empty")),
    }
}

/// The from field: empty for every origin, else one checkpoint label.
pub fn parse_from(text: &str) -> Result<Option<String>, String> {
    let t = text.trim();
    if t.is_empty() {
        Ok(None)
    } else if t.contains(char::is_whitespace) {
        Err("from must be one checkpoint label".into())
    } else {
        Ok(Some(t.to_string()))
    }
}

/// "Open at frame before": frames before the finding's last frame (0 = the last frame).
pub fn parse_before(text: &str) -> Result<u32, String> {
    text.trim()
        .parse()
        .map_err(|_| "frames before must be a whole number".into())
}

/// The Explorer tab's run form, as typed.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RunForm {
    pub kind: RunKind,
    /// Frames per branch (empty: the config's).
    pub budget: String,
    pub jobs: String,
    pub from: String,
}

impl RunForm {
    /// The run's arguments on the db at `db`, or the first field error.
    pub fn args(&self, db: &Path) -> Result<RunArgs, String> {
        Ok(RunArgs {
            kind: self.kind,
            budget: parse_budget(&self.budget)?,
            jobs: parse_jobs(&self.jobs)?,
            from: parse_from(&self.from)?,
            db: db.to_path_buf(),
        })
    }
}

/// Where the studio's `explore spine` or `explore branch` is ([`RunSlot::state`]).
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

/// The studio's `explore` run, shared by the UI and its poll thread. Hold it in the app state:
/// dropping the last clone drops the [`ExplorerProc`], which kills the run. Every method may
/// wait on the process (spawn, kill and reap), so the UI calls them off its thread.
#[derive(Clone, Default)]
pub struct RunSlot(Arc<Mutex<Slot>>);

impl RunSlot {
    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts `explore <args>` logging to `log`, unless a run is going. A failed start is
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

/// The Explorer tab's last poll, with the branch rate averaged over the last [`RATE_POLLS`]
/// polls ([`next_poll`]).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Polled {
    /// `None` until the first poll.
    pub poll: Option<Poll>,
    pub rate: Option<f64>,
    /// `(when, branches)` of the recent polls that read the db, oldest first.
    samples: Vec<(Instant, u64)>,
}

impl Polled {
    pub fn running(&self) -> bool {
        self.poll.as_ref().is_some_and(|p| p.run.running())
    }
}

/// Polls the branch rate is averaged over.
pub const RATE_POLLS: usize = 5;

/// `prev` updated with poll `p` taken at `at`, and whether the db's totals changed (the Map
/// tab's coverage then reads the db again). The rate is the branches added per second over
/// the last [`RATE_POLLS`] polls; the window restarts when the db cannot be read or its branch
/// count goes down (a reset db).
pub fn next_poll(prev: &Polled, p: Poll, at: Instant) -> (Polled, bool) {
    let stats = |p: Option<&Poll>| {
        p.and_then(|p| p.model.as_ref().ok())
            .map(|m| m.stats.clone())
    };
    let (before, now) = (stats(prev.poll.as_ref()), stats(Some(&p)));
    let changed = before != now;
    let mut samples = prev.samples.clone();
    match &now {
        Some(n) if samples.last().is_none_or(|&(_, last)| n.branches >= last) => {
            samples.push((at, n.branches));
            if samples.len() > RATE_POLLS + 1 {
                samples.remove(0);
            }
        }
        Some(n) => samples = vec![(at, n.branches)],
        None => samples.clear(),
    }
    let rate = match (samples.first(), samples.last()) {
        (Some(&(t0, n0)), Some(&(t1, n1))) if samples.len() > 1 => {
            per_sec(n0, n1, t1.saturating_duration_since(t0))
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

/// Rows per second between two polls; `None` without time between them or when the count
/// went down (a reset db).
pub fn per_sec(prev: u64, now: u64, elapsed: Duration) -> Option<f64> {
    let secs = elapsed.as_secs_f64();
    (secs > 0.0 && now >= prev).then(|| (now - prev) as f64 / secs)
}

/// The novelty kinds' names in the stats, by `SeenKind` number.
const SEEN_NAMES: [(i64, &str); 5] = [
    (2, "maps"),
    (3, "scripts"),
    (0, "flags"),
    (1, "var values"),
    (4, "cells"),
];

/// The stats list: `(label, value)` rows.
pub fn stats_rows(m: &ExplorerModel, rate: Option<f64>) -> Vec<(&'static str, String)> {
    let s = &m.stats;
    let open = m.findings.iter().filter(|f| f.status == "open").count();
    let spine = if s.spine_steps == 0 {
        "not played".to_string()
    } else {
        let mut t = format!(
            "{} of {} steps, {} frames",
            s.spine_done, s.spine_steps, s.spine_frames
        );
        if let Some(e) = &s.spine_error {
            t.push_str(&format!(" (stopped: {e})"));
        }
        t
    };
    let labels = m.labels();
    let outcomes: Vec<String> = (s.outcomes.iter())
        .filter(|(_, n)| **n > 0)
        .map(|(o, n)| format!("{n} {o}"))
        .collect();
    let novelty: Vec<String> = SEEN_NAMES
        .iter()
        .map(|(k, n)| format!("{} {n}", s.seen.get(k).copied().unwrap_or(0)))
        .collect();
    vec![
        ("spine", spine),
        (
            "checkpoints",
            if labels.is_empty() {
                "none".into()
            } else {
                labels.join(", ")
            },
        ),
        (
            "branches",
            if outcomes.is_empty() {
                s.branches.to_string()
            } else {
                format!("{} ({})", s.branches, outcomes.join(", "))
            },
        ),
        ("branches/s", rate.map_or("-".into(), |r| format!("{r:.1}"))),
        ("beyond the spine", novelty.join(", ")),
        (
            "cells",
            format!("{} ({} by the spine)", s.cells, s.spine_cells),
        ),
        ("findings", format!("{} ({open} open)", s.findings)),
    ]
}

/// A spine checkpoint's cells in the spine table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointCells {
    pub label: String,
    /// The position (steps played).
    pub step: String,
    pub frame: String,
    pub map: String,
    pub cell: String,
}

pub fn checkpoint_cells(p: &PositionRow) -> CheckpointCells {
    CheckpointCells {
        label: p.label.clone().unwrap_or_default(),
        step: p.idx.to_string(),
        frame: p.frame.to_string(),
        map: map_name(p.map),
        cell: format!("({}, {})", p.pos.0, p.pos.1),
    }
}

/// A branch's cells in the branches table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchCells {
    pub id: String,
    /// The origin's spine position.
    pub origin: String,
    pub kind: String,
    pub action: String,
    pub outcome: String,
    /// Explore + recover + window frames.
    pub frames: String,
    /// New against the spine: maps, scripts, flags, var values, cells.
    pub novelty: String,
    /// The message's first line.
    pub message: String,
}

pub fn branch_cells(b: &BranchRow) -> BranchCells {
    BranchCells {
        id: b.id.to_string(),
        origin: b.origin.to_string(),
        kind: b.kind.clone(),
        action: b.action.clone(),
        outcome: b.outcome.clone(),
        frames: format!(
            "{}+{}+{}",
            b.explore_frames, b.recover_frames, b.window_frames
        ),
        novelty: format!(
            "{}m {}s {}f {}v {}c",
            b.new_maps, b.new_scripts, b.new_flags, b.new_vars, b.new_cells
        ),
        message: b.message.lines().next().unwrap_or("").to_string(),
    }
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
            branch: None,
            origin: None,
            macro_: None,
            inputs: Vec::new(),
            tail_frames: 0,
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
            first_run: 1,
            last_run: 1,
        }
    }

    fn branch(id: i64, outcome: &str, points: i64) -> BranchRow {
        BranchRow {
            id,
            origin: 3,
            action: "Macro((kind: Settle))".into(),
            kind: "warp".into(),
            outcome: outcome.into(),
            explored: Some("exhausted".into()),
            macros: Vec::new(),
            explore_frames: 100,
            recover_frames: 20,
            window_frames: 30,
            new_maps: 1,
            new_scripts: 2,
            new_flags: 3,
            new_vars: 4,
            new_cells: 5,
            points,
            first_found: 0,
            message: "first\nsecond".into(),
            run: 1,
        }
    }

    #[test]
    fn branches_needing_a_look_come_first() {
        let mut v = vec![
            branch(1, "rejoined", 50),
            branch(2, "diverged", 1),
            branch(3, "bug", 0),
            branch(4, "rejoined", 90),
            branch(5, "aborted", 99),
        ];
        sort_branches(&mut v);
        let ids: Vec<i64> = v.iter().map(|b| b.id).collect();
        assert_eq!(ids, [3, 2, 4, 1, 5]);
        let c = branch_cells(&v[0]);
        assert_eq!(c.frames, "100+20+30");
        assert_eq!(c.novelty, "1m 2s 3f 4v 5c");
        assert_eq!(c.message, "first");
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
            kind: RunKind::Spine,
            budget: Some(900),
            jobs: Some(4),
            from: None,
            db: db.clone(),
        };
        assert_eq!(
            a.to_args(),
            ["spine", "--db", "t/explore.db"],
            "spine ignores the rest"
        );
        let b = RunArgs {
            kind: RunKind::Branch,
            from: Some("pokedex".into()),
            ..a.clone()
        };
        assert_eq!(
            b.to_args(),
            [
                "branch",
                "--budget",
                "900",
                "--jobs",
                "4",
                "--from",
                "pokedex",
                "--db",
                "t/explore.db"
            ]
        );
        let c = RunArgs {
            kind: RunKind::Branch,
            budget: None,
            jobs: None,
            from: None,
            db,
        };
        assert_eq!(c.to_args(), ["branch", "--db", "t/explore.db"]);
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
        assert_eq!(parse_budget(" "), Ok(None));
        assert_eq!(parse_budget(" 6000 "), Ok(Some(6000)));
        assert_eq!(parse_budget("0x10"), Ok(Some(16)));
        for bad in [
            "0",
            "-3",
            "x",
            "1.5",
            &(u64::from(u32::MAX) + 1).to_string(),
        ] {
            assert!(parse_budget(bad).is_err(), "{bad:?}");
        }
        assert_eq!(parse_jobs(" "), Ok(None));
        assert_eq!(parse_jobs("4"), Ok(Some(4)));
        for bad in ["0", "-1", "x", &(MAX_JOBS + 1).to_string()] {
            assert!(parse_jobs(bad).is_err(), "{bad:?}");
        }
        assert_eq!(parse_from(""), Ok(None));
        assert_eq!(parse_from(" lab "), Ok(Some("lab".into())));
        assert!(parse_from("a b").is_err());
        assert_eq!(parse_before("120"), Ok(120));
        assert_eq!(parse_before("0"), Ok(0));
        assert!(parse_before("-1").is_err());
        for k in RunKind::ALL {
            assert_eq!(RunKind::from_key(k.key()), Some(k));
        }
        assert_eq!(RunKind::from_key("run"), None);

        let db = Path::new("t/explore.db");
        let form = RunForm::default();
        assert_eq!(
            form.args(db).unwrap().to_args(),
            ["branch", "--db", "t/explore.db"]
        );
        let form = RunForm {
            kind: RunKind::Branch,
            budget: "600".into(),
            jobs: "2".into(),
            from: String::new(),
        };
        assert_eq!(
            form.args(db).unwrap().to_args(),
            [
                "branch",
                "--budget",
                "600",
                "--jobs",
                "2",
                "--db",
                "t/explore.db"
            ]
        );
        let bad = RunForm {
            jobs: "0".into(),
            ..form.clone()
        };
        assert!(bad.args(db).unwrap_err().contains("jobs"));
    }

    #[test]
    fn stats_rows_name_the_spine_branches_and_rate() {
        let mut outcomes: std::collections::BTreeMap<String, u64> =
            Outcome::NAMES.iter().map(|n| (n.to_string(), 0)).collect();
        outcomes.insert("rejoined".into(), 30);
        outcomes.insert("bug".into(), 1);
        let mut m = ExplorerModel {
            stats: DbStats {
                spine_steps: 10,
                spine_done: 10,
                spine_frames: 5000,
                spine_error: None,
                positions: 11,
                branches: 31,
                outcomes,
                findings: 2,
                seen: [(2, 3), (4, 70)].into_iter().collect(),
                cells: 77,
                spine_cells: 40,
            },
            checkpoints: vec![PositionRow {
                idx: 4,
                label: Some("lab".into()),
                segment: Some("lab".into()),
                frame: 900,
                map: MapId(u16::MAX),
                pos: (6, 5),
                digest: 0,
            }],
            branches: Vec::new(),
            findings: vec![row(1, "open", 2), row(2, "stale", 1)],
        };
        let get =
            |rows: &[(&str, String)], k: &str| rows.iter().find(|r| r.0 == k).unwrap().1.clone();
        let rows = stats_rows(&m, None);
        assert_eq!(get(&rows, "spine"), "10 of 10 steps, 5000 frames");
        assert_eq!(get(&rows, "checkpoints"), "lab");
        assert_eq!(get(&rows, "branches"), "31 (1 bug, 30 rejoined)");
        assert_eq!(get(&rows, "branches/s"), "-");
        assert_eq!(
            get(&rows, "beyond the spine"),
            "3 maps, 0 scripts, 0 flags, 0 var values, 70 cells"
        );
        assert_eq!(get(&rows, "cells"), "77 (40 by the spine)");
        assert_eq!(get(&rows, "findings"), "2 (1 open)");
        m.stats.spine_error = Some("step 3: no".into());
        let rows = stats_rows(&m, Some(12.345));
        assert_eq!(get(&rows, "branches/s"), "12.3");
        assert!(get(&rows, "spine").ends_with("(stopped: step 3: no)"));
        let c = checkpoint_cells(&m.checkpoints[0]);
        assert_eq!(
            (c.label.as_str(), c.step.as_str(), c.cell.as_str()),
            ("lab", "4", "(6, 5)")
        );
        m.stats = DbStats::default();
        m.checkpoints.clear();
        let rows = stats_rows(&m, None);
        assert_eq!(get(&rows, "spine"), "not played");
        assert_eq!(get(&rows, "checkpoints"), "none");

        let s = Duration::from_secs(2);
        assert_eq!(per_sec(10, 30, s), Some(10.0));
        assert_eq!(per_sec(30, 10, s), None, "a reset db");
        assert_eq!(per_sec(10, 30, Duration::ZERO), None);
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

    fn poll_of(branches: Option<u64>, cells: u64) -> Poll {
        Poll {
            model: match branches {
                Some(branches) => Ok(ExplorerModel {
                    stats: DbStats {
                        branches,
                        cells,
                        ..DbStats::default()
                    },
                    checkpoints: Vec::new(),
                    branches: Vec::new(),
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
        // Branches commit in batches: the average smooths the bursts.
        let (p, _) = next_poll(&p, poll_of(Some(40), 1), at(4));
        assert_eq!(p.rate, Some(20.0));
        let (p, changed) = next_poll(&p, poll_of(Some(40), 1), at(6));
        assert!(!changed, "same totals");
        assert_eq!(p.rate, Some(10.0), "40 branches over 4 s");
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
        assert!(!building("spine (emerald act1): 78 of 78 steps\n"));
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
