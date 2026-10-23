//! The party menu (Gen3 `party_menu.c`; Milestone 9 Task PT). Wave 0: a stub.

use super::{PartyMode, Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartyMenu {
    pub mode: PartyMode,
}

impl PartyMenu {
    pub fn new(mode: PartyMode) -> Self {
        PartyMenu { mode }
    }
}

impl Screen for PartyMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "party menu")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
