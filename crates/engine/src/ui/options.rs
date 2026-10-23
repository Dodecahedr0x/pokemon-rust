//! The options menu (Gen3 `option_menu.c`; Milestone 9 Task SM). Wave 0: a stub. The options
//! themselves are `crate::options::Options`.

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct OptionsMenu {}

impl OptionsMenu {
    pub fn new() -> Self {
        OptionsMenu {}
    }
}

impl Screen for OptionsMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "options menu")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
