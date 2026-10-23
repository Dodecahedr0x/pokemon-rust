//! `cargo xtask vanilla <spine|branches>` (Task O2): the vanilla oracle against our engine.
//!
//! - `spine`: plays our Emerald spine and BPEE's side by side, prints the `Semantic` diff at
//!   every step boundary and the frames per step, and stores the diffs outside the tolerances
//!   as `Parity` findings in the explore db.
//! - `branches [--limit N] [--outcome NAME]`: then replays the db's `rejoined` and
//!   `diverged_expected` branches (run `cargo xtask explore branch` first) on both games from
//!   their origins, and diffs the end states.
//!
//! Options: `--db PATH` (default: the explore db), `--no-db` (print only), `--quiet` (no
//! per-step deltas); for iterating on the driver, `--cache DIR` (save vanilla's state at every
//! position), `--from N` (start from the cache's position N instead of a new game) and
//! `--to N` (stop after position N). Needs `PKR_ROM_US` (BPEE).

use anyhow::{Context, Result, bail, ensure};
use content::Emerald;
use engine::checkpoint::build_hash;
use explore::branch::Action;
use explore::config::Config;
use explore::db::{Db, RunRow};
use explore::semantic::map_name;
use explore::spine;
use std::path::PathBuf;
use std::time::Instant;
use vanilla::oracle::{self, BranchPlan, OracleRun, Row};

const USAGE: &str = "usage: vanilla <spine|branches> [--db PATH] [--no-db] [--quiet] \
                     [--limit N] [--outcome NAME] [--cache DIR] [--from N] [--to N]";

/// New deltas printed per row (all are stored).
const MAX_PRINTED: usize = 30;

struct Opts {
    db: Option<PathBuf>,
    quiet: bool,
    limit: Option<usize>,
    outcome: Option<String>,
    cache: Option<PathBuf>,
    from: usize,
    to: Option<usize>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        bail!(USAGE);
    };
    let mut o = Opts {
        db: Some(Db::default_path(content::FLAVOR)),
        quiet: false,
        limit: None,
        outcome: None,
        cache: None,
        from: 0,
        to: None,
    };
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => o.db = Some(PathBuf::from(it.next().context(USAGE)?)),
            "--no-db" => o.db = None,
            "--quiet" => o.quiet = true,
            "--limit" => o.limit = Some(it.next().context(USAGE)?.parse()?),
            "--outcome" => o.outcome = Some(it.next().context(USAGE)?.clone()),
            "--cache" => o.cache = Some(PathBuf::from(it.next().context(USAGE)?)),
            "--from" => o.from = it.next().context(USAGE)?.parse()?,
            "--to" => o.to = Some(it.next().context(USAGE)?.parse()?),
            _ => bail!(USAGE),
        }
    }
    match cmd.as_str() {
        "spine" => run(&o, false),
        "branches" => run(&o, true),
        _ => bail!(USAGE),
    }
}

fn print_row(r: &Row, quiet: bool) {
    let step = r
        .step
        .map_or_else(|| "start".to_string(), |s| format!("{s:?}"));
    let step: String = step.chars().take(70).collect();
    println!(
        "{:>3} {:<70} ours {:>6} vanilla {:>6}  deltas {:>3} (new {:>2}, noise {:>2}){}",
        r.index,
        step,
        r.ours_frames,
        r.vanilla_frames,
        r.deltas.len(),
        r.new.len(),
        r.noise.len(),
        if r.tolerance.levels { "" } else { " ~lv" }.to_string()
            + if r.tolerance.hp { "" } else { " ~hp" }
    );
    if let Some(e) = &r.error {
        println!("      VANILLA FAILED: {e}");
    }
    if let Some(n) = &r.note {
        println!("      NOTE: {n}");
    }
    if r.rerolls > 0 {
        println!("      vanilla lost a battle: replayed {} times", r.rerolls);
    }
    if !quiet {
        for d in r.new.iter().take(MAX_PRINTED) {
            println!("      + {d}");
        }
        if r.new.len() > MAX_PRINTED {
            println!("      + … {} more", r.new.len() - MAX_PRINTED);
        }
    }
}

fn run(o: &Opts, branches: bool) -> Result<()> {
    let rom = vanilla::rom_from_env().context("PKR_ROM_US (BPEE) is not set")?;
    let game = Emerald::for_shells();
    let cfg = Config::default();
    let t = Instant::now();
    let ours = spine::run(&game, spine::steps(), &cfg);
    println!(
        "ours: {} of {} steps, {} frames in {:.1} s{}",
        ours.positions.len().saturating_sub(1),
        ours.steps.len(),
        ours.frames,
        t.elapsed().as_secs_f64(),
        ours.error
            .as_ref()
            .map_or(String::new(), |e| format!(" (stopped: {e})"))
    );
    let (v, notes) = vanilla::Vanilla::open(&rom)?;
    for n in notes {
        println!("sym: {n}");
    }
    let t = Instant::now();
    let mut resume = oracle::Resume {
        from: o.from,
        to: o.to,
        cache: o.cache.clone(),
        ..Default::default()
    };
    let mut live = if o.from == 0 {
        let live = oracle::start(v, &game, &ours)?;
        println!(
            "vanilla: booted, intro played and aligned in {} frames",
            live.v.frames
        );
        live
    } else {
        ensure!(!branches, "branches need a whole run (no --from)");
        let dir = o.cache.as_ref().context("--from needs --cache")?;
        let mut live = vanilla::live::Live::new(v, &game);
        resume.history = oracle::Resume::load(dir, o.from, &mut live)?;
        println!(
            "vanilla: resumed at position {} from {}",
            o.from,
            dir.display()
        );
        live
    };
    let quiet = o.quiet;
    let run = oracle::run_spine_range(&game, &ours, &mut live, branches, resume, |r| {
        print_row(r, quiet)
    })?;
    let wall = t.elapsed();
    summary(&ours, &run, wall.as_secs_f64());
    let mut db = match &o.db {
        Some(p) => {
            let (mut db, _) = Db::open(p, build_hash(&game), content::FLAVOR)?;
            let id = db.record_run(&RunRow {
                started: now_text(),
                command: if branches {
                    "vanilla-branches"
                } else {
                    "vanilla-spine"
                }
                .into(),
                config: cfg.to_ron(),
                threads: 1,
                branches: 0,
                frames: live.v.frames,
                wall_ms: wall.as_millis() as u64,
            })?;
            db.put_spine(&ours, 0, id)?;
            let findings: Vec<_> = (run.log.findings.iter())
                .map(|(i, f)| (*i, None, f.clone()))
                .collect();
            db.put_parity(&ours, &findings, id)?;
            println!(
                "{} Parity findings stored in {}",
                findings.len(),
                p.display()
            );
            Some((db, id))
        }
        None => None,
    };
    if !branches {
        return Ok(());
    }
    let Some((db, id)) = &mut db else {
        bail!("branches need the explore db (no --no-db)");
    };
    let rows = db.branches()?;
    let want = |outcome: &str| match &o.outcome {
        Some(w) => outcome == w,
        None => outcome == "rejoined" || outcome == "diverged_expected",
    };
    let mut picked: Vec<_> = rows
        .iter()
        .filter(|b| want(&b.outcome) && (b.origin as usize) < run.states.len())
        .collect();
    let skipped = rows.iter().filter(|b| want(&b.outcome)).count() - picked.len();
    if let Some(n) = o.limit {
        picked.truncate(n);
    }
    println!(
        "branches: {} to replay ({} more start past vanilla's last position)",
        picked.len(),
        skipped
    );
    let mut all = Vec::new();
    let (mut equal, mut differ, mut failed) = (0, 0, 0);
    for b in picked {
        let Some(action) = Action::from_text(&b.action) else {
            println!("  #{}: bad action {:?}", b.id, b.action);
            continue;
        };
        let plan = BranchPlan {
            id: b.id,
            origin: b.origin as usize,
            action,
            kind: &b.kind,
            outcome: &b.outcome,
            macros: &b.macros,
        };
        match oracle::branch(&game, &ours, &cfg, &run, &mut live, &plan) {
            Ok((row, findings)) => {
                if row.vanilla_error.is_some() || row.ours_error.is_some() {
                    failed += 1;
                } else if row.deltas.is_empty() {
                    equal += 1;
                } else {
                    differ += 1;
                }
                println!(
                    "  #{:<5} origin {:>3} {:<6} {:<18} {:>2} macros  ours {:>6} vanilla {:>6}  \
                     deltas {}",
                    row.id,
                    row.origin,
                    row.kind,
                    row.outcome,
                    row.macros,
                    row.ours_frames,
                    row.vanilla_frames,
                    row.deltas.len()
                );
                if let Some(e) = &row.ours_error {
                    println!("      OURS FAILED: {e}");
                }
                if let Some(e) = &row.vanilla_error {
                    println!("      VANILLA FAILED: {e}");
                }
                if !o.quiet {
                    for d in &row.deltas {
                        println!("      + {d}");
                    }
                }
                all.extend(findings.into_iter().map(|f| (row.origin, Some(row.id), f)));
            }
            Err(e) => {
                failed += 1;
                println!("  #{}: {e:#}", b.id);
            }
        }
    }
    db.put_parity(&ours, &all, *id)?;
    println!(
        "branches: {equal} equal, {differ} with deltas, {failed} failed; {} Parity findings \
         stored",
        all.len()
    );
    Ok(())
}

fn summary(ours: &spine::SpineRun, run: &OracleRun, wall: f64) {
    let failed = run.rows.iter().filter(|r| r.error.is_some()).count();
    let (of, vf): (u64, u64) = (run.rows.iter())
        .map(|r| (r.ours_frames, r.vanilla_frames))
        .fold((0, 0), |(a, b), (x, y)| (a + x, b + y));
    let last = run.rows.last();
    println!(
        "vanilla spine: {} of {} positions compared in {wall:.1} s; {failed} steps vanilla \
         failed; frames ours {of} / vanilla {vf} (+{} to the start); {} Parity findings{}",
        run.rows.len(),
        ours.positions.len(),
        run.start_frames,
        run.log.findings.len(),
        run.stopped
            .as_ref()
            .map_or(String::new(), |s| format!("; stopped: {s}"))
    );
    if let Some(r) = last {
        let p = &ours.positions[r.index];
        println!(
            "  last: position {} ({}), ours on {} {:?}; {} deltas there",
            r.index,
            p.segment.unwrap_or("the new game"),
            map_name(p.map()),
            p.pos(),
            r.deltas.len()
        );
    }
    let failures: Vec<_> = (run.rows.iter())
        .filter_map(|r| r.error.as_ref().map(|e| (r.index, e)))
        .collect();
    for (i, e) in failures {
        println!("  failed at {i}: {e}");
    }
    let noise: usize = run.rows.iter().map(|r| r.noise.len()).sum();
    println!(
        "  Parity findings (deltas lasting past the next moving step; {noise} row-deltas of \
         RNG/timing noise left out):"
    );
    for (i, f) in &run.log.findings {
        println!("    {i:>3} {}", f.message);
    }
    // Frames per checkpoint segment.
    let mut seg: Vec<(&str, u64, u64)> = Vec::new();
    for r in &run.rows {
        let name = r.segment.unwrap_or("start");
        match seg.last_mut() {
            Some((n, a, b)) if *n == name => {
                *a += r.ours_frames;
                *b += r.vanilla_frames;
            }
            _ => seg.push((name, r.ours_frames, r.vanilla_frames)),
        }
    }
    for (n, a, b) in seg {
        println!(
            "  segment {n:<16} frames ours {a:>7} vanilla {b:>7} ({:+.0}%)",
            (a as f64 / (b.max(1)) as f64 - 1.0) * 100.0
        );
    }
}

fn now_text() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("unix {secs}")
}
