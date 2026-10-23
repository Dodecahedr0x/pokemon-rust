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
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::sync::Arc;

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
    /// The map's [`Terrain`] under `overrides`: built by the first search, or handed over by
    /// [`Grid::with_terrain`].
    terrain: OnceCell<Arc<Terrain>>,
}

/// A map's cells under a set of `setmetatile` overrides, classified once for the searches
/// (row-major): a search looks each cell up instead of classifying its block at every edge.
/// Objects are not in it (they move; [`Grid`] masks them per search).
pub(crate) struct Terrain {
    map: &'static MapData,
    behavior: Vec<Behavior>,
    /// [`Grid::open`].
    open: Vec<bool>,
    /// Per cell, per direction in `Dir::ALL` order: the cell index a step lands on as far as
    /// the terrain decides (arrow warps, walls, ledges jumped), or [`NO_STEP`].
    land: Vec<[u32; 4]>,
}

/// No landing cell in [`Terrain::land`].
const NO_STEP: u32 = u32::MAX;

impl Terrain {
    fn new<G: GameData>(game: &G, map: &'static MapData, overrides: &[((i16, i16), u16)]) -> Self {
        let (w, h) = (map.width as usize, map.height as usize);
        let cell = |i: usize| ((i % w) as i16, (i / w) as i16);
        let mut behavior = Vec::with_capacity(w * h);
        let mut open = Vec::with_capacity(w * h);
        for i in 0..w * h {
            let block = block_of(map, overrides, cell(i));
            let b = block_behavior(game, map, block);
            behavior.push(b);
            open.push(block_collision(block) == 0 && b != Behavior::Blocked && !b.enters_warp());
        }
        let index = |(x, y): (i16, i16)| in_bounds(map, x, y).then(|| y as usize * w + x as usize);
        let land = (0..w * h)
            .map(|i| {
                let c = cell(i);
                Dir::ALL.map(|d| {
                    // As the planner's step rules (module docs), objects and grass aside.
                    if behavior[i] == Behavior::ArrowWarp(d) {
                        return NO_STEP;
                    }
                    let (dx, dy) = d.delta();
                    let Some(mut n) = index((c.0 + dx, c.1 + dy)) else {
                        return NO_STEP;
                    };
                    if Behavior::walled(behavior[i], behavior[n], d) {
                        return NO_STEP;
                    }
                    if let Behavior::Ledge(ld) = behavior[n] {
                        if ld != d {
                            return NO_STEP;
                        }
                        match index((c.0 + 2 * dx, c.1 + 2 * dy)) {
                            Some(j) => n = j,
                            None => return NO_STEP,
                        }
                    }
                    n as u32
                })
            })
            .collect();
        Terrain {
            map,
            behavior,
            open,
            land,
        }
    }
}

/// The block at an in-bounds cell, overrides (sorted by cell) first.
fn block_of(map: &MapData, overrides: &[((i16, i16), u16)], (x, y): (i16, i16)) -> u16 {
    match overrides.binary_search_by_key(&(x, y), |&(c, _)| c) {
        Ok(i) => overrides[i].1,
        Err(_) => map.blocks[y as usize * map.width as usize + x as usize],
    }
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
            terrain: OnceCell::new(),
        }
    }

    /// This grid with `terrain`, which the caller built for the same map and overrides
    /// ([`Grid::terrain`] of an earlier grid); a table of another map is ignored.
    pub fn with_terrain(self, terrain: Option<Arc<Terrain>>) -> Self {
        if let Some(t) = terrain
            && std::ptr::eq(t.map, self.map)
        {
            let _ = self.terrain.set(t);
        }
        self
    }

    /// The terrain table, built on the first call.
    pub fn terrain(&self) -> &Arc<Terrain> {
        (self.terrain).get_or_init(|| Arc::new(Terrain::new(self.game, self.map, self.overrides)))
    }

    /// The cells objects other than the player occupy (`pos` and `prev`).
    pub fn occupied_cells(&self) -> &[(i16, i16)] {
        &self.occupied
    }

    fn index(&self, (x, y): (i16, i16)) -> usize {
        y as usize * self.map.width as usize + x as usize
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
    pub fn block(&self, c: (i16, i16)) -> u16 {
        block_of(self.map, self.overrides, c)
    }

    /// Behavior of an in-bounds cell (`Blocked` out of bounds).
    pub fn behavior(&self, c: (i16, i16)) -> Behavior {
        if !self.in_bounds(c) {
            return Behavior::Blocked;
        }
        match self.terrain.get() {
            Some(t) => t.behavior[self.index(c)],
            None => block_behavior(self.game, self.map, self.block(c)),
        }
    }

    pub fn occupied(&self, c: (i16, i16)) -> bool {
        self.occupied.contains(&c)
    }

    /// Terrain only (no objects): collision 0 and a passable behavior.
    pub fn open(&self, c: (i16, i16)) -> bool {
        if let (true, Some(t)) = (self.in_bounds(c), self.terrain.get()) {
            return t.open[self.index(c)];
        }
        self.in_bounds(c)
            && block_collision(self.block(c)) == 0
            && self.behavior(c) != Behavior::Blocked
            && !self.behavior(c).enters_warp()
    }

    pub fn walkable(&self, c: (i16, i16), avoid_grass: bool) -> bool {
        self.open(c) && !self.occupied(c) && !(avoid_grass && self.behavior(c).land_encounter())
    }

    /// Per cell: whether a search may land there ([`Grid::walkable`]); `target` always may
    /// (doors, objects' cells).
    fn landable(&self, t: &Terrain, avoid_grass: bool, target: Option<(i16, i16)>) -> Vec<bool> {
        let mut ok: Vec<bool> = (t.open.iter().zip(&t.behavior))
            .map(|(&open, &b)| open && !(avoid_grass && b == Behavior::TallGrass))
            .collect();
        for &c in &self.occupied {
            if self.in_bounds(c) {
                ok[self.index(c)] = false;
            }
        }
        if let Some(c) = target.filter(|&c| self.in_bounds(c)) {
            ok[self.index(c)] = true;
        }
        ok
    }

    /// BFS distances from `from` (row-major, `u16::MAX` = unreachable).
    pub fn distances(&self, from: (i16, i16), avoid_grass: bool) -> Vec<u16> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let mut dist = vec![u16::MAX; w * h];
        if !self.in_bounds(from) {
            return dist;
        }
        let t = self.terrain();
        let ok = self.landable(t, avoid_grass, None);
        let start = self.index(from);
        dist[start] = 0;
        let mut queue = VecDeque::from([start as u32]);
        while let Some(c) = queue.pop_front() {
            let dc = dist[c as usize];
            for n in t.land[c as usize] {
                if n != NO_STEP && ok[n as usize] && dist[n as usize] == u16::MAX {
                    dist[n as usize] = dc.saturating_add(1).min(u16::MAX - 1);
                    queue.push_back(n);
                }
            }
        }
        dist
    }

    /// The first direction of a shortest path from `from` to `to` (`to` itself may be any
    /// in-bounds cell); ties go to the first direction in `Dir::ALL` order.
    pub fn first_dir(&self, from: (i16, i16), to: (i16, i16), avoid_grass: bool) -> Option<Dir> {
        if !self.in_bounds(from) || !self.in_bounds(to) || from == to {
            return None;
        }
        let t = self.terrain();
        let ok = self.landable(t, avoid_grass, Some(to));
        let (start, goal) = (self.index(from) as u32, self.index(to) as u32);
        let mut seen = vec![false; ok.len()];
        seen[start as usize] = true;
        // (cell, the index in `Dir::ALL` of the path's first step).
        let mut queue: VecDeque<(u32, usize)> = VecDeque::new();
        for (d, n) in t.land[start as usize].into_iter().enumerate() {
            if n != NO_STEP && ok[n as usize] && !seen[n as usize] {
                seen[n as usize] = true;
                queue.push_back((n, d));
            }
        }
        while let Some((c, d0)) = queue.pop_front() {
            if c == goal {
                return Some(Dir::ALL[d0]);
            }
            for n in t.land[c as usize] {
                if n != NO_STEP && ok[n as usize] && !seen[n as usize] {
                    seen[n as usize] = true;
                    queue.push_back((n, d0));
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

/// [`next_dir`] for a world that is not ours (the vanilla oracle's, read from RAM): the first
/// step from `from` toward `to` on `map`, around the `occupied` cells.
pub fn first_dir_on<G: GameData>(
    game: &G,
    map: MapId,
    overrides: &[((i16, i16), u16)],
    occupied: Vec<(i16, i16)>,
    from: (i16, i16),
    to: (i16, i16),
) -> Option<Dir> {
    let grid = Grid::new(game, map, overrides, occupied);
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
