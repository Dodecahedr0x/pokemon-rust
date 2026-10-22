//! Evaluate/sim/db glue: `Explore<G>` and its generation loop (Task G, Wave 2).
//!
//! One generation: `Runner::plan` → `runner::jobs` with each start from `cache::reach` →
//! `run_jobs(sim::simulate with Detectors)` on the explorer's own rayon pool → `Runner::merge`
//! (with the bug weight, see [`BugWeighted`]) → `Db::apply` (one transaction) → the fresh end
//! states into the `SnapCache`.
//!
//! Everything the search decides is a function of the db rows, the seed and the config: a run
//! resumed from the db (`Explore::open` again) continues exactly as a straight run, whatever the
//! thread count.

use crate::cache::{self, SnapCache};
use crate::config::{Budget, Config};
use crate::db::{Db, DbStats, FindingRow, OpenState, RunRow};
use crate::detect::Detectors;
use crate::expand;
use crate::findings;
use crate::fitness::{Archive, Evaluator};
use crate::guard;
use crate::model::{
    AbstractKey, Evaluate, Evaluation, FindingDraft, FindingKind, GenStats, Macro, NodeRow,
    StepResult,
};
use crate::observe::{NoObserver, StepObserver};
use crate::policy::Policy;
use crate::probe::Probe;
use crate::rebuild::{self, RevalidateReport};
use crate::runner::{self, Runner};
use crate::sim;
use crate::softlock::Trap;
use anyhow::{Context, anyhow, bail};
use engine::checkpoint::{Checkpoint, build_hash};
use engine::data::GameData;
use game_native::Session;
use game_native::trace::{Parent, Run, Trace, TraceRecorder, rle};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

/// The explorer on one db: the store, the snapshot cache, the runner, the evaluator (its
/// archive loaded from the db) and a rayon pool of `cfg.threads` threads (0 = rayon's default).
pub struct Explore<G> {
    pub game: G,
    pub db: Db,
    pub cache: SnapCache,
    pub runner: Runner,
    pub eval: Evaluator,
    pub cfg: Config,
    pool: rayon::ThreadPool,
    /// How the db was opened.
    pub opened: OpenState,
    /// The revalidation `open` ran (a build change, or stale nodes left by an interrupted one).
    pub revalidated: Option<RevalidateReport>,
    /// The config differs from the last `run` row's.
    pub config_changed: bool,
}

/// One generation's outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GenOutcome {
    pub generation: u32,
    pub stats: GenStats,
    pub findings: u32,
    /// The most goals reached by a node this generation inserted (0 if none).
    pub max_progress: u32,
    /// Host time per phase (report-only, never in the db).
    pub phases: Phases,
}

/// Host time of a generation's phases: planning and reaching the starts (sequential), the
/// jobs (parallel), the merge and the db transaction (sequential).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Phases {
    pub plan: Duration,
    pub jobs: Duration,
    pub merge: Duration,
    pub apply: Duration,
}

impl std::ops::AddAssign for Phases {
    fn add_assign(&mut self, o: Phases) {
        self.plan += o.plan;
        self.jobs += o.jobs;
        self.merge += o.merge;
        self.apply += o.apply;
    }
}

/// What `Explore::run` did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RunSummary {
    pub generations: u32,
    /// Live nodes inserted.
    pub nodes: u64,
    /// Frames run, the softlock probes' included.
    pub frames: u64,
    pub findings: u64,
    /// `SlowFrame`s seen (report-only, `Expanded::slow`).
    pub slow_frames: u64,
    /// Expansion notes (`Expanded::notes`, e.g. the `Cross` fallback talks).
    pub notes: u64,
    pub wall: Duration,
    pub phases: Phases,
    /// Stopped because the runner stalled, before the budget.
    pub stalled: bool,
}

/// `Explore::run`'s progress callback argument.
#[derive(Clone, Copy)]
pub struct Progress<'a> {
    pub summary: &'a RunSummary,
    pub last: &'a GenOutcome,
    /// The db, the generation committed.
    pub db: &'a Db,
}

impl<G: GameData + Clone + Send + Sync + 'static> Explore<G> {
    /// Opens (or creates) the db at `path` for `game`'s build and the content's flavor.
    ///
    /// - A fresh db records `cfg.seed` as `meta.seed`; an existing one keeps its own seed
    ///   (`cfg.seed` is replaced by it), so a resumed run plans what a straight run would.
    /// - On a build change every valid node becomes stale and is revalidated before exploring
    ///   (repair = re-expanding the node's macro from its parent's new end state). Stale nodes
    ///   left by an interrupted revalidation are revalidated too.
    /// - `config_changed` tells whether `cfg` differs from the last run's config
    ///   (`Config::differs_from_ron`: the report-only `slow_step_us` is left out).
    /// - The runner and the archive are rebuilt from the rows (`Graph::from_rows`,
    ///   `Archive::from_db`); the next generation follows the db's last one.
    pub fn open(game: G, path: &Path, mut cfg: Config) -> anyhow::Result<Explore<G>> {
        let (mut db, opened) = Db::open(path, build_hash(&game), content::STUB)
            .with_context(|| format!("opening {}", path.display()))?;
        if opened.fresh {
            db.set_meta("seed", &cfg.seed.to_string())?;
        } else if let Some(seed) = db.meta("seed")? {
            cfg.seed = seed
                .parse()
                .map_err(|_| anyhow!("meta seed = {seed:?} is not a number"))?;
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cfg.threads)
            .build()
            .context("building the thread pool")?;
        let mut revalidated = None;
        if opened.build_changed {
            db.mark_all_stale()?;
        }
        if opened.build_changed || db.stats()?.stale > 0 {
            revalidated = Some(pool.install(|| revalidate_db(&mut db, &game, &cfg))?);
        }
        let config_changed = db
            .last_run_config()?
            .is_some_and(|last| cfg.differs_from_ron(&last));
        let (runner, eval) = load(&db, &cfg)?;
        Ok(Explore {
            game,
            db,
            cache: SnapCache::new(cfg.cache_capacity),
            runner,
            eval,
            cfg,
            pool,
            opened,
            revalidated,
            config_changed,
        })
    }

    /// Marks every valid node stale and revalidates the db on this build (`explore
    /// revalidate`), then reloads the runner and the archive.
    pub fn revalidate(&mut self) -> anyhow::Result<RevalidateReport> {
        self.db.mark_all_stale()?;
        let (db, game, cfg) = (&mut self.db, &self.game, &self.cfg);
        let report = self.pool.install(|| revalidate_db(db, game, cfg))?;
        let (runner, eval) = load(&self.db, &self.cfg)?;
        self.runner = runner;
        self.eval = eval;
        self.cache.clear();
        Ok(report)
    }

    /// Whether exploration cannot go on (`Runner::stalled`).
    pub fn stalled(&self) -> bool {
        self.runner.stalled()
    }

    /// One generation; `None` when the runner is stalled (nothing planned).
    pub fn generation(&mut self) -> anyhow::Result<Option<GenOutcome>> {
        let t = Instant::now();
        let mut phases = Phases::default();
        let planned = self.runner.plan();
        if planned.is_empty() {
            return Ok(None);
        }
        let mut err = None;
        let (game, db, cache) = (&self.game, &self.db, &mut self.cache);
        let jobs = runner::jobs(planned, |parent| {
            let id = parent?;
            match cache::reach(game, db, cache, id) {
                Ok(cp) => Some(cp),
                Err(e) => {
                    err.get_or_insert_with(|| anyhow!("reaching node {id}: {e}"));
                    None
                }
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        phases.plan = t.elapsed();
        let t = Instant::now();
        let cfg = &self.cfg;
        let results = self.pool.install(|| {
            runner::run_jobs(&jobs, |job| {
                let mut obs =
                    || -> Box<dyn StepObserver<G>> { Box::new(Detectors::<G>::new(&cfg.detect)) };
                sim::simulate(game, job, cfg, &mut obs)
            })
        });
        phases.jobs = t.elapsed();
        let t = Instant::now();
        let mut eval = BugWeighted {
            inner: &mut self.eval,
            weight: cfg.weights.bug,
        };
        let delta = self.runner.merge(results, &mut eval);
        phases.merge = t.elapsed();
        let t = Instant::now();
        self.db.apply(&delta)?;
        phases.apply = t.elapsed();
        for (id, cp) in self.runner.take_fresh() {
            self.cache.put(id, cp);
        }
        Ok(Some(GenOutcome {
            generation: delta.generation,
            stats: delta.stats,
            findings: delta.findings.len() as u32,
            max_progress: delta.nodes.iter().map(|n| n.progress).max().unwrap_or(0),
            phases,
        }))
    }

    /// Generations until `budget` (counted from this call) is spent or the runner stalls;
    /// `progress` sees every generation. Records a `run` row.
    pub fn run(
        &mut self,
        budget: Budget,
        mut progress: impl FnMut(&Progress),
    ) -> anyhow::Result<RunSummary> {
        let start = Instant::now();
        let mut sum = RunSummary::default();
        while !spent(budget, &sum, start.elapsed()) {
            let Some(g) = self.generation()? else {
                sum.stalled = true;
                break;
            };
            sum.generations += 1;
            sum.nodes += u64::from(g.stats.inserted);
            sum.frames += g.stats.frames + g.stats.probe_frames;
            sum.findings += u64::from(g.findings);
            sum.slow_frames += u64::from(g.stats.slow_frames);
            sum.notes += u64::from(g.stats.notes);
            sum.phases += g.phases;
            sum.wall = start.elapsed();
            progress(&Progress {
                summary: &sum,
                last: &g,
                db: &self.db,
            });
        }
        sum.wall = start.elapsed();
        self.db.record_run(&RunRow {
            started: unix_secs().to_string(),
            config: self.cfg.to_ron(),
            threads: self.pool.current_num_threads() as u32,
            generations: sum.generations,
            nodes: sum.nodes,
            frames: sum.frames,
            wall_ms: sum.wall.as_millis() as u64,
        })?;
        Ok(sum)
    }

    /// The pool's thread count.
    pub fn threads(&self) -> usize {
        self.pool.current_num_threads()
    }

    pub fn stats(&self) -> anyhow::Result<DbStats> {
        Ok(self.db.stats()?)
    }
}

/// The runner and the evaluator from the db rows.
fn load(db: &Db, cfg: &Config) -> anyhow::Result<(Runner, Evaluator)> {
    let rows = db.nodes()?;
    let keys: Vec<(AbstractKey, u32)> = rows
        .iter()
        .map(|n| n.key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|k| (k, 1))
        .collect();
    let archive = Archive::from_db(&db.seen()?, &keys, &db.coverage(None)?);
    // `meta.generation` is the last applied one; a db without it starts at 0.
    let next = match db.meta("generation")? {
        Some(_) => db.stats()?.generation + 1,
        None => 0,
    };
    Ok((
        Runner::new(cfg.clone(), rows, next),
        Evaluator::new(archive, cfg.clone()),
    ))
}

/// `rebuild::revalidate` with [`repair`] (on the caller's pool).
fn revalidate_db<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
) -> anyhow::Result<RevalidateReport> {
    let fix = |start: Option<&Checkpoint>, m: &Macro| repair(game, start, m, cfg);
    Ok(rebuild::revalidate(db, game, cfg, &fix)?)
}

/// The revalidation repair: re-expands `m` from `start` (a new game for `None`, the root's
/// `Settle`) with the policy and no detectors. A repaired node keeps its `progress` and
/// fitness (`rebuild` as built; re-scoring would need the expansion's summary along the whole
/// lineage).
pub fn repair<G: GameData + Clone>(
    game: &G,
    start: Option<&Checkpoint>,
    m: &Macro,
    cfg: &Config,
) -> Option<(Vec<Run>, Checkpoint)> {
    let probe = Probe::new(game.clone());
    let mut s = match start {
        None => Session::new(probe),
        Some(cp) => Session::restore(probe, cp).ok()?,
    };
    let ex = expand::expand(&mut s, m, &mut Policy::new(), &mut NoObserver, &cfg.limits).ok()?;
    let end = s.checkpoint().ok()?;
    Some((rle(ex.keys), end))
}

fn spent(budget: Budget, sum: &RunSummary, elapsed: Duration) -> bool {
    match budget {
        Budget::Nodes(n) => sum.nodes >= n,
        Budget::Frames(n) => sum.frames >= n,
        Budget::Generations(n) => sum.generations >= n,
        Budget::Secs(n) => elapsed.as_secs() >= n,
    }
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The evaluator with the bug weight: a step the job's finding follows (`StepResult::bug`, the
/// last step of a job that stopped on a finding) scores `weight` (`weights.bug`) more in
/// `fitness.bug` and `fitness.total`, so selection returns to the area of a bug. `Evaluator`
/// itself passes `bugs = 0` (Task B).
pub struct BugWeighted<'a> {
    pub inner: &'a mut Evaluator,
    pub weight: i64,
}

impl Evaluate for BugWeighted<'_> {
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        let mut ev = self.inner.evaluate(parent, step);
        if step.bug {
            ev.fitness.bug = ev.fitness.bug.saturating_add(self.weight);
            ev.fitness.total = ev.fitness.total.saturating_add(self.weight);
        }
        ev
    }
}

/// A finding row as the draft that raised it (for signatures and `findings::reproduce`).
pub fn draft_of(f: &FindingRow) -> FindingDraft {
    FindingDraft {
        kind: f.kind,
        message: f.message.clone(),
        location: f.location.clone(),
        macro_: f.macro_,
        tail: f.tail.clone(),
        map: f.map,
        pos: f.pos,
        script: f.script,
        log: Vec::new(),
    }
}

/// Finding `id` as one NewGame trace (its node's lineage inputs, then the tail), cut `before`
/// frames before its last frame (clamped to 0 frames).
pub fn finding_trace<G: GameData + Clone>(
    game: &G,
    db: &Db,
    id: i64,
    before: u32,
) -> anyhow::Result<(FindingRow, Trace)> {
    let f = db.finding(id)?.ok_or_else(|| anyhow!("no finding {id}"))?;
    let lineage = match f.node {
        Some(n) => db.lineage(n)?,
        None => Vec::new(),
    };
    let runs: Vec<&[Run]> = lineage.iter().map(|n| n.inputs.as_slice()).collect();
    let trace = match findings::repro_trace(game, &runs, &f.tail) {
        Ok(t) => t,
        Err((_, frame)) => bail!("finding {id}: the replay panics early, at frame {frame}"),
    };
    if before == 0 {
        return Ok((f, trace));
    }
    let frames = trace.frames().saturating_sub(before);
    Ok((f, prefix(game, &trace, frames)?))
}

/// The first `frames` frames of a NewGame `trace`, with its hashes, by replay.
fn prefix<G: GameData + Clone>(game: &G, trace: &Trace, frames: u32) -> anyhow::Result<Trace> {
    let mut s = guard::catch(|| Session::new(game.clone()))
        .map_err(|p| anyhow!("the new game panics: {}", p.message))?;
    let mut rec = TraceRecorder::new(0);
    for k in trace.keys().take(frames as usize) {
        guard::step_guarded(&mut s, k).map_err(|p| anyhow!("panic: {}", p.message))?;
        rec.push(k, s.world().hash());
    }
    Ok(rec.trace(Parent::NewGame, trace.build))
}

/// Whether finding `id` reproduces on `game` from a new game: `Ok(Ok(()))` when it does,
/// `Ok(Err(why))` when it does not, `Err` when it cannot be tried (no such finding, a db
/// error). `findings::reproduce` on its trace, or for a softlock the probe again from the
/// trace's end state (`findings::end_state`), which must be trapped on the same map and place.
pub fn reproduce<G: GameData + Clone>(
    game: &G,
    db: &Db,
    id: i64,
    cfg: &Config,
) -> anyhow::Result<Result<(), String>> {
    if db.finding(id)?.is_none() {
        bail!("no finding {id}");
    }
    let (f, trace) = match finding_trace(game, db, id, 0) {
        Ok(ft) => ft,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let want = draft_of(&f);
    if f.kind != FindingKind::Softlock {
        return Ok(findings::reproduce(game, &trace, &want, &cfg.detect));
    }
    let end = match findings::end_state(game, &trace) {
        Ok(end) => end,
        Err(e) => return Ok(Err(e)),
    };
    let map = end.world.field.map;
    let k = sim::softlock_k(cfg, map);
    let probed = sim::trap(game, &end, k, cfg.detect.softlock_breadth, cfg);
    Ok(match probed.trap {
        Trap::Trapped { .. } if findings::softlock_signature(map, probed.place) == f.signature => {
            Ok(())
        }
        Trap::Trapped { .. } => Err(format!(
            "trapped again, but elsewhere (place {}, map {:#06x})",
            probed.place, map.0
        )),
        t => Err(format!("not trapped any more: {t:?}")),
    })
}
