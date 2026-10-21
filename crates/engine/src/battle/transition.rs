//! Field-to-battle transition. **Task G.**
//!
//! Intro flashes (`TransitionIntro_FadeToGray` / `FadeFromGray`, twice, with `Frame::lighten`),
//! then a tile-based pattern on BG0: `GRID_SQUARES` for wild battles, a horizontal-bars sweep
//! for trainers. Ends fully black (`brightness = BRIGHTNESS_BLACK`). It must not touch field
//! VRAM before the screen is black, except BG0 and BG palette bank 15.

use super::BattleKind;
use crate::hash::Fnv64;
use crate::video::Frame;

pub struct BattleTransition {
    // Task G.
}

#[allow(unused_variables)]
impl BattleTransition {
    /// Picks the pattern from the battle kind (wild: grid squares; trainer: bars). Task G.
    pub fn new(kind: &BattleKind) -> Self {
        todo!("Task G: BattleTransition::new")
    }

    /// One frame; true once the screen is fully black. Task G.
    pub fn update(&mut self, f: &mut Frame) -> bool {
        todo!("Task G: BattleTransition::update")
    }

    /// Task G.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        todo!("Task G: BattleTransition::hash")
    }
}
