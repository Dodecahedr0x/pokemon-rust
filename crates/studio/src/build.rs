//! One-click ROM build (plan: Task D): `cargo xtask rom` as a child process whose stdout and
//! stderr are collected line by line, polled without blocking from the UI.
//!
//! On unix the build runs in its own process group, so dropping a running build kills the
//! xtask and the inner cargo with it, not only the top-level `cargo`.

use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// After the child exits, how long `poll` keeps reporting `Running` while its output pipes
/// are still open (a grandchild may hold them) before it reports the final state with the
/// lines received so far.
pub const DRAIN_GRACE: Duration = Duration::from_millis(500);

/// Where a [`RomBuild`] is. The lines are stdout and stderr interleaved in arrival order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildState {
    Running(Vec<String>),
    /// The build succeeded: `target/game.gba` is fresh (the UI reloads the ROM pane). The
    /// lines keep the warnings and the budget summary.
    Ok(Vec<String>),
    /// The build failed or could not be waited on; the last line says how it exited.
    Failed(Vec<String>),
}

/// A running (or finished) `cargo xtask rom`.
pub struct RomBuild {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
    readers: Vec<JoinHandle<()>>,
    /// The child's exit, and when `poll` saw it.
    exited: Option<(io::Result<ExitStatus>, Instant)>,
    done: Option<BuildState>,
}

/// The build command: `cargo xtask rom` in `workspace`. `$CARGO` (set when the studio itself
/// runs under cargo) is preferred over the `cargo` on the `PATH`.
pub fn rom_command(workspace: &Path) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut c = Command::new(cargo);
    c.args(["xtask", "rom"]).current_dir(workspace);
    c
}

#[cfg(unix)]
unsafe extern "C" {
    /// libc's `kill(2)`.
    fn kill(pid: i32, sig: i32) -> i32;
}
#[cfg(unix)]
const SIGKILL: i32 = 9;

impl RomBuild {
    /// Starts `cargo xtask rom` in `workspace`.
    pub fn start(workspace: &Path) -> io::Result<RomBuild> {
        Self::spawn(rom_command(workspace))
    }

    /// Starts `cmd` with its output captured (stdin closed), in a new process group on unix.
    pub fn spawn(mut cmd: Command) -> io::Result<RomBuild> {
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let lines = Arc::new(Mutex::new(Vec::new()));
        let mut readers = Vec::new();
        let out: Option<Box<dyn Read + Send>> = child.stdout.take().map(|o| Box::new(o) as _);
        let err: Option<Box<dyn Read + Send>> = child.stderr.take().map(|e| Box::new(e) as _);
        for pipe in [out, err].into_iter().flatten() {
            let lines = Arc::clone(&lines);
            readers.push(std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines() {
                    let Ok(line) = line else { break };
                    lines.lock().unwrap_or_else(|e| e.into_inner()).push(line);
                }
            }));
        }
        Ok(RomBuild {
            child,
            lines,
            readers,
            exited: None,
            done: None,
        })
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn drained(&self) -> bool {
        self.readers.iter().all(|r| r.is_finished())
    }

    /// Never blocks. After the child exits, its output is drained until the pipes close, or
    /// for at most [`DRAIN_GRACE`]; then the final state is returned on every later call.
    pub fn poll(&mut self) -> BuildState {
        if let Some(done) = &self.done {
            return done.clone();
        }
        if self.exited.is_none() {
            match self.child.try_wait() {
                Ok(None) => return BuildState::Running(self.lines()),
                Ok(Some(s)) => self.exited = Some((Ok(s), Instant::now())),
                Err(e) => self.exited = Some((Err(e), Instant::now())),
            }
        }
        let drained = self.drained();
        let (status, at) = self.exited.as_ref().expect("set above");
        if !drained && at.elapsed() < DRAIN_GRACE {
            return BuildState::Running(self.lines());
        }
        let mut lines = self.lines();
        let state = match status {
            Ok(s) if s.success() => BuildState::Ok(lines),
            Ok(s) => {
                lines.push(format!("build exited with {s}"));
                BuildState::Failed(lines)
            }
            Err(e) => {
                lines.push(format!("waiting for the build failed: {e}"));
                BuildState::Failed(lines)
            }
        };
        if drained {
            // Finished threads: joining does not block.
            for r in self.readers.drain(..) {
                let _ = r.join();
            }
        }
        self.done = Some(state.clone());
        state
    }

    /// Kills the build's process group (unix) and the child, then reaps the child.
    fn kill(&mut self) {
        #[cfg(unix)]
        if let Ok(pgid) = i32::try_from(self.child.id()) {
            // SAFETY: a plain syscall. `-pgid` names the group `spawn` created; its id is the
            // child's pid, not reused while the child is unreaped (we reap it just below) or
            // while the group still has members.
            unsafe {
                kill(-pgid, SIGKILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for RomBuild {
    /// A build still running when its handle goes (the studio closes) is killed with its
    /// whole process group, not orphaned; so are leftovers still holding its output pipes.
    fn drop(&mut self) {
        let running = self.exited.is_none() && matches!(self.child.try_wait(), Ok(None));
        if running || !self.drained() {
            self.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_command_is_cargo_xtask_rom_in_the_workspace() {
        let c = rom_command(Path::new("/some/workspace"));
        let program = c.get_program().to_string_lossy().into_owned();
        assert!(
            Path::new(&program)
                .file_stem()
                .is_some_and(|s| s.to_string_lossy().starts_with("cargo")),
            "{program}"
        );
        let args: Vec<_> = c.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["xtask", "rom"]);
        assert_eq!(c.get_current_dir(), Some(Path::new("/some/workspace")));
    }

    /// Polls until a final state; every single `poll` must return promptly.
    fn wait(b: &mut RomBuild) -> BuildState {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let t = Instant::now();
            let s = b.poll();
            assert!(t.elapsed() < Duration::from_millis(100), "poll blocked");
            match s {
                BuildState::Running(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                s => return s,
            }
        }
    }

    #[cfg(unix)]
    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    #[cfg(unix)]
    fn alive(pid: i32) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { kill(pid, 0) == 0 }
    }

    /// Whether `pid` is gone within a few seconds (an orphan is reaped by init).
    #[cfg(unix)]
    fn gone(pid: i32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(pid) {
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    #[cfg(unix)]
    #[test]
    fn failed_build_keeps_both_streams_and_the_exit() {
        let mut b = RomBuild::spawn(sh("echo out; echo err >&2; exit 3")).unwrap();
        let BuildState::Failed(lines) = wait(&mut b) else {
            panic!("expected Failed");
        };
        assert!(lines.contains(&"out".to_string()), "{lines:?}");
        assert!(lines.contains(&"err".to_string()), "{lines:?}");
        assert!(lines.last().unwrap().contains('3'), "{lines:?}");
        // Final states are sticky.
        assert_eq!(b.poll(), BuildState::Failed(lines));
    }

    #[cfg(unix)]
    #[test]
    fn running_then_ok_with_its_lines() {
        let mut b = RomBuild::spawn(sh("echo first; sleep 0.3")).unwrap();
        assert!(matches!(b.poll(), BuildState::Running(_)));
        let BuildState::Ok(lines) = wait(&mut b) else {
            panic!("expected Ok");
        };
        assert_eq!(lines, ["first"]);
        assert_eq!(b.poll(), BuildState::Ok(lines));
    }

    #[cfg(unix)]
    #[test]
    fn a_grandchild_holding_the_pipe_neither_blocks_poll_nor_outlives_the_build() {
        let mut b = RomBuild::spawn(sh("sleep 30 & echo $!; exit 0")).unwrap();
        let BuildState::Ok(lines) = wait(&mut b) else {
            panic!("expected Ok");
        };
        let pid: i32 = lines[0].parse().unwrap();
        assert!(alive(pid), "the grandchild still runs");
        drop(b);
        assert!(gone(pid), "the grandchild survived the build's drop");
    }

    #[cfg(unix)]
    #[test]
    fn dropping_a_running_build_kills_its_process_group() {
        let mut b = RomBuild::spawn(sh("sleep 30 & echo $!; wait")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let pid: i32 = loop {
            match b.poll() {
                BuildState::Running(l) if !l.is_empty() => break l[0].parse().unwrap(),
                BuildState::Running(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                s => panic!("{s:?}"),
            }
        };
        let child = b.child.id() as i32;
        drop(b);
        assert!(gone(pid), "the grandchild survived");
        assert!(!alive(child), "the child was not reaped");
    }

    #[test]
    fn a_missing_program_is_an_error_not_a_panic() {
        assert!(RomBuild::spawn(Command::new("/nonexistent/definitely-not-cargo")).is_err());
    }
}
