//! Starter choice screen, as Gen3 `starter_choose.c`. **Task G.**
//!
//! Bag background, three Poké Ball OBJs, a cursor; A shows the circle with the mon's front pic
//! and a yes/no; B or no returns to the choice; it fades to black when chosen.

use crate::data::{GameData, SpeciesId};
use crate::flow::Input;
use crate::hash::Fnv64;
use crate::video::Frame;

pub struct StarterChoice {
    // Task G.
}

#[allow(unused_variables)]
impl StarterChoice {
    /// Task G.
    pub fn new(choices: [SpeciesId; 3]) -> Self {
        todo!("Task G: StarterChoice::new")
    }

    /// Uploads one budgeted chunk while black; true when everything is loaded. Task G.
    pub fn load_step(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        todo!("Task G: StarterChoice::load_step")
    }

    /// One frame; `Some(index)` once a starter is confirmed and the screen is black. Task G.
    pub fn update(&mut self, game: &impl GameData, input: &Input, f: &mut Frame) -> Option<u8> {
        todo!("Task G: StarterChoice::update")
    }

    /// Task G.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        todo!("Task G: StarterChoice::hash")
    }
}
