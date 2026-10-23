//! The Poké Mart (Gen3 `shop.c`; Milestone 9 Task M). Wave 0: a stub.

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::{GameData, ItemId};
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mart {
    pub items: &'static [ItemId],
}

impl Mart {
    pub fn new(items: &'static [ItemId]) -> Self {
        Mart { items }
    }
}

impl Screen for Mart {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "pokemart")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
