//! Tournament selection (Task D).

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::{NodeId, NodeRow};

/// `cfg.tournament` uniform picks from the frontier (at least one); the best `selection_score`
/// wins, lower id on ties. `None` when the frontier is empty.
pub fn tournament(g: &Graph, rng: &mut Rng64, generation: u32, cfg: &Config) -> Option<NodeId> {
    let frontier: Vec<&NodeRow> = g.frontier().collect();
    tournament_in(g, &frontier, rng, generation, cfg)
}

/// [`tournament`] over a frontier collected once (`g.frontier()`, id ascending), for a whole
/// batch.
pub fn tournament_in(
    g: &Graph,
    frontier: &[&NodeRow],
    rng: &mut Rng64,
    generation: u32,
    cfg: &Config,
) -> Option<NodeId> {
    if frontier.is_empty() {
        return None;
    }
    let mut best: Option<(i64, NodeId)> = None;
    for _ in 0..cfg.tournament.max(1) {
        let n = frontier[rng.below(frontier.len() as u64) as usize];
        let s = selection_score(n, generation, g.key_hits(n.key), cfg);
        // Higher score wins; on a tie the lower id.
        if best.is_none_or(|(bs, bid)| s > bs || (s == bs && n.id < bid)) {
            best = Some((s, n.id));
        }
    }
    best.map(|(_, id)| id)
}

/// The selection weight: `fitness::selection_score` (Task B), re-exported for Task D's callers.
pub use crate::fitness::selection_score;
