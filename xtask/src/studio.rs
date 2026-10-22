//! `cargo xtask studio [--smoke [secs]]`: runs the studio (release). `--smoke` closes it after
//! `secs` (default 10) through `STUDIO_SMOKE_SECS` and fails if a pane delivered under
//! [`MIN_FPS`] or the studio exited non-zero.

use crate::{root, run};
use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};

/// The smoke gate (the spike acceptance is 58 fps in release on macOS).
const MIN_FPS: f64 = 50.0;
const DEFAULT_SMOKE_SECS: u32 = 10;

fn studio() -> Command {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root())
        .args(["run", "-p", "studio", "--features", "ui", "--release"]);
    cmd
}

pub fn main(args: &[String]) -> Result<()> {
    match args {
        [] => run(&mut studio()),
        [flag, rest @ ..] if flag == "--smoke" => {
            let secs = match rest {
                [] => DEFAULT_SMOKE_SECS,
                [s] => s
                    .parse()
                    .with_context(|| format!("--smoke {s}: not a number"))?,
                _ => bail!("usage: cargo xtask studio [--smoke [secs]]"),
            };
            smoke(secs)
        }
        _ => bail!("usage: cargo xtask studio [--smoke [secs]]"),
    }
}

fn smoke(secs: u32) -> Result<()> {
    let mut cmd = studio();
    cmd.env("STUDIO_SMOKE_SECS", secs.to_string())
        .stdout(Stdio::piped());
    let out = cmd.output().with_context(|| format!("spawning {cmd:?}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    print!("{stdout}");
    if !out.status.success() {
        bail!("the studio exited with {}", out.status);
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
                   SMOKE pane=b fps=nan latency_ms=nan (no reports)\nSMOKE fps=3\n";
        assert_eq!(
            parse_smoke(out),
            [("a".to_string(), 60.0), ("b".to_string(), 0.0)]
        );
    }
}
