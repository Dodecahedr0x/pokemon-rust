//! Batch-synchronous generations: plan, parallel jobs, merge (Task D).
//!
//! A generation is planned sequentially from an RNG seeded by `(Config::seed, generation)`
//! only, simulated in parallel ([`run_jobs`], results in job order) and merged sequentially in
//! job order. The outcome is a function of the seed, the config, the graph and the job
//! results, never of the thread count or of how many generations ran in this process.

use crate::config::Config;
use crate::graph::{Graph, Rng64, mix};
use crate::model::{
    AbstractKey, BattleSummary, Evaluate, GenerationDelta, Job, JobResult, Macro, MacroKind,
    NodeId, NodeRow, NodeStatus, Opts, Plan, StepResult, Stop, Touch,
};
use crate::mutate::{barren, mutate_with};
use crate::runs;
use crate::select::tournament_in;
use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::MapId;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub struct Runner {
    pub graph: Graph,
    /// The generation the next `plan`/`merge` pair makes (0 = the root's). `merge` advances it.
    pub generation: u32,
    cfg: Config,
    /// End states of the nodes the last merges inserted, for the snapshot cache.
    fresh: Vec<(NodeId, Arc<Checkpoint>)>,
}

/// The batch when `Config::batch` is 0.
pub const DEFAULT_BATCH: usize = 32;

/// Jobs per generation: `cfg.batch`, or [`DEFAULT_BATCH`] when it is 0. Never derived from the
/// thread count, so the results do not depend on it.
pub fn batch_size(cfg: &Config) -> usize {
    if cfg.batch > 0 {
        cfg.batch
    } else {
        DEFAULT_BATCH
    }
}

/// The RNG that plans `generation`: a function of the seed and the generation only, so a run
/// resumed from the db plans what a straight run would.
pub fn generation_rng(seed: u64, generation: u32) -> Rng64 {
    Rng64(seed ^ mix(u64::from(generation) ^ 0x6A09_E667_F3BC_C908))
}

/// The root's macro: settle a new game.
pub const ROOT_MACRO: Macro = Macro {
    kind: MacroKind::Settle,
    opts: Opts {
        run_wild: false,
        choice: 0,
    },
};

impl Runner {
    /// `rows` are the db's nodes (any order and status; their inputs are dropped, see
    /// `Graph::insert`); `generation` is the next one to make (0 on an empty db, else the db's
    /// last generation + 1). The db's `key_hit` rows go to `graph.add_key_hits`.
    pub fn new(cfg: Config, rows: Vec<NodeRow>, generation: u32) -> Runner {
        Runner {
            graph: Graph::from_rows(rows),
            generation,
            cfg,
            fresh: Vec::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Whether exploration cannot go on: the root job already ran and left no node (it failed
    /// before its first step), or no node is left on the frontier. `plan` then plans nothing;
    /// the caller stops.
    pub fn stalled(&self) -> bool {
        if self.graph.is_empty() {
            self.generation > 0
        } else {
            self.graph.frontier().next().is_none()
        }
    }

    /// The next generation's jobs (an empty graph plans the root job; a stalled runner plans
    /// none). `start` is filled by the caller (cache::reach).
    ///
    /// A barren node (see `mutate::barren`) gets its forced probe once per plan, however often
    /// it is selected. Its `expanded` grows with every job planned from it, so the forced probe
    /// never repeats at one `expanded` count (and this needs no state outside the rows).
    pub fn plan(&mut self) -> Vec<(Option<NodeId>, Plan)> {
        if self.stalled() {
            return Vec::new();
        }
        if self.graph.is_empty() {
            return vec![(None, Plan::Given(vec![ROOT_MACRO]))];
        }
        let mut rng = generation_rng(self.cfg.seed, self.generation);
        let frontier: Vec<&NodeRow> = self.graph.frontier().collect();
        let mut probed = BTreeSet::new();
        let mut jobs = Vec::new();
        for _ in 0..batch_size(&self.cfg) {
            let Some(sel) =
                tournament_in(&self.graph, &frontier, &mut rng, self.generation, &self.cfg)
            else {
                break;
            };
            let force = barren(&self.graph, sel) && probed.insert(sel);
            jobs.push(mutate_with(&self.graph, &mut rng, sel, &self.cfg, force));
        }
        jobs
    }

    /// Merges in job order: evaluates every step, inserts new keys, supersedes a same-key node
    /// that reached fewer goals, or as many goals in more total frames (old -> Merged), drops
    /// duplicates, collects findings. A node can thus be superseded by its own descendant (a
    /// later step of its lineage back on its key with more goals).
    ///
    /// A finding's tail starts at the job parent's end state (it includes the job's steps), so
    /// it attaches to the job parent (`None` for the root job: the db's root start).
    ///
    /// A duplicate step (its key is live with at least as many goals and, on equal goals, at
    /// most as many total frames) is not a live node, and its sightings (`seen`, cells) are
    /// credited to the live node. When a later step of its job is inserted below it and must
    /// replay from its actual end state (its digest differs from the live node's), it is kept
    /// as a `Merged` history row (`merged_into` = the live node), written just before that
    /// step. Otherwise it leaves no row and adds one `key_hit` to its key (schema v3: no
    /// childless history rows); the job goes on from the live node when the digests match.
    pub fn merge(
        &mut self,
        mut results: Vec<JobResult>,
        eval: &mut dyn Evaluate,
    ) -> GenerationDelta {
        results.sort_by_key(|r| r.index);
        let first_new = self.graph.next_id();
        let mut d = GenerationDelta {
            generation: self.generation,
            ..GenerationDelta::default()
        };
        d.stats.jobs = results.len() as u32;
        let mut expanded: BTreeMap<NodeId, u32> = BTreeMap::new();
        let mut key_hits: BTreeMap<AbstractKey, u32> = BTreeMap::new();
        for r in results {
            d.stats.probe_frames += r.probe_frames;
            d.stats.slow_frames += r.slow_frames;
            d.stats.notes += r.notes.len() as u32;
            if let Some(p) = r.parent {
                *expanded.entry(p).or_default() += 1;
            }
            let step_frames: u64 = r
                .steps
                .iter()
                .map(|s| u64::from(runs::frames(&s.inputs)))
                .sum();
            let n = r.steps.len();
            let mut job = JobMerge {
                d: &mut d,
                key_hits: &mut key_hits,
                first_new,
                held: Vec::new(),
            };
            let mut at = At::Node(r.parent);
            for (i, step) in r.steps.into_iter().enumerate() {
                at = self.merge_step(&mut job, eval, at, step, i + 1 < n);
            }
            // Held duplicates nothing was inserted below.
            for h in std::mem::take(&mut job.held) {
                if h.id.is_none() {
                    job.hit(&mut self.graph, h.row.key);
                }
            }
            match r.stop {
                Some(Stop::Finding(f)) => {
                    let tail = u64::from(runs::frames(&f.tail));
                    // The steps' frames are counted already.
                    d.stats.frames += tail.saturating_sub(step_frames);
                    let start = r
                        .parent
                        .and_then(|id| self.graph.node(id))
                        .map_or(0, |p| p.total_frames);
                    d.findings.push((f, r.parent, start + tail));
                }
                Some(Stop::Pruned(_)) => d.stats.pruned += 1,
                Some(Stop::NotTrapped) | None => {}
            }
        }
        for (id, n) in expanded {
            if let Some(row) = self.graph.node_mut(id) {
                row.expanded += n;
            }
            d.expanded.push((id, n));
        }
        d.key_hits = key_hits.into_iter().collect();
        self.generation += 1;
        d
    }

    /// Merges one step that starts from `at`'s end state; returns where the job's next step
    /// starts from.
    fn merge_step(
        &mut self,
        job: &mut JobMerge,
        eval: &mut dyn Evaluate,
        at: At,
        step: StepResult,
        more: bool,
    ) -> At {
        let parent = match at {
            At::Node(id) => id.and_then(|id| self.graph.node(id)),
            At::Held(i) => Some(&job.held[i].row),
        };
        let ev = eval.evaluate(parent, &step);
        let frames = runs::frames(&step.inputs);
        let total = parent.map_or(0, |p| p.total_frames) + u64::from(frames);
        let depth = parent.map_or(0, |p| p.depth + 1);
        let digest = state_digest(&step.end.world);
        job.d.stats.steps += 1;
        job.d.stats.frames += u64::from(frames);

        let mut superseded = None;
        let mut duplicate_of = None;
        if let Some(old) = self.graph.best(ev.key) {
            let o = self.graph.node(old).expect("best is a node");
            // More goals reached wins, then fewer frames.
            if (ev.progress, std::cmp::Reverse(total))
                > (o.progress, std::cmp::Reverse(o.total_frames))
            {
                superseded = Some(old);
            } else {
                job.d.stats.duplicates += 1;
                record_sightings(job.d, &ev.seen, &step.summary.cells, old);
                if !more || o.digest == digest {
                    job.hit(&mut self.graph, ev.key);
                    return At::Node(Some(old));
                }
                duplicate_of = Some(old);
            }
        }

        let snap = &step.end.world;
        let mut held = Held {
            row: NodeRow {
                id: 0,
                parent: None,
                depth,
                macro_: step.macro_,
                inputs: step.inputs,
                frames,
                total_frames: total,
                end_frame: step.end.header.frame,
                key: ev.key,
                digest,
                world_hash: step.end.header.world_hash,
                map: snap.field.map,
                pos: snap.objects.objs.first().map_or((0, 0), |o| o.pos),
                progress: ev.progress,
                fitness: ev.fitness,
                generation: self.generation,
                expanded: 0,
                status: NodeStatus::Valid,
                merged_into: None,
                build: step.end.header.build,
            },
            parent: at,
            touches: ev.touches,
            battles: step.summary.battles,
            end: step.end,
            id: None,
        };
        if let Some(old) = duplicate_of {
            // Kept back until a later step of the job is inserted below it.
            held.row.status = NodeStatus::Merged;
            held.row.merged_into = Some(old);
            job.held.push(held);
            return At::Held(job.held.len() - 1);
        }
        let id = self.insert(job, held);
        job.d.stats.inserted += 1;
        if let Some(old) = superseded {
            self.graph.supersede(old, id);
            if old >= job.first_new {
                // Inserted by this delta: it is written already merged.
                let r = job
                    .d
                    .nodes
                    .iter_mut()
                    .find(|r| r.id == old)
                    .expect("in delta");
                r.status = NodeStatus::Merged;
                r.merged_into = Some(id);
            } else {
                job.d.merged.push((old, id));
            }
        }
        record_sightings(job.d, &ev.seen, &step.summary.cells, id);
        At::Node(Some(id))
    }

    /// Writes `held` (its held parents first) with the next id: the delta row, the graph row
    /// (without inputs), its touches, battles and snapshot, and its end state for the cache.
    fn insert(&mut self, job: &mut JobMerge, mut held: Held) -> NodeId {
        held.row.parent = match held.parent {
            At::Node(p) => p,
            At::Held(i) => Some(match job.held[i].id {
                Some(id) => id,
                None => {
                    let parent = job.held[i].clone();
                    let id = self.insert(job, parent);
                    job.held[i].id = Some(id);
                    id
                }
            }),
        };
        let id = self.graph.next_id();
        held.row.id = id;
        let depth = held.row.depth;
        self.graph.insert(held.row.clone());
        job.d.nodes.push(held.row);
        job.d.touches.extend(held.touches.iter().map(|&t| (id, t)));
        job.d.battles.extend(held.battles.iter().map(|&b| (id, b)));
        let every = self.cfg.snapshot_every_depth;
        if every > 0 && depth.is_multiple_of(every) {
            job.d.snapshots.push((id, held.end.clone()));
        }
        self.fresh.push((id, held.end));
        id
    }

    /// The end states of the nodes inserted since the last call (every one goes to the
    /// snapshot cache; the delta's `snapshots` are only those the db keeps).
    pub fn take_fresh(&mut self) -> Vec<(NodeId, Arc<Checkpoint>)> {
        std::mem::take(&mut self.fresh)
    }
}

/// Where a job's next step starts: a node (`None` = the root start), or a held duplicate of
/// the job (`JobMerge::held`).
#[derive(Clone, Copy)]
enum At {
    Node(Option<NodeId>),
    Held(usize),
}

/// A duplicate step kept back (or a step about to be inserted).
#[derive(Clone)]
struct Held {
    row: NodeRow,
    parent: At,
    touches: Vec<Touch>,
    battles: Vec<BattleSummary>,
    end: Arc<Checkpoint>,
    /// Its id once written.
    id: Option<NodeId>,
}

/// One job's merge state.
struct JobMerge<'a> {
    d: &'a mut GenerationDelta,
    key_hits: &'a mut BTreeMap<AbstractKey, u32>,
    first_new: NodeId,
    held: Vec<Held>,
}

impl JobMerge<'_> {
    /// A duplicate that leaves no row: one key hit, in the delta and the graph.
    fn hit(&mut self, g: &mut Graph, k: AbstractKey) {
        *self.key_hits.entry(k).or_default() += 1;
        g.hit_key(k);
    }
}

fn record_sightings(
    d: &mut GenerationDelta,
    seen: &[(crate::model::SeenKind, i64)],
    cells: &BTreeSet<(MapId, i16, i16)>,
    node: NodeId,
) {
    d.seen.extend(seen.iter().map(|&(k, v)| (k, v, node)));
    d.cells
        .extend(cells.iter().map(|&(m, x, y)| (m, x, y, node)));
}

/// rayon par_iter, results in job order. Runs on the caller's thread pool (`pool.install`).
pub fn run_jobs<F: Fn(&Job) -> JobResult + Sync>(jobs: &[Job], f: F) -> Vec<JobResult> {
    jobs.par_iter().map(&f).collect()
}

/// The planned jobs with their indices, `start` filled by `start(parent)`.
pub fn jobs(
    planned: Vec<(Option<NodeId>, Plan)>,
    mut start: impl FnMut(Option<NodeId>) -> Option<Arc<Checkpoint>>,
) -> Vec<Job> {
    planned
        .into_iter()
        .enumerate()
        .map(|(i, (parent, plan))| Job {
            index: i as u32,
            parent,
            start: start(parent),
            plan,
        })
        .collect()
}
