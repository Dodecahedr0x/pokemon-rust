//! The Easy Chat screen (Gen3 `easy_chat.c`: `ShowEasyChatScreen`, `HandleEasyChatInput`).
//!
//! - The phrase (`INPUTSTATE_PHRASE`): the cursor moves over its words and wraps into the
//!   button row (`HandleEasyChatInput_Phrase`). A opens the group keyboard. B asks to quit
//!   (`StartConfirmExitPrompt`). START tries to confirm (`TryConfirmWords`).
//! - The buttons (`INPUTSTATE_MAIN_SCREEN_BUTTONS`): DEL. ALL, CANCEL, OK.
//! - The keyboard (`INPUTSTATE_KEYBOARD`): the groups a new game lists (Trainer through
//!   Adjectives, `SetUnlockedEasyChatGroups`). A opens that group's words
//!   (`INPUTSTATE_WORD_SELECT`); B returns to the phrase.
//! - A word is `EC_WORD(group, index)` from [`words`], in Gen3's selection order. A places it
//!   in the phrase cell and returns to the phrase.
//! - Quit (`ShowConfirmExitPrompt`, yes/no on NO): yes closes with `Bool(false)`. The contest
//!   interview and the apprentice (`StartConfirmExitPrompt`) cannot quit: B and an empty
//!   confirm show "You cannot quit here" and wait for A or B.
//! - Confirm (`ShowConfirmPrompt`, yes/no on YES): yes closes with `Bool(true)` only when the
//!   phrase changed (`GetEasyChatCompleted`). An unchanged phrase asks to quit instead, except
//!   the questionnaire ([`ASKS_CONFIRM`]), which always asks to confirm, and an interview or
//!   apprentice phrase that is not empty.
//!
//! Deviations: no alphabet keyboard (the same words are on the group lists). Pokémon, moves,
//! Events and trendy sayings stay locked, as they do before the Pokédex, the Hall of Fame and
//! that flag. Localized word glyphs are not drawn (the phrase is the word ids). Quiz footers
//! have the three buttons, not a fourth.

use super::kit::Gfx;
use super::kit::widgets::{self, YesNo};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;

#[path = "easy_chat_words.rs"]
mod words;

/// The phrase window: left, top, width, height (tiles).
pub const WINDOW: (u8, u8, u8, u8) = (3, 3, 24, 8);
/// The profile frame (`FRAMEID_GENERAL_2x2`). Other types use their own frame.
pub const ROWS: i8 = 2;
pub const COLUMNS: i8 = 2;
/// `EASY_CHAT_TYPE_INTERVIEW`: quitting is allowed.
pub const INTERVIEW: u8 = 5;
/// `EASY_CHAT_TYPE_CONTEST_INTERVIEW`: `StartConfirmExitPrompt` refuses to quit.
pub const CONTEST_INTERVIEW: u8 = 11;
/// `EASY_CHAT_TYPE_APPRENTICE`: quitting is refused, same as the contest interview.
pub const APPRENTICE: u8 = 19;
/// `EASY_CHAT_TYPE_QUESTIONNAIRE`: `TryConfirmWords` always asks to confirm.
pub const ASKS_CONFIRM: u8 = 20;

const BARD: u8 = 6;
const MAX_WORDS: usize = 10;
const GROUP_COLS: i8 = 2;
const WORD_COLS: i8 = 2;
const VISIBLE: i8 = 4;
const KEYBOARD: (u8, u8, u8, u8) = (1, 11, 28, 8);

/// Where the input is (Gen3 `sEasyChatScreen->inputState`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// The cursor on word (`row`, `col`) of the phrase.
    Phrase { row: i8, col: i8 },
    /// The cursor on button `col` (DEL. ALL, CANCEL, OK).
    Buttons { col: i8 },
    /// The group keyboard. `scroll` is the first visible row.
    Keyboard { row: i8, col: i8, scroll: i8 },
    /// Words of `group`, two per row.
    Words {
        group: u8,
        row: i8,
        col: i8,
        scroll: i8,
    },
    /// "Quit?" yes/no, `cursor` 0 yes.
    Quit { cursor: u8 },
    /// DEL. ALL yes/no, `cursor` 0 yes. Opens on NO.
    Delete { cursor: u8 },
    /// "Is this OK?" yes/no, `cursor` 0 yes.
    Confirm { cursor: u8 },
    /// "You cannot quit here." A or B returns to `back`.
    CannotQuit { back: Back },
}

/// The phrase or the button row a prompt returns to (`GetEasyChatBackupState`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Back {
    Phrase,
    Buttons { col: i8 },
}

struct Render {
    phrase: Panel,
    keyboard: Option<Panel>,
    yes_no: Option<YesNo>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EasyChat {
    kind: u8,
    cols: i8,
    rows: i8,
    slots: usize,
    words: [u16; MAX_WORDS],
    saved: [u16; MAX_WORDS],
    phrase_row: i8,
    phrase_col: i8,
    /// Group-keyboard cursor while a word list is open.
    kb_row: i8,
    kb_col: i8,
    kb_scroll: i8,
    back: Back,
    st: Prompt,
    gfx: Gfx<Render>,
}

/// `(columns, rows)` of `sEasyChatScreenTemplates` for `kind` (the first template of that type).
fn layout(kind: u8) -> (i8, i8) {
    match kind {
        1 | 2 | 3 | BARD | APPRENTICE => (2, 3),
        4 | 16 | 17 => (2, 5),
        7 | 10 | CONTEST_INTERVIEW | 12 | 14 | 15 | 18 => (1, 1),
        9 | 13 => (2, 1),
        _ => (COLUMNS, ROWS),
    }
}

fn group_at(row: i8, col: i8, scroll: i8) -> Option<u8> {
    let i = (GROUP_COLS * (row + scroll) + col) as usize;
    words::OPEN.get(i).copied()
}

fn group_rows() -> i8 {
    (words::OPEN.len() as i8 + GROUP_COLS - 1) / GROUP_COLS
}

impl EasyChat {
    /// `kind`: `EASY_CHAT_TYPE_*` (`gSpecialVar_0x8004`). The phrase starts empty.
    pub fn new(kind: u8) -> Self {
        let (cols, rows) = layout(kind);
        let slots = ((cols * rows) as usize).min(MAX_WORDS);
        EasyChat {
            kind,
            cols,
            rows,
            slots,
            words: [words::EMPTY; MAX_WORDS],
            saved: [words::EMPTY; MAX_WORDS],
            phrase_row: 0,
            phrase_col: 0,
            kb_row: 0,
            kb_col: 0,
            kb_scroll: 0,
            back: Back::Phrase,
            st: Prompt::Phrase { row: 0, col: 0 },
            gfx: Gfx::none(),
        }
    }

    pub fn prompt(&self) -> Prompt {
        self.st
    }

    /// The phrase's word ids (`EC_EMPTY_WORD` for a blank cell), in row-major order.
    pub fn phrase(&self) -> &[u16] {
        &self.words[..self.slots]
    }

    fn changed(&self) -> bool {
        self.words[..self.slots] != self.saved[..self.slots]
    }

    fn phrase_empty(&self) -> bool {
        self.words[..self.slots].iter().all(|&w| w == words::EMPTY)
    }

    fn forbids_quit(&self) -> bool {
        self.kind == CONTEST_INTERVIEW || self.kind == APPRENTICE
    }

    fn phrase_here(&self) -> Prompt {
        Prompt::Phrase {
            row: self.phrase_row,
            col: self.phrase_col,
        }
    }

    fn restore(&self) -> Prompt {
        match self.back {
            Back::Phrase => self.phrase_here(),
            Back::Buttons { col } => Prompt::Buttons { col },
        }
    }

    fn open_quit(&mut self, from: Back) -> Prompt {
        self.back = from;
        if self.forbids_quit() {
            Prompt::CannotQuit { back: from }
        } else {
            Prompt::Quit { cursor: 1 }
        }
    }

    /// `TryConfirmWords`.
    fn confirm_or_quit(&mut self, from: Back) -> Prompt {
        self.back = from;
        if self.kind == ASKS_CONFIRM {
            return Prompt::Confirm { cursor: 0 };
        }
        if self.forbids_quit() {
            return if self.phrase_empty() {
                Prompt::CannotQuit { back: from }
            } else {
                Prompt::Confirm { cursor: 0 }
            };
        }
        if self.phrase_empty() || !self.changed() {
            Prompt::Quit { cursor: 1 }
        } else {
            Prompt::Confirm { cursor: 0 }
        }
    }

    fn move_phrase(&mut self, mut row: i8, mut col: i8, p: Buttons) -> Prompt {
        if p.contains(Buttons::UP) {
            row -= 1;
        } else if p.contains(Buttons::DOWN) {
            row += 1;
        } else if p.contains(Buttons::LEFT) {
            col -= 1;
        } else if p.contains(Buttons::RIGHT) {
            col += 1;
        } else {
            return Prompt::Phrase { row, col };
        }
        if row < 0 {
            row = self.rows;
        } else if row > self.rows {
            row = 0;
        }
        if row == self.rows {
            return Prompt::Buttons {
                col: col.clamp(0, 2),
            };
        }
        if col < 0 {
            col = self.cols - 1;
        } else if col >= self.cols {
            col = 0;
        }
        if self.cols == 2 && self.rows == 5 && col == 1 && row == 4 {
            col = 0;
        }
        self.phrase_row = row;
        self.phrase_col = col;
        Prompt::Phrase { row, col }
    }

    fn move_keyboard(&self, mut row: i8, mut col: i8, mut scroll: i8, p: Buttons) -> Prompt {
        let rows = group_rows();
        if p.contains(Buttons::UP) {
            if row + scroll > 0 {
                if row > 0 {
                    row -= 1;
                } else {
                    scroll -= 1;
                }
            }
        } else if p.contains(Buttons::DOWN) {
            if row + scroll < rows - 1 {
                if row < VISIBLE - 1 {
                    row += 1;
                } else {
                    scroll += 1;
                }
            }
        } else if p.contains(Buttons::LEFT) || p.contains(Buttons::RIGHT) {
            let other = if col == 0 { 1 } else { 0 };
            if group_at(row, other, scroll).is_some() {
                col = other;
            }
        }
        if group_at(row, col, scroll).is_none() {
            col = 0;
        }
        Prompt::Keyboard { row, col, scroll }
    }

    fn move_words(
        &self,
        group: u8,
        mut row: i8,
        mut col: i8,
        mut scroll: i8,
        p: Buttons,
    ) -> Prompt {
        let n = words::group(group).len();
        let last = if n == 0 { 0 } else { ((n - 1) / 2) as i8 };
        if p.contains(Buttons::UP) {
            if row + scroll > 0 {
                if row > 0 {
                    row -= 1;
                } else {
                    scroll -= 1;
                }
            }
        } else if p.contains(Buttons::DOWN) {
            if row + scroll < last {
                if row < VISIBLE - 1 {
                    row += 1;
                } else {
                    scroll += 1;
                }
            }
        } else if p.contains(Buttons::LEFT) || p.contains(Buttons::RIGHT) {
            let other = if col == 0 { 1 } else { 0 };
            if self.word_at(group, row, other, scroll).is_some() {
                col = other;
            }
        }
        if self.word_at(group, row, col, scroll).is_none() {
            col = 0;
        }
        Prompt::Words {
            group,
            row,
            col,
            scroll,
        }
    }

    fn word_at(&self, group: u8, row: i8, col: i8, scroll: i8) -> Option<u16> {
        let i = (WORD_COLS * (row + scroll) + col) as usize;
        words::group(group).get(i).copied()
    }

    fn place(&mut self, word: u16) -> Prompt {
        let i = (self.phrase_row * self.cols + self.phrase_col) as usize;
        if i < self.slots {
            self.words[i] = word;
        }
        self.phrase_here()
    }

    fn compose<G: GameData>(cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&r.phrase);
        if let Some(k) = &r.keyboard {
            map.put(k);
        }
        if let Some(y) = &r.yes_no {
            map.put(&y.panel);
        }
        map.flush(cx.f);
    }

    fn cursor_on(game: &impl GameData, panel: &mut Panel, col: i8, row: i8) {
        let x = 8 + i16::from(col) * 96;
        let y = 8 + i16::from(row) * 16;
        panel.text(game, &[widgets::CHAR_CURSOR, widgets::EOS], x, y);
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let game = cx.game;
        let (l, t, w, h) = WINDOW;
        let mut tiles = TileAlloc::full();
        let mut phrase = tiles.panel(game, l, t, w, h, true);
        if let Prompt::Phrase { row, col } = self.st {
            Self::cursor_on(game, &mut phrase, col, row);
        }
        let keyboard = match self.st {
            Prompt::Keyboard { row, col, .. } => {
                let (l, t, w, h) = KEYBOARD;
                let mut panel = tiles.panel(game, l, t, w, h, true);
                Self::cursor_on(game, &mut panel, col, row);
                Some(panel)
            }
            Prompt::Words { col, row, .. } => {
                let (l, t, w, h) = KEYBOARD;
                let mut panel = tiles.panel(game, l, t, w, h, true);
                Self::cursor_on(game, &mut panel, col, row);
                Some(panel)
            }
            _ => None,
        };
        let yes_no = match self.st {
            Prompt::Quit { cursor } | Prompt::Confirm { cursor } | Prompt::Delete { cursor } => {
                Some(YesNo::new(game, &mut tiles, cursor == 1))
            }
            _ => None,
        };
        let r = Render {
            phrase,
            keyboard,
            yes_no,
        };
        Self::compose(cx, &r);
        self.gfx.0 = Some(alloc::boxed::Box::new(r));
    }
}

impl Screen for EasyChat {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let game = cx.game;
        let p = input.pressed;
        let before = self.st;
        let mut out = ScreenOut::Running;
        self.st = match self.st {
            Prompt::Phrase { row, col } => {
                self.phrase_row = row;
                self.phrase_col = col;
                if p.contains(Buttons::A) {
                    Prompt::Keyboard {
                        row: 0,
                        col: 0,
                        scroll: 0,
                    }
                } else if p.contains(Buttons::B) {
                    self.open_quit(Back::Phrase)
                } else if p.contains(Buttons::START) {
                    self.confirm_or_quit(Back::Phrase)
                } else {
                    self.move_phrase(row, col, p)
                }
            }
            Prompt::Buttons { col } => {
                if p.contains(Buttons::A) {
                    match col {
                        0 if self.kind != BARD => {
                            self.back = Back::Buttons { col };
                            Prompt::Delete { cursor: 1 }
                        }
                        1 => self.open_quit(Back::Buttons { col }),
                        2 => self.confirm_or_quit(Back::Buttons { col }),
                        _ => Prompt::Buttons { col },
                    }
                } else if p.contains(Buttons::B) {
                    self.open_quit(Back::Buttons { col })
                } else if p.contains(Buttons::START) {
                    self.confirm_or_quit(Back::Buttons { col })
                } else if p.contains(Buttons::UP) {
                    let row = self.rows - 1;
                    let mut c = col.min(self.cols - 1);
                    if self.cols == 2 && self.rows == 5 && row == 4 {
                        c = 0;
                    }
                    self.phrase_row = row;
                    self.phrase_col = c;
                    Prompt::Phrase { row, col: c }
                } else if p.contains(Buttons::DOWN) {
                    let c = col.min(self.cols - 1);
                    self.phrase_row = 0;
                    self.phrase_col = c;
                    Prompt::Phrase { row: 0, col: c }
                } else if p.contains(Buttons::LEFT) {
                    Prompt::Buttons { col: (col + 2) % 3 }
                } else if p.contains(Buttons::RIGHT) {
                    Prompt::Buttons { col: (col + 1) % 3 }
                } else {
                    Prompt::Buttons { col }
                }
            }
            Prompt::Keyboard { row, col, scroll } => {
                if p.contains(Buttons::B) {
                    self.phrase_here()
                } else if p.contains(Buttons::A) {
                    if let Some(group) = group_at(row, col, scroll) {
                        self.kb_row = row;
                        self.kb_col = col;
                        self.kb_scroll = scroll;
                        Prompt::Words {
                            group,
                            row: 0,
                            col: 0,
                            scroll: 0,
                        }
                    } else {
                        Prompt::Keyboard { row, col, scroll }
                    }
                } else {
                    self.move_keyboard(row, col, scroll, p)
                }
            }
            Prompt::Words {
                group,
                row,
                col,
                scroll,
            } => {
                if p.contains(Buttons::B) {
                    Prompt::Keyboard {
                        row: self.kb_row,
                        col: self.kb_col,
                        scroll: self.kb_scroll,
                    }
                } else if p.contains(Buttons::A) {
                    if let Some(word) = self.word_at(group, row, col, scroll) {
                        self.place(word)
                    } else {
                        Prompt::Words {
                            group,
                            row,
                            col,
                            scroll,
                        }
                    }
                } else {
                    self.move_words(group, row, col, scroll, p)
                }
            }
            Prompt::CannotQuit { back } => {
                if p.contains(Buttons::A) || p.contains(Buttons::B) {
                    self.back = back;
                    self.restore()
                } else {
                    Prompt::CannotQuit { back }
                }
            }
            Prompt::Quit { .. } | Prompt::Confirm { .. } | Prompt::Delete { .. } => {
                let which = self.st;
                let answer = (self.gfx.0.as_deref_mut())
                    .and_then(|r| r.yes_no.as_mut())
                    .map(|y| (y.input(game, p), y.cursor));
                match answer {
                    Some((Some(true), _)) if matches!(which, Prompt::Delete { .. }) => {
                        self.words[..self.slots].fill(words::EMPTY);
                        Prompt::Buttons { col: 0 }
                    }
                    Some((Some(true), _)) if matches!(which, Prompt::Confirm { .. }) => {
                        let changed = self.changed();
                        self.saved = self.words;
                        out = ScreenOut::Done(ScreenResult::Bool(changed));
                        which
                    }
                    Some((Some(true), _)) => {
                        out = ScreenOut::Done(ScreenResult::Bool(false));
                        which
                    }
                    Some((Some(false), _)) if matches!(which, Prompt::Delete { .. }) => {
                        Prompt::Buttons { col: 0 }
                    }
                    Some((Some(false), _)) => self.restore(),
                    Some((None, cursor)) => match which {
                        Prompt::Quit { .. } => Prompt::Quit { cursor },
                        Prompt::Delete { .. } => Prompt::Delete { cursor },
                        _ => Prompt::Confirm { cursor },
                    },
                    None => which,
                }
            }
        };
        if self.gfx.0.is_none() || (before != self.st && out == ScreenOut::Running) {
            self.draw(cx);
        }
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(r) = self.gfx.0.as_deref_mut() {
            if let Some(y) = &mut r.yes_no {
                y.panel.flush_within(cx.f, &mut budget);
            }
            if let Some(k) = &mut r.keyboard {
                k.flush_within(cx.f, &mut budget);
            }
            r.phrase.flush_within(cx.f, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        let (tag, a, b) = match self.st {
            Prompt::Phrase { row, col } => (0, row as u8, col as u8),
            Prompt::Buttons { col } => (1, col as u8, 0),
            Prompt::Quit { cursor } => (2, cursor, 0),
            Prompt::Confirm { cursor } => (3, cursor, 0),
            Prompt::Keyboard {
                row,
                col,
                scroll: _,
            } => (4, row as u8, col as u8),
            Prompt::Words {
                group, row, col, ..
            } => (5, group, row as u8 | ((col as u8) << 4)),
            Prompt::CannotQuit { .. } => (6, 0, 0),
            Prompt::Delete { cursor } => (7, cursor, 0),
        };
        // The first four bytes of a phrase, the buttons, quit and confirm match the screen
        // from before the keyboard, while the phrase is still empty.
        h.write(&[self.kind, tag, a, b]);
        if let Prompt::Keyboard { scroll, .. } = self.st {
            h.write(&[scroll as u8]);
        }
        if let Prompt::Words { scroll, .. } = self.st {
            h.write(&[scroll as u8]);
        }
        if self.words[..self.slots].iter().any(|&w| w != words::EMPTY) {
            h.write(&[0xEC]);
            for w in &self.words[..self.slots] {
                h.write_u16(*w);
            }
        }
    }
}

#[cfg(test)]
#[path = "easy_chat_tests.rs"]
mod tests;
