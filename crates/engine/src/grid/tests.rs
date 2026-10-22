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
// Then a mirrored pair: "south" (4x3) with "upper" (3x2) north of it, one cell right, and
// "upper" connected back; and "skew", whose north connection to "upper" is not mirrored.
static MAPS: [MapData; 6] = [
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
    map(
        "south",
        4,
        3,
        &[40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51],
        &[Connection {
            dir: Dir::Up,
            offset: 1,
            map: MapId(4),
        }],
    ),
    map(
        "upper",
        3,
        2,
        &[60, 61, 62, 63, 64, 65],
        &[Connection {
            dir: Dir::Down,
            offset: -1,
            map: MapId(3),
        }],
    ),
    map(
        "skew",
        4,
        3,
        &[40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51],
        &[Connection {
            dir: Dir::Up,
            offset: 0,
            map: MapId(4),
        }],
    ),
];

#[test]
fn a_mirrored_crossing_has_a_seam_where_both_maps_draw_alike() {
    let (south, upper) = (MapId(3), MapId(4));
    let s = Game.map(south);
    let (to, shift) = (neighbor(&Game, s, 2, -1))
        .map(|n| (n.map, (n.x - 2, n.y + 1)))
        .unwrap();
    assert_eq!((to, shift), (upper, (-1, 2)));
    let there = seam(&Game, south, upper, shift).unwrap();
    assert_eq!(
        there,
        Seam {
            vertical: true,
            along: (1, 4),
            across: (-2, 3)
        }
    );
    let u = Game.map(upper);
    for y in -5..8 {
        for x in -5..9 {
            let on = (1..4).contains(&x) && (-2..3).contains(&y);
            let same = block_at(&Game, s, x, y) == block_at(&Game, u, x + shift.0, y + shift.1);
            assert!(!on || same, "({x}, {y}) on the seam differs");
        }
    }
    // And back, from the upper map.
    let back = seam(&Game, upper, south, (1, -2)).unwrap();
    assert_eq!((back.along, back.across), ((0, 3), (0, 5)));
}

#[test]
fn a_crossing_without_a_mirror_has_no_seam() {
    // `north` has no connection back; `skew`'s offset is not mirrored by `upper`'s.
    assert_eq!(seam(&Game, MapId(0), MapId(1), (1, 1)), None);
    assert_eq!(seam(&Game, MapId(5), MapId(4), (0, 2)), None);
}

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
