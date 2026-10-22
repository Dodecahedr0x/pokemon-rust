//! The glyph canvas and the text printer state machine, shared by the field [`TextBox`] and the
//! battle windows (`battle::ui`).
//!
//! [`TextBox`]: super::TextBox

use super::{
    DYNAMIC, EOS, EXT_ARG_LEN, EXT_COLOR, EXT_COLOR_HIGHLIGHT_SHADOW, EXT_CTRL, EXT_ESCAPE,
    EXT_HIGHLIGHT, EXT_PAUSE, EXT_PAUSE_UNTIL_PRESS, EXT_SHADOW, EXTRA_SYMBOL, GLYPH_HEIGHT,
    KEYPAD_ICON, LINE_HEIGHT, NEWLINE, PLACEHOLDER, PROMPT_CLEAR, PROMPT_SCROLL, SCROLL_SPEED,
    Window, unit_len,
};
use crate::Tile;
use crate::data::{Font, WindowGfx};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::video::{Cmd, Frame};
use alloc::vec;
use alloc::vec::Vec;

/// `{CLEAR}`, `{SKIP}` and `{CLEAR_TO}` (`EXT_CTRL_CODE_*`): only [`Canvas::draw_text`] (the
/// instant menu printer) handles them; the timed [`Printer`] skips them, as M3 did.
const EXT_CLEAR: u8 = 0x11;
const EXT_SKIP: u8 = 0x12;
const EXT_CLEAR_TO: u8 = 0x13;

/// Text colors (palette indices).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Colors {
    pub fg: u8,
    pub shadow: u8,
    pub bg: u8,
}

impl Colors {
    pub const fn new(fg: u8, shadow: u8, bg: u8) -> Self {
        Self { fg, shadow, bg }
    }

    pub fn of(w: &WindowGfx) -> Self {
        Self {
            fg: w.text_fg,
            shadow: w.text_shadow,
            bg: w.text_bg,
        }
    }
}

/// Text colors plus a lookup like Gen3 `sFontHalfRowLookupTable`, by nibble: two font pixels
/// (2bpp, leftmost in bits 3-2) -> two canvas nibbles (leftmost in the low one). A font byte
/// (4 pixels, leftmost in bits 7-6) is two lookups. Small, as a pen is made per menu text.
pub struct Pen {
    pub(crate) colors: Colors,
    two: [u16; 16],
}

impl Pen {
    pub fn new(colors: Colors) -> Self {
        let map = [colors.bg, colors.fg, colors.shadow, colors.bg].map(|c| c as u16 & 0xF);
        let mut two = [0u16; 16];
        for (n, t) in two.iter_mut().enumerate() {
            *t = map[n >> 2] | map[n & 3] << 4;
        }
        Self { colors, two }
    }

    pub const fn colors(&self) -> Colors {
        self.colors
    }

    /// 8 pixels of a glyph tile row (`.latfont` u16 as little-endian bytes) as a 4bpp row.
    pub(crate) fn row(&self, lo: u8, hi: u8) -> u32 {
        let half =
            |b: u8| self.two[b as usize >> 4] as u32 | (self.two[b as usize & 0xF] as u32) << 8;
        half(hi) | half(lo) << 16
    }
}

/// Mask of the first `n` (<= 8) pixels of a 4bpp row.
pub(crate) const fn px_mask(n: usize) -> u32 {
    if n >= 8 { u32::MAX } else { (1 << (4 * n)) - 1 }
}

const fn fill_word(c: u8) -> u32 {
    (c as u32 & 0xF) * 0x1111_1111
}

/// A window's pixels as 4bpp tiles, row-major, with per-tile dirty flags. `flush` uploads the
/// dirty tiles to `char_block` from tile `win.base()` on.
pub struct Canvas {
    pub(crate) win: Window,
    char_block: u8,
    pub(crate) tiles: Vec<Tile>,
    dirty: Vec<bool>,
    /// What [`Canvas::redraw`] last drew, while nothing else has drawn since: redrawing the
    /// same text is then skipped (text is slow on the GBA). A cache, not state.
    memo: Option<(Vec<u8>, i16, i16, Colors)>,
}

impl Canvas {
    pub fn new(win: Window, char_block: u8) -> Self {
        let n = win.width as usize * win.height as usize;
        Self {
            win,
            char_block,
            tiles: vec![[0; 8]; n],
            dirty: vec![false; n],
            memo: None,
        }
    }

    /// `fill(colors.bg)` then `draw_text(font, text, x, y, colors)`, skipped when the canvas
    /// still shows exactly that (the same call last, nothing drawn since). Its tiles then stay
    /// clean: nothing to upload either.
    pub fn redraw(&mut self, font: &Font, text: &[u8], x: i16, y: i16, colors: Colors) {
        if let Some((t, mx, my, c)) = &self.memo
            && (t.as_slice(), *mx, *my, *c) == (text, x, y, colors)
        {
            return;
        }
        self.fill(colors.bg);
        self.draw_text(font, text, x, y, colors);
        self.memo = Some((text.to_vec(), x, y, colors));
    }

    pub const fn window(&self) -> Window {
        self.win
    }

    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    fn w_px(&self) -> i16 {
        self.win.width as i16 * 8
    }

    fn h_px(&self) -> i16 {
        self.win.height as i16 * 8
    }

    /// Palette index of pixel (x, y), 0 outside the canvas.
    pub fn pixel(&self, x: i16, y: i16) -> u8 {
        if x < 0 || y < 0 || x >= self.w_px() || y >= self.h_px() {
            return 0;
        }
        let (x, y) = (x as usize, y as usize);
        let t = self.tiles[(y / 8) * self.win.width as usize + x / 8];
        ((t[y % 8] >> ((x % 8) * 4)) & 0xF) as u8
    }

    pub fn fill(&mut self, c: u8) {
        self.memo = None;
        self.tiles.fill([fill_word(c); 8]);
        self.dirty.fill(true);
    }

    pub fn fill_rect(&mut self, x: i16, y: i16, w: i16, h: i16, c: u8) {
        self.memo = None;
        // Clipped to the canvas, then whole tile rows at a time (masked at both ends).
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = (
            x.saturating_add(w).min(self.w_px()),
            y.saturating_add(h).min(self.h_px()),
        );
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let (x0, x1, y0, y1) = (x0 as usize, x1 as usize, y0 as usize, y1 as usize);
        let (w_tiles, word) = (self.win.width as usize, fill_word(c));
        for tx in x0 / 8..x1.div_ceil(8) {
            let from = x0.max(tx * 8) - tx * 8;
            let to = x1.min(tx * 8 + 8) - tx * 8;
            let mask = px_mask(to) & !px_mask(from);
            for py in y0..y1 {
                let t = (py / 8) * w_tiles + tx;
                let r = &mut self.tiles[t][py % 8];
                *r = (*r & !mask) | (word & mask);
                self.dirty[t] = true;
            }
        }
    }

    /// Gen3 `ScrollWindow` upwards by `n` pixels, filling the bottom with `c`.
    fn scroll_up(&mut self, n: i16, c: u8) {
        self.memo = None;
        let (w, h) = (self.win.width as usize, self.h_px() as usize);
        for tx in 0..w {
            for py in 0..h {
                let src = py + n as usize;
                let word = if src < h {
                    self.tiles[(src / 8) * w + tx][src % 8]
                } else {
                    fill_word(c)
                };
                self.tiles[(py / 8) * w + tx][py % 8] = word;
            }
        }
        self.dirty.fill(true);
    }

    /// Writes the pixels of `w` under mask `m` into row `row` of tile `tx` of the tile row
    /// starting at `base` (nothing past the canvas' right edge).
    #[inline(always)]
    fn put(&mut self, base: usize, tx: usize, w_tiles: usize, row: usize, w: u32, m: u32) {
        if tx >= w_tiles || m == 0 {
            return;
        }
        let t = base + tx;
        let r = &mut self.tiles[t][row];
        *r = (*r & !m) | (w & m);
        self.dirty[t] = true;
    }

    /// Draws glyph `id` with its top-left at (x, y) as Gen3 `CopyGlyphToWindow` does: `width`
    /// columns and 15 rows, 2bpp values 0/3 -> bg, 1 -> fg, 2 -> shadow. Returns the advance.
    /// Works on whole tile rows: each 8-pixel glyph half is shifted into two canvas words.
    pub fn draw_glyph(&mut self, font: &Font, id: u16, x: i16, y: i16, pen: &Pen) -> i16 {
        self.memo = None;
        let (Some(glyph), Some(&width)) =
            (font.glyphs.get(id as usize), font.widths.get(id as usize))
        else {
            return 0;
        };
        if x < 0 {
            return width as i16;
        }
        let (x, w_tiles) = (x as usize, self.win.width as usize);
        let width_px = (width as usize).min(16);
        let h_px = self.h_px() as i32;
        // 32-bit words only (a u64 shift is a library call on the GBA): a glyph half's row
        // lands in its tile shifted left, and its overflow in the next tile shifted right.
        let shift = (x % 8) * 4;
        let halves = width_px.div_ceil(8);
        for row in 0..GLYPH_HEIGHT as i32 {
            let cy = y as i32 + row;
            if cy < 0 || cy >= h_px {
                continue;
            }
            let (cy, row) = (cy as usize, row as usize);
            let base = (cy / 8) * w_tiles;
            for half in 0..halves {
                let off = ((row / 8) * 2 + half) * 16 + (row % 8) * 2;
                let word = pen.row(glyph[off], glyph[off + 1]);
                let mask = px_mask(width_px - half * 8);
                let tx = x / 8 + half;
                self.put(base, tx, w_tiles, cy % 8, word << shift, mask << shift);
                if shift != 0 {
                    let (w, m) = (word >> (32 - shift), mask >> (32 - shift));
                    self.put(base, tx + 1, w_tiles, cy % 8, w, m);
                }
            }
        }
        width as i16
    }

    /// Draws one line of `text` (stops at 0xFF or `\n`), skipping control codes.
    pub fn draw_line(&mut self, font: &Font, text: &[u8], mut x: i16, y: i16, pen: &Pen) {
        let mut i = 0;
        while let Some(&b) = text.get(i) {
            match b {
                EOS | NEWLINE => break,
                PROMPT_CLEAR | PROMPT_SCROLL => {}
                EXTRA_SYMBOL => {
                    if let Some(&a) = text.get(i + 1) {
                        x = x.saturating_add(self.draw_glyph(font, 0x100 | a as u16, x, y, pen));
                    }
                }
                EXT_CTRL | PLACEHOLDER | DYNAMIC | KEYPAD_ICON => {}
                c => x = x.saturating_add(self.draw_glyph(font, c as u16, x, y, pen)),
            }
            i += unit_len(text, i);
        }
    }

    /// Draws all of `text` at once from (x, y), as a Gen3 printer of speed 0 does: `\n` (and
    /// `\l`, `\p`) start a new line at `x`, `{CLEAR_TO n}` / `{SKIP n}` move to `x + n` (the
    /// first clearing the span to the background), `{CLEAR n}` clears `n` pixels, the color
    /// codes change `colors` and `{ESCAPE n}` draws glyph 0x100 + n. Other codes are skipped.
    pub fn draw_text(&mut self, font: &Font, text: &[u8], x: i16, y: i16, colors: Colors) {
        self.memo = None;
        let mut pen = Pen::new(colors);
        let (mut cx, mut cy) = (x, y);
        let mut i = 0;
        while let Some(&b) = text.get(i) {
            let arg = |k: usize| text.get(i + k).copied().unwrap_or(0);
            match b {
                EOS => break,
                NEWLINE | PROMPT_CLEAR | PROMPT_SCROLL => {
                    cx = x;
                    cy = cy.saturating_add(LINE_HEIGHT);
                }
                EXTRA_SYMBOL => {
                    let g = 0x100 | arg(1) as u16;
                    cx = cx.saturating_add(self.draw_glyph(font, g, cx, cy, &pen));
                }
                EXT_CTRL => {
                    let c = pen.colors;
                    match arg(1) {
                        EXT_COLOR => pen = Pen::new(Colors { fg: arg(2), ..c }),
                        EXT_HIGHLIGHT => pen = Pen::new(Colors { bg: arg(2), ..c }),
                        EXT_SHADOW => {
                            pen = Pen::new(Colors {
                                shadow: arg(2),
                                ..c
                            })
                        }
                        EXT_COLOR_HIGHLIGHT_SHADOW => {
                            pen = Pen::new(Colors::new(arg(2), arg(4), arg(3)));
                        }
                        EXT_ESCAPE => {
                            let g = 0x100 | arg(2) as u16;
                            cx = cx.saturating_add(self.draw_glyph(font, g, cx, cy, &pen));
                        }
                        EXT_CLEAR => {
                            let w = arg(2) as i16;
                            self.fill_rect(cx, cy, w, LINE_HEIGHT, c.bg);
                            cx = cx.saturating_add(w);
                        }
                        EXT_SKIP => cx = x.saturating_add(arg(2) as i16),
                        EXT_CLEAR_TO => {
                            let to = x.saturating_add(arg(2) as i16);
                            if to > cx {
                                self.fill_rect(cx, cy, to - cx, LINE_HEIGHT, c.bg);
                                cx = to;
                            }
                        }
                        _ => {}
                    }
                }
                PLACEHOLDER | DYNAMIC | KEYPAD_ICON => {}
                c => cx = cx.saturating_add(self.draw_glyph(font, c as u16, cx, cy, &pen)),
            }
            i += unit_len(text, i);
        }
    }

    /// Uploads the dirty tiles (`Cmd::BgTile`) and clears their flags.
    pub fn flush(&mut self, f: &mut Frame) {
        for (i, d) in self.dirty.iter_mut().enumerate() {
            if core::mem::take(d) {
                f.cmds.push(Cmd::BgTile {
                    char_block: self.char_block,
                    index: self.win.base + i as u16,
                    tile: self.tiles[i],
                });
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
    /// No message, or the message has been fully printed.
    Idle,
    Printing,
    /// `{PAUSE n}`: frames left.
    Pause(u8),
    /// `{PAUSE_UNTIL_PRESS}`.
    WaitPress,
    /// `\l`: waiting for A/B before scrolling.
    WaitScroll,
    /// Pixels left to scroll.
    Scrolling(u8),
    /// `\p`: waiting for A/B before clearing.
    WaitClear,
}

impl State {
    fn tag(self) -> [u8; 2] {
        match self {
            State::Idle => [0, 0],
            State::Printing => [1, 0],
            State::Pause(n) => [2, n],
            State::WaitPress => [3, 0],
            State::WaitScroll => [4, 0],
            State::Scrolling(n) => [5, n],
            State::WaitClear => [6, 0],
        }
    }
}

const fn ab(pressed: Buttons) -> bool {
    pressed.0 & (Buttons::A.0 | Buttons::B.0) != 0
}

/// A timed text printer on a [`Canvas`]: Gen3 `RenderText` for `\n`, `\l`, `\p`, pauses and
/// color codes, with `frames_per_char` frames per drawn glyph (control codes cost nothing) and
/// A/B speed-up (`hasPrintBeenSpedUp`: a new press skips the current delay, then one glyph per
/// frame while A or B stays held).
pub struct Printer {
    pub(crate) canvas: Canvas,
    pub(crate) state: State,
    pub(crate) text: Vec<u8>,
    /// Digest of `text`, computed once per message so the per-frame hash stays O(1).
    pub(crate) text_digest: u64,
    pub(crate) pos: usize,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) delay: u8,
    /// A/B was pressed while printing this message (Gen3 `hasPrintBeenSpedUp`).
    pub(crate) sped_up: bool,
    pub(crate) pen: Pen,
    /// Printer origin inside the window (`AddTextPrinterParameterized`'s x and y).
    origin: (i16, i16),
    frames_per_char: u8,
}

impl Printer {
    /// A printer on window `win` (canvas tiles in `char_block`), starting each page at `origin`.
    pub fn new(win: Window, char_block: u8, origin: (i16, i16), frames_per_char: u8) -> Self {
        Self {
            canvas: Canvas::new(win, char_block),
            state: State::Idle,
            text: Vec::new(),
            text_digest: 0,
            pos: 0,
            x: origin.0,
            y: origin.1,
            delay: 0,
            sped_up: false,
            pen: Pen::new(Colors::default()),
            origin,
            frames_per_char: frames_per_char.max(1),
        }
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// Sets the colors and fills the canvas with their background; stops printing.
    pub fn clear(&mut self, colors: Colors) {
        self.pen = Pen::new(colors);
        self.canvas.fill(self.pen.colors.bg);
        self.state = State::Idle;
    }

    /// Starts printing `text` (Gen3-encoded, expanded, 0xFF-terminated) in `colors`, clearing
    /// the canvas first if `clear`. The first character appears on the next `update`.
    pub fn start(&mut self, text: Vec<u8>, colors: Colors, clear: bool) {
        self.text = text;
        let mut d = Fnv64::new();
        d.write(&self.text);
        self.text_digest = d.finish();
        self.pos = 0;
        self.x = self.origin.0;
        self.y = self.origin.1;
        self.delay = 0;
        self.sped_up = false;
        self.pen = Pen::new(colors);
        if clear {
            self.canvas.fill(self.pen.colors.bg);
        }
        self.state = State::Printing;
    }

    /// Advances one frame. `pressed` = buttons newly pressed this frame, `held` = all held.
    pub fn update(&mut self, font: &Font, pressed: Buttons, held: Buttons) {
        let press = ab(pressed);
        match self.state {
            State::Idle => {}
            State::Printing => {
                // Gen3 `RenderText`, RENDER_STATE_HANDLE_CHAR.
                if self.sped_up && ab(held) {
                    self.delay = 0;
                }
                if self.delay > 0 {
                    self.delay -= 1;
                    if press {
                        self.sped_up = true;
                        self.delay = 0;
                    }
                } else {
                    self.step(font);
                }
            }
            State::Pause(n) => {
                self.state = if n > 0 {
                    State::Pause(n - 1)
                } else {
                    State::Printing
                };
            }
            State::WaitPress if press => self.state = State::Printing,
            State::WaitScroll if press => {
                self.x = self.origin.0;
                self.state = State::Scrolling(LINE_HEIGHT as u8);
            }
            State::Scrolling(0) => self.state = State::Printing,
            State::Scrolling(n) => {
                let s = n.min(SCROLL_SPEED);
                self.canvas.scroll_up(s as i16, self.pen.colors.bg);
                self.state = State::Scrolling(n - s);
            }
            State::WaitClear if press => {
                self.canvas.fill(self.pen.colors.bg);
                self.x = self.origin.0;
                self.y = self.origin.1;
                self.state = State::Printing;
            }
            State::WaitPress | State::WaitScroll | State::WaitClear => {}
        }
    }

    fn next(&mut self) -> u8 {
        let b = self.text.get(self.pos).copied().unwrap_or(EOS);
        self.pos += 1;
        b
    }

    /// Consumes codes until a glyph is drawn or the state changes (modelled on Gen3 `RenderText`;
    /// control codes take no frames here).
    fn step(&mut self, font: &Font) {
        while self.state == State::Printing {
            let b = self.next();
            let glyph = match b {
                EOS => {
                    self.state = State::Idle;
                    return;
                }
                NEWLINE => {
                    self.x = self.origin.0;
                    self.y = self.y.saturating_add(LINE_HEIGHT);
                    continue;
                }
                PROMPT_SCROLL => {
                    self.state = State::WaitScroll;
                    return;
                }
                PROMPT_CLEAR => {
                    self.state = State::WaitClear;
                    return;
                }
                PLACEHOLDER | DYNAMIC | KEYPAD_ICON => {
                    self.pos += 1;
                    continue;
                }
                EXTRA_SYMBOL => 0x100 | self.next() as u16,
                EXT_CTRL => match self.ext_ctrl() {
                    Some(g) => g,
                    None => continue,
                },
                c => c as u16,
            };
            let w = self
                .canvas
                .draw_glyph(font, glyph, self.x, self.y, &self.pen);
            self.x = self.x.saturating_add(w);
            self.delay = self.frames_per_char - 1;
            // The message is complete as soon as its last character is on screen.
            if self.text.get(self.pos).is_none_or(|&b| b == EOS) {
                self.state = State::Idle;
            }
            return;
        }
    }

    /// Handles an `FC` code; returns a glyph to draw for `{ESCAPE n}`.
    fn ext_ctrl(&mut self) -> Option<u16> {
        let code = self.next();
        let &len = EXT_ARG_LEN.get(code as usize)?;
        let at = self.pos;
        let arg = |i: usize| self.text.get(at + i).copied().unwrap_or(0);
        let (a0, a1, a2) = (arg(0), arg(1), arg(2));
        self.pos += len as usize;
        let c = self.pen.colors;
        match code {
            EXT_COLOR => self.pen = Pen::new(Colors { fg: a0, ..c }),
            EXT_HIGHLIGHT => self.pen = Pen::new(Colors { bg: a0, ..c }),
            EXT_SHADOW => self.pen = Pen::new(Colors { shadow: a0, ..c }),
            EXT_COLOR_HIGHLIGHT_SHADOW => {
                self.pen = Pen::new(Colors {
                    fg: a0,
                    bg: a1,
                    shadow: a2,
                })
            }
            EXT_PAUSE => self.state = State::Pause(a0),
            EXT_PAUSE_UNTIL_PRESS => self.state = State::WaitPress,
            EXT_ESCAPE => return Some(0x100 | a0 as u16),
            _ => {}
        }
        None
    }

    /// True while a message is being printed, including its `\p`/`\l`/pause waits.
    pub fn is_printing(&self) -> bool {
        self.state != State::Idle
    }

    /// Uploads the canvas tiles changed since the last flush.
    pub fn flush(&mut self, f: &mut Frame) {
        self.canvas.flush(f);
    }

    /// Feeds the printer state into `h`: delay, speed-up, colors, state, position, text length
    /// and digest (the canvas follows from them). The field `TextBox` hash embeds these bytes.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.delay,
            self.sped_up as u8,
            self.pen.colors.fg,
            self.pen.colors.shadow,
            self.pen.colors.bg,
        ]);
        h.write(&self.state.tag());
        h.write_u32(self.pos as u32);
        h.write_u16(self.x as u16);
        h.write_u16(self.y as u16);
        h.write_u32(self.text.len() as u32);
        h.write(&self.text_digest.to_le_bytes());
    }
}

#[cfg(test)]
mod tests;
