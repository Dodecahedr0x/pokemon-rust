//! Dialogue text box on BG0: a Gen3 text printer (variable-width font, `\n`, `\l`, `\p`,
//! placeholders, `{…}` control codes), the dialogue frame (Gen3 `DrawDialogueFrame`) and the
//! yes/no menu.
//!
//! VRAM use (char block `layout::TEXT_CHAR_BLOCK`, screen block `SCREEN_BLOCKS[TEXT_BG]`):
//! - tile 0: blank (cleared map entries show nothing),
//! - tiles `FRAME_BASE..`: `WindowGfx::tiles` (Gen3 loads them at `DLG_WINDOW_BASE_TILE_NUM`),
//! - tiles `MSG_WINDOW.base..`: the message canvas, 27x4 tiles, row-major,
//! - tiles `YES_NO_WINDOW.base..`: the yes/no canvas, 5x4 tiles, row-major.
//!
//! Canvases are kept in RAM; every call that changes them ends by uploading the changed tiles
//! with `Cmd::BgTile`. BG0 scroll is expected to stay (0, 0).

use crate::Tile;
use crate::data::{Font, GameData, UiText, WindowGfx};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, layout};
use alloc::vec;
use alloc::vec::Vec;

/// A text window: its tilemap rectangle (in tiles) and its first canvas tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Window {
    pub left: u8,
    pub top: u8,
    pub width: u8,
    pub height: u8,
    /// First canvas tile in `TEXT_CHAR_BLOCK`.
    pub base: u16,
}

/// Gen3 `sStandardTextBox_WindowTemplates[0]`: frame at tile rows 14..=19, two text lines.
pub const MSG_WINDOW: Window = Window {
    left: 2,
    top: 15,
    width: 27,
    height: 4,
    base: 32,
};

/// Gen3 `sYesNo_WindowTemplates`.
pub const YES_NO_WINDOW: Window = Window {
    left: 21,
    top: 9,
    width: 5,
    height: 4,
    base: 160,
};

/// First tile of `WindowGfx::tiles` in `TEXT_CHAR_BLOCK` (at most `MAX_FRAME_TILES` are loaded).
pub const FRAME_BASE: u16 = 1;
pub const MAX_FRAME_TILES: usize = 31;
/// Gen3 "medium" text speed.
pub const FRAMES_PER_CHAR: u8 = 4;
/// Gen3 `sWindowVerticalScrollSpeeds[OPTIONS_TEXT_SPEED_MID]`, pixels per frame.
pub const SCROLL_SPEED: u8 = 2;
/// `maxLetterHeight + lineSpacing` of `FONT_NORMAL`.
pub const LINE_HEIGHT: i16 = 16;
/// Rows drawn per glyph (`gCurGlyph.height` of `FONT_NORMAL`).
const GLYPH_HEIGHT: i16 = 15;
/// Printer origin inside a window (`AddTextPrinterParameterized2`: x 0, y 1).
const TEXT_X: i16 = 0;
const TEXT_Y: i16 = 1;
/// Yes/no layout (`CreateYesNoMenu`, `InitMenuInUpperLeftCorner`, `sMenuCursorDimensions`).
const YES_NO_TEXT_X: i16 = 8;
const CURSOR_W: i16 = 8;
const CURSOR_H: i16 = 15;
/// `▶` (`gText_SelectorArrow3`).
const CHAR_CURSOR: u16 = 0xEF;

const EOS: u8 = 0xFF;
const NEWLINE: u8 = 0xFE;
const PLACEHOLDER: u8 = 0xFD;
const EXT_CTRL: u8 = 0xFC;
const PROMPT_CLEAR: u8 = 0xFB;
const PROMPT_SCROLL: u8 = 0xFA;
const EXTRA_SYMBOL: u8 = 0xF9;
const KEYPAD_ICON: u8 = 0xF8;
const DYNAMIC: u8 = 0xF7;

const PLACEHOLDER_PLAYER: u8 = 0x01;
const PLACEHOLDER_RIVAL: u8 = 0x06;

const EXT_COLOR: u8 = 0x01;
const EXT_HIGHLIGHT: u8 = 0x02;
const EXT_SHADOW: u8 = 0x03;
const EXT_COLOR_HIGHLIGHT_SHADOW: u8 = 0x04;
const EXT_PAUSE: u8 = 0x08;
const EXT_PAUSE_UNTIL_PRESS: u8 = 0x09;
const EXT_ESCAPE: u8 = 0x0C;
/// Argument bytes of each `EXT_CTRL_CODE` (`GetExtCtrlCodeLength` minus one).
const EXT_ARG_LEN: [u8; 0x19] = [
    0, 1, 1, 1, 3, 1, 1, 0, 1, 0, 0, 2, 1, 1, 1, 0, 2, 1, 1, 1, 1, 0, 0, 0, 0,
];

/// Bytes taken by the code unit starting at `text[i]` (a character or a code and its arguments).
fn unit_len(text: &[u8], i: usize) -> usize {
    match text.get(i).copied() {
        Some(EXT_CTRL) => match text.get(i + 1) {
            Some(&c) if (c as usize) < EXT_ARG_LEN.len() => 2 + EXT_ARG_LEN[c as usize] as usize,
            _ => 2,
        },
        Some(PLACEHOLDER | DYNAMIC | KEYPAD_ICON | EXTRA_SYMBOL) => 2,
        _ => 1,
    }
}

/// Copies `text` up to its 0xFF, expanding `{PLAYER}` and `{RIVAL}`; other placeholders expand
/// to nothing. The result always ends with 0xFF.
pub fn expand(game: &impl GameData, text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < text.len() && text[i] != EOS {
        let n = unit_len(text, i);
        if text[i] == PLACEHOLDER {
            let name: &[u8] = match text.get(i + 1).copied() {
                Some(PLACEHOLDER_PLAYER) => game.new_game().player_name,
                Some(PLACEHOLDER_RIVAL) => game.ui_text(UiText::RivalName),
                _ => &[],
            };
            out.extend(name.iter().take_while(|&&b| b != EOS));
        } else {
            out.extend_from_slice(&text[i..(i + n).min(text.len())]);
        }
        i += n;
    }
    out.push(EOS);
    out
}

/// Text colors (palette indices in `WINDOW_PALETTE`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Colors {
    fg: u8,
    shadow: u8,
    bg: u8,
}

impl Colors {
    fn of(w: &WindowGfx) -> Self {
        Self {
            fg: w.text_fg,
            shadow: w.text_shadow,
            bg: w.text_bg,
        }
    }
}

const fn fill_word(c: u8) -> u32 {
    (c as u32 & 0xF) * 0x1111_1111
}

/// A window's pixels as 4bpp tiles, row-major, with per-tile dirty flags.
struct Canvas {
    win: Window,
    tiles: Vec<Tile>,
    dirty: Vec<bool>,
}

impl Canvas {
    fn new(win: Window) -> Self {
        let n = win.width as usize * win.height as usize;
        Self {
            win,
            tiles: vec![[0; 8]; n],
            dirty: vec![false; n],
        }
    }

    fn w_px(&self) -> i16 {
        self.win.width as i16 * 8
    }

    fn h_px(&self) -> i16 {
        self.win.height as i16 * 8
    }

    fn set(&mut self, x: i16, y: i16, c: u8) {
        if x < 0 || y < 0 || x >= self.w_px() || y >= self.h_px() {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let t = (y / 8) * self.win.width as usize + x / 8;
        let shift = (x % 8) * 4;
        let row = &mut self.tiles[t][y % 8];
        *row = (*row & !(0xF << shift)) | ((c as u32 & 0xF) << shift);
        self.dirty[t] = true;
    }

    fn fill(&mut self, c: u8) {
        self.tiles.fill([fill_word(c); 8]);
        self.dirty.fill(true);
    }

    fn fill_rect(&mut self, x: i16, y: i16, w: i16, h: i16, c: u8) {
        for py in y..y + h {
            for px in x..x + w {
                self.set(px, py, c);
            }
        }
    }

    /// Gen3 `ScrollWindow` upwards by `n` pixels, filling the bottom with `c`.
    fn scroll_up(&mut self, n: i16, c: u8) {
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

    /// Draws glyph `id` with its top-left at (x, y) as Gen3 `CopyGlyphToWindow` does: `width`
    /// columns and 15 rows, 2bpp values 0/3 -> bg, 1 -> fg, 2 -> shadow. Returns the advance.
    fn draw_glyph(&mut self, font: &Font, id: u16, x: i16, y: i16, colors: Colors) -> i16 {
        let (Some(glyph), Some(&width)) =
            (font.glyphs.get(id as usize), font.widths.get(id as usize))
        else {
            return 0;
        };
        let map = [colors.bg, colors.fg, colors.shadow, colors.bg];
        for row in 0..GLYPH_HEIGHT as usize {
            for col in 0..(width as usize).min(16) {
                let off = ((row / 8) * 2 + col / 8) * 16 + (row % 8) * 2;
                let bits = u16::from_le_bytes([glyph[off], glyph[off + 1]]);
                let v = (bits >> (14 - 2 * (col % 8))) & 3;
                self.set(x + col as i16, y + row as i16, map[v as usize]);
            }
        }
        width as i16
    }

    /// Draws one line of `text` (stops at 0xFF or `\n`), skipping control codes.
    fn draw_line(&mut self, font: &Font, text: &[u8], mut x: i16, y: i16, colors: Colors) {
        let mut i = 0;
        while let Some(&b) = text.get(i) {
            match b {
                EOS | NEWLINE => break,
                PROMPT_CLEAR | PROMPT_SCROLL => {}
                EXTRA_SYMBOL => {
                    if let Some(&a) = text.get(i + 1) {
                        x += self.draw_glyph(font, 0x100 | a as u16, x, y, colors);
                    }
                }
                EXT_CTRL | PLACEHOLDER | DYNAMIC | KEYPAD_ICON => {}
                c => x += self.draw_glyph(font, c as u16, x, y, colors),
            }
            i += unit_len(text, i);
        }
    }

    fn flush(&mut self, f: &mut Frame) {
        for (i, d) in self.dirty.iter_mut().enumerate() {
            if core::mem::take(d) {
                f.cmds.push(Cmd::BgTile {
                    char_block: layout::TEXT_CHAR_BLOCK,
                    index: self.win.base + i as u16,
                    tile: self.tiles[i],
                });
            }
        }
    }

    fn hash(&self, h: &mut Fnv64) {
        for t in &self.tiles {
            for &row in t {
                h.write_u32(row);
            }
        }
    }
}

const fn frame_entry(tile: u16, vflip: bool) -> u16 {
    (FRAME_BASE + tile) | ((vflip as u16) << 11) | ((layout::WINDOW_PALETTE as u16) << 12)
}

impl Window {
    /// Screen-block entry index of the first row returned by [`Window::frame_entries`].
    pub const fn frame_start(&self) -> u16 {
        (self.top as u16 - 1) * MAP_SIZE as u16
    }

    /// Map entries for tile rows `top - 1 ..= top + height` (32 per row): the dialogue frame as
    /// Gen3 `WindowFunc_DrawDialogueFrame` lays it out (frame tiles 1, 3, 4, 5, 6 on top, the
    /// same flipped vertically at the bottom, 7 / 9 / 10 on the sides), with the window's
    /// canvas tiles inside. Entries outside the frame are 0.
    pub fn frame_entries(&self) -> Vec<u16> {
        let (l, t, w, h) = (
            self.left as i16,
            self.top as i16,
            self.width as i16,
            self.height as i16,
        );
        let mut e = vec![0u16; (h as usize + 2) * MAP_SIZE];
        let mut fill = |x0: i16, y0: i16, cw: i16, ch: i16, tile: u16, vflip: bool| {
            for y in y0..y0 + ch {
                for x in x0..x0 + cw {
                    if (0..MAP_SIZE as i16).contains(&x) && (t - 1..=t + h).contains(&y) {
                        e[(y - t + 1) as usize * MAP_SIZE + x as usize] = frame_entry(tile, vflip);
                    }
                }
            }
        };
        for (row, vflip) in [(t - 1, false), (t + h, true)] {
            fill(l - 2, row, 1, 1, 1, vflip);
            fill(l - 1, row, 1, 1, 3, vflip);
            fill(l, row, w - 1, 1, 4, vflip);
            fill(l + w - 1, row, 1, 1, 5, vflip);
            fill(l + w, row, 1, 1, 6, vflip);
        }
        fill(l - 2, t, 1, h, 7, false);
        fill(l - 1, t, w + 1, h, 9, false);
        fill(l + w, t, 1, h, 10, false);
        let pal = (layout::WINDOW_PALETTE as u16) << 12;
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let tile = self.base + (y * self.width as usize + x) as u16;
                e[(y + 1) * MAP_SIZE + self.left as usize + x] = tile | pal;
            }
        }
        e
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
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

/// The field dialogue window, its text printer and the yes/no menu.
pub struct TextBox {
    /// BG0 enabled with the window palette and frame tiles loaded.
    bg_on: bool,
    /// Dialogue frame drawn.
    open: bool,
    state: State,
    text: Vec<u8>,
    pos: usize,
    x: i16,
    y: i16,
    delay: u8,
    colors: Colors,
    msg: Canvas,
    yes_no: Canvas,
    /// Cursor position (0 = yes) while the yes/no menu is shown.
    yes_no_cursor: Option<u8>,
    yes_no_result: Option<bool>,
}

impl Default for TextBox {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBox {
    pub fn new() -> Self {
        Self {
            bg_on: false,
            open: false,
            state: State::Idle,
            text: Vec::new(),
            pos: 0,
            x: TEXT_X,
            y: TEXT_Y,
            delay: 0,
            colors: Colors::default(),
            msg: Canvas::new(MSG_WINDOW),
            yes_no: Canvas::new(YES_NO_WINDOW),
            yes_no_cursor: None,
            yes_no_result: None,
        }
    }

    fn screen_block() -> u8 {
        layout::SCREEN_BLOCKS[layout::TEXT_BG as usize]
    }

    fn enable_bg(&mut self, w: &'static WindowGfx, f: &mut Frame) {
        if self.bg_on {
            return;
        }
        static BLANK: [Tile; 1] = [[0; 8]];
        f.cmds.push(Cmd::BgPalette {
            bank: layout::WINDOW_PALETTE,
            colors: *w.palette,
        });
        f.cmds.push(Cmd::BgTiles {
            char_block: layout::TEXT_CHAR_BLOCK,
            first: 0,
            tiles: &BLANK,
        });
        f.cmds.push(Cmd::BgTiles {
            char_block: layout::TEXT_CHAR_BLOCK,
            first: FRAME_BASE,
            tiles: &w.tiles[..w.tiles.len().min(MAX_FRAME_TILES)],
        });
        f.cmds.push(Cmd::MapFill {
            screen_block: Self::screen_block(),
            entry: 0,
        });
        f.cmds.push(Cmd::Bg {
            layer: layout::TEXT_BG,
            control: BgControl {
                enabled: true,
                priority: layout::BG_PRIORITY[layout::TEXT_BG as usize],
                char_block: layout::TEXT_CHAR_BLOCK,
                screen_block: Self::screen_block(),
            },
        });
        self.bg_on = true;
    }

    fn put_frame(win: &Window, f: &mut Frame) {
        f.cmds.push(Cmd::MapEntries {
            screen_block: Self::screen_block(),
            start: win.frame_start(),
            entries: win.frame_entries(),
        });
    }

    /// Enables BG0 and draws an empty dialogue window.
    pub fn open(&mut self, game: &impl GameData, f: &mut Frame) {
        let w = game.window();
        self.colors = Colors::of(w);
        self.enable_bg(w, f);
        Self::put_frame(&MSG_WINDOW, f);
        self.msg.fill(self.colors.bg);
        self.msg.flush(f);
        self.open = true;
        self.state = State::Idle;
    }

    /// Clears the window and starts printing `text` (Gen3-encoded, 0xFF-terminated). Opens the
    /// window first if needed. The first character appears on the next `update`.
    pub fn print(&mut self, game: &impl GameData, text: &'static [u8], f: &mut Frame) {
        if !self.open {
            self.open(game, f);
        }
        self.text = expand(game, text);
        self.pos = 0;
        self.x = TEXT_X;
        self.y = TEXT_Y;
        self.delay = 0;
        self.colors = Colors::of(game.window());
        self.msg.fill(self.colors.bg);
        self.msg.flush(f);
        self.state = State::Printing;
    }

    /// Advances one frame. `pressed` = buttons newly pressed this frame, `held` = all held.
    pub fn update(
        &mut self,
        game: &impl GameData,
        pressed: Buttons,
        _held: Buttons,
        f: &mut Frame,
    ) {
        if self.yes_no_cursor.is_some() {
            self.update_yes_no(game, pressed, f);
        } else if self.open {
            self.update_printer(game.font(), pressed);
        }
        self.msg.flush(f);
        self.yes_no.flush(f);
    }

    fn update_printer(&mut self, font: &Font, pressed: Buttons) {
        let press = ab(pressed);
        match self.state {
            State::Idle => {}
            State::Printing => {
                if press {
                    // A/B finishes the page at once.
                    while self.state == State::Printing {
                        self.delay = 0;
                        self.step(font);
                    }
                } else if self.delay > 0 {
                    self.delay -= 1;
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
                self.x = TEXT_X;
                self.state = State::Scrolling(LINE_HEIGHT as u8);
            }
            State::Scrolling(0) => self.state = State::Printing,
            State::Scrolling(n) => {
                let s = n.min(SCROLL_SPEED);
                self.msg.scroll_up(s as i16, self.colors.bg);
                self.state = State::Scrolling(n - s);
            }
            State::WaitClear if press => {
                self.msg.fill(self.colors.bg);
                self.x = TEXT_X;
                self.y = TEXT_Y;
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

    /// Consumes codes until a glyph is drawn or the state changes (Gen3 `RenderText`).
    fn step(&mut self, font: &Font) {
        while self.state == State::Printing {
            let b = self.next();
            let glyph = match b {
                EOS => {
                    self.state = State::Idle;
                    return;
                }
                NEWLINE => {
                    self.x = TEXT_X;
                    self.y += LINE_HEIGHT;
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
            self.x += self
                .msg
                .draw_glyph(font, glyph, self.x, self.y, self.colors);
            self.delay = FRAMES_PER_CHAR - 1;
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
        match code {
            EXT_COLOR => self.colors.fg = a0,
            EXT_HIGHLIGHT => self.colors.bg = a0,
            EXT_SHADOW => self.colors.shadow = a0,
            EXT_COLOR_HIGHLIGHT_SHADOW => {
                self.colors = Colors {
                    fg: a0,
                    bg: a1,
                    shadow: a2,
                }
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

    /// Shows the yes/no menu (Gen3 `DisplayYesNoMenuDefaultYes`), cursor on yes.
    pub fn start_yes_no(&mut self, game: &impl GameData, f: &mut Frame) {
        let w = game.window();
        let colors = Colors::of(w);
        self.enable_bg(w, f);
        Self::put_frame(&YES_NO_WINDOW, f);
        let font = game.font();
        self.yes_no.fill(colors.bg);
        self.yes_no.draw_line(
            font,
            game.ui_text(UiText::Yes),
            YES_NO_TEXT_X,
            TEXT_Y,
            colors,
        );
        self.yes_no.draw_line(
            font,
            game.ui_text(UiText::No),
            YES_NO_TEXT_X,
            TEXT_Y + LINE_HEIGHT,
            colors,
        );
        self.yes_no_cursor = Some(0);
        self.yes_no_result = None;
        self.draw_cursor(font, colors, 0);
        self.yes_no.flush(f);
    }

    fn draw_cursor(&mut self, font: &Font, colors: Colors, pos: u8) {
        let y = TEXT_Y + pos as i16 * LINE_HEIGHT;
        self.yes_no.draw_glyph(font, CHAR_CURSOR, 0, y, colors);
    }

    fn update_yes_no(&mut self, game: &impl GameData, pressed: Buttons, f: &mut Frame) {
        let Some(cur) = self.yes_no_cursor else {
            return;
        };
        let result = if pressed.contains(Buttons::A) {
            Some(cur == 0)
        } else if pressed.contains(Buttons::B) {
            Some(false)
        } else {
            let to = if pressed.contains(Buttons::UP) {
                0
            } else if pressed.contains(Buttons::DOWN) {
                1
            } else {
                cur
            };
            if to != cur {
                let colors = Colors::of(game.window());
                let y = TEXT_Y + cur as i16 * LINE_HEIGHT;
                self.yes_no.fill_rect(0, y, CURSOR_W, CURSOR_H, colors.bg);
                self.draw_cursor(game.font(), colors, to);
                self.yes_no_cursor = Some(to);
            }
            None
        };
        if result.is_some() {
            // Gen3 `EraseYesNoWindow`.
            self.yes_no_result = result;
            self.yes_no_cursor = None;
            let win = YES_NO_WINDOW;
            f.cmds.push(Cmd::MapEntries {
                screen_block: Self::screen_block(),
                start: win.frame_start(),
                entries: vec![0; (win.height as usize + 2) * MAP_SIZE],
            });
        }
    }

    /// True while the yes/no menu waits for a choice.
    pub fn is_yes_no_active(&self) -> bool {
        self.yes_no_cursor.is_some()
    }

    /// The last yes/no choice (`Some(true)` = yes), until the next `start_yes_no` or `close`.
    pub fn yes_no_result(&self) -> Option<bool> {
        self.yes_no_result
    }

    /// Clears BG0's map, disables it and forgets any message or menu.
    pub fn close(&mut self, f: &mut Frame) {
        f.cmds.push(Cmd::MapFill {
            screen_block: Self::screen_block(),
            entry: 0,
        });
        f.cmds.push(Cmd::Bg {
            layer: layout::TEXT_BG,
            control: BgControl {
                enabled: false,
                priority: layout::BG_PRIORITY[layout::TEXT_BG as usize],
                char_block: layout::TEXT_CHAR_BLOCK,
                screen_block: Self::screen_block(),
            },
        });
        *self = Self::new();
    }

    /// True while the dialogue window is shown.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Feeds all state into `h` (parity contract).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.bg_on as u8,
            self.open as u8,
            self.delay,
            self.colors.fg,
            self.colors.shadow,
            self.colors.bg,
        ]);
        h.write(&self.state.tag());
        h.write_u32(self.pos as u32);
        h.write_u16(self.x as u16);
        h.write_u16(self.y as u16);
        h.write_u32(self.text.len() as u32);
        h.write(&self.text);
        h.write(&[
            self.yes_no_cursor.map_or(0xFF, |c| c),
            self.yes_no_result.map_or(0xFF, |r| r as u8),
        ]);
        self.msg.hash(h);
        self.yes_no.hash(h);
    }
}

#[cfg(test)]
mod tests;
