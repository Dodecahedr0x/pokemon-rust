//! The slice's goal list, in story order (Task B).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::model::SimSummary;
use engine::checkpoint::WorldSnapshot;

pub struct Goal {
    pub name: &'static str,
    pub reached: fn(&GoalCtx) -> bool,
}

pub struct GoalCtx<'a> {
    pub snap: &'a WorldSnapshot,
    pub summary: &'a SimSummary,
}

/// The slice's goals in story order (`content::STUB` picks the stub list).
pub fn goals() -> &'static [Goal] {
    todo!("Task B")
}

/// `parent` goals reached so far; advances over consecutive goals satisfied by `ctx`.
pub fn progress(parent: u32, ctx: &GoalCtx) -> u32 {
    todo!("Task B")
}
