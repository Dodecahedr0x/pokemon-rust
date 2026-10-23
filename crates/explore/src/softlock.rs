//! Softlock probe: is the player trapped? (Task E)
//!
//! Decision 11: trapped = the map has exits, and every state reachable within K macro-steps
//! (bounded breadth) has the start's map and place (`key::place_region`: map, story flags and
//! vars). A map without exits is never a finding.

use engine::checkpoint::{Checkpoint, WorldSnapshot, state_digest};
use engine::data::{GameData, MapId, MovementType};
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

/// The states after a wait from a state: (place, map, end state), or `None` when the wait
/// could not end on a capturable frame.
pub type Wait<'a> = dyn FnMut(&Checkpoint) -> Option<(u64, MapId, Checkpoint)> + 'a;

/// [`probe`], tolerant of objects that block the way for a while (Task N: the wandering aide
/// of Birch's lab blocks its dead-end corridor for about 1,100 frames, longer than the probe's
/// own wait). When `start` is trapped and an object of its map walks on its own
/// ([`walks`]), up to `reprobes` times: `wait` from the last state, then probe again from
/// there. A wait that leaves the start's place or map frees it; a wait that fails keeps the
/// verdict. Returns the verdict and how many re-probes ran.
#[allow(clippy::too_many_arguments)]
pub fn probe_waiting(
    start: &Checkpoint,
    place: u64,
    map_exits: usize,
    k: u32,
    breadth: u32,
    reprobes: u32,
    expand_all: &mut ExpandAll<'_>,
    wait: &mut Wait<'_>,
) -> (Trap, u32) {
    let mut trap = probe(start, place, map_exits, k, breadth, expand_all);
    let map = start.world.field.map;
    if !matches!(trap, Trap::Trapped { .. }) || !walkers(&start.world) {
        return (trap, 0);
    }
    let mut cp = start.clone();
    let mut n = 0;
    while n < reprobes {
        let Some((r, m, next)) = wait(&cp) else {
            break;
        };
        n += 1;
        if r != place || m != map {
            return (Trap::Free, n);
        }
        trap = probe(&next, place, map_exits, k, breadth, expand_all);
        if !matches!(trap, Trap::Trapped { .. }) {
            break;
        }
        cp = next;
    }
    (trap, n)
}

/// Whether a movement type walks off its cell by itself (wanders, walks back and forth or in
/// a sequence): such an object can block a way out for a while, not for good.
pub fn walks(m: MovementType) -> bool {
    matches!(
        m,
        MovementType::Wander
            | MovementType::WanderVertical
            | MovementType::WanderHorizontal
            | MovementType::WalkBackAndForth(_)
            | MovementType::WalkSequence(_)
    )
}

/// Whether an object other than the player, visible and not frozen, walks by itself.
pub fn walkers(w: &WorldSnapshot) -> bool {
    w.objects
        .objs
        .iter()
        .skip(1)
        .any(|o| o.visible && !o.frozen && walks(o.movement))
}

/// Warps + connections.
pub fn map_exits<G: GameData>(game: &G, map: MapId) -> usize {
    let m = game.map(map);
    m.warps.len() + m.connections.len()
}
