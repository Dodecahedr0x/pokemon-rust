//! The field start menu (Gen3 `start_menu.c`; Milestone 9 Task SM). Wave 0: a stub.

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct StartMenu {}

impl StartMenu {
    pub fn new() -> Self {
        StartMenu {}
    }
}

impl Screen for StartMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "start menu")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
