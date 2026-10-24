//! Planner rules on small synthetic maps.

use super::*;
use content::Emerald;
use engine::data::{
    BLOCK_COLLISION_SHIFT, Font, GfxId, MapScripts, NewGame, ScriptId, SpriteData, TextId, Tileset,
    UiText, WindowGfx,
};
use engine::script::{Ctx, Script};

const NORMAL: u16 = 0;
const BLOCKED: u16 = 1;
const DOOR: u16 = 2;
const WARP: u16 = 3;
const LEDGE_DOWN: u16 = 4;
const GRASS: u16 = 5;
const ARROW_DOWN: u16 = 6;
/// A cell walled on its east side (Gen3 `MB_IMPASSABLE_EAST`).
const WALL_EAST: u16 = 7;
const WALL: u16 = 1 << BLOCK_COLLISION_SHIFT;

/// A synthetic map: metatile `i` has raw behavior `i` (classified below); the rest is the
/// content build's.
struct Synth(&'static MapData);

fn synth(width: u16, rows: &[&[u16]]) -> Synth {
    let blocks: Vec<u16> = rows.iter().flat_map(|r| r.iter().copied()).collect();
    // A warp event on every door and warp cell (a warp behaviour without one is a floor).
    let warps: Vec<engine::data::WarpEvent> = (blocks.iter().enumerate())
        .filter(|(_, b)| matches!(**b, DOOR | WARP))
        .map(|(i, _)| engine::data::WarpEvent {
            x: (i % width as usize) as i16,
            y: (i / width as usize) as i16,
            elevation: 0,
            dest_map: MapId(0),
            dest_warp: 0,
        })
        .collect();
    let ts: &'static Tileset = Box::leak(Box::new(Tileset {
        tiles: &[],
        palettes: &[],
        metatiles: &[],
        attributes: &[0, 1, 2, 3, 4, 5, 6, 7],
    }));
    Synth(Box::leak(Box::new(MapData {
        name: "Synth",
        width,
        height: rows.len() as u16,
        blocks: Box::leak(blocks.into_boxed_slice()),
        border: [0; 4],
        primary: ts,
        secondary: ts,
        objects: &[],
        warps: Box::leak(warps.into_boxed_slice()),
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: MapScripts::default(),
        allow_running: false,
        map_type: 0,
    })))
}

impl GameData for Synth {
    fn map(&self, _: MapId) -> &'static MapData {
        self.0
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Emerald::for_shells().sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Emerald::for_shells().player_sprite()
    }
    fn font(&self) -> &'static Font {
        Emerald::for_shells().font()
    }
    fn window(&self) -> &'static WindowGfx {
        Emerald::for_shells().window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Emerald::for_shells().text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Emerald::for_shells().ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        match u16::from(raw) {
            BLOCKED => Behavior::Blocked,
            DOOR => Behavior::Door,
            WARP => Behavior::Warp,
            LEDGE_DOWN => Behavior::Ledge(Dir::Down),
            GRASS => Behavior::TallGrass,
            ARROW_DOWN => Behavior::ArrowWarp(Dir::Down),
            WALL_EAST => Behavior::Impassable(engine::data::Sides::of(&[Dir::Right])),
            _ => Behavior::Normal,
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        Emerald::for_shells().script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        Emerald::for_shells().new_game()
    }
}

fn dist(g: &Grid<'_, Synth>, from: (i16, i16), c: (i16, i16)) -> Option<u16> {
    let d = g.distances(from, false)[c.1 as usize * g.map.width as usize + c.0 as usize];
    (d != u16::MAX).then_some(d)
}

#[test]
fn ledges_are_one_way() {
    let (n, l) = (NORMAL, LEDGE_DOWN);
    let game = synth(3, &[&[n, WALL, n], &[l, WALL, n], &[n, n, n]]);
    let g = Grid::new(&game, MapId(0), &[], Vec::new());
    // Down over the ledge: two cells in one jump.
    assert_eq!(dist(&g, (0, 0), (0, 2)), Some(1));
    assert_eq!(g.first_dir((0, 0), (0, 2), false), Some(Dir::Down));
    assert_eq!(dist(&g, (0, 0), (0, 1)), None, "never stood on");
    // Back up only the long way round.
    assert_eq!(dist(&g, (0, 2), (0, 0)), None);
    assert_eq!(dist(&g, (0, 2), (2, 0)), Some(4));
}

/// A directional wall blocks crossing its side both ways (the walled cell's side, or the
/// neighbour's opposite side) and nothing else: the walled cell is entered from the other sides.
#[test]
fn walls_block_crossing_their_side_only() {
    let (n, e) = (NORMAL, WALL_EAST);
    let game = synth(2, &[&[n, n], &[e, n], &[n, n]]);
    let g = Grid::new(&game, MapId(0), &[], Vec::new());
    // (0, 1) -> (1, 1) crosses the wall: round by row 0 or row 2.
    assert_eq!(dist(&g, (0, 1), (1, 1)), Some(3));
    assert_ne!(g.first_dir((0, 1), (1, 1), false), Some(Dir::Right));
    // Into the walled cell from the east: the neighbour's west side is the same wall.
    assert_eq!(dist(&g, (1, 1), (0, 1)), Some(3));
    // From above and below it is open.
    assert_eq!(dist(&g, (0, 0), (0, 1)), Some(1));
    assert_eq!(dist(&g, (0, 2), (0, 1)), Some(1));
}

#[test]
fn doors_and_warps_are_targets_not_passages() {
    let n = NORMAL;
    let game = synth(5, &[&[n, DOOR, n, WARP, n]]);
    let g = Grid::new(&game, MapId(0), &[], Vec::new());
    assert_eq!(dist(&g, (0, 0), (2, 0)), None);
    assert_eq!(dist(&g, (0, 0), (1, 0)), None);
    // As the walk's target, the door is entered.
    assert_eq!(g.first_dir((0, 0), (1, 0), false), Some(Dir::Right));
    assert_eq!(g.first_dir((0, 0), (2, 0), false), None);
    assert_eq!(g.first_dir((2, 0), (3, 0), false), Some(Dir::Right));
}

#[test]
fn overrides_and_objects() {
    let n = NORMAL;
    let game = synth(3, &[&[n, n, n]]);
    let open = Grid::new(&game, MapId(0), &[], Vec::new());
    assert_eq!(dist(&open, (0, 0), (2, 0)), Some(2));
    // A `setmetatile` wall in the middle.
    let ov = [((1, 0), WALL)];
    let walled = Grid::new(&game, MapId(0), &ov, Vec::new());
    assert_eq!(walled.block((1, 0)), WALL);
    assert_eq!(dist(&walled, (0, 0), (2, 0)), None);
    // An override can open a cell too.
    let game = synth(3, &[&[n, BLOCKED, n]]);
    let ov = [((1, 0), NORMAL)];
    let opened = Grid::new(&game, MapId(0), &ov, Vec::new());
    assert_eq!(dist(&opened, (0, 0), (2, 0)), Some(2));
    // An object's cell (pos or prev) is occupied.
    let game = synth(3, &[&[n, n, n]]);
    let busy = Grid::new(&game, MapId(0), &[], vec![(1, 0)]);
    assert!(busy.occupied((1, 0)) && !busy.walkable((1, 0), false));
    assert_eq!(dist(&busy, (0, 0), (2, 0)), None);
    // Still enterable as the walk's target.
    assert_eq!(busy.first_dir((0, 0), (1, 0), false), Some(Dir::Right));
}

#[test]
fn arrow_warps_are_never_left_toward_their_arrow() {
    let n = NORMAL;
    let game = synth(2, &[&[n, n], &[ARROW_DOWN, n], &[n, n]]);
    let g = Grid::new(&game, MapId(0), &[], Vec::new());
    assert_eq!(dist(&g, (0, 0), (0, 1)), Some(1), "walkable");
    // Not straight down through it: around it.
    assert_eq!(dist(&g, (0, 0), (0, 2)), Some(4));
    assert_eq!(g.first_dir((0, 1), (0, 2), false), Some(Dir::Right));
}

#[test]
fn tall_grass_only_when_needed() {
    let n = NORMAL;
    let game = synth(3, &[&[n, GRASS, n], &[n, n, n]]);
    let g = Grid::new(&game, MapId(0), &[], Vec::new());
    assert_eq!(g.first_dir((0, 0), (2, 0), true), Some(Dir::Down));
    assert_eq!(g.first_dir((0, 0), (2, 0), false), Some(Dir::Right));
}

/// The planner's step rule as written before the terrain table (a block classified at every
/// edge): the reference [`Terrain`] answers like.
fn ref_step<G: GameData>(
    g: &Grid<'_, G>,
    (c, e): ((i16, i16), u8),
    d: Dir,
    avoid_grass: bool,
    target: Option<(i16, i16)>,
) -> Option<((i16, i16), u8)> {
    if g.behavior(c) == Behavior::ArrowWarp(d) {
        return None;
    }
    let (dx, dy) = d.delta();
    let mut n = (c.0 + dx, c.1 + dy);
    if !g.in_bounds(n) || Behavior::walled(g.behavior(c), g.behavior(n), d) {
        return None;
    }
    if let Behavior::Ledge(ld) = g.behavior(n) {
        if ld != d {
            return None;
        }
        n = (n.0 + dx, n.1 + dy);
    }
    if !g.in_bounds(n) {
        return None;
    }
    let (ec, en) = (block_elevation(g.block(c)), block_elevation(g.block(n)));
    let water = |c| matches!(g.behavior(c), Behavior::Water(_));
    let off_the_water = en == 3 && water(c) && !water(n);
    if (Some(n) != target || g.walkable(n, false))
        && e != 0
        && en != 0
        && en != 15
        && en != e
        && !off_the_water
    {
        return None;
    }
    let e2 = if ec == 15 || en == 15 { e } else { en };
    (Some(n) == target || g.walkable(n, avoid_grass)).then_some((n, e2))
}

/// The elevation a reference search starts with: the start cell's (0 on a 15).
fn ref_start<G: GameData>(g: &Grid<'_, G>, from: (i16, i16)) -> u8 {
    match block_elevation(g.block(from)) {
        15 => 0,
        e => e,
    }
}

/// `Grid::distances` before the terrain table.
fn ref_distances<G: GameData>(g: &Grid<'_, G>, from: (i16, i16), avoid_grass: bool) -> Vec<u16> {
    let (w, h) = (g.map.width as usize, g.map.height as usize);
    let mut dist = vec![u16::MAX; w * h];
    if !g.in_bounds(from) {
        return dist;
    }
    let idx = |(x, y): (i16, i16)| y as usize * w + x as usize;
    let mut seen = std::collections::HashSet::new();
    let start = (from, ref_start(g, from));
    seen.insert(start);
    dist[idx(from)] = 0;
    let mut queue = VecDeque::from([(start, 0u16)]);
    while let Some((c, dc)) = queue.pop_front() {
        for d in Dir::ALL {
            if let Some(n) = ref_step(g, c, d, avoid_grass, None)
                && seen.insert(n)
            {
                let dn = dc.saturating_add(1).min(u16::MAX - 1);
                dist[idx(n.0)] = dist[idx(n.0)].min(dn);
                queue.push_back((n, dn));
            }
        }
    }
    dist
}

/// `Grid::first_dir` before the terrain table.
fn ref_first_dir<G: GameData>(
    g: &Grid<'_, G>,
    from: (i16, i16),
    to: (i16, i16),
    avoid_grass: bool,
) -> Option<Dir> {
    if !g.in_bounds(from) || !g.in_bounds(to) || from == to {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    let start = (from, ref_start(g, from));
    seen.insert(start);
    let mut queue = VecDeque::from([(start, None::<Dir>)]);
    while let Some((c, d0)) = queue.pop_front() {
        if c.0 == to {
            return d0;
        }
        for d in Dir::ALL {
            if let Some(n) = ref_step(g, c, d, avoid_grass, Some(to))
                && seen.insert(n)
            {
                queue.push_back((n, Some(d0.unwrap_or(d))));
            }
        }
    }
    None
}

/// The terrain table's searches answer as the per-edge rules on `map`: the distance maps and
/// the first directions (to every cell, or about 64 of them on a larger map) from every cell (or
/// about 6), both grass settings, with `occupied` and `overrides`.
fn assert_same_answers<G: GameData>(
    game: &G,
    map: MapId,
    overrides: &[((i16, i16), u16)],
    occupied: Vec<(i16, i16)>,
) {
    let fast = Grid::new(game, map, overrides, occupied.clone());
    let slow = Grid::new(game, map, overrides, occupied);
    let (w, h) = (fast.map.width as usize, fast.map.height as usize);
    let cells: Vec<(i16, i16)> = (0..w * h)
        .map(|i| ((i % w) as i16, (i / w) as i16))
        .collect();
    let sample = |n: usize| cells.iter().copied().step_by((cells.len() / n).max(1));
    for from in sample(6) {
        for grass in [false, true] {
            assert_eq!(
                fast.distances(from, grass),
                ref_distances(&slow, from, grass),
                "{} from {from:?}",
                fast.map.name
            );
            for to in sample(64) {
                assert_eq!(
                    fast.first_dir(from, to, grass),
                    ref_first_dir(&slow, from, to, grass),
                    "{} {from:?} -> {to:?} grass {grass}",
                    fast.map.name
                );
            }
        }
    }
    assert!(
        slow.terrain.get().is_none(),
        "the reference never built a table"
    );
}

#[test]
fn the_terrain_table_answers_as_the_per_edge_rules_on_synthetic_maps() {
    let (n, b, g, l) = (NORMAL, BLOCKED, GRASS, LEDGE_DOWN);
    let game = synth(
        6,
        &[
            &[n, n, g, g, n, DOOR],
            &[l, WALL, g, ARROW_DOWN, n, n],
            &[n, WALL_EAST, n, b, WARP, n],
            &[n, n, l, n, g, n],
            &[g, n, n, WALL, n, n],
        ],
    );
    // Every pair of cells (30 cells: the samples take them all).
    for c in 0..30 {
        let from = ((c % 6) as i16, (c / 6) as i16);
        for to in (0..30).map(|i| ((i % 6) as i16, (i / 6) as i16)) {
            let fast = Grid::new(&game, MapId(0), &[], vec![(1, 3)]);
            let slow = Grid::new(&game, MapId(0), &[], vec![(1, 3)]);
            for grass in [false, true] {
                let want = ref_first_dir(&slow, from, to, grass);
                assert_eq!(fast.first_dir(from, to, grass), want, "{from:?} -> {to:?}");
            }
        }
    }
    assert_same_answers(&game, MapId(0), &[], Vec::new());
    assert_same_answers(
        &game,
        MapId(0),
        &[((3, 2), NORMAL), ((4, 0), WALL)],
        Vec::new(),
    );
    assert_same_answers(&game, MapId(0), &[], vec![(1, 3), (4, 3), (5, 1)]);
}

/// The build's sample maps (`content::samples`; every map of the stub), with objects and an
/// override.
#[test]
fn the_terrain_table_answers_as_the_per_edge_rules_on_the_game_maps() {
    let game = Emerald::for_shells();
    for map in content::samples::map_ids() {
        let data = game.map(map);
        let (w, h) = (data.width as i16, data.height as i16);
        let occupied = vec![(w / 2, h / 2), (w / 3, h / 4)];
        let overrides = [((w / 4, h / 3), 0u16)];
        assert_same_answers(&game, map, &overrides, occupied);
    }
}

#[test]
#[cfg(not(pkr_silice))]
fn a_surfer_gets_off_the_water_onto_elevation_3_land() {
    // Route 131's Sky Pillar islet (elevation 3) in its water (elevation 1), no shore between
    // (Gen3 `CanStopSurfing`). Emerald only (Silice has no `MAP_ROUTE131`).
    if content::FLAVOR != content::Flavor::Emerald {
        return;
    }
    let game = Emerald::for_shells();
    let route131 = content::consts::MAP_ROUTE131;
    // `LAYOUT_ROUTE131_SKY_PILLAR` (`gMapLayouts` number 0x13F), which the islet is on.
    let g = Grid::new(&game, route131, &[], Vec::new()).with_layout(route131, Some(0x13F));
    assert_eq!(g.first_dir((36, 9), (36, 7), false), Some(Dir::Up));
    // Not the other way: getting on the water takes Surf.
    let w = game.map(route131).width as usize;
    assert_eq!(g.distances((36, 8), false)[9 * w + 36], u16::MAX);
}
