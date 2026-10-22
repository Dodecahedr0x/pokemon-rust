//! The built-in world used when `content/extracted/` is absent (fresh clone, CI): one small
//! map drawn with the M1 test tiles, a plain player sprite, a 1-glyph font and a plain window.
//! Needs no ROM.

use crate::battle::{BattleIn, MonGfxIn, PicIn, UiGfx};
use crate::classify::{STRS, StrSrc};
use crate::input::{Glyph, SpriteIn, Tile, TilesetIn, World};
use extract::battle_data::STAT_NAME_KEYS;
use extract::schema::{
    Header, LandEncounters, Layout, MapFile, MapScripts, MonCoords, Move, MovesFile, Row, Species,
    SpriteFile, TilesetFile, Trainer, TypesFile, WildMon, WindowFile, to_rows,
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
/// The one tall-grass cell (`MB_TALL_GRASS`), off the stub parity route.
pub const TALL_GRASS: (usize, usize) = (12, 3);
/// Gen3 `MB_TALL_GRASS`.
const MB_TALL_GRASS: u16 = 0x02;

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
const TALL_GRASS_METATILE: u16 = 4;
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
    // Metatile i: tile i on the bottom layer, transparent top layer; palette 0. Metatile 4 is
    // tall grass, drawn with the dot tile.
    let mut metatiles = (0..4u16)
        .map(|t| [t, t, t, t, BLANK, BLANK, BLANK, BLANK])
        .collect::<Vec<_>>();
    metatiles.push([DOT, DOT, DOT, DOT, BLANK, BLANK, BLANK, BLANK]);
    let mut attributes = vec![0; metatiles.len()];
    attributes[TALL_GRASS_METATILE as usize] = MB_TALL_GRASS;
    vec![
        TilesetIn {
            file: TilesetFile {
                name: "StubPrimary".into(),
                secondary: false,
                tile_count: TILES.len() as u16,
                palettes: palettes(),
                attributes,
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
            if (x, y) == TALL_GRASS {
                TALL_GRASS_METATILE | ELEVATION_3
            } else if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
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
        battle: battle(),
    }
}

fn species(id: &str, ty: &str, learnset: &[(u8, &str)]) -> Species {
    Species {
        id: id.into(),
        name: id.trim_start_matches("SPECIES_").replace('_', " "),
        base: [45, 49, 49, 45, 65, 65],
        types: [ty.into(), ty.into()],
        catch_rate: 45,
        exp_yield: 64,
        ev_yield: [0, 0, 0, 0, 1, 0],
        gender_ratio: 31,
        friendship: 70,
        growth: "GROWTH_MEDIUM_SLOW".into(),
        abilities: ["ABILITY_NONE".into(), "ABILITY_NONE".into()],
        learnset: learnset.iter().map(|&(l, m)| (l, m.into())).collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn mov(id: &str, effect: &str, power: u8, ty: &str, accuracy: u8, pp: u8, flags: &[&str]) -> Move {
    Move {
        id: id.into(),
        name: id.trim_start_matches("MOVE_").replace('_', " "),
        effect: effect.into(),
        power,
        ty: ty.into(),
        accuracy,
        pp,
        chance: 0,
        target: "MOVE_TARGET_SELECTED".into(),
        priority: 0,
        flags: flags.iter().map(|f| f.to_string()).collect(),
    }
}

/// A 64x64 pic: a filled square of color `c` inside a transparent frame.
fn pic(c: u32) -> Vec<Tile> {
    (0..64)
        .map(|i| {
            let (x, y) = (i % 8, i / 8);
            if (2..6).contains(&x) && (2..8).contains(&y) {
                [c * 0x1111_1111; 8]
            } else {
                [0; 8]
            }
        })
        .collect()
}

fn mon_gfx(c: u32) -> MonGfxIn {
    let mut palette = [0u16; 16];
    palette[1] = rgb(28, 12, 4);
    palette[2] = rgb(4, 16, 28);
    let mut shiny = palette;
    shiny[1] = rgb(28, 28, 4);
    shiny[2] = rgb(4, 28, 16);
    MonGfxIn {
        front: pic(c),
        back: pic(c),
        palette,
        shiny,
        coords: MonCoords {
            front_y: 0,
            back_y: 0,
            elevation: 0,
        },
    }
}

/// The stub's battle data: two species, three moves (plus `MOVE_NONE`), a three-entry type
/// chart, one encounter table on `StubTown` (its tall-grass cell), one heal location, every
/// battle string as a placeholder, pics for both species and the player, and no interface
/// graphics (`GfxImage::EMPTY`, which the engine draws as nothing).
pub fn battle() -> BattleIn {
    const HIT: &str = "MOVE_STUB_HIT";
    let moves = vec![
        mov("MOVE_NONE", "EFFECT_HIT", 0, "TYPE_NORMAL", 0, 0, &[]),
        mov(
            HIT,
            "EFFECT_HIT",
            40,
            "TYPE_NORMAL",
            100,
            35,
            &["FLAG_MAKES_CONTACT", "FLAG_PROTECT_AFFECTED"],
        ),
        mov(
            "MOVE_STUB_GROWL",
            "EFFECT_ATTACK_DOWN",
            0,
            "TYPE_NORMAL",
            100,
            40,
            &["FLAG_PROTECT_AFFECTED"],
        ),
        mov(
            "MOVE_STUB_STRUGGLE",
            "EFFECT_RECOIL",
            50,
            "TYPE_NORMAL",
            0,
            1,
            &["FLAG_MAKES_CONTACT"],
        ),
    ];
    let mut strings = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        strings.insert(k.to_string(), v);
    };
    for (_, src) in STRS {
        match *src {
            StrSrc::Key(k) => put(k, "TEXTE!".into()),
            StrSrc::UsedMove => {
                put(
                    "sText_AttackerUsedX",
                    "{B_ATK_NAME_WITH_PREFIX} {B_BUFF2}".into(),
                );
                for k in ["", "2", "3", "4", "5"] {
                    put(&format!("sText_ExclamationMark{k}"), "!".into());
                }
            }
            StrSrc::Sharply { adverb, verb } => {
                put(adverb, "TEXTE!".into());
                put(verb, "TEXTE!".into());
            }
            StrSrc::Buff1 { key, code } => put(key, format!("{code} TEXTE!")),
            StrSrc::TypeNames | StrSrc::StatNames | StrSrc::AbilityNames => {}
        }
    }
    for (i, k) in STAT_NAME_KEYS.iter().enumerate() {
        put(k, format!("STAT{i}"));
    }
    let slot = |species: &str, level: u8| WildMon {
        species: species.into(),
        min_level: level,
        max_level: level,
    };
    let encounters = BTreeMap::from([(
        MAP_NAME.to_string(),
        LandEncounters {
            rate: 20,
            mons: (0..12)
                .map(|i| {
                    if i % 2 == 0 {
                        slot("SPECIES_STUB_A", 2)
                    } else {
                        slot("SPECIES_STUB_B", 3)
                    }
                })
                .collect(),
        },
    )]);
    let mut back_pal = [0u16; 16];
    back_pal[1] = rgb(31, 8, 8);
    BattleIn {
        species: vec![
            species("SPECIES_NONE", "TYPE_NORMAL", &[]),
            species(
                "SPECIES_STUB_A",
                "TYPE_NORMAL",
                &[(1, HIT), (3, "MOVE_STUB_GROWL")],
            ),
            species("SPECIES_STUB_B", "TYPE_NORMAL", &[(1, HIT)]),
        ],
        moves: MovesFile {
            moves,
            nature_power: vec![HIT.into(); 10],
        },
        types: TypesFile {
            names: (0..18).map(|i| format!("TYPE{i}")).collect(),
            chart: vec![
                ("TYPE_FIRE".into(), "TYPE_GRASS".into(), 20),
                ("TYPE_NORMAL".into(), "TYPE_ROCK".into(), 5),
                ("TYPE_NORMAL".into(), "TYPE_GHOST".into(), 0),
            ],
            foresight_split: 2,
            ability_names: (0..78).map(|i| format!("ABILITY{i}")).collect(),
        },
        trainers: vec![Trainer {
            id: "TRAINER_NONE".into(),
            class: "TRAINER_CLASS_PKMN_TRAINER_1".into(),
            class_name: String::new(),
            name: String::new(),
            pic: "TRAINER_PIC_HIKER".into(),
            female: false,
            ai: vec![],
            double: false,
            party: vec![],
        }],
        referenced: Default::default(),
        encounters,
        heal: BTreeMap::from([(
            "HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F".to_string(),
            (MAP_NAME.to_string(), START.0, START.1),
        )]),
        strings,
        mon_gfx: BTreeMap::from([
            ("SPECIES_STUB_A".to_string(), mon_gfx(1)),
            ("SPECIES_STUB_B".to_string(), mon_gfx(2)),
        ]),
        trainer_pics: BTreeMap::new(),
        player_back: PicIn {
            tiles: (0..4).flat_map(|_| pic(1)).collect(),
            palette: back_pal,
            y: 8,
        },
        ui: UiGfx::default(),
        small_glyphs: vec![],
        small_widths: vec![],
        starters: ["SPECIES_STUB_A", "SPECIES_STUB_B", "SPECIES_STUB_A"].map(String::from),
        first_foe: ("SPECIES_STUB_B".into(), 2),
        stub: true,
        struggle: "MOVE_STUB_STRUGGLE".into(),
    }
}
