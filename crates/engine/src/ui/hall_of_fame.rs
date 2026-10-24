//! The Hall of Fame record and the credits (Gen3 `hall_of_fame.c` `CB2_DoHallOfFameScreen`,
//! `credits.c`; Milestone 9 Task S), opened by `special GameClear` (`Request::HallOfFame`).
//! Degraded visuals: frames and text on a full screen, with Gen3's timings.
//!
//! - "SAVING… DON'T TURN OFF THE POWER." (`Task_Hof_InitTeamSaveData`), then
//!   [`WAIT_TO_DISPLAY`] frames (`Task_Hof_TrySaveData` → `Task_Hof_WaitToDisplayMon`). The
//!   save itself is the World's (`world/save.rs`: the Hall of Fame request marks it pending and
//!   it runs on the first capturable frame after the credits' warp, where Gen3's CONTINUE puts
//!   the player).
//! - Each party mon in turn (`Task_Hof_DisplayMon`, `Task_Hof_PrintMonInfoAfterAnimating`): its
//!   nickname and level on a new row, then [`MON_FRAMES`] frames (`Task_Hof_TryDisplayAnotherMon`).
//!   Deviation: no mon pictures, so no slide-in (`SpriteCB_GetOnScreenAndAnimate`).
//! - [`WELCOME_FRAMES`] frames of confetti (`Task_Hof_DoConfetti`; the welcome text is not
//!   extracted, so nothing is added), [`PLAYER_WAIT_FRAMES`] (`Task_Hof_WaitToDisplayPlayer`,
//!   7 to 16), then [`PLAYER_FRAMES`] and the trainer picture's slide, [`PLAYER_SLIDE_FRAMES`]
//!   (`Task_Hof_WaitAndPrintPlayerInfo`: x 120 to 192).
//! - The player's name and play time (`HallOfFame_PrintPlayerInfo`), until A is pressed
//!   (`Task_Hof_ExitOnKeyPressed`).
//! - The credits (`CB2_StartCreditsSequence`), degraded to a black screen of [`CREDITS_FRAMES`]
//!   (`Task_UpdatePage`: 72 frames, then each of the 57 pages fades in, waits 115 frames and
//!   fades out; the bike and Pokémon scenes between pages are left out), then THE END
//!   (`Task_CreditsTheEnd6`: black too here) until a key is pressed or [`THE_END_FRAMES`] pass.
//!   Deviation: START skips the credits pages to THE END (Gen3 cannot skip them), for tools.
//!
//! The screen answers `Bool(true)`; the `GameClear` script then warps to the player's room.

use super::kit::widgets::{EOS, EXTRA_SYMBOL, Message, SYMBOL_LV, push, push_number};
use super::kit::window::{FULL_BASE, MapBuf, Panel, TileAlloc, frame_kind};
use super::kit::{FADE_FRAMES, Gfx};
use super::save::{draw_rows, play_time};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::{GameData, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::text::Window;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `Task_Hof_TrySaveData`: `tFrameCount = 32`.
pub const WAIT_TO_DISPLAY: u16 = 32;
/// `Task_Hof_PrintMonInfoAfterAnimating`: `tFrameCount = 120`.
pub const MON_FRAMES: u16 = 120;
/// `Task_Hof_PaletteFadeAndPrintWelcomeText`: `tFrameCount = 400`.
pub const WELCOME_FRAMES: u16 = 400;
/// `Task_Hof_WaitToDisplayPlayer`: from 7 to 16.
pub const PLAYER_WAIT_FRAMES: u16 = 16 - 7;
/// `Task_Hof_DisplayPlayer`: `tFrameCount = 120`.
pub const PLAYER_FRAMES: u16 = 120;
/// `Task_Hof_WaitAndPrintPlayerInfo`: the picture moves one pixel a frame from 120 to 192.
pub const PLAYER_SLIDE_FRAMES: u16 = 192 - 120;
/// Gen3 `PAGE_COUNT` (`src/data/credits.h`).
pub const CREDITS_PAGES: u16 = 57;
/// `Task_UpdatePage`: 72 frames, then per page a 16-step fade in (17 frames), 115 frames, a
/// 16-step fade out (17 frames) and the frame that clears it.
pub const CREDITS_FRAMES: u16 = 72 + CREDITS_PAGES * (17 + 115 + 17 + 1);
/// `Task_CreditsTheEnd5`: `tDelay = 7200`.
pub const THE_END_FRAMES: u16 = 7200;
/// The record's list: left, top, width, height.
pub const LIST_RECT: (u8, u8, u8, u8) = (2, 1, 26, 12);
/// The message box (window 0).
pub const MSG_RECT: (u8, u8, u8, u8) = (2, 15, 27, 4);

/// Where the screen is (tools drive it from it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// "SAVING…", then the wait.
    Saving,
    /// Showing party mon `i`.
    Mon(u8),
    /// The confetti, the player's picture.
    Welcome,
    /// The player's info: waits for A.
    Champion,
    /// The credits' pages (START skips).
    Credits,
    /// THE END: any key.
    TheEnd,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HallOfFame {
    stage: Stage,
    /// Frames left in the stage (`tFrameCount`).
    left: u16,
    /// Rows shown on the list.
    shown: u8,
    msg: Option<Box<Message>>,
    gfx: Gfx<Panel>,
}

impl Default for HallOfFame {
    fn default() -> Self {
        HallOfFame::new()
    }
}

impl HallOfFame {
    pub fn new() -> Self {
        HallOfFame {
            stage: Stage::Saving,
            left: WAIT_TO_DISPLAY,
            shown: 0,
            msg: None,
            gfx: Gfx::none(),
        }
    }

    /// The credits alone (`ScreenKind::Credits`): the black pages, then THE END.
    pub fn credits() -> Self {
        HallOfFame {
            stage: Stage::Credits,
            left: CREDITS_FRAMES,
            ..HallOfFame::new()
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    fn list<G: GameData>(&self, cx: &mut ScreenCx<'_, G>) -> Panel {
        let game = cx.game;
        let (l, t, w, h) = LIST_RECT;
        let mut p = TileAlloc::full().panel(game, l, t, w, h, true);
        for (i, mon) in cx.party.iter().take(self.shown as usize).enumerate() {
            let y = 1 + 16 * i as i16;
            let mut name = Vec::with_capacity(12);
            push(&mut name, &mon.nickname);
            name.push(EOS);
            p.text(game, &name, 0, y);
            let mut lv = alloc::vec![EXTRA_SYMBOL, SYMBOL_LV];
            push_number(&mut lv, u32::from(mon.level), 3, false);
            lv.push(EOS);
            p.text(game, &lv, 96, y);
        }
        if self.stage == Stage::Champion {
            // `HallOfFame_PrintPlayerInfo`: the player's name and play time.
            let mut name = Vec::with_capacity(8);
            push(&mut name, &cx.player_name[..]);
            name.push(EOS);
            let rows = alloc::vec![
                (game.ui_text(UiText::MainPlayer), name),
                (game.ui_text(UiText::MainTime), play_time(cx.frame)),
            ];
            let mut info = Panel::new(l, t, w, h, p.window().base(), true, 0);
            info.clear(game);
            draw_rows(game, &mut info, &rows);
            return info;
        }
        p
    }

    fn redraw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let list = self.list(cx);
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&list);
        if let Some(m) = &mut self.msg
            && m.shown
        {
            m.touch();
            map.put_window(m.window(), true);
        }
        map.flush(cx.f);
        self.gfx.0 = Some(Box::new(list));
    }

    fn message<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, text: Option<UiText>) {
        let (l, t, w, h) = MSG_RECT;
        let base = FULL_BASE + LIST_RECT.2 as u16 * LIST_RECT.3 as u16;
        let m = self.msg.get_or_insert_with(|| {
            Box::new(Message::new(
                Window::new(l, t, w, h, base),
                cx.options.text_speed,
            ))
        });
        match text {
            Some(t) => m.start(
                cx.game,
                cx.game.ui_text(t),
                &cx.player_name[..],
                None,
                false,
            ),
            None => m.hide(),
        }
    }

    fn enter<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, stage: Stage, left: u16) {
        self.stage = stage;
        self.left = left;
        match stage {
            Stage::Mon(i) => {
                self.shown = i + 1;
                self.message(cx, None);
            }
            Stage::Champion => self.message(cx, None),
            _ => {}
        }
        self.gfx.0 = None;
    }
}

impl Screen for HallOfFame {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if self.msg.is_none() {
            self.message(cx, Some(UiText::SaveSaving));
        }
        let black = matches!(self.stage, Stage::Credits | Stage::TheEnd);
        if self.gfx.0.is_none() && !black {
            self.redraw(cx);
        }
        let pressed = input.pressed;
        let mons = cx.party.len().min(6) as u8;
        let mut out = ScreenOut::Running;
        self.left = self.left.saturating_sub(1);
        match self.stage {
            Stage::Saving if self.left == 0 => {
                if mons == 0 {
                    self.enter(cx, Stage::Welcome, WELCOME_FRAMES)
                } else {
                    self.enter(cx, Stage::Mon(0), MON_FRAMES)
                }
            }
            Stage::Mon(i) if self.left == 0 => {
                if i + 1 < mons {
                    self.enter(cx, Stage::Mon(i + 1), MON_FRAMES)
                } else {
                    let total =
                        WELCOME_FRAMES + PLAYER_WAIT_FRAMES + PLAYER_FRAMES + PLAYER_SLIDE_FRAMES;
                    self.enter(cx, Stage::Welcome, total)
                }
            }
            Stage::Welcome if self.left == 0 => self.enter(cx, Stage::Champion, 0),
            Stage::Champion if pressed.contains(Buttons::A) => {
                self.stage = Stage::Credits;
                self.left = CREDITS_FRAMES;
            }
            Stage::Credits if self.left == 0 || pressed.contains(Buttons::START) => {
                self.stage = Stage::TheEnd;
                self.left = THE_END_FRAMES;
            }
            Stage::TheEnd if self.left == 0 || pressed != Buttons::NONE => {
                out = ScreenOut::Done(ScreenResult::Bool(true));
            }
            _ => {}
        }
        if let Some(m) = &mut self.msg {
            m.update(cx.game, pressed, input.held);
        }
        if matches!(self.stage, Stage::Credits | Stage::TheEnd) {
            cx.f.brightness = FADE_FRAMES;
            cx.f.lighten = false;
        } else {
            let mut budget = super::kit::frame_budget(cx.f);
            if let Some(m) = &mut self.msg {
                m.flush_within(cx.f, &mut budget);
            }
            if let Some(p) = self.gfx.0.as_deref_mut() {
                p.flush_within(cx.f, &mut budget);
            }
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        let (tag, i) = match self.stage {
            Stage::Saving => (0, 0),
            Stage::Mon(i) => (1, i),
            Stage::Welcome => (2, 0),
            Stage::Champion => (3, 0),
            Stage::Credits => (4, 0),
            Stage::TheEnd => (5, 0),
        };
        h.write(&[tag, i, self.shown]);
        h.write_u16(self.left);
        if let Some(m) = &self.msg {
            m.hash(h);
        }
    }
}
