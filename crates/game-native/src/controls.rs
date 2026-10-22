//! The `play` bin's save, load and rewind logic, without a window or a store (Milestone 5
//! Task G): `main` maps keys, does the store IO this asks for, and draws.
//!
//! - **F5 saves:** at once when the world is capturable, else at the first capturable frame
//!   ("saving…"). The request carries the trace from the current parent; once the store has
//!   added it ([`Controls::saved`]) the new node is the parent and recording restarts.
//! - **F9 loads** the parent node ([`Controls::load`] with its materialized checkpoint).
//! - **F8 rewinds:** a ring of [`RING_LEN`] capturable checkpoints, one every [`RING_EVERY`]
//!   frames of the current trace; it restores the newest one at least [`REWIND_BACK`] frames
//!   back (else the oldest) and cuts the recorded trace there.

use crate::Session;
use crate::trace::{CheckpointId, Parent, Trace, TraceRecorder};
use engine::Buttons;
use engine::checkpoint::{BuildHash, Checkpoint, RestoreError, build_hash};
use engine::data::GameData;
use std::collections::VecDeque;

/// Rewind ring entries kept (about a minute at [`RING_EVERY`]).
pub const RING_LEN: usize = 64;
/// Frames between rewind ring entries.
pub const RING_EVERY: u32 = 60;
/// A rewind goes back at least this many frames (when the ring reaches that far).
pub const REWIND_BACK: u32 = 120;

/// A save the store must perform: `store.add(&label, trace, &checkpoint)`, then
/// [`Controls::saved`] with the new id.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaveRequest {
    pub label: String,
    pub trace: Trace,
    pub checkpoint: Checkpoint,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct RingEntry {
    /// Frames of the current trace at capture.
    frames: u32,
    cp: Checkpoint,
}

pub struct Controls<G: GameData> {
    session: Session<G>,
    build: BuildHash,
    parent: Parent,
    recorder: TraceRecorder,
    pending: bool,
    ring: VecDeque<RingEntry>,
    last: String,
}

impl<G: GameData> Controls<G> {
    /// A new game.
    pub fn new(game: G) -> Self {
        let build = build_hash(&game);
        let session = Session::new(game);
        let mut c = Self {
            recorder: TraceRecorder::new(session.world().frame()),
            session,
            build,
            parent: Parent::NewGame,
            pending: false,
            ring: VecDeque::new(),
            last: "new game".into(),
        };
        c.ring_tick();
        c
    }

    /// Starts at node `id`, whose state is `cp`.
    pub fn resume(game: G, id: CheckpointId, cp: &Checkpoint) -> Result<Self, RestoreError> {
        let mut c = Self::new(game);
        c.load(id, cp)?;
        Ok(c)
    }

    pub fn session(&self) -> &Session<G> {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut Session<G> {
        &mut self.session
    }

    /// The node the current trace starts from.
    pub fn parent(&self) -> Parent {
        self.parent
    }

    /// The trace recorded since the parent.
    pub fn trace(&self) -> Trace {
        self.recorder.trace(self.parent, self.build)
    }

    /// A save is waiting for a capturable frame.
    pub fn pending(&self) -> bool {
        self.pending
    }

    /// The trace frame of each rewind ring entry, oldest first.
    pub fn ring_frames(&self) -> Vec<u32> {
        self.ring.iter().map(|e| e.frames).collect()
    }

    /// Steps the game; gives a pending save once the world is capturable.
    pub fn step(&mut self, keys: Buttons) -> Option<SaveRequest> {
        self.session.step(keys);
        self.recorder.push(keys, self.session.world().hash());
        self.ring_tick();
        if self.pending {
            return self.save();
        }
        None
    }

    /// F5: the save request now if the world is capturable, else it is pending. Nothing when
    /// no frame was played since the parent node ("already saved").
    pub fn save(&mut self) -> Option<SaveRequest> {
        if self.recorder.frames() == 0 && self.parent != Parent::NewGame {
            self.pending = false;
            self.last = "already saved".into();
            return None;
        }
        match self.session.checkpoint() {
            Ok(checkpoint) => {
                self.pending = false;
                let w = self.session.world();
                Some(SaveRequest {
                    label: format!("map {:#06x} f{}", w.map().0, w.frame()),
                    trace: self.trace(),
                    checkpoint,
                })
            }
            Err(_) => {
                if !self.pending {
                    self.pending = true;
                    self.last = "saving…".into();
                }
                None
            }
        }
    }

    /// The store added the last [`SaveRequest`] as `id`: it becomes the parent.
    pub fn saved(&mut self, id: CheckpointId) {
        self.parent = Parent::Checkpoint(id);
        self.restart();
        self.last = format!("saved {id}");
    }

    /// Notes a message in the title (a failed save or load).
    pub fn note(&mut self, msg: impl Into<String>) {
        self.last = msg.into();
    }

    /// F9 / `--load`: restarts from node `id`, whose state is `cp`. On error nothing changes.
    pub fn load(&mut self, id: CheckpointId, cp: &Checkpoint) -> Result<(), RestoreError> {
        self.session.load(cp)?;
        self.parent = Parent::Checkpoint(id);
        self.pending = false;
        self.restart();
        self.last = format!("loaded {id}");
        Ok(())
    }

    /// F8: restores the newest ring entry at least [`REWIND_BACK`] frames back (the oldest
    /// when none is), cutting the trace there. `false` when the ring is empty.
    pub fn rewind(&mut self) -> bool {
        let now = self.recorder.frames();
        let Some(i) = (self.ring.iter())
            .rposition(|e| e.frames + REWIND_BACK <= now)
            .or((!self.ring.is_empty()).then_some(0))
        else {
            self.last = "nothing to rewind to".into();
            return false;
        };
        let e = &self.ring[i];
        if let Err(err) = self.session.load(&e.cp) {
            // A ring entry was captured on this build: it always restores.
            self.last = format!("rewind failed: {err:?}");
            return false;
        }
        self.recorder.truncate(e.frames);
        self.ring.truncate(i + 1);
        let e = &self.ring[i];
        self.pending = false;
        self.last = format!("rewound {} frames", now - e.frames);
        true
    }

    /// The window title: frame, parent node and the last action.
    pub fn title(&self) -> String {
        let parent = match self.parent {
            Parent::NewGame => "new game".to_string(),
            Parent::Checkpoint(id) => id.to_string(),
        };
        format!(
            "pokemon-rust  f{}  from {parent}  {}",
            self.session.world().frame(),
            self.last
        )
    }

    /// A fresh trace from the current state, the ring holding only it.
    fn restart(&mut self) {
        self.recorder = TraceRecorder::new(self.session.world().frame());
        self.ring.clear();
        self.ring_tick();
    }

    /// Pushes a ring entry when one is due and the world is capturable.
    fn ring_tick(&mut self) {
        let frames = self.recorder.frames();
        let due = self
            .ring
            .back()
            .is_none_or(|e| frames >= e.frames + RING_EVERY);
        if !due {
            return;
        }
        if let Ok(cp) = self.session.checkpoint() {
            if self.ring.len() == RING_LEN {
                self.ring.pop_front();
            }
            self.ring.push_back(RingEntry { frames, cp });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::cached_route;

    fn game() -> content::Emerald {
        content::Emerald::for_shells()
    }

    /// Steps `c` until the world is capturable (pressing nothing).
    fn settle(c: &mut Controls<content::Emerald>) {
        for _ in 0..2000 {
            if c.session().world().capturable().is_ok() {
                return;
            }
            assert_eq!(c.step(Buttons::NONE), None);
        }
        panic!("never capturable");
    }

    #[test]
    fn ring_holds_one_capturable_entry_every_60_frames_up_to_64() {
        let mut c = Controls::new(game());
        for _ in 0..(RING_LEN as u32 + 10) * RING_EVERY {
            c.step(Buttons::NONE);
        }
        let f = c.ring_frames();
        assert_eq!(f.len(), RING_LEN);
        assert!(f.windows(2).all(|w| w[1] >= w[0] + RING_EVERY), "{f:?}");
        // Standing still the world is capturable on every due frame: exactly 60 apart.
        let tail = &f[f.len() - 10..];
        assert!(tail.windows(2).all(|w| w[1] == w[0] + RING_EVERY), "{f:?}");
        assert!(*f.last().unwrap() + RING_EVERY > c.trace().frames());
    }

    #[test]
    fn a_save_while_not_capturable_waits_for_a_capturable_frame() {
        let mut c = Controls::new(game());
        // Frame 0 is the boot fade: not capturable.
        assert!(c.session().world().capturable().is_err());
        assert_eq!(c.save(), None);
        assert!(c.pending());
        assert!(c.title().contains("saving…"), "{}", c.title());
        let mut req = None;
        for _ in 0..2000 {
            if let Some(r) = c.step(Buttons::NONE) {
                req = Some(r);
                break;
            }
        }
        let req = req.expect("the save happened");
        assert!(!c.pending());
        let w = c.session().world();
        assert!(w.capturable().is_ok());
        assert_eq!(req.checkpoint.header.frame, w.frame());
        assert_eq!(req.trace.parent, Parent::NewGame);
        assert_eq!(req.trace.start_frame, 0);
        assert_eq!(req.trace.frames(), w.frame());
        assert_eq!(req.label, format!("map {:#06x} f{}", w.map().0, w.frame()));
        // The store adds it: the node is the parent and the trace restarts.
        c.saved(CheckpointId(4));
        assert_eq!(c.parent(), Parent::Checkpoint(CheckpointId(4)));
        assert_eq!(c.trace().frames(), 0);
        assert_eq!(c.trace().start_frame, req.checkpoint.header.frame);
        assert_eq!(c.ring_frames(), [0]);
        // Nothing played since: refused, not pending.
        assert_eq!(c.save(), None);
        assert!(!c.pending());
        assert!(c.title().contains("already saved"), "{}", c.title());
        // One idle frame later a save is immediate.
        assert_eq!(c.step(Buttons::NONE), None);
        let again = c.save().expect("immediate");
        assert_eq!(again.trace.parent, Parent::Checkpoint(CheckpointId(4)));
        assert_eq!(again.trace.frames(), 1);
    }

    #[test]
    fn load_restarts_from_the_node() {
        let mut c = Controls::new(game());
        settle(&mut c);
        for _ in 0..40 {
            c.step(Buttons::LEFT);
        }
        settle(&mut c);
        let req = c.save().unwrap();
        c.saved(CheckpointId(1));
        let at = c.session().world().hash();
        for _ in 0..90 {
            c.step(Buttons::DOWN);
        }
        // F9: back to the parent node.
        c.load(CheckpointId(1), &req.checkpoint).unwrap();
        assert_eq!(c.session().world().hash(), at);
        assert_eq!(c.session().world().frame(), req.checkpoint.header.frame);
        assert_eq!(c.trace().frames(), 0);
        assert_eq!(c.parent(), Parent::Checkpoint(CheckpointId(1)));
        assert!(c.title().contains("c00001"), "{}", c.title());
        // `resume` is the same start.
        let r = Controls::resume(game(), CheckpointId(1), &req.checkpoint).unwrap();
        assert_eq!(r.session().world().hash(), at);
        assert_eq!(r.parent(), Parent::Checkpoint(CheckpointId(1)));
    }

    #[test]
    fn rewind_restores_an_entry_at_least_120_frames_back_and_truncates_the_trace() {
        let rec = cached_route();
        let mut c = Controls::new(game());
        settle(&mut c);
        let req = c.save().unwrap();
        c.saved(CheckpointId(1));
        let start = c.session().world().frame();
        // Live hashes and keys from the parent on.
        let mut hashes = vec![c.session().world().hash()];
        let mut keys = Vec::new();
        for i in 0..400u32 {
            let k = rec.keys_at(start + i);
            c.step(k);
            keys.push(k);
            hashes.push(c.session().world().hash());
        }
        let ring = c.ring_frames();
        let want = *ring
            .iter()
            .rev()
            .find(|&&f| f + REWIND_BACK <= 400)
            .unwrap();
        assert!(c.rewind());
        assert_eq!(c.ring_frames().last(), Some(&want));
        let t = c.trace();
        assert_eq!(t.frames(), want, "the trace is cut at the entry");
        assert_eq!(t.keys().collect::<Vec<_>>(), keys[..want as usize]);
        assert_eq!(c.session().world().frame(), start + want);
        assert_eq!(c.session().world().hash(), hashes[want as usize]);
        // Recording goes on from there, and the trace replays from the parent.
        for i in 0..30 {
            c.step(rec.keys_at(start + want + i));
        }
        let t = c.trace();
        let mut w = engine::World::restore(&game(), &req.checkpoint).unwrap();
        let report = crate::trace::replay(&game(), &mut w, &t, |_, _| {});
        assert_eq!(report.first_mismatch, None);
        assert_eq!(report.end_hash, c.session().world().hash());
        // Rewinding with less than 120 frames recorded goes to the oldest entry: the parent.
        let mut fresh = Controls::resume(game(), CheckpointId(1), &req.checkpoint).unwrap();
        for _ in 0..50 {
            fresh.step(Buttons::NONE);
        }
        assert!(fresh.rewind());
        assert_eq!(fresh.trace().frames(), 0);
        assert_eq!(fresh.session().world().frame(), start);
    }

    #[test]
    fn rewind_with_an_empty_ring_does_nothing() {
        let mut c = Controls::new(game());
        assert!(c.ring_frames().is_empty(), "frame 0 is not capturable");
        assert!(!c.rewind());
        assert_eq!(c.session().world().frame(), 0);
    }
}
