//! The save dialog (Gen3 `start_menu.c` `SaveGame` tasks; Milestone 9 Task S). Wave 0: a stub
//! (answers "not saved").

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SaveDialog {}

impl SaveDialog {
    pub fn new() -> Self {
        SaveDialog {}
    }
}

impl Screen for SaveDialog {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "save dialog")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
