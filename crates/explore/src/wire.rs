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
//!
//! A db explores from a new game, or from a checkpoint-store node (`run --from`, Milestone 8):
//! its [`Root`], recorded in `meta.root` (and the store's place in `meta.root_base` and
//! `meta.root_local`). Node and finding exports then start at that node
//! (`Parent::Checkpoint`).

use crate::cache::{self, SnapCache};
use crate::config::{Budget, Config};
use crate::db::{Db, DbStats, FindingRow, OpenState, RunRow};
use crate::detect::Detectors;
use crate::expand;
use crate::findings;
use crate::fitness::{Archive, Evaluator};
use crate::goals::Act;
use crate::guard;
use crate::model::{
    AbstractKey, Evaluate, Evaluation, FindingDraft, FindingKind, GenStats, Macro, NodeId, NodeRow,
    NodeStatus, StepResult,
};
use crate::observe::{NoObserver, StepObserver};
use crate::policy::Policy;
use crate::rebuild::{self, RevalidateReport};
use crate::runner::{self, Runner};
use crate::runs;
use crate::sim;
use crate::softlock::Trap;
use anyhow::{Context, anyhow, bail};
use engine::checkpoint::{Checkpoint, build_hash, state_digest};
use engine::data::GameData;
use game_native::Session;
use game_native::store::Store;
use game_native::trace::{CheckpointId, Parent, Run, Trace, TraceRecorder, rle};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Where a `--from` db's root node starts: a checkpoint-store node, materialized on this build.
#[derive(Clone, Debug)]
pub struct Root {
    pub id: CheckpointId,
    pub label: String,
    pub checkpoint: Arc<Checkpoint>,
    /// The store it was taken from: its own root and, if layered, its base.
    pub local: PathBuf,
    pub base: Option<PathBuf>,
}

impl Root {
    /// Node `id_or_label` of `store`, materialized on `game`'s build. A node whose replay
    /// diverges from its recorded digest is refused (verify the store first).
    pub fn load<G: GameData + Clone>(
        game: &G,
        store: &Store,
        id_or_label: &str,
    ) -> anyhow::Result<Root> {
        let node = store
            .find(id_or_label)
            .with_context(|| format!("finding checkpoint {id_or_label:?}"))?;
        let m = game_native::checkpoints::materialize(store, game, node.meta.id)
            .map_err(|e| anyhow!("materializing {}: {e}", node.meta.id))?;
        if m.diverged {
            bail!(
                "checkpoint {} diverges on this build: run `cargo xtask checkpoints verify` first",
                node.meta.id
            );
        }
        Ok(Root {
            id: node.meta.id,
            label: node.meta.label,
            checkpoint: Arc::new(m.checkpoint),
            local: store.root().to_path_buf(),
            base: store.base().map(Path::to_path_buf),
        })
    }

    /// The store the root was taken from.
    pub fn store(&self) -> anyhow::Result<Store> {
        Ok(match &self.base {
            Some(base) => Store::layered(base, &self.local)?,
            None => Store::open(&self.local)?,
        })
    }

    /// The parent of traces that start at the root.
    pub fn parent(&self) -> Parent {
        Parent::Checkpoint(self.id)
    }
}

/// The db's root (`meta.root`), materialized from the store recorded with it; `None` for a db
/// that explores from a new game. The root node's state must still be the one the db was
/// rooted at (`meta.root_digest`).
pub fn db_root<G: GameData + Clone>(game: &G, db: &Db) -> anyhow::Result<Option<Root>> {
    let Some(id) = db.meta("root")? else {
        return Ok(None);
    };
    let local = PathBuf::from(db.meta("root_local")?.unwrap_or_default());
    let base = db
        .meta("root_base")?
        .filter(|b| !b.is_empty())
        .map(PathBuf::from);
    let store = match &base {
        Some(base) => Store::layered(base, &local)?,
        None => Store::open(&local)?,
    };
    let root = Root::load(game, &store, &id)?;
    let want = db.meta("root_digest")?.unwrap_or_default();
    let got = state_digest(&root.checkpoint.world).to_string();
    if got != want {
        bail!(
            "the db's root {id} reaches another state than when the db was created (digest {got}, \
             recorded {want}): run `explore reset --yes`"
        );
    }
    Ok(Some(root))
}

/// A fresh db's meta: its seed, act and root (`None`: a new game), in one transaction.
fn set_fresh_meta(db: &mut Db, cfg: &Config, root: Option<&Root>) -> anyhow::Result<()> {
    let mut kv = vec![
        ("seed", cfg.seed.to_string()),
        ("act", cfg.act.name().to_string()),
    ];
    if let Some(root) = root {
        let base = root
            .base
            .as_ref()
            .map_or(String::new(), |b| b.display().to_string());
        kv.extend([
            ("root", root.id.to_string()),
            (
                "root_digest",
                state_digest(&root.checkpoint.world).to_string(),
            ),
            ("root_local", root.local.display().to_string()),
            ("root_base", base),
        ]);
    }
    Ok(db.set_metas(&kv)?)
}

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
    /// Where the root node starts (`None`: a new game).
    pub root: Option<Root>,
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
    /// Finding rows raised, the report-only ones included.
    pub findings: u32,
    /// Of `findings`, the report-only ones (`FindingKind::report_only`).
    pub reports: u32,
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
    /// Findings raised, the report-only ones included.
    pub findings: u64,
    /// Of `findings`, the report-only ones (`FindingKind::report_only`).
    pub reports: u64,
    /// `SlowFrame`s seen (report-only, `Expanded::slow`).
    pub slow_frames: u64,
    /// Expansion notes (`Expanded::notes`, e.g. dropped engine events).
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
    /// - A fresh db records `cfg.seed` as `meta.seed` and `cfg.act` as `meta.act`; an existing
    ///   one keeps its own seed and act (`cfg` takes them), so a resumed run plans what a
    ///   straight run would.
    /// - An existing `--from` db opens at its own root ([`db_root`]); a fresh db explores from
    ///   a new game ([`Explore::open_from`] roots it at a store node).
    /// - On a build change every valid node becomes stale and is revalidated before exploring
    ///   (repair = re-expanding the node's macro from its parent's new end state). Stale nodes
    ///   left by an interrupted revalidation are revalidated too.
    /// - `config_changed` tells whether `cfg` differs from the last run's config
    ///   (`Config::differs_from_ron`: the report-only `slow_step_us` is left out).
    /// - The runner and the archive are rebuilt from the rows (`Graph::from_rows`,
    ///   `Archive::from_db`); the next generation follows the db's last one.
    pub fn open(game: G, path: &Path, cfg: Config) -> anyhow::Result<Explore<G>> {
        Self::open_at(game, path, cfg, None)
    }

    /// [`Explore::open`] for a db rooted at node `id_or_label` of `store` (`run --from`): a
    /// fresh db records it as its root; an existing db must have that root.
    pub fn open_from(
        game: G,
        path: &Path,
        cfg: Config,
        store: &Store,
        id_or_label: &str,
    ) -> anyhow::Result<Explore<G>> {
        let root = Root::load(&game, store, id_or_label)?;
        Self::open_at(game, path, cfg, Some(root))
    }

    fn open_at(
        game: G,
        path: &Path,
        mut cfg: Config,
        from: Option<Root>,
    ) -> anyhow::Result<Explore<G>> {
        let (mut db, opened) = Db::open(path, build_hash(&game), content::STUB)
            .with_context(|| format!("opening {}", path.display()))?;
        let root = if opened.fresh {
            set_fresh_meta(&mut db, &cfg, from.as_ref())?;
            from
        } else {
            if let Some(seed) = db.meta("seed")? {
                cfg.seed = seed
                    .parse()
                    .map_err(|_| anyhow!("meta seed = {seed:?} is not a number"))?;
            }
            if let Some(act) = db.meta("act")? {
                cfg.act = Act::parse(&act).ok_or_else(|| anyhow!("meta act = {act:?}"))?;
            }
            let root = db_root(&game, &db)?;
            match (&root, &from) {
                (_, None) => {}
                (Some(r), Some(f)) if r.id == f.id => {}
                (r, Some(f)) => bail!(
                    "{} explores from {}, not {}: use another --db, or `explore reset --yes`",
                    path.display(),
                    r.as_ref().map_or("a new game".into(), |r| r.id.to_string()),
                    f.id
                ),
            }
            root
        };
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(cfg.threads)
            .build()
            .context("building the thread pool")?;
        let mut revalidated = None;
        if opened.build_changed {
            db.mark_all_stale()?;
        }
        let start = root.as_ref().map(|r| r.checkpoint.clone());
        if opened.build_changed || db.stats()?.stale > 0 {
            revalidated =
                Some(pool.install(|| revalidate_db(&mut db, &game, &cfg, start.clone()))?);
        }
        let config_changed = db
            .last_run_config()?
            .is_some_and(|last| cfg.differs_from_ron(&last));
        let (runner, eval) = load(&db, &cfg)?;
        let mut cache = SnapCache::new(cfg.cache_capacity);
        cache.set_root(start);
        Ok(Explore {
            game,
            db,
            cache,
            runner,
            eval,
            cfg,
            pool,
            root,
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
        let start = self.root.as_ref().map(|r| r.checkpoint.clone());
        let report = self.pool.install(|| revalidate_db(db, game, cfg, start))?;
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
            let Some(id) = parent else {
                return cache.root().cloned();
            };
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
            reports: delta
                .findings
                .iter()
                .filter(|f| f.0.kind.report_only())
                .count() as u32,
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
            sum.reports += u64::from(g.reports);
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

/// The runner and the evaluator from the db rows (without their inputs) and `key_hit`.
fn load(db: &Db, cfg: &Config) -> anyhow::Result<(Runner, Evaluator)> {
    let rows = db.nodes_light()?;
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
    let mut runner = Runner::new(cfg.clone(), rows, next);
    runner.graph.add_key_hits(&db.key_hits()?);
    Ok((runner, Evaluator::new(archive, cfg.clone())))
}

/// `rebuild::revalidate_from` the root's start with [`repair`] (on the caller's pool).
fn revalidate_db<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
    root: Option<Arc<Checkpoint>>,
) -> anyhow::Result<RevalidateReport> {
    let fix = |start: Option<&Checkpoint>, m: &Macro| repair(game, start, m, cfg);
    Ok(rebuild::revalidate_from(db, game, cfg, root, &fix)?)
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
    let game = game.clone();
    let mut s = match start {
        None => Session::new(game),
        Some(cp) => Session::restore(game, cp).ok()?,
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

/// The `(id, state)` a trace from `root` starts at (`None`: a new game).
fn start_of(root: Option<&Root>) -> Option<(CheckpointId, &Checkpoint)> {
    root.map(|r| (r.id, r.checkpoint.as_ref()))
}

/// Finding `id` as one trace from the db's root start (its node's lineage inputs, then the
/// tail): a NewGame trace, or one whose parent is the root node of a `--from` db. Cut `before`
/// frames before its last frame (clamped to 0 frames).
pub fn finding_trace<G: GameData + Clone>(
    game: &G,
    db: &Db,
    id: i64,
    before: u32,
) -> anyhow::Result<(FindingRow, Trace)> {
    let root = db_root(game, db)?;
    finding_trace_at(game, db, root.as_ref(), id, before)
}

/// [`finding_trace`] from `root`, the db's root ([`db_root`]) loaded once by the caller.
fn finding_trace_at<G: GameData + Clone>(
    game: &G,
    db: &Db,
    root: Option<&Root>,
    id: i64,
    before: u32,
) -> anyhow::Result<(FindingRow, Trace)> {
    let f = db.finding(id)?.ok_or_else(|| anyhow!("no finding {id}"))?;
    let lineage = match f.node {
        Some(n) => db.lineage(n)?,
        None => Vec::new(),
    };
    let runs: Vec<&[Run]> = lineage.iter().map(|n| n.inputs.as_slice()).collect();
    let trace = match findings::repro_trace_from(game, start_of(root), &runs, &f.tail) {
        Ok(t) => t,
        Err((_, frame)) => bail!("finding {id}: the replay panics early, at frame {frame}"),
    };
    if before == 0 {
        return Ok((f, trace));
    }
    let frames = trace.frames().saturating_sub(before);
    Ok((f, prefix(game, root, &trace, frames)?))
}

/// A session at `root`'s start (a new game for `None`), under the guard.
fn session_from<G: GameData + Clone>(game: &G, root: Option<&Root>) -> anyhow::Result<Session<G>> {
    match root {
        None => guard::catch(|| Session::new(game.clone()))
            .map_err(|p| anyhow!("the new game panics: {}", p.message)),
        Some(r) => guard::catch(|| Session::restore(game.clone(), &r.checkpoint))
            .map_err(|p| anyhow!("restoring {} panics: {}", r.id, p.message))?
            .map_err(|e| anyhow!("restoring {}: {e:?}", r.id)),
    }
}

/// The first `frames` frames of `trace` (from `root`'s start), with its hashes, by replay.
fn prefix<G: GameData + Clone>(
    game: &G,
    root: Option<&Root>,
    trace: &Trace,
    frames: u32,
) -> anyhow::Result<Trace> {
    let mut s = session_from(game, root)?;
    let mut rec = TraceRecorder::new(trace.start_frame);
    for k in trace.keys().take(frames as usize) {
        guard::step_guarded(&mut s, k).map_err(|p| anyhow!("panic: {}", p.message))?;
        rec.push(k, s.world().hash());
    }
    Ok(rec.trace(trace.parent, trace.build))
}

/// The prefix of a store label from [`export_to_store`].
pub const EXPORT_PREFIX: &str = "explore:";

/// The default label of an exported node: `explore:<MAP_*>:n<id>`, decomp constants only.
pub fn export_label(n: &NodeRow) -> String {
    let map = extract::names::map((n.map.0 >> 8) as u8, n.map.0 as u8).map_or_else(
        || format!("MAP_{:#06x}", n.map.0),
        |m| m.constant.to_string(),
    );
    format!("{EXPORT_PREFIX}{map}:n{}", n.id)
}

/// Writes node `id` into `store` as a checkpoint node: its lineage's inputs from the db's root
/// start (a NewGame trace, or one whose parent is the root node), with hashes by replay, and
/// its end state, which must have the node's digest on this build. `label` defaults to
/// [`export_label`]; a taken label gets a ` #2`, ` #3`, … suffix. The replay is the node's
/// verification: its verdict (Valid) is recorded in the store's `status.ron`, so that
/// `cargo xtask checkpoints promote` accepts it.
///
/// `root` is the db's ([`db_root`], loaded once by the caller). `store` must be a layered
/// store (local saves over the committed tree): the committed tree is written only by
/// `promote`.
pub fn export_to_store<G: GameData + Clone>(
    game: &G,
    db: &Db,
    root: Option<&Root>,
    id: NodeId,
    store: &Store,
    label: Option<&str>,
) -> anyhow::Result<CheckpointId> {
    if store.base().is_none() {
        bail!(
            "{} is not a local store over the committed tree: exports go to the local store \
             (`cargo xtask checkpoints promote` moves them into the committed tree)",
            store.root().display()
        );
    }
    if db.meta("root")? != root.map(|r| r.id.to_string()) {
        bail!("the given root is not the db's (`wire::db_root`)");
    }
    let lineage = db.lineage(id)?;
    let n = lineage.last().ok_or_else(|| anyhow!("no node {id}"))?;
    if n.status == NodeStatus::Broken {
        bail!("node {id} is broken on this build");
    }
    let mut s = session_from(game, root)?;
    let mut rec = TraceRecorder::new(s.world().frame());
    for k in lineage.iter().flat_map(|n| runs::keys(&n.inputs)) {
        guard::step_guarded(&mut s, k).map_err(|p| anyhow!("node {id}: panic: {}", p.message))?;
        rec.push(k, s.world().hash());
    }
    let cp = s
        .checkpoint()
        .map_err(|e| anyhow!("node {id}: its end is not capturable: {e:?}"))?;
    if state_digest(&cp.world) != n.digest {
        bail!("node {id} replays to another state on this build: run `explore revalidate`");
    }
    let parent = root.map_or(Parent::NewGame, Root::parent);
    let trace = rec.trace(parent, build_hash(game));
    let want = label.map_or_else(|| export_label(n), str::to_string);
    let mut label = want.clone();
    let mut i = 1;
    loop {
        match store.add(&label, trace.clone(), &cp) {
            Err(game_native::store::StoreError::Label(_)) if i < 1000 => {
                i += 1;
                label = format!("{want} #{i}");
            }
            r => {
                let new = r?;
                store.merge_statuses(&[(new, game_native::store::Status::Valid)])?;
                return Ok(new);
            }
        }
    }
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
    let root = db_root(game, db)?;
    let start = start_of(root.as_ref());
    let (f, trace) = match finding_trace_at(game, db, root.as_ref(), id, 0) {
        Ok(ft) => ft,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let want = draft_of(&f);
    if f.kind != FindingKind::Softlock {
        return Ok(findings::reproduce_from(
            game,
            start,
            &trace,
            &want,
            &cfg.detect,
        ));
    }
    let end = match findings::end_state_from(game, start, &trace) {
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
