//! The Explorer tab's core (M7 Task S1, `docs/plans/2026-10-22-milestone-7-explorer.md`): the
//! explorer db read-only ([`ExplorerModel`], [`CoverageGrid`]), the `explore` child process
//! ([`ExplorerProc`]) and a finding as a NewGame trace to open in the native pane
//! ([`finding_trace`], then `NativeCmd::OpenTrace`).
//!
//! The studio never writes the db: `explore run` does, in its own process (a panic there never
//! takes the UI down), and the studio reads it through `Db::open_read_only` while it runs (WAL).

use crate::paths::StudioPaths;
use anyhow::{Context, bail};
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

/// Finding `id` as a NewGame trace (its node's lineage inputs, then its tail; hashes by replay
/// with `content::Emerald::for_shells()`) and the frame to open it at: its last frame minus
/// `before`, clamped to 0. `wire::finding_trace` with nothing cut: `NativeCmd::OpenTrace` stops
/// at the frame itself, so the trace is replayed once here and once in the pane.
pub fn finding_trace(db: &Db, id: i64, before: u32) -> anyhow::Result<(Trace, u32)> {
    let (_, trace) = wire::finding_trace(&content::Emerald::for_shells(), db, id, 0)?;
    let frames = trace.frames().saturating_sub(before);
    Ok((trace, frames))
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
/// `cargo run` replaces itself with the program on Unix (exec), so once the build is done the
/// child is the explorer itself and `stop` ends it. The db commits one transaction per
/// generation, so a kill loses at most the generation in flight (and the run's `run` row).
pub struct ExplorerProc {
    child: Child,
    log: PathBuf,
    status: Option<ExitStatus>,
}

impl ExplorerProc {
    /// `cargo run -p explore --release -- <args>` in the workspace (`$CARGO`, else `cargo`),
    /// logging to `log` (truncated; its directory is created).
    pub fn start(args: &[String], log: &Path) -> io::Result<ExplorerProc> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = Command::new(cargo);
        cmd.current_dir(StudioPaths::detect().workspace)
            .args(["run", "-p", "explore", "--release", "--"])
            .args(args);
        Self::spawn(cmd, log)
    }

    /// Spawns `cmd` with no stdin and its output in `log`.
    pub fn spawn(mut cmd: Command, log: &Path) -> io::Result<ExplorerProc> {
        if let Some(dir) = log.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        let out = File::create(log)?;
        cmd.stdin(Stdio::null())
            .stdout(out.try_clone()?)
            .stderr(out);
        Ok(ExplorerProc {
            child: cmd.spawn()?,
            log: log.to_path_buf(),
            status: None,
        })
    }

    /// The log file.
    pub fn log(&self) -> &Path {
        &self.log
    }

    /// Whether the process still runs.
    pub fn running(&mut self) -> bool {
        self.exit_status().is_none()
    }

    /// How it ended, `None` while it runs.
    pub fn exit_status(&mut self) -> Option<ExitStatus> {
        if self.status.is_none() {
            self.status = self.child.try_wait().ok().flatten();
        }
        self.status
    }

    /// Kills the process (if it still runs) and reaps it.
    pub fn stop(&mut self) {
        if self.running() {
            let _ = self.child.kill();
        }
        if self.status.is_none() {
            self.status = self.child.wait().ok();
        }
    }
}

impl Drop for ExplorerProc {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::checkpoint::BuildHash;
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
        let pid = p.child.id();
        drop(p);
        // The pid is reaped: `kill -0` finds no such process.
        let alive = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!alive, "pid {pid} still runs");

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
}
