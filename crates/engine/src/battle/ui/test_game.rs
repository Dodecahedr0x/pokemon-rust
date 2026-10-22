//! A synthetic `GameData` for the battle text and window tests: the first species and moves of
//! the shared `battle::test_data` tables with ASCII names, a tiny font and placeholder battle
//! strings (Gen3 encoding, ASCII letters via [`enc`]).

use crate::battle::test_data::{MOVES, SPECIES, TABLES};
use crate::battle::{BattleGfx, BattleMon, EMPTY_GFX, GfxImage, Str};
use crate::data::{
    Behavior, Dir, Font, GameData, GfxId, MapData, MapId, MoveId, NewGame, ScriptId, SpeciesId,
    SpriteData, TextId, UiText, WindowGfx,
};
use crate::mon::{
    BattleTables, Gender, MoveInfo, NAME_LEN, Party, Pokemon, SpeciesInfo, Stat, Status, Type,
};
use crate::script::{Ctx, Script};
use crate::video::Tile;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

/// Gen3 encoding of ASCII letters, digits, space and `/`, 0xFF-terminated.
pub fn enc(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s
        .bytes()
        .map(|c| match c {
            b'A'..=b'Z' => 0xBB + (c - b'A'),
            b'a'..=b'z' => 0xD5 + (c - b'a'),
            b'0'..=b'9' => 0xA1 + (c - b'0'),
            b'/' => 0xBA,
            b'?' => 0xAC,
            b'!' => 0xAB,
            _ => 0x00,
        })
        .collect();
    v.push(0xFF);
    v
}

pub fn leak(v: Vec<u8>) -> &'static [u8] {
    Box::leak(v.into_boxed_slice())
}

/// `enc(s)` with `{…}` placeholders spliced in: `parts` alternate text and raw byte runs.
pub fn msg_text(parts: &[&[u8]]) -> &'static [u8] {
    let mut v = Vec::new();
    for p in parts {
        v.extend_from_slice(p);
    }
    v.push(0xFF);
    leak(v)
}

/// Text without its 0xFF.
pub fn t(s: &str) -> Vec<u8> {
    let mut v = enc(s);
    v.pop();
    v
}

pub const FD: u8 = 0xFD;

pub use crate::battle::test_data::{EMBER, GROWL, TACKLE};
pub const POOCHY: SpeciesId = SpeciesId(1);
pub const TORCH: SpeciesId = SpeciesId(2);

/// Glyph `c` is `c % 7 + 2` px wide with its fg pixel at (0, 0), so lines are easy to spot.
fn font() -> &'static Font {
    let mut glyphs = vec![[0u8; 64]; 0x200];
    let mut widths = vec![0u8; 0x200];
    for c in 1..0x1F7usize {
        glyphs[c][1] = 0x40;
        widths[c] = (c % 7 + 2) as u8;
    }
    Box::leak(Box::new(Font {
        glyphs: Box::leak(glyphs.into_boxed_slice()),
        widths: Box::leak(widths.into_boxed_slice()),
    }))
}

/// Textbox tiles: tile `i` is filled with color `i % 16` (so map entries are recognisable).
fn gfx() -> &'static BattleGfx {
    let tiles: Vec<Tile> = (0..0x100u32).map(|i| [(i % 16) * 0x1111_1111; 8]).collect();
    let mut map = vec![0u16; 64 * 32];
    for (i, e) in map.iter_mut().enumerate() {
        // Rows 14..20 of each view carry their row and column, palette 1 from row 34 on.
        let (row, col) = (i / 32, i % 32);
        if row % 20 >= 14 {
            *e = ((row as u16) << 5 | col as u16) & 0x3FF | if row >= 34 { 0x1000 } else { 0 };
        }
    }
    let mut pal5 = [0u16; 16];
    for (i, c) in pal5.iter_mut().enumerate() {
        *c = 0x500 + i as u16;
    }
    let mut pp = [0u16; 16];
    for (i, c) in pp.iter_mut().enumerate() {
        *c = 0x700 + i as u16;
    }
    let palettes = Box::leak(vec![[0x100u16; 16], [0x200; 16], pal5, pp].into_boxed_slice());
    Box::leak(Box::new(BattleGfx {
        textbox: GfxImage {
            tiles: Box::leak(tiles.into_boxed_slice()),
            palettes,
            tilemap: Box::leak(map.into_boxed_slice()),
        },
        small_font: font(),
        ..EMPTY_GFX
    }))
}

pub struct Game {
    pub font: &'static Font,
    pub tables: &'static BattleTables,
    pub gfx: &'static BattleGfx,
    pub window: &'static WindowGfx,
    /// `Str::WildPrefix`.
    pub wild_tag: &'static [u8],
}

pub fn game() -> Game {
    let named =
        |names: &[&str]| -> Vec<&'static [u8]> { names.iter().map(|n| leak(enc(n))).collect() };
    let moves: Vec<MoveInfo> = MOVES
        .iter()
        .zip(named(&["NONE", "TACKLE", "GROWL", "EMBER"]))
        .map(|(m, name)| MoveInfo { name, ..*m })
        .collect();
    let species: Vec<SpeciesInfo> = SPECIES
        .iter()
        .zip(named(&["NONE", "POOCHY", "TORCH"]))
        .map(|(s, name)| SpeciesInfo { name, ..*s })
        .collect();
    Game {
        font: font(),
        tables: Box::leak(Box::new(BattleTables {
            species: Box::leak(species.into_boxed_slice()),
            moves: Box::leak(moves.into_boxed_slice()),
            ..TABLES
        })),
        gfx: gfx(),
        window: Box::leak(Box::new(WindowGfx {
            tiles: &[],
            palette: &[0; 16],
            text_fg: 2,
            text_shadow: 3,
            text_bg: 1,
        })),
        wild_tag: leak(enc(" wild")),
    }
}

pub fn mon(name: &str, species: SpeciesId, moves: [Option<MoveId>; 4], pp: [u8; 4]) -> BattleMon {
    let mut nickname = [0xFF; NAME_LEN];
    for (i, b) in t(name).into_iter().take(NAME_LEN - 1).enumerate() {
        nickname[i] = b;
    }
    BattleMon {
        party: 0,
        species,
        nickname,
        level: 5,
        hp: 20,
        max_hp: 20,
        status: Status::None,
        gender: Gender::Male,
        moves,
        pp,
        max_pp: [35, 40, 25, 10],
    }
}

/// A leaked party whose slots carry `nicknames`.
pub fn party(nicknames: &[&str]) -> &'static Party {
    let mut p = Party::default();
    for name in nicknames {
        let mut nickname = [0xFF; NAME_LEN];
        for (i, b) in t(name).into_iter().take(NAME_LEN - 1).enumerate() {
            nickname[i] = b;
        }
        let _ = p.push(Pokemon {
            species: POOCHY,
            personality: 0,
            ot_id: 0,
            nickname,
            level: 5,
            exp: 0,
            ivs: [0; 6],
            evs: [0; 6],
            moves: [None; 4],
            pp: [0; 4],
            pp_bonuses: 0,
            friendship: 0,
            ability_slot: 0,
            status: Status::None,
            hp: 20,
            stats: [20; 6],
        });
    }
    Box::leak(Box::new(p))
}

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        self.font
    }
    fn window(&self) -> &'static WindowGfx {
        self.window
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
        Box::pin(async {})
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
    fn battle(&self) -> &'static BattleTables {
        self.tables
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.gfx
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        let raw = |v: Vec<u8>| leak(v);
        match s {
            // French-style suffix tags (leading space), as the FR ROM's sText_WildPkmnPrefix.
            Str::WildPrefix => self.wild_tag,
            Str::FoePrefix => raw(enc(" foe")),
            Str::StatName(Stat::Atk) => raw(enc("ATTACK")),
            Str::StatName(_) => raw(enc("STAT")),
            Str::TypeName(Type::Fire) => raw(enc("FIRE")),
            Str::TypeName(_) => raw(enc("NORMAL")),
            Str::StatRose => raw(enc("rose!")),
            Str::BattleMenu => {
                // "FIGHT{CLEAR_TO 56}BAG\nPKMN{CLEAR_TO 56}RUN"
                let mut v = t("FIGHT");
                v.extend([0xFC, 0x13, 56]);
                v.extend(t("BAG"));
                v.push(0xFE);
                v.extend(t("PKMN"));
                v.extend([0xFC, 0x13, 56]);
                v.extend(t("RUN"));
                v.push(0xFF);
                raw(v)
            }
            Str::WhatWillPkmnDo => {
                // "What will\n{B_ACTIVE_NAME_WITH_PREFIX} do?"
                let mut v = t("What will");
                v.extend([0xFE, FD, 0x12]);
                v.extend(t(" do?"));
                v.push(0xFF);
                raw(v)
            }
            Str::MoveInterfacePP => raw(enc("PP ")),
            Str::MoveInterfaceType => raw(enc("TYPE/")),
            Str::BattleYesNoChoice => {
                let mut v = vec![0xFC, 0x05, 5, 0xFC, 0x04, 13, 14, 15];
                v.extend(t("YES"));
                v.push(0xFE);
                v.extend(t("NO"));
                v.push(0xFF);
                raw(v)
            }
            _ => &[0xFF],
        }
    }
}
