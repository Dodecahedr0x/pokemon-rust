//! The save dialog (Gen3 `start_menu.c` `SaveGame` tasks; Milestone 9 Task S).
//!
//! An overlay over the field (the start menu's SAVE, or `special SaveGame`):
//! - `SaveConfirmSaveCallback`: the info window ([`INFO_RECT`], `sSaveInfoWindowTemplate`, two
//!   rows shorter without the Pokédex flag, `ShowSaveInfoWindow`: the player's name, the badges,
//!   the Pokédex count and the play time, each right-aligned at 0x70) and "Would you like to
//!   save the game?" in the message box;
//! - once printed, the yes/no (`DisplayYesNoMenuDefaultYes`); no or B closes the dialog
//!   (`SAVE_CANCELED`: the start menu shows again; `special SaveGame` answers 0);
//! - yes: the overwrite question ([`SaveQuestion::Overwrite`], default yes) or the different
//!   file's warning ([`SaveQuestion::Different`], default no) when Gen3 asks one
//!   (`SaveFileExistsCallback`), then "SAVING… DON'T TURN OFF THE POWER." (`SaveSavingMessageCallback`);
//! - once that is printed, the save itself ([`ScreenOut::Save`], `SaveDoSaveCallback` →
//!   `TrySavingData`): the World saves and raises `Event::Saved`, the result comes back the next
//!   frame (a shell's failed write turns it to "not saved", `World::save_failed`);
//! - "{PLAYER} saved the game." then [`SAVE_TIMER`] frames or A held (`SaveSuccesTimer`), or
//!   "Save error." then [`SAVE_TIMER`] frames and A held (`SaveErrorTimer`); the dialog answers
//!   `Bool(ok)` (`SAVE_SUCCESS` / `SAVE_ERROR`: the start menu closes).
//!
//! Deviations: no location line (the engine has no map section names); the message box uses
//! the menu frame, not the dialogue frame; no colours on the info window's values (Gen3 prints
//! them blue or red by gender); no sound effects, so `SaveReturnSuccessCallback` does not wait
//! for `SE_SAVE` to end.

use super::kit::Gfx;
use super::kit::widgets::{CHAR_0, EOS, Message, YesNo, push, push_number};
use super::kit::window::{FULL_BASE, MapBuf, Panel, TileAlloc, frame_kind};
use super::{SaveQuestion, Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::{GameData, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::pokedex::DexFlag;
use crate::text::Window;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `sSaveInfoWindowTemplate`: left, top, width, height (two rows less without a Pokédex).
pub const INFO_RECT: (u8, u8, u8, u8) = (1, 1, 14, 10);
/// The message box (Gen3 window 0, the field's dialogue window).
pub const MSG_RECT: (u8, u8, u8, u8) = (2, 15, 27, 4);
/// Gen3 `SaveStartTimer`.
pub const SAVE_TIMER: u8 = 60;
/// The right edge the info window's values align to (`GetStringRightAlignXOffset(.., 0x70)`).
const VALUE_RIGHT: i16 = 0x70;
/// `:` (Gen3 `CHAR_COLON`).
const CHAR_COLON: u8 = 0xF0;

/// What the dialog waits for (tools drive it from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// A message printing.
    Message,
    /// A yes/no (`cursor` 0 yes); `overwrite`: the second question.
    YesNo { cursor: u8, overwrite: bool },
    /// The save ran; its result arrives next frame.
    Saving,
    /// "saved" or "Save error" printed: the timer runs.
    Done { ok: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    /// The first frame: windows not drawn yet.
    Open,
    /// A message prints; then `next`.
    Msg(Next),
    /// A yes/no; `overwrite`: the second question.
    Ask { cursor: u8, overwrite: bool },
    /// [`ScreenOut::Save`] answered; waiting for the result.
    Saving,
    /// The result's message prints.
    Result(bool),
    /// The result is printed: the timer.
    Timer { ok: bool, left: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Next {
    /// The first yes/no.
    Ask,
    /// The overwrite yes/no (`default_no`: the different file's warning).
    Overwrite { default_no: bool },
    /// "SAVING…" is printed: save.
    Save,
}

struct Render {
    info: Panel,
    yes_no: Option<YesNo>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaveDialog {
    st: St,
    msg: Option<Box<Message>>,
    gfx: Gfx<Render>,
}

impl Default for SaveDialog {
    fn default() -> Self {
        SaveDialog::new()
    }
}

/// Pixel width of `text` in the game's font (up to its terminator).
fn width(game: &impl GameData, text: &[u8]) -> i16 {
    let w = game.font().widths;
    (text.iter().take_while(|&&c| c != EOS))
        .map(|&c| w.get(c as usize).copied().unwrap_or(0) as i16)
        .sum()
}

/// Gen3 `BufferSaveMenuText(SAVE_MENU_PLAY_TIME)`: hours (up to 3 digits), `:`, minutes on
/// two digits, from `frame` at 60 frames a second.
pub fn play_time(frame: u32) -> Vec<u8> {
    let minutes = frame / 3600;
    let (h, m) = ((minutes / 60).min(999), minutes % 60);
    let mut s = Vec::with_capacity(8);
    push_number(&mut s, h, 3, false);
    s.push(CHAR_COLON);
    s.push(CHAR_0 + (m / 10) as u8);
    s.push(CHAR_0 + (m % 10) as u8);
    s.push(EOS);
    s
}

/// The CONTINUE / save info window's rows (Gen3 `ShowSaveInfoWindow`,
/// `PrintContinueStats`): (label, value), the Pokédex row only with `dex`.
pub fn info_rows(
    game: &impl GameData,
    name: &[u8],
    badges: u8,
    dex: Option<u16>,
    frame: u32,
) -> Vec<(&'static [u8], Vec<u8>)> {
    let num = |n: u32| {
        let mut v = Vec::with_capacity(4);
        push_number(&mut v, n, 3, false);
        v.push(EOS);
        v
    };
    let mut player = Vec::with_capacity(8);
    push(&mut player, name);
    player.push(EOS);
    let mut rows = alloc::vec![
        (game.ui_text(UiText::MainPlayer), player),
        (game.ui_text(UiText::MainBadges), num(u32::from(badges))),
    ];
    if let Some(n) = dex {
        rows.push((game.ui_text(UiText::MainPokedex), num(u32::from(n))));
    }
    rows.push((game.ui_text(UiText::MainTime), play_time(frame)));
    rows
}

/// Draws `rows` on `p`, one per 16 pixels from y 1, values right-aligned at 0x70.
pub fn draw_rows(game: &impl GameData, p: &mut Panel, rows: &[(&'static [u8], Vec<u8>)]) {
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = 1 + 16 * i as i16;
        p.text(game, label, 0, y);
        p.text(game, value, VALUE_RIGHT - width(game, value), y);
    }
}

impl SaveDialog {
    pub fn new() -> Self {
        SaveDialog {
            st: St::Open,
            msg: None,
            gfx: Gfx::none(),
        }
    }

    /// What the dialog waits for.
    pub fn prompt(&self) -> Prompt {
        match self.st {
            St::Open | St::Msg(_) | St::Result(_) => Prompt::Message,
            St::Ask { cursor, overwrite } => Prompt::YesNo { cursor, overwrite },
            St::Saving => Prompt::Saving,
            St::Timer { ok, .. } => Prompt::Done { ok },
        }
    }

    fn layout<G: GameData>(&self, cx: &ScreenCx<'_, G>) -> Render {
        let game = cx.game;
        let ids = game.special_ids();
        let has_dex = ids.flag_pokedex.is_some_and(|f| cx.vars.flag(f));
        let (l, t, w, h) = INFO_RECT;
        let mut tiles = TileAlloc::overlay();
        let mut info = tiles.panel(game, l, t, w, if has_dex { h } else { h - 2 }, true);
        let badges = (ids.flag_badges.iter().flatten())
            .filter(|&&f| cx.vars.flag(f))
            .count() as u8;
        let dex = has_dex.then(|| cx.pokedex.count(DexFlag::Caught));
        let rows = info_rows(game, &cx.player_name[..], badges, dex, cx.frame);
        draw_rows(game, &mut info, &rows);
        Render { info, yes_no: None }
    }

    fn compose<G: GameData>(&self, cx: &mut ScreenCx<'_, G>, r: &Render) {
        let mut map = MapBuf::overlay(frame_kind(cx.game, cx.options.frame));
        map.put(&r.info);
        if let Some(m) = &self.msg
            && m.shown
        {
            map.put_window(m.window(), true);
        }
        if let Some(y) = &r.yes_no {
            map.put(&y.panel);
        }
        map.flush(cx.f);
    }

    fn show_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, text: UiText) {
        let (l, t, w, h) = MSG_RECT;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(
                Window::new(l, t, w, h, FULL_BASE),
                cx.options.text_speed,
            ))
        });
        m.start(
            cx.game,
            cx.game.ui_text(text),
            &cx.player_name[..],
            None,
            false,
        );
    }

    /// Opens a yes/no (or drops it) and recomposes the map.
    fn set_yes_no<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, default_no: Option<bool>) {
        let Some(mut r) = self.gfx.0.take() else {
            return;
        };
        r.yes_no = default_no.map(|no| {
            let base = FULL_BASE + MSG_RECT.2 as u16 * MSG_RECT.3 as u16;
            YesNo::new(cx.game, &mut TileAlloc::at(base), no)
        });
        self.compose(cx, &r);
        self.gfx.0 = Some(r);
    }

    fn redraw_all<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let mut r = self.layout(cx);
        if let St::Ask { cursor, .. } = self.st {
            let base = FULL_BASE + MSG_RECT.2 as u16 * MSG_RECT.3 as u16;
            r.yes_no = Some(YesNo::new(cx.game, &mut TileAlloc::at(base), cursor == 1));
        }
        if let Some(m) = &mut self.msg {
            m.touch();
        }
        self.compose(cx, &r);
        self.gfx.0 = Some(Box::new(r));
    }

    /// The message is printed: what follows it.
    fn after_msg<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, next: Next) -> ScreenOut {
        match next {
            Next::Ask => {
                self.st = St::Ask {
                    cursor: 0,
                    overwrite: false,
                };
                self.set_yes_no(cx, Some(false));
                ScreenOut::Running
            }
            Next::Overwrite { default_no } => {
                self.st = St::Ask {
                    cursor: default_no as u8,
                    overwrite: true,
                };
                self.set_yes_no(cx, Some(default_no));
                ScreenOut::Running
            }
            Next::Save => {
                self.st = St::Saving;
                ScreenOut::Save
            }
        }
    }

    /// Yes on the first question (`SaveConfirmInputCallback`) or the second.
    fn yes<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, overwrite: bool) {
        self.set_yes_no(cx, None);
        let (text, next) = match (overwrite, cx.save_question) {
            (false, SaveQuestion::Overwrite) => {
                (UiText::SaveOverwrite, Next::Overwrite { default_no: false })
            }
            // Deviation: the warning's own text (`gText_DifferentSaveFile`) is not extracted;
            // the overwrite question stands in for it, with its default no.
            (false, SaveQuestion::Different) => {
                (UiText::SaveOverwrite, Next::Overwrite { default_no: true })
            }
            _ => (UiText::SaveSaving, Next::Save),
        };
        self.show_msg(cx, text);
        self.st = St::Msg(next);
        if let Some(r) = self.gfx.0.take() {
            self.compose(cx, &r);
            self.gfx.0 = Some(r);
        }
    }
}

impl Screen for SaveDialog {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let game = cx.game;
        if self.st == St::Open {
            // `SaveConfirmSaveCallback`.
            self.show_msg(cx, UiText::SaveAsk);
            self.st = St::Msg(Next::Ask);
        }
        if self.gfx.0.is_none() {
            self.redraw_all(cx);
        }
        let (pressed, held) = (input.pressed, input.held);
        let out = match self.st {
            St::Open => ScreenOut::Running,
            St::Msg(next) => {
                let done = (self.msg.as_mut()).is_none_or(|m| m.update(game, pressed, held));
                if done {
                    self.after_msg(cx, next)
                } else {
                    ScreenOut::Running
                }
            }
            St::Ask { overwrite, .. } => {
                let r = self.gfx.0.as_deref_mut().and_then(|r| r.yes_no.as_mut());
                let answer = r.and_then(|y| {
                    let a = y.input(game, pressed);
                    self.st = St::Ask {
                        cursor: y.cursor,
                        overwrite,
                    };
                    a
                });
                match answer {
                    Some(true) => {
                        self.yes(cx, overwrite);
                        ScreenOut::Running
                    }
                    // `HideSaveInfoWindow`, `HideSaveMessageWindow`: `SAVE_CANCELED`.
                    Some(false) => ScreenOut::Done(ScreenResult::Cancel),
                    None => ScreenOut::Running,
                }
            }
            St::Saving => match cx.saved.take() {
                Some(ok) => {
                    // `SaveDoSaveCallback`: the result's message, and `SaveStartTimer`.
                    self.show_msg(
                        cx,
                        if ok {
                            UiText::SaveDone
                        } else {
                            UiText::SaveError
                        },
                    );
                    self.st = St::Result(ok);
                    ScreenOut::Running
                }
                None => ScreenOut::Running,
            },
            St::Result(ok) => {
                let done = (self.msg.as_mut()).is_none_or(|m| m.update(game, pressed, held));
                if done {
                    self.st = St::Timer {
                        ok,
                        left: SAVE_TIMER,
                    };
                }
                ScreenOut::Running
            }
            St::Timer { ok: true, left } => {
                // `SaveSuccesTimer`: one less a frame; A held or 0 ends it.
                let left = left.saturating_sub(1);
                self.st = St::Timer { ok: true, left };
                if held.contains(Buttons::A) || left == 0 {
                    ScreenOut::Done(ScreenResult::Bool(true))
                } else {
                    ScreenOut::Running
                }
            }
            St::Timer { ok: false, left } => {
                // `SaveErrorTimer`: the timer first, then A held.
                if left != 0 {
                    self.st = St::Timer {
                        ok: false,
                        left: left - 1,
                    };
                    ScreenOut::Running
                } else if held.contains(Buttons::A) {
                    ScreenOut::Done(ScreenResult::Bool(false))
                } else {
                    ScreenOut::Running
                }
            }
        };
        let mut budget = super::kit::frame_budget(cx.f);
        if let Some(m) = &mut self.msg {
            m.flush_within(cx.f, &mut budget);
        }
        if let Some(r) = self.gfx.0.as_deref_mut() {
            if let Some(y) = &mut r.yes_no {
                y.panel.flush_within(cx.f, &mut budget);
            }
            r.info.flush_within(cx.f, &mut budget);
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        let (tag, a, b) = match self.st {
            St::Open => (0, 0, 0),
            St::Msg(n) => (
                1,
                match n {
                    Next::Ask => 0,
                    Next::Overwrite { default_no } => 1 + default_no as u8,
                    Next::Save => 3,
                },
                0,
            ),
            St::Ask { cursor, overwrite } => (2, cursor, overwrite as u8),
            St::Saving => (3, 0, 0),
            St::Result(ok) => (4, ok as u8, 0),
            St::Timer { ok, left } => (5, ok as u8, left),
        };
        h.write(&[tag, a, b]);
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}
