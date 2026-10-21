//! The turn engine and damage pipeline. **Task D.**
//!
//! Gen3 references: `battle_main.c` (`GetWhoStrikesFirst`, `SetActionsAndBattlersTurnOrder`,
//! `HandleEndTurn_*`), `battle_script_commands.c`, `battle_util.c`,
//! `data/battle_scripts_1.s`. Pure and video-free: takes `Choice`s, pushes `BattleEvent`s, and
//! draws every random roll from the `Rng` it is handed.

use super::{BattleEvent, BattleMon, BattleSetup, Choice, Need, Side};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::mon::Party;
use crate::rng::Rng;
use alloc::vec::Vec;

/// A running battle. Core-private per-battler state (stat stages, the STATUS2/3 subset, Bide,
/// Leech Seed, Focus Energy, confusion, flinch, last move) lives here next to the public
/// [`BattleMon`]s. Task D.
pub struct Battle {
    // Task D.
}

#[allow(unused_variables, clippy::ptr_arg)] // stub: drop once implemented
impl Battle {
    /// Task D.
    pub fn new(game: &impl GameData, setup: BattleSetup, player: &Party, rng: &mut Rng) -> Self {
        todo!("Task D: Battle::new")
    }

    /// Pushes the intro events (wild or trainer). Task D.
    pub fn start(&mut self, game: &impl GameData, rng: &mut Rng, out: &mut Vec<BattleEvent>) {
        todo!("Task D: Battle::start")
    }

    /// Task D.
    pub fn need(&self) -> Need {
        todo!("Task D: Battle::need")
    }

    /// Answers the current `Need` and runs until the next one. Task D.
    pub fn choose(
        &mut self,
        game: &impl GameData,
        rng: &mut Rng,
        c: Choice,
        out: &mut Vec<BattleEvent>,
    ) {
        todo!("Task D: Battle::choose")
    }

    /// Task D.
    pub fn mon(&self, side: Side) -> &BattleMon {
        todo!("Task D: Battle::mon")
    }

    /// Player: written back to the World at the end. Task D.
    pub fn party(&self, side: Side) -> &Party {
        todo!("Task D: Battle::party")
    }

    /// Task D.
    pub fn setup(&self) -> &BattleSetup {
        todo!("Task D: Battle::setup")
    }

    /// Task D.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        todo!("Task D: Battle::hash")
    }
}
