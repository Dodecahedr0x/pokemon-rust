//! Synthetic maps for the flow tests.

use crate::data::*;
use crate::script::{Ctx, Script};

pub const DOOR: u16 = 1;
pub const WARP: u16 = 2;
pub const ARROW_UP: u16 = 3;
pub const ARROW_DOWN: u16 = 4;
pub const COUNTER: u16 = 5;

static PRIMARY: Tileset = Tileset {
    tiles: &[],
    palettes: &[],
    metatiles: &[],
    // Raw behavior = metatile id (low byte); layer bits must be ignored.
    attributes: &[0x1000, 1, 2, 0x2003, 4, 5],
};

static SECONDARY: Tileset = Tileset {
    tiles: &[],
    palettes: &[],
    metatiles: &[],
    attributes: &[1],
};

pub const E3: u16 = 3 << BLOCK_ELEVATION_SHIFT;

// "town", 6x6.
#[rustfmt::skip]
static TOWN_BLOCKS: [u16; 36] = [
    0, ARROW_UP, 0, 0, 0, 0,
    0, 0, DOOR, 0, 0, 0,
    0, 0, 0, E3, 0, 0,
    0, COUNTER, 0, 0, 0, 0,
    0, 0, 0, 0, WARP | E3, 0,
    0, 0, 0, 0, 0, 512,
];

#[rustfmt::skip]
static HOUSE_BLOCKS: [u16; 16] = [
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, DOOR, 0, ARROW_DOWN,
];

pub const fn warp(x: i16, y: i16, elevation: u8, map: u16, dest_warp: u8) -> WarpEvent {
    WarpEvent {
        x,
        y,
        elevation,
        dest_map: MapId(map),
        dest_warp,
    }
}

const fn coord(x: i16, y: i16, elevation: u8, var: u16, value: u16, script: u16) -> CoordEvent {
    CoordEvent {
        x,
        y,
        elevation,
        var: VarId(var),
        value,
        script: ScriptId(script),
    }
}

const fn bg(x: i16, y: i16, elevation: u8, kind: BgKind) -> BgEvent {
    BgEvent {
        x,
        y,
        elevation,
        kind,
    }
}

const fn vs(var: u16, value: u16, script: u16) -> VarScript {
    VarScript {
        var: VarId(var),
        value,
        script: ScriptId(script),
    }
}

static MAPS: [MapData; 2] = [
    MapData {
        name: "town",
        width: 6,
        height: 6,
        blocks: &TOWN_BLOCKS,
        border: [0; 4],
        primary: &PRIMARY,
        secondary: &SECONDARY,
        objects: &[],
        warps: &[
            warp(2, 1, 0, 1, 0),
            warp(4, 4, 3, 1, 1),
            warp(1, 0, 0, 1, 1),
            warp(5, 5, 0, 1, 2),
        ],
        coords: &[
            coord(3, 3, 0, 0x4001, 0, 10),
            coord(3, 3, 0, 0x4001, 1, 11),
            coord(2, 2, 4, 0x4002, 0, 12),
            coord(0, 0, 0, 0x4003, 0x101, 13),
            coord(5, 0, 0, 0, 0, 14),
        ],
        bgs: &[
            bg(
                0,
                2,
                0,
                BgKind::Script {
                    facing: Some(Dir::Up),
                    script: ScriptId(20),
                },
            ),
            bg(
                5,
                2,
                0,
                BgKind::Script {
                    facing: None,
                    script: ScriptId(21),
                },
            ),
            bg(
                5,
                3,
                0,
                BgKind::HiddenItem {
                    item: 1,
                    flag: FlagId(0x500),
                },
            ),
            bg(
                4,
                0,
                2,
                BgKind::Script {
                    facing: None,
                    script: ScriptId(22),
                },
            ),
            bg(
                4,
                0,
                0,
                BgKind::Script {
                    facing: None,
                    script: ScriptId(23),
                },
            ),
        ],
        connections: &[],
        scripts: MapScripts {
            on_load: None,
            on_transition: None,
            on_resume: None,
            on_return_to_field: None,
            on_frame: &[vs(0x4005, 1, 30), vs(0x4005, 1, 31), vs(0x4006, 0x4007, 32)],
            on_warp_into: &[vs(0x4005, 2, 40), vs(0x4006, 0, 41)],
        },
        allow_running: true,
    },
    MapData {
        name: "house",
        width: 4,
        height: 4,
        blocks: &HOUSE_BLOCKS,
        border: [0; 4],
        primary: &PRIMARY,
        secondary: &SECONDARY,
        objects: &[],
        warps: &[
            warp(1, 3, 0, 0, 0),
            warp(2, 0, 0, 0, 1),
            warp(3, 3, 0, 0, 2),
        ],
        coords: &[],
        bgs: &[],
        connections: &[],
        scripts: MapScripts {
            on_load: None,
            on_transition: None,
            on_resume: None,
            on_return_to_field: None,
            on_frame: &[],
            on_warp_into: &[],
        },
        allow_running: false,
    },
];

pub struct Game;

impl GameData for Game {
    fn map(&self, id: MapId) -> &'static MapData {
        &MAPS[id.0 as usize]
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        unimplemented!()
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!()
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, raw: u8) -> Behavior {
        match raw as u16 {
            DOOR => Behavior::Door,
            WARP => Behavior::Warp,
            ARROW_UP => Behavior::ArrowWarp(Dir::Up),
            ARROW_DOWN => Behavior::ArrowWarp(Dir::Down),
            COUNTER => Behavior::Counter,
            _ => Behavior::Normal,
        }
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        alloc::boxed::Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
}

pub const TOWN: MapId = MapId(0);
pub const HOUSE: MapId = MapId(1);

pub fn town() -> &'static MapData {
    Game.map(TOWN)
}
