//! Finding signatures, repro traces and reproduction (Task E).
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::DetectConfig;
use crate::model::FindingDraft;
use engine::data::GameData;
use game_native::trace::{Run, Trace};

/// Dedupe signature: kind, location (or message with digits stripped), map.
pub fn signature(f: &FindingDraft) -> u64 {
    todo!("Task E")
}

/// The finding as one NewGame trace: the lineage's inputs then the tail, hashes by replay.
pub fn repro_trace<G: GameData + Clone>(game: &G, lineage: &[&[Run]], tail: &[Run]) -> Trace {
    todo!("Task E")
}

/// Replays `trace` with the guard and the detectors; Ok iff a finding with `want`'s signature
/// is raised at its last frame.
pub fn reproduce<G: GameData + Clone>(
    game: &G,
    trace: &Trace,
    want: &FindingDraft,
    cfg: &DetectConfig,
) -> Result<(), String> {
    todo!("Task E")
}
