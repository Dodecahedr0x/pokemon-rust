//! The evolution scene (Gen3 `evolution_scene.c`, simplified; Milestone 9 Task E). Wave 0: a
//! stub (answers "not evolved").

use super::{Screen, ScreenCx, ScreenInput, ScreenOut, stub_step};
use crate::data::{GameData, SpeciesId};
use crate::hash::Fnv64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EvolutionScene {
    pub slot: u8,
    pub into: SpeciesId,
    pub can_cancel: bool,
}

impl EvolutionScene {
    pub fn new(slot: u8, into: SpeciesId, can_cancel: bool) -> Self {
        EvolutionScene {
            slot,
            into,
            can_cancel,
        }
    }
}

impl Screen for EvolutionScene {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let _ = input;
        stub_step(cx, "evolution scene")
    }

    fn hash(&self, h: &mut Fnv64) {
        let _ = h;
    }
}
