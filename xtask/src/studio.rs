//! `cargo xtask studio [--smoke [secs] | --check]`: runs the studio (release).
//! - `--smoke` closes it after `secs` (default 10) through `STUDIO_SMOKE_SECS` and fails if a
//!   gated pane delivered under [`MIN_FPS`], the studio exited non-zero, or it was still
//!   running `secs` + [`SMOKE_GRACE_SECS`] after start (then it is killed). The gated panes
//!   (`SMOKE` lines) are two transport test patterns the studio adds in a smoke run, next to
//!   the real native and ROM panes (running a new game); those publish only changed pictures,
//!   so their rate is printed as `INFO` lines and gated loosely: their thread must still run
//!   (`alive=true`) and they must have drawn at least one frame (`total`). A run killed at the
//!   timeout prints what it printed so far and a `sample` of its threads (macOS; also saved to
//!   `target/studio/smoke-sample.txt`). Needs a logged-in GUI session; the window stays on top
//!   and takes focus.
//!
//!   The delivered rate is load-sensitive (one run under a load average of about 20 gave
//!   35 fps), so a run that fails only the fps gate is retried once. The studio may log one
//!   `ERROR dioxus_desktop::edits` line as its window closes: an edit batch the closing webview
//!   never acknowledged; it is harmless.
//! - `--check` runs the display-free `ui` gates the workspace gates skip: `cargo test -p studio
//!   --features ui` and its clippy.

use crate::{root, run};
use anyhow::{Context, Result, bail};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The smoke gate (the spike acceptance is 58 fps in release on macOS).
const MIN_FPS: f64 = 50.0;
const DEFAULT_SMOKE_SECS: u32 = 10;
/// How long past `secs` a smoke run may take (window start-up and shutdown) before it is killed.
const SMOKE_GRACE_SECS: u64 = 60;
const USAGE: &str = "usage: cargo xtask studio [--smoke [secs] | --check]";

fn cargo(args: &[&str]) -> Command {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root()).args(args);
    cmd
}

const UI: [&str; 4] = ["-p", "studio", "--features", "ui"];

pub fn main(args: &[String]) -> Result<()> {
    match args {
        [] => run(cargo(&["run", "--release"]).args(UI)),
        [flag] if flag == "--check" => {
            run(cargo(&["test"]).args(UI))?;
            run(cargo(&["clippy", "--all-targets"])
                .args(UI)
                .args(["--", "-D", "warnings"]))
        }
        [flag, rest @ ..] if flag == "--smoke" => {
            let secs = match rest {
                [] => DEFAULT_SMOKE_SECS,
                [s] => s
                    .parse()
                    .with_context(|| format!("--smoke {s}: not a number"))?,
                _ => bail!(USAGE),
            };
            smoke(secs)
        }
        _ => bail!(USAGE),
    }
}

/// The release binary `cargo build` wrote (honours `CARGO_TARGET_DIR`).
fn release_bin() -> PathBuf {
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root().join("target"), PathBuf::from);
    target
        .join("release")
        .join(format!("studio{}", std::env::consts::EXE_SUFFIX))
}

/// Where a hung smoke run's `sample` profile is saved.
fn sample_path() -> PathBuf {
    root().join("target/studio/smoke-sample.txt")
}

/// macOS: a 3 s `sample` of the studio's threads (where it hangs), saved to
/// [`sample_path`] and printed. Elsewhere, or without `sample`, a note.
fn sample(pid: u32) {
    let path = sample_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let out = Command::new("sample")
        .args([&pid.to_string(), "3", "-file"])
        .arg(&path)
        .output();
    match out {
        Ok(o) if o.status.success() => match std::fs::read_to_string(&path) {
            Ok(s) => println!("--- sample {pid} ({}):\n{s}", path.display()),
            Err(e) => println!("--- sample {pid}: reading {}: {e}", path.display()),
        },
        Ok(o) => println!(
            "--- sample {pid} failed: {}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => println!("--- sample {pid}: cannot run `sample`: {e}"),
    }
}

fn smoke(secs: u32) -> Result<()> {
    // Build first, so the timeout covers the run only.
    run(cargo(&["build", "--release"]).args(UI))?;
    let slow = smoke_run(secs)?;
    if slow.is_empty() {
        return Ok(());
    }
    // The rate depends on the machine's load: one more try before failing.
    println!(
        "panes under {MIN_FPS} fps: {}; retrying once",
        slow.join(", ")
    );
    let slow = smoke_run(secs)?;
    if !slow.is_empty() {
        bail!("panes under {MIN_FPS} fps: {}", slow.join(", "));
    }
    Ok(())
}

/// One smoke run: `Ok` with the gated panes under [`MIN_FPS`] (none: it passed), `Err` on any
/// other failure.
fn smoke_run(secs: u32) -> Result<Vec<String>> {
    let mut cmd = Command::new(release_bin());
    cmd.current_dir(root())
        .env("STUDIO_SMOKE_SECS", secs.to_string())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| format!("spawning {cmd:?}"))?;
    let mut pipe = child.stdout.take().context("the studio's stdout")?;
    // Collected as it arrives, so a hung run still shows what it printed.
    let captured = Arc::new(Mutex::new(Vec::new()));
    let reader = std::thread::spawn({
        let captured = Arc::clone(&captured);
        move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                captured
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend_from_slice(&buf[..n]);
            }
        }
    });
    let output = || {
        String::from_utf8_lossy(&captured.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
    };
    let deadline = Instant::now() + Duration::from_secs(u64::from(secs) + SMOKE_GRACE_SECS);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            print!("{}", output());
            sample(child.id());
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "the studio was still running {} s after start: killed (output above)",
                u64::from(secs) + SMOKE_GRACE_SECS
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = reader.join();
    let stdout = output();
    print!("{stdout}");
    if !status.success() {
        bail!("the studio exited with {status}");
    }
    let panes = parse_smoke(&stdout);
    if !panes.iter().any(|p| p.gated) {
        bail!("the studio printed no SMOKE lines");
    }
    let slow: Vec<String> = panes
        .iter()
        .filter(|p| p.gated && p.fps < MIN_FPS)
        .map(|p| format!("{} ({} fps)", p.name, p.fps))
        .collect();
    // The play panes are gated loosely: their thread runs and they drew at least once.
    let dead: Vec<String> = panes
        .iter()
        .filter(|p| !p.gated && (p.alive == Some(false) || p.total == 0.0))
        .map(|p| format!("{} (alive {:?}, {} frames)", p.name, p.alive, p.total))
        .collect();
    if !dead.is_empty() {
        bail!("play panes not running: {}", dead.join(", "));
    }
    if !slow.is_empty() {
        return Ok(slow);
    }
    println!(
        "studio smoke: {} panes at or above {MIN_FPS} fps, {} play panes running",
        panes.iter().filter(|p| p.gated).count(),
        panes.iter().filter(|p| !p.gated).count()
    );
    Ok(Vec::new())
}

/// One pane line of a smoke run.
#[derive(Clone, Debug, PartialEq)]
struct PaneLine {
    name: String,
    /// A `SMOKE` line (fps gated), else `INFO`.
    gated: bool,
    fps: f64,
    /// Frames drawn since the start.
    total: f64,
    alive: Option<bool>,
}

/// The `SMOKE pane=<name> fps=<f> …` and `INFO pane=<name> …` lines; an unparsable or
/// non-finite number counts as 0.
fn parse_smoke(stdout: &str) -> Vec<PaneLine> {
    stdout
        .lines()
        .filter_map(|l| {
            let (gated, rest) = match l.split_once(' ')? {
                ("SMOKE", rest) => (true, rest),
                ("INFO", rest) => (false, rest),
                _ => return None,
            };
            let field = |k: &str| {
                rest.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(k)?.strip_prefix('='))
            };
            let num = |k: &str| {
                field(k)
                    .and_then(|f| f.parse::<f64>().ok())
                    .filter(|f| f.is_finite())
                    .unwrap_or(0.0)
            };
            Some(PaneLine {
                name: field("pane")?.to_string(),
                gated,
                fps: num("fps"),
                total: num("total"),
                alive: field("alive").and_then(|a| a.parse().ok()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_lines_parse() {
        let out = "noise\nSMOKE pane=a fps=60.0 latency_ms=1.02 max_ms=26.36 total=600\n\
                   INFO pane=native fps=0.0 latency_ms=nan total=1 alive=true (no reports)\n\
                   INFO vdom_polls=120\n\
                   INFO pane=rom fps=3 total=0 alive=false\n\
                   SMOKE pane=b fps=nan latency_ms=nan (no reports)\nSMOKE fps=3\n";
        let p = parse_smoke(out);
        let line = |name: &str, gated, fps, total, alive| PaneLine {
            name: name.into(),
            gated,
            fps,
            total,
            alive,
        };
        assert_eq!(
            p,
            [
                line("a", true, 60.0, 600.0, None),
                line("native", false, 0.0, 1.0, Some(true)),
                line("rom", false, 3.0, 0.0, Some(false)),
                line("b", true, 0.0, 0.0, None),
            ]
        );
    }
}
