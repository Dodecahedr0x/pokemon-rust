//! Re-validation after a build change (Task C).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::Config;
use crate::db::{Db, DbError};
use crate::model::Macro;
use engine::checkpoint::Checkpoint;
use engine::data::GameData;
use game_native::trace::Run;

/// Re-expands a macro from a start state: new inputs and end state, or `None` (broken).
pub type Repair = dyn Fn(&Checkpoint, &Macro) -> Option<(Vec<Run>, Checkpoint)> + Sync;

pub struct RevalidateReport {
    pub valid: u64,
    pub repaired: u64,
    pub broken: u64,
}

/// After a build change: every Stale node, parents first, depth levels in parallel (rayon).
/// Replays its inputs from its parent's (new) end state; digest equal -> Valid on this build;
/// otherwise `repair(start, macro)` re-expands; Some -> Valid with new inputs, digest, key and
/// pos (repaired); None -> Broken, and its subtree Broken.
pub fn revalidate<G: GameData + Clone + Send + Sync>(
    db: &mut Db,
    game: &G,
    cfg: &Config,
    repair: &Repair,
) -> Result<RevalidateReport, DbError> {
    todo!("Task C")
}
