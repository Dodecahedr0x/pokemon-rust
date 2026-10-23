//! The boot main menu: CONTINUE, NEW GAME, OPTION (Gen3 `main_menu.c`; Milestone 9 Task S).
//! Wave 0: a stub. The shells run it at boot when a save exists.

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MainMenu {}

impl MainMenu {
    pub fn new() -> Self {
        MainMenu {}
    }
}

impl Screen for MainMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "main menu")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
