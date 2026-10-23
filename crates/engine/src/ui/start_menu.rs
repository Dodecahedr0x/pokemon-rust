//! The field start menu (Gen3 `start_menu.c`; Milestone 9 Task SM, its first cut pulled
//! forward with the party menu).
//!
//! The entries are `GameData::start_menu`, filtered by `shown_if` (Gen3
//! `BuildNormalStartMenu`), in a framed window at (22, 1), 7 tiles wide and two rows per entry
//! (`AddStartMenuWindow`). Up/down move with wrap, A runs the entry, B and START close.
//! POKéMON opens the party menu and BAG the bag (and the menu shows again when they close,
//! Gen3 `CB2_ReturnToFieldWithOpenMenu`); EXIT closes; a game-defined `Script` entry closes the
//! menu and runs its script. Not yet: POKéDEX, the trainer card, SAVE and OPTION report
//! `Unsupported` and keep the menu open.
//!
//! Deviations: the cursor starts on the first entry at every opening (Gen3 keeps
//! `sStartMenuCursorPos` between openings); no help line (Emerald has none either).

use super::kit::Gfx;
use super::kit::list::{MenuCursor, MenuInput};
use super::kit::widgets::{draw_menu, menu_step};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{PartyMode, Screen, ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult};
use crate::data::{GameData, StartAction, StartItem};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::script::Report;
use alloc::vec::Vec;

/// Gen3 `AddStartMenuWindow`: left, top and width of the window.
pub const WINDOW: (u8, u8, u8) = (22, 1, 7);

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct StartMenu {
    cursor: Option<MenuCursor>,
    gfx: Gfx<Panel>,
}

impl StartMenu {
    pub fn new() -> Self {
        StartMenu::default()
    }

    /// The cursor's entry (among the shown entries), once drawn.
    pub fn cursor(&self) -> Option<u8> {
        self.cursor.map(|c| c.pos)
    }
}

/// The entries shown (Gen3 `BuildNormalStartMenu`).
pub fn entries<G: GameData>(cx: &ScreenCx<'_, G>) -> Vec<StartItem> {
    cx.game
        .start_menu()
        .iter()
        .filter(|i| i.shown_if.is_none_or(|f| cx.vars.flag(f)))
        .copied()
        .collect()
}

impl Screen for StartMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let game = cx.game;
        let items = entries(cx);
        let n = items.len().min(9) as u8;
        let cursor = self.cursor.get_or_insert(MenuCursor::new(n, 0));
        cursor.count = n;
        if cursor.pos >= n {
            cursor.pos = 0;
        }
        if self.gfx.0.is_none() {
            let (l, t, w) = WINDOW;
            let mut tiles = TileAlloc::overlay();
            let mut p = tiles.panel(game, l, t, w, n.max(1) * 2, true);
            let labels: Vec<&[u8]> = items.iter().map(|i| game.text(i.label)).collect();
            draw_menu(game, &mut p, &labels, cursor.pos);
            let mut map = MapBuf::overlay(frame_kind(game, cx.options.frame));
            map.put(&p);
            map.flush(cx.f);
            self.gfx.0 = Some(alloc::boxed::Box::new(p));
        }
        let p = self.gfx.0.as_deref_mut().expect("drawn");
        let pressed = input.pressed;
        let out = if pressed.contains(Buttons::START) {
            ScreenOut::Done(ScreenResult::Cancel)
        } else {
            match menu_step(game, p, cursor, pressed) {
                MenuInput::Chosen(i) => match items.get(i as usize).map(|i| i.action) {
                    Some(StartAction::Party) => {
                        self.gfx.0 = None;
                        ScreenOut::Push(ScreenKind::Party(PartyMode::Field))
                    }
                    Some(StartAction::Bag) => {
                        self.gfx.0 = None;
                        ScreenOut::Push(ScreenKind::Bag(super::BagMode::Field))
                    }
                    Some(StartAction::Exit) | None => ScreenOut::Done(ScreenResult::Cancel),
                    Some(StartAction::Script(id)) => ScreenOut::RunScript(id),
                    Some(a) => {
                        cx.report(Report::Unsupported(match a {
                            StartAction::Pokedex => "pokedex",
                            StartAction::PlayerCard => "trainer card",
                            StartAction::Save => "save dialog",
                            _ => "options",
                        }));
                        ScreenOut::Running
                    }
                },
                MenuInput::Cancel => ScreenOut::Done(ScreenResult::Cancel),
                _ => ScreenOut::Running,
            }
        };
        if let Some(p) = self.gfx.0.as_deref_mut() {
            p.flush_within(cx.f, &mut super::kit::frame_budget(cx.f));
        }
        out
    }

    fn hash(&self, h: &mut Fnv64) {
        match &self.cursor {
            None => h.write(&[0xFF]),
            Some(c) => c.hash(h),
        }
    }
}
