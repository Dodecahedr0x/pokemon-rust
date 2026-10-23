//! Silice's spine: the authored main path through the chapters, as ordered high-level steps
//! ([`Step`]) that a runner expands deterministically into buttons. The end-to-end tests
//! (`sil1_f_ch0.rs`, `sil2_ch1.rs`) play them with [`driver::Run`]; the explorer's Silice spine
//! can import the same lists ([`ch00::steps`], [`ch01::steps`]) and map each step onto its own.
//!
//! **The vocabulary** (minimal on purpose):
//! - [`Step::Goto`]: walk to a cell of the current map (asserted), on a shortest path that avoids
//!   the `avoid` cells (trainers' sight lines, spore grates) and, when it can, tall grass. Cells
//!   a script opened (`setmetatile`) are not planned through: [`Step::Walk`] crosses them.
//! - [`Step::Enter`]: hold a direction until the map changes (a door, a mat, a connection), and
//!   assert the new map.
//! - [`Step::Interact`]: face a direction and press A (talk, read, pick up).
//! - [`Step::Walk`]: one step (a bump into a scripted cell, or across an opened cell).
//! - [`Step::Settle`]: wait for the field to be free.
//! - [`Step::Hook`]: a test-only grant around one script's next run ([`Hook`]); the explorer
//!   leaves these out, or replays them as its own setup.
//! - [`Step::Check`]: a labelled checkpoint with its assertions; a natural place for the
//!   explorer's `Checkpoint(label)`.
//!
//! Every step first settles: scripts run to their end, messages advance, yes/no boxes answer
//! yes, the starter screen takes the right ball, and battles use the first move (the explorer's
//! `WaitScript`, `Choose` and `Battle(policy)` are this settle policy, applied everywhere).

#![allow(dead_code)]

pub mod ch00;
pub mod ch01;
pub mod driver;

use engine::data::{Dir, FlagId, MapId, SpeciesId};

pub use driver::Run;

/// One step of the spine (see the module docs).
#[derive(Clone, Copy)]
pub enum Step {
    Goto {
        map: MapId,
        x: i16,
        y: i16,
        avoid: &'static [(i16, i16)],
    },
    Enter {
        dir: Dir,
        map: MapId,
    },
    Interact(Dir),
    Walk(Dir),
    Settle,
    Hook(Hook),
    Check(&'static str, fn(&Run)),
}

/// A test-only grant, applied around the next run of the named script.
#[derive(Clone, Copy)]
pub enum Hook {
    /// The flag is set before the script runs and cleared after it (a later chapter's unlock,
    /// tried early).
    FlagDuring(&'static str, FlagId),
    /// A Pokémon of that species and level joins the party before the script runs.
    MonBefore(&'static str, SpeciesId, u8),
}

/// [`Step::Goto`] with nothing to avoid.
pub const fn goto(map: MapId, x: i16, y: i16) -> Step {
    Step::Goto {
        map,
        x,
        y,
        avoid: &[],
    }
}

/// [`Step::Goto`] avoiding `avoid`.
pub const fn goto_avoiding(map: MapId, x: i16, y: i16, avoid: &'static [(i16, i16)]) -> Step {
    Step::Goto { map, x, y, avoid }
}
