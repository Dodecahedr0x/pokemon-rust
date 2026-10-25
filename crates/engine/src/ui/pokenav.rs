//! The PokéNav (Gen3 `pokenav.c`, the Match Call list in `pokenav_match_call_list.c`).
//!
//! START → POKéNAV opens the main menu. MATCH CALL lists `GameData::match_calls` (the
//! contacts whose flag is set; `shown_if: None` is always listed, Mr. Stone). A contact's
//! CALL option speaks the line [`spoken_line`] picks, the way
//! `MatchCall_BufferCallMessageText` walks the text table, and sets that line's flag.
//!
//! `OpenPokenavForTutorial` opens it in `POKENAV_MODE_FORCE_CALL_READY`: only MATCH CALL is
//! accepted until a call is placed, then SWITCH OFF (or B) leaves
//! (`POKENAV_MODE_FORCE_CALL_EXIT`). The cursor starts on MATCH CALL. Gen3's menu also has
//! the map, condition and ribbons, and a CHECK page beside CALL; those are not this screen.
//! Route-trainer rematch messages are whatever the game's match-call table contains.

use super::kit::Gfx;
use super::kit::frame_budget;
use super::kit::list::{ListMenu, MenuInput, Paging};
use super::kit::widgets::{CURSOR_W, Message, ROW_H, draw_cursor, draw_menu};
use super::kit::window::{FULL_BASE, MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::{FlagId, GameData, MatchCallLine};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::text::Window;
use alloc::boxed::Box;

/// The menu window: wide enough for a name, tall enough for six rows.
const MENU: (u8, u8, u8, u8) = (1, 1, 16, 12);
/// The call, in the field message box's place. Its tiles sit past the menu's canvas.
const MSG: Window = Window::new(2, 15, 27, 4, FULL_BASE + 16 * 12);
const LIST_ROWS: u16 = 6;

/// "MATCH CALL"
const LABEL_MATCH: &[u8] = &[
    0xC7, 0xBB, 0xCE, 0xBD, 0xC2, 0x00, 0xBD, 0xBB, 0xC6, 0xC6, 0xFF,
];
/// "SWITCH OFF"
const LABEL_OFF: &[u8] = &[
    0xCD, 0xD1, 0xC3, 0xCE, 0xBD, 0xC2, 0x00, 0xC9, 0xC0, 0xC0, 0xFF,
];
/// "CALL"
const LABEL_CALL: &[u8] = &[0xBD, 0xBB, 0xC6, 0xC6, 0xFF];
/// "CANCEL"
const LABEL_CANCEL: &[u8] = &[0xBD, 0xBB, 0xC8, 0xBD, 0xBF, 0xC6, 0xFF];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Main = 0,
    List = 1,
    Options = 2,
    Call = 3,
}

/// `POKENAV_MODE_*` for the part of the tutorial this screen runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tut {
    Off = 0,
    /// `POKENAV_MODE_FORCE_CALL_READY`: the call has not been placed.
    Ready = 1,
    /// `POKENAV_MODE_FORCE_CALL_EXIT`: the call was placed.
    Exit = 2,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pokenav {
    tut: Tut,
    page: Page,
    main: u8,
    options: u8,
    list: ListMenu,
    /// The contact under the cursor (`GameData::match_calls` name).
    name: Option<&'static [u8]>,
    /// The line the open call is speaking.
    call: Option<&'static [u8]>,
    msg: Option<Message>,
    gfx: Gfx<Panel>,
}

impl Pokenav {
    pub fn new(tutorial: bool) -> Self {
        Pokenav {
            tut: if tutorial { Tut::Ready } else { Tut::Off },
            page: Page::Main,
            main: 0,
            options: 0,
            list: ListMenu::new(1, 1, Paging::None),
            name: None,
            call: None,
            msg: None,
            gfx: Gfx::none(),
        }
    }

    /// The contact selected in the Match Call list.
    pub fn list_name(&self) -> Option<&'static [u8]> {
        if matches!(self.page, Page::List | Page::Options) {
            self.name
        } else {
            None
        }
    }

    /// The line of the call being placed, until it is dismissed.
    pub fn call(&self) -> Option<&'static [u8]> {
        self.call
    }
}

/// The line `MatchCall_BufferCallMessageText` would copy: from the last entry, the latest
/// whose `requires` flag is set, or the first when none is.
pub fn spoken_line<'a>(
    lines: &'a [MatchCallLine],
    flag: impl Fn(FlagId) -> bool,
) -> Option<&'a MatchCallLine> {
    if lines.is_empty() {
        return None;
    }
    let mut i = lines.len() - 1;
    while i > 0 {
        if lines[i].requires.is_some_and(&flag) {
            break;
        }
        i -= 1;
    }
    Some(&lines[i])
}

fn visible<G: GameData>(cx: &ScreenCx<'_, G>) -> alloc::vec::Vec<usize> {
    cx.game
        .match_calls()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.shown_if.is_none_or(|f| cx.vars.flag(f)))
        .map(|(i, _)| i)
        .collect()
}

impl Pokenav {
    fn remember<G: GameData>(&mut self, cx: &ScreenCx<'_, G>, visible: &[usize]) {
        self.name = visible
            .get(self.list.selected() as usize)
            .map(|&i| cx.game.match_calls()[i].name);
    }

    fn enter_list(&mut self, n: u16) {
        self.list = ListMenu::new(n.max(1), LIST_ROWS, Paging::Dpad);
        self.page = Page::List;
        self.gfx.0 = None;
    }

    fn place<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, visible: &[usize]) {
        let Some(&i) = visible.get(self.list.selected() as usize) else {
            return;
        };
        let call = &cx.game.match_calls()[i];
        let Some(line) = spoken_line(call.lines, |f| cx.vars.flag(f)) else {
            return;
        };
        if let Some(f) = line.sets {
            cx.vars.set_flag(f, true);
        }
        self.call = Some(line.text);
        let speed = cx.options.text_speed;
        let msg = self.msg.get_or_insert_with(|| Message::new(MSG, speed));
        msg.start(cx.game, line.text, &cx.player_name[..], None, true);
        if self.tut == Tut::Ready {
            self.tut = Tut::Exit;
        }
        self.page = Page::Call;
        self.gfx.0 = None;
    }

    fn step_call<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> bool {
        let Some(msg) = self.msg.as_mut() else {
            return true;
        };
        let done = msg.update(cx.game, input.pressed, input.held);
        msg.flush_within(cx.f, &mut frame_budget(cx.f));
        if !done {
            return false;
        }
        self.msg = None;
        self.call = None;
        if self.tut == Tut::Exit {
            self.page = Page::Main;
            self.main = 1;
        } else {
            self.page = Page::List;
        }
        self.gfx.0 = None;
        true
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, visible: &[usize]) {
        let game = cx.game;
        if self.gfx.0.is_none() {
            let (l, t, w, h) = MENU;
            let p = TileAlloc::full().panel(game, l, t, w, h, true);
            self.gfx.0 = Some(Box::new(p));
        }
        let p = self.gfx.0.as_deref_mut().expect("panel");
        match self.page {
            Page::Main => draw_menu(game, p, &[LABEL_MATCH, LABEL_OFF], self.main),
            Page::Options => draw_menu(game, p, &[LABEL_CALL, LABEL_CANCEL], self.options),
            Page::List | Page::Call => {
                p.clear(game);
                let calls = game.match_calls();
                for (row, idx) in self.list.shown().enumerate() {
                    let Some(&i) = visible.get(idx as usize) else {
                        break;
                    };
                    p.text(game, calls[i].name, CURSOR_W, row as i16 * ROW_H + 1);
                }
                let rows = self.list.shown().count() as u8;
                draw_cursor(game, p, rows, self.list.row as u8);
            }
        }
        let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
        map.put(p);
        if self.page == Page::Call
            && let Some(m) = &mut self.msg
        {
            m.touch();
            map.put_window(m.window(), true);
        }
        map.flush(cx.f);
        p.flush_within(cx.f, &mut frame_budget(cx.f));
        if self.page == Page::Call
            && let Some(m) = &mut self.msg
        {
            m.flush_within(cx.f, &mut frame_budget(cx.f));
        }
    }
}

impl Screen for Pokenav {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let visible = visible(cx);
        if self.page == Page::Call {
            let left = self.step_call(cx, input);
            if left {
                self.remember(cx, &visible);
                self.draw(cx, &visible);
            } else if let Some(p) = self.gfx.0.as_deref_mut() {
                p.flush_within(cx.f, &mut frame_budget(cx.f));
            }
            return ScreenOut::Running;
        }
        let pressed = input.pressed;
        let mut closed = false;
        match self.page {
            Page::Main => {
                if pressed.contains(Buttons::UP) || pressed.contains(Buttons::DOWN) {
                    self.main ^= 1;
                    self.gfx.0 = None;
                } else if pressed.contains(Buttons::A) {
                    if self.main == 0 {
                        if visible.is_empty() {
                            if self.tut == Tut::Ready {
                                self.tut = Tut::Exit;
                                self.main = 1;
                            }
                        } else {
                            self.enter_list(visible.len() as u16);
                        }
                    } else if self.tut != Tut::Ready {
                        closed = true;
                    }
                } else if pressed.contains(Buttons::B) && self.tut != Tut::Ready {
                    closed = true;
                }
            }
            Page::List => match self.list.input(pressed) {
                MenuInput::Chosen(_) => {
                    self.page = Page::Options;
                    self.options = 0;
                    self.gfx.0 = None;
                }
                MenuInput::Cancel if self.tut != Tut::Ready => {
                    self.page = Page::Main;
                    self.gfx.0 = None;
                }
                MenuInput::Moved => self.gfx.0 = None,
                _ => {}
            },
            Page::Options => {
                if pressed.contains(Buttons::UP) || pressed.contains(Buttons::DOWN) {
                    self.options ^= 1;
                    self.gfx.0 = None;
                } else if pressed.contains(Buttons::A) {
                    if self.options == 0 {
                        self.place(cx, &visible);
                    } else {
                        self.page = Page::List;
                        self.gfx.0 = None;
                    }
                } else if pressed.contains(Buttons::B) {
                    self.page = Page::List;
                    self.gfx.0 = None;
                }
            }
            Page::Call => {}
        }
        if closed {
            return ScreenOut::Done(ScreenResult::Cancel);
        }
        self.remember(cx, &visible);
        let fresh = self.gfx.0.is_none();
        if fresh || self.page == Page::Call {
            self.draw(cx, &visible);
        } else if let Some(p) = self.gfx.0.as_deref_mut() {
            p.flush_within(cx.f, &mut frame_budget(cx.f));
        }
        ScreenOut::Running
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.page as u8, self.tut as u8, self.main, self.options]);
        self.list.hash(h);
        h.write(&[self.call.is_some() as u8]);
    }
}

#[cfg(test)]
#[path = "pokenav_tests.rs"]
mod tests;
