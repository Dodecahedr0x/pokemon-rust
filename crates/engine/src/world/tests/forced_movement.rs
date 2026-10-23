//! Forced movement and the per-step puzzles on synthetic maps (Gen3 `ForcedMovement_*`,
//! `SootopolisGymIcePerStepCallback`, `CrackedFloorPerStepCallback`, `EventScript_FallDownHole`):
//! slide distances, stops on obstacles, arrow chains, slide arrows and muddy slopes keeping the
//! facing, the hold after a warp load, thin ice cracking, and a cracked floor turning into a hole
//! that drops the player to the map below. Step lengths are Gen3's: `walk_normal` 16 frames,
//! `walk_fast` 8.

use super::*;
use crate::checkpoint::Checkpoint;
use crate::world::puzzle::{FLOOR_BREAKS, ICE_CRACKS, NOTICED};

// Metatile k has raw behavior k (`Rink::behavior`).
const FL: u16 = 0;
const WL: u16 = 1 | 1 << BLOCK_COLLISION_SHIFT;
const ICE: u16 = 2;
const WE: u16 = 3;
const WS: u16 = 4;
const SN: u16 = 5;
const MUD: u16 = 6;
const THIN: u16 = 7;
const CRACKED: u16 = 8;
const BROKEN: u16 = 9;
const CFLOOR: u16 = 10;
const HOLE: u16 = 11;

static RINK_TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0x1234; 16]; 16],
    metatiles: &[[0; 8]; 12],
    attributes: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
};

const RINK: MapId = MapId(0);
const GYM: MapId = MapId(1);
const CAVE: MapId = MapId(2);
const BELOW: MapId = MapId(3);

const VAR_ICE: VarId = VarId(0x4021);
const VAR_ROW: VarId = VarId(0x4022);

/// 10x8, walls around:
/// - row 1: ice (2..=5, 1) between floors (1, 1) and (6, 1);
/// - row 2: ice (2..=4, 2) up to a wall (5, 2);
/// - row 3: walk arrows east (2, 3), (3, 3) and south (4, 3);
/// - row 4: a slide arrow north (5, 4) and a muddy slope (7, 4);
/// - row 6: ice (2..=8, 6) to the east wall.
#[rustfmt::skip]
static RINK_BLOCKS: [u16; 80] = [
    WL, WL,  WL,  WL,  WL,  WL,  WL,  WL,  WL,  WL,
    WL, FL,  ICE, ICE, ICE, ICE, FL,  WL,  FL,  WL,
    WL, FL,  ICE, ICE, ICE, WL,  FL,  FL,  FL,  WL,
    WL, FL,  WE,  WE,  WS,  FL,  FL,  FL,  FL,  WL,
    WL, FL,  FL,  FL,  FL,  SN,  FL,  MUD, FL,  WL,
    WL, FL,  FL,  FL,  FL,  FL,  FL,  FL,  FL,  WL,
    WL, FL,  ICE, ICE, ICE, ICE, ICE, ICE, ICE, WL,
    WL, WL,  WL,  WL,  WL,  WL,  WL,  WL,  WL,  WL,
];

/// 6x4: thin ice (2, 1), (3, 1), cracked ice (4, 1).
#[rustfmt::skip]
static GYM_BLOCKS: [u16; 24] = [
    WL, WL, WL,   WL,   WL,      WL,
    WL, FL, THIN, THIN, CRACKED, WL,
    WL, FL, FL,   FL,   FL,      WL,
    WL, WL, WL,   WL,   WL,      WL,
];

/// 6x4: a cracked floor (2, 1) and a hole (3, 2).
#[rustfmt::skip]
static CAVE_BLOCKS: [u16; 24] = [
    WL, WL, WL,     WL,   WL, WL,
    WL, FL, CFLOOR, FL,   FL, WL,
    WL, FL, FL,     HOLE, FL, WL,
    WL, WL, WL,     WL,   WL, WL,
];

static BELOW_BLOCKS: [u16; 24] = [FL; 24];

const fn map(name: &'static str, w: u16, h: u16, blocks: &'static [u16]) -> MapData {
    MapData {
        name,
        width: w,
        height: h,
        blocks,
        border: [WL; 4],
        primary: &RINK_TS,
        secondary: &TS2,
        objects: &[],
        warps: &[],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: true,
    }
}

static RINK_MAPS: [MapData; 4] = [
    map("rink", 10, 8, &RINK_BLOCKS),
    map("gym", 6, 4, &GYM_BLOCKS),
    map("cave", 6, 4, &CAVE_BLOCKS),
    map("below", 6, 4, &BELOW_BLOCKS),
];

static GYM_PUZZLE: IcePuzzle = IcePuzzle {
    steps: Some(VAR_ICE),
    cracked: CRACKED,
    broken: BROKEN,
    rows: Some(IceRows {
        left: 2,
        top: 1,
        width: 3,
        vars: &[Some(VAR_ROW)],
    }),
};

static CAVE_FLOOR: CrackedFloor = CrackedFloor {
    steps: Some(VAR_ICE),
    holes: &[(CFLOOR, HOLE)],
    hole: HOLE,
};

/// `EventScript_FallDownHole`, shortened: lock, then `warphole MAP_UNDEFINED`.
async fn fall(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.delay(20).await;
    ctx.warp_hole(None).await;
}

const FALL: ScriptId = ScriptId(1);

/// The test [`Game`]'s graphics, with the maps above, running allowed, and the puzzles.
struct Rink;

impl GameData for Rink {
    fn map(&self, id: MapId) -> &'static MapData {
        &RINK_MAPS[id.0 as usize]
    }
    fn has_map(&self, id: MapId) -> bool {
        (id.0 as usize) < RINK_MAPS.len()
    }
    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            flag_running: Some(FLAG_RUN),
            ..SpecialIds::default()
        }
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
            ICE => Behavior::Ice,
            WE => Behavior::ForcedWalk(Dir::Right),
            WS => Behavior::ForcedWalk(Dir::Down),
            SN => Behavior::Slide(Dir::Up),
            MUD => Behavior::MuddySlope,
            THIN => Behavior::ThinIce,
            CRACKED => Behavior::CrackedIce,
            CFLOOR => Behavior::CrackedFloor,
            HOLE => Behavior::CrackedFloorHole,
            _ => Behavior::Normal,
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id {
            FALL => Box::pin(fall(ctx)),
            _ => Box::pin(async {}),
        }
    }
    fn new_game(&self) -> NewGame {
        // On the first east arrow: held until the player moves.
        NewGame {
            map: RINK,
            x: 2,
            y: 3,
            facing: Dir::Down,
            script: None,
            player_name: &[1, 0xFF],
        }
    }
    fn step_callback(&self, map: MapId) -> Option<StepCallback> {
        match map {
            GYM => Some(StepCallback::Ice(&GYM_PUZZLE)),
            CAVE => Some(StepCallback::CrackedFloor(&CAVE_FLOOR)),
            _ => None,
        }
    }
    fn hole_warp(&self, map: MapId) -> Option<MapId> {
        (map == CAVE).then_some(BELOW)
    }
    fn fall_script(&self) -> Option<ScriptId> {
        Some(FALL)
    }
}

const FLAG_RUN: FlagId = FlagId(0x860);

fn step(w: &mut World, keys: Buttons) {
    w.step(&Rink, keys).validate().unwrap();
}

/// A new game, faded in, on the rink's held arrow.
fn rink() -> World {
    let mut w = World::new(&Rink);
    for _ in 0..20 {
        step(&mut w, Buttons::NONE);
    }
    w
}

/// Puts the player on (x, y) of the current map facing `d`, standing still.
fn place(w: &mut World, (x, y): (i16, i16), d: Dir) {
    w.s.objects.set_pos(PLAYER, x, y);
    w.s.objects.face(PLAYER, d);
}

/// Steps with `keys` held until the player stands still with control on `at` and nothing
/// moves them on (no forced step pending, no script, no transition); returns the frames and
/// the cells the player started a step towards, in order. Panics after 300 frames.
fn run_to(w: &mut World, keys: Buttons, at: (i16, i16)) -> (u32, Vec<(i16, i16)>) {
    let mut cells = Vec::new();
    for n in 1..=300 {
        let before = pos(w);
        step(w, keys);
        if pos(w) != before {
            cells.push(pos(w));
        }
        let p = w.objects().player();
        if pos(w) == at && p.is_settled() && !w.script_running() && !w.transition_active() {
            return (n, cells);
        }
    }
    panic!("never settled on {at:?}, at {:?} after {cells:?}", pos(w));
}

/// Frames each step of the player lasted, from `keys` held until the player settles on `at`
/// (a step starts on the frame the player's cell changes).
fn step_lengths(w: &mut World, keys: Buttons, at: (i16, i16)) -> Vec<u32> {
    let mut starts = Vec::new();
    let mut n = 0;
    loop {
        n += 1;
        let before = pos(w);
        step(w, keys);
        if pos(w) != before {
            starts.push(n);
        }
        if pos(w) == at && w.objects().player().is_settled() {
            starts.push(n + 1);
            break;
        }
        assert!(n < 300, "never settled on {at:?}, at {:?}", pos(w));
    }
    starts.windows(2).map(|s| s[1] - s[0]).collect()
}

/// `ForcedMovement_Slip`: one walked step onto the ice, then `walk_fast` (8 frames) cell after
/// cell, without a frame between steps, onto the first cell that is not ice.
#[test]
fn ice_slides_to_the_first_cell_that_is_not_ice() {
    let mut w = rink();
    place(&mut w, (1, 1), Dir::Right);
    // Held right only for the first step: the slide goes on alone.
    step(&mut w, Buttons::RIGHT);
    let mut lengths = vec![];
    let mut n = 1;
    let mut cells = vec![pos(&w)];
    loop {
        let before = pos(&w);
        step(&mut w, Buttons::NONE);
        n += 1;
        if pos(&w) != before {
            lengths.push(n);
            cells.push(pos(&w));
        }
        if w.objects().player().is_settled() && n > 70 {
            break;
        }
    }
    assert_eq!(cells, [(2, 1), (3, 1), (4, 1), (5, 1), (6, 1)]);
    // Walk 16 frames onto the ice, then the slip steps start 8 frames apart.
    assert_eq!(lengths, [17, 25, 33, 41]);
    assert_eq!(w.objects().player().facing, Dir::Right);
}

/// Gen3 skips the keypad while a forced step goes: the player cannot turn or stop mid-slide,
/// and B does not make a slip any faster.
#[test]
fn a_slide_ignores_the_keypad_and_running() {
    let mut w = rink();
    w.s.vars.set_flag(FLAG_RUN, true);
    place(&mut w, (1, 1), Dir::Right);
    let lens = step_lengths(&mut w, Buttons::RIGHT | Buttons::B, (2, 1));
    // Running onto the ice: 8 frames.
    assert_eq!(lens, [8]);
    let lens = step_lengths(&mut w, Buttons::UP | Buttons::B, (6, 1));
    assert_eq!(lens, [8, 8, 8, 8]);
    assert_eq!(w.objects().player().facing, Dir::Right);
}

/// A slide stops on the last ice cell before an obstacle (`DoForcedMovement`'s collision
/// ends it), and the keypad then moves the player off the ice.
#[test]
fn ice_stops_before_an_obstacle_and_the_player_walks_off() {
    let mut w = rink();
    place(&mut w, (1, 2), Dir::Right);
    let (_, cells) = run_to(&mut w, Buttons::RIGHT, (4, 2));
    assert_eq!(cells, [(2, 2), (3, 2), (4, 2)]);
    // Pushing on into the wall bumps; stays put.
    for _ in 0..40 {
        step(&mut w, Buttons::RIGHT);
    }
    assert_eq!(pos(&w), (4, 2));
    // Up: onto the ice of row 1, then a slip north into the wall row stops at once.
    let (_, cells) = run_to(&mut w, Buttons::UP, (4, 1));
    assert_eq!(cells, [(4, 1)]);
    // Left from there slides west to the floor at (1, 1).
    let (_, cells) = run_to(&mut w, Buttons::LEFT, (1, 1));
    assert_eq!(cells, [(3, 1), (2, 1), (1, 1)]);
}

/// A slide up to the map's east wall, over seven cells, ends on the last ice cell.
#[test]
fn a_long_slide_ends_on_the_last_ice_cell() {
    let mut w = rink();
    place(&mut w, (1, 6), Dir::Right);
    let (frames, cells) = run_to(&mut w, Buttons::RIGHT, (8, 6));
    assert_eq!(cells, (2..=8).map(|x| (x, 6)).collect::<Vec<_>>());
    // 16 frames walked onto the ice, then six 8-frame slips.
    assert_eq!(frames, 16 + 6 * 8);
}

/// `ForcedMovement_Walk*`: each arrow forces a `walk_normal` step its way, chained, until a
/// plain cell.
#[test]
fn walk_arrows_chain_and_stop_on_floor() {
    let mut w = rink();
    place(&mut w, (1, 3), Dir::Right);
    step(&mut w, Buttons::RIGHT);
    let mut cells = vec![pos(&w)];
    let mut starts = vec![0];
    for n in 1..100 {
        let before = pos(&w);
        step(&mut w, Buttons::NONE);
        if pos(&w) != before {
            cells.push(pos(&w));
            starts.push(n);
        }
    }
    assert_eq!(cells, [(2, 3), (3, 3), (4, 3), (4, 4)]);
    assert_eq!(starts, [0, 16, 32, 48], "walk_normal steps, back to back");
    assert_eq!(w.objects().player().facing, Dir::Down);
}

/// `ForcedMovement_Slide*`: a `walk_fast` step the arrow's way with the facing kept.
#[test]
fn slide_arrows_keep_the_facing() {
    let mut w = rink();
    place(&mut w, (4, 4), Dir::Right);
    let lens = step_lengths(&mut w, Buttons::RIGHT, (5, 3));
    assert_eq!(lens, [16, 8]);
    assert_eq!(w.objects().player().facing, Dir::Right);
}

/// `ForcedMovement_MuddySlope` on foot: walking up onto it slides the player back south,
/// still facing north.
#[test]
fn a_muddy_slope_slides_the_player_back_south() {
    let mut w = rink();
    place(&mut w, (7, 5), Dir::Up);
    step(&mut w, Buttons::UP);
    assert_eq!(pos(&w), (7, 4));
    let (_, cells) = run_to(&mut w, Buttons::NONE, (7, 5));
    assert_eq!(cells, [(7, 5)]);
    assert_eq!(w.objects().player().facing, Dir::Up);
}

/// Gen3 `PLAYER_AVATAR_FLAG_CONTROLLABLE`: a new game (a warp load) on an arrow holds the
/// forced movement until the player moves; then the keypad moves them as usual, and the
/// next arrow they step on pushes them.
#[test]
fn an_arrival_on_an_arrow_waits_for_the_player_to_move() {
    let mut w = rink();
    assert_eq!(pos(&w), (2, 3));
    assert!(w.objects().forced_held());
    // The held state is captured and restored.
    let cp: Checkpoint = w.checkpoint(&Rink).unwrap();
    let mut r = World::restore(&Rink, &cp).unwrap();
    assert_eq!(r.hash(), w.hash());
    assert!(r.objects().forced_held());
    for _ in 0..30 {
        step(&mut w, Buttons::NONE);
        r.step(&Rink, Buttons::NONE);
    }
    assert_eq!((pos(&w), pos(&r)), ((2, 3), (2, 3)));
    assert_eq!(r.hash(), w.hash());
    // A tapped turn keeps the hold (`TURN_DIRECTION`, then `NOT_MOVING`).
    step(&mut w, Buttons::LEFT);
    for _ in 0..20 {
        step(&mut w, Buttons::NONE);
    }
    assert_eq!(pos(&w), (2, 3));
    assert!(w.objects().forced_held());
    // Facing the way held, the keypad walks the player off before the hold lifts.
    let mut off = World::restore(&Rink, &w.checkpoint(&Rink).unwrap()).unwrap();
    off.s.objects.face(PLAYER, Dir::Left);
    let (_, cells) = run_to(&mut off, Buttons::LEFT, (1, 3));
    assert_eq!(cells, [(1, 3)]);
    assert!(!off.objects().forced_held());
    // Back onto the arrow: pushed east, chained to (4, 4).
    let (_, cells) = run_to(&mut off, Buttons::RIGHT, (4, 4));
    assert_eq!(cells, [(2, 3), (3, 3), (4, 3), (4, 4)]);
    // Held another way, the turn sets `MOVING` once it faces that way, which lifts the hold:
    // the arrow then pushes the player on (as Gen3).
    step(&mut w, Buttons::UP);
    let (_, cells) = run_to(&mut w, Buttons::UP, (4, 4));
    assert_eq!(cells, [(3, 3), (4, 3), (4, 4)]);
}

/// A capture in the middle of a slide restores and replays to the same frames.
#[test]
fn a_slide_round_trips_through_a_checkpoint() {
    let mut w = rink();
    place(&mut w, (1, 6), Dir::Right);
    for _ in 0..30 {
        step(&mut w, Buttons::RIGHT);
    }
    let cp: Checkpoint = w.checkpoint(&Rink).unwrap();
    let mut r = World::restore(&Rink, &cp).unwrap();
    assert_eq!(r.hash(), w.hash());
    for i in 0..60 {
        step(&mut w, Buttons::NONE);
        r.step(&Rink, Buttons::NONE).validate().unwrap();
        assert_eq!(r.hash(), w.hash(), "frame {i}");
    }
    assert_eq!(pos(&r), (8, 6));
}

/// Gen3 `FieldGetPlayerInput`: no step triggers and no A on a forced-movement cell at a step's
/// end, so the player cannot stop on ice to talk.
#[test]
fn the_tile_center_on_ice_reads_no_input() {
    let mut w = rink();
    place(&mut w, (1, 1), Dir::Right);
    let _ = step_lengths(&mut w, Buttons::RIGHT, (2, 1));
    assert!(w.objects().at_forced_tile_center(&w.s.ground(&Rink)));
}

fn gym() -> World {
    let mut w = rink();
    w.debug_warp(&Rink, GYM, None, 1, 1);
    for _ in 0..80 {
        step(&mut w, Buttons::NONE);
    }
    assert_eq!((w.map(), pos(&w)), (GYM, (1, 1)));
    w
}

fn metatile(w: &World, x: i16, y: i16) -> u16 {
    block_metatile(w.s.field.block(&Rink, x, y))
}

/// `SootopolisGymIcePerStepCallback`: the step onto thin ice is counted on its second frame,
/// and the ice cracks five frames later (`tDelay` 4), remembered in its row's var; cracked ice
/// zeroes the count and breaks.
#[test]
fn thin_ice_cracks_and_cracked_ice_breaks() {
    let mut w = gym();
    w.s.vars.set_var(VAR_ICE, 1);
    place(&mut w, (1, 1), Dir::Right);
    let mut seen = Vec::new();
    for t in 1..=16u8 {
        step(&mut w, Buttons::RIGHT);
        seen.push((t, w.var(VAR_ICE), metatile(&w, 2, 1)));
    }
    assert_eq!(pos(&w), (2, 1));
    for &(t, steps, mt) in &seen {
        assert_eq!(steps, if t < NOTICED { 1 } else { 2 }, "frame {t}");
        assert_eq!(mt, if t < ICE_CRACKS { THIN } else { CRACKED }, "frame {t}");
    }
    assert_eq!(w.var(VAR_ROW), 0b001);
    // Next thin ice, then the cracked ice.
    let (_, cells) = run_to(&mut w, Buttons::RIGHT, (4, 1));
    assert_eq!(cells, [(3, 1), (4, 1)]);
    assert_eq!(w.var(VAR_ROW), 0b011);
    assert_eq!(w.var(VAR_ICE), 0);
    assert_eq!(metatile(&w, 3, 1), CRACKED);
    assert_eq!(metatile(&w, 4, 1), BROKEN);
    // Walking back onto cracked ice breaks it too; the floor below does nothing.
    let (_, _) = run_to(&mut w, Buttons::DOWN, (4, 2));
    assert_eq!(metatile(&w, 4, 2), block_metatile(FL));
}

fn cave() -> World {
    let mut w = rink();
    w.debug_warp(&Rink, CAVE, None, 1, 1);
    for _ in 0..80 {
        step(&mut w, Buttons::NONE);
    }
    assert_eq!((w.map(), pos(&w)), (CAVE, (1, 1)));
    w
}

/// `CrackedFloorPerStepCallback`: a cracked floor sets the fall var on the step's second frame
/// and becomes a hole three frames later (`tFloor1Delay`); the step ends on the hole, which
/// runs the fall script (`EventScript_FallDownHole`), whose `warphole` drops the player onto
/// the same cell of the map below (`setholewarp`).
#[test]
fn a_cracked_floor_becomes_a_hole_and_drops_the_player() {
    let mut w = cave();
    w.s.vars.set_var(VAR_ICE, 1);
    place(&mut w, (1, 1), Dir::Right);
    let mut seen = Vec::new();
    for t in 1..=16u8 {
        step(&mut w, Buttons::RIGHT);
        seen.push((t, w.var(VAR_ICE), metatile(&w, 2, 1)));
    }
    for &(t, var, mt) in &seen {
        assert_eq!(var, if t < NOTICED { 1 } else { 0 }, "frame {t}");
        assert_eq!(
            mt,
            if t < FLOOR_BREAKS { CFLOOR } else { HOLE },
            "frame {t}"
        );
    }
    // The step ended on the hole this frame: the fall script runs, then the warp.
    assert!(w.script_running());
    let (_, _) = run_to(&mut w, Buttons::NONE, (2, 1));
    assert_eq!(w.map(), BELOW);
    assert_eq!(w.objects().player().facing, Dir::Down);
}

/// A hole already there drops the player as soon as a step ends on it, and standing on one
/// keeps the fall var at 0 (Gen3's map `on_frame` then drops the player).
#[test]
fn stepping_on_a_hole_falls_to_the_map_below() {
    let mut w = cave();
    place(&mut w, (3, 1), Dir::Down);
    w.s.vars.set_var(VAR_ICE, 1);
    let mut started = false;
    for _ in 0..16 {
        step(&mut w, Buttons::DOWN);
        started |= w.script_running();
    }
    assert!(started);
    assert_eq!(w.var(VAR_ICE), 0);
    let (_, _) = run_to(&mut w, Buttons::NONE, (3, 2));
    assert_eq!(w.map(), BELOW);
}
