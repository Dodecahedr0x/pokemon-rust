//! The Pokémon summary (Gen3 `pokemon_summary_screen.c`; Milestone 9 Task PT). Wave 0: a stub.

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, SummaryMode, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Summary {
    pub slot: u8,
    pub mode: SummaryMode,
}

impl Summary {
    pub fn new(slot: u8, mode: SummaryMode) -> Self {
        Summary { slot, mode }
    }
}

impl Screen for Summary {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "summary")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
