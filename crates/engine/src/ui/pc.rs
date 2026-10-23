//! The PC menus: item storage and the Pokémon storage list (Gen3 `player_pc.c`,
//! `pokemon_storage_system.c`; Milestone 9 Task M). Wave 0: a stub. The storage itself is
//! `crate::pc`.

use super::{PcMode, Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::GameData;
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PcMenu {
    pub mode: PcMode,
}

impl PcMenu {
    pub fn new(mode: PcMode) -> Self {
        PcMenu { mode }
    }
}

impl Screen for PcMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "pc")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
