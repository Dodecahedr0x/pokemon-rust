//! The boot main menu: CONTINUE, NEW GAME, OPTION (Gen3 `main_menu.c`, `HAS_SAVED_GAME`;
//! Milestone 9 Task S). Power-on with a save opens the title (`ui/title.rs`, `World::main_menu`);
//! Start there opens this menu, and the shells swap the World for the choice
//! (`World::main_menu_choice`).
//!
//! - Windows (`sWindowTemplates_MainMenu`): CONTINUE at (2, 1), 26 x 6 tiles, with the save's
//!   player (name right-aligned at 100), time (from 0x6C, right-aligned at 0xD0), Pokédex
//!   (caught, right-aligned at 100) and badges (from 0x6C, at 0xD0)
//!   (`MainMenu_FormatSavegameText`); NEW GAME at (2, 9) and OPTION at (2, 13), 26 x 2.
//! - Up and down move without wrap (`HandleMainMenuInput`); A chooses: CONTINUE and NEW GAME
//!   close the menu with [`CONTINUE`] or [`NEW_GAME`], OPTION opens the options and comes back.
//!   B returns to the title (`Task_HandleMainMenuInput`).
//!
//! Deviations: the selection is a `▶` cursor before the entry rather than Gen3's highlighted
//! window (`HighlightSelectedMainMenuItem`); the Pokédex row shows when anything was caught
//! (the save's metadata has no `FLAG_SYS_POKEDEX_GET`); no colours.

use super::kit::Gfx;
use super::kit::widgets::{CHAR_CURSOR, CURSOR_W, EOS, push, push_number};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::save::play_time;
use super::{Screen, ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult};
use crate::checkpoint::SaveMeta;
use crate::data::{GameData, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// The answers: `Value(CONTINUE)` and `Value(NEW_GAME)`.
pub const CONTINUE: u16 = 0;
pub const NEW_GAME: u16 = 1;
/// The third entry (opens the options).
pub const OPTION: u8 = 2;
/// `sWindowTemplates_MainMenu[2..=4]` (with a save): left, top, width, height.
pub const RECTS: [(u8, u8, u8, u8); 3] = [(2, 1, 26, 6), (2, 9, 26, 2), (2, 13, 26, 2)];

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MainMenu {
    cursor: u8,
    meta: Option<SaveMeta>,
    /// OPTION is open over the menu.
    child: bool,
    gfx: Gfx<[Panel; 3]>,
}

fn right(game: &impl GameData, text: &[u8], edge: i16) -> i16 {
    let w = game.font().widths;
    edge - (text.iter().take_while(|&&c| c != EOS))
        .map(|&c| w.get(c as usize).copied().unwrap_or(0) as i16)
        .sum::<i16>()
}

impl MainMenu {
    pub fn new() -> Self {
        MainMenu::default()
    }

    /// The save the CONTINUE window describes.
    pub fn set_meta(&mut self, meta: SaveMeta) {
        self.meta = Some(meta);
        self.gfx.0 = None;
    }

    /// The selected entry (0 CONTINUE, 1 NEW GAME, 2 OPTION).
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    fn draw<G: GameData>(&self, cx: &mut ScreenCx<'_, G>) -> [Panel; 3] {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let mut panels = RECTS.map(|(l, t, w, h)| tiles.panel(game, l, t, w, h, true));
        let labels = [
            UiText::MainContinue,
            UiText::MainNewGame,
            UiText::MainOption,
        ];
        for (i, p) in panels.iter_mut().enumerate() {
            if i as u8 == self.cursor {
                p.text(game, &[CHAR_CURSOR, EOS], 0, 1);
            }
            p.text(game, game.ui_text(labels[i]), CURSOR_W, 1);
        }
        if let Some(m) = &self.meta {
            let p = &mut panels[0];
            let num = |n: u32| {
                let mut v = Vec::with_capacity(4);
                push_number(&mut v, n, 3, false);
                v.push(EOS);
                v
            };
            let mut name = Vec::with_capacity(8);
            push(&mut name, &m.name);
            name.push(EOS);
            p.text(game, game.ui_text(UiText::MainPlayer), 0, 17);
            p.text(game, &name, right(game, &name, 100), 17);
            p.text(game, game.ui_text(UiText::MainTime), 0x6C, 17);
            let t = play_time(m.frame);
            p.text(game, &t, right(game, &t, 0xD0), 17);
            if m.dex_caught > 0 {
                let d = num(u32::from(m.dex_caught));
                p.text(game, game.ui_text(UiText::MainPokedex), 0, 33);
                p.text(game, &d, right(game, &d, 100), 33);
            }
            let b = num(u32::from(m.badges));
            p.text(game, game.ui_text(UiText::MainBadges), 0x6C, 33);
            p.text(game, &b, right(game, &b, 0xD0), 33);
        }
        let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
        for p in &panels {
            map.put(p);
        }
        map.flush(cx.f);
        panels
    }
}

impl Screen for MainMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if cx.child.take().is_some() {
            self.child = false;
            self.gfx.0 = None;
        }
        if self.child {
            return ScreenOut::Running;
        }
        if self.gfx.0.is_none() {
            self.gfx.0 = Some(Box::new(self.draw(cx)));
        }
        let pressed = input.pressed;
        let to = if pressed.contains(Buttons::UP) && self.cursor > 0 {
            self.cursor - 1
        } else if pressed.contains(Buttons::DOWN) && self.cursor < OPTION {
            self.cursor + 1
        } else {
            self.cursor
        };
        let out = if pressed.contains(Buttons::A) {
            match self.cursor {
                OPTION => {
                    self.child = true;
                    self.gfx.0 = None;
                    ScreenOut::Push(ScreenKind::Options)
                }
                c => ScreenOut::Done(ScreenResult::Value(u16::from(c))),
            }
        } else if pressed.contains(Buttons::B) {
            ScreenOut::Done(ScreenResult::Cancel)
        } else {
            if to != self.cursor {
                self.cursor = to;
                self.gfx.0 = Some(Box::new(self.draw(cx)));
            }
            ScreenOut::Running
        };
        if let Some(panels) = self.gfx.0.as_deref_mut() {
            let mut budget = super::kit::frame_budget(cx.f);
            for p in panels.iter_mut() {
                p.flush_within(cx.f, &mut budget);
            }
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.cursor, self.child as u8, self.meta.is_some() as u8]);
    }
}
