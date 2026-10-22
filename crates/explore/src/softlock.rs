//! Softlock probe: is the player trapped? (Task E)
//!
//! Decision 11: trapped = the map has exits, and every state reachable within K macro-steps
//! (bounded breadth) has the start's map and region. A map without exits is never a finding.

use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::{GameData, MapId};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trap {
    /// Every one of the `reached` states (0 = no macro applied at all) kept the start's map and
    /// region.
    Trapped {
        reached: u32,
    },
    Free,
    NoExits,
}

/// The states one macro away from a state: (region, map, end state) per expansion.
pub type ExpandAll<'a> = dyn FnMut(&Checkpoint) -> Vec<(u64, MapId, Checkpoint)> + 'a;

/// From `start`, BFS over macros to depth k (at most `breadth` expansions, deterministic
/// order): Trapped iff the map has exits and every reached state has start's region and map.
///
/// The start's region is not known here (it needs the key config), so it is taken to be the
/// region of the first reached state: every reached state must share one region. Callers that
/// know it (the node's `region`) use [`probe_region`].
pub fn probe(
    start: &Checkpoint,
    map_exits: usize,
    k: u32,
    breadth: u32,
    expand_all: &mut ExpandAll<'_>,
) -> Trap {
    bfs(start, None, map_exits, k, breadth, expand_all)
}

/// [`probe`] against a known start region.
pub fn probe_region(
    start: &Checkpoint,
    region: u64,
    map_exits: usize,
    k: u32,
    breadth: u32,
    expand_all: &mut ExpandAll<'_>,
) -> Trap {
    bfs(start, Some(region), map_exits, k, breadth, expand_all)
}

fn bfs(
    start: &Checkpoint,
    mut region: Option<u64>,
    map_exits: usize,
    k: u32,
    breadth: u32,
    expand_all: &mut ExpandAll<'_>,
) -> Trap {
    if map_exits == 0 {
        return Trap::NoExits;
    }
    let map = start.world.field.map;
    let mut seen = BTreeSet::from([state_digest(&start.world)]);
    let mut queue = VecDeque::from([(start.clone(), 0u32)]);
    let mut reached = 0u32;
    while let Some((cp, depth)) = queue.pop_front() {
        if depth >= k || reached >= breadth {
            continue;
        }
        for (r, m, end) in expand_all(&cp) {
            if reached >= breadth {
                break;
            }
            reached += 1;
            if m != map || *region.get_or_insert(r) != r {
                return Trap::Free;
            }
            if seen.insert(state_digest(&end.world)) {
                queue.push_back((end, depth + 1));
            }
        }
    }
    Trap::Trapped { reached }
}

/// Warps + connections.
pub fn map_exits<G: GameData>(game: &G, map: MapId) -> usize {
    let m = game.map(map);
    m.warps.len() + m.connections.len()
}
