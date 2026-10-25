//! World integration tests on a small synthetic game: a town with an NPC, a sign (behind a
//! scriptless object), a coord trigger and a house door, a house with a doormat and map scripts
//! (`on_frame`, `on_resume`, `on_warp_into`), and a route connected north of the town.

use super::*;
use crate::data::*;
use crate::script::{Ctx, Script};
use crate::video::Tile;
use alloc::boxed::Box;

const FLOOR: u16 = 0;
const WALL: u16 = 1 | 1 << BLOCK_COLLISION_SHIFT;
const DOOR: u16 = 2;
const MAT: u16 = 3;
const GRASS: u16 = 4;
/// A border unlike the wall: what a crossing leaves drawn from the other map shows.
const ROUTE_BORDER: u16 = 5 | 1 << BLOCK_COLLISION_SHIFT;

static TILES: [Tile; 5] = [
    [0; 8],
    [0x1111_1111; 8],
    [0x2222_2222; 8],
    [0x3333_3333; 8],
    [0x4444_4444; 8],
];
static TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0x1234; 16]; 16],
    // Metatile k uses tile k on the bottom layer; 5 is the route's border (tile 2, blocked).
    metatiles: &[
        [0; 8],
        [1, 1, 1, 1, 0, 0, 0, 0],
        [2; 8],
        [3; 8],
        [4; 8],
        [2, 2, 2, 2, 0, 0, 0, 0],
    ],
    // Raw behavior = metatile id (5: blocked, as the wall).
    attributes: &[0, 1, 2, 3, 4, 1],
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
/// Rows 2..4 of the route are tall grass.
const fn route_blocks() -> [u16; 100] {
    let mut b = [FLOOR; 100];
    let mut i = 20;
    while i < 40 {
        b[i] = GRASS;
        i += 1;
    }
    b
}
static ROUTE_BLOCKS: [u16; 100] = route_blocks();

pub const TOWN: MapId = MapId(0);
/// The test game's `SpecialIds::max_money`, `multichoice_cancel` and `pocket_slots` (Gen3
/// Emerald's).
pub const MAX_MONEY: u32 = 999_999;
pub const MULTI_B_PRESSED: u16 = 127;
pub const POCKET_SLOTS: [u8; 5] = [30, 30, 16, 64, 46];
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
const FLAG_HIDE_HOUSE_NPC: FlagId = FlagId(0x102);
const FLAG_WARP_INTO: FlagId = FlagId(0x301);
const FLAG_DONE: FlagId = FlagId(0x302);
const VAR_HOUSE: VarId = VarId(0x4030);
const VAR_RESULT: VarId = VarId(0x800D);
/// The test game's `SpecialIds::var_daily_wilds`.
const VAR_DAILY_WILDS: VarId = VarId(0x4040);

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

/// An object without a script (talking to it falls through to the bg event behind).
const fn scriptless(local_id: u8, x: i16, y: i16) -> ObjectEvent {
    let mut o = obj(local_id, x, y, 0, None);
    o.script = None;
    o
}

const NO_SCRIPTS: MapScripts = MapScripts {
    on_load: None,
    on_transition: None,
    on_resume: None,
    on_return_to_field: None,
    on_frame: &[],
    on_warp_into: &[],
};

/// A room whose warp 0 leads to [`crate::grid::MAP_DYNAMIC`] (the truck, secret bases).
pub const DYN_ROOM: MapId = MapId(3);

static MAPS: [MapData; 4] = [
    MapData {
        name: "town",
        width: 10,
        height: 10,
        blocks: &TOWN_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        objects: &[
            obj(1, 6, 5, 1, None),
            obj(2, 8, 1, 5, Some(FLAG_HIDE_2)),
            scriptless(3, 2, 5),
        ],
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
        map_type: 0,
    },
    MapData {
        name: "house",
        width: 6,
        height: 6,
        blocks: &HOUSE_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        // Hidden by the flag `on_resume` sets: it must run before spawning.
        objects: &[obj(1, 4, 2, 1, Some(FLAG_HIDE_HOUSE_NPC))],
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
        scripts: MapScripts {
            on_resume: Some(ScriptId(12)),
            on_frame: &[VarScript {
                var: VAR_HOUSE,
                value: 0,
                script: ScriptId(10),
            }],
            on_warp_into: &[VarScript {
                var: VAR_HOUSE,
                value: 0,
                script: ScriptId(11),
            }],
            ..NO_SCRIPTS
        },
        allow_running: false,
        map_type: 0,
    },
    MapData {
        name: "route",
        width: 10,
        height: 10,
        blocks: &ROUTE_BLOCKS,
        border: [ROUTE_BORDER; 4],
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
        map_type: 0,
    },
    MapData {
        name: "dynamic room",
        width: 10,
        height: 10,
        blocks: &TOWN_BLOCKS,
        border: [WALL; 4],
        primary: &TS,
        secondary: &TS2,
        objects: &[],
        warps: &[WarpEvent {
            x: 1,
            y: 1,
            elevation: 0,
            dest_map: crate::grid::MAP_DYNAMIC,
            dest_warp: 0,
        }],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: false,
        map_type: 0,
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
/// "{PLAYER}"; every other text id is `TEXT`.
const PLAYER_TEXT: TextId = TextId(1);

pub struct Game;

const RESPAWN: u8 = 1;
const TRAINER: TrainerId = TrainerId(1);
const WILD: crate::mon::WildMon = crate::mon::WildMon {
    species: crate::battle::test_data::NORMAL,
    min_level: 2,
    max_level: 3,
};
/// Every check passes `WildEncounterCheck` (rate 255 is capped at 2880 / 2880).
static ROUTE_ENCOUNTERS: crate::mon::LandEncounters = crate::mon::LandEncounters {
    rate: 255,
    mons: [WILD; 12],
};
static TRAINER_PIC: crate::mon::TrainerPic = crate::mon::TrainerPic {
    tiles: &[],
    palette: &[0; 16],
    y: 8,
};
static TRAINER_DATA: crate::mon::TrainerData = crate::mon::TrainerData {
    class_name: &[1, 0xFF],
    name: &[2, 0xFF],
    pic: &TRAINER_PIC,
    female: false,
    ai: 0,
    double: false,
    party: &[crate::mon::TrainerMon {
        species: crate::battle::test_data::NORMAL,
        level: 2,
        iv: 0,
        moves: None,
        item: None,
    }],
};

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

/// House `on_frame` while `VAR_HOUSE == 0`: runs once, as a normal script.
async fn house_frame(ctx: Ctx) {
    ctx.set_var(VAR_HOUSE, 1).await;
}

async fn house_warp_into(ctx: Ctx) {
    ctx.set_flag(FLAG_WARP_INTO, true).await;
}

async fn house_resume(ctx: Ctx) {
    ctx.set_flag(FLAG_HIDE_HOUSE_NPC, true).await;
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
            map_type: 0,
        };
        MAPS.get(id.0 as usize).unwrap_or(&MISSING)
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_facing: Some(VAR_FACING),
            var_last_talked: Some(VAR_LAST_TALKED),
            flag_running: None,
            flags_cleared_on_map_change: &[FLAG_CLEARED],
            respawn_default: RESPAWN,
            var_wild_set: None,
            script_whiteout: None,
            max_money: MAX_MONEY,
            multichoice_cancel: MULTI_B_PRESSED,
            pocket_slots: POCKET_SLOTS,
            var_daily_wilds: Some(VAR_DAILY_WILDS),
            pc_boxes: 14,
            ..SpecialIds::default()
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
    fn text(&self, id: TextId) -> &'static [u8] {
        match id {
            PLAYER_TEXT => &[0xFD, 0x01, 0xFF], // "{PLAYER}"
            _ => &TEXT,
        }
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[1, 0xFF]
    }
    fn behavior(&self, raw: u8) -> Behavior {
        match raw as u16 {
            1 => Behavior::Blocked,
            2 => Behavior::Door,
            3 => Behavior::ArrowWarp(Dir::Down),
            4 => Behavior::TallGrass,
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
            10 => Box::pin(house_frame(ctx)),
            11 => Box::pin(house_warp_into(ctx)),
            12 => Box::pin(house_resume(ctx)),
            _ => Box::pin(async {}),
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        &crate::battle::test_data::TABLES
    }
    fn wild_encounters(&self, map: MapId, _: u16) -> Option<&'static crate::mon::LandEncounters> {
        (map == ROUTE).then_some(&ROUTE_ENCOUNTERS)
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static crate::mon::TrainerData> {
        (id == TRAINER).then_some(&TRAINER_DATA)
    }
    fn partner(&self) -> Option<&'static crate::mon::PartnerData> {
        Some(&wave_g_doubles::PARTNER)
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        (id == RESPAWN).then_some((HOUSE, 3, 3))
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

/// Spins until `CB2_InitBattle` has built the battle (the frame after the transition).
fn until_core(w: &mut World) {
    for _ in 0..400 {
        if w.battle().is_some() {
            return;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("the battle never opened");
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
    assert_eq!(f0.objs.len(), 4, "player and the three NPCs");
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
    // Gen3 DIR_EAST = 4 (DIR_SOUTH 1, DIR_NORTH 2, DIR_WEST 3).
    assert_eq!(w.var(VAR_FACING), 4);
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
    // Sign at (2, 5) behind scriptless object 3: walk left to (3, 5) and read it (Gen3
    // `GetInteractionScript` falls through to the bg event).
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
    assert!(w.flag(FLAG_WARP_INTO), "on_warp_into ran");
    assert!(w.flag(FLAG_HIDE_HOUSE_NPC), "on_resume ran");
    assert!(
        w.objects().get(1).is_none(),
        "on_resume ran before spawning"
    );
    assert_eq!(w.var(VAR_HOUSE), 0);
    run(&mut w, Buttons::NONE, 2);
    assert_eq!(w.var(VAR_HOUSE), 1, "on_frame ran once control returned");
    assert!(!w.script_running());
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

/// Gen3 `GetObjectEventIdByPosition` does not filter by map: talking to an NPC of the previous
/// map, still in view after a crossing, selects it, so `faceplayer` turns it (Task L).
#[test]
fn faceplayer_turns_a_previous_map_npc() {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    assert_eq!(w.map(), ROUTE);
    let old = w
        .objects()
        .iter()
        .find(|o| o.map == TOWN && o.local_id != PLAYER)
        .map(|o| o.local_id)
        .expect("a town NPC still in view");
    maps::set_talk_target(&mut w.s, &Game.special_ids(), Some((old, Dir::Up, TOWN)));
    let facing = |w: &World| {
        w.objects()
            .iter()
            .find(|o| o.map == TOWN && o.local_id == old)
            .unwrap()
            .facing
    };
    assert_ne!(facing(&w), Dir::Down);
    let mut host_frame = Frame::default();
    let mut runner = Runner::new();
    runner.start(Box::pin({
        let ctx = runner.ctx();
        async move { ctx.face_player().await }
    }));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut host_frame,
        immediate: false,
    };
    runner.step(&mut host);
    assert_eq!(facing(&w), Dir::Down, "the opposite of the player's facing");
    // The hash and checkpoints keep the local id only (captures happen outside scripts).
    assert_eq!(w.s.selected, Some((old, TOWN)));
}

/// The same through a real A press: the talk target found ahead is the previous map's NPC
/// (`(id, facing, map)` from the interaction), its script's `lock` waits for that NPC to
/// settle and freezes it, and `faceplayer` turns it.
#[test]
fn talking_to_a_previous_map_npc_locks_and_turns_it() {
    let mut w = started();
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    run(&mut w, Buttons::NONE, 30);
    assert_eq!(w.map(), ROUTE);
    assert!(!w.script_running());
    let town_npc = |w: &World| {
        w.objects()
            .iter()
            .find(|o| o.map == TOWN && o.local_id == 1)
            .cloned()
            .expect("the town's NPC 1 still in view")
    };
    // The town's NPC (script `npc`: lock, faceplayer, say, release) right below the player.
    let (px, py) = pos(&w);
    w.s.objects.set_pos_on(1, TOWN, px, py + 1);
    w.s.objects.face(PLAYER, Dir::Down);
    assert_ne!(town_npc(&w).facing, Dir::Up);
    run(&mut w, Buttons::A, 1);
    assert!(
        w.script_running(),
        "the A press started the town NPC's script"
    );
    assert_eq!(w.s.selected, Some((1, TOWN)));
    run(&mut w, Buttons::NONE, 10);
    let npc = town_npc(&w);
    assert!(npc.frozen, "locked");
    assert_eq!(npc.facing, Dir::Up, "faces the player");
    assert_eq!(npc.pos(), (px, py + 1));
    assert!(w.s.text.is_open(), "then says its text");
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

/// Regression: `Object.moves` hashed only the table length, so two different movement scripts
/// of the same length collided.
#[test]
fn movement_scripts_hash_their_steps() {
    static A: [Step; 2] = [Step::Walk(Dir::Up, Speed::Normal), Step::Face(Dir::Left)];
    static B: [Step; 2] = [Step::Walk(Dir::Down, Speed::Normal), Step::Face(Dir::Left)];
    static A2: [Step; 2] = [Step::Walk(Dir::Up, Speed::Normal), Step::Face(Dir::Left)];
    let with = |steps: &'static [Step]| {
        let mut w = started();
        w.s.objects.start_moves(PLAYER, steps);
        w.hash()
    };
    assert_ne!(with(&A), with(&B));
    assert_eq!(with(&A), with(&A2));
}

#[test]
fn the_player_name_is_hashed_state() {
    let mut a = started();
    let before = a.hash();
    a.set_player_name(&[1, 0xFF]);
    assert_eq!(a.hash(), before, "same name as NewGame");
    a.set_player_name(&[0xBB, 0xCD, 0xC2, 0xFF]); // "ASH"
    assert_ne!(a.hash(), before);
}

/// Messages, the starter screen and battles read the World's name, not `NewGame`'s.
#[test]
fn the_worlds_player_name_reaches_messages_the_starter_screen_and_battles() {
    const ASH: [u8; 4] = [0xBB, 0xCD, 0xC2, 0xFF];
    const PADDED: PlayerName = [0xBB, 0xCD, 0xC2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let named = || {
        let mut w = started();
        w.set_player_name(&ASH);
        w
    };

    let mut w = named();
    start(&mut w, |ctx| {
        Box::pin(async move { ctx.message(PLAYER_TEXT).await })
    });
    run(&mut w, Buttons::NONE, 1);
    assert_eq!(w.s.text.printed(), ASH);

    let mut w = named();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.choose_starter([SpeciesId(1), SpeciesId(2), SpeciesId(3)])
                .await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert_eq!(w.starter().map(|s| *s.player()), Some(PADDED));

    let mut w = named();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(crate::battle::test_data::WATER, 5).await;
            ctx.battle_wild(crate::battle::test_data::NORMAL, 3, false)
                .await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    for _ in 0..400 {
        if w.battle().is_some() {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    assert_eq!(w.battle().map(|b| *b.2.player()), Some(PADDED));
}

#[test]
fn a_wild_battle_counts_the_daily_wilds() {
    // Gen3 `IncrementDailyWildBattles`, from every wild battle's setup.
    let mut w = started();
    w.s.vars.set_var(VAR_DAILY_WILDS, 4);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(crate::battle::test_data::WATER, 5).await;
            ctx.battle_wild(crate::battle::test_data::NORMAL, 3, false)
                .await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    assert!(w.in_battle());
    assert_eq!(w.var(VAR_DAILY_WILDS), 5);
}

#[test]
fn the_log_is_not_state() {
    let mut a = started();
    let before = a.hash();
    a.s.log("x");
    assert_eq!(a.hash(), before);
}

/// Starts `script` in the world's runner, as an interaction would.
fn start(w: &mut World, script: impl FnOnce(Ctx) -> Script) {
    let ctx = w.runner.ctx();
    w.runner.start(script(ctx));
}

/// Runs frames with nothing held until the script ends (at most `max`); returns the count.
fn until_done(w: &mut World, max: u32) -> u32 {
    for n in 0..max {
        if !w.script_running() {
            return n;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("script still running after {max} frames");
}

#[test]
fn yes_no_answers_yes_no_and_b() {
    for (keys, want) in [
        (&[Buttons::A][..], 1),
        (&[Buttons::DOWN, Buttons::A][..], 0),
        (&[Buttons::B][..], 0),
    ] {
        let mut w = started();
        start(&mut w, |ctx| {
            Box::pin(async move {
                ctx.message(TextId(0)).await;
                let yes = ctx.yes_no().await;
                ctx.set_var(VAR_RESULT, yes as u16 + 10).await;
            })
        });
        run(&mut w, Buttons::NONE, 5);
        assert!(w.text().is_yes_no_active());
        for &k in keys {
            tap(&mut w, k, 2);
        }
        until_done(&mut w, 10);
        assert_eq!(w.var(VAR_RESULT), want + 10, "{keys:?}");
    }
}

#[test]
fn wait_button_waits_for_a_or_b() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.wait_button().await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    run(&mut w, Buttons::DOWN, 30);
    assert!(!w.flag(FLAG_DONE) && w.script_running());
    tap(&mut w, Buttons::B, 2);
    assert!(w.flag(FLAG_DONE) && !w.script_running());
}

#[test]
fn delay_takes_its_frames() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.delay(10).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    // Frame 1 starts the delay; it counts down on frames 2..=11.
    run(&mut w, Buttons::NONE, 10);
    assert!(!w.flag(FLAG_DONE));
    run(&mut w, Buttons::NONE, 1);
    assert!(w.flag(FLAG_DONE));
}

#[test]
fn fade_waits_until_done() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.fade(Fade::ToBlack).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    let mut levels = Vec::new();
    while !w.flag(FLAG_DONE) {
        levels.push(run(&mut w, Buttons::NONE, 1).brightness);
        assert!(levels.len() < 40);
    }
    assert_eq!(levels.last(), Some(&16));
    assert!(levels.windows(2).all(|p| p[0] <= p[1]), "{levels:?}");
}

#[test]
fn a_warp_ends_the_script() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.message(TextId(0)).await;
            ctx.warp(HOUSE, Some(0), 0, 0).await;
            ctx.wait_state().await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    run(&mut w, Buttons::NONE, 2);
    assert!(w.transition_active());
    for _ in 0..120 {
        run(&mut w, Buttons::NONE, 1);
        assert!(
            !w.flag(FLAG_DONE),
            "the script never resumes after the warp"
        );
    }
    assert_eq!(w.map(), HOUSE);
    assert!(!w.transition_active());
    assert!(
        !w.text().is_open(),
        "the dialogue window closed with the load"
    );
    assert_eq!(
        w.var(VAR_HOUSE),
        1,
        "the house on_frame script ran after the reset"
    );
}

#[test]
fn wait_state_waits_for_a_fade() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.request(crate::script::Request::Fade(Fade::ToBlack))
                .await;
            ctx.wait_state().await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    run(&mut w, Buttons::NONE, 2);
    assert!(!w.flag(FLAG_DONE));
    until_done(&mut w, 40);
    assert!(w.flag(FLAG_DONE));
}

#[test]
fn wait_move_zero_waits_for_the_player() {
    static DOWN: [Step; 1] = [Step::Walk(Dir::Down, Speed::Normal)];
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.move_obj(PLAYER, &DOWN).await;
            ctx.wait_move(0).await;
            ctx.set_flag(FLAG_DONE, true).await;
        })
    });
    run(&mut w, Buttons::NONE, 8);
    assert!(!w.flag(FLAG_DONE), "mid-step");
    until_done(&mut w, 30);
    assert!(w.flag(FLAG_DONE));
    assert_eq!(pos(&w), (5, 6));
}

#[test]
fn wait_move_names_the_last_mover_and_add_object_is_idempotent() {
    let mut w = started();
    let mut f = Frame::default();
    let mut runner = Runner::new();
    let ctx = runner.ctx();
    runner.start(Box::pin(async move {
        ctx.wait_move(1).await;
        ctx.add_object(1).await;
    }));
    let mut host = Host {
        game: &Game,
        s: &mut w.s,
        f: &mut f,
        immediate: false,
    };
    let mut steps = 0;
    while runner.step(&mut host) {
        steps += 1;
        assert!(steps < 5);
    }
    assert_eq!(w.s.last_mover, 1);
    assert_eq!(w.objects().iter().filter(|o| o.local_id == 1).count(), 1);
}

mod battles;
mod cable_car;
mod cave_encounters;
mod checkpoint;
mod contest;
mod forced_movement;
mod goldens;
// Milestone 8 Wave 1: one file per task, so no task edits another's tests.
mod m8_f;
mod m8_g;
mod m8_h;
mod m8_i;
mod m8_m;
mod mystery_gift;
// Milestone 9: one file per task.
mod decor_mart;
mod hidden_items;
mod m9_b;
mod m9_bm;
mod m9_e;
mod m9_f;
mod m9_k;
mod m9_m;
mod m9_pc;
mod m9_pt;
mod m9_r;
mod m9_s;
mod m9_sm;
mod m9_w;
mod m9_wave0;
mod naming;
mod partners;
mod pokedex_screen;
mod pushes;
mod rtc;
mod sil6_clock;
mod sky;
mod ui_pictures;
// Wave C: one file per cluster.
mod field_moves;
mod warp_exits;
mod water_moves;
mod wave_c_query;
mod wavec_field;
mod wavef_multi;
mod wavef_rotating_tiles;
// Wave G: double battles.
mod wave_g_doubles;

std::thread_local! {
    /// `World::hash` calls made on this thread (each test runs on its own thread).
    pub(super) static HASH_CALLS: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// The `World::hash` calls `f` makes.
fn hash_calls(f: impl FnOnce()) -> u64 {
    let before = HASH_CALLS.with(|n| n.get());
    f();
    HASH_CALLS.with(|n| n.get()) - before
}

// ---- Milestone 8 Wave 0 contracts ----

#[test]
fn boot_events_stay_visible_through_the_first_step() {
    let mut w = World::new(&Game);
    let h = w.hash();
    assert!(
        w.events().contains(&Event::MapEntered(TOWN)),
        "{:?}",
        w.events()
    );
    run(&mut w, Buttons::NONE, 1);
    assert!(
        w.events().contains(&Event::MapEntered(TOWN)),
        "{:?}",
        w.events()
    );
    run(&mut w, Buttons::NONE, 1);
    assert!(
        !w.events().contains(&Event::MapEntered(TOWN)),
        "cleared from step 2"
    );
    // Events are not hashed.
    let mut other = World::new(&Game);
    other.s.events.clear();
    assert_eq!(other.hash(), h);
}

#[test]
fn events_report_map_entries_scripts_and_reports_for_one_step() {
    let mut w = started();
    assert!(w.events().is_empty());
    walk(&mut w, Dir::Up, 5);
    run(&mut w, Buttons::UP, 1);
    assert_eq!(w.map(), ROUTE);
    assert!(
        w.events().contains(&Event::MapEntered(ROUTE)),
        "{:?}",
        w.events()
    );
    run(&mut w, Buttons::NONE, 1);
    assert!(
        !w.events().contains(&Event::MapEntered(ROUTE)),
        "cleared each step"
    );
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.report(Report::Unsupported("special Foo")).await;
            let picked = ctx.multichoice(0, 0, 0, 0, true).await;
            let fit = ctx.item_add(ItemId(1), 1).await;
            ctx.set_var(VAR_RESULT, picked + fit as u16).await;
        })
    });
    run(&mut w, Buttons::NONE, 1);
    // The test game has no multichoice lists or items (Task H reports them).
    assert_eq!(
        w.events(),
        &[
            Event::Report(Report::Unsupported("special Foo")),
            Event::Report(Report::Degraded("multichoice: unknown list")),
            Event::Report(Report::Degraded("additem: unknown item")),
        ]
    );
    until_done(&mut w, 4);
    assert_eq!(w.var(VAR_RESULT), MULTI_B_PRESSED);
    assert_eq!(w.events_dropped(), 0);
}

#[test]
fn the_comparison_register_is_shared_by_ctx_clones_and_starts_less() {
    let runner = Runner::new();
    let (a, b) = (runner.ctx(), runner.ctx());
    assert_eq!(a.cond(), core::cmp::Ordering::Less);
    a.set_cond(core::cmp::Ordering::Greater);
    assert_eq!(b.cond(), core::cmp::Ordering::Greater);
}
