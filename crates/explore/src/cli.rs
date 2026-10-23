//! The `explore` command line (`docs/plans/2026-10-23-explorer-spine.md` §5): `spine`,
//! `branch`, `branches`, `status`, `findings`, `show`, `export`, `repro`, `coverage`,
//! `digest`, `reset`.
//!
//! Every command takes `--db <path>` (default `target/explore/{stub|emerald|silice}/explore.db`,
//! after the content flavour) and `--config <path>` (a RON `Config`; default: the built-in
//! defaults).

use crate::branch::{self, Action, Outcome, Target, TargetKind};
use crate::config::Config;
use crate::db::{BranchRow, Db, FindingRow, RunRow};
use crate::goals::Act;
use crate::model::{FindingKind, SeenKind};
use crate::path::Grid;
use crate::softlock::Trap;
use crate::spine::{self, SpineRun};
use crate::{findings, macros, path, sim, store};
use anyhow::{Context, anyhow, bail};
use content::Emerald;
use engine::checkpoint::build_hash;
use engine::data::{GameData, MapId, block_collision};
use game_native::store::Store;
use std::path::{Path, PathBuf};
use std::time::Instant;

const USAGE: &str = "\
usage: explore <command> [--db <path>] [--config <path>]
  spine [--export] [--label L] [--trace]
                              plays the flavour's spine from a new game: positions, frames,
                              coverage; compares its checkpoints with the committed <act>:<label>
                              nodes; --export writes them to the local checkpoint store as
                              spine:<act>:<label> (promote with `cargo xtask checkpoints promote`);
                              --trace: every step's end (map, cell, frame, vars and flags changed)
  branch [--from L] [--budget F] [--jobs N] [--every N] [--min-novelty N] [--max-steps N]
         [--limit N]
                              plays the spine, then branches from its positions (--from: the
                              segment of checkpoint L) toward what it never touched; each branch
                              explores until its budget (frames) or its steps run out, then
                              returns to the spine and plays the next steps; it is aborted
                              instead when its novelty rate (points per 1000 frames) falls under
                              --min-novelty past its target; --limit: at most N branches
  branches [--outcome O]      the db's branches (O: rejoined, diverged_expected, diverged,
                              stranded, aborted, bug, unplayable)
  status                      db totals: the spine, the branches, novelty, findings, build
  findings [--all] [--bugs]   open findings (--all: stale ones too; --bugs: without the
                              report-only ones, `Unsupported`)
  show <id>                   one finding in full
  export <id> [--before N] [--out <path>]
                              the finding as a trace (RON) from a new game, N frames before its end
  repro <id>                  replays the finding (a divergence: plays its branch again); exit 0
                              iff it reproduces
  coverage <map>              ASCII grid of the cells stood on (map id or name): `s` by the
                              spine, `1`..`4` by branches only
  digest                      content digest of the db (determinism checks)
  reset --yes                 deletes the db";

/// Runs the `explore` CLI on its arguments (without the program name).
pub fn main(args: Vec<String>) -> anyhow::Result<()> {
    let mut a = Args::parse(args)?;
    let Some(cmd) = a.positional.first().cloned() else {
        println!("{USAGE}");
        return Ok(());
    };
    let db_path = a
        .take("--db")
        .map(PathBuf::from)
        .unwrap_or_else(|| Db::default_path(content::FLAVOR));
    let mut cfg = match a.take("--config") {
        Some(p) if !Path::new(&p).exists() => bail!("no config file {p}"),
        Some(p) => Config::load_or_default(Path::new(&p))?,
        None => Config::default(),
    };
    // The spine's `Expect`s are the goals of the build's act: a config naming another act
    // (another flavour's, or the Emerald slice) would fail them.
    if cfg.act != Act::default() {
        let why = match Act::parse_in_build(cfg.act.name()) {
            Err(e) => e,
            Ok(_) => format!("the spine plays {}", Act::default().name()),
        };
        bail!("config act {:?}: {why}", cfg.act.name());
    }
    match cmd.as_str() {
        "spine" => {
            let export = a.flag("--export");
            let trace = a.flag("--trace");
            let label = a.take("--label");
            a.done()?;
            spine_cmd(&db_path, &cfg, export, trace, label.as_deref())
        }
        "branch" => {
            if let Some(n) = a.take_num("--jobs")? {
                cfg.jobs = n as usize;
            }
            if let Some(n) = a.take_num("--budget")? {
                cfg.branch.budget_frames = n as u32;
            }
            if let Some(n) = a.take_num("--every")? {
                cfg.branch.every = n as u32;
            }
            if let Some(n) = a.take_num("--min-novelty")? {
                cfg.branch.min_novelty = n as u32;
            }
            if let Some(n) = a.take_num("--max-steps")? {
                cfg.branch.max_steps = n as u32;
            }
            let limit = a.take_num("--limit")?.map(|n| n as usize);
            let from = a.take("--from");
            a.done()?;
            branch_cmd(&db_path, &cfg, from.as_deref(), limit)
        }
        "branches" => {
            let outcome = a.take("--outcome");
            a.done()?;
            if let Some(o) = &outcome
                && !Outcome::NAMES.contains(&o.as_str())
            {
                bail!("unknown outcome {o:?} (one of {:?})", Outcome::NAMES);
            }
            list_branches(&db_path, outcome.as_deref())
        }
        "status" => {
            a.done()?;
            status(&db_path)
        }
        "findings" => {
            let all = a.flag("--all");
            let bugs = a.flag("--bugs");
            a.done()?;
            list_findings(&db_path, all, bugs)
        }
        "show" => {
            let id = a.id()?;
            a.done()?;
            show(&db_path, id)
        }
        "export" => {
            let id = a.id()?;
            let before = a.take_num("--before")?.unwrap_or(0);
            let out = a.take("--out").map(PathBuf::from);
            a.done()?;
            export(&db_path, id, before, out)
        }
        "repro" => {
            let id = a.id()?;
            a.done()?;
            let db = Db::open_read_only(&db_path)?;
            match reproduce(&Emerald::for_shells(), &db, id, &cfg)? {
                Ok(()) => {
                    println!("finding {id} reproduces");
                    Ok(())
                }
                Err(why) => Err(NotReproduced(format!(
                    "finding {id} does not reproduce: {why}"
                )))?,
            }
        }
        "coverage" => {
            let map = a
                .positional
                .get(1)
                .cloned()
                .ok_or_else(|| anyhow!("coverage <map>"))?;
            a.positional.truncate(1);
            a.done()?;
            coverage(&db_path, &map)
        }
        "digest" => {
            a.done()?;
            let db = Db::open_read_only(&db_path)?;
            println!("{:#018x}", db.content_digest()?);
            Ok(())
        }
        "reset" => {
            let yes = a.flag("--yes");
            a.done()?;
            if !yes {
                bail!("reset deletes {}: confirm with --yes", db_path.display());
            }
            if !db_path.exists() {
                println!("no db at {}", db_path.display());
                return Ok(());
            }
            lock_for_reset(&db_path)?;
            for suffix in ["", "-wal", "-shm"] {
                let mut p = db_path.clone().into_os_string();
                p.push(suffix);
                match std::fs::remove_file(&p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e).context(format!("removing {p:?}")),
                }
            }
            println!("deleted {}", db_path.display());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => bail!("unknown command {other:?}\n{USAGE}"),
    }
}

/// `repro`'s verdict when the finding does not reproduce (the bin exits 1; other errors exit 2).
#[derive(Debug)]
pub struct NotReproduced(pub String);

impl std::fmt::Display for NotReproduced {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotReproduced {}

/// The bin's exit code for `main`'s result: 0 on success; for `repro`, 1 when the finding
/// does not reproduce and 2 on any other error; 1 for errors of the other commands.
pub fn exit_code(args: &[String], r: &anyhow::Result<()>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) if e.is::<NotReproduced>() => 1,
        Err(_) if args.first().is_some_and(|c| c == "repro") => 2,
        Err(_) => 1,
    }
}

/// Proves `path` is an explore db no one is writing: it opens read-only (schema checked), then
/// takes the write lock (`BEGIN IMMEDIATE`, no busy wait) and releases it. A db of an older
/// schema is deleted too (the reset hint of `DbError::Schema` sends here).
fn lock_for_reset(path: &Path) -> anyhow::Result<()> {
    match Db::open_read_only(path) {
        Ok(db) => drop(db),
        Err(crate::db::DbError::Schema { .. } | crate::db::DbError::KeyVersion { .. }) => {}
        Err(e) => {
            return Err(anyhow!(e))
                .with_context(|| format!("{} is not an explore db", path.display()));
        }
    }
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(std::time::Duration::ZERO)?;
    conn.execute_batch("BEGIN IMMEDIATE").map_err(|e| {
        anyhow!(
            "{} is busy (an explorer is writing it?): {e}",
            path.display()
        )
    })?;
    conn.execute_batch("ROLLBACK")?;
    drop(conn);
    Ok(())
}

/// `--flag value` options, `--flag` switches and positionals.
struct Args {
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(args: Vec<String>) -> anyhow::Result<Args> {
        const SWITCHES: [&str; 5] = ["--all", "--bugs", "--yes", "--export", "--trace"];
        const HELP: [&str; 2] = ["--help", "-h"];
        let mut a = Args {
            positional: Vec::new(),
            options: Vec::new(),
        };
        let mut it = args.into_iter();
        while let Some(s) = it.next() {
            if SWITCHES.contains(&s.as_str()) {
                a.options.push((s, None));
            } else if HELP.contains(&s.as_str()) {
                a.positional.push(s);
            } else if s.starts_with("--") {
                let v = it.next().ok_or_else(|| anyhow!("{s} needs a value"))?;
                a.options.push((s, Some(v)));
            } else {
                a.positional.push(s);
            }
        }
        Ok(a)
    }

    fn take(&mut self, name: &str) -> Option<String> {
        let i = self.options.iter().rposition(|(n, _)| n == name)?;
        let v = self.options.remove(i).1;
        self.options.retain(|(n, _)| n != name);
        v
    }

    fn take_num(&mut self, name: &str) -> anyhow::Result<Option<u64>> {
        self.take(name)
            .map(|v| parse_u64(&v).ok_or_else(|| anyhow!("{name} {v:?} is not a number")))
            .transpose()
    }

    fn flag(&mut self, name: &str) -> bool {
        let had = self.options.iter().any(|(n, _)| n == name);
        self.options.retain(|(n, _)| n != name);
        had
    }

    /// The id after the command.
    fn id(&mut self) -> anyhow::Result<i64> {
        let v = self
            .positional
            .get(1)
            .cloned()
            .ok_or_else(|| anyhow!("missing the finding id"))?;
        self.positional.truncate(1);
        v.parse().map_err(|_| anyhow!("bad finding id {v:?}"))
    }

    /// Errors on anything left over.
    fn done(&self) -> anyhow::Result<()> {
        if let Some(p) = self.positional.get(1) {
            bail!("unexpected argument {p:?}\n{USAGE}");
        }
        if let Some((n, _)) = self.options.first() {
            bail!("unexpected option {n}\n{USAGE}");
        }
        Ok(())
    }
}

/// Decimal or `0x` hexadecimal.
fn parse_u64(s: &str) -> Option<u64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(&h.replace('_', ""), 16).ok(),
        None => s.replace('_', "").parse().ok(),
    }
}

/// The committed checkpoint tree with the local saves over it (`play`'s store).
fn default_store() -> anyhow::Result<Store> {
    Ok(Store::layered(
        Store::default_root(content::FLAVOR),
        Store::local_root(content::FLAVOR),
    )?)
}

fn now_text() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("unix {secs}")
}

fn open_db(path: &Path, game: &Emerald, cfg: &Config) -> anyhow::Result<Db> {
    let (mut db, state) = Db::open(path, build_hash(game), content::FLAVOR)?;
    if state.build_changed {
        println!("note: the db was made on another build: the spine replaces its own");
    }
    db.set_meta("act", cfg.act.name())?;
    Ok(db)
}

/// Plays the spine and stores it; prints it.
fn play_spine(db: &mut Db, game: &Emerald, cfg: &Config) -> anyhow::Result<SpineRun> {
    let t = Instant::now();
    let s = spine::run(game, spine::steps(), cfg);
    let wall = t.elapsed();
    let run = db.record_run(&RunRow {
        started: now_text(),
        command: "spine".into(),
        config: cfg.to_ron(),
        threads: 1,
        branches: 0,
        frames: s.frames,
        wall_ms: wall.as_millis() as u64,
    })?;
    db.put_spine(&s, wall.as_millis() as u64, run)?;
    println!(
        "spine ({} {}): {} of {} steps, {} frames in {:.1} s ({:.0} frames/s); {} maps, {} \
         cells, {} scripts, {} spine flags",
        content::FLAVOR.name(),
        cfg.act.name(),
        s.positions.len().saturating_sub(1),
        s.steps.len(),
        s.frames,
        wall.as_secs_f64(),
        s.frames as f64 / wall.as_secs_f64().max(1e-9),
        s.coverage.maps.len(),
        s.coverage.cells.len(),
        s.coverage.scripts.len(),
        s.spine_flags.len(),
    );
    for p in s.positions.iter().filter(|p| p.label.is_some()) {
        let (x, y) = p.pos();
        println!(
            "  {:<20} step {:>3}  frame {:>7}  map {:#06x} ({x},{y})",
            p.label.unwrap_or_default(),
            p.index,
            p.frame,
            p.map().0
        );
    }
    if !s.reports.is_empty() {
        println!("  {} report-only findings (Unsupported)", s.reports.len());
        for r in &s.reports {
            println!("    map {:#06x} {:?}: {}", r.map.0, r.pos, r.message);
        }
    }
    let bypasses = s.bypasses();
    if !bypasses.is_empty() {
        println!(
            "  {} bypasses (engine gaps stepped over; inputs reproduce the spine up to frame {}):",
            bypasses.len(),
            s.first_bypass_frame().unwrap_or(0)
        );
        for (i, issue) in bypasses {
            println!("    step {i:>3}  {issue}");
        }
    }
    if let Some(e) = &s.error {
        println!("spine stopped at step {}: {}", e.step, e.message);
        dump(game, &s, e.step, cfg);
    }
    Ok(s)
}

/// Every position: the step, where it ended, and what it changed.
/// Emerald's decomp names (`extract::names`); hex ids on other flavours (Silice's ids name
/// other things).
fn var_label(v: u16) -> String {
    match content::FLAVOR {
        content::Flavor::Silice => format!("var {v:#06x}"),
        _ => extract::names::var_name(v),
    }
}

fn flag_label(f: u16) -> String {
    match content::FLAVOR {
        content::Flavor::Silice => format!("flag {f:#06x}"),
        _ => extract::names::flag_name(f),
    }
}

fn print_trace(s: &SpineRun) {
    for w in s.positions.windows(2) {
        let (a, b) = (&w[0].end.world, &w[1].end.world);
        let step = &s.steps[w[1].index - 1];
        let mut changes = Vec::new();
        for (v, x) in &b.vars.vars {
            let old = a.vars.vars.iter().find(|(u, _)| u == v).map_or(0, |p| p.1);
            if old != *x {
                changes.push(format!("{}={x}", var_label(v.0)));
            }
        }
        for (v, _) in &a.vars.vars {
            if !b.vars.vars.iter().any(|(u, _)| u == v) {
                changes.push(format!("{}=0", var_label(v.0)));
            }
        }
        for f in &b.vars.flags {
            if a.vars.flags.binary_search(f).is_err() {
                changes.push(format!("+{}", flag_label(f.0)));
            }
        }
        for f in &a.vars.flags {
            if b.vars.flags.binary_search(f).is_err() {
                changes.push(format!("-{}", flag_label(f.0)));
            }
        }
        let p = &w[1];
        let party: Vec<String> = b
            .party
            .iter()
            .map(|m| format!("L{} {}hp", m.level, m.hp))
            .collect();
        println!(
            "  {:>3} {:>7} {:#06x} {:?} {:?} [{}]  {step:?}  {}",
            p.index,
            p.frame,
            p.map().0,
            p.pos(),
            p.facing(),
            party.join(", "),
            changes.join(" ")
        );
    }
}

/// What a failed spine step's author needs: the last position and its candidates.
fn dump(game: &Emerald, s: &SpineRun, step: usize, cfg: &Config) {
    let Some(p) = s.positions.last() else {
        return;
    };
    let w = &p.end.world;
    println!(
        "  last position {} (after step {}): map {:#06x} {:?} facing {:?}, frame {}",
        p.index,
        p.index as i64 - 1,
        p.map().0,
        p.pos(),
        p.facing(),
        p.frame
    );
    if let Ok(sess) = game_native::Session::restore(*game, &p.end) {
        let w = sess.world();
        println!(
            "  script running {}, in battle {}, transition {}, capturable {:?}",
            w.script_running(),
            w.in_battle(),
            w.transition_active(),
            w.capturable().err()
        );
    }
    let played: Vec<_> = s.played.iter().filter(|(i, _)| *i + 3 >= step).collect();
    for m in played {
        println!("  played: {m:?}");
    }
    let reach = path::reach(game, w);
    for m in macros::candidates_with(game, w, &reach, cfg.key.cell_shift) {
        println!("  candidate {:?}", m.kind);
    }
}

fn spine_cmd(
    db_path: &Path,
    cfg: &Config,
    export: bool,
    trace: bool,
    label: Option<&str>,
) -> anyhow::Result<()> {
    let game = Emerald::for_shells();
    let mut db = open_db(db_path, &game, cfg)?;
    let s = play_spine(&mut db, &game, cfg)?;
    if trace {
        print_trace(&s);
    }
    let store = default_store()?;
    for c in store::compare(&s, cfg.act, &store)? {
        println!(
            "  {} {} {}: {}",
            if c.equal {
                "same progress as"
            } else {
                "DIFFERS from"
            },
            c.node,
            c.label,
            c.detail
        );
    }
    if export {
        for (name, id, new) in store::export(&game, &s, cfg.act, &store, label)? {
            println!(
                "  {} {name} {id} in {}",
                if new { "exported" } else { "kept" },
                store.root().display()
            );
        }
    }
    if let Some(e) = &s.error {
        bail!("the spine stopped at step {}: {}", e.step, e.message);
    }
    Ok(())
}

fn branch_cmd(
    db_path: &Path,
    cfg: &Config,
    from: Option<&str>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    let game = Emerald::for_shells();
    let mut db = open_db(db_path, &game, cfg)?;
    let s = play_spine(&mut db, &game, cfg)?;
    if let Some(f) = from
        && s.labelled(f).is_none()
    {
        bail!(
            "the spine has no checkpoint {f:?} (it has {:?})",
            s.labels()
        );
    }
    let origins = branch::origins(&s, &cfg.branch, from);
    let t = Instant::now();
    let mut targets = branch::all_targets(&game, &s, &origins, cfg);
    if let Some(n) = limit {
        targets.truncate(n);
    }
    let plan = t.elapsed();
    println!(
        "{} branches from {} origins (targets in {:.1} s), {} jobs, budget {} frames",
        targets.len(),
        origins.len(),
        plan.as_secs_f64(),
        crate::workers::threads_or_default(cfg.jobs),
        cfg.branch.budget_frames
    );
    let seen_before = db.stats()?.seen.values().sum::<u64>();
    let t = Instant::now();
    let results = branch::run_all(&game, &s, &targets, cfg);
    let wall = t.elapsed();
    let frames: u64 = results.iter().map(|r| r.frames()).sum();
    let run = db.record_run(&RunRow {
        started: now_text(),
        command: "branch".into(),
        config: cfg.to_ron(),
        threads: crate::workers::threads_or_default(cfg.jobs) as u32,
        branches: results.len() as u64,
        frames,
        wall_ms: wall.as_millis() as u64,
    })?;
    db.put_branches(&s, &results, run)?;
    let stats = db.stats()?;
    let novel = stats.seen.values().sum::<u64>() - seen_before;
    let mins = wall.as_secs_f64() / 60.0;
    println!(
        "{} branches, {frames} frames in {:.1} s ({:.0} frames/s): {novel} new things ({:.1} per \
         minute)",
        results.len(),
        wall.as_secs_f64(),
        frames as f64 / wall.as_secs_f64().max(1e-9),
        novel as f64 / mins.max(1e-9),
    );
    for name in Outcome::NAMES {
        let n = results.iter().filter(|r| r.outcome.name() == name).count();
        if n > 0 {
            println!("  {name:<18} {n}");
        }
    }
    let bugs: Vec<_> = results.iter().flat_map(|r| &r.findings).collect();
    println!(
        "  findings {} ({} divergences), reports {}",
        bugs.len(),
        bugs.iter()
            .filter(|f| f.kind == FindingKind::Divergence)
            .count(),
        results.iter().map(|r| r.reports.len()).sum::<usize>()
    );
    let names = [
        (SeenKind::Map, "maps"),
        (SeenKind::Script, "scripts"),
        (SeenKind::Flag, "flags"),
        (SeenKind::VarValue, "var values"),
        (SeenKind::Cell, "cells"),
    ];
    let parts: Vec<String> = names
        .iter()
        .map(|(k, n)| format!("{} {n}", stats.seen.get(&(*k as i64)).copied().unwrap_or(0)))
        .collect();
    println!("  beyond the spine (db): {}", parts.join(", "));
    Ok(())
}

fn branch_line(b: &BranchRow) -> String {
    format!(
        "{:>5}  pos {:>3}  {:<6} {:<18} {:>6}+{:>5}+{:>5} f  +{}m +{}s +{}f +{}v +{}c  {}  {}",
        b.id,
        b.origin,
        b.kind,
        b.outcome,
        b.explore_frames,
        b.recover_frames,
        b.window_frames,
        b.new_maps,
        b.new_scripts,
        b.new_flags,
        b.new_vars,
        b.new_cells,
        b.action,
        b.message
    )
}

fn list_branches(db_path: &Path, outcome: Option<&str>) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let bs: Vec<BranchRow> = db
        .branches()?
        .into_iter()
        .filter(|b| outcome.is_none_or(|o| b.outcome == o))
        .collect();
    if bs.is_empty() {
        println!("no branches");
    }
    for b in &bs {
        println!("{}", branch_line(b));
    }
    Ok(())
}

fn status(db_path: &Path) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let s = db.stats()?;
    println!("db        {}", db_path.display());
    println!("act       {}", db.meta("act")?.unwrap_or_default());
    match db.spine()? {
        None => println!("spine     not played (`explore spine`)"),
        Some(sp) => println!(
            "spine     {} of {} steps, {} frames, {:.1} s{}",
            sp.done,
            sp.steps,
            sp.frames,
            sp.wall_ms as f64 / 1000.0,
            sp.error
                .map_or(String::new(), |e| format!(" (stopped: {e})"))
        ),
    }
    let labels: Vec<String> = db
        .positions()?
        .into_iter()
        .filter_map(|p| p.label.map(|l| format!("{l}@{}", p.idx)))
        .collect();
    println!("checkpoints {}", labels.join(" "));
    let outcomes: Vec<String> = s
        .outcomes
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(o, n)| format!("{o} {n}"))
        .collect();
    println!("branches  {} ({})", s.branches, outcomes.join(", "));
    let seen: Vec<String> = s.seen.iter().map(|(k, n)| format!("{k}:{n}")).collect();
    println!(
        "novelty   {} ({})",
        s.seen.values().sum::<u64>(),
        seen.join(" ")
    );
    let reports = db.report_count()?;
    println!(
        "cells {} ({} by the spine)  findings {}  reports {reports}",
        s.cells,
        s.spine_cells,
        s.findings - reports
    );
    println!("build {:?}", db.build());
    Ok(())
}

fn finding_line(f: &FindingRow) -> String {
    format!(
        "{:>4}  {:<17} map {:#06x} ({:>3},{:>3})  hits {:>4}  {:<5}  {}",
        f.id,
        format!("{:?}", f.kind),
        f.map.0,
        f.pos.0,
        f.pos.1,
        f.hits,
        f.status,
        f.message
    )
}

fn list_findings(db_path: &Path, all: bool, bugs: bool) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let mut fs: Vec<FindingRow> = db
        .findings()?
        .into_iter()
        .filter(|f| all || f.status == "open")
        .filter(|f| !bugs || !f.kind.report_only())
        .collect();
    fs.sort_by(|a, b| b.hits.cmp(&a.hits).then(a.id.cmp(&b.id)));
    if fs.is_empty() {
        println!("no {}findings", if all { "" } else { "open " });
    }
    for f in &fs {
        println!("{}", finding_line(f));
    }
    Ok(())
}

fn show(db_path: &Path, id: i64) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let f = db.finding(id)?.ok_or_else(|| anyhow!("no finding {id}"))?;
    println!("{}", finding_line(&f));
    match f.branch.and_then(|b| db.branch(b).ok().flatten()) {
        Some(b) => println!("branch    {}", branch_line(&b)),
        None => println!("branch    - (the spine)"),
    }
    println!("macro     {:?}", f.macro_);
    println!("location  {}", f.location.as_deref().unwrap_or("-"));
    println!("script    {:?}", f.script.map(|s| s.0));
    println!(
        "frame     {} (tail {} frames from {})",
        f.total_frame,
        f.tail_frames,
        f.origin
            .map_or("a new game".into(), |o| format!("spine position {o}"))
    );
    println!(
        "build     {:?}  runs {}..{}",
        f.build, f.first_run, f.last_run
    );
    if !f.log.is_empty() {
        println!("log:");
        for l in f.log.lines() {
            println!("  {l}");
        }
    }
    println!("repro:    cargo xtask explore repro {id}");
    Ok(())
}

fn export(db_path: &Path, id: i64, before: u64, out: Option<PathBuf>) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let f = db.finding(id)?.ok_or_else(|| anyhow!("no finding {id}"))?;
    let total = u64::from(crate::runs::frames(&f.inputs));
    let inputs = crate::runs::split_at(&f.inputs, total.saturating_sub(before)).0;
    let trace = match findings::repro_trace(&Emerald::for_shells(), &[], &inputs) {
        Ok(t) | Err((t, _)) => t,
    };
    let out = out.unwrap_or_else(|| {
        // Always under `target/` (never next to a `--db` elsewhere).
        let default = Db::default_path(content::FLAVOR);
        let dir = default.parent().unwrap_or(Path::new("."));
        let name = if before == 0 {
            format!("finding-{id}.ron")
        } else {
            format!("finding-{id}-before-{before}.ron")
        };
        dir.join("repro").join(name)
    });
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = ron::ser::to_string_pretty(&trace, ron::ser::PrettyConfig::default())?;
    std::fs::write(&out, text).with_context(|| format!("writing {}", out.display()))?;
    println!("{} ({} frames)", out.display(), trace.frames());
    Ok(())
}

/// Whether finding `id` reproduces on `game`: `Ok(Ok(()))` when it does, `Ok(Err(why))` when
/// it does not, `Err` when it cannot be tried.
///
/// - A divergence: the spine again, then its branch again (same origin and action), which
///   must raise a divergence of the same signature.
/// - A softlock: the probe again from the inputs' end state, trapped on the same map and place.
/// - Any other: `findings::reproduce` on the inputs from a new game.
pub fn reproduce<G: GameData + Clone + Send + Sync + 'static>(
    game: &G,
    db: &Db,
    id: i64,
    cfg: &Config,
) -> anyhow::Result<Result<(), String>> {
    let f = db.finding(id)?.ok_or_else(|| anyhow!("no finding {id}"))?;
    let want = f.draft();
    match f.kind {
        FindingKind::Divergence => {
            let b = f
                .branch
                .and_then(|b| db.branch(b).ok().flatten())
                .ok_or_else(|| anyhow!("finding {id} has no branch row"))?;
            let action = Action::from_text(&b.action)
                .ok_or_else(|| anyhow!("branch {}: bad action {:?}", b.id, b.action))?;
            let kind = TargetKind::parse(&b.kind)
                .ok_or_else(|| anyhow!("branch {}: bad kind {:?}", b.id, b.kind))?;
            let s = spine::run(game, spine::steps(), cfg);
            if s.positions.len() <= b.origin as usize {
                return Ok(Err(format!(
                    "the spine no longer reaches position {}",
                    b.origin
                )));
            }
            let t = Target {
                origin: b.origin as usize,
                action,
                kind,
            };
            let r = branch::run(game, &s, &t, cfg);
            let sig = findings::signature(&want);
            Ok(
                if r.findings.iter().any(|g| findings::signature(g) == sig) {
                    Ok(())
                } else {
                    Err(format!(
                        "the branch now ends {}: {}",
                        r.outcome.name(),
                        r.message
                    ))
                },
            )
        }
        FindingKind::Softlock => {
            let trace = match findings::repro_trace(game, &[], &f.inputs) {
                Ok(t) => t,
                Err((_, at)) => return Ok(Err(format!("a panic at frame {at}"))),
            };
            let end = match findings::end_state(game, &trace) {
                Ok(end) => end,
                Err(e) => return Ok(Err(e)),
            };
            let map = end.world.field.map;
            let k = sim::softlock_k(cfg, map);
            let probed = sim::trap(game, &end, k, cfg.detect.softlock_breadth, cfg);
            Ok(match probed.trap {
                Trap::Trapped { .. }
                    if findings::softlock_signature(map, probed.place) == f.signature =>
                {
                    Ok(())
                }
                Trap::Trapped { .. } => Err(format!(
                    "trapped again, but elsewhere (place {}, map {:#06x})",
                    probed.place, map.0
                )),
                t => Err(format!("not trapped any more: {t:?}")),
            })
        }
        _ => {
            let trace = match findings::repro_trace(game, &[], &f.inputs) {
                Ok(t) | Err((t, _)) => t,
            };
            Ok(findings::reproduce(game, &trace, &want, &cfg.detect))
        }
    }
}

/// The map named `s`: an id (decimal or `0x`) or a map name (`MapData::name`).
fn parse_map<G: GameData>(game: &G, s: &str) -> anyhow::Result<MapId> {
    if let Some(n) = parse_u64(s) {
        let id = MapId(u16::try_from(n).map_err(|_| anyhow!("map id {s} out of range"))?);
        if !game.has_map(id) {
            bail!("no map {s}");
        }
        return Ok(id);
    }
    content::data::MAP_IDS
        .iter()
        .copied()
        .find(|&id| game.map(id).name.eq_ignore_ascii_case(s))
        .ok_or_else(|| anyhow!("no map named {s:?}"))
}

/// One character per cell: `#` blocked, `.` open and never stood on, `s` stood on by the
/// spine, then `1`..`4` for 1, 2-9, 10-99 and 100+ branches.
fn coverage(db_path: &Path, map: &str) -> anyhow::Result<()> {
    let game = Emerald::for_shells();
    let id = parse_map(&game, map)?;
    let db = Db::open_read_only(db_path)?;
    let m = game.map(id);
    let (w, h) = (usize::from(m.width), usize::from(m.height));
    let mut hits = vec![0u32; w * h];
    let mut spine = vec![false; w * h];
    for (_, x, y, s, n) in db.coverage(Some(id))? {
        if (0..w as i16).contains(&x) && (0..h as i16).contains(&y) {
            let i = y as usize * w + x as usize;
            hits[i] = n;
            spine[i] = s;
        }
    }
    let (mut open, mut seen) = (0, 0);
    println!("{} ({:#06x}), {w}x{h}", m.name, id.0);
    for y in 0..h {
        let row: String = (0..w)
            .map(|x| {
                let i = y * w + x;
                let blocked = block_collision(m.blocks[i]) != 0;
                if !blocked {
                    open += 1;
                }
                if hits[i] > 0 || spine[i] {
                    seen += 1;
                }
                if spine[i] {
                    return 's';
                }
                match findings::heat_bucket(hits[i]) {
                    0 if blocked => '#',
                    0 => '.',
                    b => char::from(b'0' + b),
                }
            })
            .collect();
        println!("{row}");
    }
    // Reachable: the planner's walkable cells (terrain only) a walk from a cell stood on can
    // reach.
    let grid = Grid::new(&game, id, &[], Vec::new());
    let mut reachable = vec![false; w * h];
    for i in 0..w * h {
        if (hits[i] == 0 && !spine[i]) || reachable[i] {
            continue;
        }
        let from = ((i % w) as i16, (i / w) as i16);
        for (r, d) in reachable.iter_mut().zip(grid.distances(from, false)) {
            *r |= d != u16::MAX;
        }
    }
    let reach = reachable.iter().filter(|r| **r).count();
    let on = (0..w * h)
        .filter(|&i| reachable[i] && (hits[i] > 0 || spine[i]))
        .count();
    println!(
        "{seen} cells stood on, {open} without collision; {on} of the {reach} cells reachable \
         from them ({:.1}%)",
        100.0 * on as f64 / reach.max(1) as f64
    );
    Ok(())
}
