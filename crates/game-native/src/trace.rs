//! Replay traces: the parent checkpoint, RLE input frames, the build and periodic
//! `World::hash`es (Milestone 5 Task D).
//!
//! A trace is pure input: replaying it from its parent state on any build reaches its node's
//! state, or shows where that build diverges (the first recorded hash it does not reproduce).

use engine::checkpoint::BuildHash;
use engine::data::GameData;
use engine::video::Frame;
use engine::{Buttons, World};
use serde::{Deserialize, Serialize};

/// 2: `Trace::inputs` is written as `keys*frames` run strings (`runs_text`).
pub const TRACE_SCHEMA: u16 = 2;
/// A trace records `World::hash` every this many frames (and at its end).
pub const HASH_EVERY: u32 = 60;

/// A node of a checkpoint store (files: `c00042`). Serialized as its bare number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckpointId(pub u32);

impl core::fmt::Display for CheckpointId {
    /// The file stem: `c00042`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "c{:05}", self.0)
    }
}

/// Where a trace starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Parent {
    /// `World::new(&game)`.
    NewGame,
    Checkpoint(CheckpointId),
}

impl core::fmt::Display for Parent {
    /// `new game` or the node's id (`c00042`).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Parent::NewGame => f.pad("new game"),
            Parent::Checkpoint(id) => f.pad(&id.to_string()),
        }
    }
}

/// `frames` consecutive frames with `keys` held. Serialized as a `(keys, frames)` tuple, so a
/// pretty RON trace holds one run per line.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "(Buttons, u32)", into = "(Buttons, u32)")]
pub struct Run {
    pub keys: Buttons,
    pub frames: u32,
}

impl From<(Buttons, u32)> for Run {
    fn from((keys, frames): (Buttons, u32)) -> Self {
        Self { keys, frames }
    }
}

impl From<Run> for (Buttons, u32) {
    fn from(r: Run) -> Self {
        (r.keys, r.frames)
    }
}

/// Run-length encodes per-frame keys: no zero-length run, adjacent runs differ.
pub fn rle(keys: impl IntoIterator<Item = Buttons>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for keys in keys {
        match runs.last_mut() {
            Some(run) if run.keys == keys => run.frames += 1,
            _ => runs.push(Run { keys, frames: 1 }),
        }
    }
    runs
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub schema: u16,
    pub parent: Parent,
    pub build: BuildHash,
    /// The parent's `World::frame` (0 for `NewGame`).
    pub start_frame: u32,
    /// RLE; adjacent runs differ. Serialized as strings of up to [`RUNS_PER_LINE`]
    /// `keys*frames` runs (keys as the raw `Buttons` bits), one string per pretty RON line.
    #[serde(with = "runs_text")]
    pub inputs: Vec<Run>,
    /// (frames since start, `World::hash`) every `HASH_EVERY` frames and at the end.
    pub hashes: Vec<(u32, u64)>,
}

/// Runs per string of a serialized [`Trace::inputs`].
pub const RUNS_PER_LINE: usize = 16;

/// [`Trace::inputs`] as strings like `"0*24 16*41 64*112"`: compact, yet one diffable line per
/// [`RUNS_PER_LINE`] runs in a pretty RON node file.
mod runs_text {
    use super::{RUNS_PER_LINE, Run};
    use engine::Buttons;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(runs: &[Run], s: S) -> Result<S::Ok, S::Error> {
        let lines: Vec<String> = runs
            .chunks(RUNS_PER_LINE)
            .map(|c| {
                let runs: Vec<String> = c
                    .iter()
                    .map(|r| format!("{}*{}", r.keys.0, r.frames))
                    .collect();
                runs.join(" ")
            })
            .collect();
        lines.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Run>, D::Error> {
        let mut runs = Vec::new();
        for line in Vec::<String>::deserialize(d)? {
            for run in line.split_ascii_whitespace() {
                let bad = || D::Error::custom(format!("bad input run {run:?} (want keys*frames)"));
                let (keys, frames) = run.split_once('*').ok_or_else(bad)?;
                runs.push(Run {
                    keys: Buttons(keys.parse().map_err(|_| bad())?),
                    frames: frames.parse().map_err(|_| bad())?,
                });
            }
        }
        Ok(runs)
    }
}

impl Trace {
    /// Frames the trace steps.
    pub fn frames(&self) -> u32 {
        self.inputs
            .iter()
            .fold(0u32, |n, r| n.saturating_add(r.frames))
    }

    /// The keys of every frame, in order.
    pub fn keys(&self) -> impl Iterator<Item = Buttons> + '_ {
        self.inputs
            .iter()
            .flat_map(|r| core::iter::repeat_n(r.keys, r.frames as usize))
    }
}

/// Whether a trace of `frames` frames records the hash after frame `i` (1-based).
fn hash_point(i: u32, frames: u32) -> bool {
    i.is_multiple_of(HASH_EVERY) || i == frames
}

/// Records a trace while a session plays.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraceRecorder {
    start_frame: u32,
    keys: Vec<Buttons>,
    /// `World::hash` after each frame (so `truncate` can end anywhere).
    hashes: Vec<u64>,
}

impl TraceRecorder {
    pub fn new(start_frame: u32) -> Self {
        Self {
            start_frame,
            keys: Vec::new(),
            hashes: Vec::new(),
        }
    }

    /// One stepped frame: its keys and `World::hash` after the step.
    pub fn push(&mut self, keys: Buttons, hash_after: u64) {
        self.keys.push(keys);
        self.hashes.push(hash_after);
    }

    pub fn frames(&self) -> u32 {
        self.keys.len() as u32
    }

    /// Rewind: drops the frames (and hashes) past `frames`.
    pub fn truncate(&mut self, frames: u32) {
        self.keys.truncate(frames as usize);
        self.hashes.truncate(frames as usize);
    }

    pub fn trace(&self, parent: Parent, build: BuildHash) -> Trace {
        let n = self.frames();
        Trace {
            schema: TRACE_SCHEMA,
            parent,
            build,
            start_frame: self.start_frame,
            inputs: rle(self.keys.iter().copied()),
            hashes: (1..=n)
                .filter(|&i| hash_point(i, n))
                .map(|i| (i, self.hashes[i as usize - 1]))
                .collect(),
        }
    }
}

/// A recorded hash that replay did not reproduce.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HashMismatch {
    /// Frames since the trace's start (as in [`Trace::hashes`]).
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
    replay_with_hashes(game, world, trace, each).0
}

/// [`replay`], also returning the hashes this replay observed at the recorder's cadence
/// (every [`HASH_EVERY`] frames and at the end): the trace's `hashes` re-recorded on this
/// build, for `verify --accept`.
///
/// Every frame is stepped (a mismatch never stops the replay) and `each` sees the world and
/// frame after every step. A recorded hash past the trace's end is a mismatch (`got` is the
/// end hash); out-of-order hashes are skipped. Panics if `world` is not at `start_frame`.
pub fn replay_with_hashes(
    game: &impl GameData,
    world: &mut World,
    trace: &Trace,
    mut each: impl FnMut(&World, &Frame),
) -> (ReplayReport, Vec<(u32, u64)>) {
    assert_eq!(
        world.frame(),
        trace.start_frame,
        "replay: the world must be at the trace's start frame"
    );
    let n = trace.frames();
    let mut want = trace.hashes.iter().copied().peekable();
    let mut observed = Vec::new();
    let mut first_mismatch = None;
    let mut end_hash = world.hash();
    for (i, keys) in (1..).zip(trace.keys()) {
        let frame = world.step(game, keys);
        each(world, &frame);
        while want.next_if(|&(at, _)| at < i).is_some() {}
        let check = want.next_if(|&(at, _)| at == i);
        if !hash_point(i, n) && check.is_none() {
            continue;
        }
        let got = world.hash();
        if hash_point(i, n) {
            observed.push((i, got));
        }
        if let Some((_, w)) = check
            && w != got
            && first_mismatch.is_none()
        {
            first_mismatch = Some(HashMismatch {
                frame: i,
                want: w,
                got,
            });
        }
        end_hash = got;
    }
    // A recorded hash past the end was never reproduced: the trace was cut short.
    if first_mismatch.is_none()
        && let Some(&(at, want)) = trace.hashes.iter().find(|&&(at, _)| at > n)
    {
        first_mismatch = Some(HashMismatch {
            frame: at,
            want,
            got: end_hash,
        });
    }
    let report = ReplayReport {
        frames: n,
        end_hash,
        first_mismatch,
    };
    (report, observed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Buttons = Buttons::A;
    const B: Buttons = Buttons::B;
    const N: Buttons = Buttons::NONE;

    fn recorder(keys: &[Buttons]) -> TraceRecorder {
        let mut r = TraceRecorder::new(7);
        for (i, &k) in keys.iter().enumerate() {
            r.push(k, 1000 + i as u64 + 1);
        }
        r
    }

    #[test]
    fn rle_merges_equal_neighbours_and_round_trips() {
        let keys = [A, A, A, N, B, B, A];
        let t = recorder(&keys).trace(Parent::NewGame, BuildHash::default());
        assert_eq!(
            t.inputs,
            [
                Run { keys: A, frames: 3 },
                Run { keys: N, frames: 1 },
                Run { keys: B, frames: 2 },
                Run { keys: A, frames: 1 },
            ]
        );
        assert_eq!(t.frames(), 7);
        assert_eq!(t.keys().collect::<Vec<_>>(), keys);
        assert_eq!(t.start_frame, 7);
        assert_eq!(t.schema, TRACE_SCHEMA);
        assert_eq!(t.parent, Parent::NewGame);
    }

    #[test]
    fn inputs_serialize_as_run_strings_of_16_per_line() {
        let keys: Vec<Buttons> = (0..40u16)
            .flat_map(|i| core::iter::repeat_n(Buttons(i % 2 * 16), 1 + i as usize))
            .collect();
        let t = TraceRecorder::new(0).trace(Parent::NewGame, BuildHash::default());
        let t = Trace {
            inputs: rle(keys),
            ..t
        };
        assert_eq!(t.inputs.len(), 40);
        let text = ron::ser::to_string_pretty(&t, ron::ser::PrettyConfig::new()).unwrap();
        let first: String = (0..16)
            .map(|i| format!("{}*{}", i % 2 * 16, i + 1))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains(&format!("\"{first}\",")), "{text}");
        let strings = text
            .lines()
            .filter(|l| l.trim_start().starts_with('"'))
            .count();
        assert_eq!(strings, 3, "16 + 16 + 8 runs: {text}");
        let back: Trace = ron::from_str(&text).unwrap();
        assert_eq!(back, t);
        for bad in ["\"1*\"", "\"x*2\"", "\"3\"", "\"1*2*3\""] {
            let text = text.replacen(&format!("\"{first}\""), bad, 1);
            assert!(ron::from_str::<Trace>(&text).is_err(), "{bad}");
        }
    }

    #[test]
    fn rle_never_stores_empty_runs() {
        assert_eq!(rle([]), []);
        assert_eq!(rle([N]), [Run { keys: N, frames: 1 }]);
        let t = TraceRecorder::new(0).trace(Parent::NewGame, BuildHash::default());
        assert!(t.inputs.is_empty() && t.hashes.is_empty());
        assert_eq!(t.frames(), 0);
        assert_eq!(t.keys().count(), 0);
    }

    #[test]
    fn keys_skip_zero_length_runs_of_hand_edited_traces() {
        let t = Trace {
            schema: TRACE_SCHEMA,
            parent: Parent::NewGame,
            build: BuildHash::default(),
            start_frame: 0,
            inputs: vec![Run { keys: A, frames: 0 }, Run { keys: B, frames: 2 }],
            hashes: vec![],
        };
        assert_eq!(t.frames(), 2);
        assert_eq!(t.keys().collect::<Vec<_>>(), [B, B]);
    }

    #[test]
    fn recorder_hashes_every_60_frames_and_at_the_end() {
        let r = recorder(&[N; 130]);
        assert_eq!(r.frames(), 130);
        let t = r.trace(Parent::NewGame, BuildHash::default());
        assert_eq!(t.hashes, [(60, 1060), (120, 1120), (130, 1130)]);
        // On a multiple of the cadence the end hash is not repeated.
        let t = recorder(&[N; 120]).trace(Parent::NewGame, BuildHash::default());
        assert_eq!(t.hashes, [(60, 1060), (120, 1120)]);
    }

    #[test]
    fn truncate_cuts_mid_run_and_drops_later_hashes() {
        let mut keys = vec![A; 50];
        keys.extend([B; 100]);
        let mut r = recorder(&keys);
        r.truncate(70);
        assert_eq!(r.frames(), 70);
        let t = r.trace(Parent::Checkpoint(CheckpointId(3)), BuildHash::default());
        assert_eq!(
            t.inputs,
            [
                Run {
                    keys: A,
                    frames: 50
                },
                Run {
                    keys: B,
                    frames: 20
                }
            ]
        );
        assert_eq!(t.hashes, [(60, 1060), (70, 1070)]);
        assert_eq!(t.parent, Parent::Checkpoint(CheckpointId(3)));
        // Recording goes on from the cut.
        r.push(A, 5);
        let t = r.trace(Parent::NewGame, BuildHash::default());
        assert_eq!(t.frames(), 71);
        assert_eq!(t.hashes, [(60, 1060), (71, 5)]);
        // Truncating past the end changes nothing.
        r.truncate(1000);
        assert_eq!(r.frames(), 71);
    }
}
