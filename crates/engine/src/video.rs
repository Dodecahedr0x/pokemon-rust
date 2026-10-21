//! Virtual PPU. Each engine step returns a [`Frame`]: VRAM/palette mutations ([`Cmd`], applied in
//! order) plus the full register state for the next displayed picture (scroll, sprites,
//! brightness). Shells apply a `Frame` at vblank (GBA) or into a software PPU (native) that
//! follows GBA mode-0 rules, so both show identical pixels.

use alloc::vec::Vec;

/// 8x8 4bpp tile: one u32 per row, pixel x stored in nibble x (low nibble first), GBA layout.
pub type Tile = [u32; 8];

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

/// BG VRAM bytes (char blocks 0..3; screen blocks 0..31 overlap them).
pub const BG_VRAM_BYTES: usize = 0x1_0000;
const CHAR_BLOCK_BYTES: usize = 0x4000;
const SCREEN_BLOCK_BYTES: usize = 0x800;
const TILE_BYTES: usize = 32;

impl Cmd {
    /// Checks the command against the hardware ranges both shells assume: palette bank < 16,
    /// BG tile and map writes inside BG VRAM, OBJ tiles inside OBJ VRAM, layer < 4 and BG
    /// control fields within their register widths. Native panics on an error; the GBA shell
    /// `debug_assert`s it.
    pub fn validate(&self) -> Result<(), &'static str> {
        let ok = |c: bool, e| if c { Ok(()) } else { Err(e) };
        let bg_tiles = |cb: u8, first: u16, n: usize| {
            ok(
                cb < 4
                    && cb as usize * CHAR_BLOCK_BYTES + (first as usize + n) * TILE_BYTES
                        <= BG_VRAM_BYTES,
                "BG tiles past BG VRAM",
            )
        };
        match self {
            Cmd::BgPalette { bank, .. } | Cmd::ObjPalette { bank, .. } => {
                ok(*bank < 16, "palette bank out of range")
            }
            Cmd::BgTiles {
                char_block,
                first,
                tiles,
            } => bg_tiles(*char_block, *first, tiles.len()),
            Cmd::BgTile {
                char_block, index, ..
            } => bg_tiles(*char_block, *index, 1),
            Cmd::ObjTiles { first, tiles } => ok(
                *first as usize + tiles.len() <= OBJ_TILES,
                "OBJ tiles past OBJ VRAM",
            ),
            Cmd::MapEntries {
                screen_block,
                start,
                entries,
            } => ok(
                *screen_block < 32
                    && *screen_block as usize * SCREEN_BLOCK_BYTES
                        + (*start as usize + entries.len()) * 2
                        <= BG_VRAM_BYTES,
                "map entries past BG VRAM",
            ),
            Cmd::MapFill { screen_block, .. } => {
                ok(*screen_block < 32, "screen block out of range")
            }
            Cmd::Bg { layer, control } => ok(
                (*layer as usize) < BG_LAYERS
                    && control.priority < 4
                    && control.char_block < 4
                    && control.screen_block < 32,
                "BG layer or control out of range",
            ),
        }
    }
}

impl Obj {
    /// Fields within their OAM widths (x/y wrap and are always valid).
    pub fn validate(&self) -> Result<(), &'static str> {
        if (self.tile as usize) < OBJ_TILES
            && self.palette < 16
            && self.size < 4
            && self.priority < 4
        {
            Ok(())
        } else {
            Err("obj tile/palette/size/priority out of range")
        }
    }
}

impl Frame {
    /// Every cmd and obj valid and at most `MAX_OBJS` objs.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.objs.len() > MAX_OBJS {
            return Err("more than MAX_OBJS objs");
        }
        self.cmds.iter().try_for_each(Cmd::validate)?;
        self.objs.iter().try_for_each(Obj::validate)
    }
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

    #[test]
    fn validate_checks_ranges() {
        static T: [Tile; 2] = [[0; 8]; 2];
        let pal = |bank| Cmd::BgPalette {
            bank,
            colors: [0; 16],
        };
        assert!(pal(15).validate().is_ok());
        assert!(pal(16).validate().is_err());
        let bg = |char_block, first| Cmd::BgTiles {
            char_block,
            first,
            tiles: &T,
        };
        assert!(bg(0, 510).validate().is_ok()); // runs on into char block 1
        assert!(bg(3, 510).validate().is_ok());
        assert!(bg(3, 511).validate().is_err());
        assert!(bg(4, 0).validate().is_err());
        let obj = |first| Cmd::ObjTiles { first, tiles: &T };
        assert!(obj(1022).validate().is_ok());
        assert!(obj(1023).validate().is_err());
        let map = |screen_block, start, n| Cmd::MapEntries {
            screen_block,
            start,
            entries: alloc::vec![0; n],
        };
        assert!(map(31, 0, 1024).validate().is_ok());
        assert!(map(31, 1, 1024).validate().is_err());
        assert!(map(32, 0, 1).validate().is_err());
        let ctl = |layer, priority| Cmd::Bg {
            layer,
            control: BgControl {
                enabled: true,
                priority,
                char_block: 0,
                screen_block: 0,
            },
        };
        assert!(ctl(3, 3).validate().is_ok());
        assert!(ctl(4, 0).validate().is_err());
        assert!(ctl(0, 4).validate().is_err());
        let mut f = Frame::default();
        f.objs.push(Obj::default());
        assert!(f.validate().is_ok());
        f.objs[0].palette = 16;
        assert!(f.validate().is_err());
        f.objs = alloc::vec![Obj::default(); MAX_OBJS + 1];
        assert!(f.validate().is_err());
    }
}
