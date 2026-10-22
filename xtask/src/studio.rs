//! `cargo xtask studio [--smoke [secs] | --check]`: runs the studio (release).
//! - `--smoke` closes it after `secs` (default 10) through `STUDIO_SMOKE_SECS` and fails if a
//!   gated pane delivered under [`MIN_FPS`], the studio exited non-zero, or it was still
//!   running `secs` + [`SMOKE_GRACE_SECS`] after start (then it is killed). The gated panes
//!   (`SMOKE` lines) are two transport test patterns the studio adds in a smoke run, next to
//!   the real native and ROM panes (running a new game); those publish only changed pictures,
//!   so their rate is printed as `INFO` lines and not gated. Needs a logged-in GUI session; the
//!   window stays on top and takes focus.
//! - `--check` runs the display-free `ui` gates the workspace gates skip: `cargo test -p studio
//!   --features ui` and its clippy.

use crate::{root, run};
use anyhow::{Context, Result, bail};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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

fn smoke(secs: u32) -> Result<()> {
    // Build first, so the timeout covers the run only.
    run(cargo(&["build", "--release"]).args(UI))?;
    let mut cmd = Command::new(release_bin());
    cmd.current_dir(root())
        .env("STUDIO_SMOKE_SECS", secs.to_string())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| format!("spawning {cmd:?}"))?;
    let mut pipe = child.stdout.take().context("the studio's stdout")?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = pipe.read_to_end(&mut out);
        out
    });
    let deadline = Instant::now() + Duration::from_secs(u64::from(secs) + SMOKE_GRACE_SECS);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "the studio was still running {} s after start: killed",
                u64::from(secs) + SMOKE_GRACE_SECS
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let out = reader.join().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out);
    print!("{stdout}");
    if !status.success() {
        bail!("the studio exited with {status}");
    }
    let panes = parse_smoke(&stdout);
    if panes.is_empty() {
        bail!("the studio printed no SMOKE lines");
    }
    let slow: Vec<String> = panes
        .iter()
        .filter(|(_, fps)| *fps < MIN_FPS)
        .map(|(p, fps)| format!("{p} ({fps} fps)"))
        .collect();
    if !slow.is_empty() {
        bail!("panes under {MIN_FPS} fps: {}", slow.join(", "));
    }
    println!(
        "studio smoke: {} panes at or above {MIN_FPS} fps",
        panes.len()
    );
    Ok(())
}

/// `(pane, fps)` from the `SMOKE pane=<name> fps=<f> …` lines; an unparsable or non-finite
/// fps counts as 0.
fn parse_smoke(stdout: &str) -> Vec<(String, f64)> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("SMOKE "))
        .filter_map(|l| {
            let field = |k: &str| {
                l.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(k)?.strip_prefix('='))
            };
            let pane = field("pane")?.to_string();
            let fps = field("fps")
                .and_then(|f| f.parse::<f64>().ok())
                .filter(|f| f.is_finite())
                .unwrap_or(0.0);
            Some((pane, fps))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_lines_parse() {
        let out = "noise\nSMOKE pane=a fps=60.0 latency_ms=1.02 max_ms=26.36\n\
                   INFO pane=native fps=0.0 latency_ms=nan (no reports)\n\
                   SMOKE pane=b fps=nan latency_ms=nan (no reports)\nSMOKE fps=3\n";
        assert_eq!(
            parse_smoke(out),
            [("a".to_string(), 60.0), ("b".to_string(), 0.0)]
        );
    }
}
