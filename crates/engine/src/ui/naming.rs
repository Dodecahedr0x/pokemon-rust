//! The naming screen (Gen3 `naming_screen.c`).
//!
//! The upper-case page opens under the cursor (`MainState_FadeIn` forces `KBPAGE_LETTERS_UPPER`).
//! A types the key, B deletes, SELECT cycles the page (upper, lower, symbols), START jumps to
//! OK. OK stores the text when it has a character other than space (`SaveInputText`); a blank
//! entry keeps the name that was already there. A full entry moves the cursor to OK. The
//! button column is page, backspace, OK (`sButtonKeyRoles`). Lengths are Gen3's:
//! `PLAYER_NAME_LENGTH` 7 (the player and the rival), `POKEMON_NAME_LENGTH` 10, `BOX_NAME_LENGTH` 8.
//!
//! Deviations: no icon, gender mark, or page-swap animation, and the d-pad does not repeat.
//! One window shows the text entered so far. `{RIVAL}` uses the stored name once it is set.

use super::kit::Gfx;
use super::kit::widgets::{self, EOS, push};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{NamingKind, Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::checkpoint::PLAYER_NAME_SIZE;
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::NAME_LEN;
use crate::pc::BOX_NAME_SIZE;
use alloc::vec::Vec;

/// The entry window: left, top, width, height (tiles).
pub const WINDOW: (u8, u8, u8, u8) = (1, 1, 16, 2);

/// Gen3 `KBPAGE_*`, in the order SELECT cycles (`sPageToNextKeyboardId`).
const PAGE_SYMBOLS: u8 = 0;
const PAGE_UPPER: u8 = 1;
const PAGE_LOWER: u8 = 2;

const ROWS: i8 = 4;
const LETTER_COLS: i8 = 8;
const SYMBOL_COLS: i8 = 6;
const BUTTONS: i8 = 3;

/// `sKeyboardChars`, charmap bytes. Letter rows are `KBCOL_COUNT` (8). The symbols page uses
/// the first 6 (`sPageColumnCounts`). Column 8 of a letter page, and column 6 of the symbols
/// page, is the button column, not a character.
const UPPER: [[u8; 8]; 4] = [
    [0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0x00, 0xAD], // ABCDEF .
    [0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0x00, 0xB8], // GHIJKL ,
    [0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0x00], // MNOPQRS
    [0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0x00], // TUVWXYZ
];
const LOWER: [[u8; 8]; 4] = [
    [0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0x00, 0xAD],
    [0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0, 0x00, 0xB8],
    [0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0x00],
    [0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0x00],
];
const SYMBOLS: [[u8; 6]; 4] = [
    [0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x00], // 01234
    [0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0x00], // 56789
    [0xAB, 0xAC, 0xB5, 0xB6, 0xBA, 0xAE], // !?♂♀/-
    [0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0x00], // …“”‘'
];

/// How many characters this screen accepts.
pub fn max_chars(kind: NamingKind) -> usize {
    match kind {
        NamingKind::Player | NamingKind::Rival => PLAYER_NAME_SIZE - 1,
        NamingKind::Mon(_) => NAME_LEN - 1,
        NamingKind::Box(_) => BOX_NAME_SIZE - 1,
    }
}

fn cols(page: u8) -> i8 {
    if page == PAGE_SYMBOLS {
        SYMBOL_COLS
    } else {
        LETTER_COLS
    }
}

/// The character at (`x`, `y`) on `page`, or `None` on the button column.
pub fn key(page: u8, x: i8, y: i8) -> Option<u8> {
    if !(0..ROWS).contains(&y) || x < 0 || x >= cols(page) {
        return None;
    }
    let (x, y) = (x as usize, y as usize);
    Some(match page {
        PAGE_UPPER => UPPER[y][x],
        PAGE_LOWER => LOWER[y][x],
        _ => SYMBOLS[y][x],
    })
}

/// `sKeyRowToButtonRow`: which button a key row lands on.
fn key_row_to_button(row: i8) -> i8 {
    match row {
        0 => 0,
        1 | 2 => 1,
        _ => 2,
    }
}

/// `sButtonRowToKeyRow`: which key row a button returns to.
fn button_row_to_key(row: i8) -> i8 {
    match row {
        0 | 1 => 0,
        _ => 3,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Char,
    Page,
    Back,
    Ok,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamingScreen {
    pub kind: NamingKind,
    page: u8,
    x: i8,
    y: i8,
    /// Key row to restore when leaving the middle button (`tButtonId`).
    button_row: i8,
    /// Gen3 `textBuffer`: 0xFF-terminated, 0xFF-padded.
    text: [u8; 16],
    gfx: Gfx<Panel>,
}

impl NamingScreen {
    pub fn new(kind: NamingKind) -> Self {
        NamingScreen {
            kind,
            page: PAGE_UPPER,
            x: 0,
            y: 0,
            button_row: 0,
            text: [EOS; 16],
            gfx: Gfx::none(),
        }
    }

    fn role(&self) -> Role {
        if self.x < cols(self.page) {
            Role::Char
        } else {
            match self.y {
                0 => Role::Page,
                1 => Role::Back,
                _ => Role::Ok,
            }
        }
    }

    fn to_ok(&mut self) {
        self.x = cols(self.page);
        self.y = 2;
    }

    fn swap_page(&mut self) {
        let old = cols(self.page);
        let on_button = self.x == old;
        self.page = (self.page + 1) % 3;
        let new = cols(self.page);
        if on_button {
            self.x = new;
        } else if self.x >= new {
            self.x = new - 1;
        }
    }

    /// `HandleDpadMovement`, one new press (this engine's menus do not repeat).
    fn move_cursor(&mut self, pressed: Buttons) {
        let n = cols(self.page);
        let prev_x = self.x;
        if pressed.contains(Buttons::UP) {
            self.y -= 1;
        } else if pressed.contains(Buttons::DOWN) {
            self.y += 1;
        } else if pressed.contains(Buttons::LEFT) {
            self.x -= 1;
        } else if pressed.contains(Buttons::RIGHT) {
            self.x += 1;
        } else {
            return;
        }
        if self.x < 0 {
            self.x = n;
        } else if self.x > n {
            self.x = 0;
        }
        if self.x != prev_x {
            if self.x == n {
                self.button_row = self.y;
                self.y = key_row_to_button(self.y);
            } else if prev_x == n {
                self.y = if self.y == BUTTONS / 2 {
                    self.button_row
                } else {
                    button_row_to_key(self.y)
                };
            }
        }
        if self.x == n {
            if self.y < 0 {
                self.y = BUTTONS - 1;
            } else if self.y >= BUTTONS {
                self.y = 0;
            }
            if self.y == 0 {
                self.button_row = 1;
            } else if self.y == BUTTONS - 1 {
                self.button_row = 2;
            }
        } else if self.y < 0 {
            self.y = ROWS - 1;
        } else if self.y >= ROWS {
            self.y = 0;
        }
    }

    fn caret(&self) -> usize {
        let max = max_chars(self.kind);
        self.text[..max]
            .iter()
            .position(|&c| c == EOS)
            .unwrap_or(max - 1)
    }

    fn prev_caret(&self) -> usize {
        let max = max_chars(self.kind);
        for i in (1..max).rev() {
            if self.text[i] != EOS {
                return i;
            }
        }
        0
    }

    /// `AddTextCharacter`. True when the entry is now full (`maxChars - 1` is filled).
    fn add_char(&mut self, ch: u8) -> bool {
        let max = max_chars(self.kind);
        let i = self.caret();
        self.text[i] = ch;
        self.prev_caret() == max - 1
    }

    fn delete(&mut self) {
        let i = self.prev_caret();
        self.text[i] = EOS;
    }

    /// `SaveInputText`: nothing when every character is space or the terminator.
    fn saved(&self) -> Option<&[u8]> {
        let n = max_chars(self.kind);
        self.text[..n]
            .iter()
            .any(|&c| c != widgets::CHAR_SPACE && c != EOS)
            .then_some(&self.text[..n])
    }

    fn commit<G: GameData>(&self, cx: &mut ScreenCx<'_, G>) {
        let Some(raw) = self.saved() else {
            return;
        };
        match self.kind {
            NamingKind::Player => *cx.player_name = fit::<PLAYER_NAME_SIZE>(raw),
            NamingKind::Rival => *cx.rival_name = Some(fit::<PLAYER_NAME_SIZE>(raw)),
            NamingKind::Mon(slot) => {
                if let Some(mon) = cx.party.get_mut(usize::from(slot)) {
                    mon.nickname = fit::<NAME_LEN>(raw);
                }
            }
            NamingKind::Box(b) => cx.pc.set_box_name(usize::from(b), raw),
        }
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let game = cx.game;
        let p = self.gfx.0.get_or_insert_with(|| {
            let (l, t, w, h) = WINDOW;
            let p = TileAlloc::full().panel(game, l, t, w, h, true);
            let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
            map.put(&p);
            map.flush(cx.f);
            alloc::boxed::Box::new(p)
        });
        p.clear(game);
        let mut name = Vec::new();
        push(&mut name, &self.text);
        name.push(EOS);
        p.text(game, &name, 0, 1);
        p.flush(cx.f);
    }
}

/// Copies `raw` up to its terminator into an `N`-byte name (at most `N - 1` characters).
fn fit<const N: usize>(raw: &[u8]) -> [u8; N] {
    let mut out = [EOS; N];
    for (o, &c) in out[..N - 1]
        .iter_mut()
        .zip(raw.iter().take_while(|&&c| c != EOS))
    {
        *o = c;
    }
    out
}

impl Screen for NamingScreen {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let pressed = input.pressed;
        let first = self.gfx.0.is_none();
        if pressed.contains(Buttons::SELECT) {
            self.swap_page();
        } else if pressed.contains(Buttons::B) {
            self.delete();
        } else if pressed.contains(Buttons::START) {
            self.to_ok();
        } else if pressed.contains(Buttons::A) {
            match self.role() {
                Role::Char => {
                    if let Some(ch) = key(self.page, self.x, self.y)
                        && self.add_char(ch)
                    {
                        self.to_ok();
                    }
                }
                Role::Page => self.swap_page(),
                Role::Back => self.delete(),
                Role::Ok => {
                    self.commit(cx);
                    return ScreenOut::Done(ScreenResult::Bool(true));
                }
            }
        } else if pressed.0 & (Buttons::UP.0 | Buttons::DOWN.0 | Buttons::LEFT.0 | Buttons::RIGHT.0)
            != 0
        {
            self.move_cursor(pressed);
        }
        if first || pressed != Buttons::NONE {
            self.draw(cx);
        }
        ScreenOut::Running
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.page, self.x as u8, self.y as u8, self.button_row as u8]);
        h.write(&self.text);
    }
}

#[cfg(test)]
#[path = "naming_tests.rs"]
mod tests;
