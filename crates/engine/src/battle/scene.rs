//! Battle scene: plays the event stream, runs the menus, returns the player's choice.
//! **Task K** (Wave 2).

use super::{Battle, BattleEvent, BattleSetup, Choice};
use crate::data::GameData;
use crate::flow::Input;
use crate::hash::Fnv64;
use crate::video::Frame;
use alloc::vec::Vec;

pub struct BattleScene {
    // Task K.
}

#[allow(unused_variables, clippy::ptr_arg)] // stub: drop once implemented
impl BattleScene {
    /// Task K.
    pub fn new(setup: &BattleSetup) -> Self {
        todo!("Task K: BattleScene::new")
    }

    /// Uploads in budgeted chunks while black; true when everything is loaded. Task K.
    pub fn load(&mut self, game: &impl GameData, battle: &Battle, f: &mut Frame) -> bool {
        todo!("Task K: BattleScene::load")
    }

    /// Queues events (drains `events`). Task K.
    pub fn push(&mut self, events: &mut Vec<BattleEvent>) {
        todo!("Task K: BattleScene::push")
    }

    /// One frame of playback / menus. Task K.
    pub fn update(
        &mut self,
        game: &impl GameData,
        battle: &Battle,
        input: &Input,
        f: &mut Frame,
    ) -> Option<Choice> {
        todo!("Task K: BattleScene::update")
    }

    /// `End` played and its last message dismissed. Task K.
    pub fn finished(&self) -> bool {
        todo!("Task K: BattleScene::finished")
    }

    /// Task K.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        todo!("Task K: BattleScene::hash")
    }
}
