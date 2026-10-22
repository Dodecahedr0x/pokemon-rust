//! Per-frame bug detectors (Task E).
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::config::DetectConfig;
use crate::observe::{Flag, StepObserver};
use crate::probe::Probe;
use engine::Buttons;
use engine::data::GameData;
use game_native::Session;

pub struct Detectors<G: GameData> {
    cfg: DetectConfig,
    /// Shadow session after a round trip, and the frames it still follows.
    shadow: Option<(Session<G>, u32)>,
    /// Consecutive idle-but-not-capturable frames.
    idle_not_capturable: u32,
    frames: u32,
}

impl<G: GameData + Clone> Detectors<G> {
    pub fn new(cfg: &DetectConfig) -> Self {
        todo!("Task E")
    }
}

impl<G: GameData + Clone> StepObserver<G> for Detectors<G> {
    /// Round trip, follow, capturable stuck, slow step.
    fn after_step(&mut self, s: &Session<Probe<G>>, keys: Buttons, step_ns: u64) -> Option<Flag> {
        todo!("Task E")
    }
}
