//! Dialogue text box on BG0: a text printer for Gen3-encoded text (variable-width font, `\n`,
//! `\l`, `\p`, placeholders, `{…}` control codes), the dialogue frame (laid out as Gen3
//! `DrawDialogueFrame`) and the yes/no menu.
//!
//! Rendering follows Gen3 (glyph shape, colors, positions, line scroll); timing is simplified:
//! each drawn glyph costs `FRAMES_PER_CHAR` frames and control codes cost nothing, whereas Gen3
//! "medium" spends about 5 frames per glyph and also charges for control codes. A/B speeds
//! printing up as Gen3 `RenderText` does: a new press skips the current delay, and while A or B
//! stays held after that, one glyph is drawn per frame (never more, which bounds the frame's
//! cost on the GBA).
//!
//! VRAM use (char block `layout::TEXT_CHAR_BLOCK`, screen block `SCREEN_BLOCKS[TEXT_BG]`):
//! - tile 0: blank (cleared map entries show nothing),
//! - tiles `FRAME_BASE..`: `WindowGfx::tiles` (Gen3 loads them at `DLG_WINDOW_BASE_TILE_NUM`),
//! - tiles `MSG_WINDOW.base..`: the message canvas, 27x4 tiles, row-major,
//! - tiles `YES_NO_WINDOW.base..`: the yes/no canvas, 5x4 tiles, row-major.
//!
//! Canvases are kept in RAM; every call that changes them ends by uploading the changed tiles
//! with `Cmd::BgTile`. BG0 scroll is expected to stay (0, 0).
//!
//! The glyph canvas and the printer state machine live in [`printer`], shared with the battle
//! windows (`battle::ui`).

mod printer;

#[cfg(test)]
use self::printer::State;
pub(crate) use self::printer::px_mask;
pub use self::printer::{Canvas, Colors, Pen, Printer};

use crate::Tile;
use crate::data::{Font, GameData, UiText, WindowGfx};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::menu::{Choice, Multichoice};
use crate::str_vars::StrVars;
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, layout};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

/// A text window: its tilemap rectangle (in tiles) and its first canvas tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Window {
    left: u8,
    top: u8,
    width: u8,
    height: u8,
    /// First canvas tile in `TEXT_CHAR_BLOCK`.
    base: u16,
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
/// Frames per drawn glyph (our fixed speed; close to, not equal to, Gen3 "medium").
pub const FRAMES_PER_CHAR: u8 = 4;
/// Gen3 `sWindowVerticalScrollSpeeds[OPTIONS_TEXT_SPEED_MID]`, pixels per frame.
pub const SCROLL_SPEED: u8 = 2;
/// `maxLetterHeight + lineSpacing` of `FONT_NORMAL`.
pub const LINE_HEIGHT: i16 = 16;
/// Rows drawn per glyph (`gCurGlyph.height` of `FONT_NORMAL`).
const GLYPH_HEIGHT: i16 = 15;
/// Printer origin inside a window (`AddTextPrinterParameterized2`: x 0, y 1).
const TEXT_X: i16 = 0;
pub(crate) const TEXT_Y: i16 = 1;
/// Yes/no layout (`CreateYesNoMenu`, `InitMenuInUpperLeftCorner`, `sMenuCursorDimensions`).
pub(crate) const YES_NO_TEXT_X: i16 = 8;
pub(crate) const CURSOR_W: i16 = 8;
pub(crate) const CURSOR_H: i16 = 15;
/// `▶` (`gText_SelectorArrow3`).
pub(crate) const CHAR_CURSOR: u16 = 0xEF;

pub(crate) const EOS: u8 = 0xFF;
pub(crate) const NEWLINE: u8 = 0xFE;
const PLACEHOLDER: u8 = 0xFD;
const EXT_CTRL: u8 = 0xFC;
const PROMPT_CLEAR: u8 = 0xFB;
const PROMPT_SCROLL: u8 = 0xFA;
pub(crate) const EXTRA_SYMBOL: u8 = 0xF9;
const KEYPAD_ICON: u8 = 0xF8;
const DYNAMIC: u8 = 0xF7;

const PLACEHOLDER_PLAYER: u8 = 0x01;
/// `{STR_VAR_1}`; `{STR_VAR_2}` and `{STR_VAR_3}` follow.
const PLACEHOLDER_STR_VAR_1: u8 = 0x02;
const PLACEHOLDER_STR_VAR_3: u8 = 0x04;
const PLACEHOLDER_RIVAL: u8 = 0x06;
/// Gen3 `gStringVar4`'s size: an expansion keeps at most this many bytes, terminator included.
pub const EXPANDED_MAX: usize = 1000;
/// Placeholders inside expanded placeholders are expanded this deep (Gen3 recurses without
/// bound, so a string var naming itself hangs it); deeper ones expand to nothing.
const EXPAND_DEPTH: u8 = 3;

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
pub(crate) fn unit_len(text: &[u8], i: usize) -> usize {
    match text.get(i).copied() {
        Some(EXT_CTRL) => match text.get(i + 1) {
            Some(&c) if (c as usize) < EXT_ARG_LEN.len() => 2 + EXT_ARG_LEN[c as usize] as usize,
            _ => 2,
        },
        Some(PLACEHOLDER | DYNAMIC | KEYPAD_ICON | EXTRA_SYMBOL) => 2,
        _ => 1,
    }
}

/// Copies `text` up to its 0xFF, expanding `{PLAYER}` to `player` (the World's name, up to its
/// 0xFF) and `{RIVAL}`; other placeholders, string vars included, expand to nothing. The result
/// always ends with 0xFF. See [`expand_vars`].
pub fn expand(game: &impl GameData, text: &[u8], player: &[u8]) -> Vec<u8> {
    expand_vars(game, text, player, None)
}

/// Gen3 `StringExpandPlaceholders`: copies `text` up to its 0xFF, expanding `{PLAYER}` to
/// `player`, `{RIVAL}`, and `{STR_VAR_1}`..`{STR_VAR_3}` to the string vars (when given), each
/// expansion itself expanded (up to `EXPAND_DEPTH` deep). Other placeholders expand to nothing
/// (`{KUN}` is empty in Emerald). The result keeps whole code units, at most [`EXPANDED_MAX`]
/// bytes, and always ends with 0xFF.
pub fn expand_vars(
    game: &impl GameData,
    text: &[u8],
    player: &[u8],
    vars: Option<&StrVars>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 8);
    expand_into(game, text, player, vars, 0, &mut out);
    out.push(EOS);
    out
}

fn expand_into(
    game: &impl GameData,
    text: &[u8],
    player: &[u8],
    vars: Option<&StrVars>,
    depth: u8,
    out: &mut Vec<u8>,
) {
    let mut i = 0;
    while i < text.len() && text[i] != EOS {
        let n = unit_len(text, i);
        if text[i] == PLACEHOLDER {
            let sub: &[u8] = match text.get(i + 1).copied() {
                Some(PLACEHOLDER_PLAYER) => player,
                Some(PLACEHOLDER_RIVAL) => game.ui_text(UiText::RivalName),
                Some(v @ PLACEHOLDER_STR_VAR_1..=PLACEHOLDER_STR_VAR_3) => {
                    vars.map_or(&[], |s| &s.get((v - PLACEHOLDER_STR_VAR_1) as usize)[..])
                }
                _ => &[],
            };
            if depth < EXPAND_DEPTH {
                expand_into(game, sub, player, vars, depth + 1, out);
            }
        } else {
            let unit = &text[i..(i + n).min(text.len())];
            if out.len() + unit.len() >= EXPANDED_MAX {
                return;
            }
            out.extend_from_slice(unit);
        }
        i += n;
    }
}

const fn frame_entry(tile: u16, vflip: bool) -> u16 {
    (FRAME_BASE + tile) | ((vflip as u16) << 11) | ((layout::WINDOW_PALETTE as u16) << 12)
}

impl Window {
    /// A window at tile (`left`, `top`) of `width` x `height` tiles whose canvas starts at tile
    /// `base` of its char block (Gen3 `struct WindowTemplate`).
    pub const fn new(left: u8, top: u8, width: u8, height: u8, base: u16) -> Self {
        Self {
            left,
            top,
            width,
            height,
            base,
        }
    }

    pub const fn left(&self) -> u8 {
        self.left
    }

    pub const fn top(&self) -> u8 {
        self.top
    }

    pub const fn width(&self) -> u8 {
        self.width
    }

    pub const fn height(&self) -> u8 {
        self.height
    }

    /// Screen-block entry index of the first row returned by [`Window::frame_entries`].
    pub const fn frame_start(&self) -> u16 {
        self.top.saturating_sub(1) as u16 * MAP_SIZE as u16
    }

    /// First canvas tile in `TEXT_CHAR_BLOCK`.
    pub const fn base(&self) -> u16 {
        self.base
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
                let col = self.left as usize + x;
                if col < MAP_SIZE {
                    e[(y + 1) * MAP_SIZE + col] = tile | pal;
                }
            }
        }
        e
    }
}

/// The field dialogue window, its text printer and the yes/no menu.
pub struct TextBox {
    /// BG0 enabled with the window palette and frame tiles loaded.
    bg_on: bool,
    /// Dialogue frame drawn.
    open: bool,
    /// The message window's printer.
    printer: Printer,
    yes_no: Canvas,
    /// Cursor position (0 = yes) while the yes/no menu is shown.
    yes_no_cursor: Option<u8>,
    yes_no_result: Option<bool>,
    /// The script multichoice while it is shown (Milestone 8).
    menu: Option<Box<Multichoice>>,
    /// The last multichoice answer, until the next `start_multichoice` or `close`.
    menu_result: Option<u16>,
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
            printer: Printer::new(
                MSG_WINDOW,
                layout::TEXT_CHAR_BLOCK,
                (TEXT_X, TEXT_Y),
                FRAMES_PER_CHAR,
            ),
            yes_no: Canvas::new(YES_NO_WINDOW, layout::TEXT_CHAR_BLOCK),
            yes_no_cursor: None,
            yes_no_result: None,
            menu: None,
            menu_result: None,
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
        self.open_unflushed(game, f);
        self.printer.flush(f);
    }

    /// [`TextBox::open`] without uploading the cleared canvas (the caller flushes it).
    fn open_unflushed(&mut self, game: &impl GameData, f: &mut Frame) {
        let w = game.window();
        self.printer.clear(Colors::of(w));
        self.enable_bg(w, f);
        Self::put_frame(&MSG_WINDOW, f);
        self.open = true;
    }

    /// Clears the window and starts printing `text` (Gen3-encoded, 0xFF-terminated). Opens the
    /// window first if needed. `{PLAYER}` prints `player`; string vars print nothing (see
    /// [`TextBox::print_vars`]). The first character appears on the next `update`.
    pub fn print(
        &mut self,
        game: &impl GameData,
        text: &'static [u8],
        player: &[u8],
        f: &mut Frame,
    ) {
        self.print_vars(game, text, player, None, f);
    }

    /// [`TextBox::print`], with `{STR_VAR_1}`..`{STR_VAR_3}` printing `vars` (Gen3
    /// `StringExpandPlaceholders` into `gStringVar4`, as `message` does).
    pub fn print_vars(
        &mut self,
        game: &impl GameData,
        text: &'static [u8],
        player: &[u8],
        vars: Option<&StrVars>,
        f: &mut Frame,
    ) {
        // Opening clears the canvas with the default colours already; it is flushed once below.
        let cleared = !self.open;
        if cleared {
            self.open_unflushed(game, f);
        }
        let colors = Colors::of(game.window());
        self.printer
            .start(expand_vars(game, text, player, vars), colors, !cleared);
        self.printer.flush(f);
    }

    /// Advances one frame. `pressed` = buttons newly pressed this frame, `held` = all held.
    pub fn update(&mut self, game: &impl GameData, pressed: Buttons, held: Buttons, f: &mut Frame) {
        if let Some(menu) = &mut self.menu {
            if let Choice::Done(answer) = menu.update(game, pressed, f) {
                self.menu = None;
                self.menu_result = Some(answer);
            }
        } else if self.yes_no_cursor.is_some() {
            self.update_yes_no(game, pressed, f);
        } else if self.open {
            self.printer.update(game.font(), pressed, held);
        }
        self.printer.flush(f);
        self.yes_no.flush(f);
    }

    /// True while a message is being printed, including its `\p`/`\l`/pause waits.
    pub fn is_printing(&self) -> bool {
        self.printer.is_printing()
    }

    /// The expanded text of the last message (tests only).
    #[cfg(test)]
    pub(crate) fn printed(&self) -> &[u8] {
        &self.printer.text
    }

    /// Shows the yes/no menu (Gen3 `DisplayYesNoMenuDefaultYes`), cursor on yes.
    pub fn start_yes_no(&mut self, game: &impl GameData, f: &mut Frame) {
        let w = game.window();
        let pen = Pen::new(Colors::of(w));
        self.enable_bg(w, f);
        Self::put_frame(&YES_NO_WINDOW, f);
        let font = game.font();
        self.yes_no.fill(pen.colors.bg);
        self.yes_no
            .draw_line(font, game.ui_text(UiText::Yes), YES_NO_TEXT_X, TEXT_Y, &pen);
        self.yes_no.draw_line(
            font,
            game.ui_text(UiText::No),
            YES_NO_TEXT_X,
            TEXT_Y + LINE_HEIGHT,
            &pen,
        );
        self.yes_no_cursor = Some(0);
        self.yes_no_result = None;
        self.draw_cursor(font, &pen, 0);
        self.yes_no.flush(f);
    }

    fn draw_cursor(&mut self, font: &Font, pen: &Pen, pos: u8) {
        let y = TEXT_Y + pos as i16 * LINE_HEIGHT;
        self.yes_no.draw_glyph(font, CHAR_CURSOR, 0, y, pen);
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
                let pen = Pen::new(Colors::of(game.window()));
                let y = TEXT_Y + cur as i16 * LINE_HEIGHT;
                self.yes_no
                    .fill_rect(0, y, CURSOR_W, CURSOR_H, pen.colors.bg);
                self.draw_cursor(game.font(), &pen, to);
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

    /// Shows the script multichoice `menu` (laid out by [`Multichoice::new`]). Enables BG0 if
    /// needed; the window is drawn by the following updates, the first one this frame.
    pub fn start_multichoice(&mut self, game: &impl GameData, menu: Multichoice, f: &mut Frame) {
        self.enable_bg(game.window(), f);
        self.menu = Some(Box::new(menu));
        self.menu_result = None;
    }

    /// True while a multichoice waits for a choice.
    pub fn is_multichoice_active(&self) -> bool {
        self.menu.is_some()
    }

    /// The open multichoice, if any.
    pub fn multichoice(&self) -> Option<&Multichoice> {
        self.menu.as_deref()
    }

    /// The last multichoice answer (an index or `MULTI_B_PRESSED`), until the next
    /// `start_multichoice` or `close`.
    pub fn multichoice_result(&self) -> Option<u16> {
        self.menu_result
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

    /// Same hash as `TextBox::new()` (what `close` leaves): nothing on BG0 needs restoring.
    pub fn is_pristine(&self) -> bool {
        let digest = |t: &TextBox| {
            let mut h = Fnv64::new();
            t.hash(&mut h);
            h.finish()
        };
        digest(self) == digest(&TextBox::new())
    }

    /// Feeds all state into `h` (parity contract).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.bg_on as u8, self.open as u8]);
        self.printer.hash(h);
        h.write(&[
            self.yes_no_cursor.map_or(0xFF, |c| c),
            self.yes_no_result.map_or(0xFF, |r| r as u8),
        ]);
        // Nothing while no multichoice was shown, so the hash of a world without one is the
        // same as before multichoices existed.
        if let Some(m) = &self.menu {
            h.write(&[1]);
            m.hash(h);
        }
        if let Some(r) = self.menu_result {
            h.write(&[2]);
            h.write_u16(r);
        }
        // The canvases are not hashed: they follow from the state above.
    }
}

#[cfg(test)]
mod tests;
