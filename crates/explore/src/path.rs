//! Pathfinding on collision data, `setmetatile` overrides and object positions (Task A).
//!
//! Rules (shared by [`reach`] and [`next_dir`]), as the scenario driver's
//! (`game_native::scenario`), elevation ignored (decision 16):
//! - a cell is walkable when it is in the map, its block (with overrides) has collision 0, its
//!   behavior is not `Blocked`, `Door` or `Warp` (warp cells are targets, never passages) and no
//!   object other than the player stands on it or is leaving it (`pos`, `prev`);
//! - a `Ledge(d)` cell is only jumped over toward `d`, landing two cells further.

use engine::World;
use engine::checkpoint::WorldSnapshot;
use engine::data::{Behavior, Dir, GameData, MapData, MapId, PLAYER, block_collision};
use engine::flow::block_behavior;
use engine::grid::in_bounds;
use std::collections::VecDeque;

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
    /// Steps from the player's cell (a ledge jump counts one), `None` if unreachable or out of
    /// the map.
    pub fn dist(&self, x: i16, y: i16) -> Option<u16> {
        if x < 0 || y < 0 || x as u16 >= self.width || y as u16 >= self.height {
            return None;
        }
        let d = self.dist[y as usize * self.width as usize + x as usize];
        (d != u16::MAX).then_some(d)
    }

    /// Reachable cells, row-major.
    pub fn cells(&self) -> impl Iterator<Item = (i16, i16)> + '_ {
        let w = self.width.max(1) as usize;
        self.dist
            .iter()
            .enumerate()
            .filter(|(_, d)| **d != u16::MAX)
            .map(move |(i, _)| ((i % w) as i16, (i / w) as i16))
    }

    /// The map this reach set is of.
    pub fn map(&self) -> MapId {
        self.map
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

/// A map's own cells as the planner sees them: blocks with `setmetatile` overrides and the
/// cells objects occupy.
pub(crate) struct Grid<'a, G: GameData> {
    pub game: &'a G,
    pub map: &'static MapData,
    overrides: &'a [((i16, i16), u16)],
    occupied: Vec<(i16, i16)>,
}

impl<'a, G: GameData> Grid<'a, G> {
    pub fn new(
        game: &'a G,
        map: MapId,
        overrides: &'a [((i16, i16), u16)],
        occupied: Vec<(i16, i16)>,
    ) -> Self {
        Grid {
            game,
            map: game.map(map),
            overrides,
            occupied,
        }
    }

    /// The planner's view of a capturable snapshot.
    pub fn of_snapshot(game: &'a G, snap: &'a WorldSnapshot) -> Self {
        let occupied = snap.objects.objs[1..]
            .iter()
            .flat_map(|o| [o.pos, o.prev])
            .collect();
        Grid::new(game, snap.field.map, &snap.field.overrides, occupied)
    }

    pub fn in_bounds(&self, (x, y): (i16, i16)) -> bool {
        in_bounds(self.map, x, y)
    }

    /// The block at an in-bounds cell, overrides first.
    pub fn block(&self, (x, y): (i16, i16)) -> u16 {
        match self.overrides.binary_search_by_key(&(x, y), |&(c, _)| c) {
            Ok(i) => self.overrides[i].1,
            Err(_) => self.map.blocks[y as usize * self.map.width as usize + x as usize],
        }
    }

    /// Behavior of an in-bounds cell (`Blocked` out of bounds).
    pub fn behavior(&self, c: (i16, i16)) -> Behavior {
        if !self.in_bounds(c) {
            return Behavior::Blocked;
        }
        block_behavior(self.game, self.map, self.block(c))
    }

    pub fn occupied(&self, c: (i16, i16)) -> bool {
        self.occupied.contains(&c)
    }

    /// Terrain only (no objects): collision 0 and a passable behavior.
    pub fn open(&self, c: (i16, i16)) -> bool {
        self.in_bounds(c)
            && block_collision(self.block(c)) == 0
            && self.behavior(c) != Behavior::Blocked
            && !self.behavior(c).enters_warp()
    }

    pub fn walkable(&self, c: (i16, i16), avoid_grass: bool) -> bool {
        self.open(c) && !self.occupied(c) && !(avoid_grass && self.behavior(c).land_encounter())
    }

    /// Where a step from `c` toward `d` lands, if the planner allows it: a ledge toward `d` is
    /// jumped (two cells); `target` may be entered whatever it is (doors, objects' cells).
    fn step(
        &self,
        c: (i16, i16),
        d: Dir,
        avoid_grass: bool,
        target: Option<(i16, i16)>,
    ) -> Option<(i16, i16)> {
        // Holding `d` on an `ArrowWarp(d)` cell warps: never leave it that way.
        if self.behavior(c) == Behavior::ArrowWarp(d) {
            return None;
        }
        let (dx, dy) = d.delta();
        let mut n = (c.0 + dx, c.1 + dy);
        if !self.in_bounds(n) || Behavior::walled(self.behavior(c), self.behavior(n), d) {
            return None;
        }
        if let Behavior::Ledge(ld) = self.behavior(n) {
            if ld != d {
                return None;
            }
            n = (n.0 + dx, n.1 + dy);
        }
        (self.in_bounds(n) && (Some(n) == target || self.walkable(n, avoid_grass))).then_some(n)
    }

    /// BFS distances from `from` (row-major, `u16::MAX` = unreachable).
    pub fn distances(&self, from: (i16, i16), avoid_grass: bool) -> Vec<u16> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let mut dist = vec![u16::MAX; w * h];
        if !self.in_bounds(from) {
            return dist;
        }
        let idx = |(x, y): (i16, i16)| y as usize * w + x as usize;
        dist[idx(from)] = 0;
        let mut queue = VecDeque::from([from]);
        while let Some(c) = queue.pop_front() {
            let dc = dist[idx(c)];
            for d in Dir::ALL {
                if let Some(n) = self.step(c, d, avoid_grass, None)
                    && dist[idx(n)] == u16::MAX
                {
                    dist[idx(n)] = dc.saturating_add(1).min(u16::MAX - 1);
                    queue.push_back(n);
                }
            }
        }
        dist
    }

    /// The first direction of a shortest path from `from` to `to` (`to` itself may be any
    /// in-bounds cell).
    pub fn first_dir(&self, from: (i16, i16), to: (i16, i16), avoid_grass: bool) -> Option<Dir> {
        if !self.in_bounds(from) || !self.in_bounds(to) || from == to {
            return None;
        }
        let w = self.map.width as usize;
        let idx = |(x, y): (i16, i16)| y as usize * w + x as usize;
        let mut seen = vec![false; w * self.map.height as usize];
        seen[idx(from)] = true;
        let mut queue = VecDeque::from([(from, None::<Dir>)]);
        while let Some((c, d0)) = queue.pop_front() {
            if c == to {
                return d0;
            }
            for d in Dir::ALL {
                if let Some(n) = self.step(c, d, avoid_grass, Some(to))
                    && !seen[idx(n)]
                {
                    seen[idx(n)] = true;
                    queue.push_back((n, Some(d0.unwrap_or(d))));
                }
            }
        }
        None
    }
}

pub fn reach<G: GameData>(game: &G, snap: &WorldSnapshot) -> Reach {
    let grid = Grid::of_snapshot(game, snap);
    let from = snap.objects.objs[0].pos;
    Reach {
        width: grid.map.width,
        height: grid.map.height,
        dist: grid.distances(from, false),
        map: snap.field.map,
    }
}

/// The first step toward `to` from the live world (re-planned every frame; encounter tiles
/// (`Behavior::land_encounter`: tall grass, cave floors, deep sand) only when there is no other way). `overrides` come from the macro's start snapshot.
pub fn next_dir<G: GameData>(
    game: &G,
    w: &World,
    overrides: &[((i16, i16), u16)],
    to: (i16, i16),
) -> Option<Dir> {
    let grid = live_grid(game, w, overrides);
    let from = w.objects().player().pos();
    grid.first_dir(from, to, true)
        .or_else(|| grid.first_dir(from, to, false))
}

/// The planner's view of the live world (objects re-read).
pub(crate) fn live_grid<'a, G: GameData>(
    game: &'a G,
    w: &World,
    overrides: &'a [((i16, i16), u16)],
) -> Grid<'a, G> {
    let occupied = w
        .objects()
        .iter()
        .filter(|o| o.local_id != PLAYER)
        .flat_map(|o| [o.pos(), o.prev])
        .collect();
    Grid::new(game, w.map(), overrides, occupied)
}

#[cfg(test)]
mod tests;
