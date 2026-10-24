//! The Easy Chat screen (Gen3 `easy_chat.c`: `ShowEasyChatScreen`, `CB2_EasyChatScreen`,
//! `HandleEasyChatInput`), as far as its flow goes.
//!
//! - The phrase (`INPUTSTATE_PHRASE`): the cursor moves over its words and wraps into the
//!   button row below (`HandleEasyChatInput_Phrase`); B asks to quit
//!   (`StartConfirmExitPrompt`); START tries to confirm (`TryConfirmWords`).
//! - The buttons (`INPUTSTATE_MAIN_SCREEN_BUTTONS`): DEL. ALL (not modelled), CANCEL (the quit
//!   prompt) and OK (`TryConfirmWords`); B asks to quit; UP goes back to the phrase.
//! - The quit prompt (`ShowConfirmExitPrompt`, its yes/no on NO): yes closes with
//!   `Bool(false)` (`gSpecialVar_Result = 0`), no or B goes back.
//! - The confirm prompt (`ShowConfirmPrompt`, its yes/no on YES): yes closes with `Bool(true)`
//!   (the caller's `SetSpecialEasyChatResult`), no or B goes back.
//!
//! Deviations (degraded): no keyboard and no word lists, so the phrase never changes: A on a
//! word does nothing, and `TryConfirmWords` takes its "unchanged phrase" branch: the quit
//! prompt, except for the questionnaire ([`ASKS_CONFIRM`]), which always asks to confirm. The
//! types that cannot be quit in Gen3 (the apprentice, the contest interview: "You cannot quit
//! here") quit here. One framed window stands for the screen; no title, words or cursor sprite.

use super::kit::Gfx;
use super::kit::widgets::YesNo;
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;
use alloc::boxed::Box;

/// The phrase window: left, top, width, height (tiles).
pub const WINDOW: (u8, u8, u8, u8) = (3, 3, 24, 8);
/// The phrase's rows and columns (the profile's 2x2 frame, `FRAMEID_GENERAL_2x2`).
pub const ROWS: i8 = 2;
pub const COLUMNS: i8 = 2;
/// `EASY_CHAT_TYPE_QUESTIONNAIRE`: `TryConfirmWords` always asks to confirm.
pub const ASKS_CONFIRM: u8 = 20;

/// Where the input is (Gen3 `sEasyChatScreen->inputState`), for tools.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// The cursor on word (`row`, `col`) of the phrase.
    Phrase { row: i8, col: i8 },
    /// The cursor on button `col` (DEL. ALL, CANCEL, OK).
    Buttons { col: i8 },
    /// "Quit?" yes/no, `cursor` 0 yes.
    Quit { cursor: u8 },
    /// "Is this OK?" yes/no, `cursor` 0 yes.
    Confirm { cursor: u8 },
}

struct Render {
    phrase: Panel,
    yes_no: Option<YesNo>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EasyChat {
    kind: u8,
    st: Prompt,
    gfx: Gfx<Render>,
}

impl EasyChat {
    /// `kind`: `EASY_CHAT_TYPE_*` (`gSpecialVar_0x8004`).
    pub fn new(kind: u8) -> Self {
        EasyChat {
            kind,
            st: Prompt::Phrase { row: 0, col: 0 },
            gfx: Gfx::none(),
        }
    }

    pub fn prompt(&self) -> Prompt {
        self.st
    }

    fn compose<G: GameData>(cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&r.phrase);
        if let Some(y) = &r.yes_no {
            map.put(&y.panel);
        }
        map.flush(cx.f);
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let game = cx.game;
        let (l, t, w, h) = WINDOW;
        let mut tiles = TileAlloc::full();
        let phrase = tiles.panel(game, l, t, w, h, true);
        let yes_no = match self.st {
            Prompt::Quit { cursor } | Prompt::Confirm { cursor } => {
                Some(YesNo::new(game, &mut tiles, cursor == 1))
            }
            _ => None,
        };
        let r = Render { phrase, yes_no };
        Self::compose(cx, &r);
        self.gfx.0 = Some(Box::new(r));
    }

    /// `TryConfirmWords` with the phrase unchanged.
    fn try_confirm(&self) -> Prompt {
        if self.kind == ASKS_CONFIRM {
            Prompt::Confirm { cursor: 0 }
        } else {
            Prompt::Quit { cursor: 1 }
        }
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
                if p.contains(Buttons::B) {
                    Prompt::Quit { cursor: 1 }
                } else if p.contains(Buttons::START) {
                    self.try_confirm()
                } else {
                    let (mut r, mut c) = (row, col);
                    if p.contains(Buttons::UP) {
                        r -= 1;
                    } else if p.contains(Buttons::DOWN) {
                        r += 1;
                    } else if p.contains(Buttons::LEFT) {
                        c -= 1;
                    } else if p.contains(Buttons::RIGHT) {
                        c += 1;
                    }
                    // Wrap the row through the button row (`numRows`).
                    if r < 0 {
                        r = ROWS;
                    } else if r > ROWS {
                        r = 0;
                    }
                    if r == ROWS {
                        Prompt::Buttons { col: c.clamp(0, 2) }
                    } else {
                        Prompt::Phrase {
                            row: r,
                            col: c.rem_euclid(COLUMNS),
                        }
                    }
                }
            }
            Prompt::Buttons { col } => {
                if p.contains(Buttons::A) {
                    match col {
                        1 => Prompt::Quit { cursor: 1 },
                        2 => self.try_confirm(),
                        // DEL. ALL: not modelled (the phrase stays).
                        _ => Prompt::Buttons { col },
                    }
                } else if p.contains(Buttons::B) {
                    Prompt::Quit { cursor: 1 }
                } else if p.contains(Buttons::UP) {
                    Prompt::Phrase {
                        row: ROWS - 1,
                        col: col.min(COLUMNS - 1),
                    }
                } else if p.contains(Buttons::DOWN) {
                    Prompt::Phrase {
                        row: 0,
                        col: col.min(COLUMNS - 1),
                    }
                } else if p.contains(Buttons::LEFT) {
                    Prompt::Buttons { col: (col + 2) % 3 }
                } else if p.contains(Buttons::RIGHT) {
                    Prompt::Buttons { col: (col + 1) % 3 }
                } else {
                    Prompt::Buttons { col }
                }
            }
            Prompt::Quit { .. } | Prompt::Confirm { .. } => {
                let quit = matches!(self.st, Prompt::Quit { .. });
                let answer = (self.gfx.0.as_deref_mut())
                    .and_then(|r| r.yes_no.as_mut())
                    .map(|y| (y.input(game, p), y.cursor));
                match answer {
                    Some((Some(true), _)) => {
                        out = ScreenOut::Done(ScreenResult::Bool(!quit));
                        self.st
                    }
                    // No or B: back to the phrase (`GetEasyChatBackupState`).
                    Some((Some(false), _)) => Prompt::Phrase { row: 0, col: 0 },
                    Some((None, cursor)) if quit => Prompt::Quit { cursor },
                    Some((None, cursor)) => Prompt::Confirm { cursor },
                    None => self.st,
                }
            }
        };
        let reopened = core::mem::discriminant(&before) != core::mem::discriminant(&self.st);
        if self.gfx.0.is_none() || (reopened && out == ScreenOut::Running) {
            self.draw(cx);
        }
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(r) = self.gfx.0.as_deref_mut() {
            if let Some(y) = &mut r.yes_no {
                y.panel.flush_within(cx.f, &mut budget);
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
        };
        h.write(&[self.kind, tag, a, b]);
    }
}

#[cfg(test)]
#[path = "easy_chat_tests.rs"]
mod tests;
