#![cfg_attr(not(test), no_std)]
//! The game: all Hoenn-specific data and scripts. Depends on `engine`, never the reverse.

use engine::{Content, Scene, Tile};

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

    /// The native renderer ignores flip and palette-bank bits, so content must
    /// not use them until it supports them (or GBA and native would diverge).
    #[test]
    fn map_uses_no_flip_or_palette_bank_bits() {
        assert!(Emerald.scene().map.iter().all(|&e| e & 0xFC00 == 0));
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
