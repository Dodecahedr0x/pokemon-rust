//! Virtual PPU. Each engine step returns a [`Frame`]: VRAM/palette mutations ([`Cmd`], applied in
//! order) plus the full register state for the next displayed picture (scroll, sprites,
//! brightness). Shells apply a `Frame` at vblank (GBA) or into a software PPU (native) that
//! follows GBA mode-0 rules, so both show identical pixels.

use crate::Tile;
use alloc::vec::Vec;

pub const SCREEN_W: usize = 240;
pub const SCREEN_H: usize = 160;
pub const BG_LAYERS: usize = 4;
/// Entries per side of a BG tilemap (all BGs are 32x32 entries, 256x256 px, 4bpp).
pub const MAP_SIZE: usize = 32;
pub const MAX_OBJS: usize = 128;
/// Tiles of OBJ VRAM (32 KiB of 4bpp tiles, 1D mapping).
pub const OBJ_TILES: usize = 1024;
/// Brightness at which the picture is fully black.
pub const BRIGHTNESS_BLACK: u8 = 16;

/// Fixed VRAM layout shared by every engine module.
pub mod layout {
    /// Field tiles: primary 0..512 and secondary 512..1024, spanning char blocks 0 and 1.
    pub const FIELD_CHAR_BLOCK: u8 = 0;
    /// Dialogue window: frame tiles and glyph canvas.
    pub const TEXT_CHAR_BLOCK: u8 = 2;
    /// BG layer roles. BG0 is the text window, BG1..3 the field's top, middle, bottom layers.
    pub const TEXT_BG: u8 = 0;
    pub const FIELD_TOP_BG: u8 = 1;
    pub const FIELD_MIDDLE_BG: u8 = 2;
    pub const FIELD_BOTTOM_BG: u8 = 3;
    /// Screen block of BG `n` is `SCREEN_BLOCKS[n]`.
    pub const SCREEN_BLOCKS: [u8; 4] = [31, 28, 29, 30];
    /// Priority of BG `n` is `n` (text on top, field bottom at the back).
    pub const BG_PRIORITY: [u8; 4] = [0, 1, 2, 3];
    /// Usual object priority: above the middle field layer, below the top one.
    pub const OBJ_PRIORITY: u8 = 2;
    /// BG palette banks 0..13 belong to the field (`data::PALS_TOTAL`).
    pub const WINDOW_PALETTE: u8 = 15;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BgControl {
    pub enabled: bool,
    /// 0 is frontmost.
    pub priority: u8,
    pub char_block: u8,
    pub screen_block: u8,
}

/// A VRAM or palette write.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Cmd {
    BgPalette {
        bank: u8,
        colors: [u16; 16],
    },
    ObjPalette {
        bank: u8,
        colors: [u16; 16],
    },
    /// Copy `tiles` to BG VRAM starting at tile `first` of char block `char_block` (may run on
    /// into the following char blocks).
    BgTiles {
        char_block: u8,
        first: u16,
        tiles: &'static [Tile],
    },
    BgTile {
        char_block: u8,
        index: u16,
        tile: Tile,
    },
    ObjTiles {
        first: u16,
        tiles: &'static [Tile],
    },
    /// Write `entries` into screen block `screen_block` starting at entry `start` (row-major,
    /// 32 per row).
    MapEntries {
        screen_block: u8,
        start: u16,
        entries: Vec<u16>,
    },
    /// Set all 1024 entries of `screen_block` to `entry`.
    MapFill {
        screen_block: u8,
        entry: u16,
    },
    Bg {
        layer: u8,
        control: BgControl,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ObjShape {
    #[default]
    Square,
    Wide,
    Tall,
}

/// An OAM entry (regular, non-affine sprite). Pixel size is GBA's (shape, size) table, e.g.
/// Tall/2 = 16x32, Square/2 = 32x32.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Obj {
    /// Screen position of the top-left corner. Shells keep 9 bits of x and 8 of y, so it wraps
    /// like OAM does.
    pub x: i16,
    pub y: i16,
    /// First tile in OBJ VRAM.
    pub tile: u16,
    pub palette: u8,
    pub shape: ObjShape,
    pub size: u8,
    pub hflip: bool,
    pub vflip: bool,
    pub priority: u8,
}

impl Obj {
    /// Pixel (width, height) of a GBA (shape, size) pair.
    pub const fn dimensions(shape: ObjShape, size: u8) -> (u8, u8) {
        const SQUARE: [u8; 4] = [8, 16, 32, 64];
        const LONG: [(u8, u8); 4] = [(16, 8), (32, 8), (32, 16), (64, 32)];
        let s = (size & 3) as usize;
        match shape {
            ObjShape::Square => (SQUARE[s], SQUARE[s]),
            ObjShape::Wide => LONG[s],
            ObjShape::Tall => (LONG[s].1, LONG[s].0),
        }
    }

    /// The (shape, size) pair for a sprite of `w`x`h` pixels, if GBA supports it.
    pub const fn shape_for(w: u8, h: u8) -> Option<(ObjShape, u8)> {
        let shapes = [ObjShape::Square, ObjShape::Wide, ObjShape::Tall];
        let mut i = 0;
        while i < 3 {
            let mut size = 0;
            while size < 4 {
                let (dw, dh) = Obj::dimensions(shapes[i], size);
                if dw == w && dh == h {
                    return Some((shapes[i], size));
                }
                size += 1;
            }
            i += 1;
        }
        None
    }
}

/// One step's video output.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Frame {
    /// Applied in order before the picture is shown.
    pub cmds: Vec<Cmd>,
    /// (x, y) scroll of BG0..BG3.
    pub scroll: [(u16, u16); BG_LAYERS],
    /// At most `MAX_OBJS`; index 0 is drawn in front among equal priorities. Unused OAM is hidden.
    pub objs: Vec<Obj>,
    /// 0 = normal, `BRIGHTNESS_BLACK` = black: GBA brightness decrease (BLDY) on every BG,
    /// sprite and the backdrop.
    pub brightness: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_dimensions_round_trip() {
        for shape in [ObjShape::Square, ObjShape::Wide, ObjShape::Tall] {
            for size in 0..4 {
                let (w, h) = Obj::dimensions(shape, size);
                assert_eq!(Obj::shape_for(w, h), Some((shape, size)));
            }
        }
        assert_eq!(Obj::dimensions(ObjShape::Tall, 2), (16, 32));
        assert_eq!(Obj::shape_for(16, 32), Some((ObjShape::Tall, 2)));
        assert_eq!(Obj::shape_for(24, 24), None);
    }
}
