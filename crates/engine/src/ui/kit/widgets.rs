//! Widgets every screen shares: the message in a screen (the field printer on a screen
//! window), the yes/no menu, the menu cursor glyph, numbers, the money box and the quantity
//! selector.

use super::list::{MenuCursor, MenuInput};
use super::window::{Panel, TileAlloc, colors};
use crate::data::{GameData, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::options::TextSpeed;
use crate::str_vars::StrVars;
use crate::text::{self, Colors, Printer, Window};
use crate::video::Frame;
use alloc::vec::Vec;

/// Gen3 charmap: `'0'`, `' '`, `'/'`, `'▶'`, `{LV_2}`'s extra symbol.
pub const CHAR_0: u8 = 0xA1;
pub const CHAR_SPACE: u8 = 0x00;
pub const CHAR_SLASH: u8 = 0xBA;
pub const CHAR_CURSOR: u8 = 0xEF;
pub const CHAR_MALE: u8 = 0xB5;
pub const CHAR_FEMALE: u8 = 0xB6;
pub const CHAR_CURRENCY: u8 = 0xB7;
pub const CHAR_TIMES: u8 = 0xB9;
pub const EXTRA_SYMBOL: u8 = 0xF9;
pub const SYMBOL_LV: u8 = 0x05;
pub const EOS: u8 = 0xFF;
/// `GetMenuCursorDimensionByFont(FONT_NORMAL, 0)`: entries start this far right of the cursor.
pub const CURSOR_W: i16 = 8;
/// One menu row (`FONT_NORMAL` height plus spacing).
pub const ROW_H: i16 = 16;

/// Appends `s` up to its terminator.
pub fn push(out: &mut Vec<u8>, s: &[u8]) {
    out.extend(s.iter().take_while(|&&b| b != EOS));
}

/// Gen3 `ConvertIntToDecimalStringN`: `digits` wide, right-aligned with spaces
/// (`STR_CONV_MODE_RIGHT_ALIGN`) or left-aligned without padding (`STR_CONV_MODE_LEFT_ALIGN`).
pub fn push_number(out: &mut Vec<u8>, n: u32, digits: u8, right_align: bool) {
    let mut buf = [0u8; 10];
    let mut v = n;
    let mut len = 0;
    loop {
        buf[len] = CHAR_0 + (v % 10) as u8;
        v /= 10;
        len += 1;
        if v == 0 || len == buf.len() {
            break;
        }
    }
    let digits = digits.max(1) as usize;
    let len = len.min(digits);
    if right_align {
        out.extend(core::iter::repeat_n(CHAR_SPACE, digits - len));
    }
    out.extend(buf[..len].iter().rev());
}

/// A menu's labels drawn one per row after the cursor column, with `▶` on the cursor's row
/// (Gen3 `PrintMenuTable` + `InitMenuInUpperLeftCorner`).
pub fn draw_menu(game: &impl GameData, p: &mut Panel, labels: &[&[u8]], cursor: u8) {
    p.clear(game);
    for (i, l) in labels.iter().enumerate() {
        p.text(game, l, CURSOR_W, i as i16 * ROW_H + 1);
    }
    draw_cursor(game, p, labels.len() as u8, cursor);
}

/// Redraws the cursor column only (`Menu_MoveCursor` erases and redraws `▶`).
pub fn draw_cursor(game: &impl GameData, p: &mut Panel, rows: u8, cursor: u8) {
    let c = colors(game);
    for i in 0..rows {
        let y = i as i16 * ROW_H + 1;
        p.canvas.fill_rect(0, y, CURSOR_W, ROW_H - 1, c.bg);
        if i == cursor {
            p.text(game, &[CHAR_CURSOR, EOS], 0, y);
        }
    }
}

/// A message printed in a screen (Gen3 `DisplayPartyMenuMessage` and friends): the field's
/// timed printer on a screen window, with the text speed option, then (if asked) a wait for
/// A or B.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Message {
    pub panel_window: Window,
    printer: Printer,
    /// Waiting for A/B once printed (`keepOpen`'s `IsPartyMenuTextPrinterActive` + press).
    wait_press: bool,
    pub shown: bool,
}

impl Message {
    pub fn new(win: Window, speed: TextSpeed) -> Self {
        let mut printer = Printer::new(
            win,
            super::window::CHAR_BLOCK,
            (0, text::TEXT_Y),
            text::FRAMES_PER_CHAR,
        );
        printer.set_frames_per_char(speed.frames_per_char());
        Message {
            panel_window: win,
            printer,
            wait_press: false,
            shown: false,
        }
    }

    /// Starts printing `text` (placeholders expanded with `vars`); `wait_press`: the message
    /// counts as done only after A or B once printed.
    pub fn start(
        &mut self,
        game: &impl GameData,
        text: &[u8],
        player: &[u8],
        vars: Option<&StrVars>,
        wait_press: bool,
    ) {
        let colors = colors(game);
        self.printer
            .start(text::expand_vars(game, text, player, vars), colors, true);
        self.wait_press = wait_press;
        self.shown = true;
    }

    /// One frame; whether the message is done (printed, and pressed if it waits).
    pub fn update(&mut self, game: &impl GameData, pressed: Buttons, held: Buttons) -> bool {
        if self.printer.is_printing() {
            self.printer.update(game.font(), pressed, held);
            return false;
        }
        if self.wait_press {
            if pressed.contains(Buttons::A) || pressed.contains(Buttons::B) {
                self.wait_press = false;
                return true;
            }
            return false;
        }
        true
    }

    pub fn is_done(&self) -> bool {
        !self.printer.is_printing() && !self.wait_press
    }

    pub fn flush_within(&mut self, f: &mut Frame, budget: &mut usize) {
        self.printer.flush_within(f, budget);
    }

    pub fn flush(&mut self, f: &mut Frame) {
        self.printer.flush(f);
    }

    /// Uploads the whole canvas on the next flush (after another screen used the tiles).
    pub fn touch(&mut self) {
        self.printer.touch();
    }

    /// Hides the message (its window leaves the map; the printer keeps its text).
    pub fn hide(&mut self) {
        self.shown = false;
        self.wait_press = false;
    }

    pub fn window(&self) -> Window {
        self.panel_window
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        self.printer.hash(h);
        h.write(&[self.wait_press as u8, self.shown as u8]);
    }
}

/// The yes/no menu in a screen (Gen3 `PartyMenuDisplayYesNoMenu`, `CreateYesNoMenu` at
/// `sPartyMenuYesNoWindowTemplate`: (21, 9), 5 x 4 tiles), `Menu_ProcessInputNoWrapClearOnChoose`:
/// no wrap, B answers no.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct YesNo {
    pub panel: Panel,
    pub cursor: u8,
}

/// Gen3 `sPartyMenuYesNoWindowTemplate` / `sYesNo_WindowTemplates`.
pub const YES_NO_RECT: (u8, u8, u8, u8) = (21, 9, 5, 4);

impl YesNo {
    pub fn new(game: &impl GameData, tiles: &mut TileAlloc, default_no: bool) -> Self {
        let (l, t, w, h) = YES_NO_RECT;
        let mut y = YesNo {
            panel: tiles.panel(game, l, t, w, h, true),
            cursor: default_no as u8,
        };
        y.draw(game);
        y
    }

    fn draw(&mut self, game: &impl GameData) {
        let labels = [game.ui_text(UiText::Yes), game.ui_text(UiText::No)];
        draw_menu(game, &mut self.panel, &labels, self.cursor);
    }

    /// `Some(true)` yes, `Some(false)` no or B.
    pub fn input(&mut self, game: &impl GameData, pressed: Buttons) -> Option<bool> {
        if pressed.contains(Buttons::A) {
            return Some(self.cursor == 0);
        }
        if pressed.contains(Buttons::B) {
            return Some(false);
        }
        let to = if pressed.contains(Buttons::UP) {
            0
        } else if pressed.contains(Buttons::DOWN) {
            1
        } else {
            return None;
        };
        if to != self.cursor {
            self.cursor = to;
            draw_cursor(game, &mut self.panel, 2, self.cursor);
        }
        None
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.cursor]);
    }
}

/// The money box (Gen3 `DrawMoneyBox(amount, x, y)`: a framed 10 x 2 window at (x + 1, y + 1);
/// `PrintMoneyAmountInMoneyBox`: the amount left-aligned on 6 digits padded with spaces in
/// front, then the currency). Deviation: the currency sign follows the amount as `¥`
/// (Gen3 prints `gText_PokedollarVar1`, a language text not extracted).
pub fn money_box(game: &impl GameData, tiles: &mut TileAlloc, money: u32, x: u8, y: u8) -> Panel {
    let mut p = tiles.panel(game, x + 1, y + 1, 10, 2, true);
    let mut s = Vec::with_capacity(10);
    push_number(&mut s, money, 6, true);
    s.push(CHAR_CURRENCY);
    s.push(EOS);
    p.text(game, &s, 0, 1);
    p
}

/// The coin box (Gen3 `ShowCoinsWindow(coins, x, y)`: a framed 8 x 2 window at (x, y),
/// `PrintCoinsString`'s 4 digits).
pub fn coin_box(game: &impl GameData, tiles: &mut TileAlloc, coins: u16, x: u8, y: u8) -> Panel {
    let mut p = tiles.panel(game, x, y, 8, 2, true);
    let mut s = Vec::with_capacity(8);
    push_number(&mut s, coins as u32, 4, true);
    s.push(EOS);
    p.text(game, &s, 0, 1);
    p
}

/// Gen3 `AdjustQuantityAccordingToDPadInput` (`menu_helpers.c`): up/down by 1 and left/right
/// by 10, between 1 and `max`, up past `max` wrapping to 1 and down past 1 wrapping to `max`
/// (left/right clamp instead).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Quantity {
    pub value: u16,
    pub max: u16,
}

impl Quantity {
    pub const fn new(max: u16) -> Self {
        Quantity {
            value: 1,
            max: if max == 0 { 1 } else { max },
        }
    }

    /// Whether the value changed.
    pub fn input(&mut self, pressed: Buttons) -> bool {
        let old = self.value;
        if pressed.contains(Buttons::UP) {
            self.value = if self.value >= self.max {
                1
            } else {
                self.value + 1
            };
        } else if pressed.contains(Buttons::DOWN) {
            self.value = if self.value <= 1 {
                self.max
            } else {
                self.value - 1
            };
        } else if pressed.contains(Buttons::RIGHT) {
            self.value = self.value.saturating_add(10).min(self.max);
        } else if pressed.contains(Buttons::LEFT) {
            self.value = self.value.saturating_sub(10).max(1);
        }
        self.value != old
    }

    /// `PrintItemQuantity`: `×` and the value, `digits` wide.
    pub fn text(&self, digits: u8) -> Vec<u8> {
        let mut s = Vec::with_capacity(digits as usize + 2);
        s.push(CHAR_TIMES);
        push_number(&mut s, self.value as u32, digits, false);
        s.push(EOS);
        s
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.value);
        h.write_u16(self.max);
    }
}

/// The max a quantity selector allows: what the bag holds, the money buys, or the bag can
/// take, at most 99 (Gen3 `MAX_BAG_ITEM_CAPACITY`).
pub fn quantity_max(have: u32, price: u32, money: u32, cap: u32) -> u16 {
    let afford = money.checked_div(price).unwrap_or(cap);
    have.min(afford).min(cap).min(99) as u16
}

/// A cursor over a [`MenuCursor`] drawn in a panel: one call per frame.
pub fn menu_step(
    game: &impl GameData,
    p: &mut Panel,
    cursor: &mut MenuCursor,
    pressed: Buttons,
) -> MenuInput {
    let r = cursor.input(pressed);
    if r == MenuInput::Moved {
        draw_cursor(game, p, cursor.count, cursor.pos);
    }
    r
}

/// The colors for a greyed entry (Gen3 `sFontColorTable[1]`-like: the shadow color as text).
pub fn grey(game: &impl GameData) -> Colors {
    let c = colors(game);
    Colors::new(c.shadow, c.bg, c.bg)
}

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod tests;
