//! The bag (Gen3 `item_menu.c`; Milestone 9 Task B). Wave 0: a stub.

use super::{BagMode, Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BagMenu {
    pub mode: BagMode,
}

impl BagMenu {
    pub fn new(mode: BagMode) -> Self {
        BagMenu { mode }
    }
}

impl Screen for BagMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "bag")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
