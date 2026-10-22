//! One-click ROM build (plan: Task D): `cargo xtask rom` as a child process whose stdout and
//! stderr are collected line by line, polled without blocking from the UI.

use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Where a [`RomBuild`] is. The lines are stdout and stderr interleaved in arrival order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildState {
    Running(Vec<String>),
    /// The build succeeded: `target/game.gba` is fresh (the UI reloads the ROM pane).
    Ok,
    /// The build failed or could not be waited on; the last line says how it exited.
    Failed(Vec<String>),
}

/// A running (or finished) `cargo xtask rom`.
pub struct RomBuild {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
    readers: Vec<JoinHandle<()>>,
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

impl RomBuild {
    /// Starts `cargo xtask rom` in `workspace`.
    pub fn start(workspace: &Path) -> io::Result<RomBuild> {
        Self::spawn(rom_command(workspace))
    }

    /// Starts `cmd` with its output captured (stdin closed).
    pub fn spawn(mut cmd: Command) -> io::Result<RomBuild> {
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
            done: None,
        })
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Non-blocking. Once the child has exited, its output is drained (the pipes close with
    /// it) and the final state is returned on every later call.
    pub fn poll(&mut self) -> BuildState {
        if let Some(done) = &self.done {
            return done.clone();
        }
        let status = match self.child.try_wait() {
            Ok(None) => return BuildState::Running(self.lines()),
            Ok(Some(status)) => Ok(status),
            Err(e) => Err(e),
        };
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
        let state = match status {
            Ok(s) if s.success() => BuildState::Ok,
            Ok(s) => {
                let mut lines = self.lines();
                lines.push(format!("build exited with {s}"));
                BuildState::Failed(lines)
            }
            Err(e) => {
                let mut lines = self.lines();
                lines.push(format!("waiting for the build failed: {e}"));
                BuildState::Failed(lines)
            }
        };
        self.done = Some(state.clone());
        state
    }
}

impl Drop for RomBuild {
    /// A build still running when its handle goes (the studio closes) is killed, not orphaned.
    fn drop(&mut self) {
        if self.done.is_none() && matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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

    fn wait(b: &mut RomBuild) -> BuildState {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match b.poll() {
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
    fn running_then_ok() {
        let mut b = RomBuild::spawn(sh("echo first; sleep 0.3")).unwrap();
        assert!(matches!(b.poll(), BuildState::Running(_)));
        assert_eq!(wait(&mut b), BuildState::Ok);
        assert_eq!(b.poll(), BuildState::Ok);
    }

    #[test]
    fn a_missing_program_is_an_error_not_a_panic() {
        assert!(RomBuild::spawn(Command::new("/nonexistent/definitely-not-cargo")).is_err());
    }
}
