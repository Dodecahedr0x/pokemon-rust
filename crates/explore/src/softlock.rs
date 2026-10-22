//! Softlock probe: is the player trapped? (Task E)
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use engine::checkpoint::Checkpoint;
use engine::data::{GameData, MapId};

pub enum Trap {
    Trapped { reached: u32 },
    Free,
    NoExits,
}

/// The states one macro away from a state: (region, map, end state) per expansion.
pub type ExpandAll<'a> = dyn FnMut(&Checkpoint) -> Vec<(u64, MapId, Checkpoint)> + 'a;

/// From `start`, BFS over macros to depth k (at most `breadth` expansions, deterministic
/// order): Trapped iff the map has exits and every reached state has start's region and map.
pub fn probe(
    start: &Checkpoint,
    map_exits: usize,
    k: u32,
    breadth: u32,
    expand_all: &mut ExpandAll<'_>,
) -> Trap {
    todo!("Task E")
}

/// Warps + connections.
pub fn map_exits<G: GameData>(game: &G, map: MapId) -> usize {
    todo!("Task E")
}
