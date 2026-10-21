//! Applies an engine `video::Frame` to the GBA's PPU.
//!
//! [`Video::apply`] runs at vblank: it first writes the registers (BGxCNT/DISPCNT, scroll, OAM
//! from a 128-entry shadow, BLDCNT/BLDY), which are small and must land inside vblank, then the
//! frame's VRAM/palette commands in order. Bulk copies (tile runs, map rows, OAM) use DMA3,
//! which halts the CPU and moves one word (or halfword) per transfer; small writes are plain
//! word stores. Large uploads (a whole tileset) can exceed vblank: the engine does them while
//! the screen is black. The shell counts late applies in the debug region: `late_black` when
//! the screen stays black before and after (nothing visible can differ), `late` otherwise.
//!
//! Commands are checked with `Frame::validate` in debug builds (native panics on the same
//! errors). In release, register fields are masked to their widths; out-of-range VRAM writes
//! are the caller's bug.

use engine::video::{BG_LAYERS, BRIGHTNESS_BLACK, BgControl, Cmd, Frame, MAX_OBJS, Obj, ObjShape};

const DISPCNT: usize = 0x0400_0000;
const BGCNT: usize = 0x0400_0008;
/// BGxHOFS/BGxVOFS pairs, 4 bytes apart.
const BGOFS: usize = 0x0400_0010;
const BLDCNT: usize = 0x0400_0050;
const BLDY: usize = 0x0400_0054;
const DMA3SAD: usize = 0x0400_00D4;
const DMA3DAD: usize = 0x0400_00D8;
const DMA3CNT: usize = 0x0400_00DC;

const BG_PALETTE: usize = 0x0500_0000;
const OBJ_PALETTE: usize = 0x0500_0200;
const VRAM: usize = 0x0600_0000;
const OBJ_VRAM: usize = 0x0601_0000;
const OAM: usize = 0x0700_0000;
const CHAR_BLOCK_BYTES: usize = 0x4000;
const SCREEN_BLOCK_BYTES: usize = 0x800;
const TILE_BYTES: usize = 32;

const DISPCNT_FORCED_BLANK: u16 = 1 << 7;
/// Mode 0, OBJ 1D mapping, OBJ layer on.
pub const DISPCNT_BASE: u16 = (1 << 6) | (1 << 12);
const DISPCNT_BG0: u16 = 1 << 8;
/// Brightness decrease on BG0-3, OBJ and backdrop; with BLDY = 0 it is the identity.
pub const BLDCNT_DARKEN_ALL: u16 = 0x3F | (3 << 6);
const OAM_HIDDEN: u16 = 1 << 9;

const DMA_ENABLE: u32 = 1 << 31;
const DMA_32BIT: u32 = 1 << 26;
const DMA_SRC_FIXED: u32 = 2 << 23;
/// DMA3's count field is 16 bits.
const DMA_MAX_UNITS: usize = 0x8000;

/// # Safety
/// `addr` must be a writable, aligned I/O register or memory address.
unsafe fn write16(addr: usize, v: u16) {
    unsafe { (addr as *mut u16).write_volatile(v) }
}

/// # Safety
/// As [`write16`].
unsafe fn write32(addr: usize, v: u32) {
    unsafe { (addr as *mut u32).write_volatile(v) }
}

/// Copies `units` words (32-bit flag) or halfwords with DMA3 from `src` to `dst`, both aligned
/// to the unit. With `DMA_SRC_FIXED`, `src` is repeated (a fill).
///
/// # Safety
/// Both ranges must be valid for the transfer.
unsafe fn dma3(src: usize, dst: usize, units: usize, flags: u32) {
    let unit = if flags & DMA_32BIT != 0 { 4 } else { 2 };
    let fixed = flags & DMA_SRC_FIXED != 0;
    let mut done = 0;
    while done < units {
        let n = (units - done).min(DMA_MAX_UNITS);
        let s = if fixed {
            src
        } else {
            src.wrapping_add(done * unit)
        };
        unsafe {
            write32(DMA3SAD, s as u32);
            write32(DMA3DAD, dst.wrapping_add(done * unit) as u32);
            write32(DMA3CNT, DMA_ENABLE | flags | n as u32);
            // Immediate DMA halts the CPU; reading the control back covers its 2-cycle start.
            while (DMA3CNT as *const u32).read_volatile() & DMA_ENABLE != 0 {}
        }
        done += n;
    }
}

/// Word copy to VRAM/palette RAM at `dst`: plain stores up to 8 words, DMA3 beyond.
///
/// # Safety
/// `dst..dst + 4 * src.len()` must be valid, word-aligned video memory.
unsafe fn copy_words(src: &[u32], dst: usize) {
    if src.len() <= 8 {
        for (i, &w) in src.iter().enumerate() {
            unsafe { write32(dst.wrapping_add(4 * i), w) };
        }
    } else {
        unsafe { dma3(src.as_ptr() as usize, dst, src.len(), DMA_32BIT) };
    }
}

fn palette_words(colors: &[u16; 16]) -> [u32; 8] {
    core::array::from_fn(|i| colors[2 * i] as u32 | (colors[2 * i + 1] as u32) << 16)
}

fn bg_tile_addr(char_block: u8, tile: u16) -> usize {
    VRAM + (char_block & 3) as usize * CHAR_BLOCK_BYTES + tile as usize * TILE_BYTES
}

fn screen_block(sb: u8) -> usize {
    VRAM + (sb & 31) as usize * SCREEN_BLOCK_BYTES
}

fn bgcnt(c: BgControl) -> u16 {
    // 4bpp, 256x256 (size 0), no mosaic.
    (c.priority & 3) as u16 | ((c.char_block & 3) as u16) << 2 | ((c.screen_block & 31) as u16) << 8
}

fn oam_entry(o: &Obj) -> [u16; 4] {
    let shape = match o.shape {
        ObjShape::Square => 0,
        ObjShape::Wide => 1,
        ObjShape::Tall => 2,
    };
    [
        (o.y as u16 & 0xFF) | shape << 14,
        (o.x as u16 & 0x1FF)
            | (o.hflip as u16) << 12
            | (o.vflip as u16) << 13
            | ((o.size & 3) as u16) << 14,
        (o.tile & 0x3FF) | ((o.priority & 3) as u16) << 10 | ((o.palette & 15) as u16) << 12,
        0,
    ]
}

/// OAM shadow: 128 entries x 4 halfwords (attr0..2 + the affine slot, left 0). Word aligned
/// for the 32-bit DMA.
#[repr(C, align(4))]
struct OamShadow([u16; MAX_OBJS * 4]);

pub struct Video {
    dispcnt: u16,
    oam: OamShadow,
}

impl Video {
    pub const fn new() -> Self {
        Self {
            dispcnt: DISPCNT_BASE,
            oam: OamShadow([0; MAX_OBJS * 4]),
        }
    }

    /// Applies `frame` with the display off (forced blank), then turns it on. For the first
    /// frame, before vblank timing matters.
    pub fn apply_blanked(&mut self, frame: &Frame) {
        debug_assert_eq!(frame.validate(), Ok(()));
        unsafe { write16(DISPCNT, DISPCNT_FORCED_BLANK) };
        self.write_cmds(frame);
        self.write_registers(frame);
    }

    /// Registers first (they must land within vblank), then the commands in order.
    pub fn apply(&mut self, frame: &Frame) {
        debug_assert_eq!(frame.validate(), Ok(()));
        self.write_registers(frame);
        self.write_cmds(frame);
        // A `Cmd::Bg` may have changed which layers are on.
        unsafe { write16(DISPCNT, self.dispcnt) };
    }

    fn write_registers(&mut self, frame: &Frame) {
        for (i, entry) in self.oam.0.chunks_exact_mut(4).enumerate() {
            match frame.objs.get(i) {
                Some(o) => entry.copy_from_slice(&oam_entry(o)),
                None => entry[0] = OAM_HIDDEN,
            }
        }
        unsafe {
            dma3(
                self.oam.0.as_ptr() as usize,
                OAM,
                self.oam.0.len() / 2,
                DMA_32BIT,
            );
            for (i, &(x, y)) in frame.scroll.iter().enumerate() {
                write16(BGOFS + 4 * i, x & 0x1FF);
                write16(BGOFS + 4 * i + 2, y & 0x1FF);
            }
            write16(BLDCNT, BLDCNT_DARKEN_ALL);
            write16(BLDY, frame.brightness.min(BRIGHTNESS_BLACK) as u16);
            write16(DISPCNT, self.dispcnt);
        }
    }

    fn write_cmds(&mut self, frame: &Frame) {
        for cmd in &frame.cmds {
            // SAFETY: addresses are built from masked fields inside palette RAM/VRAM; lengths
            // are checked by `Frame::validate` in debug builds.
            unsafe {
                match cmd {
                    Cmd::BgPalette { bank, colors } => copy_words(
                        &palette_words(colors),
                        BG_PALETTE + (bank & 15) as usize * 32,
                    ),
                    Cmd::ObjPalette { bank, colors } => copy_words(
                        &palette_words(colors),
                        OBJ_PALETTE + (bank & 15) as usize * 32,
                    ),
                    Cmd::BgTiles {
                        char_block,
                        first,
                        tiles,
                    } => copy_words(tiles.as_flattened(), bg_tile_addr(*char_block, *first)),
                    Cmd::BgTile {
                        char_block,
                        index,
                        tile,
                    } => copy_words(tile, bg_tile_addr(*char_block, *index)),
                    Cmd::ObjTiles { first, tiles } => copy_words(
                        tiles.as_flattened(),
                        OBJ_VRAM + (*first as usize & 0x3FF) * TILE_BYTES,
                    ),
                    Cmd::MapEntries {
                        screen_block: sb,
                        start,
                        entries,
                    } => dma3(
                        entries.as_ptr() as usize,
                        screen_block(*sb) + *start as usize * 2,
                        entries.len(),
                        0,
                    ),
                    Cmd::MapFill {
                        screen_block: sb,
                        entry,
                    } => {
                        let word = *entry as u32 | (*entry as u32) << 16;
                        dma3(
                            &raw const word as usize,
                            screen_block(*sb),
                            SCREEN_BLOCK_BYTES / 4,
                            DMA_32BIT | DMA_SRC_FIXED,
                        );
                    }
                    Cmd::Bg { layer, control } => {
                        let layer = (*layer as usize) & (BG_LAYERS - 1);
                        write16(BGCNT + 2 * layer, bgcnt(*control));
                        let bit = DISPCNT_BG0 << layer;
                        if control.enabled {
                            self.dispcnt |= bit;
                        } else {
                            self.dispcnt &= !bit;
                        }
                    }
                }
            }
        }
    }
}
