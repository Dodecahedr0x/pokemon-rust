//! Synthetic `BattleGfx` for the gfx unit tests: every tile is distinct (`tile(tag, i)`).

use super::super::{BattleGfx, GfxImage};
use crate::Tile;
use crate::data::{
    Behavior, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId, UiText,
    WindowGfx,
};
use crate::mon::{BattleTables, MonGfx, SpeciesInfo, TrainerPic};
use crate::script::{Ctx, Script};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

/// A tile whose rows all read `tag << 16 | i` (unique per graphic and index).
pub fn tile(tag: u32, i: usize) -> Tile {
    [tag << 16 | i as u32; 8]
}

pub fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

pub fn image(tag: u32, n: usize, palettes: usize) -> GfxImage {
    GfxImage {
        tiles: leak((0..n).map(|i| tile(tag, i)).collect()),
        palettes: leak(
            (0..palettes)
                .map(|p| core::array::from_fn(|c| (tag as u16) << 8 | (p as u16) << 4 | c as u16))
                .collect(),
        ),
        tilemap: &[],
    }
}

/// A `.latfont` glyph with the given (x, y, 2bpp value) pixels.
pub fn glyph(pixels: &[(usize, usize, u16)]) -> [u8; 64] {
    let mut g = [0u8; 64];
    for &(x, y, v) in pixels {
        let off = ((y / 8) * 2 + x / 8) * 16 + (y % 8) * 2;
        let w = u16::from_le_bytes([g[off], g[off + 1]]) | (v << (14 - 2 * (x % 8)));
        g[off..off + 2].copy_from_slice(&w.to_le_bytes());
    }
    g
}

/// Small font: every glyph 5 px wide with fg at (0, 3) and shadow at (1, 12); digits
/// (0xA1..=0xAA) also mark their value at row 6, column `d % 4`.
pub fn small_font() -> &'static Font {
    let mut glyphs = vec![glyph(&[(0, 3, 1), (1, 12, 2)]); 0x200];
    let widths = vec![5u8; 0x200];
    for d in 0..10usize {
        glyphs[0xA1 + d] = glyph(&[(0, 3, 1), (1, 12, 2), (d % 4, 6, 1), (4, 4 + d % 8, 2)]);
    }
    glyphs[0x77] = [0; 64]; // CHAR_SPACER: blank
    Box::leak(Box::new(Font {
        glyphs: leak(glyphs),
        widths: leak(widths),
    }))
}

pub const TAG_TEXTBOX: u32 = 1;
pub const TAG_ENV: u32 = 2;
pub const TAG_BOX_PLAYER: u32 = 3;
pub const TAG_BOX_ENEMY: u32 = 4;
pub const TAG_HPBAR: u32 = 5;
pub const TAG_EXPBAR: u32 = 6;
pub const TAG_STATUS: u32 = 7;
pub const TAG_MISC: u32 = 8;
pub const TAG_BALL: u32 = 10;
pub const TAG_BACK: u32 = 11;
pub const TAG_MON_FRONT: u32 = 12;
pub const TAG_MON_BACK: u32 = 13;
pub const TAG_TRAINER: u32 = 14;

/// Environment `Grass`: 300 tiles, 3 palettes, a 64x32 tilemap (2048 entries).
pub fn gfx() -> &'static BattleGfx {
    let mut env = image(TAG_ENV, 300, 3);
    env.tilemap = leak((0..2048u16).collect());
    let mut environments = [GfxImage::EMPTY; 10];
    environments[0] = env;
    Box::leak(Box::new(BattleGfx {
        textbox: image(TAG_TEXTBOX, 4, 2),
        environments,
        healthbox_player: image(TAG_BOX_PLAYER, 128, 1),
        healthbox_opponent: image(TAG_BOX_ENEMY, 64, 1),
        hpbar: image(TAG_HPBAR, 30, 1),
        expbar: image(TAG_EXPBAR, 9, 0),
        status: image(TAG_STATUS, 30, 0),
        misc: image(TAG_MISC, 12, 0),
        ball: image(TAG_BALL, 12, 1),
        small_font: small_font(),
        player_back: TrainerPic {
            tiles: leak((0..256).map(|i| tile(TAG_BACK, i)).collect()),
            palette: Box::leak(Box::new([0x300; 16])),
            y: 4,
        },
        starter_bag: GfxImage::EMPTY,
        starter_ball: GfxImage::EMPTY,
        starter_circle: GfxImage::EMPTY,
    }))
}

pub fn mon_gfx(front_y: u8, back_y: u8, elevation: u8) -> &'static MonGfx {
    Box::leak(Box::new(MonGfx {
        front: leak((0..64).map(|i| tile(TAG_MON_FRONT, i)).collect()),
        back: leak((0..64).map(|i| tile(TAG_MON_BACK, i)).collect()),
        palette: Box::leak(Box::new(core::array::from_fn(|c| 0x100 + c as u16))),
        shiny: Box::leak(Box::new(core::array::from_fn(|c| 0x200 + c as u16))),
        front_y,
        back_y,
        elevation,
    }))
}

pub fn trainer_pic() -> &'static TrainerPic {
    Box::leak(Box::new(TrainerPic {
        tiles: leak((0..64).map(|i| tile(TAG_TRAINER, i)).collect()),
        palette: Box::leak(Box::new([0x400; 16])),
        y: 1,
    }))
}

/// A game with species 1 (`gfx` given) and the synthetic battle gfx.
pub struct Game {
    pub gfx: &'static BattleGfx,
    pub tables: &'static BattleTables,
}

pub fn game(mon: Option<&'static MonGfx>) -> Game {
    use crate::mon::{Ability, GrowthRate, Type};
    let info = SpeciesInfo {
        name: &[0xFF],
        base: [0; 6],
        types: [Type::Normal; 2],
        catch_rate: 0,
        exp_yield: 0,
        ev_yield: [0; 6],
        gender_ratio: 0,
        friendship: 0,
        growth: GrowthRate::MediumFast,
        abilities: [Ability::None; 2],
        learnset: &[],
        gfx: None,
    };
    let species = leak(vec![info, SpeciesInfo { gfx: mon, ..info }]);
    Game {
        gfx: gfx(),
        tables: Box::leak(Box::new(BattleTables {
            species,
            ..crate::mon::EMPTY_TABLES
        })),
    }
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
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        unimplemented!()
    }
    fn battle(&self) -> &'static BattleTables {
        self.tables
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        self.gfx
    }
}
