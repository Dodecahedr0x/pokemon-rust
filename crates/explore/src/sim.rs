//! One job on a fresh session (Task A).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::Config;
use crate::model::{Job, JobResult};
use crate::observe::StepObserver;
use engine::data::GameData;

/// Runs one job on a fresh session built on this thread. Plan::Softlock is wired by Task G.
pub fn simulate<G: GameData + Clone>(
    game: &G,
    job: &Job,
    cfg: &Config,
    obs: &mut dyn FnMut() -> Box<dyn StepObserver<G>>,
) -> JobResult {
    todo!("Task A")
}
