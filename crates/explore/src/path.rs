//! Pathfinding on collision data, `setmetatile` overrides and object positions (Task A).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use engine::World;
use engine::checkpoint::WorldSnapshot;
use engine::data::{Dir, GameData, MapId};

/// Walkable cells reachable from the player, from a capturable snapshot (collision bits with
/// `FieldSnap.overrides`, ledges one-way, objects' pos and prev occupied; elevation ignored).
pub struct Reach {
    width: u16,
    height: u16,
    /// Row-major; `u16::MAX` = unreachable.
    dist: Vec<u16>,
    map: MapId,
}

impl Reach {
    pub fn dist(&self, x: i16, y: i16) -> Option<u16> {
        todo!("Task A")
    }

    /// Reachable cells, row-major.
    pub fn cells(&self) -> impl Iterator<Item = (i16, i16)> + '_ {
        todo!("Task A");
        #[allow(unreachable_code)]
        std::iter::empty()
    }
}

pub fn reach<G: GameData>(game: &G, snap: &WorldSnapshot) -> Reach {
    todo!("Task A")
}

/// The first step toward `to` from the live world (re-planned every frame; tall grass only
/// when there is no other way). `overrides` come from the macro's start snapshot.
pub fn next_dir<G: GameData>(
    game: &G,
    w: &World,
    overrides: &[((i16, i16), u16)],
    to: (i16, i16),
) -> Option<Dir> {
    todo!("Task A")
}
