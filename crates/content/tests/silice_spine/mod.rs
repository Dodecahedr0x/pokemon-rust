//! Silice's spine: the authored main path through the chapters, as ordered high-level steps
//! ([`Step`]) that a runner expands deterministically into buttons. The end-to-end tests
//! (`sil1_f_ch0.rs`, `sil2_ch1.rs`, `sil2_ch2.rs`, `sil6_ch5.rs`) play them with
//! [`driver::Run`]; the explorer's Silice spine can import the same lists ([`ch00::steps`],
//! [`ch01::steps`], [`ch02::steps`]) and map each step onto its own. A chapter can also start from its documented
//! start state ([`driver::Start`], e.g. [`ch02::start`]) instead of the chapters before it;
//! Chapter 4 (`sil5_ch4.rs`, [`ch04::steps`]) only does so for now ([`ch04::start`]), as do
//! Chapter 5 (`sil6_ch5.rs`), Chapter 6 (`sil7_ch6.rs`, [`ch06::start`]), Chapter 7
//! (`sil8_ch7.rs`, [`ch07::start`]), Chapter 8 (`sil9_ch8.rs`, [`ch08::start`] and
//! [`ch08::route_start`]) and Chapter 9 (`sil10_ch9.rs`, [`ch09::start`]).
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
//!   player does between two story beats; the explorer's grind goal). `SPINE_LEVEL_<l>=<n>`
//!   trains a `level: l` step to `n` instead (a tuning aid for the neighbour rule's measures).
//! - [`Step::Settle`]: wait for the field to be free.
//! - [`Step::Swap`]: swap two party slots in the party menu (START → POKéMON → SWITCH): who
//!   leads a battle (two lead a double) and whom [`Step::Train`] raises.
//! - [`Step::Teach`]: teach a TM from the bag (`World::use_item` stands for the bag's USE, as in
//!   the explorer; then the party menu and the summary's move pick by buttons).
//! - [`Step::Hook`]: a test-only grant around one script's next run ([`Hook`]); the explorer
//!   leaves these out, or replays them as its own setup. A spine holds one only where a
//!   buttons-only run cannot do without it (documented at the hook: [`ch01::SECOND`]); probes
//!   that need one are separate lists ([`ch00::labo_brambles_probe`]).
//! - [`Step::Check`]: a labelled checkpoint with its assertions; a natural place for the
//!   explorer's `Checkpoint(label)`.
//!
//! Every step first settles: scripts run to their end, messages advance, yes/no boxes answer
//! yes, the starter screen takes the [`Step::Choose`] ball, and battles fight (the explorer's
//! `WaitScript`, `Choose` and `Battle(policy)` are this settle policy, applied everywhere).
//!
//! **The battle policy** (deterministic, from the battle's state only):
//! - FIGHT, never RUN, no bag: the usable move with the best expected damage against the
//!   opponent's types ([`driver::best_move`]: power × accuracy, ×1.5 for the same type, × Gen3's
//!   type chart; the lowest slot on a tie). In a double battle, for whichever of the player's
//!   two battlers chooses, against the standing foe with the least HP (the target cursor is
//!   moved onto it), and never a move that also hits the standing ally (Magnitude) while
//!   another is usable (the explorer's doubles policy).
//! - A new move with a free slot is learned (the engine's own). With four moves known, "forget a
//!   move?" ([`driver::learn_slot`]): the weakest damaging move (power × accuracy, ×1.5 for the
//!   same type; the lowest slot on a tie) is forgotten for the new one when the new one is worth
//!   more; otherwise B (keep the moves). Moves without power (Leer, Calm Mind) are never
//!   forgotten nor learned this way. No move is reserved for the scripts: Silice's field
//!   abilities (Cut, Strength, Flash) are the Mallette's modules, flags, not moves.
//! - No voluntary switch: the battle scene has no party screen (POKéMON redraws the action
//!   menu), and after a faint the engine sends out the first usable Pokémon.
//!
//! **The planner** ([`Step::Goto`]) walks as the engine does: collision, behaviours, ledges,
//! objects and elevations (Gen3 `IsElevationMismatchAt`: on foot, water is elevation 1 under
//! the land's 3, so it is never planned through; surfing, water is planned through and a step
//! ashore dismounts, while Surf itself is an A press, [`Step::Interact`], as Dive is on deep
//! water; a bridge's 15 keeps the elevation) and
//! forced movement (ice, walk and slide arrows, the muddy slope, currents carry the player on
//! until the next cell is blocked: a step is planned to where the player comes to rest). A walk
//! that keeps moving without arriving fails instead of eating the frames.
//!
//! Nothing is random outside the engine: the same steps give the same frames. `SPINE_DEBUG=1`
//! logs every battle's outcome and both parties (for tuning training levels and hooks).

#![allow(dead_code)]

pub mod ch00;
pub mod ch01;
pub mod ch02;
pub mod ch03;
pub mod ch04;
pub mod ch05;
pub mod ch06;
pub mod ch07;
pub mod ch08;
pub mod ch09;
pub mod driver;

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
    /// Party slots `a` and `b` swap places (START → POKéMON → slot `a` → SWITCH → slot `b`):
    /// the player's lead order, before a battle or a [`Step::Train`] of another member.
    Swap(u8, u8),
    /// A TM or HM taught from the bag to party slot `mon`, forgetting move slot `forget` when
    /// its four are full ([`TeachTm`]).
    Teach(TeachTm),
    Hook(Hook),
    Check(&'static str, fn(&Run)),
}

/// [`Step::Teach`]: the bag's USE on TM or HM `item` (`ItemId`), on party slot `mon`; "forget a
/// move?" yes and move slot `forget` when the four are full (the explorer's `UseItem`).
#[derive(Clone, Copy)]
pub struct TeachTm {
    pub item: u16,
    pub mon: u8,
    pub forget: u8,
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
