//! Shared data types: macros, jobs, results and findings. Data only, no logic.

use engine::checkpoint::Checkpoint;
use engine::data::{Dir, MapId, ScriptId};
use game_native::trace::Run;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Menu and battle choices a macro applies to whatever it triggers.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Opts {
    /// Run from wild battles (not the first battle, never trainers).
    pub run_wild: bool,
    /// 0 = yes / first option, 1 = no / second, 2 = third (starter ball index);
    /// [`crate::policy::YES_ONCE`] = yes to the first yes/no only.
    pub choice: u8,
    /// Fight with the move of the best expected damage (power, STAB, type matchups) instead
    /// of the first damaging one (the spine's `Battle(Best)`).
    #[serde(default)]
    pub best_move: bool,
    /// Learn a level-up move when it hits harder than the weakest known one, forgetting that
    /// one (the spine's `Battle(BestLearn)`); else keep the known moves.
    #[serde(default)]
    pub learn: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum MacroKind {
    /// Press nothing (and resolve scripts with the policy) until capturable. The root's macro.
    Settle,
    /// Walk to a cell of the current map.
    WalkTo { x: i16, y: i16 },
    /// Walk next to warp cell (x, y) and move `dir` into it until a transition starts.
    Enter { x: i16, y: i16, dir: Dir },
    /// Walk to edge cell (x, y) and hold `dir` until the map changes, then settle (a
    /// crossing's upload is waited out).
    Cross { x: i16, y: i16, dir: Dir },
    /// Walk next to object `local_id`, face it, press A, resolve the script.
    Talk { local_id: u8 },
    /// Face bg event (x, y) from the adjacent cell `dir` points from, press A, resolve.
    Inspect { x: i16, y: i16, dir: Dir },
    /// Pace between encounter cell (x, y) (tall grass, cave floor, …) and a walkable neighbour until a battle starts.
    Grass { x: i16, y: i16 },
    /// Press nothing for `frames` frames, then settle (the softlock probe's wait, so that
    /// wandering objects can free a way; never a search candidate).
    Wait { frames: u16 },
    /// Turn in place to `dir` (a one-frame tap), then settle. A spine step (never a
    /// candidate).
    Face { dir: Dir },
    /// Press A once on whatever is ahead, then resolve what starts. A spine step.
    Interact,
    /// Hold `keys` (`engine::Buttons` bits) for `frames` frames, then resolve. A spine step.
    Press { keys: u16, frames: u16 },
    /// Hold `dir` until the player leaves its cell (one step; a bump into a scripted cell
    /// starts its script), then resolve. A spine step.
    Step { dir: Dir },
    /// Hold `dir` until the map changes (a door, a mat or a connection from the current
    /// cell), then resolve. A spine step.
    Push { dir: Dir },
    /// From the settled field: START, BAG, `item` (an `ItemId`), USE, through the bag's
    /// messages and questions (yes), then the party menu on slot `mon`, forgetting move slot
    /// `forget` if a move must go (a TM or HM); then out of every screen to the field, and
    /// resolve. Reads the screens' state each frame (`engine::ui`). A spine step.
    UseItem { item: u16, mon: u8, forget: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Macro {
    pub kind: MacroKind,
    pub opts: Opts,
}

impl Macro {
    /// `kind` with the default options.
    pub fn of(kind: MacroKind) -> Macro {
        Macro {
            kind,
            opts: Opts::default(),
        }
    }

    /// The macro as RON text (the db's `macro` columns).
    pub fn to_text(&self) -> String {
        ron::to_string(self).expect("a macro serializes")
    }

    /// [`Macro::to_text`] back.
    pub fn from_text(s: &str) -> Option<Macro> {
        ron::from_str(s).ok()
    }
}

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct AbstractKey(pub u64);

/// The parts of an abstract key (`key::place_region`: the place, for the softlock probe).
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
    /// The heal location of the last `setrespawn` (`WorldSnapshot::respawn`; key version 2).
    pub respawn: u8,
    /// `key::templates_digest` of the object template deltas (key version 2).
    pub templates: u64,
    /// `key::key_items_digest` of the key items held (key version 3).
    pub key_items: u64,
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
    /// Started (`Event::ScriptStarted`).
    pub scripts: BTreeSet<ScriptId>,
    pub battles: Vec<BattleSummary>,
    /// The maps a battle started on, in order (one per `battles` entry).
    pub battle_maps: Vec<MapId>,
    /// `World::log().count()` delta.
    pub log_lines: u32,
    /// `World::events_dropped` delta (`EVENTS_MAX`): `maps` and `scripts` may be incomplete.
    pub events_dropped: u32,
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
    /// A script's `Request::Report` (an unsupported or degraded command or special):
    /// report-only, one per (report, map) (`findings::report_message`).
    Unsupported,
    /// A branch that changed no progress state could not rejoin the spine: its recovery or the
    /// spine's verification window failed, or the progress state differs after it.
    Divergence,
    /// The vanilla oracle (BPEE in mGBA, `cargo xtask vanilla`) disagrees with our engine on a
    /// story field at a spine step boundary or a branch's end, outside the RNG tolerances
    /// (`explore::oracle`): report-only, one per (segment, step kind, field, map).
    Parity,
}

impl FindingKind {
    /// Whether rows of this kind are reports, not bugs: they never stop a job, and progress
    /// and `explore findings --bugs` count them apart. (`SlowFrame`s never become rows.)
    pub fn report_only(self) -> bool {
        matches!(self, FindingKind::Unsupported | FindingKind::Parity)
    }
}

/// A bug seen during a job, with everything needed to reproduce it.
#[derive(Clone, PartialEq, Debug)]
pub struct FindingDraft {
    pub kind: FindingKind,
    pub message: String,
    /// Rust `file:line:col` of a panic.
    pub location: Option<String>,
    pub macro_: Option<Macro>,
    /// Inputs from the job's start state to the frame the finding was seen (inclusive).
    pub tail: Vec<Run>,
    pub map: MapId,
    pub pos: (i16, i16),
    /// Last started.
    pub script: Option<ScriptId>,
    pub log: Vec<&'static str>,
}

/// Novelty kinds (the db's `seen.kind`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SeenKind {
    Flag = 0,
    VarValue = 1,
    Map = 2,
    Script = 3,
    /// `(map << 32) | (x as u16) << 16 | y as u16`.
    Cell = 4,
}

impl SeenKind {
    pub const ALL: [SeenKind; 5] = [
        SeenKind::Flag,
        SeenKind::VarValue,
        SeenKind::Map,
        SeenKind::Script,
        SeenKind::Cell,
    ];

    /// The db number back.
    pub fn from_i64(n: i64) -> Option<SeenKind> {
        SeenKind::ALL.into_iter().find(|k| *k as i64 == n)
    }
}

/// What a job does from its start state.
#[derive(Clone, PartialEq, Debug)]
pub enum Plan {
    /// Run these macros in sequence.
    Given(Vec<Macro>),
    /// Softlock probe: bounded BFS over macros (`softlock::probe`).
    Softlock { k: u32, breadth: u32 },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub index: u32,
    /// The start state, or `None`: a new game (`World::new`).
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
    /// The job stopped on a finding right after this step (its last).
    pub bug: bool,
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
    /// Each a step from the previous one's end.
    pub steps: Vec<StepResult>,
    pub stop: Option<Stop>,
    /// Frames the job stepped beyond its steps' inputs: softlock probe expansions.
    pub probe_frames: u64,
    /// Report-only slow frames (`Expanded::slow`) of the job's steps.
    pub slow_frames: u32,
    /// What the driver did beyond the macros (`Expanded::notes`), in step order.
    pub notes: Vec<String>,
    /// Report-only `Unsupported` findings (`Expanded::reports`), one per signature, tails from
    /// the job's start; they never stop the job.
    pub reports: Vec<FindingDraft>,
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
            MacroKind::Wait { frames: 300 },
            MacroKind::Face { dir: Dir::Down },
            MacroKind::Interact,
            MacroKind::Press {
                keys: 0x0101,
                frames: 9,
            },
            MacroKind::UseItem {
                item: 344,
                mon: 0,
                forget: 2,
            },
        ];
        for (i, kind) in all.into_iter().enumerate() {
            let m = Macro {
                kind,
                opts: Opts {
                    run_wild: i % 2 == 0,
                    choice: i as u8 % 3,
                    ..Opts::default()
                },
            };
            assert_eq!(Macro::from_text(&m.to_text()), Some(m), "{m:?}");
        }
        assert_eq!(Macro::from_text("nonsense"), None);
    }

    #[test]
    fn seen_kinds_have_their_db_numbers() {
        assert_eq!(SeenKind::ALL.map(|k| k as i64), [0, 1, 2, 3, 4]);
        for k in SeenKind::ALL {
            assert_eq!(SeenKind::from_i64(k as i64), Some(k));
        }
        assert_eq!(SeenKind::from_i64(5), None);
    }

    #[test]
    fn defaults_are_empty() {
        assert_eq!(
            Opts::default(),
            Opts {
                run_wild: false,
                choice: 0,
                ..Opts::default()
            }
        );
        assert_eq!(PartyBucket::default().species, [0; 6]);
    }
}
