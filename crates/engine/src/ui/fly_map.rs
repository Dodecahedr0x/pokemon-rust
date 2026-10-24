//! Fly's destination picker (Gen3 `region_map.c`: `CB2_OpenFlyMap`, `CB_HandleFlyMapInput`,
//! `CB_ExitFlyMap`).
//!
//! The destinations are `GameData::fly_destination` in order, those whose `visited` flag is
//! set (Gen3 `GetMapsecType`: `MAPSECTYPE_*_CANFLY`). Up/down move through them with wrap, A
//! flies to the one shown ([`ScreenOut::Fly`] with its `fly_destination` index: the field
//! closes the screens and warps, Gen3 `SetFlyDestination` → `ReturnToFieldFromFlyMapSelect`),
//! B goes back to the party menu (`CB2_ReturnToPartyMenuFromFlyMap`).
//!
//! Deviations (degraded): no region map, cursor sprite or section highlights: one framed
//! window shows the name of the destination under the cursor. With no visited destination
//! the screen closes at once.

use super::kit::Gfx;
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;
use alloc::vec::Vec;

/// The name window: left, top, width, height (tiles).
pub const WINDOW: (u8, u8, u8, u8) = (1, 1, 18, 2);

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FlyMap {
    /// Index into the visited destinations.
    cursor: u8,
    gfx: Gfx<Panel>,
}

/// The `fly_destination` indices the picker lists (visited ones, in order).
pub fn listed<G: GameData>(cx: &ScreenCx<'_, G>) -> Vec<u8> {
    (0..=u8::MAX)
        .map_while(|i| cx.game.fly_destination(i).map(|d| (i, d)))
        .filter(|(_, d)| cx.vars.flag(d.visited))
        .map(|(i, _)| i)
        .collect()
}

impl FlyMap {
    pub fn new() -> Self {
        FlyMap::default()
    }

    /// The listed position under the cursor.
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, dest: u8) {
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
        if let Some(d) = game.fly_destination(dest) {
            p.text(game, d.name, 0, 1);
        }
        p.flush(cx.f);
    }
}

impl Screen for FlyMap {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let list = listed(cx);
        let n = list.len() as u8;
        if n == 0 {
            return ScreenOut::Done(ScreenResult::Cancel);
        }
        if self.cursor >= n {
            self.cursor = 0;
        }
        let pressed = input.pressed;
        let first = self.gfx.0.is_none();
        if pressed.contains(Buttons::A) {
            return ScreenOut::Fly(list[self.cursor as usize]);
        }
        if pressed.contains(Buttons::B) {
            return ScreenOut::Done(ScreenResult::Cancel);
        }
        let moved = if pressed.contains(Buttons::DOWN) {
            self.cursor = (self.cursor + 1) % n;
            true
        } else if pressed.contains(Buttons::UP) {
            self.cursor = (self.cursor + n - 1) % n;
            true
        } else {
            false
        };
        if first || moved {
            self.draw(cx, list[self.cursor as usize]);
        }
        ScreenOut::Running
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.cursor]);
    }
}
