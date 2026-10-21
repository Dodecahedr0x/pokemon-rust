//! Applies an engine `video::Frame` to the GBA's PPU.
//!
//! [`Video::apply`] runs at vblank: it first writes the registers (BGxCNT/DISPCNT, scroll, OAM
//! from a 128-entry shadow, BLDCNT/BLDY), which are small and must land inside vblank, then the
//! frame's VRAM/palette commands in order. Bulk copies (tile runs, map rows, OAM) use DMA3,
//! which halts the CPU and moves one word (or halfword) per transfer; small writes are plain
//! word stores. Large uploads (a whole tileset) can exceed vblank: the engine does them while
//! the screen is black, so a late tail does not show.

use engine::video::{BG_LAYERS, BRIGHTNESS_BLACK, BgControl, Cmd, Frame, MAX_OBJS, Obj, ObjShape};

const DISPCNT: *mut u16 = 0x0400_0000 as *mut u16;
const BGCNT: *mut u16 = 0x0400_0008 as *mut u16;
/// BGxHOFS/BGxVOFS pairs, 4 bytes apart.
const BGOFS: *mut u16 = 0x0400_0010 as *mut u16;
const BLDCNT: *mut u16 = 0x0400_0050 as *mut u16;
const BLDY: *mut u16 = 0x0400_0054 as *mut u16;
const DMA3SAD: *mut u32 = 0x0400_00D4 as *mut u32;
const DMA3DAD: *mut u32 = 0x0400_00D8 as *mut u32;
const DMA3CNT: *mut u32 = 0x0400_00DC as *mut u32;

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
const DISPCNT_BASE: u16 = (1 << 6) | (1 << 12);
const DISPCNT_BG0: u16 = 1 << 8;
/// Brightness decrease on BG0-3, OBJ and backdrop; with BLDY = 0 it is the identity.
const BLDCNT_DARKEN_ALL: u16 = 0x3F | (3 << 6);
const OAM_HIDDEN: u16 = 1 << 9;

const DMA_ENABLE: u32 = 1 << 31;
const DMA_32BIT: u32 = 1 << 26;
const DMA_SRC_FIXED: u32 = 2 << 23;
/// DMA3's count field is 16 bits.
const DMA_MAX_UNITS: usize = 0x8000;

/// Copies `units` words (or halfwords) with DMA3; `src` and `dst` aligned to the unit.
///
/// # Safety
/// Both ranges must be valid for the transfer.
unsafe fn dma3(src: *const u8, dst: *mut u8, units: usize, flags: u32) {
    let unit = if flags & DMA_32BIT != 0 { 4 } else { 2 };
    let fixed = flags & DMA_SRC_FIXED != 0;
    let mut done = 0;
    while done < units {
        let n = (units - done).min(DMA_MAX_UNITS);
        let s = if fixed {
            src
        } else {
            unsafe { src.add(done * unit) }
        };
        unsafe {
            DMA3SAD.write_volatile(s as u32);
            DMA3DAD.write_volatile(dst.add(done * unit) as u32);
            DMA3CNT.write_volatile(DMA_ENABLE | flags | n as u32);
            // Immediate DMA halts the CPU; reading the control back covers its 2-cycle start.
            while DMA3CNT.read_volatile() & DMA_ENABLE != 0 {}
        }
        done += n;
    }
}

fn copy_words(src: &[u32], dst: usize) {
    if src.len() <= 8 {
        for (i, &w) in src.iter().enumerate() {
            unsafe { (dst as *mut u32).add(i).write_volatile(w) };
        }
    } else {
        unsafe { dma3(src.as_ptr().cast(), dst as *mut u8, src.len(), DMA_32BIT) };
    }
}

fn tile_words(tiles: &[engine::Tile]) -> &[u32] {
    tiles.as_flattened()
}

fn palette_words(colors: &[u16; 16]) -> [u32; 8] {
    core::array::from_fn(|i| colors[2 * i] as u32 | (colors[2 * i + 1] as u32) << 16)
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
        unsafe { DISPCNT.write_volatile(DISPCNT_FORCED_BLANK) };
        self.write_cmds(frame);
        self.write_registers(frame);
    }

    /// Registers first (they must land within vblank), then the commands in order.
    pub fn apply(&mut self, frame: &Frame) {
        self.write_registers(frame);
        self.write_cmds(frame);
        // A `Cmd::Bg` may have changed which layers are on.
        unsafe { DISPCNT.write_volatile(self.dispcnt) };
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
                self.oam.0.as_ptr().cast(),
                OAM as *mut u8,
                self.oam.0.len() / 2,
                DMA_32BIT,
            );
            for (i, &(x, y)) in frame.scroll.iter().enumerate() {
                BGOFS.add(2 * i).write_volatile(x & 0x1FF);
                BGOFS.add(2 * i + 1).write_volatile(y & 0x1FF);
            }
            BLDCNT.write_volatile(BLDCNT_DARKEN_ALL);
            BLDY.write_volatile(frame.brightness.min(BRIGHTNESS_BLACK) as u16);
            DISPCNT.write_volatile(self.dispcnt);
        }
    }

    fn write_cmds(&mut self, frame: &Frame) {
        for cmd in &frame.cmds {
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
                } => copy_words(
                    tile_words(tiles),
                    VRAM + *char_block as usize * CHAR_BLOCK_BYTES + *first as usize * TILE_BYTES,
                ),
                Cmd::BgTile {
                    char_block,
                    index,
                    tile,
                } => copy_words(
                    tile,
                    VRAM + *char_block as usize * CHAR_BLOCK_BYTES + *index as usize * TILE_BYTES,
                ),
                Cmd::ObjTiles { first, tiles } => {
                    copy_words(tile_words(tiles), OBJ_VRAM + *first as usize * TILE_BYTES)
                }
                Cmd::MapEntries {
                    screen_block: sb,
                    start,
                    entries,
                } => unsafe {
                    dma3(
                        entries.as_ptr().cast(),
                        (screen_block(*sb) + *start as usize * 2) as *mut u8,
                        entries.len(),
                        0,
                    );
                },
                Cmd::MapFill {
                    screen_block: sb,
                    entry,
                } => {
                    let word = *entry as u32 | (*entry as u32) << 16;
                    unsafe {
                        dma3(
                            (&raw const word).cast(),
                            screen_block(*sb) as *mut u8,
                            SCREEN_BLOCK_BYTES / 4,
                            DMA_32BIT | DMA_SRC_FIXED,
                        );
                    }
                }
                Cmd::Bg { layer, control } => {
                    let layer = (*layer as usize) & (BG_LAYERS - 1);
                    unsafe { BGCNT.add(layer).write_volatile(bgcnt(*control)) };
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
