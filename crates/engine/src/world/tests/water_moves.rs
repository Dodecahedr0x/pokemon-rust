//! Wave F water field moves on a synthetic sea: the water door and the pool's south arrow warp
//! (`MB_WATER_DOOR`, `MB_WATER_SOUTH_ARROW_WARP`), Waterfall's climb (`GetInteractedWaterScript`,
//! `FLDEFF_USE_WATERFALL`), Dive down and back up (`TrySetupDiveDownScript`,
//! `TrySetupDiveEmergeScript`, `FLDEFF_USE_DIVE`, the dive and emerge connections), and the
//! arrival's avatar state (`GetAdjustedInitialTransitionFlags`).

use super::super::field_move::SHOW_MON_OUTDOORS;
use super::*;
use crate::battle::test_data::{EMBER, WATER, WATER_GUN};

// Raw behaviours (metatile k has raw behaviour k).
const OCEAN: u16 = 2;
const WATER_DOOR: u16 = 3;
const SOUTH_ARROW: u16 = 4;
const FALLS: u16 = 5;
const DEEP: u16 = 6;
const SEAWEED: u16 = 7;
const NO_SURFACING: u16 = 8;

const fn at(raw: u16, elevation: u16) -> u16 {
    raw | elevation << BLOCK_ELEVATION_SHIFT
}
const LAND: u16 = at(FLOOR, 3);
const SEA_W: u16 = at(OCEAN, 1);

static SEA_TS: Tileset = Tileset {
    tiles: &TILES,
    palettes: &[[0x1234; 16]; 16],
    metatiles: &[[0; 8]; 9],
    attributes: &[0, 1, 2, 3, 4, 5, 6, 7, 8],
};

const SEA: MapId = MapId(0);
const POOL: MapId = MapId(1);
const UNDER: MapId = MapId(2);

/// 8x8: land on rows 0–1, water below; the water door on (5, 2); a waterfall on (1, 3)–(1, 5)
/// between the water on (1, 2) and (1, 6); deep water on (6, 6) and (3, 7).
const fn sea_blocks() -> [u16; 64] {
    let mut b = [SEA_W; 64];
    let mut i = 0;
    while i < 16 {
        b[i] = LAND;
        i += 1;
    }
    b[2 * 8 + 5] = at(WATER_DOOR, 1);
    b[3 * 8 + 1] = at(FALLS, 1);
    b[4 * 8 + 1] = at(FALLS, 1);
    b[5 * 8 + 1] = at(FALLS, 1);
    b[6 * 8 + 6] = at(DEEP, 1);
    b[7 * 8 + 3] = at(DEEP, 1);
    b
}
static SEA_BLOCKS: [u16; 64] = sea_blocks();

/// 6x6 pool: land on row 0, water below, the south arrow warp on (2, 5).
const fn pool_blocks() -> [u16; 36] {
    let mut b = [SEA_W; 36];
    let mut i = 0;
    while i < 6 {
        b[i] = LAND;
        i += 1;
    }
    b[5 * 6 + 2] = at(SOUTH_ARROW, 1);
    b
}
static POOL_BLOCKS: [u16; 36] = pool_blocks();

/// 8x8 seaweed, with a no-surfacing cell on (5, 6).
const fn under_blocks() -> [u16; 64] {
    let mut b = [at(SEAWEED, 1); 64];
    b[6 * 8 + 5] = at(NO_SURFACING, 1);
    b
}
static UNDER_BLOCKS: [u16; 64] = under_blocks();

const fn warp(x: i16, y: i16, dest_map: MapId, dest_warp: u8) -> WarpEvent {
    WarpEvent {
        x,
        y,
        elevation: 1,
        dest_map,
        dest_warp,
    }
}

const fn map(
    name: &'static str,
    width: u16,
    blocks: &'static [u16],
    warps: &'static [WarpEvent],
) -> MapData {
    MapData {
        name,
        width,
        height: (blocks.len() / width as usize) as u16,
        blocks,
        border: [WALL; 4],
        primary: &SEA_TS,
        secondary: &TS2,
        objects: &[],
        warps,
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: NO_SCRIPTS,
        allow_running: false,
        map_type: 0,
    }
}

static SEA_MAPS: [MapData; 3] = [
    map("sea", 8, &SEA_BLOCKS, &[warp(5, 2, POOL, 0)]),
    map("pool", 6, &POOL_BLOCKS, &[warp(2, 5, SEA, 0)]),
    map("under", 8, &UNDER_BLOCKS, &[]),
];

const FLAG_BADGE: FlagId = FlagId(0x867);
const FLAG_REFUSED: FlagId = FlagId(0x303);
/// The moves the gates ask for (any of the test tables).
const WATERFALL_MOVE: MoveId = EMBER;
const DIVE_MOVE: MoveId = WATER_GUN;
const USE_WATERFALL: ScriptId = ScriptId(80);
const CANNOT_WATERFALL: ScriptId = ScriptId(81);
const USE_DIVE: ScriptId = ScriptId(82);
const DIVE_UP: ScriptId = ScriptId(83);

struct Sea;

async fn effect(ctx: Ctx, m: FieldMove) {
    // `EventScript_Use*` past its yes/no: `dofieldeffect`, `releaseall`, `end`.
    ctx.field_move_effect(m).await;
}

async fn refused(ctx: Ctx) {
    ctx.set_flag(FLAG_REFUSED, true).await;
}

impl GameData for Sea {
    fn map(&self, id: MapId) -> &'static MapData {
        &SEA_MAPS[id.0 as usize]
    }
    fn has_map(&self, id: MapId) -> bool {
        (id.0 as usize) < SEA_MAPS.len()
    }
    fn special_ids(&self) -> SpecialIds {
        Game.special_ids()
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
            OCEAN => Behavior::Water(Water::Ocean),
            WATER_DOOR => Behavior::Water(Water::Door),
            SOUTH_ARROW => Behavior::Water(Water::SouthArrowWarp),
            FALLS => Behavior::Water(Water::Waterfall),
            DEEP => Behavior::Water(Water::Deep),
            SEAWEED => Behavior::Water(Water::Seaweed),
            NO_SURFACING => Behavior::Water(Water::NoSurfacing),
            _ => Behavior::Normal,
        }
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        match id {
            USE_WATERFALL => Box::pin(effect(ctx, FieldMove::Waterfall)),
            USE_DIVE | DIVE_UP => Box::pin(effect(ctx, FieldMove::Dive)),
            CANNOT_WATERFALL => Box::pin(refused(ctx)),
            _ => Game.script(id, ctx),
        }
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: SEA,
            x: 5,
            y: 4,
            facing: Dir::Up,
            ..Game.new_game()
        }
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn map_type(&self, id: MapId) -> MapType {
        match id {
            POOL => MapType::Indoor,
            UNDER => MapType::Underwater,
            _ => MapType::Outdoor,
        }
    }
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        let move_id = match m {
            FieldMove::Waterfall => WATERFALL_MOVE,
            FieldMove::Dive => DIVE_MOVE,
            _ => return FieldMoveGate::Never,
        };
        FieldMoveGate::PartyMove {
            move_id,
            badge: Some(FLAG_BADGE),
        }
    }
    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        match m {
            FieldMove::Waterfall => Some(USE_WATERFALL),
            FieldMove::Dive => Some(USE_DIVE),
            _ => None,
        }
    }
    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        Some(CANNOT_WATERFALL)
    }
    fn dive_emerge_script(&self) -> Option<ScriptId> {
        Some(DIVE_UP)
    }
    fn dive_warp(
        &self,
        map: MapId,
        emerge: bool,
        at: (i16, i16),
    ) -> Option<(MapId, Option<(i16, i16)>)> {
        match (map, emerge) {
            // A fixed dive warp picked by the player's cell (Gen3 `on_dive_warp` scripts read
            // it): (3, 7) leads to the pool's (3, 3), surfing.
            (SEA, false) if at == (3, 7) => Some((POOL, Some((3, 3)))),
            (SEA, false) => Some((UNDER, None)),
            (UNDER, true) => Some((SEA, None)),
            _ => None,
        }
    }
}

fn hold(w: &mut World, keys: Buttons, frames: u32) {
    for _ in 0..frames {
        w.step(&Sea, keys).validate().unwrap();
    }
}

/// Holds `keys` until the player stands still on `to` with control, then releases a frame.
fn go(w: &mut World, keys: Buttons, to: (MapId, (i16, i16))) -> u32 {
    for n in 0..600 {
        if (w.map(), pos(w)) == to && w.objects().player().is_settled() && !w.transition_active() {
            hold(w, Buttons::NONE, 1);
            return n;
        }
        hold(w, keys, 1);
    }
    panic!("never reached {to:?}, at {:?} {:?}", w.map(), pos(w));
}

/// Idles until no script, effect nor transition runs.
fn settle(w: &mut World) {
    for _ in 0..600 {
        if !w.runner.is_running() && !w.s.field_move_busy() && !w.transition_active() {
            hold(w, Buttons::NONE, 2);
            return;
        }
        hold(w, Buttons::NONE, 1);
    }
    panic!("never settled");
}

/// The sea after the new-game fade: surfing on (5, 4) facing up, with a mon that knows both
/// moves, and the badge (`badge`).
fn sea(badge: bool) -> World {
    let mut w = World::new(&Sea);
    hold(&mut w, Buttons::NONE, 16);
    let ctx = w.runner.ctx();
    w.runner.start(Box::pin(async move {
        ctx.give_mon(WATER, 30).await;
    }));
    hold(&mut w, Buttons::NONE, 4);
    w.s.party.get_mut(0).unwrap().moves = [Some(WATERFALL_MOVE), Some(DIVE_MOVE), None, None];
    w.s.vars.set_flag(FLAG_BADGE, badge);
    w.s.objects.set_surfing(true);
    assert_eq!((w.map(), pos(&w)), (SEA, (5, 4)));
    w
}

#[test]
fn surfing_onto_the_water_door_warps_and_the_pool_arrow_warps_back() {
    let mut w = sea(false);
    // Onto the water door: its warp, arriving surfing on the pool's arrow warp, facing north
    // (`GetAdjustedInitialDirection`'s south arrow warp case).
    go(&mut w, Buttons::UP, (POOL, (2, 5)));
    assert!(w.objects().surfing());
    assert_eq!(w.objects().player().facing, Dir::Up);
    // Pushing south on it (`TryArrowWarp`): back onto the door, which faces south and steps
    // out (`Task_ExitNonAnimDoor`), still surfing.
    go(&mut w, Buttons::DOWN, (SEA, (5, 3)));
    assert!(w.objects().surfing());
    assert_eq!(w.objects().player().facing, Dir::Down);
}

#[test]
fn a_warp_onto_dry_land_ends_surfing() {
    let mut w = sea(false);
    // A debug warp is a warp like any other: the arrival cell decides.
    assert!(w.debug_warp(&Sea, POOL, None, 3, 0));
    go(&mut w, Buttons::NONE, (POOL, (3, 0)));
    assert!(!w.objects().surfing());
}

#[test]
fn waterfall_climbs_north_while_the_cells_are_falls() {
    let mut w2 = sea(true);
    // From the side, a waterfall is `EventScript_CannotUseWaterfall`.
    go(&mut w2, Buttons::LEFT, (SEA, (2, 4)));
    hold(&mut w2, Buttons::A, 1);
    settle(&mut w2);
    assert!(w2.flag(FLAG_REFUSED));
    // From its foot (1, 6), facing north: A runs the use script, the effect shows the mon, then
    // rides up one slow walk per cell to the first cell that is no waterfall.
    let mut w = sea(true);
    go(&mut w, Buttons::DOWN, (SEA, (5, 6)));
    go(&mut w, Buttons::LEFT, (SEA, (1, 6)));
    hold(&mut w, Buttons::UP, 1);
    hold(&mut w, Buttons::NONE, 10);
    assert_eq!(w.objects().player().facing, Dir::Up);
    hold(&mut w, Buttons::A, 1);
    assert_eq!(w.s.last_script, Some(USE_WATERFALL));
    let mut busy = 0;
    while busy == 0 || w.s.field_move_busy() {
        if w.s.field_move_busy() {
            busy += 1;
        }
        hold(&mut w, Buttons::NONE, 1);
        assert!(busy < 1000);
    }
    assert_eq!(pos(&w), (1, 2));
    assert!(w.objects().surfing());
    assert!(!w.flag(FLAG_REFUSED));
    // Init, the outdoor banner, then four slow walks (onto the three falls and off them; each
    // next one starts in the frame the last settles).
    let walks = busy as u16 - 1 - SHOW_MON_OUTDOORS;
    assert!((4 * 29..=4 * 32).contains(&walks), "{busy}");
}

#[test]
fn waterfall_without_the_badge_is_refused() {
    let mut w = sea(false);
    go(&mut w, Buttons::DOWN, (SEA, (5, 6)));
    go(&mut w, Buttons::LEFT, (SEA, (1, 6)));
    hold(&mut w, Buttons::UP, 1);
    hold(&mut w, Buttons::NONE, 10);
    hold(&mut w, Buttons::A, 1);
    settle(&mut w);
    assert!(w.flag(FLAG_REFUSED));
    assert_eq!(pos(&w), (1, 6));
}

#[test]
fn dive_goes_down_the_connection_and_b_surfaces() {
    let mut w = sea(true);
    // A on plain ocean water: nothing.
    hold(&mut w, Buttons::A, 1);
    settle(&mut w);
    assert_eq!(w.map(), SEA);
    go(&mut w, Buttons::DOWN, (SEA, (5, 6)));
    go(&mut w, Buttons::RIGHT, (SEA, (6, 6)));
    hold(&mut w, Buttons::A, 1);
    assert_eq!(w.s.last_script, Some(USE_DIVE));
    settle(&mut w);
    // The dive connection keeps the cell; underwater the player is on foot and keeps facing
    // (the surfing/underwater swap), and walks at the normal speed.
    assert_eq!((w.map(), pos(&w)), (UNDER, (6, 6)));
    assert!(!w.objects().surfing());
    assert_eq!(w.objects().player().facing, Dir::Right);
    // A turn (8 frames), then a `walk_normal` (16).
    assert_eq!(go(&mut w, Buttons::LEFT, (UNDER, (5, 6))), 8 + 16);
    // B on a no-surfacing cell: nothing.
    hold(&mut w, Buttons::B, 1);
    settle(&mut w);
    assert_eq!(w.map(), UNDER);
    go(&mut w, Buttons::UP, (UNDER, (5, 5)));
    hold(&mut w, Buttons::B, 1);
    assert_eq!(w.s.last_script, Some(DIVE_UP));
    settle(&mut w);
    assert_eq!((w.map(), pos(&w)), (SEA, (5, 5)));
    assert!(w.objects().surfing());
    assert_eq!(w.objects().player().facing, Dir::Up);
}

#[test]
fn the_dive_warp_depends_on_the_cell() {
    let mut w = sea(true);
    go(&mut w, Buttons::DOWN, (SEA, (5, 7)));
    go(&mut w, Buttons::LEFT, (SEA, (3, 7)));
    hold(&mut w, Buttons::A, 1);
    assert_eq!(w.s.last_script, Some(USE_DIVE));
    settle(&mut w);
    assert_eq!((w.map(), pos(&w)), (POOL, (3, 3)));
}

#[test]
fn dive_needs_the_badge() {
    let mut w = sea(false);
    go(&mut w, Buttons::DOWN, (SEA, (5, 6)));
    go(&mut w, Buttons::RIGHT, (SEA, (6, 6)));
    hold(&mut w, Buttons::A, 1);
    settle(&mut w);
    assert_eq!(w.map(), SEA);
    assert_ne!(w.s.last_script, Some(USE_DIVE));
}
