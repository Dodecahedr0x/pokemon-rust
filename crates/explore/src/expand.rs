//! The driver: expands a macro to inputs on a live session (Task A).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::Limits;
use crate::guard::Panicked;
use crate::model::{Macro, SimSummary};
use crate::observe::{Flag, StepObserver};
use crate::policy::Policy;
use crate::probe::Probe;
use engine::Buttons;
use engine::data::GameData;
use game_native::Session;

#[derive(Clone, PartialEq, Debug)]
pub enum ExpandError {
    NoPath,
    NoEffect(&'static str),
    Timeout(&'static str),
    Panicked(Panicked),
    Flagged(Flag),
}

pub struct Expanded {
    pub keys: Vec<Buttons>,
    pub summary: SimSummary,
}

/// Plays `m` on `s` to the next capturable frame. On error, `keys`/`summary` so far are
/// returned for the repro.
#[allow(clippy::result_large_err)] // the partial expansion is the repro
pub fn expand<G: GameData>(
    s: &mut Session<Probe<G>>,
    m: &Macro,
    policy: &mut Policy,
    obs: &mut dyn StepObserver<G>,
    limits: &Limits,
) -> Result<Expanded, (ExpandError, Expanded)> {
    todo!("Task A")
}

/// Root: a new game settled to its first capturable frame (`MacroKind::Settle`).
pub fn new_game<G: GameData>(game: G) -> Session<Probe<G>> {
    todo!("Task A")
}
