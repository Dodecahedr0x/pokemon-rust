use super::*;
use crate::data::*;
use crate::script::{Ctx, Script};

static TS: Tileset = Tileset {
    tiles: &[],
    palettes: &[],
    metatiles: &[],
    attributes: &[],
};

const fn map(
    name: &'static str,
    width: u16,
    height: u16,
    blocks: &'static [u16],
    connections: &'static [Connection],
) -> MapData {
    MapData {
        name,
        width,
        height,
        blocks,
        border: [0x10, 0x11, 0x12, 0x13],
        primary: &TS,
        secondary: &TS,
        objects: &[],
        warps: &[],
        coords: &[],
        bgs: &[],
        connections,
        scripts: MapScripts {
            on_load: None,
            on_transition: None,
            on_resume: None,
            on_return_to_field: None,
            on_frame: &[],
            on_warp_into: &[],
        },
        allow_running: true,
    }
}

// Center 2x2 map; north neighbor 4x1 shifted left by 1; east neighbor 1x3 shifted up by 1.
static MAPS: [MapData; 3] = [
    map(
        "center",
        2,
        2,
        &[1, 2, 3, 4],
        &[
            Connection {
                dir: Dir::Up,
                offset: -1,
                map: MapId(1),
            },
            Connection {
                dir: Dir::Right,
                offset: -1,
                map: MapId(2),
            },
        ],
    ),
    map("north", 4, 1, &[20, 21, 22, 23], &[]),
    map("east", 1, 3, &[30, 31, 32], &[]),
];

pub(crate) struct Game;

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
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
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

#[test]
fn inside_the_map() {
    let m = Game.map(MapId(0));
    assert_eq!(block_at(&Game, m, 0, 0), 1);
    assert_eq!(block_at(&Game, m, 1, 1), 4);
}

#[test]
fn north_connection_with_offset() {
    let m = Game.map(MapId(0));
    // Neighbor x = x - offset = x + 1; neighbor y = y + height(1).
    assert_eq!(block_at(&Game, m, 0, -1), 21);
    assert_eq!(block_at(&Game, m, 1, -1), 22);
    assert_eq!(
        neighbor(&Game, m, 1, -1),
        Some(Neighbor {
            map: MapId(1),
            x: 2,
            y: 0
        })
    );
    // Two rows up is past the 1-row neighbor: border.
    assert_eq!(block_at(&Game, m, 0, -2), 0x10 | 0xC00);
}

#[test]
fn east_connection_with_offset() {
    let m = Game.map(MapId(0));
    // Neighbor y = y - offset = y + 1.
    assert_eq!(block_at(&Game, m, 2, 0), 31);
    assert_eq!(block_at(&Game, m, 2, 1), 32);
    assert_eq!(
        block_at(&Game, m, 2, -1),
        0x12 | 0xC00,
        "corner is never connected"
    );
}

#[test]
fn border_tiles_2x2_and_is_impassable() {
    let m = Game.map(MapId(0));
    assert_eq!(block_at(&Game, m, -1, 0), 0x11 | 0xC00);
    assert_eq!(block_at(&Game, m, -2, 0), 0x10 | 0xC00);
    assert_eq!(block_at(&Game, m, 0, 2), 0x10 | 0xC00);
    assert_eq!(block_at(&Game, m, 1, 3), 0x13 | 0xC00);
    assert_eq!(block_collision(block_at(&Game, m, 0, 5)), 3);
}
