//! Macro-action candidates of a capturable state (Task A).

use crate::config::KeyConfig;
use crate::model::{Macro, MacroKind, Opts};
use crate::path::{Grid, Reach};
use engine::checkpoint::WorldSnapshot;
use engine::data::{Behavior, BgKind, Dir, GameData, MapData};
use engine::flow::{Vars, coord_trigger, door_warp, warp_on_enter, warp_on_push};
use engine::grid::block_at;
use std::collections::BTreeSet;

/// Every applicable macro of a capturable state, deterministic order, deduplicated, default
/// `Opts`. Order: Enter, Cross, Talk, Inspect, Grass, then WalkTo (coord-event cells first,
/// then one cell per coarse block, nearest its centre).
///
/// Coarse blocks are `1 << KeyConfig::default().cell_shift` cells wide; see
/// [`candidates_with`] for another shift.
pub fn candidates<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach) -> Vec<Macro> {
    candidates_with(game, snap, reach, KeyConfig::default().cell_shift)
}

/// [`candidates`] with coarse blocks of `1 << cell_shift` cells.
pub fn candidates_with<G: GameData>(
    game: &G,
    snap: &WorldSnapshot,
    reach: &Reach,
    cell_shift: u8,
) -> Vec<Macro> {
    let st = State::new(game, snap, reach);
    let map = st.grid.map;
    let mut kinds = Vec::new();
    for w in map.warps {
        if let Some(dir) = st.enter_dir((w.x, w.y)) {
            kinds.push(MacroKind::Enter {
                x: w.x,
                y: w.y,
                dir,
            });
        }
    }
    for c in map.connections {
        if let Some((x, y)) = st.cross_cell(c.dir) {
            kinds.push(MacroKind::Cross { x, y, dir: c.dir });
        }
    }
    for o in &snap.objects.objs[1..] {
        if st.talkable(o.local_id) {
            kinds.push(MacroKind::Talk {
                local_id: o.local_id,
            });
        }
    }
    for bg in map.bgs {
        if let Some(dir) = st.inspect_dir((bg.x, bg.y)) {
            kinds.push(MacroKind::Inspect {
                x: bg.x,
                y: bg.y,
                dir,
            });
        }
    }
    if let Some((x, y)) = st.nearest_grass() {
        kinds.push(MacroKind::Grass { x, y });
    }
    let vars = Vars::from_snapshot(&snap.vars);
    for c in map.coords {
        let active = coord_trigger(map, c.x, c.y, c.elevation, &vars).is_some();
        if active && st.walk_target((c.x, c.y)) {
            kinds.push(MacroKind::WalkTo { x: c.x, y: c.y });
        }
    }
    for (x, y) in st.block_cells(cell_shift) {
        kinds.push(MacroKind::WalkTo { x, y });
    }
    let mut seen = BTreeSet::new();
    kinds
        .into_iter()
        .filter(|k| seen.insert(*k))
        .map(|kind| Macro {
            kind,
            opts: Opts::default(),
        })
        .collect()
}

/// The cells a block `WalkTo` toward `to` may be jittered to: every reachable cell of `to`'s
/// coarse block (`1 << cell_shift` cells), row-major, except the player's cell and the active
/// coord-event cells (those are `WalkTo` candidates of their own, listed first). Empty when
/// `to` itself is an active coord-event cell or is not reachable.
///
/// `candidates` keeps one cell per block, nearest its centre, so its walks between block
/// centres follow the same lines and leave the other cells unvisited (Task G as built: 105
/// of the stub's 196 cells with 4x4 blocks). `sim`'s random plans walk to a seeded cell of
/// the chosen block instead (Task H).
pub fn block_mates<G: GameData>(
    game: &G,
    snap: &WorldSnapshot,
    reach: &Reach,
    cell_shift: u8,
    to: (i16, i16),
) -> Vec<(i16, i16)> {
    let st = State::new(game, snap, reach);
    let map = st.grid.map;
    let vars = Vars::from_snapshot(&snap.vars);
    let coords: BTreeSet<(i16, i16)> = map
        .coords
        .iter()
        .filter(|c| coord_trigger(map, c.x, c.y, c.elevation, &vars).is_some())
        .map(|c| (c.x, c.y))
        .collect();
    if coords.contains(&to) || !st.walk_target(to) {
        return Vec::new();
    }
    let shift = cell_shift.min(14);
    let block = |(x, y): (i16, i16)| (x as i32 >> shift, y as i32 >> shift);
    reach
        .cells()
        .filter(|&c| block(c) == block(to) && st.walk_target(c) && !coords.contains(&c))
        .collect()
}

/// Whether `m` is still applicable to `snap` (mutations replaying a sibling's macro).
pub fn applicable<G: GameData>(game: &G, snap: &WorldSnapshot, reach: &Reach, m: &Macro) -> bool {
    let st = State::new(game, snap, reach);
    let map = st.grid.map;
    match m.kind {
        MacroKind::Settle | MacroKind::Wait { .. } => true,
        MacroKind::WalkTo { x, y } => st.walk_target((x, y)),
        MacroKind::Enter { x, y, dir } => {
            map.warps.iter().any(|w| (w.x, w.y) == (x, y))
                && st.enter_dirs((x, y)).any(|d| d == dir)
        }
        MacroKind::Cross { x, y, dir } => {
            map.connections.iter().any(|c| c.dir == dir)
                && st.on_edge((x, y), dir)
                && st.cross_ok((x, y), dir)
        }
        MacroKind::Talk { local_id } => st.talkable(local_id),
        MacroKind::Inspect { x, y, dir } => st.inspect_dirs((x, y)).any(|d| d == dir),
        MacroKind::Grass { x, y } => {
            st.reach.dist(x, y).is_some() && st.grid.behavior((x, y)).land_encounter()
        }
    }
}

/// The cell the player walks to before triggering warp `(x, y)` with `dir`: the warp cell
/// itself for an arrow warp (push `dir` while standing on it), else the cell before it.
pub fn enter_approach(behavior: Behavior, (x, y): (i16, i16), dir: Dir) -> (i16, i16) {
    if behavior == Behavior::ArrowWarp(dir) {
        (x, y)
    } else {
        let (dx, dy) = dir.delta();
        (x - dx, y - dy)
    }
}

/// The cells from which pressing A while facing `dir` reaches an object on `at`: the adjacent
/// one, and across a counter the one before it. In `Dir` order, adjacent cells first.
pub(crate) fn talk_approaches<G: GameData>(
    grid: &Grid<'_, G>,
    at: (i16, i16),
) -> Vec<((i16, i16), Dir)> {
    let mut out = Vec::new();
    for d in Dir::ALL {
        let (dx, dy) = d.delta();
        out.push(((at.0 - dx, at.1 - dy), d));
    }
    for d in Dir::ALL {
        let (dx, dy) = d.delta();
        if grid.behavior((at.0 - dx, at.1 - dy)) == Behavior::Counter {
            out.push(((at.0 - 2 * dx, at.1 - 2 * dy), d));
        }
    }
    out
}

/// The script of the visible object `local_id` of the current map, if its template has one.
pub(crate) fn object_script(
    map: &MapData,
    snap_obj: &engine::checkpoint::ObjectSnap,
    current: engine::data::MapId,
) -> bool {
    snap_obj.visible
        && snap_obj.map == current
        && map
            .objects
            .iter()
            .any(|t| t.local_id == snap_obj.local_id && t.script.is_some())
}

struct State<'a, G: GameData> {
    grid: Grid<'a, G>,
    snap: &'a WorldSnapshot,
    reach: &'a Reach,
}

impl<'a, G: GameData> State<'a, G> {
    fn new(game: &'a G, snap: &'a WorldSnapshot, reach: &'a Reach) -> Self {
        State {
            grid: Grid::of_snapshot(game, snap),
            snap,
            reach,
        }
    }

    fn player(&self) -> (i16, i16) {
        self.snap.objects.objs[0].pos
    }

    fn reachable(&self, (x, y): (i16, i16)) -> bool {
        self.reach.dist(x, y).is_some()
    }

    /// A reachable cell other than the player's.
    fn walk_target(&self, c: (i16, i16)) -> bool {
        self.reachable(c) && c != self.player()
    }

    /// Directions that trigger warp `at`, with a reachable approach cell, in `Dir` order.
    fn enter_dirs(&self, at: (i16, i16)) -> impl Iterator<Item = Dir> + '_ {
        let map = self.grid.map;
        let b = self.grid.behavior(at);
        Dir::ALL.into_iter().filter(move |&d| {
            let Some(w) = map.warps.iter().find(|w| (w.x, w.y) == at) else {
                return false;
            };
            let e = w.elevation;
            let fires = door_warp(map, at.0, at.1, e, b, d).is_some()
                || warp_on_enter(map, at.0, at.1, e, b).is_some()
                || warp_on_push(map, at.0, at.1, e, b, d).is_some();
            fires && self.reachable(enter_approach(b, at, d))
        })
    }

    fn enter_dir(&self, at: (i16, i16)) -> Option<Dir> {
        self.enter_dirs(at).next()
    }

    fn on_edge(&self, (x, y): (i16, i16), dir: Dir) -> bool {
        let (w, h) = (self.grid.map.width as i16, self.grid.map.height as i16);
        match dir {
            Dir::Up => y == 0,
            Dir::Down => y == h - 1,
            Dir::Left => x == 0,
            Dir::Right => x == w - 1,
        }
    }

    /// An edge cell from which holding `dir` crosses: reachable, and the neighbour's cell
    /// beyond is passable.
    fn cross_ok(&self, (x, y): (i16, i16), dir: Dir) -> bool {
        let (dx, dy) = dir.delta();
        let game = self.grid.game;
        let beyond = block_at(game, self.grid.map, x + dx, y + dy);
        self.reachable((x, y))
            && engine::data::block_collision(beyond) == 0
            && engine::flow::block_behavior(game, self.grid.map, beyond) != Behavior::Blocked
            && engine::grid::neighbor(game, self.grid.map, x + dx, y + dy).is_some()
    }

    /// The crossing cell of the connection(s) on edge `dir`: the usable edge cell nearest the
    /// middle of the overlap (lower coordinate on ties).
    fn cross_cell(&self, dir: Dir) -> Option<(i16, i16)> {
        let map = self.grid.map;
        let (w, h) = (map.width as i16, map.height as i16);
        let mut best: Option<(i32, (i16, i16))> = None;
        for c in map.connections.iter().filter(|c| c.dir == dir) {
            let n = self.grid.game.map(c.map);
            let (len, nlen) = match dir {
                Dir::Up | Dir::Down => (w, n.width as i16),
                Dir::Left | Dir::Right => (h, n.height as i16),
            };
            let lo = c.offset.max(0);
            let hi = (c.offset + nlen).min(len);
            for v in lo..hi {
                let cell = match dir {
                    Dir::Up => (v, 0),
                    Dir::Down => (v, h - 1),
                    Dir::Left => (0, v),
                    Dir::Right => (w - 1, v),
                };
                if !self.cross_ok(cell, dir) {
                    continue;
                }
                let off = (2 * v as i32 - (lo as i32 + hi as i32 - 1)).abs();
                if best.is_none_or(|(b, bc)| (off, cell) < (b, bc)) {
                    best = Some((off, cell));
                }
            }
        }
        best.map(|(_, c)| c)
    }

    fn talkable(&self, local_id: u8) -> bool {
        let Some(o) = self.snap.objects.objs[1..]
            .iter()
            .find(|o| o.local_id == local_id)
        else {
            return false;
        };
        object_script(self.grid.map, o, self.snap.field.map)
            && talk_approaches(&self.grid, o.pos)
                .iter()
                .any(|&(c, _)| self.reachable(c))
    }

    /// Directions to face bg script `at` from a reachable cell, in `Dir` order.
    fn inspect_dirs(&self, at: (i16, i16)) -> impl Iterator<Item = Dir> + '_ {
        let bg = self
            .grid
            .map
            .bgs
            .iter()
            .find(|b| (b.x, b.y) == at && matches!(b.kind, BgKind::Script { .. }));
        Dir::ALL.into_iter().filter(move |&d| {
            let Some(BgKind::Script { facing, .. }) = bg.map(|b| b.kind) else {
                return false;
            };
            let (dx, dy) = d.delta();
            facing.is_none_or(|f| f == d) && self.reachable((at.0 - dx, at.1 - dy))
        })
    }

    fn inspect_dir(&self, at: (i16, i16)) -> Option<Dir> {
        self.inspect_dirs(at).next()
    }

    /// The nearest reachable encounter cell (`Behavior::land_encounter`: tall grass, cave floor,
    /// deep sand; row-major first on ties).
    fn nearest_grass(&self) -> Option<(i16, i16)> {
        self.reach
            .cells()
            .filter(|&c| self.grid.behavior(c).land_encounter())
            .min_by_key(|&(x, y)| (self.reach.dist(x, y), y, x))
    }

    /// One reachable cell per coarse block (row-major blocks), nearest the block centre
    /// (row-major first on ties), the player's block excluded.
    fn block_cells(&self, shift: u8) -> Vec<(i16, i16)> {
        let size = 1i32 << shift.min(14);
        let p = self.player();
        let pb = (p.0 as i32 >> shift, p.1 as i32 >> shift);
        let mut best: std::collections::BTreeMap<(i32, i32), (i32, (i16, i16))> =
            Default::default();
        for (x, y) in self.reach.cells() {
            let b = (y as i32 >> shift, x as i32 >> shift); // (row, col): row-major order
            if (b.1, b.0) == pb {
                continue;
            }
            // Twice the offset from the centre, so it stays integer.
            let cx = 2 * x as i32 - (2 * (b.1 * size) + size - 1);
            let cy = 2 * y as i32 - (2 * (b.0 * size) + size - 1);
            let d = cx * cx + cy * cy;
            // Cells come row-major, so the first of equal distances is kept.
            if best.get(&b).is_none_or(|&(bd, _)| d < bd) {
                best.insert(b, (d, (x, y)));
            }
        }
        best.into_values().map(|(_, c)| c).collect()
    }
}
