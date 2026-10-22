//! Batch-synchronous generations: plan, parallel jobs, merge (Task D).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::{Evaluate, GenerationDelta, Job, JobResult, NodeId, NodeRow, Plan};

pub struct Runner {
    pub graph: Graph,
    pub generation: u32,
    rng: Rng64,
    cfg: Config,
}

impl Runner {
    pub fn new(cfg: Config, rows: Vec<NodeRow>, generation: u32) -> Runner {
        todo!("Task D")
    }

    /// The next generation's jobs (an empty graph plans the root job). `start` is filled by the
    /// caller (cache::reach).
    pub fn plan(&mut self) -> Vec<(Option<NodeId>, Plan)> {
        todo!("Task D")
    }

    /// Merges in job order: evaluates every step, inserts new keys, supersedes a same-key node
    /// with more total frames (old -> Merged), drops duplicates, collects findings.
    pub fn merge(&mut self, results: Vec<JobResult>, eval: &mut dyn Evaluate) -> GenerationDelta {
        todo!("Task D")
    }
}

/// rayon par_iter, results in job order.
pub fn run_jobs<F: Fn(&Job) -> JobResult + Sync>(jobs: &[Job], f: F) -> Vec<JobResult> {
    todo!("Task D")
}
