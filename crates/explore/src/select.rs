//! Tournament selection (Task D).

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::{NodeId, NodeRow};

/// `cfg.tournament` uniform picks from the frontier (at least one); the best `selection_score`
/// wins, lower id on ties. `None` when the frontier is empty.
pub fn tournament(g: &Graph, rng: &mut Rng64, generation: u32, cfg: &Config) -> Option<NodeId> {
    let frontier: Vec<&NodeRow> = g.frontier().collect();
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

/// The selection weight (placeholder for `fitness::selection_score`, same formula; Wave 2
/// switches to Task B's): `total / (1 + key_hits) - age * age_penalty - expanded *
/// expanded_penalty`, where `age` is the generations since the node was created. Integer
/// arithmetic only, saturating.
pub fn selection_score(n: &NodeRow, generation: u32, key_hits: u32, cfg: &Config) -> i64 {
    let age = i64::from(generation.saturating_sub(n.generation));
    let decayed = n.fitness.total / (1 + i64::from(key_hits));
    decayed
        .saturating_sub(age.saturating_mul(cfg.age_penalty))
        .saturating_sub(i64::from(n.expanded).saturating_mul(cfg.expanded_penalty))
}
