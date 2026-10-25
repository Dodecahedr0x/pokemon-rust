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

    /// The battle's own VRAM layout, used while a battle owns the screen.
    pub mod battle {
        // BG: (layer, char block, screen block, priority). 32x32 maps only: Gen3's 64-row BG0
        // trick (scrolling between message / action / move views) becomes redrawing rows 14..20
        // (`battle::ui::FRAME_ROWS`).
        /// Message box, action and move menus.
        pub const TEXT: (u8, u8, u8, u8) = (0, 0, 24, 0);
        /// The yes/no box (`battle::ui`; Gen3 has it on BG0).
        pub const WINDOW: (u8, u8, u8, u8) = (1, 1, 28, 0);
        /// Reserved for move animations (char block 1 from tile 256): M4 has none, so the
        /// scene keeps this BG disabled. The field-to-battle transition runs on the field
        /// layout (`battle::transition`), not here.
        pub const ANIM: (u8, u8, u8, u8) = (2, 1, 30, 1);
        /// Environment background.
        pub const ENV: (u8, u8, u8, u8) = (3, 2, 26, 3);
        // BG palettes: 0..2 textbox (`gBattleTextboxPalette`), 2..5 environment, 5 menu text
        // (`gBattleWindowTextPalette`, PP colors swapped into colors 11/12; `battle::ui`);
        // 6..16 unused, bank 15 included (the yes/no box on BG1 uses banks 1 and 5).
        // OBJ tiles (1D, 1024), `battle::gfx::obj`: enemy pic 0, player pic 64, opponent
        // trainer 128, player trainer back 192 (64 each), player healthbox 256..384 (two 64x64
        // halves: its exp bar row is below 32 px), enemy healthbox 384..448 (two 64x32 halves),
        // HP bars 448 / 456 (8 tiles each), ball 464 (one 16x16 frame), 468..512 free; 512..
        // the starter screen (`starter`), not used during a battle.
        // OBJ palettes: 0 enemy mon, 1 player mon, 2 opponent trainer, 3 player back, 4 player
        // healthbox, 5 HP bars, 6 ball, 7 enemy healthbox; 8..16 unused. Stat tints blend the
        // mon's own bank (0 or 1) in place.
        pub const OBJ_PRIORITY_MON: u8 = 2;
        pub const OBJ_PRIORITY_BOX: u8 = 1;
    }

    /// The VRAM layout of full-screen screens (Milestone 9: party, bag, summary, mart, PC,
    /// options, main menu, evolution), used while a screen owns the screen. Folded from the
    /// decomp's BG templates (`sPartyMenuBgTemplates`, `sBgTemplates_ItemMenu`, the summary's
    /// `sBgTemplates`): Gen3's windows layer on top, a foreground layer (slots, pages) and the
    /// background picture, on 32x32 maps as the battle layout does (Gen3's 64-wide summary
    /// map scrolls between pages; ours redraws).
    pub mod menu {
        // BG: (layer, char block, screen block, priority).
        /// Text windows and frames (Gen3 BG0 of the party menu and bag).
        pub const WINDOWS: (u8, u8, u8, u8) = (0, 0, 31, 0);
        /// Foreground picture: party slots, summary page, bag sprite area.
        pub const FOREGROUND: (u8, u8, u8, u8) = (1, 1, 30, 1);
        /// Background picture.
        pub const BACKGROUND: (u8, u8, u8, u8) = (2, 2, 29, 2);
        /// Spare layer (scrolling backdrops, the evolution scene's sparkles), off by default.
        pub const SPARE: (u8, u8, u8, u8) = (3, 2, 28, 3);
        /// BG tiles per char block for the windows layer's frame (`gUserFrames`, 9 tiles) and
        /// the standard message frame, then the text canvases.
        pub const WINDOW_FRAME_BASE: u16 = 0x1F0;
        // BG palettes: 0..13 the pictures, 14 the menu text (`gStandardMenuPalette`), 15 the
        // window frame (`layout::WINDOW_PALETTE`, shared with the field).
        pub const TEXT_PALETTE: u8 = 14;
        pub const FRAME_PALETTE: u8 = super::WINDOW_PALETTE;
        // OBJ tiles (1D, 1024): mon icons 0..192 (six icons of two 32x32 frames), item icon
        // 192..208, cursor and scroll arrows 208..240, the bag sprite 256..512, 512.. free.
        pub const OBJ_MON_ICONS: u16 = 0;
        pub const OBJ_ITEM_ICON: u16 = 192;
        pub const OBJ_CURSOR: u16 = 208;
        pub const OBJ_BAG: u16 = 256;
        // OBJ palettes: 0..3 the mon icon palettes (`gMonIconPalettes`), 3 the item icon,
        // 4 cursor and arrows, 5 the bag sprite, 6..16 free.
        pub const OBJ_PAL_MON_ICONS: u8 = 0;
        pub const OBJ_PAL_ITEM_ICON: u8 = 3;
        pub const OBJ_PAL_CURSOR: u8 = 4;
        pub const OBJ_PAL_BAG: u8 = 5;
    }
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
    /// One dynamic OBJ tile (healthboxes, bars).
    ObjTile {
        index: u16,
        tile: Tile,
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

/// An OAM entry. Pixel size is GBA's (shape, size) table, e.g. Tall/2 = 16x32, Square/2 = 32x32.
/// `scale == 0` is a regular sprite. Any other value is Gen3's displayed affine scale in 8.8
/// (`0x100` is 1×); the hardware matrix is `0x10000 / scale` and flips are ignored.
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
    /// Gen3 displayed affine scale, 8.8 (`0x100` = 1×). `0` draws a regular sprite.
    pub scale: u16,
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
    /// `brightness` brightens towards white instead (BLDCNT effect 2): `BRIGHTNESS_BLACK` is then
    /// fully white.
    pub lighten: bool,
    /// This step did bulk work meant for a black screen (a map load, a whole-view redraw, a
    /// starter or battle loading chunk, the field's restore): the GBA may overrun the frame
    /// here. Not applied by the shells; the parity test checks every black overrun against it.
    pub bulk: bool,
    /// The step's sound engine calls, in order (Milestone 12, [`crate::audio`]).
    pub sound: Vec<sound::Cmd>,
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
            Cmd::ObjTile { index, .. } => {
                ok((*index as usize) < OBJ_TILES, "OBJ tile past OBJ VRAM")
            }
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
    /// Whether every pixel is black: full brightness decrease (not increase).
    pub const fn is_black(&self) -> bool {
        self.brightness >= BRIGHTNESS_BLACK && !self.lighten
    }

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
mod tests;
