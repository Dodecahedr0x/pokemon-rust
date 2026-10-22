//! Batch-synchronous generations: plan, parallel jobs, merge (Task D).
//!
//! A generation is planned sequentially from an RNG seeded by `(Config::seed, generation)`
//! only, simulated in parallel ([`run_jobs`], results in job order) and merged sequentially in
//! job order. The outcome is a function of the seed, the config, the graph and the job
//! results, never of the thread count or of how many generations ran in this process.

use crate::config::Config;
use crate::graph::{Graph, Rng64, mix};
use crate::model::{
    Evaluate, GenerationDelta, Job, JobResult, Macro, MacroKind, NodeId, NodeRow, NodeStatus, Opts,
    Plan, StepResult, Stop,
};
use crate::mutate::{barren, mutate_with};
use crate::runs;
use crate::select::tournament_in;
use engine::checkpoint::{Checkpoint, state_digest};
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
    /// `rows` are the db's nodes (any order and status); `generation` is the next one to make
    /// (0 on an empty db, else the db's last generation + 1).
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
    /// with more total frames (old -> Merged), drops duplicates, collects findings.
    ///
    /// A finding's tail starts at the job parent's end state (it includes the job's steps), so
    /// it attaches to the job parent (`None` for the root job: a new game).
    ///
    /// A duplicate step (its key is live with at most as many total frames) is not a live node.
    /// When nothing of its job follows, it is dropped. When later steps or a finding follow,
    /// they must replay from its actual end state: with the live node's digest they chain from
    /// the live node, otherwise the step is kept as a `Merged` history row (`merged_into` = the
    /// live node) and they chain from it. A dropped duplicate's sightings (`seen`, cells) are
    /// credited to the live node; it adds no key hit (`key_hits` counts rows).
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
        for r in results {
            if let Some(p) = r.parent {
                *expanded.entry(p).or_default() += 1;
            }
            let step_frames: u64 = r
                .steps
                .iter()
                .map(|s| u64::from(runs::frames(&s.inputs)))
                .sum();
            let n = r.steps.len();
            let mut at = r.parent;
            for (i, step) in r.steps.into_iter().enumerate() {
                at = Some(self.merge_step(&mut d, eval, at, step, i + 1 < n, first_new));
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
        self.generation += 1;
        d
    }

    /// Merges one step that starts from `at`'s end state; returns the node the job's next step
    /// starts from.
    fn merge_step(
        &mut self,
        d: &mut GenerationDelta,
        eval: &mut dyn Evaluate,
        at: Option<NodeId>,
        step: StepResult,
        more: bool,
        first_new: NodeId,
    ) -> NodeId {
        let parent = at.and_then(|id| self.graph.node(id));
        let ev = eval.evaluate(parent, &step);
        let frames = runs::frames(&step.inputs);
        let total = parent.map_or(0, |p| p.total_frames) + u64::from(frames);
        let depth = parent.map_or(0, |p| p.depth + 1);
        let digest = state_digest(&step.end.world);
        d.stats.steps += 1;
        d.stats.frames += u64::from(frames);

        let mut superseded = None;
        let mut status = NodeStatus::Valid;
        let mut merged_into = None;
        if let Some(old) = self.graph.best(ev.key) {
            let o = self.graph.node(old).expect("best is a node");
            // More goals reached wins (the key does not hold progress: a goal seen only in the
            // summary, like a won battle, must not be dropped), then fewer frames.
            if (ev.progress, std::cmp::Reverse(total))
                > (o.progress, std::cmp::Reverse(o.total_frames))
            {
                superseded = Some(old);
            } else {
                d.stats.duplicates += 1;
                if !more || o.digest == digest {
                    record_sightings(d, &ev.seen, &step, old);
                    return old;
                }
                status = NodeStatus::Merged;
                merged_into = Some(old);
            }
        }

        let id = self.graph.next_id();
        let snap = &step.end.world;
        let row = NodeRow {
            id,
            parent: at,
            depth,
            macro_: step.macro_,
            inputs: step.inputs.clone(),
            frames,
            total_frames: total,
            end_frame: step.end.header.frame,
            key: ev.key,
            region: ev.region,
            digest,
            world_hash: step.end.header.world_hash,
            map: snap.field.map,
            pos: snap.objects.objs.first().map_or((0, 0), |o| o.pos),
            progress: ev.progress,
            fitness: ev.fitness,
            generation: self.generation,
            expanded: 0,
            status,
            merged_into,
            build: step.end.header.build,
        };
        self.graph.insert(row.clone());
        d.nodes.push(row);
        if status == NodeStatus::Valid {
            d.stats.inserted += 1;
        }
        if let Some(old) = superseded {
            self.graph.supersede(old, id);
            if old >= first_new {
                // Inserted by this delta: it is written already merged.
                let r = d.nodes.iter_mut().find(|r| r.id == old).expect("in delta");
                r.status = NodeStatus::Merged;
                r.merged_into = Some(id);
            } else {
                d.merged.push((old, id));
            }
        }
        d.touches.extend(ev.touches.iter().map(|&t| (id, t)));
        record_sightings(d, &ev.seen, &step, id);
        d.battles
            .extend(step.summary.battles.iter().map(|&b| (id, b)));
        let every = self.cfg.snapshot_every_depth;
        if every > 0 && depth.is_multiple_of(every) {
            d.snapshots.push((id, step.end.clone()));
        }
        self.fresh.push((id, step.end));
        id
    }

    /// The end states of the nodes inserted since the last call (every one goes to the
    /// snapshot cache; the delta's `snapshots` are only those the db keeps).
    pub fn take_fresh(&mut self) -> Vec<(NodeId, Arc<Checkpoint>)> {
        std::mem::take(&mut self.fresh)
    }
}

fn record_sightings(
    d: &mut GenerationDelta,
    seen: &[(crate::model::SeenKind, i64)],
    step: &StepResult,
    node: NodeId,
) {
    d.seen.extend(seen.iter().map(|&(k, v)| (k, v, node)));
    d.cells
        .extend(step.summary.cells.iter().map(|&(m, x, y)| (m, x, y, node)));
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
