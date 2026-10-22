//! Shared data types (Wave 0 contract): nodes, jobs, results, findings and the generation
//! delta. Data only, no logic.

use engine::checkpoint::{BuildHash, Checkpoint};
use engine::data::{Dir, MapId, ScriptId};
use game_native::trace::Run;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

/// SQLite rowid of a node; assigned by the runner in merge order (deterministic). 1 = root.
pub type NodeId = i64;

/// Menu and battle choices a macro applies to whatever it triggers.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Opts {
    /// Run from wild battles (not the first battle, never trainers).
    pub run_wild: bool,
    /// 0 = yes / first option, 1 = no / second, 2 = third (starter ball index).
    pub choice: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum MacroKind {
    /// Press nothing (and resolve scripts with the policy) until capturable. The root's macro.
    Settle,
    /// Walk to a cell of the current map.
    WalkTo { x: i16, y: i16 },
    /// Walk next to warp cell (x, y) and move `dir` into it until a transition starts.
    Enter { x: i16, y: i16, dir: Dir },
    /// Walk to edge cell (x, y) and hold `dir` until the map changes, then walk on until
    /// the view is no longer stale.
    Cross { x: i16, y: i16, dir: Dir },
    /// Walk next to object `local_id`, face it, press A, resolve the script.
    Talk { local_id: u8 },
    /// Face bg event (x, y) from the adjacent cell `dir` points from, press A, resolve.
    Inspect { x: i16, y: i16, dir: Dir },
    /// Pace between tall-grass cell (x, y) and a walkable neighbour until a battle starts.
    Grass { x: i16, y: i16 },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Macro {
    pub kind: MacroKind,
    pub opts: Opts,
}

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct AbstractKey(pub u64);

/// The parts of an abstract key. `region` = the key without the cell (softlock, stall).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyParts {
    pub map: MapId,
    /// Coarse: `(x >> shift, y >> shift)`.
    pub cell: (i16, i16),
    /// Digest of story flags (KeyConfig exclusions applied).
    pub flags: u64,
    /// Digest of story vars.
    pub vars: u64,
    pub party: PartyBucket,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PartyBucket {
    pub count: u8,
    /// Party order, 0 = empty.
    pub species: [u16; 6],
    /// Sum of levels / `KeyConfig::level_step`.
    pub level_bucket: u16,
    pub fainted: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Fitness {
    pub novelty: i64,
    pub progress: i64,
    pub bug: i64,
    pub balance: i64,
    pub total: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum NodeStatus {
    Valid,
    Stale,
    Merged,
    Broken,
}

#[derive(Clone, PartialEq, Debug)]
pub struct NodeRow {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub depth: u32,
    pub macro_: Macro,
    /// From the parent's end state (a new game for the root) to this node's end state.
    pub inputs: Vec<Run>,
    pub frames: u32,
    pub total_frames: u64,
    /// `World::frame` at the end.
    pub end_frame: u32,
    pub key: AbstractKey,
    pub region: u64,
    /// `state_digest` of the end snapshot.
    pub digest: u64,
    pub world_hash: u64,
    pub map: MapId,
    pub pos: (i16, i16),
    /// Goals reached along the lineage.
    pub progress: u32,
    pub fitness: Fitness,
    pub generation: u32,
    pub expanded: u32,
    pub status: NodeStatus,
    pub merged_into: Option<NodeId>,
    pub build: BuildHash,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BattleSummary {
    /// 0 wild, 1 first battle, 2 trainer.
    pub kind: u8,
    pub trainer: Option<u16>,
    /// `BattleOutcome` as u8.
    pub outcome: Option<u8>,
    pub frames: u32,
    pub player_level: u8,
    pub opponent_level: u8,
    pub hp_lost_permille: u16,
}

/// What one expansion saw. Everything but `max_step_ns` is deterministic.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SimSummary {
    pub frames: u32,
    /// Entered, in order (consecutive duplicates dropped).
    pub maps: Vec<MapId>,
    /// Player cells stood on.
    pub cells: BTreeSet<(MapId, i16, i16)>,
    /// Started (`Probe`).
    pub scripts: BTreeSet<ScriptId>,
    pub battles: Vec<BattleSummary>,
    /// `World::log().count()` delta.
    pub log_lines: u32,
    /// Host time, report-only.
    pub max_step_ns: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum FindingKind {
    Panic,
    Softlock,
    RoundTrip,
    CapturableStuck,
    PlannerDivergence,
    SlowFrame,
}

/// A bug seen during a job, with everything needed to reproduce it.
#[derive(Clone, PartialEq, Debug)]
pub struct FindingDraft {
    pub kind: FindingKind,
    pub message: String,
    /// Rust `file:line:col` of a panic.
    pub location: Option<String>,
    pub macro_: Option<Macro>,
    /// Inputs from the job's parent end state to the frame the finding was seen (inclusive).
    pub tail: Vec<Run>,
    pub map: MapId,
    pub pos: (i16, i16),
    /// Last started.
    pub script: Option<ScriptId>,
    pub log: Vec<&'static str>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Touch {
    Map(MapId),
    Script(ScriptId),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SeenKind {
    Flag = 0,
    VarValue = 1,
    Map = 2,
    Script = 3,
}

/// What a job does from its start state.
#[derive(Clone, PartialEq, Debug)]
pub enum Plan {
    /// Pick `len` macros at random among the state's candidates (seeded), in sequence.
    Random { len: u8, seed: u64 },
    /// Run these macros in sequence (opts flips, re-expansion).
    Given(Vec<Macro>),
    /// Softlock probe: bounded BFS over macros (Task E's `softlock::probe`).
    Softlock { k: u32, breadth: u32 },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub index: u32,
    /// `None` = from a new game (the root).
    pub parent: Option<NodeId>,
    /// `None` = `World::new`.
    pub start: Option<Arc<Checkpoint>>,
    pub plan: Plan,
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub macro_: Macro,
    pub inputs: Vec<Run>,
    /// Capturable end state.
    pub end: Arc<Checkpoint>,
    pub summary: SimSummary,
}

#[derive(Clone, Debug)]
pub enum Stop {
    /// The plan could not continue (no candidate, no path, no effect): not a bug.
    Pruned(String),
    Finding(FindingDraft),
    /// The softlock probe's verdict for the start state.
    NotTrapped,
}

#[derive(Clone, Debug)]
pub struct JobResult {
    pub index: u32,
    pub parent: Option<NodeId>,
    /// Each becomes a candidate node, chained.
    pub steps: Vec<StepResult>,
    pub stop: Option<Stop>,
}

/// The merge's output: one SQLite transaction.
#[derive(Clone, Debug, Default)]
pub struct GenerationDelta {
    pub generation: u32,
    /// (old, new): old -> Merged, applied before inserts.
    pub merged: Vec<(NodeId, NodeId)>,
    /// `expanded += n`.
    pub expanded: Vec<(NodeId, u32)>,
    pub nodes: Vec<NodeRow>,
    pub snapshots: Vec<(NodeId, Arc<Checkpoint>)>,
    pub touches: Vec<(NodeId, Touch)>,
    /// First sightings; existing ones get `hits += 1`.
    pub seen: Vec<(SeenKind, i64, NodeId)>,
    pub cells: Vec<(MapId, i16, i16, NodeId)>,
    pub battles: Vec<(NodeId, BattleSummary)>,
    /// (finding, the node whose end state the tail starts at (`None` = a new game: the root
    /// job failed), total frame).
    pub findings: Vec<(FindingDraft, Option<NodeId>, u64)>,
    pub stats: GenStats,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GenStats {
    pub jobs: u32,
    pub steps: u32,
    pub inserted: u32,
    pub duplicates: u32,
    pub pruned: u32,
    pub frames: u64,
}

/// The scorer the runner merges with (Task B implements it; tests use fakes).
pub trait Evaluate {
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation;
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub key: AbstractKey,
    pub region: u64,
    pub progress: u32,
    pub fitness: Fitness,
    pub seen: Vec<(SeenKind, i64)>,
    pub touches: Vec<Touch>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_round_trip_through_ron() {
        let all = [
            MacroKind::Settle,
            MacroKind::WalkTo { x: 3, y: -1 },
            MacroKind::Enter {
                x: 1,
                y: 2,
                dir: Dir::Up,
            },
            MacroKind::Cross {
                x: 0,
                y: 9,
                dir: Dir::Left,
            },
            MacroKind::Talk { local_id: 7 },
            MacroKind::Inspect {
                x: 4,
                y: 4,
                dir: Dir::Right,
            },
            MacroKind::Grass { x: 12, y: 3 },
        ];
        for (i, kind) in all.into_iter().enumerate() {
            let m = Macro {
                kind,
                opts: Opts {
                    run_wild: i % 2 == 0,
                    choice: i as u8 % 3,
                },
            };
            let text = ron::to_string(&m).unwrap();
            assert_eq!(ron::from_str::<Macro>(&text).unwrap(), m, "{text}");
        }
    }

    #[test]
    fn seen_kinds_have_their_db_numbers() {
        assert_eq!(
            [
                SeenKind::Flag,
                SeenKind::VarValue,
                SeenKind::Map,
                SeenKind::Script
            ]
            .map(|k| k as i64),
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn defaults_are_empty() {
        assert_eq!(
            Opts::default(),
            Opts {
                run_wild: false,
                choice: 0
            }
        );
        assert_eq!(Fitness::default().total, 0);
        assert_eq!(PartyBucket::default().species, [0; 6]);
        assert!(GenerationDelta::default().nodes.is_empty());
    }
}
