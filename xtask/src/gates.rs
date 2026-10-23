//! `cargo xtask gates [--jobs N] [--only a,b] [--skip a,b] [--list]` (explorer and gate speed
//! work): the merge gates, run concurrently, with a pass or fail summary.
//!
//! Each gate runs in a **lane**, and the lanes run concurrently (`--jobs` lanes at a time,
//! default 3). A lane is a target dir, so that no two concurrent gates rebuild each other's
//! artifacts (the content build differs by flavour: the stub and Silice have their own dirs,
//! clippy has its own since it builds other features):
//!
//! | lane | target dir | gates, in order |
//! |---|---|---|
//! | `main` | `target/` | `test` (the workspace's tests), `parity` (which verifies the committed checkpoint tree first) |
//! | `lint` | `target/clippy` | `fmt`, `clippy`, `clippy-ui` |
//! | `stub` | `target/stub` | `test-stub` (`PKR_EXTRACTED_DIR=/nonexistent`) |
//! | `silice` | `target/silice` | `test-silice` (`PKR_GAME=silice`, `-p content -p explore`) |
//!
//! The `test*` gates build every test binary once (`cargo test --no-run`), then run the
//! binaries concurrently (at most `--test-jobs`, default a quarter of the available
//! parallelism; each binary still runs its own tests on threads, as `cargo test` does), then
//! the doc tests. The binaries are the ones `cargo test` would run, run from their package's
//! directory with `CARGO_MANIFEST_DIR` set as `cargo test` does; only their order and overlap
//! differ (`cargo test` runs one binary at a time). A failing gate does not stop the others.
//! Logs go to `target/gates/<gate>.log`. Exit 1 when a gate fails.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const USAGE: &str =
    "usage: cargo xtask gates [--jobs N] [--test-jobs N] [--only a,b] [--skip a,b] [--list]";

/// How a gate runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A command line.
    Cmd(&'static [&'static str]),
    /// `cargo test` with these arguments, binaries run concurrently ([`run_tests`]).
    Tests(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
struct Gate {
    name: &'static str,
    lane: &'static str,
    /// Environment set for it (`CARGO_TARGET_DIR` comes from the lane).
    env: &'static [(&'static str, &'static str)],
    kind: Kind,
}

const GATES: &[Gate] = &[
    Gate {
        name: "test",
        lane: "main",
        env: &[],
        kind: Kind::Tests(&["--workspace"]),
    },
    Gate {
        name: "parity",
        lane: "main",
        env: &[],
        kind: Kind::Cmd(&["cargo", "xtask", "parity"]),
    },
    Gate {
        name: "fmt",
        lane: "lint",
        env: &[],
        kind: Kind::Cmd(&["cargo", "fmt", "--all", "--", "--check"]),
    },
    Gate {
        name: "clippy",
        lane: "lint",
        env: &[],
        kind: Kind::Cmd(&[
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
    },
    Gate {
        name: "clippy-ui",
        lane: "lint",
        env: &[],
        kind: Kind::Cmd(&[
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--features",
            "game-native/window,studio/ui",
            "--",
            "-D",
            "warnings",
        ]),
    },
    Gate {
        name: "test-stub",
        lane: "stub",
        env: &[("PKR_EXTRACTED_DIR", "/nonexistent")],
        kind: Kind::Tests(&["--workspace"]),
    },
    Gate {
        name: "test-silice",
        lane: "silice",
        env: &[("PKR_GAME", "silice")],
        kind: Kind::Tests(&["-p", "content", "-p", "explore"]),
    },
];

/// A lane's target dir (`None`: the default, `target/`).
fn lane_target(lane: &str) -> Option<&'static str> {
    match lane {
        "lint" => Some("target/clippy"),
        "stub" => Some("target/stub"),
        "silice" => Some("target/silice"),
        _ => None,
    }
}

struct Opts {
    jobs: usize,
    test_jobs: usize,
    gates: Vec<Gate>,
}

fn parse(args: &[String]) -> Result<Option<Opts>> {
    let quarter = std::thread::available_parallelism()
        .map_or(1, |n| n.get() / 4)
        .max(1);
    let mut o = Opts {
        jobs: 3,
        test_jobs: quarter,
        gates: GATES.to_vec(),
    };
    let names = |v: &str| -> Result<Vec<&'static str>> {
        v.split(',')
            .map(|n| {
                GATES
                    .iter()
                    .find(|g| g.name == n)
                    .map(|g| g.name)
                    .with_context(|| format!("no gate {n:?}"))
            })
            .collect()
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .with_context(|| format!("{a} needs a value\n{USAGE}"))
        };
        match a.as_str() {
            "--list" => {
                for g in GATES {
                    println!("{:<12} lane {:<7} {:?}", g.name, g.lane, g.kind);
                }
                return Ok(None);
            }
            "--jobs" => o.jobs = val()?.parse().context("--jobs")?,
            "--test-jobs" => o.test_jobs = val()?.parse().context("--test-jobs")?,
            "--only" => {
                let keep = names(val()?)?;
                o.gates.retain(|g| keep.contains(&g.name));
            }
            "--skip" => {
                let drop = names(val()?)?;
                o.gates.retain(|g| !drop.contains(&g.name));
            }
            _ => bail!("unexpected {a:?}\n{USAGE}"),
        }
    }
    o.jobs = o.jobs.max(1);
    o.test_jobs = o.test_jobs.max(1);
    Ok(Some(o))
}

/// A gate's outcome.
struct Outcome {
    name: &'static str,
    ok: bool,
    time: Duration,
    note: String,
}

pub fn main(args: &[String]) -> Result<()> {
    let Some(o) = parse(args)? else {
        return Ok(());
    };
    let root = crate::root();
    let logs = root.join("target/gates");
    std::fs::create_dir_all(&logs)?;
    // Lanes in first-gate order; gates in table order within a lane.
    let mut lanes: Vec<(&str, Vec<Gate>)> = Vec::new();
    for g in &o.gates {
        match lanes.iter_mut().find(|(l, _)| *l == g.lane) {
            Some((_, gs)) => gs.push(*g),
            None => lanes.push((g.lane, vec![*g])),
        }
    }
    println!(
        "gates: {} in {} lanes, {} lanes at a time, {} test binaries at a time per gate; logs in {}",
        o.gates.len(),
        lanes.len(),
        o.jobs,
        o.test_jobs,
        logs.display()
    );
    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..o.jobs.min(lanes.len()) {
            sc.spawn(|| {
                while let Some((lane, gates)) = lanes.get(next.fetch_add(1, Ordering::Relaxed)) {
                    for g in gates {
                        let t = Instant::now();
                        let log = logs.join(format!("{}.log", g.name));
                        let r = run_gate(&root, lane, g, &log, o.test_jobs);
                        let (ok, note) = match r {
                            Ok(note) => (true, note),
                            Err(e) => (false, format!("{e:#}")),
                        };
                        println!(
                            "  {} {:<12} {:>6.0} s  {}",
                            if ok { "pass" } else { "FAIL" },
                            g.name,
                            t.elapsed().as_secs_f64(),
                            note
                        );
                        outcomes.lock().unwrap().push(Outcome {
                            name: g.name,
                            ok,
                            time: t.elapsed(),
                            note,
                        });
                    }
                }
            });
        }
    });
    let outcomes = outcomes.into_inner().unwrap();
    println!("\ngates done in {:.0} s:", start.elapsed().as_secs_f64());
    for g in &o.gates {
        let r = outcomes.iter().find(|r| r.name == g.name).expect("ran");
        println!(
            "  {} {:<12} {:>6.0} s  {}",
            if r.ok { "pass" } else { "FAIL" },
            r.name,
            r.time.as_secs_f64(),
            r.note
        );
    }
    let failed: Vec<&str> = outcomes.iter().filter(|r| !r.ok).map(|r| r.name).collect();
    if !failed.is_empty() {
        bail!(
            "failed gates: {} (logs in {})",
            failed.join(", "),
            logs.display()
        );
    }
    Ok(())
}

/// A command for gate `g` in `lane`: its environment and target dir.
fn command(root: &Path, lane: &str, g: &Gate, program: &str) -> Command {
    let mut c = Command::new(program);
    c.current_dir(root);
    // A lane's own target dir; the default lane's is `target/`, whatever the caller's.
    match lane_target(lane) {
        Some(t) => c.env("CARGO_TARGET_DIR", root.join(t)),
        None => c.env_remove("CARGO_TARGET_DIR"),
    };
    // Not this xtask's flavour selection: each gate picks its own.
    c.env_remove("PKR_GAME").env_remove("PKR_XTASK_GAME");
    for (k, v) in g.env {
        c.env(k, v);
    }
    c
}

/// Runs `cmd` with stdout and stderr appended to `log`.
fn logged(mut cmd: Command, log: &Path) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    writeln!(f, "$ {cmd:?}")?;
    let out = f.try_clone()?;
    let status = cmd
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(f))
        .status()
        .with_context(|| format!("spawning {cmd:?}"))?;
    if !status.success() {
        bail!("{status}: see {}", log.display());
    }
    Ok(())
}

fn run_gate(root: &Path, lane: &str, g: &Gate, log: &Path, test_jobs: usize) -> Result<String> {
    std::fs::write(log, "")?;
    match g.kind {
        Kind::Cmd(argv) => {
            let mut c = command(root, lane, g, argv[0]);
            c.args(&argv[1..]);
            logged(c, log)?;
            Ok(String::new())
        }
        Kind::Tests(args) => run_tests(root, lane, g, args, log, test_jobs),
    }
}

/// A test binary to run: its path, its package's directory and a name for the log.
struct TestBin {
    exe: PathBuf,
    dir: PathBuf,
    name: String,
}

/// `cargo test <args>`, its binaries run `jobs` at a time (module docs). Every binary runs,
/// even after a failure; then the doc tests.
fn run_tests(
    root: &Path,
    lane: &str,
    g: &Gate,
    args: &[&str],
    log: &Path,
    jobs: usize,
) -> Result<String> {
    // Build every test binary and list them (one line of JSON per artifact).
    let mut c = command(root, lane, g, "cargo");
    c.args([
        "test",
        "--no-run",
        "--message-format=json-render-diagnostics",
    ])
    .args(args);
    let out = c
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("spawning {c:?}"))?;
    std::fs::write(log, &out.stderr)?;
    if !out.status.success() {
        bail!("the build failed: see {}", log.display());
    }
    let mut bins = test_bins(&String::from_utf8_lossy(&out.stdout))?;
    // Longest first (the last run's times; new binaries first of all), so the long ones
    // overlap instead of ending the gate alone.
    let times_file = log.with_extension("times");
    let last: BTreeMap<String, f64> = std::fs::read_to_string(&times_file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.rsplit_once(' '))
        .filter_map(|(n, t)| Some((n.to_string(), t.parse().ok()?)))
        .collect();
    bins.sort_by(|a, b| {
        let t = |b: &TestBin| last.get(&b.name).copied().unwrap_or(f64::MAX);
        t(b).total_cmp(&t(a)).then_with(|| a.name.cmp(&b.name))
    });
    let dir = log.with_extension("d");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    let times = Mutex::new(BTreeMap::new());
    std::thread::scope(|sc| {
        // The doc tests (`cargo test --doc` builds and runs them), alongside the binaries.
        sc.spawn(|| {
            let mut c = command(root, lane, g, "cargo");
            c.args(["test", "--doc"]).args(args);
            if logged(c, &dir.join("doc.log")).is_err() {
                failed.lock().unwrap().push("doc".into());
            }
        });
        for _ in 0..jobs.min(bins.len()).max(1) {
            sc.spawn(|| {
                while let Some(b) = bins.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let t = Instant::now();
                    let mut c = Command::new(&b.exe);
                    c.current_dir(&b.dir).env("CARGO_MANIFEST_DIR", &b.dir);
                    for (k, v) in g.env {
                        c.env(k, v);
                    }
                    let r = logged(c, &dir.join(format!("{}.log", b.name)));
                    times.lock().unwrap().insert(b.name.clone(), t.elapsed());
                    if r.is_err() {
                        failed.lock().unwrap().push(b.name.clone());
                    }
                }
            });
        }
    });
    let failed = failed.into_inner().unwrap();
    let times = times.into_inner().unwrap();
    let text: String = times
        .iter()
        .map(|(n, t)| format!("{n} {:.3}\n", t.as_secs_f64()))
        .collect();
    std::fs::write(&times_file, text)?;
    let mut slow: Vec<(&String, &Duration)> = times.iter().collect();
    slow.sort_by(|a, b| b.1.cmp(a.1));
    let slowest = slow
        .iter()
        .take(3)
        .map(|(n, t)| format!("{n} {:.0} s", t.as_secs_f64()))
        .collect::<Vec<_>>()
        .join(", ");
    if !failed.is_empty() {
        bail!("failed: {} (logs in {})", failed.join(", "), dir.display());
    }
    Ok(format!("{} binaries; slowest {slowest}", bins.len()))
}

/// The test executables of `cargo test --no-run --message-format=json` output.
fn test_bins(json: &str) -> Result<Vec<TestBin>> {
    let mut bins = Vec::new();
    for line in json.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact" || v["profile"]["test"] != true {
            continue;
        }
        let Some(exe) = v["executable"].as_str() else {
            continue;
        };
        let manifest = v["manifest_path"].as_str().context("manifest_path")?;
        let dir = Path::new(manifest).parent().context("manifest dir")?;
        let pkg = dir
            .file_name()
            .map_or("?".into(), |n| n.to_string_lossy().into_owned());
        let target = v["target"]["name"].as_str().unwrap_or("?");
        let kind = v["target"]["kind"][0].as_str().unwrap_or("?");
        bins.push(TestBin {
            exe: PathBuf::from(exe),
            dir: dir.to_path_buf(),
            name: format!("{pkg}-{kind}-{target}"),
        });
    }
    if bins.is_empty() {
        bail!("cargo listed no test binaries");
    }
    Ok(bins)
}
