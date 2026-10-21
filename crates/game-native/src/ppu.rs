//! Software PPU following GBA mode-0 rules, as mGBA renders them (the parity target).
//!
//! Text BGs (4bpp, 32x32 entries), regular sprites with 1D mapping, and BLDY brightness
//! decrease (or increase, `Frame::lighten`) on everything. Invalid commands (out-of-range banks or VRAM writes) panic: native
//! is where contract violations are caught.

use engine::Tile;
use engine::video::{
    BG_LAYERS, BRIGHTNESS_BLACK, BgControl, Cmd, Frame, MAP_SIZE, MAX_OBJS, Obj, SCREEN_H, SCREEN_W,
};

/// BG VRAM (char blocks 0..3, screen blocks 0..31 overlap them).
pub const BG_VRAM: usize = 0x1_0000;
/// OBJ VRAM follows BG VRAM, as in mode 0.
pub const OBJ_VRAM: usize = 0x8000;
const CHAR_BLOCK_BYTES: usize = 0x4000;
const SCREEN_BLOCK_BYTES: usize = 0x800;
const TILE_BYTES: usize = 32;
/// mGBA's OBJ render budget per scanline (cycles, H-blank interval not free).
const OBJ_LINE_CYCLES: i32 = 1210;
/// Lines per frame including vblank; used by the OAM y-wrap rule.
const TOTAL_LINES: i32 = 228;

pub struct Ppu {
    vram: Vec<u8>,
    bg_palette: [u16; 256],
    obj_palette: [u16; 256],
    bg: [BgControl; BG_LAYERS],
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    /// Power-on state: zeroed VRAM and palettes, every BG disabled.
    pub fn new() -> Self {
        Self {
            vram: vec![0; BG_VRAM + OBJ_VRAM],
            bg_palette: [0; 256],
            obj_palette: [0; 256],
            bg: [BgControl::default(); BG_LAYERS],
        }
    }

    pub fn bg_control(&self, layer: usize) -> BgControl {
        self.bg[layer]
    }

    pub fn bg_palette(&self) -> &[u16; 256] {
        &self.bg_palette
    }

    pub fn obj_palette(&self) -> &[u16; 256] {
        &self.obj_palette
    }

    /// The 96 KiB of VRAM: BG VRAM at 0, OBJ VRAM at `BG_VRAM`.
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    /// Applies the frame's commands in order. Panics on a command `Cmd::validate` rejects.
    pub fn apply(&mut self, frame: &Frame) {
        for cmd in &frame.cmds {
            if let Err(e) = cmd.validate() {
                panic!("{e}: {cmd:?}");
            }
            self.apply_cmd(cmd);
        }
    }

    fn apply_cmd(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::BgPalette { bank, colors } => set_bank(&mut self.bg_palette, *bank, colors),
            Cmd::ObjPalette { bank, colors } => set_bank(&mut self.obj_palette, *bank, colors),
            Cmd::BgTiles {
                char_block,
                first,
                tiles,
            } => self.write_tiles(bg_tile_addr(*char_block, *first), tiles),
            Cmd::BgTile {
                char_block,
                index,
                tile,
            } => self.write_tiles(
                bg_tile_addr(*char_block, *index),
                core::slice::from_ref(tile),
            ),
            Cmd::ObjTiles { first, tiles } => {
                self.write_tiles(BG_VRAM + *first as usize * TILE_BYTES, tiles)
            }
            Cmd::ObjTile { index, tile } => self.write_tiles(
                BG_VRAM + *index as usize * TILE_BYTES,
                core::slice::from_ref(tile),
            ),
            Cmd::MapEntries {
                screen_block,
                start,
                entries,
            } => {
                let at = screen_base(*screen_block) + *start as usize * 2;
                let dst = &mut self.vram[at..at + entries.len() * 2];
                for (e, out) in entries.iter().zip(dst.chunks_exact_mut(2)) {
                    out.copy_from_slice(&e.to_le_bytes());
                }
            }
            Cmd::MapFill {
                screen_block,
                entry,
            } => {
                let at = screen_base(*screen_block);
                for c in self.vram[at..at + SCREEN_BLOCK_BYTES].chunks_exact_mut(2) {
                    c.copy_from_slice(&entry.to_le_bytes());
                }
            }
            Cmd::Bg { layer, control } => self.bg[*layer as usize] = *control,
        }
    }

    fn write_tiles(&mut self, at: usize, tiles: &[Tile]) {
        let dst = &mut self.vram[at..at + tiles.len() * TILE_BYTES];
        for (row, out) in tiles.iter().flatten().zip(dst.chunks_exact_mut(4)) {
            out.copy_from_slice(&row.to_le_bytes());
        }
    }

    /// Draws one 240x160 RGB555 picture from the current VRAM/palettes and the frame's
    /// registers (scroll, objs, brightness). Panics on objs `Frame::validate` rejects.
    pub fn render(&self, frame: &Frame, fb: &mut [u16]) {
        assert_eq!(fb.len(), SCREEN_W * SCREEN_H);
        assert!(frame.objs.len() <= MAX_OBJS, "more than {MAX_OBJS} objs");
        if let Some(e) = frame.objs.iter().find_map(|o| o.validate().err()) {
            panic!("{e}");
        }
        let sprites: Vec<Sprite> = frame
            .objs
            .iter()
            .enumerate()
            .filter_map(Sprite::clean)
            .collect();
        let evy = frame.brightness.min(BRIGHTNESS_BLACK);
        let blend = if frame.lighten { brighten } else { darken };
        let backdrop = self.bg_palette[0];
        let mut obj_line = [None; SCREEN_W];
        let mut bg_lines = [[None; SCREEN_W]; BG_LAYERS];
        // Front to back: (priority, layer) order among the enabled BGs.
        let mut order: Vec<usize> = (0..BG_LAYERS).filter(|&l| self.bg[l].enabled).collect();
        order.sort_by_key(|&l| (self.bg[l].priority & 3, l));
        for y in 0..SCREEN_H {
            self.sprite_line(&sprites, y as i32, &mut obj_line);
            for &l in &order {
                self.bg_line(l, frame.scroll[l], y, &mut bg_lines[l]);
            }
            let row = &mut fb[y * SCREEN_W..(y + 1) * SCREEN_W];
            for (x, out) in row.iter_mut().enumerate() {
                let obj = obj_line[x];
                // The first opaque BG in front-to-back order, unless the sprite (which wins
                // priority ties) is in front of it.
                let bg = order
                    .iter()
                    .find_map(|&l| bg_lines[l][x].map(|c| (self.bg[l].priority & 3, c)));
                let color = match (obj, bg) {
                    (Some((op, oc)), Some((bp, _))) if op <= bp => oc,
                    (_, Some((_, bc))) => bc,
                    (Some((_, oc)), None) => oc,
                    (None, None) => backdrop,
                };
                *out = blend(color & 0x7FFF, evy);
            }
        }
    }

    /// Colors of BG `layer` on screen line `y` (`None` = transparent). The map entry is
    /// decoded once per 8-pixel tile column.
    fn bg_line(&self, layer: usize, scroll: (u16, u16), y: usize, out: &mut [Option<u16>]) {
        const WRAP: usize = MAP_SIZE * 8 - 1;
        let bg = self.bg[layer];
        let my = (y + scroll.1 as usize) & WRAP;
        let row = screen_base(bg.screen_block & 31) + (my / 8) * MAP_SIZE * 2;
        let char_base = (bg.char_block & 3) as usize * CHAR_BLOCK_BYTES;
        let mut x = 0;
        while x < SCREEN_W {
            let mx = (x + scroll.0 as usize) & WRAP;
            let ea = row + (mx / 8) * 2;
            let entry = u16::from_le_bytes([self.vram[ea], self.vram[ea + 1]]);
            let ty = if entry & (1 << 11) != 0 {
                7 - my % 8
            } else {
                my % 8
            };
            let addr = char_base + (entry & 0x3FF) as usize * TILE_BYTES + ty * 4;
            let pal = &self.bg_palette[(entry >> 12) as usize * 16..][..16];
            let hflip = entry & (1 << 10) != 0;
            // Pixels of this tile on screen: from mx % 8 to the tile's (or the screen's) end.
            let run = (8 - mx % 8).min(SCREEN_W - x);
            for i in 0..run {
                let tx = mx % 8 + i;
                let tx = if hflip { 7 - tx } else { tx };
                // Text BGs cannot reach OBJ VRAM: such tiles are transparent.
                out[x + i] = if addr >= BG_VRAM {
                    None
                } else {
                    let idx = nibble(self.vram[addr + tx / 2], tx);
                    (idx != 0).then(|| pal[idx as usize])
                };
            }
            x += run;
        }
    }

    /// Builds the OBJ layer of line `y`: per pixel, (priority, color) of the winning sprite.
    ///
    /// Mirrors mGBA: sprites are drawn in OAM order; a pixel is taken only by a sprite with a
    /// strictly lower priority number than what is there (so lower OAM index wins ties), and a
    /// transparent pixel of such a sprite lowers the stored priority without changing the color
    /// (the hardware OBJ-priority quirk). Sprites stop when the line's cycle budget runs out.
    fn sprite_line(&self, sprites: &[Sprite], y: i32, line: &mut [Option<(u8, u16)>; SCREEN_W]) {
        line.fill(None);
        let mut cycles = OBJ_LINE_CYCLES;
        let mut last_index = 0;
        for s in sprites {
            cycles -= 2 * (s.index - last_index);
            last_index = s.index;
            if cycles <= 0 {
                break;
            }
            if y < s.y || y >= s.y + s.h {
                continue;
            }
            let o = &s.obj;
            let mut in_y = y - s.y;
            if o.vflip {
                in_y = s.h - 1 - in_y;
            }
            let prio = o.priority & 3;
            let pal = (o.palette & 15) as usize * 16;
            let row_base = (o.tile & 0x3FF) as usize * TILE_BYTES
                + (in_y as usize / 8) * (s.w as usize / 8) * TILE_BYTES
                + (in_y as usize % 8) * 4;
            for out_x in s.x.max(0)..(s.x + s.w).min(SCREEN_W as i32) {
                let mut in_x = out_x - s.x;
                if o.hflip {
                    in_x = s.w - 1 - in_x;
                }
                let in_x = in_x as usize;
                let addr = (row_base + (in_x / 8) * TILE_BYTES + (in_x % 8) / 2) & (OBJ_VRAM - 1);
                let idx = nibble(self.vram[BG_VRAM + addr], in_x);
                let px = &mut line[out_x as usize];
                if px.is_none_or(|(p, _)| p > prio) {
                    if idx != 0 {
                        *px = Some((prio, self.obj_palette[pal + idx as usize]));
                    } else if let Some((p, _)) = px {
                        *p = prio;
                    }
                }
            }
            cycles -= s.cycles;
        }
    }
}

/// An OAM entry after mGBA's per-frame cleanup: decoded position/size, render cost.
struct Sprite {
    obj: Obj,
    index: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    cycles: i32,
}

impl Sprite {
    /// `None` for sprites that cannot appear on any line.
    fn clean((index, obj): (usize, &Obj)) -> Option<Self> {
        let (w, h) = Obj::dimensions(obj.shape, obj.size);
        let (w, h) = (w as i32, h as i32);
        let raw_x = (obj.x as u16 & 0x1FF) as i32;
        let raw_y = (obj.y as u16 & 0xFF) as i32;
        // 9-bit signed x.
        let x = if raw_x >= 256 { raw_x - 512 } else { raw_x };
        let mut cycles = w - 2;
        if x < 0 {
            if x + w < 0 {
                return None;
            }
            cycles += x >> 1;
        }
        if raw_y >= SCREEN_H as i32 && raw_y + h < TOTAL_LINES {
            return None;
        }
        if raw_x >= SCREEN_W as i32 && raw_x + w < 512 {
            return None;
        }
        // 8-bit y wraps: a sprite crossing line 255 continues from the top.
        let y = if raw_y + h > 256 { raw_y - 256 } else { raw_y };
        Some(Self {
            obj: *obj,
            index: index as i32,
            x,
            y,
            w,
            h,
            cycles,
        })
    }
}

fn set_bank(palette: &mut [u16; 256], bank: u8, colors: &[u16; 16]) {
    let at = bank as usize * 16;
    palette[at..at + 16].copy_from_slice(colors);
}

fn screen_base(screen_block: u8) -> usize {
    screen_block as usize * SCREEN_BLOCK_BYTES
}

fn bg_tile_addr(char_block: u8, tile: u16) -> usize {
    char_block as usize * CHAR_BLOCK_BYTES + tile as usize * TILE_BYTES
}

/// 4bpp pixel `x` (low nibble first) of a byte holding pixels `x & !1` and `x | 1`.
const fn nibble(byte: u8, x: usize) -> u8 {
    (byte >> ((x & 1) * 4)) & 0xF
}

/// Brightness decrease by `evy` (0..=16) of an RGB555 color, exactly as mGBA computes it.
///
/// Hardware (GBATEK) does `c - ((c * evy) >> 4)` on each 5-bit channel. mGBA (32-bit color
/// build, which the parity harness reads) expands each channel to 8 bits (`c << 3 | c >> 2`),
/// packs them as 0x00BBGGRR and darkens each masked channel in place,
/// `(a - a * evy / 16) & mask`; the harness keeps the top 5 bits of each byte. Red (at bit 0)
/// thus rounds up and green/blue (shifted, so the division is exact) round down, and results
/// can be one step below the hardware value. Native follows mGBA so ROM and native pixels stay
/// identical.
pub const fn darken(color: u16, evy: u8) -> u16 {
    let evy = evy as u32;
    let mut out = 0;
    let mut ch = 0;
    while ch < 3 {
        let c = ((color >> (ch * 5)) & 31) as u32;
        let a = ((c << 3) | (c >> 2)) << (ch * 8);
        let mask = 0xFF << (ch * 8);
        let d8 = ((a - (a * evy) / 16) & mask) >> (ch * 8);
        out |= ((d8 >> 3) as u16) << (ch * 5);
        ch += 1;
    }
    out
}

/// Brightness increase by `evy` (0..=16) of an RGB555 color, exactly as mGBA computes it
/// (`_brighten`, same 8-bit model as [`darken`]): `(a + (mask - a) * evy / 16) & mask` per
/// channel, keeping the top 5 bits. Hardware (GBATEK) does `c + (((31 - c) * evy) >> 4)`.
pub const fn brighten(color: u16, evy: u8) -> u16 {
    let evy = evy as u32;
    let mut out = 0;
    let mut ch = 0;
    while ch < 3 {
        let c = ((color >> (ch * 5)) & 31) as u32;
        let a = ((c << 3) | (c >> 2)) << (ch * 8);
        let mask = 0xFF << (ch * 8);
        let b8 = ((a + ((mask - a) * evy) / 16) & mask) >> (ch * 8);
        out |= ((b8 >> 3) as u16) << (ch * 5);
        ch += 1;
    }
    out
}

/// GBATEK's brightness increase of one 5-bit channel, for reference and tests.
pub const fn brighten_channel_hw(c: u16, evy: u8) -> u16 {
    c + (((31 - c) * evy as u16) >> 4)
}

/// GBATEK's brightness decrease of one 5-bit channel, for reference and tests.
pub const fn darken_channel_hw(c: u16, evy: u8) -> u16 {
    c - ((c * evy as u16) >> 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::video::ObjShape;

    const RED: u16 = 0x001F;
    const GREEN: u16 = 0x03E0;
    const BLUE: u16 = 0x7C00;
    const WHITE: u16 = 0x7FFF;

    fn leak(tiles: Vec<Tile>) -> &'static [Tile] {
        Box::leak(tiles.into_boxed_slice())
    }

    /// Tile filled with color index `c`.
    const fn solid(c: u32) -> Tile {
        [c * 0x1111_1111; 8]
    }

    fn pal(colors: &[(usize, u16)]) -> [u16; 16] {
        let mut p = [0; 16];
        for &(i, c) in colors {
            p[i] = c;
        }
        p
    }

    fn bg(layer: u8, priority: u8, screen_block: u8) -> Cmd {
        Cmd::Bg {
            layer,
            control: BgControl {
                enabled: true,
                priority,
                char_block: 0,
                screen_block,
            },
        }
    }

    fn render(ppu: &Ppu, frame: &Frame) -> Vec<u16> {
        let mut fb = vec![0; SCREEN_W * SCREEN_H];
        ppu.render(frame, &mut fb);
        fb
    }

    fn px(fb: &[u16], x: usize, y: usize) -> u16 {
        fb[y * SCREEN_W + x]
    }

    const RED_BLOCK: u16 = 64;

    /// Tiles: 0 transparent, 1 = index 1, 2 = index 2, 3 = index 3 (BG and OBJ); OBJ tiles
    /// `RED_BLOCK..+16` are index 1.
    fn base() -> Ppu {
        let mut p = Ppu::new();
        p.apply(&Frame {
            cmds: vec![
                Cmd::BgPalette {
                    bank: 0,
                    colors: pal(&[(0, WHITE), (1, RED), (2, GREEN), (3, BLUE)]),
                },
                Cmd::BgTiles {
                    char_block: 0,
                    first: 0,
                    tiles: leak(vec![solid(0), solid(1), solid(2), solid(3)]),
                },
                Cmd::ObjPalette {
                    bank: 0,
                    colors: pal(&[(1, RED), (2, GREEN), (3, BLUE)]),
                },
                Cmd::ObjTiles {
                    first: 0,
                    tiles: leak(vec![solid(0), solid(1), solid(2), solid(3)]),
                },
                // A red block for multi-tile sprites.
                Cmd::ObjTiles {
                    first: RED_BLOCK,
                    tiles: leak(vec![solid(1); 16]),
                },
            ],
            ..Frame::default()
        });
        p
    }

    fn fill(screen_block: u8, entry: u16) -> Cmd {
        Cmd::MapFill {
            screen_block,
            entry,
        }
    }

    fn obj(x: i16, y: i16, tile: u16, priority: u8) -> Obj {
        Obj {
            x,
            y,
            tile,
            priority,
            ..Obj::default()
        }
    }

    fn with(cmds: Vec<Cmd>) -> Frame {
        Frame {
            cmds,
            ..Frame::default()
        }
    }

    #[test]
    fn backdrop_is_bg_color_zero_when_nothing_is_enabled() {
        let p = base();
        assert!(render(&p, &Frame::default()).iter().all(|&c| c == WHITE));
    }

    #[test]
    fn color_index_zero_is_transparent() {
        let mut p = base();
        p.apply(&with(vec![fill(31, 0), bg(0, 0, 31)]));
        assert_eq!(px(&render(&p, &Frame::default()), 5, 5), WHITE);
        p.apply(&with(vec![fill(31, 1)]));
        assert_eq!(px(&render(&p, &Frame::default()), 5, 5), RED);
    }

    #[test]
    fn map_entry_tile_hflip_vflip_and_palette_bank() {
        let mut p = base();
        // Tile 4: index 1 only at pixel (0, 0).
        let mut t = [0u32; 8];
        t[0] = 1;
        p.apply(&with(vec![
            Cmd::BgTile {
                char_block: 0,
                index: 4,
                tile: t,
            },
            Cmd::BgPalette {
                bank: 2,
                colors: pal(&[(1, BLUE)]),
            },
            Cmd::MapEntries {
                screen_block: 31,
                start: 0,
                // plain, hflip, vflip, both, bank 2
                entries: vec![4, 4 | 1 << 10, 4 | 1 << 11, 4 | 3 << 10, 4 | 2 << 12],
            },
            bg(0, 0, 31),
        ]));
        let fb = render(&p, &Frame::default());
        assert_eq!(px(&fb, 0, 0), RED);
        assert_eq!(px(&fb, 1, 0), WHITE);
        assert_eq!(px(&fb, 8 + 7, 0), RED);
        assert_eq!(px(&fb, 8, 0), WHITE);
        assert_eq!(px(&fb, 16, 7), RED);
        assert_eq!(px(&fb, 16, 0), WHITE);
        assert_eq!(px(&fb, 24 + 7, 7), RED);
        assert_eq!(px(&fb, 32, 0), BLUE);
    }

    #[test]
    fn scroll_wraps_at_256_both_ways() {
        let mut p = base();
        p.apply(&with(vec![
            fill(31, 0),
            Cmd::MapEntries {
                screen_block: 31,
                start: 31 * 32 + 31, // bottom-right entry
                entries: vec![1],
            },
            bg(0, 0, 31),
        ]));
        let mut f = Frame::default();
        f.scroll[0] = (248, 248);
        let fb = render(&p, &f);
        assert_eq!(px(&fb, 0, 0), RED);
        assert_eq!(px(&fb, 7, 7), RED);
        assert_eq!(px(&fb, 8, 0), WHITE);
        // Scroll values beyond 255 wrap too.
        f.scroll[0] = (248 + 512, 248 + 256);
        assert_eq!(render(&p, &f), fb);
    }

    #[test]
    fn char_and_screen_block_select_vram() {
        let mut p = base();
        p.apply(&with(vec![
            Cmd::BgTiles {
                char_block: 2,
                first: 7,
                tiles: leak(vec![solid(3)]),
            },
            fill(20, 7),
            Cmd::Bg {
                layer: 1,
                control: BgControl {
                    enabled: true,
                    priority: 0,
                    char_block: 2,
                    screen_block: 20,
                },
            },
        ]));
        assert_eq!(px(&render(&p, &Frame::default()), 100, 100), BLUE);
    }

    #[test]
    fn bg_tiles_run_on_into_next_char_block() {
        let mut p = base();
        p.apply(&with(vec![Cmd::BgTiles {
            char_block: 0,
            first: 511,
            tiles: leak(vec![solid(1), solid(2)]),
        }]));
        assert_eq!(p.vram()[CHAR_BLOCK_BYTES], 0x22);
    }

    #[test]
    fn bg_tiles_beyond_bg_vram_are_transparent() {
        let mut p = base();
        // Char block 3, tile 512 is at 0x10000 = OBJ VRAM (tile 0 is transparent there, so
        // write a solid OBJ tile 0 to show it is not read).
        p.apply(&with(vec![
            Cmd::ObjTiles {
                first: 0,
                tiles: leak(vec![solid(1)]),
            },
            fill(31, 512),
            Cmd::Bg {
                layer: 0,
                control: BgControl {
                    enabled: true,
                    priority: 0,
                    char_block: 3,
                    screen_block: 31,
                },
            },
        ]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), WHITE);
    }

    #[test]
    fn lower_priority_number_is_in_front() {
        let mut p = base();
        p.apply(&with(vec![
            fill(30, 1),
            fill(31, 2),
            bg(0, 3, 31),
            bg(1, 1, 30),
        ]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), RED);
    }

    #[test]
    fn equal_priority_bgs_lower_layer_in_front() {
        let mut p = base();
        p.apply(&with(vec![
            fill(30, 1),
            fill(31, 2),
            bg(2, 1, 31),
            bg(1, 1, 30),
        ]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), RED);
        p.apply(&with(vec![bg(2, 1, 30), bg(1, 1, 31)]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), GREEN);
    }

    #[test]
    fn transparent_bg_reveals_the_one_behind() {
        let mut p = base();
        p.apply(&with(vec![
            fill(30, 0),
            fill(31, 2),
            bg(0, 0, 30),
            bg(1, 3, 31),
        ]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), GREEN);
    }

    #[test]
    fn disabled_bg_is_not_drawn() {
        let mut p = base();
        p.apply(&with(vec![fill(31, 1), bg(0, 0, 31)]));
        p.apply(&with(vec![Cmd::Bg {
            layer: 0,
            control: BgControl::default(),
        }]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), WHITE);
    }

    #[test]
    fn sprite_in_front_of_bg_on_equal_priority_behind_on_lower() {
        let mut p = base();
        p.apply(&with(vec![fill(31, 2), bg(0, 1, 31)]));
        let mut f = Frame {
            objs: vec![obj(0, 0, 1, 1)],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &f), 0, 0), RED);
        f.objs[0].priority = 2;
        assert_eq!(px(&render(&p, &f), 0, 0), GREEN);
        f.objs[0].priority = 0;
        assert_eq!(px(&render(&p, &f), 0, 0), RED);
    }

    #[test]
    fn sprite_size_and_position() {
        let p = base();
        let f = Frame {
            objs: vec![Obj {
                shape: ObjShape::Tall,
                size: 2, // 16x32
                ..obj(10, 20, RED_BLOCK, 0)
            }],
            ..Frame::default()
        };
        let fb = render(&p, &f);
        assert_eq!(px(&fb, 10, 20), RED);
        assert_eq!(px(&fb, 25, 51), RED);
        assert_eq!(px(&fb, 9, 20), WHITE);
        assert_eq!(px(&fb, 26, 20), WHITE);
        assert_eq!(px(&fb, 10, 52), WHITE);
        assert_eq!(px(&fb, 10, 19), WHITE);
    }

    #[test]
    fn sprite_uses_obj_palette_bank() {
        let mut p = base();
        p.apply(&with(vec![Cmd::ObjPalette {
            bank: 3,
            colors: pal(&[(1, GREEN)]),
        }]));
        let f = Frame {
            objs: vec![Obj {
                palette: 3,
                ..obj(0, 0, 1, 0)
            }],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &f), 0, 0), GREEN);
    }

    #[test]
    fn sprite_1d_mapping_and_flips() {
        let mut p = base();
        // 16x16 sprite at tile 8: tiles 8, 9 (top row), 10, 11 (bottom row), consecutive in 1D.
        p.apply(&with(vec![Cmd::ObjTiles {
            first: 8,
            tiles: leak(vec![solid(1), solid(2), solid(3), solid(0)]),
        }]));
        let o = Obj {
            size: 1,
            ..obj(0, 0, 8, 0)
        };
        let check = |o: Obj, quads: [u16; 4]| {
            let fb = render(
                &p,
                &Frame {
                    objs: vec![o],
                    ..Frame::default()
                },
            );
            assert_eq!(
                [px(&fb, 0, 0), px(&fb, 8, 0), px(&fb, 0, 8), px(&fb, 8, 8)],
                quads,
                "{o:?}"
            );
        };
        check(o, [RED, GREEN, BLUE, WHITE]);
        check(Obj { hflip: true, ..o }, [GREEN, RED, WHITE, BLUE]);
        check(Obj { vflip: true, ..o }, [BLUE, WHITE, RED, GREEN]);
        check(
            Obj {
                hflip: true,
                vflip: true,
                ..o
            },
            [WHITE, BLUE, GREEN, RED],
        );
    }

    #[test]
    fn sprite_flip_mirrors_pixels_within_a_tile() {
        let mut p = base();
        let mut t = [0u32; 8];
        t[0] = 1; // pixel (0, 0)
        p.apply(&with(vec![Cmd::ObjTiles {
            first: 5,
            tiles: leak(vec![t]),
        }]));
        let o = obj(0, 0, 5, 0);
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj {
                    hflip: true,
                    vflip: true,
                    ..o
                }],
                ..Frame::default()
            },
        );
        assert_eq!(px(&fb, 7, 7), RED);
        assert_eq!(px(&fb, 0, 0), WHITE);
    }

    #[test]
    fn sprite_x_wraps_at_9_bits_and_y_at_8_bits() {
        let p = base();
        let o = Obj {
            size: 1, // 16x16
            ..obj(0, 0, RED_BLOCK, 0)
        };
        // x = 504 is -8: the right half shows at x 0..8.
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj { x: 504, ..o }],
                ..Frame::default()
            },
        );
        assert_eq!(px(&fb, 7, 0), RED);
        assert_eq!(px(&fb, 8, 0), WHITE);
        // Negative x is the same OAM value.
        let fb2 = render(
            &p,
            &Frame {
                objs: vec![Obj { x: -8, ..o }],
                ..Frame::default()
            },
        );
        assert_eq!(fb, fb2);
        // y = 248 shows its bottom half at the top of the screen.
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj { y: 248, ..o }],
                ..Frame::default()
            },
        );
        assert_eq!(px(&fb, 0, 7), RED);
        assert_eq!(px(&fb, 0, 8), WHITE);
        assert_eq!(
            fb,
            render(
                &p,
                &Frame {
                    objs: vec![Obj { y: -8, ..o }],
                    ..Frame::default()
                }
            )
        );
        // x = 250 is past the right edge and does not wrap to the left.
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj { x: 250, ..o }],
                ..Frame::default()
            },
        );
        assert!(fb.iter().all(|&c| c == WHITE));
        // A sprite at y 160..=255 that does not cross 256 is hidden.
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj { y: 200, ..o }],
                ..Frame::default()
            },
        );
        assert!(fb.iter().all(|&c| c == WHITE));
    }

    #[test]
    fn obj_tile_addressing_wraps_in_obj_vram() {
        let mut p = base();
        p.apply(&with(vec![Cmd::ObjTiles {
            first: 1023,
            tiles: leak(vec![solid(3)]),
        }]));
        // 16x8 sprite at tile 1023: second tile wraps to tile 0 (transparent here).
        let fb = render(
            &p,
            &Frame {
                objs: vec![Obj {
                    shape: ObjShape::Wide,
                    size: 0,
                    ..obj(0, 0, 1023, 0)
                }],
                ..Frame::default()
            },
        );
        assert_eq!(px(&fb, 0, 0), BLUE);
        assert_eq!(px(&fb, 8, 0), WHITE);
    }

    #[test]
    fn lower_oam_index_in_front_among_equal_priority() {
        let p = base();
        let f = Frame {
            objs: vec![obj(0, 0, 1, 2), obj(4, 0, 2, 2)],
            ..Frame::default()
        };
        let fb = render(&p, &f);
        assert_eq!(px(&fb, 5, 0), RED);
        assert_eq!(px(&fb, 9, 0), GREEN);
    }

    #[test]
    fn transparent_sprite_pixel_reveals_the_next_sprite() {
        let p = base();
        let f = Frame {
            objs: vec![obj(0, 0, 0, 2), obj(0, 0, 2, 2)],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &f), 0, 0), GREEN);
    }

    #[test]
    fn higher_oam_index_with_lower_priority_number_wins() {
        let p = base();
        let f = Frame {
            objs: vec![obj(0, 0, 1, 2), obj(0, 0, 2, 1)],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &f), 0, 0), GREEN);
    }

    /// Hardware/mGBA quirk: a later sprite with a lower priority number but a transparent pixel
    /// lifts the earlier sprite's pixel to its priority, here in front of a priority-1 BG.
    #[test]
    fn transparent_pixel_of_higher_priority_sprite_lifts_the_pixel_below() {
        let mut p = base();
        p.apply(&with(vec![fill(31, 3), bg(0, 1, 31)]));
        let f = Frame {
            objs: vec![obj(0, 0, 1, 2), obj(0, 0, 0, 0)],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &f), 0, 0), RED);
        let alone = Frame {
            objs: vec![obj(0, 0, 1, 2)],
            ..Frame::default()
        };
        assert_eq!(px(&render(&p, &alone), 0, 0), BLUE);
    }

    #[test]
    fn sprites_past_the_line_cycle_budget_are_dropped() {
        let p = base();
        // 64x64 sprites cost 62 cycles each: the 20th (index 19) exhausts 1210 cycles.
        let big = Obj {
            size: 3,
            ..obj(0, 0, 0, 0)
        };
        let mut objs = vec![big; 30];
        objs[25] = Obj {
            size: 0,
            ..obj(100, 0, 1, 0)
        };
        objs[1] = Obj {
            size: 0,
            ..obj(50, 0, 1, 0)
        };
        let fb = render(
            &p,
            &Frame {
                objs,
                ..Frame::default()
            },
        );
        assert_eq!(px(&fb, 50, 0), RED);
        assert_eq!(px(&fb, 100, 0), WHITE);
    }

    #[test]
    fn brightness_darkens_bg_sprites_and_backdrop() {
        let mut p = base();
        p.apply(&with(vec![
            Cmd::MapEntries {
                screen_block: 31,
                start: 0,
                entries: vec![1],
            },
            bg(0, 0, 31),
        ]));
        let mut f = Frame {
            objs: vec![obj(8, 0, 3, 0)],
            brightness: 8,
            ..Frame::default()
        };
        let fb = render(&p, &f);
        assert_eq!(px(&fb, 0, 0), darken(RED, 8));
        assert_eq!(px(&fb, 8, 0), darken(BLUE, 8));
        assert_eq!(px(&fb, 16, 0), darken(WHITE, 8));
        assert_eq!(darken(WHITE, 8), 16 | 15 << 5 | 15 << 10);
        f.brightness = BRIGHTNESS_BLACK;
        assert!(render(&p, &f).iter().all(|&c| c == 0));
        // BLDY saturates at 16.
        f.brightness = 31;
        assert!(render(&p, &f).iter().all(|&c| c == 0));
        f.brightness = 0;
        assert_eq!(px(&render(&p, &f), 16, 0), WHITE);
    }

    #[test]
    fn lighten_brightens_bg_sprites_and_backdrop() {
        let mut p = base();
        p.apply(&with(vec![
            Cmd::MapEntries {
                screen_block: 31,
                start: 0,
                entries: vec![1],
            },
            bg(0, 0, 31),
        ]));
        let mut f = Frame {
            objs: vec![obj(8, 0, 3, 0)],
            brightness: 8,
            lighten: true,
            ..Frame::default()
        };
        let fb = render(&p, &f);
        assert_eq!(px(&fb, 0, 0), brighten(RED, 8));
        assert_eq!(px(&fb, 8, 0), brighten(BLUE, 8));
        assert_eq!(px(&fb, 16, 0), WHITE);
        // (31, 0, 0) at evy 8: red stays, green and blue reach (255 * 8 / 16) >> 3 = 15.
        assert_eq!(brighten(RED, 8), 31 | 15 << 5 | 15 << 10);
        f.brightness = BRIGHTNESS_BLACK;
        assert!(render(&p, &f).iter().all(|&c| c == WHITE));
        f.brightness = 31;
        assert!(render(&p, &f).iter().all(|&c| c == WHITE));
        f.brightness = 0;
        assert_eq!(px(&render(&p, &f), 0, 0), RED);
    }

    #[test]
    fn brighten_matches_hardware_formula_up_to_mgba_rounding() {
        for c in 0..32u16 {
            for evy in 0..=16u8 {
                let hw = brighten_channel_hw(c, evy);
                for shift in [0, 5, 10] {
                    let ours = (brighten(c << shift, evy) >> shift) & 31;
                    assert!(ours.abs_diff(hw) <= 1, "c {c} evy {evy}: {ours} vs {hw}");
                }
            }
        }
        assert_eq!(brighten(0, 16), 0x7FFF);
        assert_eq!(brighten(0x1234, 0), 0x1234);
    }

    #[test]
    fn obj_tile_writes_one_obj_tile() {
        let mut p = Ppu::new();
        p.apply(&with(vec![Cmd::ObjTile {
            index: 1023,
            tile: [0x1111_1111, 2, 3, 4, 5, 6, 7, 0x8765_4321],
        }]));
        let at = BG_VRAM + 1023 * TILE_BYTES;
        assert_eq!(&p.vram()[at..at + 4], &[0x11; 4]);
        assert_eq!(&p.vram()[at + 28..at + 32], &[0x21, 0x43, 0x65, 0x87]);
        assert!(p.vram()[BG_VRAM..at].iter().all(|&b| b == 0));
    }

    #[test]
    fn darken_matches_hardware_formula_up_to_mgba_rounding() {
        for c in 0..32u16 {
            for evy in 0..=16u8 {
                let hw = darken_channel_hw(c, evy);
                for shift in [0, 5, 10] {
                    let ours = darken(c << shift, evy) >> shift;
                    assert!(
                        ours == hw || ours + 1 == hw,
                        "c {c} evy {evy}: {ours} vs {hw}"
                    );
                }
            }
        }
        // Observed in mGBA: 0x0421 (1, 1, 1) at evy 1 shows as (1, 0, 0).
        assert_eq!(darken(0x0421, 1), 0x0001);
        assert_eq!(darken(0x7FFF, 16), 0);
        assert_eq!(darken(0x7FFF, 0), 0x7FFF);
    }

    #[test]
    fn apply_runs_cmds_in_order() {
        let mut p = base();
        p.apply(&with(vec![fill(31, 1), fill(31, 2), bg(0, 0, 31)]));
        assert_eq!(px(&render(&p, &Frame::default()), 0, 0), GREEN);
        assert_eq!(p.bg_control(0).screen_block, 31);
        assert_eq!(p.bg_palette()[1], RED);
        assert_eq!(p.obj_palette()[3], BLUE);
    }

    /// Per-pixel BG lookup, written independently of the row renderer.
    fn naive_bg(p: &Ppu, layer: usize, scroll: (u16, u16), x: usize, y: usize) -> Option<u16> {
        let bg = p.bg_control(layer);
        let mx = (x + scroll.0 as usize) % 256;
        let my = (y + scroll.1 as usize) % 256;
        let ea = bg.screen_block as usize * 0x800 + ((my / 8) * 32 + mx / 8) * 2;
        let entry = u16::from_le_bytes([p.vram()[ea], p.vram()[ea + 1]]);
        let tx = if entry & 0x400 != 0 {
            7 - mx % 8
        } else {
            mx % 8
        };
        let ty = if entry & 0x800 != 0 {
            7 - my % 8
        } else {
            my % 8
        };
        let addr = bg.char_block as usize * 0x4000 + (entry & 0x3FF) as usize * 32 + ty * 4;
        if addr >= BG_VRAM {
            return None;
        }
        let idx = (p.vram()[addr + tx / 2] >> (4 * (tx % 2))) & 15;
        (idx != 0).then(|| p.bg_palette()[(entry >> 12) as usize * 16 + idx as usize])
    }

    #[test]
    fn row_renderer_matches_per_pixel_lookup_at_any_scroll() {
        let mut p = Ppu::new();
        // Deterministic pseudo-random tiles, map entries (all flips, banks) and palettes.
        let mut seed = 0x1234_5678u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let tiles: Vec<Tile> = (0..1024)
            .map(|_| core::array::from_fn(|_| next()))
            .collect();
        let entries: Vec<u16> = (0..1024).map(|_| next() as u16).collect();
        let mut cmds = vec![
            Cmd::BgTiles {
                char_block: 2,
                first: 0,
                tiles: leak(tiles),
            },
            Cmd::MapEntries {
                screen_block: 4,
                start: 0,
                entries,
            },
            Cmd::Bg {
                layer: 2,
                control: BgControl {
                    enabled: true,
                    priority: 0,
                    char_block: 2,
                    screen_block: 4,
                },
            },
        ];
        for bank in 0..16 {
            cmds.push(Cmd::BgPalette {
                bank,
                colors: core::array::from_fn(|_| next() as u16 & 0x7FFF),
            });
        }
        p.apply(&with(cmds));
        for scroll in [(0, 0), (3, 5), (7, 1), (250, 254), (129, 77), (511, 300)] {
            let mut f = Frame::default();
            f.scroll[2] = scroll;
            let fb = render(&p, &f);
            for y in 0..SCREEN_H {
                for x in 0..SCREEN_W {
                    let want = naive_bg(&p, 2, scroll, x, y).unwrap_or(p.bg_palette()[0]);
                    assert_eq!(px(&fb, x, y), want, "scroll {scroll:?} at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn invalid_obj_panics() {
        let f = Frame {
            objs: vec![Obj {
                palette: 16,
                ..Obj::default()
            }],
            ..Frame::default()
        };
        render(&Ppu::new(), &f);
    }

    #[test]
    #[should_panic(expected = "past")]
    fn obj_tiles_past_obj_vram_panic() {
        let mut p = Ppu::new();
        p.apply(&with(vec![Cmd::ObjTiles {
            first: 1023,
            tiles: leak(vec![solid(1), solid(1)]),
        }]));
    }
}
