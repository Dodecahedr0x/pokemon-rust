//! The naming screen (Gen3 `naming_screen.c`; Milestone 9 Task W, stretch). Wave 0: a stub.

use super::{NamingKind, Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamingScreen {
    pub kind: NamingKind,
}

impl NamingScreen {
    pub fn new(kind: NamingKind) -> Self {
        NamingScreen { kind }
    }
}

impl Screen for NamingScreen {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "naming screen")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
