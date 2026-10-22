//! Field menus (Milestone 8 Task H): the script multichoice (Gen3 `ScriptMenu_Multichoice`,
//! `script_menu.c`), shown by the field [`TextBox`] on BG0 next to the message window and the
//! yes/no box, with their frame tiles, colors and cursor.
//!
//! Gen3 draws the whole window at once. Here the window appears a choice at a time, at most
//! [`GLYPHS_PER_FRAME`] glyphs a frame (a choice's two tile rows are cleared with its first
//! glyphs, its map rows written with its last), so a frame's cost stays bounded whatever the
//! list (the M3 text budget): a full-width choice (28 tiles) holds up to about 50 glyphs, and
//! drawn at once cost more than a frame on the GBA. Input waits as Gen3's does
//! (`sProcessInputDelay`: from the third update on), and also until every choice is drawn; for
//! up to three short choices that is the same frame.
//!
//! [`TextBox`]: crate::text::TextBox

use crate::data::{Font, GameData};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::text::{
    CHAR_CURSOR, CURSOR_H, CURSOR_W, Canvas, Colors, EOS, EXTRA_SYMBOL, LINE_HEIGHT, NEWLINE, Pen,
    TEXT_Y, Window, YES_NO_TEXT_X, unit_len,
};
use crate::video::{Cmd, Frame, MAP_SIZE, layout};
use alloc::vec;
use alloc::vec::Vec;

/// Gen3 `MAX_MULTICHOICE_WIDTH` (tiles).
pub const MAX_WIDTH: u8 = 28;
/// First canvas tile of the multichoice window in `TEXT_CHAR_BLOCK` (after the yes/no canvas).
pub const CANVAS_BASE: u16 = 192;
/// Canvas tiles left in the char block from [`CANVAS_BASE`]; longer lists are cut to fit.
const CANVAS_TILES: usize = 512 - CANVAS_BASE as usize;
/// Gen3 `sProcessInputDelay` for a multichoice: updates before input is read.
pub const INPUT_DELAY: u8 = 2;
/// Gen3 `MULTI_B_PRESSED`.
pub const B_PRESSED: u16 = crate::script::MULTI_B_PRESSED;
/// Glyphs drawn per frame at most (half on the opening frame). Measured on the GBA (scratch
/// build, five choices of 34 and 60 glyphs): 91–129 lines of step a frame while drawing; one
/// whole 20-glyph choice a frame peaked at 195 lines with the window's opening, and whole
/// 34- or 60-glyph choices ran late.
pub const GLYPHS_PER_FRAME: u8 = 12;
/// Tile rows of the screen (BG0 map rows the window may use).
const SCREEN_ROWS: u8 = 20;

/// Gen3 `GetStringWidth(FONT_NORMAL, text, 0)`: the pixel width of the longest line of `text`
/// (glyph widths; control codes and placeholders take none).
pub fn text_width(font: &Font, text: &[u8]) -> i16 {
    let (mut w, mut best) = (0i16, 0i16);
    let mut i = 0;
    while let Some(&b) = text.get(i) {
        let glyph = match b {
            EOS => break,
            NEWLINE => {
                best = best.max(w);
                w = 0;
                None
            }
            EXTRA_SYMBOL => text.get(i + 1).map(|&a| 0x100 | a as usize),
            c if c >= 0xF7 => None,
            c => Some(c as usize),
        };
        if let Some(&gw) = glyph.and_then(|g| font.widths.get(g)) {
            w = w.saturating_add(gw as i16);
        }
        i += unit_len(text, i);
    }
    best.max(w)
}

/// Gen3 `ConvertPixelWidthToTileWidth`.
pub const fn tile_width(px: i16) -> u8 {
    let px = if px < 0 { 0 } else { px as i32 };
    let t = (px + 9) / 8 + 1;
    if t > MAX_WIDTH as i32 {
        MAX_WIDTH
    } else {
        t as u8
    }
}

/// Gen3 `ScriptMenu_AdjustLeftCoordFromWidth`.
pub const fn adjust_left(left: u8, width: u8) -> u8 {
    if left as u16 + width as u16 > MAX_WIDTH as u16 {
        MAX_WIDTH.saturating_sub(width)
    } else {
        left
    }
}

/// An open multichoice window: Gen3 `DrawMultichoiceMenu` and `Task_HandleMultichoiceInput`.
pub struct Multichoice {
    items: &'static [&'static [u8]],
    list: u8,
    canvas: Canvas,
    /// BG0 map entries of the window and its frame, rows `top - 1 ..= top + height`.
    entries: Vec<u16>,
    /// Choices drawn so far.
    drawn: u8,
    /// In choice `drawn`: the byte of its text drawn next (0: the choice is not started), and
    /// the pen's x.
    at: u16,
    pen_x: i16,
    cursor: u8,
    /// Gen3 `sProcessInputDelay`.
    delay: u8,
    /// More than three choices: the cursor wraps (Gen3 `tDoWrap`).
    wrap: bool,
    /// B answers [`B_PRESSED`] (Gen3 `!tIgnoreBPress`).
    cancel: bool,
}

/// What an update of the menu decided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Still open.
    Open,
    /// Closed with this answer (a choice index, or [`B_PRESSED`]).
    Done(u16),
}

fn screen_block() -> u8 {
    layout::SCREEN_BLOCKS[layout::TEXT_BG as usize]
}

impl Multichoice {
    /// Gen3 `ScriptMenu_MultichoiceWithDefault(left, top, list, ignoreBPress = !cancel,
    /// default)`: lays the window out (width from the longest choice, moved left to fit
    /// [`MAX_WIDTH`]) and clears it; [`Multichoice::update`] then draws it. An out-of-range
    /// `default` starts on the first choice (Gen3 `InitMenu`). Choices that do not fit the
    /// screen or the canvas are left out.
    pub fn new(
        font: &Font,
        list: u8,
        items: &'static [&'static [u8]],
        (left, top): (u8, u8),
        default: u8,
        cancel: bool,
    ) -> Self {
        let px = (items.iter()).fold(0, |w, t| w.max(text_width(font, t)));
        let width = tile_width(px);
        // Gen3 `CreateWindowFromRect(left, top, ..)`: the window one tile right of and below
        // `(left, top)`, where its frame's top-left corner goes.
        let left = adjust_left(left, width) + 1;
        let top = top.min(SCREEN_ROWS - 4) + 1;
        // The bottom frame row (`top + 2 count`) stays on screen.
        let fit_screen = (SCREEN_ROWS - 1 - top) / 2;
        let fit_canvas = (CANVAS_TILES / (2 * width as usize)) as u8;
        let count = (items.len().min(u8::MAX as usize) as u8)
            .min(fit_screen)
            .min(fit_canvas)
            .max(1);
        let items = &items[..(count as usize).min(items.len())];
        let win = Window::new(left, top, width, count * 2, CANVAS_BASE);
        Multichoice {
            items,
            list,
            canvas: Canvas::new(win, layout::TEXT_CHAR_BLOCK),
            entries: win.frame_entries(),
            drawn: 0,
            at: 0,
            pen_x: YES_NO_TEXT_X,
            cursor: if default < count { default } else { 0 },
            delay: INPUT_DELAY,
            wrap: count > 3,
            cancel,
        }
    }

    /// The window (its canvas rectangle).
    pub fn window(&self) -> Window {
        self.canvas.window()
    }

    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// Whether every choice is on screen.
    pub fn is_drawn(&self) -> bool {
        self.drawn as usize >= self.items.len()
    }

    /// One frame: draws the next choice, counts the input delay down, then reads `pressed`
    /// (Gen3 `Menu_ProcessInput`, or `Menu_ProcessInputNoWrap` for three choices or fewer).
    pub fn update(&mut self, game: &impl GameData, pressed: Buttons, f: &mut Frame) -> Choice {
        let pen = Pen::new(Colors::of(game.window()));
        if !self.is_drawn() {
            // The opening frame also lays the window out (and measures every choice): half
            // the glyphs.
            let opening = self.delay == INPUT_DELAY && self.drawn == 0 && self.at == 0;
            let budget = GLYPHS_PER_FRAME >> opening as u8;
            self.draw_next(game.font(), &pen, budget, f);
        }
        if self.delay > 0 {
            self.delay -= 1;
            return Choice::Open;
        }
        if !self.is_drawn() {
            return Choice::Open;
        }
        let count = self.items.len() as u8;
        let to = if pressed.contains(Buttons::A) {
            return self.close(self.cursor as u16, f);
        } else if pressed.contains(Buttons::B) {
            if !self.cancel {
                return Choice::Open;
            }
            return self.close(B_PRESSED, f);
        } else if pressed.contains(Buttons::UP) {
            match self.cursor {
                0 if self.wrap => count - 1,
                0 => 0,
                c => c - 1,
            }
        } else if pressed.contains(Buttons::DOWN) {
            match self.cursor + 1 {
                c if c < count => c,
                _ if self.wrap => 0,
                _ => self.cursor,
            }
        } else {
            return Choice::Open;
        };
        // Gen3 `RedrawMenuCursor` (also when the cursor stays, at either end without wrap).
        let y = TEXT_Y + self.cursor as i16 * LINE_HEIGHT;
        (self.canvas).fill_rect(0, y, CURSOR_W, CURSOR_H, pen.colors().bg);
        self.cursor = to;
        self.draw_cursor(game.font(), &pen);
        self.canvas.flush(f);
        Choice::Open
    }

    fn draw_cursor(&mut self, font: &Font, pen: &Pen) {
        let y = TEXT_Y + self.cursor as i16 * LINE_HEIGHT;
        self.canvas.draw_glyph(font, CHAR_CURSOR, 0, y, pen);
    }

    /// Up to `budget` glyphs of choice `drawn` (Gen3 `PrintMenuTable`: x 8,
    /// y `16 i + 1`): its two tile rows cleared first; when its text ends, the cursor if it is
    /// on it, then its map rows (the top frame row with the first choice, the bottom one with
    /// the last). The tiles drawn are uploaded each frame.
    fn draw_next(&mut self, font: &Font, pen: &Pen, budget: u8, f: &mut Frame) {
        let i = self.drawn;
        let y = i as i16 * LINE_HEIGHT;
        if self.at == 0 {
            let w_px = self.window().width() as i16 * 8;
            (self.canvas).fill_rect(0, y, w_px, LINE_HEIGHT, pen.colors().bg);
        }
        let text = self.items[i as usize];
        let mut at = self.at as usize;
        let mut glyphs = 0;
        let done = loop {
            let Some(&b) = text.get(at) else {
                break true;
            };
            let glyph = match b {
                EOS | NEWLINE => break true,
                EXTRA_SYMBOL => text.get(at + 1).map(|&a| 0x100 | a as u16),
                c if c >= 0xF7 => None,
                c => Some(c as u16),
            };
            if let Some(g) = glyph {
                if glyphs == budget {
                    break false;
                }
                let w = (self.canvas).draw_glyph(font, g, self.pen_x, y + TEXT_Y, pen);
                self.pen_x = self.pen_x.saturating_add(w);
                glyphs += 1;
            }
            at += unit_len(text, at);
        };
        if !done {
            self.at = at.min(u16::MAX as usize) as u16;
            self.canvas.flush(f);
            return;
        }
        if self.cursor == i {
            self.draw_cursor(font, pen);
        }
        self.canvas.flush(f);
        self.drawn += 1;
        self.at = 0;
        self.pen_x = YES_NO_TEXT_X;
        let first = if i == 0 { 0 } else { 1 + 2 * i as usize };
        let last = if self.is_drawn() {
            self.entries.len() / MAP_SIZE - 1
        } else {
            2 + 2 * i as usize
        };
        for row in first..=last {
            self.put_row(row, false, f);
        }
    }

    /// Writes entry row `row` (0 = the frame's top row, one above the window) of the window's
    /// columns, or blanks it. The window starts on screen row 1 or below, so its frame is on
    /// screen.
    fn put_row(&self, row: usize, blank: bool, f: &mut Frame) {
        let win = self.window();
        let screen_row = win.top() as usize + row - 1;
        let c0 = win.left().saturating_sub(2) as usize;
        let c1 = (win.left() as usize + win.width() as usize + 1).min(MAP_SIZE);
        let at = row * MAP_SIZE;
        f.cmds.push(Cmd::MapEntries {
            screen_block: screen_block(),
            start: (screen_row * MAP_SIZE + c0) as u16,
            entries: if blank {
                vec![0; c1 - c0]
            } else {
                self.entries[at + c0..at + c1].to_vec()
            },
        });
    }

    /// Gen3 `ClearToTransparentAndRemoveWindow`: blanks the window's map rows.
    fn close(&mut self, answer: u16, f: &mut Frame) -> Choice {
        for row in 0..self.entries.len() / MAP_SIZE {
            self.put_row(row, true, f);
        }
        Choice::Done(answer)
    }

    /// Feeds the menu state (list, layout, progress, cursor, delay) into `h`; the canvas
    /// follows from it.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        let win = self.window();
        h.write(&[
            self.list,
            win.left(),
            win.top(),
            win.width(),
            self.items.len() as u8,
            self.drawn,
            self.at as u8,
            (self.at >> 8) as u8,
            self.pen_x as u8,
            (self.pen_x >> 8) as u8,
            self.cursor,
            self.delay,
            self.wrap as u8 | (self.cancel as u8) << 1,
        ]);
    }
}

#[cfg(test)]
mod tests;
