//! Battle and menu policy (Task F). Its keys are intents: the driver applies the tap rule.
#![allow(unused_variables, dead_code)] // Wave 0 stub: remove when implemented.

use crate::model::Opts;
use engine::battle::BattleMon;
use engine::mon::BattleTables;
use engine::{Buttons, World};

pub struct Policy {
    last: Buttons,
}

impl Default for Policy {
    fn default() -> Self {
        Policy::new()
    }
}

impl Policy {
    pub fn new() -> Policy {
        todo!("Task F")
    }

    /// Next intent while a battle owns the frame: first damaging usable move (power > 0, pp > 0;
    /// slot 0 if none), RUN from wild battles when `opts.run_wild` (never the first battle or a
    /// trainer), keep known moves (B on learn prompts), A through messages.
    pub fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons {
        todo!("Task F")
    }

    /// Next intent while a script or the starter screen runs: A through text; yes/no -> A for
    /// choice 0, B otherwise; starter -> move to ball `opts.choice % 3`, then A.
    pub fn script(&mut self, w: &World, opts: Opts) -> Buttons {
        todo!("Task F")
    }
}

pub fn first_damaging_slot(m: &BattleMon, t: &BattleTables) -> u8 {
    todo!("Task F")
}
