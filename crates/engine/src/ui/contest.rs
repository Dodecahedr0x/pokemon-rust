//! The contest screens (`startcontest`, `showcontestresults`, `showcontestpainting`).
//!
//! Judging finished before the screen opened. Appeals, results, and the painting each wait
//! until A or B. The appeal scene is the five rounds' outcome, not the per-effect animations.

use super::kit::Gfx;
use super::kit::frame_budget;
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::contest::{self, ContestPage, ContestView};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;
use alloc::vec::Vec;

/// One contest page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContestScreen {
    view: ContestView,
    gfx: Gfx<Panel>,
}

impl ContestScreen {
    pub fn new(view: ContestView) -> Self {
        Self {
            view,
            gfx: Gfx::none(),
        }
    }

    fn redraw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let mut tiles = TileAlloc::full();
        let mut panel = tiles.panel(cx.game, 1, 1, 26, 12, true);
        let title = match self.view.page {
            ContestPage::Appeals => "APPEALS",
            ContestPage::Results => "RESULTS",
            ContestPage::Painting(_) => "PAINTING",
        };
        panel.text(cx.game, &gen3(title), 8, 0);
        panel.text(cx.game, contest::name(self.view.category), 8, 16);
        for place in 0..contest::COUNT {
            let who = self
                .view
                .place
                .iter()
                .position(|&p| p == place as u8)
                .unwrap_or(place);
            let mut line = alloc::vec![0xA1 + place as u8 + 1, 0xFF];
            if who as u8 == self.view.player {
                line.pop();
                line.extend(gen3(" YOU"));
            }
            panel.text(cx.game, &line, 8, 32 + place as i16 * 16);
        }
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&panel);
        map.flush(cx.f);
        self.gfx.0 = Some(alloc::boxed::Box::new(panel));
    }
}

impl Screen for ContestScreen {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        if self.gfx.0.is_none() {
            self.redraw(cx);
        }
        if let Some(panel) = self.gfx.0.as_mut() {
            let mut budget = frame_budget(cx.f);
            panel.flush_within(cx.f, &mut budget);
        }
        if input.pressed.contains(Buttons::A) || input.pressed.contains(Buttons::B) {
            ScreenOut::Done(ScreenResult::Value(0))
        } else {
            ScreenOut::Running
        }
    }

    fn hash(&self, h: &mut Fnv64) {
        let page = match self.view.page {
            ContestPage::Appeals => 0,
            ContestPage::Results => 1,
            ContestPage::Painting(id) => 2 + id,
        };
        h.write(&[page, self.view.player, self.view.category]);
        h.write(&self.view.place);
    }
}

fn gen3(s: &str) -> Vec<u8> {
    let mut o: Vec<u8> = s
        .bytes()
        .map(|c| match c {
            b' ' => 0,
            b'0'..=b'9' => 0xA1 + (c - b'0'),
            b'A'..=b'Z' => 0xBB + (c - b'A'),
            _ => 0,
        })
        .collect();
    o.push(0xFF);
    o
}
