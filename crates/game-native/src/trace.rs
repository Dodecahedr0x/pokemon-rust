//! Replay traces: the parent checkpoint, RLE input frames, the build and periodic
//! `World::hash`es (Milestone 5 Task D).

use engine::checkpoint::BuildHash;
use engine::data::GameData;
use engine::video::Frame;
use engine::{Buttons, World};
use serde::{Deserialize, Serialize};

pub const TRACE_SCHEMA: u16 = 1;
/// A trace records `World::hash` every this many frames (and at its end).
pub const HASH_EVERY: u32 = 60;

/// A node of a checkpoint store (files: `c00042`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct CheckpointId(pub u32);

/// Where a trace starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Parent {
    /// `World::new(&game)`.
    NewGame,
    Checkpoint(CheckpointId),
}

/// `frames` consecutive frames with `keys` held.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Run {
    pub keys: Buttons,
    pub frames: u32,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub schema: u16,
    pub parent: Parent,
    pub build: BuildHash,
    /// The parent's `World::frame` (0 for `NewGame`).
    pub start_frame: u32,
    /// RLE; adjacent runs differ.
    pub inputs: Vec<Run>,
    /// (frames since start, `World::hash`) every `HASH_EVERY` frames and at the end.
    pub hashes: Vec<(u32, u64)>,
}

impl Trace {
    /// Frames the trace steps.
    pub fn frames(&self) -> u32 {
        todo!("Milestone 5 Task D")
    }

    /// The keys of every frame, in order.
    #[allow(unreachable_code)]
    pub fn keys(&self) -> impl Iterator<Item = Buttons> + '_ {
        todo!("Milestone 5 Task D");
        core::iter::empty()
    }
}

/// Records a trace while a session plays.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraceRecorder {
    start_frame: u32,
    keys: Vec<Buttons>,
    hashes: Vec<(u32, u64)>,
}

impl TraceRecorder {
    pub fn new(start_frame: u32) -> Self {
        let _ = start_frame;
        todo!("Milestone 5 Task D")
    }

    /// One stepped frame: its keys and `World::hash` after the step.
    pub fn push(&mut self, keys: Buttons, hash_after: u64) {
        let _ = (keys, hash_after);
        todo!("Milestone 5 Task D")
    }

    pub fn frames(&self) -> u32 {
        todo!("Milestone 5 Task D")
    }

    /// Rewind: drops the frames (and hashes) past `frames`.
    pub fn truncate(&mut self, frames: u32) {
        let _ = frames;
        todo!("Milestone 5 Task D")
    }

    pub fn trace(&self, parent: Parent, build: BuildHash) -> Trace {
        let _ = (parent, build);
        todo!("Milestone 5 Task D")
    }
}

/// A recorded hash that replay did not reproduce.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HashMismatch {
    pub frame: u32,
    pub want: u64,
    pub got: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReplayReport {
    pub frames: u32,
    pub end_hash: u64,
    pub first_mismatch: Option<HashMismatch>,
}

/// Steps `world` through the trace (it must be at `start_frame`), comparing the recorded hashes.
pub fn replay(
    game: &impl GameData,
    world: &mut World,
    trace: &Trace,
    each: impl FnMut(&World, &Frame),
) -> ReplayReport {
    let _ = (game, world, trace, each);
    todo!("Milestone 5 Task D")
}
