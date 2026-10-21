//! World integration tests on a small synthetic game: a town with an NPC, a sign, a coord
//! trigger and a house door, a house with a doormat, and a route connected north of the town.

use super::*;
use crate::data::*;
use crate::script::{Ctx, Script};
use crate::video::Tile;
use alloc::boxed::Box;

const FLOOR: u16 = 0;
const WALL: u16 = 1 | 1 << BLOCK_COLLISION_SHIFT;
const DOOR: u16 = 2;
const MAT: u16 = 3;

static TILES: [Tile; 4] = [[0; 8], [0x1111_1111; 8], [0x2222_2222; 8], [0x3333_3333; 8]];
static TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0x1234; 16]; 16],
    // Metatile k uses tile k on the bottom layer.
    metatiles: &[[0; 8], [1, 1, 1, 1, 0, 0, 0, 0], [2; 8], [3; 8]],
    // Raw behavior = metatile id.
    attributes: &[0, 1, 2, 3],
};
static TS2: Tileset = Tileset {
    tiles: &[],
    palettes: &[],
    metatiles: &[],
    attributes: &[],
};

const fn town_blocks() -> [u16; 100] {
    let mut b = [FLOOR; 100];
    let mut i = 0;
    while i < 10 {
        b[90 + i] = WALL; // south wall
        b[i * 10] = WALL; // west wall
        b[i * 10 + 9] = WALL; // east wall
        i += 1;
    }
    b[2 * 10 + 3] = DOOR;
    b
}
static TOWN_BLOCKS: [u16; 100] = town_blocks();

const fn house_blocks() -> [u16; 36] {
    let mut b = [FLOOR; 36];
    b[5 * 6 + 2] = MAT;
    b
}
static HOUSE_BLOCKS: [u16; 36] = house_blocks();
static ROUTE_BLOCKS: [u16; 100] = [FLOOR; 100];

pub const TOWN: MapId = MapId(0);
pub const HOUSE: MapId = MapId(1);
pub const ROUTE: MapId = MapId(2);
const NPC_GFX: GfxId = GfxId(1);
const VAR_FACING: VarId = VarId(0x800C);
const VAR_LAST_TALKED: VarId = VarId(0x800F);
const VAR_COORD: VarId = VarId(0x4020);
const FLAG_NEW_GAME: FlagId = FlagId(0x200);
const FLAG_COORD: FlagId = FlagId(0x300);
const FLAG_HIDE_2: FlagId = FlagId(0x101);
const FLAG_CLEARED: FlagId = FlagId(0x4001);

const fn obj(local_id: u8, x: i16, y: i16, script: u16, hide: Option<FlagId>) -> ObjectEvent {
    ObjectEvent {
        local_id,
        gfx: Gfx::Fixed(NPC_GFX),
        x,
        y,
        elevation: 0,
        movement: MovementType::Face(Dir::Left),
        range: (0, 0),
        trainer_type: 0,
        trainer_sight: 0,
        script: Some(ScriptId(script)),
        hide_flag: hide,
    }
}

const NO_SCRIPTS: MapScripts = MapScripts {
    on_load: None,
    on_transition: None,
    on_resume: None,
    on_return_to_field: None,
    on_frame: &[],
    on_warp_into: &[],
};

static MAPS: [MapData; 3] = [
    MapData {
        name: "town",
        width: 10,
        height: 10,
        blocks: &TOWN_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        objects: &[obj(1, 6, 5, 1, None), obj(2, 8, 1, 5, Some(FLAG_HIDE_2))],
        warps: &[WarpEvent {
            x: 3,
            y: 2,
            elevation: 0,
            dest_map: HOUSE,
            dest_warp: 0,
        }],
        coords: &[CoordEvent {
            x: 5,
            y: 8,
            elevation: 0,
            var: VAR_COORD,
            value: 0,
            script: ScriptId(3),
        }],
        bgs: &[BgEvent {
            x: 2,
            y: 5,
            elevation: 0,
            kind: BgKind::Script {
                facing: None,
                script: ScriptId(2),
            },
        }],
        connections: &[Connection {
            dir: Dir::Up,
            offset: 0,
            map: ROUTE,
        }],
        scripts: MapScripts {
            on_transition: Some(ScriptId(4)),
            ..NO_SCRIPTS
        },
        allow_running: true,
    },
    MapData {
        name: "house",
        width: 6,
        height: 6,
        blocks: &HOUSE_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        objects: &[],
        warps: &[WarpEvent {
            x: 2,
            y: 5,
            elevation: 0,
            dest_map: TOWN,
            dest_warp: 0,
        }],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: false,
    },
    MapData {
        name: "route",
        width: 10,
        height: 10,
        blocks: &ROUTE_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        objects: &[obj(1, 2, 2, 1, None)],
        warps: &[],
        coords: &[],
        bgs: &[],
        connections: &[Connection {
            dir: Dir::Down,
            offset: 0,
            map: TOWN,
        }],
        scripts: MapScripts {
            on_transition: Some(ScriptId(6)),
            ..NO_SCRIPTS
        },
        allow_running: true,
    },
];

static SPRITE_TILES: [Tile; 8] = [[0x1111_1111; 8]; 8];
static SPRITE: SpriteData = SpriteData {
    width: 16,
    height: 32,
    frames: &[
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
        &SPRITE_TILES,
    ],
    palette: &[0x0F0F; 16],
};
static GLYPHS: [[u8; 64]; 4] = [[0x55; 64]; 4];
static FONT: Font = Font {
    glyphs: &GLYPHS,
    widths: &[6; 4],
};
static WINDOW_TILES: [Tile; 14] = [[0x1111_1111; 8]; 14];
static WINDOW: WindowGfx = WindowGfx {
    tiles: &WINDOW_TILES,
    palette: &[0x7FFF; 16],
    text_fg: 2,
    text_shadow: 3,
    text_bg: 1,
};
/// Two glyphs, a `\p` page break, one glyph.
static TEXT: [u8; 5] = [1, 2, 0xFB, 3, 0xFF];

pub struct Game;

async fn npc(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    ctx.say(TextId(0)).await;
    ctx.release().await;
}

async fn sign(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.say(TextId(0)).await;
    ctx.release_all().await;
}

async fn coord(ctx: Ctx) {
    ctx.set_var(VAR_COORD, 1).await;
    ctx.set_flag(FLAG_COORD, true).await;
}

/// Town `on_transition`: object 2 spawns at (7, 7) this visit.
async fn town_transition(ctx: Ctx) {
    ctx.set_object_template_pos(2, 7, 7).await;
}

async fn remove_me(ctx: Ctx) {
    let me = ctx.var(VAR_LAST_TALKED).await;
    ctx.remove_object(me as u8).await;
}

async fn route_transition(ctx: Ctx) {
    ctx.log("route").await;
    // A wait in a map script is answered at once.
    ctx.wait_move(1).await;
}

async fn new_game(ctx: Ctx) {
    ctx.set_flag(FLAG_NEW_GAME, true).await;
    ctx.set_flag(FLAG_CLEARED, true).await;
}

impl GameData for Game {
    fn map(&self, id: MapId) -> &'static MapData {
        static MISSING: MapData = MapData {
            name: "missing",
            width: 0,
            height: 0,
            blocks: &[],
            border: [0; 4],
            primary: &TS,
            secondary: &TS2,
            objects: &[],
            warps: &[],
            coords: &[],
            bgs: &[],
            connections: &[],
            scripts: NO_SCRIPTS,
            allow_running: false,
        };
        MAPS.get(id.0 as usize).unwrap_or(&MISSING)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_facing: Some(VAR_FACING),
            var_last_talked: Some(VAR_LAST_TALKED),
            flag_running: None,
            flags_cleared_on_map_change: &[FLAG_CLEARED],
        }
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        (gfx == NPC_GFX).then_some(&SPRITE)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        &SPRITE
    }
    fn font(&self) -> &'static Font {
        &FONT
    }
    fn window(&self) -> &'static WindowGfx {
        &WINDOW
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &TEXT
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[1, 0xFF]
    }
    fn behavior(&self, raw: u8) -> Behavior {
        match raw as u16 {
            1 => Behavior::Blocked,
            2 => Behavior::Door,
            3 => Behavior::ArrowWarp(Dir::Down),
            _ => Behavior::Normal,
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id.0 {
            1 => Box::pin(npc(ctx)),
            2 => Box::pin(sign(ctx)),
            3 => Box::pin(coord(ctx)),
            4 => Box::pin(town_transition(ctx)),
            5 => Box::pin(remove_me(ctx)),
            6 => Box::pin(route_transition(ctx)),
            9 => Box::pin(new_game(ctx)),
            _ => Box::pin(async {}),
        }
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: TOWN,
            x: 5,
            y: 5,
            facing: Dir::Down,
            script: Some(ScriptId(9)),
            player_name: &[1, 0xFF],
        }
    }
}

fn run(w: &mut World, keys: Buttons, frames: u32) -> Frame {
    let mut f = Frame::default();
    for _ in 0..frames {
        f = w.step(&Game, keys);
        f.validate().unwrap();
    }
    f
}

/// Taps `keys` for one frame, then waits `frames - 1` frames.
fn tap(w: &mut World, keys: Buttons, frames: u32) {
    run(w, keys, 1);
    run(w, Buttons::NONE, frames - 1);
}

fn pos(w: &World) -> (i16, i16) {
    w.objects().player().pos()
}

/// Holds `dir` until the player has walked `cells` cells that way and stopped, then releases
/// for a frame. Panics after 200 frames.
fn walk(w: &mut World, dir: Dir, cells: i16) {
    let keys = match dir {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    };
    let (dx, dy) = dir.delta();
    let (x, y) = pos(w);
    let target = (x + dx * cells, y + dy * cells);
    for _ in 0..200 {
        if pos(w) == target && w.objects().player().is_settled() {
            run(w, Buttons::NONE, 1);
            return;
        }
        run(w, keys, 1);
    }
    panic!("never reached {target:?}, at {:?}", pos(w));
}

fn started() -> World {
    let mut w = World::new(&Game);
    run(&mut w, Buttons::NONE, 16);
    w
}

#[test]
fn new_game_runs_its_script_loads_the_map_and_fades_in() {
    let mut w = World::new(&Game);
    assert!(w.flag(FLAG_NEW_GAME));
    assert!(!w.flag(FLAG_CLEARED), "map-change flags cleared on load");
    assert_eq!(w.map(), TOWN);
    assert_eq!(pos(&w), (5, 5));
    let f0 = w.initial_frame();
    assert_eq!(f0.brightness, 16);
    assert!(f0.cmds.iter().any(|c| matches!(c, Cmd::BgTiles { .. })));
    assert!(f0.cmds.iter().any(|c| matches!(c, Cmd::MapEntries { .. })));
    assert_eq!(f0.objs.len(), 3, "player and both NPCs");
    assert!(w.initial_frame().cmds.is_empty(), "taken once");
    let levels: Vec<u8> = (0..16)
        .map(|_| w.step(&Game, Buttons::NONE).brightness)
        .collect();
    assert_eq!(levels, (0..16).rev().collect::<Vec<u8>>());
}

use crate::video::Cmd;

#[test]
fn camera_centers_the_player() {
    let mut w = started();
    let f = run(&mut w, Buttons::NONE, 1);
    // Player cell (5, 5) at screen (112, 64): BG scroll = 5*16 - 112 and 5*16 - 64 mod 256.
    assert_eq!(f.scroll[1], ((80 - 112i32).rem_euclid(256) as u16, 16));
    let me = f
        .objs
        .iter()
        .find(|o| o.x == 112 && o.y == 64 - 16)
        .copied();
    assert!(me.is_some(), "player sprite at (112, 48): {:?}", f.objs);
}

#[test]
fn turn_then_walk() {
    let mut w = started();
    run(&mut w, Buttons::LEFT, 8);
    assert_eq!(pos(&w), (5, 5), "turning in place");
    assert_eq!(w.objects().player().facing, Dir::Left);
    run(&mut w, Buttons::LEFT, 16);
    assert_eq!(pos(&w), (4, 5));
    assert!(w.objects().player().is_settled());
}

#[test]
fn talking_to_an_npc_runs_its_script() {
    let mut w = started();
    tap(&mut w, Buttons::RIGHT, 10);
    assert_eq!(w.objects().player().facing, Dir::Right);
    tap(&mut w, Buttons::A, 4);
    assert!(w.script_running());
    assert_eq!(w.var(VAR_LAST_TALKED), 1);
    assert_eq!(w.var(VAR_FACING), Dir::Right as u16 + 1);
    assert_eq!(
        w.objects().get(1).unwrap().facing,
        Dir::Left,
        "faces the player"
    );
    assert!(w.text().is_open());
    // Prints, waits for \p, then the last glyph.
    run(&mut w, Buttons::NONE, 20);
    assert!(w.text().is_printing(), "waiting on the page break");
    tap(&mut w, Buttons::A, 10);
    assert!(!w.text().is_printing());
    assert!(w.script_running(), "waiting for a button");
    // Frozen while talking: holding a direction does nothing.
    run(&mut w, Buttons::DOWN, 20);
    assert_eq!(pos(&w), (5, 5));
    tap(&mut w, Buttons::A, 2);
    assert!(!w.script_running());
    assert!(!w.text().is_open());
    walk(&mut w, Dir::Down, 1);
    assert_eq!(pos(&w), (5, 6), "control is back");
}

#[test]
fn signs_and_coord_events() {
    let mut w = started();
    // Sign at (2, 5): walk left to (3, 5) and read it.
    walk(&mut w, Dir::Left, 2);
    tap(&mut w, Buttons::A, 4);
    assert!(w.script_running() && w.text().is_open());
    assert_eq!(w.var(VAR_LAST_TALKED), 0);
    run(&mut w, Buttons::NONE, 30);
    tap(&mut w, Buttons::A, 30);
    tap(&mut w, Buttons::A, 2);
    assert!(!w.script_running());
    // Coord event at (5, 8) while VAR_COORD == 0.
    walk(&mut w, Dir::Down, 3);
    walk(&mut w, Dir::Right, 2);
    assert_eq!(pos(&w), (5, 8));
    run(&mut w, Buttons::NONE, 2);
    assert!(w.flag(FLAG_COORD));
    assert_eq!(w.var(VAR_COORD), 1);
}

#[test]
fn template_edits_and_remove_object() {
    let mut w = started();
    let o2 = w.objects().get(2).unwrap();
    assert_eq!(o2.pos(), (7, 7), "on_transition moved the template");
    // Walk to (7, 6), face down, talk: object 2 removes itself and sets its hide flag.
    walk(&mut w, Dir::Down, 1);
    walk(&mut w, Dir::Right, 2);
    assert_eq!(pos(&w), (7, 6));
    tap(&mut w, Buttons::DOWN, 10);
    tap(&mut w, Buttons::A, 3);
    assert!(w.objects().get(2).is_none());
    assert!(w.flag(FLAG_HIDE_2));
}

#[test]
fn door_warp_and_doormat_exit() {
    let mut w = started();
    // Door at (3, 2): stand on (3, 3) and push up.
    walk(&mut w, Dir::Left, 2);
    walk(&mut w, Dir::Up, 2);
    assert_eq!(pos(&w), (3, 3));
    run(&mut w, Buttons::UP, 1);
    assert!(w.transition_active());
    let mut seen_load = false;
    for _ in 0..120 {
        run(&mut w, Buttons::NONE, 1);
        seen_load |= w.map() == HOUSE;
        if !w.transition_active() {
            break;
        }
    }
    assert!(seen_load && !w.transition_active());
    assert_eq!(w.map(), HOUSE);
    assert_eq!(pos(&w), (2, 5), "on the doormat, no exit walk");
    assert_eq!(w.objects().player().facing, Dir::Up);
    // Push down on the doormat: back to the town door, then walk out one cell.
    run(&mut w, Buttons::DOWN, 10);
    assert!(w.transition_active());
    run(&mut w, Buttons::NONE, 120);
    assert!(!w.transition_active());
    assert_eq!(w.map(), TOWN);
    assert_eq!(pos(&w), (3, 3), "walked out of the door");
    assert!(w.objects().player().visible);
}

#[test]
fn crossing_a_connection_switches_maps() {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    assert_eq!((w.map(), pos(&w)), (TOWN, (5, 0)));
    let log_before = w.log().count();
    run(&mut w, Buttons::UP, 1);
    assert_eq!(w.map(), ROUTE, "switched as the step starts");
    assert_eq!(pos(&w), (5, 9));
    let lines: Vec<&str> = w.log().lines().collect();
    assert!(lines.contains(&"route"), "{lines:?}");
    assert!(
        w.log().count() >= log_before + 2,
        "wait in map script logged"
    );
    // The town's NPCs stay until they leave the view; the route's spawned.
    assert!(w.objects().get(1).is_some_and(|o| o.map == ROUTE));
    run(&mut w, Buttons::NONE, 16);
    assert!(w.objects().player().is_settled());
    run(&mut w, Buttons::DOWN, 8);
    assert_eq!(w.map(), ROUTE, "turning");
    run(&mut w, Buttons::DOWN, 1);
    assert_eq!((w.map(), pos(&w)), (TOWN, (5, 0)), "and back");
}

#[test]
fn missing_map_warps_are_ignored() {
    let mut w = started();
    let mut host_frame = Frame::default();
    let mut runner = Runner::new();
    runner.start(Box::pin({
        let ctx = runner.ctx();
        async move { ctx.warp(MapId(77), None, 1, 1).await }
    }));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut host_frame,
        immediate: false,
    };
    assert!(!runner.step(&mut host));
    assert!(!w.transition_active());
    assert_eq!(
        w.log().lines().last(),
        Some("warp to a missing map ignored")
    );
}

#[test]
fn same_inputs_same_hash_and_different_inputs_differ() {
    let inputs = [
        Buttons::RIGHT,
        Buttons::NONE,
        Buttons::A,
        Buttons::DOWN,
        Buttons::LEFT,
    ];
    let (mut a, mut b) = (World::new(&Game), World::new(&Game));
    for k in inputs {
        for _ in 0..20 {
            a.step(&Game, k);
            b.step(&Game, k);
            assert_eq!(a.hash(), b.hash());
        }
    }
    let (mut c, mut d) = (World::new(&Game), World::new(&Game));
    c.step(&Game, Buttons::LEFT);
    d.step(&Game, Buttons::RIGHT);
    assert_ne!(c.hash(), d.hash());
}

/// Golden values pin the GBA/native parity contract on this synthetic game: any change to the
/// hashed state, its order or the frame logic shows up here. Update them (and the note in
/// `docs/`) only for an intended change.
#[test]
fn golden_hashes() {
    let mut w = World::new(&Game);
    let h0 = w.hash();
    for k in [Buttons::RIGHT, Buttons::A, Buttons::NONE, Buttons::DOWN] {
        run(&mut w, k, 25);
    }
    let h1 = w.hash();
    assert_eq!(
        (h0, h1),
        (0x2737_8ceb_8a11_0230, 0x4b5b_c94f_9ed5_70e4),
        "{h0:#018x} {h1:#018x}"
    );
}

#[test]
fn the_log_is_not_state() {
    let mut a = started();
    let before = a.hash();
    a.s.log("x");
    assert_eq!(a.hash(), before);
}
