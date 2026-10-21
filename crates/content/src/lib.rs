#![cfg_attr(not(test), no_std)]
//! The game: all Hoenn-specific data and scripts. Depends on `engine`, never the reverse.
//!
//! `build/main.rs` compiles `content/extracted/` into `consts` (Gen3 constants, text ids),
//! `data` (static tables) and `scripts` (the generated per-map script modules). Without an
//! extracted dir it builds a small built-in world and sets `cfg(pkr_stub)`.

extern crate alloc;

use engine::data::{
    Behavior, Dir, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId,
    UiText, WindowGfx,
};
use engine::script::{Ctx, Script};
use engine::{Content, Scene, Tile};

pub mod prelude;

/// Gen3 constants (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
/// `OBJ_EVENT_GFX_*`) and one `TextId` per text, named after its key.
#[allow(non_upper_case_globals, dead_code, unused_imports)]
pub mod consts {
    use engine::data::{FlagId, MapId, TextId, VarId};
    include!(concat!(env!("OUT_DIR"), "/consts.rs"));
}

/// Static tables: tilesets (`TS_*`), maps (`M_*`), sprites (`S_*`), font, window, texts.
#[allow(non_upper_case_globals, non_snake_case, dead_code, clippy::all)]
pub mod data {
    include!(concat!(env!("OUT_DIR"), "/data.rs"));
}

/// Generated scripts, one module per map file (`include!`d from the extracted dir).
pub mod scripts {
    include!(concat!(env!("OUT_DIR"), "/scripts.rs"));
}

pub use data::script_ids;

pub const T_GRASS: u16 = 0;
pub const T_CHECKER: u16 = 1;
pub const T_BORDER: u16 = 2;
pub const T_DOT: u16 = 3;

const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

const PALETTE: [u16; 16] = {
    let mut p = [0u16; 16];
    p[0] = rgb(0, 0, 0);
    p[1] = rgb(8, 24, 8);
    p[2] = rgb(14, 28, 10);
    p[3] = rgb(4, 8, 16);
    p[4] = rgb(31, 28, 4);
    p
};

static TILES: [Tile; 4] = [
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
];

const fn build_map() -> [u16; 1024] {
    let mut m = [0u16; 1024];
    let mut i = 0;
    while i < 1024 {
        let (x, y) = (i % 32, i / 32);
        m[i] = if x == 0 || y == 0 || x == 31 || y == 31 {
            T_BORDER
        } else if x % 8 == 3 && y % 8 == 3 {
            T_DOT
        } else if (x / 4 + y / 4) % 2 == 0 {
            T_GRASS
        } else {
            T_CHECKER
        };
        i += 1;
    }
    m
}

static MAP: [u16; 1024] = build_map();

static SCENE: Scene = Scene {
    palette: PALETTE,
    tiles: &TILES,
    map: &MAP,
};

/// The game's content root.
pub struct Emerald;

impl Content for Emerald {
    fn scene(&self) -> &Scene {
        &SCENE
    }
}

/// Whether `id` is an exported map. `GameData::map` gives [`data::MISSING_MAP`] (0x0 cells,
/// no events) for the others, e.g. warp destinations outside the slice.
pub fn has_map(id: MapId) -> bool {
    data::map(id.0).is_some()
}

impl GameData for Emerald {
    fn map(&self, id: MapId) -> &'static MapData {
        data::map(id.0).unwrap_or(&data::MISSING_MAP)
    }

    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        data::sprite(gfx.0)
    }

    fn player_sprite(&self) -> &'static SpriteData {
        data::PLAYER_SPRITE
    }

    fn font(&self) -> &'static Font {
        &data::FONT
    }

    fn window(&self) -> &'static WindowGfx {
        &data::WINDOW
    }

    fn text(&self, id: TextId) -> &'static [u8] {
        data::TEXTS.get(id.0 as usize).copied().unwrap_or(&[0xFF])
    }

    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => data::UI_YES,
            UiText::No => data::UI_NO,
            UiText::RivalName => data::UI_RIVAL_NAME,
        }
    }

    fn behavior(&self, raw: u8) -> Behavior {
        data::BEHAVIORS[raw as usize]
    }

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        data::script(id.0, ctx)
    }

    /// Placeholder (Task I decides): Littleroot, outside the player's house, facing down.
    fn new_game(&self) -> NewGame {
        let (map, x, y) = data::NEW_GAME;
        NewGame {
            map,
            x,
            y,
            facing: Dir::Down,
            script: None,
            player_name: data::PLAYER_NAME,
        }
    }
}

#[cfg(test)]
mod data_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Content;

    #[test]
    fn every_map_entry_points_at_a_tile() {
        let s = Emerald.scene();
        assert!(
            s.map
                .iter()
                .all(|&e| engine::scene::map_tile_index(e) < s.tiles.len())
        );
    }

    /// `World::initial_frame` uploads the scene palette to bank 0 only.
    #[test]
    fn map_uses_palette_bank_zero() {
        assert!(Emerald.scene().map.iter().all(|&e| e & 0xF000 == 0));
    }

    #[test]
    fn palette_is_rgb555() {
        assert!(Emerald.scene().palette.iter().all(|&c| c <= 0x7FFF));
    }

    #[test]
    fn map_has_border_and_checker() {
        let m = Emerald.scene().map;
        assert_eq!(m[0], T_BORDER);
        assert_eq!(m[32 + 1], T_GRASS);
        assert_eq!(m[32 + 4], T_CHECKER);
        assert_eq!(m[3 * 32 + 3 + 8], T_DOT);
    }
}
