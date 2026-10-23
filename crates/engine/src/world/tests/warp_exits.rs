//! World tests of the warp exits per arrival behavior (Gen3 `GetAdjustedInitialDirection`,
//! `SetUpWarpExitTask`): a forest whose north exit leads onto a route's deep south warp, which
//! has a wall south of it (Route 104's Petalburg Woods exit), and a ladder.

use super::*;

/// Raw behaviors of this game's tileset (metatile k has raw behavior k).
const STAIRS: u16 = 2;
const DEEP_SOUTH: u16 = 3;
const LADDER: u16 = 4;

static EXITS_TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0x1234; 16]; 16],
    metatiles: &[[0; 8], [1, 1, 1, 1, 0, 0, 0, 0], [2; 8], [3; 8], [4; 8]],
    attributes: &[0, 1, 2, 3, 4],
};

const WOODS: MapId = MapId(0);
const ROUTE_N: MapId = MapId(1);

/// 5x5 floor; (2, 0) is the north exit (stairs-like), (0, 4) a ladder.
const fn woods_blocks() -> [u16; 25] {
    let mut b = [FLOOR; 25];
    b[2] = STAIRS;
    b[4 * 5] = LADDER;
    b
}
static WOODS_BLOCKS: [u16; 25] = woods_blocks();

/// 5x6: floor on rows 0..=3, the deep south warp on (2, 3), a wall row 4 under it, floor on
/// row 5; a ladder on (4, 0).
const fn route_blocks() -> [u16; 30] {
    let mut b = [FLOOR; 30];
    b[3 * 5 + 2] = DEEP_SOUTH;
    let mut i = 0;
    while i < 5 {
        b[4 * 5 + i] = WALL;
        i += 1;
    }
    b[4] = LADDER;
    b
}
static ROUTE_N_BLOCKS: [u16; 30] = route_blocks();

const fn warp(x: i16, y: i16, dest_map: MapId, dest_warp: u8) -> WarpEvent {
    WarpEvent {
        x,
        y,
        elevation: 0,
        dest_map,
        dest_warp,
    }
}

static EXIT_MAPS: [MapData; 2] = [
    MapData {
        name: "woods",
        width: 5,
        height: 5,
        blocks: &WOODS_BLOCKS,
        border: [WALL; 4],
        primary: &EXITS_TS,
        secondary: &TS2,
        objects: &[],
        warps: &[warp(2, 0, ROUTE_N, 0), warp(0, 4, ROUTE_N, 1)],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: false,
    },
    MapData {
        name: "route north",
        width: 5,
        height: 6,
        blocks: &ROUTE_N_BLOCKS,
        border: [WALL; 4],
        primary: &EXITS_TS,
        secondary: &TS2,
        objects: &[],
        warps: &[warp(2, 3, WOODS, 0), warp(4, 0, WOODS, 1)],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: false,
    },
];

/// The test [`Game`]'s graphics and text, with [`EXIT_MAPS`] and content-like behaviors.
struct Exits;

impl GameData for Exits {
    fn map(&self, id: MapId) -> &'static MapData {
        &EXIT_MAPS[id.0 as usize]
    }
    fn has_map(&self, id: MapId) -> bool {
        (id.0 as usize) < EXIT_MAPS.len()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        match raw as u16 {
            1 => Behavior::Blocked,
            STAIRS => Behavior::NonAnimDoor(Dir::Down),
            DEEP_SOUTH => Behavior::NonAnimDoor(Dir::Up),
            LADDER => Behavior::Ladder,
            _ => Behavior::Normal,
        }
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: WOODS,
            x: 2,
            y: 2,
            facing: Dir::Up,
            script: None,
            player_name: &[1, 0xFF],
        }
    }
}

fn step(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(&Exits, keys).validate().unwrap();
    }
}

/// Holds `keys` until the player stands still on `to` with control (no transition), then
/// releases for a frame. Panics after 400 frames.
fn go(w: &mut World, keys: Buttons, to: (MapId, (i16, i16))) {
    for _ in 0..400 {
        if (w.map(), pos(w)) == to && w.objects().player().is_settled() && !w.transition_active() {
            step(w, Buttons::NONE, 1);
            return;
        }
        step(w, keys, 1);
    }
    panic!("never reached {to:?}, at {:?} {:?}", w.map(), pos(w));
}

fn exits_started() -> World {
    let mut w = World::new(&Exits);
    step(&mut w, Buttons::NONE, 20);
    assert_eq!((w.map(), pos(&w)), (WOODS, (2, 2)));
    w
}

/// Gen3 `MetatileBehavior_IsDeepSouthWarp`: walking north out of the woods lands on the
/// route's deep south warp facing north, and the exit walk goes north (`Task_ExitNonAnimDoor`
/// walks the way the player faces), not south into the wall.
#[test]
fn a_deep_south_warp_arrival_faces_and_walks_north() {
    let mut w = exits_started();
    go(&mut w, Buttons::UP, (ROUTE_N, (2, 2)));
    assert_eq!(w.objects().player().facing, Dir::Up);
    // And the player has control to go on north.
    go(&mut w, Buttons::UP, (ROUTE_N, (2, 1)));
    // Back south onto the deep south warp: into the woods' stairs-like exit, out south.
    go(&mut w, Buttons::DOWN, (WOODS, (2, 1)));
    assert_eq!(w.objects().player().facing, Dir::Down);
}

/// Gen3 `GetAdjustedInitialDirection`: a ladder arrival keeps the facing the player warped
/// with, and does not walk out.
#[test]
fn a_ladder_arrival_keeps_the_facing_without_walking() {
    let mut w = exits_started();
    go(&mut w, Buttons::DOWN, (WOODS, (2, 4)));
    go(&mut w, Buttons::LEFT, (WOODS, (1, 4)));
    // Stepping left onto the ladder warps to the route's ladder, still facing left, in place.
    go(&mut w, Buttons::LEFT, (ROUTE_N, (4, 0)));
    assert_eq!(w.objects().player().facing, Dir::Left);
}
