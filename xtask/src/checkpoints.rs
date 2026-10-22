//! `cargo xtask checkpoints <list|verify|seed|label|rm|sav>` (Milestone 5 Task G).

use anyhow::Result;
use game_native::store::Status;
use game_native::trace::CheckpointId;
use std::path::Path;

/// The outcome of [`verify`]: every node's status, in topological order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VerifySummary {
    pub nodes: Vec<(CheckpointId, Status)>,
}

impl VerifySummary {
    /// No node is `Diverged` or `Broken` (`Stale` nodes are only regenerated).
    // Wave 0: used by Tasks G and H.
    #[allow(dead_code)]
    pub fn ok(&self) -> bool {
        self.nodes
            .iter()
            .all(|(_, s)| !matches!(s, Status::Diverged { .. } | Status::Broken(_)))
    }
}

pub fn main(args: &[String]) -> Result<()> {
    let _ = args;
    todo!("Milestone 5 Task G")
}

/// Replays every node of the store at `root` on the current build (`--accept` re-baselines).
// Wave 0 stub: `main` and `parity()` call it in Tasks G and H.
#[allow(dead_code)]
pub fn verify(root: &Path, accept: bool) -> Result<VerifySummary> {
    let _ = (root, accept);
    todo!("Milestone 5 Task G")
}
