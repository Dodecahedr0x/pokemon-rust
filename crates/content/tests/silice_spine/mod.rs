//! Silice's spine: the authored main path through the chapters, as ordered high-level steps
//! ([`Step`]) that a runner expands deterministically into buttons. The end-to-end tests
//! (`sil1_f_ch0.rs`, `sil2_ch1.rs`, `sil2_ch2.rs`) play them with [`driver::Run`]; the
//! explorer's Silice spine can import the same lists ([`ch00::steps`], [`ch01::steps`],
//! [`ch02::steps`]) and map each step onto its own. A chapter can also start from its documented
//! start state ([`driver::Start`], e.g. [`ch02::start`]) instead of the chapters before it.
//!
//! **The vocabulary** (minimal on purpose):
//! - [`Step::Goto`]: walk to a cell of the current map (asserted), on a shortest path that avoids
//!   the `avoid` cells (trainers' sight lines, spore grates) and, when it can, tall grass, on the
//!   field as it is (cells a script repainted with `setmetatile`, such as an opened door, count).
//! - [`Step::Enter`]: hold a direction until the map changes (a door, a mat, a connection), and
//!   assert the new map.
//! - [`Step::Interact`]: face a direction and press A (talk, read, pick up).
//! - [`Step::Walk`]: one step (a bump into a scripted cell, a coord event stepped on).
//! - [`Step::Choose`]: the index the next choice takes (the starter screen's ball); a player
//!   decision, so it is spine data.
//! - [`Step::Train`]: pace between two tall-grass cells until the lead Pokémon reaches a level,
//!   playing the `heal` steps whenever it is below half its HP or has a status (the training a
//!   player does between two story beats; the explorer's grind goal).
//! - [`Step::Settle`]: wait for the field to be free.
//! - [`Step::Hook`]: a test-only grant around one script's next run ([`Hook`]); the explorer
//!   leaves these out, or replays them as its own setup. A spine holds one only where a
//!   buttons-only run cannot do without it (documented at the hook: [`ch01::SECOND`]); probes
//!   that need one are separate lists ([`ch00::labo_brambles_probe`]).
//! - [`Step::Check`]: a labelled checkpoint with its assertions; a natural place for the
//!   explorer's `Checkpoint(label)`.
//!
//! Every step first settles: scripts run to their end, messages advance, yes/no boxes answer
//! yes, the starter screen takes the [`Step::Choose`] ball, and battles fight (the explorer's
//! `WaitScript`, `Choose` and `Battle(policy)` are this settle policy, applied everywhere). The
//! battle policy ([`driver::best_move`]): FIGHT, never RUN; the usable move with the best
//! expected damage against the opponent's types (power, accuracy, same-type bonus, the type
//! chart); new moves are learned into free slots only (B on "forget a move?"). Nothing is random
//! outside the engine: the same steps give the same frames.

#![allow(dead_code)]

pub mod ch00;
pub mod ch01;
pub mod ch02;
pub mod driver;
// Chapter 4 (SIL3), and the chapter-entry runs it needs (`Run::at_chapter_entry`).
pub mod ch04;
pub mod entry;

use engine::data::{Dir, FlagId, MapId, SpeciesId};

pub use driver::{Run, Start};

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
    Choose(u8),
    Train {
        a: (i16, i16),
        b: (i16, i16),
        level: u8,
        heal: &'static [Step],
    },
    Settle,
    Hook(Hook),
    Check(&'static str, fn(&Run)),
}

/// A test-only grant, applied around the next run of the named script.
#[derive(Clone, Copy)]
pub enum Hook {
    /// The flag is set before the script runs and cleared after it (a later chapter's unlock,
    /// tried early). The script must run to its end: a warp drops it before the clear.
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
