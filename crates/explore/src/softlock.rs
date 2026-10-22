//! Softlock probe: is the player trapped? (Task E)
//!
//! Decision 11: trapped = the map has exits, and every state reachable within K macro-steps
//! (bounded breadth) has the start's map and place (`key::place_region`: map, story flags and
//! vars). A map without exits is never a finding.

use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::{GameData, MapId};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trap {
    /// Every one of the `reached` states (0 = no macro applied at all) kept the start's map and
    /// place.
    Trapped {
        reached: u32,
    },
    Free,
    NoExits,
}

/// The states one macro away from a state, given the expansions still allowed (`remaining`,
/// at least 1; the closure may stop there): (place, map, end state) per expansion.
pub type ExpandAll<'a> = dyn FnMut(&Checkpoint, u32) -> Vec<(u64, MapId, Checkpoint)> + 'a;

/// From `start` (whose place, `key::place_region`, is `place`), BFS over macros to depth k
/// (at most `breadth` expansions, deterministic order, identical states expanded once):
/// Trapped iff the map has exits and every reached state has start's place and map.
///
/// `NoExits` when `map_exits == 0` (never a finding); `Free` when `k == 0 || breadth == 0`
/// (nothing was probed, so nothing is claimed).
pub fn probe(
    start: &Checkpoint,
    place: u64,
    map_exits: usize,
    k: u32,
    breadth: u32,
    expand_all: &mut ExpandAll<'_>,
) -> Trap {
    if map_exits == 0 {
        return Trap::NoExits;
    }
    if k == 0 || breadth == 0 {
        return Trap::Free;
    }
    let map = start.world.field.map;
    let mut seen = BTreeSet::from([state_digest(&start.world)]);
    let mut queue = VecDeque::from([(start.clone(), 0u32)]);
    let mut reached = 0u32;
    while let Some((cp, depth)) = queue.pop_front() {
        if depth >= k || reached >= breadth {
            continue;
        }
        for (r, m, end) in expand_all(&cp, breadth - reached) {
            if reached >= breadth {
                break;
            }
            reached += 1;
            if m != map || r != place {
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
