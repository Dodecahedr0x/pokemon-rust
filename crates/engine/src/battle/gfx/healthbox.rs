//! Singles healthboxes (`battle_interface.c`): `CreateBattlerHealthboxSprites`, the element
//! updates of `UpdateHealthboxAttribute` (nickname + gender, level, the player's HP numbers,
//! the status icon) and the HP bar sprite placement (`SpriteCB_HealthBar`).
//!
//! Text is printed with `FONT_SMALL` into an 8x2-tile window (`sHealthboxWindowTemplate`,
//! filled with color 2, text colors 1 / shadow 3) and copied into the box tiles as Gen3's
//! `TextIntoHealthboxObject` (bottom row whole, top row only its last 3 pixel rows) and
//! `HpTextIntoHealthboxObject` (bottom row tiles). The box keeps a RAM copy of its tiles and
//! uploads only the tiles that changed, at most [`HEALTHBOX_TILES_PER_FRAME`] per frame and
//! never past the frame's shared `ObjTile` budget ([`super::OBJ_TILES_PER_FRAME`], counting
//! what other components already sent): update the healthboxes after sprites and bars.
//!
//! Omitted: the opponent HP bar's third subsprite (an 8x8 at -32, which only shows the
//! caught-ball icon, not implemented).

use super::super::{BattleGfx, BattleMon, Side};
use super::{hash_i16, obj, side_tag};
use crate::Tile;
use crate::data::Font;
use crate::hash::Fnv64;
use crate::mon::{Gender, NAME_LEN, Status};
use crate::text::{Colors, Pen, px_mask};
use crate::video::{Cmd, Frame, Obj, ObjShape, layout::battle::OBJ_PRIORITY_BOX};
use alloc::vec;
use alloc::vec::Vec;

/// `Cmd::ObjTile`s a visible healthbox may send per frame (an HP drain changes 3).
pub const HEALTHBOX_TILES_PER_FRAME: usize = 10;

const EOS: u8 = 0xFF;
const CHAR_EXTRA_SYMBOL: u8 = 0xF9;
const CHAR_LV_2: u8 = 0x05;
const CHAR_0: u8 = 0xA1;
const CHAR_SPACER: u8 = 0x77;
const CHAR_SLASH: u8 = 0xBA;
const CHAR_MALE: u8 = 0xB5;
const CHAR_FEMALE: u8 = 0xB6;
/// `AddTextPrinterAndCreateWindowOnHealthbox`: bg 2, fg 1, shadow 3.
const COLOR_BG: u8 = 2;
const COLOR_FG: u8 = 1;
const COLOR_SHADOW: u8 = 3;
/// `gText_HealthboxGender_Male` / `_Female`: `{COLOR DYNAMIC_COLOR2}` / `DYNAMIC_COLOR1`.
const COLOR_MALE: u8 = 0x0B;
const COLOR_FEMALE: u8 = 0x0A;
/// `gCurGlyph.height` of `FONT_SMALL`.
const GLYPH_HEIGHT: usize = 13;
/// `StartHealthboxSlideIn`: x2 = 0x73, 5 px per frame.
const SLIDE_START: i16 = 0x73;
const SLIDE_SPEED: i16 = 5;
/// `SpriteCB_HitAnimHealthoxEffect` runs 21 frames.
const SHAKE_FRAMES: u8 = 21;
/// `sStatusIconColors`, by status icon (PSN, PRZ, SLP, FRZ, BRN).
const STATUS_COLORS: [u16; 5] = [
    rgb(24, 12, 24),
    rgb(23, 23, 3),
    rgb(20, 20, 17),
    rgb(17, 22, 28),
    rgb(28, 14, 10),
];
/// `HEALTHBOX_GFX_39`: the blank status tile (`misc` tile 3).
const MISC_BLANK_STATUS: usize = 3;
/// `HEALTHBOX_GFX_65` (`misc_frameend`, after the 11 `misc` tiles).
const MISC_FRAME_END: usize = 11;

const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | g << 5 | b << 10
}

/// Status icon index (`HEALTHBOX_GFX_STATUS_*` order: PSN, PRZ, SLP, FRZ, BRN), Gen3 priority.
const fn status_icon(s: Status) -> Option<u8> {
    match s {
        Status::None => None,
        Status::Sleep(_) => Some(2),
        Status::Poison | Status::Toxic(_) => Some(0),
        Status::Burn => Some(4),
        Status::Freeze => Some(3),
        Status::Paralysis => Some(1),
    }
}

/// Per-side layout, in box tiles (Gen3 byte offsets / 32).
#[derive(Clone, Copy)]
struct Layout {
    tiles: usize,
    vram: u16,
    bar: u16,
    palette: u8,
    /// Gen3 sprite centre minus the 64x32 template's `centerToCornerVec`.
    pos: (i16, i16),
    /// Healthbar sprite centre (`SpriteCB_HealthBar`) relative to `pos`.
    bar_pos: (i16, i16),
    shape: (ObjShape, u8),
    /// (dest tile, window tile, width) runs of `TextIntoHealthboxObject` for the nickname.
    nick: &'static [(usize, usize, usize)],
    level: usize,
    status: usize,
    status_color: usize,
}

const PLAYER: Layout = Layout {
    tiles: 128,
    vram: obj::PLAYER_BOX,
    bar: obj::PLAYER_BAR,
    palette: obj::PAL_PLAYER_BOX,
    pos: (158 - 32, 88 - 16),
    bar_pos: (32, 16),
    shape: (ObjShape::Square, 3),
    nick: &[(2, 0, 6), (64, 6, 1)],
    level: 0x820 / 32,
    status: 0x1A,
    status_color: 12,
};

const OPPONENT: Layout = Layout {
    tiles: 64,
    vram: obj::ENEMY_BOX,
    bar: obj::ENEMY_BAR,
    palette: obj::PAL_ENEMY_BOX,
    pos: (44 - 32, 30 - 16),
    bar_pos: (24, 16),
    shape: (ObjShape::Wide, 3),
    nick: &[(1, 0, 7)],
    level: 0x400 / 32,
    status: 0x11,
    status_color: 13,
};

/// `InitBattlerHealthboxCoords` in doubles (sprite centres), by battler position.
const DOUBLE_XY: [(i16, i16); 4] = [(159, 76), (44, 19), (171, 101), (32, 44)];

/// The layout of `side`'s box. Doubles (degraded): the singles graphics at the doubles
/// positions, the player's boxes cut to their top 64x32 (no exp bar, as Gen3's doubles box);
/// the right battlers use their own VRAM and palette banks.
fn layout(side: Side, double: bool) -> Layout {
    let mut l = if side.is_player() { PLAYER } else { OPPONENT };
    if side.is_right() {
        (l.vram, l.bar, l.palette) = if side.is_player() {
            (obj::PLAYER2_BOX, obj::PLAYER2_BAR, obj::PAL_PLAYER2_BOX)
        } else {
            (obj::ENEMY2_BOX, obj::ENEMY2_BAR, obj::PAL_ENEMY2_BOX)
        };
    }
    if double {
        let (x, y) = DOUBLE_XY[side.i()];
        l.pos = (x - 32, y - 16);
        if side.is_player() {
            l.shape = (ObjShape::Wide, 3);
        }
    }
    l
}

/// HP numbers of the player's box: `HpTextIntoHealthboxObject` destinations.
const HP_CUR_TILES: [(usize, usize); 3] = [(0x3E0 / 32, 8), (0xB00 / 32, 9), (0xB00 / 32 + 1, 10)];
const HP_MAX_TILES: [(usize, usize); 2] = [(0xB40 / 32, 8), (0xB40 / 32 + 1, 9)];

/// `sHealthboxWindowTemplate`: 8x2 tiles, row-major.
struct Window([Tile; 16]);

impl Window {
    fn new() -> Self {
        Window([[(COLOR_BG as u32) * 0x1111_1111; 8]; 16])
    }

    /// Gen3 `CopyGlyphToWindow` for `FONT_SMALL`: `width` x 13 pixels, 2bpp 0/3 -> bg,
    /// 1 -> fg, 2 -> shadow. Returns the advance. Whole tile rows at a time (as
    /// `text::Canvas::draw_glyph`): text is slow on the GBA.
    fn glyph(&mut self, font: &Font, id: u16, x: usize, y: usize, fg: u8) -> usize {
        let (Some(g), Some(&w)) = (font.glyphs.get(id as usize), font.widths.get(id as usize))
        else {
            return 0;
        };
        let pen = Pen::new(Colors::new(fg, COLOR_SHADOW, COLOR_BG));
        let width = (w as usize).min(16);
        let shift = (x % 8) * 4;
        for gy in 0..GLYPH_HEIGHT {
            let cy = y + gy;
            if cy >= 16 {
                break;
            }
            for half in 0..width.div_ceil(8) {
                let off = ((gy / 8) * 2 + half) * 16 + (gy % 8) * 2;
                let word = pen.row(g[off], g[off + 1]);
                let mask = px_mask(width - half * 8);
                let tx = x / 8 + half;
                self.put_row(tx, cy, word << shift, mask << shift);
                if shift != 0 {
                    self.put_row(tx + 1, cy, word >> (32 - shift), mask >> (32 - shift));
                }
            }
        }
        w as usize
    }

    /// Writes `word` under `mask` into pixel row `y` of tile column `tx` (clipped to 8x2).
    #[inline(always)]
    fn put_row(&mut self, tx: usize, y: usize, word: u32, mask: u32) {
        if tx < 8 && mask != 0 {
            let r = &mut self.0[(y / 8) * 8 + tx][y % 8];
            *r = (*r & !mask) | (word & mask);
        }
    }

    /// Prints `text` (glyph bytes, `{EXTRA_SYMBOL n}` pairs) at (x, y) in the normal color.
    fn print(&mut self, font: &Font, text: &[u8], mut x: usize, y: usize) -> usize {
        let mut i = 0;
        while let Some(&b) = text.get(i) {
            match b {
                EOS => break,
                CHAR_EXTRA_SYMBOL => {
                    if let Some(&a) = text.get(i + 1) {
                        x += self.glyph(font, 0x100 | a as u16, x, y, COLOR_FG);
                    }
                    i += 1;
                }
                c => x += self.glyph(font, c as u16, x, y, COLOR_FG),
            }
            i += 1;
        }
        x
    }
}

/// A short glyph string on the stack (no per-frame allocation).
struct Text {
    buf: [u8; 8],
    len: usize,
}

impl Text {
    const fn new() -> Self {
        Text {
            buf: [EOS; 8],
            len: 0,
        }
    }

    fn push(&mut self, b: u8) {
        if self.len < self.buf.len() {
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// `ConvertIntToDecimalStringN` for up to 3 digits: right-aligned pads with `CHAR_SPACER`.
fn decimal(v: u16, right_align: bool, out: &mut Text) {
    let v = v.min(999);
    let mut started = false;
    for p in [100, 10, 1] {
        let d = (v / p % 10) as u8;
        if started || d != 0 || p == 1 {
            started = true;
            out.push(CHAR_0 + d);
        } else if right_align {
            out.push(CHAR_SPACER);
        }
    }
}

/// One side's healthbox: two OBJ halves plus the HP bar sprite.
#[derive(Clone)]
pub struct Healthbox {
    side: Side,
    double: bool,
    gfx: &'static BattleGfx,
    /// RAM copy of the box tiles (what VRAM holds once `dirty` is flushed).
    tiles: Vec<Tile>,
    dirty: u128,
    /// HP bar sprite tiles 0..2: "HP", or black + frame end for a statused opponent.
    label: [Tile; 2],
    label_dirty: bool,
    palette: [u16; 16],
    palette_dirty: bool,
    nick: [u8; NAME_LEN],
    gender: Gender,
    level: u8,
    hp: u16,
    max_hp: u16,
    status: Option<u8>,
    visible: bool,
    /// Gen3 x2 / y2.
    off: (i16, i16),
    slide: i16,
    shake: u8,
}

impl Healthbox {
    pub fn new(side: Side, gfx: &'static BattleGfx) -> Self {
        Self::with(side, false, gfx)
    }

    /// [`Healthbox::new`] at the singles (`double` false) or doubles position.
    pub fn with(side: Side, double: bool, gfx: &'static BattleGfx) -> Self {
        let n = layout(side, double).tiles;
        Healthbox {
            side,
            double,
            gfx,
            tiles: vec![[0; 8]; n],
            dirty: 0,
            label: [[0; 8]; 2],
            label_dirty: false,
            palette: [0; 16],
            palette_dirty: false,
            nick: [EOS; NAME_LEN],
            gender: Gender::Genderless,
            level: 0,
            hp: 0,
            max_hp: 0,
            status: None,
            visible: false,
            off: (0, 0),
            slide: 0,
            shake: 0,
        }
    }

    /// Uploads the box (`ObjTiles` of the base graphic, palettes) and draws every element for
    /// `mon`, all in `f`: a bulk upload, for black frames only. Hidden afterwards. Mid-battle
    /// (a new mon on a loaded box) use [`Healthbox::set_mon`], which uploads within budget.
    /// The exp bar tiles are reset to the base graphic: redraw them (`ExpBar::invalidate`).
    pub fn load(&mut self, mon: &BattleMon, f: &mut Frame) {
        let l = layout(self.side, self.double);
        let base = if self.side.is_player() {
            &self.gfx.healthbox_player
        } else {
            &self.gfx.healthbox_opponent
        };
        let n = base.tiles.len().min(l.tiles);
        if n > 0 {
            f.cmds.push(Cmd::ObjTiles {
                first: l.vram,
                tiles: &base.tiles[..n],
            });
        }
        self.tiles.fill([0; 8]);
        self.tiles[..n].copy_from_slice(&base.tiles[..n]);
        // Tiles the graphic does not cover are stale in VRAM: write them too.
        self.dirty = mask(n, l.tiles);
        self.palette = base.palettes.first().copied().unwrap_or([0; 16]);
        if let Some(p) = self.gfx.hpbar.palettes.first() {
            f.cmds.push(Cmd::ObjPalette {
                bank: obj::PAL_HP_BAR,
                colors: *p,
            });
        }
        self.label = [[0; 8]; 2];
        self.visible = false;
        self.off = (0, 0);
        self.slide = 0;
        self.shake = 0;
        self.draw_all(mon);
        self.label_dirty = true;
        self.palette_dirty = true;
        self.flush(f, usize::MAX);
    }

    /// Redraws every element for `mon` (`UpdateHealthboxAttribute(HEALTHBOX_ALL)`, bars
    /// aside); uploads follow over the next frames.
    pub fn set_mon(&mut self, mon: &BattleMon) {
        self.draw_all(mon);
    }

    fn draw_all(&mut self, mon: &BattleMon) {
        self.nick = mon.nickname;
        self.gender = mon.gender;
        self.draw_nick();
        self.set_level(mon.level);
        self.max_hp = mon.max_hp;
        self.hp = mon.hp;
        if self.side.is_player() {
            self.draw_hp_max();
            self.draw_hp();
        }
        self.status = status_icon(mon.status);
        self.draw_status();
    }

    /// The current HP shown (`UpdateHpTextInHealthbox(HP_CURRENT)`): numbers on the player's
    /// box only, as Gen3 singles.
    pub fn set_hp(&mut self, hp: u16) {
        if hp != self.hp {
            self.hp = hp;
            if self.side.is_player() {
                self.draw_hp();
            }
        }
    }

    pub fn set_level(&mut self, level: u8) {
        self.level = level;
        let mut text = Text::new();
        text.push(CHAR_EXTRA_SYMBOL);
        text.push(CHAR_LV_2);
        decimal(level as u16, false, &mut text);
        let digits = text.len - 2;
        let mut w = Window::new();
        w.print(
            self.gfx.small_font,
            text.as_slice(),
            5 * (3 - digits.min(3)),
            3,
        );
        self.text_into(&w, layout(self.side, self.double).level, 0, 3);
    }

    pub fn set_status(&mut self, status: Status) {
        let icon = status_icon(status);
        if icon != self.status {
            self.status = icon;
            self.draw_status();
        }
    }

    fn draw_nick(&mut self) {
        let mut w = Window::new();
        let len = self.nick.iter().position(|&b| b == EOS).unwrap_or(NAME_LEN);
        let font = self.gfx.small_font;
        let x = w.print(font, &self.nick[..len], 0, 3);
        match self.gender {
            Gender::Male => {
                w.glyph(font, CHAR_MALE as u16, x, 3, COLOR_MALE);
            }
            Gender::Female => {
                w.glyph(font, CHAR_FEMALE as u16, x, 3, COLOR_FEMALE);
            }
            Gender::Genderless => {}
        }
        for &(dst, src, n) in layout(self.side, self.double).nick {
            self.text_into(&w, dst, src, n);
        }
    }

    fn draw_hp(&mut self) {
        let mut text = Text::new();
        decimal(self.hp, true, &mut text);
        text.push(CHAR_SLASH);
        let mut w = Window::new();
        w.print(self.gfx.small_font, text.as_slice(), 4, 5);
        for (dst, src) in HP_CUR_TILES {
            self.put(dst, w.0[src]);
        }
    }

    fn draw_hp_max(&mut self) {
        let mut text = Text::new();
        decimal(self.max_hp, true, &mut text);
        let mut w = Window::new();
        w.print(self.gfx.small_font, text.as_slice(), 0, 5);
        for (dst, src) in HP_MAX_TILES {
            self.put(dst, w.0[src]);
        }
    }

    /// `UpdateStatusIconInHealthbox`.
    fn draw_status(&mut self) {
        let l = layout(self.side, self.double);
        let g = self.gfx;
        let get = |t: &'static [Tile], i: usize| t.get(i).copied().unwrap_or([0; 8]);
        let hp_label = [get(g.hpbar.tiles, 1), get(g.hpbar.tiles, 2)];
        let label = match self.status {
            None => {
                for i in 0..3 {
                    self.put(l.status + i, get(g.misc.tiles, MISC_BLANK_STATUS));
                }
                hp_label
            }
            Some(icon) => {
                let set = if !self.side.is_player() { 15 } else { 0 };
                for i in 0..3 {
                    let t = get(g.status.tiles, set + icon as usize * 3 + i);
                    self.put(l.status + i, t);
                }
                self.palette[l.status_color] = STATUS_COLORS[icon as usize];
                self.palette_dirty = true;
                if !self.side.is_player() {
                    [get(g.hpbar.tiles, 0), get(g.misc.tiles, MISC_FRAME_END)]
                } else {
                    hp_label
                }
            }
        };
        if label != self.label {
            self.label = label;
            self.label_dirty = true;
        }
    }

    /// `TextIntoHealthboxObject(box + dst, window + src, n)`.
    fn text_into(&mut self, w: &Window, dst: usize, src: usize, n: usize) {
        for i in 0..n {
            self.put(dst + 8 + i, w.0[src + 8 + i]);
            let mut t = self.tiles[dst + i];
            t[5..].copy_from_slice(&w.0[src + i][5..]);
            self.put(dst + i, t);
        }
    }

    fn put(&mut self, i: usize, t: Tile) {
        if i < self.tiles.len() && self.tiles[i] != t {
            self.tiles[i] = t;
            self.dirty |= 1 << i;
        }
    }

    /// Visible at its place.
    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.slide = 0;
        self.off = (0, 0);
    }

    /// `StartHealthboxSlideIn` + `SetHealthboxSpriteVisible`: from 115 px outside, 5 px/frame.
    pub fn slide_in(&mut self) {
        self.visible = true;
        let (start, speed) = if self.side.is_player() {
            (SLIDE_START, SLIDE_SPEED)
        } else {
            (-SLIDE_START, -SLIDE_SPEED)
        };
        self.off.0 = start;
        self.slide = speed;
    }

    /// `DoHitAnimHealthboxEffect`: shakes 1 px up and down for 21 frames.
    pub fn shake(&mut self) {
        self.shake = 1;
    }

    pub fn busy(&self) -> bool {
        self.slide != 0 || self.shake != 0
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// The HP the numbers show (`set_hp` / `set_mon`).
    pub fn hp(&self) -> u16 {
        self.hp
    }

    /// Tile uploads still pending.
    pub fn pending(&self) -> u32 {
        self.dirty.count_ones() + 2 * self.label_dirty as u32
    }

    /// Top-left of the box this frame.
    pub fn pos(&self) -> (i16, i16) {
        let p = layout(self.side, self.double).pos;
        (p.0 + self.off.0, p.1 + self.off.1)
    }

    pub fn update(&mut self, f: &mut Frame) {
        if self.slide != 0 {
            self.off.0 -= self.slide;
            if self.off.0 == 0 {
                self.slide = 0;
            }
        }
        if self.shake != 0 {
            if self.shake >= SHAKE_FRAMES {
                self.off.1 = 0;
                self.shake = 0;
            } else {
                self.off.1 = if self.shake % 2 == 1 { 1 } else { -1 };
                self.shake += 1;
            }
        }
        let budget = HEALTHBOX_TILES_PER_FRAME.min(super::obj_tiles_left(f));
        self.flush(f, budget);
        if self.visible {
            self.push_objs(f);
        }
    }

    fn flush(&mut self, f: &mut Frame, budget: usize) {
        let l = layout(self.side, self.double);
        let mut left = budget;
        if self.palette_dirty {
            f.cmds.push(Cmd::ObjPalette {
                bank: l.palette,
                colors: self.palette,
            });
            self.palette_dirty = false;
        }
        if self.label_dirty && left >= 2 {
            for (i, t) in self.label.iter().enumerate() {
                f.cmds.push(Cmd::ObjTile {
                    index: l.bar + i as u16,
                    tile: *t,
                });
            }
            self.label_dirty = false;
            left -= 2;
        }
        while self.dirty != 0 && left > 0 {
            let i = self.dirty.trailing_zeros() as usize;
            self.dirty &= !(1 << i);
            f.cmds.push(Cmd::ObjTile {
                index: l.vram + i as u16,
                tile: self.tiles[i],
            });
            left -= 1;
        }
    }

    fn push_objs(&self, f: &mut Frame) {
        let l = layout(self.side, self.double);
        let (x, y) = self.pos();
        let o = |x, y, tile, palette, (shape, size)| Obj {
            x,
            y,
            tile,
            palette,
            shape,
            size,
            priority: OBJ_PRIORITY_BOX,
            ..Obj::default()
        };
        // The bar sprite is created after the box but drawn over it (`SUBSPRITES_IGNORE_PRIORITY`,
        // priority 1, lower OAM index): it goes first. Subsprites at -16 and +16 of its centre.
        let (bx, by) = (x + l.bar_pos.0, y + l.bar_pos.1);
        let wide = (ObjShape::Wide, 1);
        f.objs.push(o(bx, by, l.bar, obj::PAL_HP_BAR, wide));
        f.objs
            .push(o(bx + 32, by, l.bar + 4, obj::PAL_HP_BAR, wide));
        let half = (l.tiles / 2) as u16;
        f.objs.push(o(x, y, l.vram, l.palette, l.shape));
        f.objs.push(o(x + 64, y, l.vram + half, l.palette, l.shape));
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            side_tag(self.side),
            self.visible as u8,
            self.shake,
            self.label_dirty as u8,
            self.palette_dirty as u8,
            self.level,
            self.gender as u8,
            self.status.map_or(0xFF, |s| s),
        ]);
        h.write(&self.nick);
        h.write_u16(self.hp);
        h.write_u16(self.max_hp);
        hash_i16(h, self.off.0);
        hash_i16(h, self.off.1);
        hash_i16(h, self.slide);
        for t in &self.label {
            for w in t {
                h.write_u32(*w);
            }
        }
        for c in self.palette {
            h.write_u16(c);
        }
        h.write(&self.dirty.to_le_bytes());
    }
}

/// Bits `from..to` set.
fn mask(from: usize, to: usize) -> u128 {
    let upto = |n: usize| {
        if n >= 128 {
            u128::MAX
        } else {
            (1u128 << n) - 1
        }
    };
    upto(to) & !upto(from)
}

#[cfg(test)]
mod tests;
