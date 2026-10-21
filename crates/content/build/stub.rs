//! The built-in world used when `content/extracted/` is absent (fresh clone, CI): one small
//! map drawn with the M1 test tiles, a plain player sprite, a 1-glyph font and a plain window.
//! Needs no ROM.

use crate::input::{Glyph, SpriteIn, Tile, TilesetIn, World};
use extract::schema::{
    Header, Layout, MapFile, MapScripts, Row, SpriteFile, TilesetFile, WindowFile, to_rows,
};
use extract::text::Lang;
use std::collections::BTreeMap;

pub const MAP_NAME: &str = "StubTown";
/// The stub map takes `MAP_LITTLEROOT_TOWN`'s number, so `new_game()` is the same in both modes.
pub const MAP_GROUP: u8 = 0;
pub const MAP_NUM: u8 = 9;
pub const WIDTH: u16 = 16;
pub const HEIGHT: u16 = 16;
pub const START: (i16, i16) = (5, 5);

const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

/// The M1 scene tiles (grass, checker, border, dot) and a transparent tile.
const TILES: [Tile; 5] = [
    [0x1111_1111; 8],
    [
        0x2121_2121,
        0x1212_1212,
        0x2121_2121,
        0x1212_1212,
        0x2121_2121,
        0x1212_1212,
        0x2121_2121,
        0x1212_1212,
    ],
    [0x3333_3333; 8],
    [
        0x1111_1111,
        0x1111_1111,
        0x1111_1111,
        0x1114_4111,
        0x1114_4111,
        0x1111_1111,
        0x1111_1111,
        0x1111_1111,
    ],
    [0; 8],
];
const BLANK: u16 = 4;
const GRASS: u16 = 0;
const CHECKER: u16 = 1;
const BORDER: u16 = 2;
const DOT: u16 = 3;
const COLLISION: u16 = 1 << 10;
const ELEVATION_3: u16 = 3 << 12;

fn palettes() -> Vec<[u16; 16]> {
    let mut p = [0u16; 16];
    p[1] = rgb(8, 24, 8);
    p[2] = rgb(14, 28, 10);
    p[3] = rgb(4, 8, 16);
    p[4] = rgb(31, 28, 4);
    let mut all = vec![[0u16; 16]; 16];
    all[0] = p;
    all
}

fn tilesets() -> Vec<TilesetIn> {
    // Metatile i: tile i on the bottom layer, transparent top layer; palette 0.
    let metatiles = (0..4u16)
        .map(|t| [t, t, t, t, BLANK, BLANK, BLANK, BLANK])
        .collect::<Vec<_>>();
    vec![
        TilesetIn {
            file: TilesetFile {
                name: "StubPrimary".into(),
                secondary: false,
                tile_count: TILES.len() as u16,
                palettes: palettes(),
                attributes: vec![0; metatiles.len()],
                metatiles,
            },
            tiles: TILES.to_vec(),
        },
        TilesetIn {
            file: TilesetFile {
                name: "StubSecondary".into(),
                secondary: true,
                tile_count: 1,
                palettes: vec![[0; 16]; 16],
                metatiles: vec![],
                attributes: vec![],
            },
            tiles: vec![[0; 8]],
        },
    ]
}

fn map() -> MapFile {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let blocks: Vec<u16> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                BORDER | COLLISION | ELEVATION_3
            } else if x % 8 == 3 && y % 8 == 3 {
                DOT | ELEVATION_3
            } else if (x / 4 + y / 4) % 2 == 0 {
                GRASS | ELEVATION_3
            } else {
                CHECKER | ELEVATION_3
            }
        })
        .collect();
    MapFile {
        name: MAP_NAME.into(),
        group: MAP_GROUP,
        num: MAP_NUM,
        layout: Layout {
            width: WIDTH,
            height: HEIGHT,
            primary: "StubPrimary".into(),
            secondary: "StubSecondary".into(),
            border: vec![Row(vec![BORDER | COLLISION; 2]); 2],
            blocks: to_rows(&blocks, w),
        },
        header: Header {
            music: 0,
            region_map_section: 0,
            cave: 0,
            weather: 0,
            map_type: 1,
            allow_cycling: true,
            allow_escaping: false,
            allow_running: true,
            show_map_name: false,
            battle_type: 0,
        },
        objects: vec![],
        warps: vec![],
        coords: vec![],
        bgs: vec![],
        connections: vec![],
        scripts: MapScripts::default(),
    }
}

/// Player sprite: 18 frames of 16x32 (9 walk + 9 run, as `BRENDAN_NORMAL`), a filled body
/// whose color index tells the frame group apart.
fn player() -> SpriteIn {
    let frames = (0..18u32)
        .map(|k| {
            let c = if k < 9 { 1 } else { 2 };
            let fill = c * 0x1111_1111;
            (0..8)
                .map(|t| if t < 2 { [0; 8] } else { [fill; 8] })
                .collect()
        })
        .collect();
    let mut palette = [0u16; 16];
    palette[1] = rgb(31, 8, 8);
    palette[2] = rgb(8, 8, 31);
    SpriteIn {
        file: SpriteFile {
            name: "BRENDAN_NORMAL".into(),
            gfx_id: 0,
            width: 16,
            height: 32,
            cell_width: 16,
            cell_height: 32,
            frame_sizes: vec![(16, 32); 18],
            palette_tag: 0x1100,
            palette,
        },
        frames,
    }
}

pub fn world() -> World {
    let mut window_pal = [0u16; 16];
    window_pal[1] = rgb(31, 31, 31);
    window_pal[2] = rgb(4, 4, 4);
    window_pal[3] = rgb(20, 20, 20);
    window_pal[4] = rgb(4, 8, 16);
    let glyph: Glyph = [0; 64];
    World {
        lang: Lang::Fr,
        maps: vec![map()],
        tilesets: tilesets(),
        sprites: vec![player()],
        glyphs: vec![glyph],
        widths: vec![3],
        window: WindowFile {
            tile_count: 14,
            palette: window_pal,
            text_fg: 2,
            text_shadow: 3,
            text_bg: 1,
        },
        window_tiles: (0..14)
            .map(|i| {
                if i == 0 {
                    [0x1111_1111; 8]
                } else {
                    [0x4444_4444; 8]
                }
            })
            .collect(),
        texts: BTreeMap::new(),
        index: None,
        sha1: String::new(),
        dir: None,
    }
}
