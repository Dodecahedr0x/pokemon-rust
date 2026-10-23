//! The `explore` command line (Task G, Wave 2): `run`, `status`, `findings`, `show`, `export`,
//! `repro`, `revalidate`, `coverage`, `digest`, `reset`.
//!
//! Every command takes `--db <path>` (default `target/explore/{stub|emerald}/explore.db`, after
//! the content flavor) and `--config <path>` (a RON `Config`; default: the built-in defaults).

use crate::config::{Budget, Config};
use crate::db::{Db, DbStats, FindingRow};
use crate::goals::Act;
use crate::model::NodeId;
use crate::path::Grid;
use crate::wire::{self, Explore};
use crate::{findings, goals};
use anyhow::{Context, anyhow, bail};
use content::Emerald;
use engine::data::{GameData, MapId, block_collision};
use game_native::store::Store;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
usage: explore <command> [--db <path>] [--config <path>]
  run [--nodes N | --frames N | --generations N | --secs N] [--jobs N] [--seed S]
      [--cache N] [--from <checkpoint id|label>] [--act slice|act1|sil_ch0]
      explore (default budget: --generations 10); one line per 10 generations;
      --jobs N (or --threads N): worker threads (default: the available parallelism less a
      quarter); the result never depends on it;
      --cache N: snapshots kept in memory (default 65536; 16384 is about 10% slower);
      --from: a fresh db explores from that checkpoint node (the committed tree with the
      local saves over it); an existing db keeps its own root and refuses another;
      --act: the goal list (default act1, which begins with the slice's; a db keeps its own)
  status                      db totals, goals, seed and build
  findings [--all] [--bugs]   open findings (--all: stale and gone ones too; --bugs: without
                              the report-only ones, `Unsupported`)
  show <id>                   one finding in full
  export <id> [--before N] [--out <path>]
                              the finding as a trace (RON) from the db's root start, N frames
                              before its end
  export --to-store <node> [--label L]
                              the node as a local checkpoint node (its inputs from the db's
                              root start; default label explore:<MAP_*>:n<node>); promote it
                              with `cargo xtask checkpoints promote <id> --label L`
  repro <id>                  replays the finding from the db's root start; exit 0 iff it reproduces
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
        .unwrap_or_else(|| Db::default_path(content::FLAVOR));
    let mut cfg = match a.take("--config") {
        Some(p) if !Path::new(&p).exists() => bail!("no config file {p}"),
        Some(p) => Config::load_or_default(Path::new(&p))?,
        None => Config::default(),
    };
    match cmd.as_str() {
        "run" => {
            for name in ["--threads", "--jobs"] {
                if let Some(t) = a.take_num(name)? {
                    cfg.threads = t as usize;
                }
            }
            if let Some(n) = a.take_num("--cache")? {
                cfg.cache_capacity = n as usize;
            }
            let seed = a.take_num("--seed")?;
            if let Some(s) = seed {
                cfg.seed = s;
            }
            let act = a
                .take("--act")
                .map(|s| Act::parse_in_build(&s).map_err(|e| anyhow!("--act: {e}")))
                .transpose()?;
            if let Some(act) = act {
                cfg.act = act;
            }
            let from = a.take("--from");
            let budget = budget(&mut a)?;
            a.done()?;
            run(&db_path, cfg, Asked { seed, act, from }, budget)
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
        "export" if a.options.iter().any(|(n, _)| n == "--to-store") => {
            let node = a.take_num("--to-store")?.expect("present") as NodeId;
            let label = a.take("--label");
            a.done()?;
            let game = Emerald::for_shells();
            let db = Db::open_read_only(&db_path)?;
            let root = wire::db_root(&game, &db)?;
            let store = match &root {
                Some(root) => root.store()?,
                None => default_store()?,
            };
            let id =
                wire::export_to_store(&game, &db, root.as_ref(), node, &store, label.as_deref())?;
            let n = store.find(&id.to_string())?;
            println!(
                "node {node} -> {id} {:?} in {}",
                n.meta.label,
                store.root().display()
            );
            Ok(())
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
                "revalidated in {:.1} s: {} valid, {} repaired, {} broken, {} kept without a replay",
                t.elapsed().as_secs_f64(),
                r.valid,
                r.repaired,
                r.broken,
                r.kept
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
        const SWITCHES: [&str; 3] = ["--all", "--bugs", "--yes"];
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

/// "goal i/n (name)" of a progress on `act`'s list.
fn goal_text(act: Act, progress: u32) -> String {
    let n = goals::goals_of(act).len();
    match progress.checked_sub(1).and_then(|i| goals::name_of(act, i)) {
        Some(name) => format!("{progress}/{n} ({name})"),
        None => format!("0/{n}"),
    }
}

/// The db's act (`meta.act`; a db without one is the default act).
fn db_act(db: &Db) -> anyhow::Result<Act> {
    match db.meta("act")? {
        Some(a) => Act::parse(&a).ok_or_else(|| anyhow!("meta act = {a:?}")),
        None => Ok(Act::default()),
    }
}

/// The committed checkpoint tree with the local saves over it (`play`'s store).
fn default_store() -> anyhow::Result<Store> {
    Ok(Store::layered(
        Store::default_root(content::FLAVOR),
        Store::local_root(content::FLAVOR),
    )?)
}

/// What `run` was asked for, beyond the config.
struct Asked {
    seed: Option<u64>,
    act: Option<Act>,
    from: Option<String>,
}

fn run(db_path: &Path, cfg: Config, asked: Asked, budget: Budget) -> anyhow::Result<()> {
    let game = Emerald::for_shells();
    let mut ex = match &asked.from {
        Some(from) => Explore::open_from(game, db_path, cfg, &default_store()?, from)?,
        None => Explore::open(game, db_path, cfg)?,
    };
    if let Some(s) = asked.seed
        && s != ex.cfg.seed
    {
        println!(
            "note: the db explores with seed {:#x} (its own), not {s:#x}",
            ex.cfg.seed
        );
    }
    if let Some(a) = asked.act
        && a != ex.cfg.act
    {
        println!(
            "note: the db explores act {} (its own), not {}",
            ex.cfg.act.name(),
            a.name()
        );
    }
    let act = ex.cfg.act;
    if ex.config_changed {
        println!("note: the config differs from the last run's (see the db's run table)");
    }
    if let Some(r) = ex.revalidated {
        println!(
            "revalidated: {} valid, {} repaired, {} broken, {} kept without a replay",
            r.valid, r.repaired, r.broken, r.kept
        );
    }
    println!(
        "exploring {} ({} threads, seed {:#x}, act {}, from {}, {budget:?})",
        db_path.display(),
        ex.threads(),
        ex.cfg.seed,
        act.name(),
        ex.root
            .as_ref()
            .map_or("a new game".into(), |r| format!("{} {:?}", r.id, r.label)),
    );
    // `gens` counts the db's generations (1-based); inserted/s and frames/s are this run's
    // (frames include the softlock probes').
    // Findings are the bugs; the report-only ones (`Unsupported`) are counted apart.
    let line = |s: &DbStats, reports: u64, sum: &wire::RunSummary| {
        let secs = sum.wall.as_secs_f64().max(1e-9);
        println!(
            "gens {:>5}  valid nodes {:>7}  inserted/s {:>7.1}  frames/s {:>9.0}  goals {}  maps {}  \
             cells {}  findings {}  reports {reports}",
            s.generation + 1,
            s.valid,
            sum.nodes as f64 / secs,
            sum.frames as f64 / secs,
            goal_text(act, s.max_progress),
            s.maps,
            s.cells,
            s.findings - reports
        );
    };
    // Goals first reached in this run, with the generation and the run's wall time.
    let mut best = ex.stats()?.max_progress;
    let sum = ex.run(budget, |p| {
        // Every goal passed this generation, in order (a lineage can pass several at once).
        while best < p.last.max_progress {
            best += 1;
            println!(
                "goal {} first reached in generation {} at {:.1} s",
                goal_text(act, best),
                p.last.generation + 1,
                p.summary.wall.as_secs_f64()
            );
        }
        if p.summary.generations.is_multiple_of(10)
            && let Ok(db) = p.db()
            && let Ok(s) = db.stats()
            && let Ok(reports) = db.report_count()
        {
            line(&s, reports, p.summary);
        }
    })?;
    if !sum.generations.is_multiple_of(10) {
        line(&ex.stats()?, ex.db.report_count()?, &sum);
    }
    println!(
        "{} generations, {} nodes inserted, {} frames ({} in {} softlock probes) in {:.1} s; {} \
         findings raised ({} report-only), {} slow frames, {} notes{}",
        sum.generations,
        sum.nodes,
        sum.frames,
        sum.probe_frames,
        sum.probes,
        sum.wall.as_secs_f64(),
        sum.findings,
        sum.reports,
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
    // `meta.generation` is the last applied one (0-based); count them like `run` does.
    let generations = match db.meta("generation")? {
        Some(_) => s.generation + 1,
        None => 0,
    };
    println!("generations {generations}");
    let act = db_act(&db)?;
    println!(
        "goals     {} (act {})",
        goal_text(act, s.max_progress),
        act.name()
    );
    if let Some(root) = db.meta("root")? {
        println!("from      {root}");
    }
    let names = content::data::SCRIPT_NAMES.len();
    // The stub has no named scripts: no share to give.
    let scripts = if names == 0 {
        format!("{}", s.scripts)
    } else {
        format!(
            "{} of {names} ({:.1}%)",
            s.scripts,
            100.0 * f64::from(s.scripts) / names as f64
        )
    };
    let reports = db.report_count()?;
    println!(
        "maps {}  cells {}  scripts {scripts}  findings {}  reports {reports}",
        s.maps,
        s.cells,
        s.findings - reports
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
    println!(
        "node      {}",
        f.node
            .map_or("the root start".into(), |n: NodeId| n.to_string())
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
                match findings::heat_bucket(hits[i]) {
                    0 if blocked => '#',
                    0 => '.',
                    b => char::from(b'0' + b),
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
