//! Mutation operators (Task D).

use crate::config::Config;
use crate::graph::{Graph, Rng64};
use crate::model::{NodeId, Plan};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationOp {
    /// 60%: random macros from the selected node.
    Extend,
    /// 20%: random macros from the ancestor `up` levels up (1..=2, stopping at the root).
    ReplaceSuffix { up: u8 },
    /// 15%: the selected node's macro with other options, from its parent (`Extend` for the
    /// root).
    FlipOpts,
    /// 5%, or forced for a node tried at least [`PROBE_EXPANDED`] times with no child: the
    /// softlock probe.
    Probe,
}

/// A node expanded this often without a single child gets the softlock probe.
pub const PROBE_EXPANDED: u32 = 3;

/// Draws the operator for `sel` (one `below(100)`, plus `up` for `ReplaceSuffix`).
pub fn choose(g: &Graph, rng: &mut Rng64, sel: NodeId) -> MutationOp {
    let r = rng.below(100);
    let barren = g
        .node(sel)
        .is_some_and(|n| n.expanded >= PROBE_EXPANDED && g.children(sel) == 0);
    if barren {
        return MutationOp::Probe;
    }
    match r {
        0..60 => MutationOp::Extend,
        60..80 => MutationOp::ReplaceSuffix {
            up: 1 + rng.below(2) as u8,
        },
        80..95 => MutationOp::FlipOpts,
        _ => MutationOp::Probe,
    }
}

/// The job's start node (`None` = a new game) and plan for the selected node `sel`.
pub fn mutate(g: &Graph, rng: &mut Rng64, sel: NodeId, cfg: &Config) -> (Option<NodeId>, Plan) {
    let op = choose(g, rng, sel);
    apply(g, rng, sel, op, cfg)
}

/// The job for `op` on `sel`.
pub fn apply(
    g: &Graph,
    rng: &mut Rng64,
    sel: NodeId,
    op: MutationOp,
    cfg: &Config,
) -> (Option<NodeId>, Plan) {
    let random = |rng: &mut Rng64| Plan::Random {
        len: 1 + rng.below(u64::from(cfg.max_plan_len.max(1))) as u8,
        seed: rng.next_u64(),
    };
    match op {
        MutationOp::Extend => (Some(sel), random(rng)),
        MutationOp::ReplaceSuffix { up } => (Some(g.ancestor(sel, u32::from(up))), random(rng)),
        MutationOp::FlipOpts => {
            // The root has no parent to replay from: extend it instead.
            let Some(n) = g.node(sel).filter(|n| n.parent.is_some()) else {
                return (Some(sel), random(rng));
            };
            let mut m = n.macro_;
            if rng.below(2) == 0 {
                m.opts.run_wild = !m.opts.run_wild;
            } else {
                m.opts.choice = (m.opts.choice + 1) % 3;
            }
            (n.parent, Plan::Given(vec![m]))
        }
        MutationOp::Probe => {
            let k = g
                .node(sel)
                .and_then(|n| cfg.detect.softlock_k_per_map.get(&n.map.0).copied())
                .unwrap_or(cfg.detect.softlock_k);
            (
                Some(sel),
                Plan::Softlock {
                    k,
                    breadth: cfg.detect.softlock_breadth,
                },
            )
        }
    }
}
