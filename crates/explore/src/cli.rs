//! The `explore` command line (Task G, Wave 2): `run`, `status`, `findings`, `show`, `export`,
//! `repro`, `revalidate`, `coverage`, `digest`, `reset`.
//!
//! Every command takes `--db <path>` (default `target/explore/{stub|emerald}/explore.db`, after
//! the content flavor) and `--config <path>` (a RON `Config`; default: the built-in defaults).

use crate::config::{Budget, Config};
use crate::db::{Db, DbStats, FindingRow};
use crate::goals;
use crate::model::NodeId;
use crate::path::Grid;
use crate::wire::{self, Explore};
use anyhow::{Context, anyhow, bail};
use content::Emerald;
use engine::data::{GameData, MapId, block_collision};
use std::path::{Path, PathBuf};

const USAGE: &str = "\
usage: explore <command> [--db <path>] [--config <path>]
  run [--nodes N | --frames N | --generations N | --secs N] [--threads N] [--seed S]
      explore (default budget: --generations 10); one line per 10 generations
  status                      db totals, goals, seed and build
  findings [--all]            open findings (--all: stale and gone ones too)
  show <id>                   one finding in full
  export <id> [--before N] [--out <path>]
                              the finding as a NewGame trace (RON), N frames before its end
  repro <id>                  replays the finding from a new game; exit 0 iff it reproduces
  revalidate                  replays every node on this build (repairs or breaks them)
  coverage <map>              ASCII heat grid of the cells stood on (map id or name)
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
        .unwrap_or_else(|| Db::default_path(content::STUB));
    let mut cfg = match a.take("--config") {
        Some(p) if !Path::new(&p).exists() => bail!("no config file {p}"),
        Some(p) => Config::load_or_default(Path::new(&p))?,
        None => Config::default(),
    };
    match cmd.as_str() {
        "run" => {
            if let Some(t) = a.take_num("--threads")? {
                cfg.threads = t as usize;
            }
            let seed = a.take_num("--seed")?;
            if let Some(s) = seed {
                cfg.seed = s;
            }
            let budget = budget(&mut a)?;
            a.done()?;
            run(&db_path, cfg, seed, budget)
        }
        "status" => {
            a.done()?;
            status(&db_path)
        }
        "findings" => {
            let all = a.flag("--all");
            a.done()?;
            list_findings(&db_path, all)
        }
        "show" => {
            let id = a.id()?;
            a.done()?;
            show(&db_path, id)
        }
        "export" => {
            let id = a.id()?;
            let before = a.take_num("--before")?.unwrap_or(0) as u32;
            let out = a.take("--out").map(PathBuf::from);
            a.done()?;
            export(&db_path, id, before, out)
        }
        "repro" => {
            let id = a.id()?;
            a.done()?;
            let db = Db::open_read_only(&db_path)?;
            match wire::reproduce(&Emerald::for_shells(), &db, id, &cfg)? {
                Ok(()) => {
                    println!("finding {id} reproduces");
                    Ok(())
                }
                Err(why) => Err(NotReproduced(format!(
                    "finding {id} does not reproduce: {why}"
                )))?,
            }
        }
        "revalidate" => {
            a.done()?;
            let t = std::time::Instant::now();
            let mut ex = Explore::open(Emerald::for_shells(), &db_path, cfg)?;
            // A build change (or an interrupted revalidation) is revalidated by `open` already.
            let r = match ex.revalidated {
                Some(r) => r,
                None => ex.revalidate()?,
            };
            println!(
                "revalidated in {:.1} s: {} valid, {} repaired, {} broken",
                t.elapsed().as_secs_f64(),
                r.valid,
                r.repaired,
                r.broken
            );
            Ok(())
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
/// takes the write lock (`BEGIN IMMEDIATE`, no busy wait) and releases it.
fn lock_for_reset(path: &Path) -> anyhow::Result<()> {
    drop(
        Db::open_read_only(path)
            .with_context(|| format!("{} is not an explore db", path.display()))?,
    );
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
        const SWITCHES: [&str; 2] = ["--all", "--yes"];
        let mut a = Args {
            positional: Vec::new(),
            options: Vec::new(),
        };
        let mut it = args.into_iter();
        while let Some(s) = it.next() {
            if SWITCHES.contains(&s.as_str()) {
                a.options.push((s, None));
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

    /// The finding id after the command.
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

fn budget(a: &mut Args) -> anyhow::Result<Budget> {
    let mut b = None;
    for (name, make) in [
        ("--nodes", Budget::Nodes as fn(u64) -> Budget),
        ("--frames", Budget::Frames),
        ("--generations", |n| Budget::Generations(n as u32)),
        ("--secs", Budget::Secs),
    ] {
        if let Some(n) = a.take_num(name)? {
            if b.is_some() {
                bail!("give one budget (--nodes, --frames, --generations or --secs)");
            }
            b = Some(make(n));
        }
    }
    Ok(b.unwrap_or(Budget::Generations(10)))
}

/// "goal i/n (name)" of a progress.
fn goal_text(progress: u32) -> String {
    let n = goals::goals().len();
    match progress.checked_sub(1).and_then(goals::name) {
        Some(name) => format!("{progress}/{n} ({name})"),
        None => format!("0/{n}"),
    }
}

fn run(db_path: &Path, cfg: Config, seed: Option<u64>, budget: Budget) -> anyhow::Result<()> {
    let mut ex = Explore::open(Emerald::for_shells(), db_path, cfg)?;
    if let Some(s) = seed
        && s != ex.cfg.seed
    {
        println!(
            "note: the db explores with seed {:#x} (its own), not {s:#x}",
            ex.cfg.seed
        );
    }
    if ex.config_changed {
        println!("note: the config differs from the last run's (see the db's run table)");
    }
    if let Some(r) = ex.revalidated {
        println!(
            "revalidated: {} valid, {} repaired, {} broken",
            r.valid, r.repaired, r.broken
        );
    }
    println!(
        "exploring {} ({} threads, seed {:#x}, {budget:?})",
        db_path.display(),
        ex.threads(),
        ex.cfg.seed
    );
    // `gens` counts the db's generations (1-based); inserted/s and frames/s are this run's
    // (frames include the softlock probes').
    let line = |s: &DbStats, sum: &wire::RunSummary| {
        let secs = sum.wall.as_secs_f64().max(1e-9);
        println!(
            "gens {:>5}  valid nodes {:>7}  inserted/s {:>7.1}  frames/s {:>9.0}  goals {}  maps {}  \
             cells {}  findings {}",
            s.generation + 1,
            s.valid,
            sum.nodes as f64 / secs,
            sum.frames as f64 / secs,
            goal_text(s.max_progress),
            s.maps,
            s.cells,
            s.findings
        );
    };
    // Goals first reached in this run, with the generation and the run's wall time.
    let mut best = ex.stats()?.max_progress;
    let sum = ex.run(budget, |p| {
        if p.last.max_progress > best {
            best = p.last.max_progress;
            println!(
                "goal {} first reached in generation {} at {:.1} s",
                goal_text(best),
                p.last.generation + 1,
                p.summary.wall.as_secs_f64()
            );
        }
        if p.summary.generations.is_multiple_of(10)
            && let Ok(s) = p.db.stats()
        {
            line(&s, p.summary);
        }
    })?;
    if !sum.generations.is_multiple_of(10) {
        line(&ex.stats()?, &sum);
    }
    println!(
        "{} generations, {} nodes, {} frames in {:.1} s; {} slow frames, {} notes{}",
        sum.generations,
        sum.nodes,
        sum.frames,
        sum.wall.as_secs_f64(),
        sum.slow_frames,
        sum.notes,
        if sum.stalled { " (stalled)" } else { "" }
    );
    let ph = sum.phases;
    println!(
        "host time: plan + reach {:.1} s, jobs {:.1} s, merge {:.1} s, db {:.1} s; cache hits {}, \
         misses {}",
        ph.plan.as_secs_f64(),
        ph.jobs.as_secs_f64(),
        ph.merge.as_secs_f64(),
        ph.apply.as_secs_f64(),
        ex.cache.hit_stats().0,
        ex.cache.hit_stats().1,
    );
    Ok(())
}

fn status(db_path: &Path) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let s = db.stats()?;
    println!("db        {}", db_path.display());
    println!(
        "nodes     {} ({} valid, {} stale, {} merged, {} broken)",
        s.nodes, s.valid, s.stale, s.merged, s.broken
    );
    println!("generation {}", s.generation);
    println!("goals     {}", goal_text(s.max_progress));
    let names = content::data::SCRIPT_NAMES.len();
    println!(
        "maps {}  cells {}  scripts {} of {names} ({:.1}%)  findings {}",
        s.maps,
        s.cells,
        s.scripts,
        100.0 * f64::from(s.scripts) / names.max(1) as f64,
        s.findings
    );
    println!(
        "seed {}  build {:?}",
        db.meta("seed")?.unwrap_or_default(),
        db.build()
    );
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

fn list_findings(db_path: &Path, all: bool) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let mut fs: Vec<FindingRow> = db
        .findings()?
        .into_iter()
        .filter(|f| all || f.status == "open")
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
    println!(
        "node      {}",
        f.node.map_or("new game".into(), |n: NodeId| n.to_string())
    );
    println!("macro     {:?}", f.macro_);
    println!("location  {}", f.location.as_deref().unwrap_or("-"));
    println!("script    {:?}", f.script.map(|s| s.0));
    println!(
        "frame     {} (tail {} frames)",
        f.total_frame,
        crate::runs::frames(&f.tail)
    );
    println!(
        "build     {:?}  generations {}..{}",
        f.build, f.first_generation, f.last_generation
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

fn export(db_path: &Path, id: i64, before: u32, out: Option<PathBuf>) -> anyhow::Result<()> {
    let db = Db::open_read_only(db_path)?;
    let (_, trace) = wire::finding_trace(&Emerald::for_shells(), &db, id, before)?;
    let out = out.unwrap_or_else(|| {
        // Always under `target/` (never next to a `--db` elsewhere).
        let default = Db::default_path(content::STUB);
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

/// One character per cell: `#` blocked, `.` open and never stood on, then `1`..`4` for
/// 1, 2-9, 10-99 and 100+ hits.
fn coverage(db_path: &Path, map: &str) -> anyhow::Result<()> {
    let game = Emerald::for_shells();
    let id = parse_map(&game, map)?;
    let db = Db::open_read_only(db_path)?;
    let m = game.map(id);
    let (w, h) = (usize::from(m.width), usize::from(m.height));
    let mut hits = vec![0u32; w * h];
    for (_, x, y, n) in db.coverage(Some(id))? {
        if (0..w as i16).contains(&x) && (0..h as i16).contains(&y) {
            hits[y as usize * w + x as usize] = n;
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
                if hits[i] > 0 {
                    seen += 1;
                }
                match hits[i] {
                    0 if blocked => '#',
                    0 => '.',
                    1 => '1',
                    2..=9 => '2',
                    10..=99 => '3',
                    _ => '4',
                }
            })
            .collect();
        println!("{row}");
    }
    // Reachable: the planner's walkable cells (terrain only: no objects, no overrides) a walk
    // from a cell stood on can reach.
    let grid = Grid::new(&game, id, &[], Vec::new());
    let mut reachable = vec![false; w * h];
    for i in 0..w * h {
        if hits[i] == 0 || reachable[i] {
            continue;
        }
        let from = ((i % w) as i16, (i / w) as i16);
        for (r, d) in reachable.iter_mut().zip(grid.distances(from, false)) {
            *r |= d != u16::MAX;
        }
    }
    let reach = reachable.iter().filter(|r| **r).count();
    let on = (0..w * h).filter(|&i| reachable[i] && hits[i] > 0).count();
    println!(
        "{seen} cells stood on, {open} without collision; {on} of the {reach} cells reachable \
         from them ({:.1}%)",
        100.0 * on as f64 / reach.max(1) as f64
    );
    Ok(())
}
