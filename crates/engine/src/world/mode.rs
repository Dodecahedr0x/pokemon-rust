//! What owns the screen (moved out of `world/battle.rs` by Milestone 9 Wave 0, which added
//! [`Mode::Screens`]). Frozen after Wave 0.

use super::battle::{BattleRun, StarterRun};
use crate::hash::Fnv64;
use crate::ui::ScreenStack;
use alloc::boxed::Box;

/// What owns the screen.
pub(crate) enum Mode {
    /// The overworld (field, objects, text box, scripts).
    Field,
    /// `special ChooseStarter`, until a starter is confirmed.
    Starter(Box<StarterRun>),
    Battle(Box<BattleRun>),
    /// Milestone 9: one or more screens over (or instead of) the field, hosted by
    /// `world/screens.rs`. Never empty: closing the last screen returns to `Field`.
    Screens(Box<ScreenStack>),
}

impl Mode {
    pub(super) fn hash(&self, h: &mut Fnv64) {
        match self {
            Mode::Field => h.write(&[0]),
            Mode::Starter(run) => {
                h.write(&[1]);
                run.choice.hash(h);
                match &run.leaving {
                    None => h.write(&[0xFF]),
                    Some(fader) => fader.hash(h),
                }
            }
            Mode::Battle(run) => {
                h.write(&[2]);
                run.hash(h);
            }
            Mode::Screens(stack) => {
                h.write(&[3]);
                stack.hash(h);
            }
        }
    }
}
