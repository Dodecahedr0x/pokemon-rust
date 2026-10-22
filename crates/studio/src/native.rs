//! The native play pane (Task C): a [`Controls`] session on its pane thread, driven by
//! [`NativeCmd`]s, publishing an [`Inspector`] status with a small event timeline.
//!
//! Saves go to the layered store (local over the committed tree) with the current parent,
//! which may be a committed node. Loads and seeks go through a shared [`Materializer`]: its
//! lock is held only to fetch the checkpoint or the seek start; the session is built and
//! stepped on this thread (`World` is `!Send`).

use crate::pane::{Pane, RunMode};
use engine::checkpoint::NotCapturable;
use engine::data::{GameData, MapId};
use engine::{Buttons, World};
use game_native::checkpoints::{Materializer, SeekPrep, add_save};
use game_native::controls::{Controls, SaveRequest};
use game_native::store::Store;
use game_native::trace::{CheckpointId, Parent};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

type Game = content::Emerald;

/// Events kept in [`Inspector::events`].
pub const EVENTS: usize = 64;

pub enum NativeCmd {
    /// The buttons held from now on.
    Keys(Buttons),
    Mode(RunMode),
    /// Pauses and steps `n` frames on the next tick.
    Step(u32),
    Rewind,
    Save,
    /// Restarts from node `id` (the layered store resolves local and committed ids).
    Load(CheckpointId),
    /// Node `id`'s state `frames` frames after its parent (clamped to its trace); recording
    /// goes on from there, so a save branches from inside the trace.
    Seek {
        id: CheckpointId,
        frames: u32,
    },
    NewGame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub frame: u32,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    MapEntered(MapId),
    ScriptStarted,
    ScriptEnded,
    BattleStarted,
    BattleEnded,
    Log(&'static str),
    Saved(CheckpointId),
    Loaded(CheckpointId),
    /// A seek into node `id`'s trace, `frames` after its parent.
    Seeked(CheckpointId, u32),
    /// Frames rewound.
    Rewound(u32),
    NewGame,
}

/// The pane's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspector {
    pub frame: u32,
    pub map: MapId,
    /// The decomp identifier (`MapData::name`).
    pub map_name: &'static str,
    pub hash: u64,
    /// `"field"`, `"battle"` or `"starter"`.
    pub mode: &'static str,
    pub capturable: Result<(), NotCapturable>,
    /// The field camera's top-left map pixel (`World::camera`); `None` outside the field.
    pub camera: Option<(i32, i32)>,
    pub run: RunMode,
    /// The node the current trace starts from.
    pub parent: Parent,
    pub trace_frames: u32,
    pub pending_save: bool,
    pub last: String,
    pub ring_frames: Vec<u32>,
    /// Newest last, at most [`EVENTS`].
    pub events: Vec<Event>,
    /// The last save (the UI refreshes the tree when it changes).
    pub saved: Option<CheckpointId>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            frame: 0,
            map: MapId(0),
            map_name: "",
            hash: 0,
            mode: "field",
            capturable: Ok(()),
            camera: None,
            run: RunMode::Pause,
            parent: Parent::NewGame,
            trace_frames: 0,
            pending_save: false,
            last: String::new(),
            ring_frames: Vec::new(),
            events: Vec::new(),
            saved: None,
        }
    }
}

/// The state the events are diffed from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Seen {
    map: MapId,
    script: bool,
    battle: bool,
    logs: u32,
}

impl Seen {
    fn of(w: &World) -> Self {
        Self {
            map: w.map(),
            script: w.script_running(),
            battle: w.in_battle(),
            logs: w.log().count(),
        }
    }
}

/// The events between two frames' states, in order: map, script, battle, then the new log
/// lines (`log` holds the kept lines after the frame, oldest first).
fn changes(was: Seen, now: Seen, log: impl Iterator<Item = &'static str>) -> Vec<EventKind> {
    let mut out = Vec::new();
    if now.map != was.map {
        out.push(EventKind::MapEntered(now.map));
    }
    match (was.script, now.script) {
        (false, true) => out.push(EventKind::ScriptStarted),
        (true, false) => out.push(EventKind::ScriptEnded),
        _ => {}
    }
    match (was.battle, now.battle) {
        (false, true) => out.push(EventKind::BattleStarted),
        (true, false) => out.push(EventKind::BattleEnded),
        _ => {}
    }
    let new = now.logs.wrapping_sub(was.logs) as usize;
    if new > 0 {
        let mut lines: Vec<&'static str> = log.collect();
        let fresh = lines.split_off(lines.len().saturating_sub(new));
        out.extend(fresh.into_iter().map(EventKind::Log));
    }
    out
}

/// Locks the materializer the panes share. A lock poisoned by a panic on another pane thread is
/// recovered: its caches are only changed by whole operations, but they are dropped to be safe.
pub fn lock_materializer(m: &Mutex<Materializer<Game>>) -> MutexGuard<'_, Materializer<Game>> {
    m.lock().unwrap_or_else(|poisoned| {
        m.clear_poison();
        let mut g = poisoned.into_inner();
        g.clear();
        g
    })
}

pub struct NativePane {
    controls: Controls<Game>,
    run: RunMode,
    held: Buttons,
    /// Frames a `Step` asked for, stepped on the next tick.
    steps: u32,
    store: Store,
    materializer: Arc<Mutex<Materializer<Game>>>,
    seen: Seen,
    events: VecDeque<Event>,
    saved: Option<CheckpointId>,
}

impl NativePane {
    /// A new game, paused. `store` is the layered store (`Store::layered(shared, local)`).
    pub fn new(store: Store, materializer: Arc<Mutex<Materializer<Game>>>) -> Self {
        let controls = Controls::new(content::Emerald::for_shells());
        Self {
            seen: Seen::of(controls.session().world()),
            controls,
            run: RunMode::Pause,
            held: Buttons::NONE,
            steps: 0,
            store,
            materializer,
            events: VecDeque::new(),
            saved: None,
        }
    }

    pub fn controls(&self) -> &Controls<Game> {
        &self.controls
    }

    fn world(&self) -> &World {
        self.controls.session().world()
    }

    fn event(&mut self, kind: EventKind) {
        if self.events.len() == EVENTS {
            self.events.pop_front();
        }
        let frame = self.world().frame();
        self.events.push_back(Event { frame, kind });
    }

    /// The events of the frame just stepped.
    fn diff(&mut self) {
        let now = Seen::of(self.world());
        let kinds = changes(self.seen, now, self.world().log().lines());
        self.seen = now;
        for kind in kinds {
            self.event(kind);
        }
    }

    /// A new session (load, seek, rewind, new game): no events from the jump itself.
    fn jumped(&mut self, kind: EventKind) {
        self.seen = Seen::of(self.world());
        self.event(kind);
    }

    fn persist(&mut self, req: Option<SaveRequest>) {
        let Some(req) = req else { return };
        match add_save(&self.store, &req) {
            Ok(id) => {
                self.controls.saved(id);
                self.saved = Some(id);
                self.event(EventKind::Saved(id));
            }
            Err(e) => self.controls.note(format!("save failed: {e}")),
        }
    }

    fn step(&mut self, n: u32) {
        for _ in 0..n {
            let req = self.controls.step(self.held);
            self.diff();
            self.persist(req);
        }
    }

    fn load(&mut self, id: CheckpointId) -> Result<(), String> {
        let m = self.materializer().get(&self.store, id)?;
        self.controls
            .load(id, &m.checkpoint)
            .map_err(|e| format!("restore: {e:?}"))?;
        self.jumped(EventKind::Loaded(id));
        if m.diverged {
            self.controls.note(format!("{id} diverged on this build"));
        }
        Ok(())
    }

    fn materializer(&self) -> MutexGuard<'_, Materializer<Game>> {
        lock_materializer(&self.materializer)
    }

    fn seek(&mut self, id: CheckpointId, frames: u32) -> Result<(), String> {
        // At most one `get` under the lock; a keyframe replay runs here, off the lock.
        let prep = self.materializer().prepare_seek(&self.store, id, frames)?;
        let start = match prep {
            SeekPrep::Ready(start) => start,
            SeekPrep::Keyframes(plan) => {
                let built = plan.build(&content::Emerald::for_shells())?;
                let start = built.start();
                self.materializer().insert_keyframes(built);
                start
            }
        };
        let frames = start.frames;
        let seeked = start.run(content::Emerald::for_shells())?;
        self.controls = Controls::from_seek(seeked);
        self.jumped(EventKind::Seeked(id, frames));
        Ok(())
    }
}

impl Pane for NativePane {
    type Cmd = NativeCmd;
    type Status = Inspector;

    fn handle(&mut self, cmd: NativeCmd) {
        match cmd {
            NativeCmd::Keys(b) => self.held = b,
            NativeCmd::Mode(m) => {
                // A leftover `Step` never fires on a later pause.
                self.steps = 0;
                self.run = match m {
                    RunMode::Fast(n) => RunMode::Fast(n.clamp(2, 16)),
                    m => m,
                };
            }
            NativeCmd::Step(n) => {
                self.run = RunMode::Pause;
                self.steps = self.steps.saturating_add(n);
            }
            NativeCmd::Rewind => {
                let before = self.world().frame();
                if self.controls.rewind() {
                    let back = before - self.world().frame();
                    self.jumped(EventKind::Rewound(back));
                }
            }
            NativeCmd::Save => {
                let req = self.controls.save();
                self.persist(req);
            }
            NativeCmd::Load(id) => {
                if let Err(e) = self.load(id) {
                    self.controls.note(format!("load {id} failed: {e}"));
                }
            }
            NativeCmd::Seek { id, frames } => {
                if let Err(e) = self.seek(id, frames) {
                    self.controls.note(format!("seek {id} failed: {e}"));
                }
            }
            NativeCmd::NewGame => {
                self.controls = Controls::new(content::Emerald::for_shells());
                self.jumped(EventKind::NewGame);
            }
        }
    }

    fn tick(&mut self) {
        let n = match self.run {
            RunMode::Pause => core::mem::take(&mut self.steps),
            RunMode::Play => 1,
            RunMode::Fast(n) => n.clamp(2, 16) as u32,
        };
        self.step(n);
    }

    fn framebuffer(&mut self) -> &[u16] {
        self.controls.session_mut().framebuffer()
    }

    fn status(&self) -> Inspector {
        let s = self.controls.session();
        let w = s.world();
        let mode = if w.in_battle() {
            "battle"
        } else if w.starter().is_some() {
            "starter"
        } else {
            "field"
        };
        Inspector {
            frame: w.frame(),
            map: w.map(),
            map_name: s.game().map(w.map()).name,
            hash: w.hash(),
            mode,
            capturable: w.capturable(),
            camera: w.camera(),
            run: self.run,
            parent: self.controls.parent(),
            trace_frames: self.controls.trace_frames(),
            pending_save: self.controls.pending(),
            last: self.controls.last().to_string(),
            ring_frames: self.controls.ring_frames(),
            events: self.events.iter().cloned().collect(),
            saved: self.saved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDLE: Seen = Seen {
        map: MapId(1),
        script: false,
        battle: false,
        logs: 0,
    };

    #[test]
    fn no_change_no_event() {
        assert_eq!(changes(IDLE, IDLE, ["a"].into_iter()), []);
    }

    #[test]
    fn every_edge_is_an_event_in_order() {
        let now = Seen {
            map: MapId(7),
            script: true,
            battle: true,
            logs: 2,
        };
        assert_eq!(
            changes(IDLE, now, ["old", "x", "y"].into_iter()),
            [
                EventKind::MapEntered(MapId(7)),
                EventKind::ScriptStarted,
                EventKind::BattleStarted,
                EventKind::Log("x"),
                EventKind::Log("y"),
            ]
        );
        assert_eq!(
            changes(now, IDLE.with_logs(2), [].into_iter()),
            [
                EventKind::MapEntered(MapId(1)),
                EventKind::ScriptEnded,
                EventKind::BattleEnded,
            ]
        );
    }

    #[test]
    fn more_new_lines_than_kept_gives_the_kept_ones() {
        let now = IDLE.with_logs(40);
        let kept = ["a", "b", "c"];
        let got = changes(IDLE, now, kept.into_iter());
        assert_eq!(got, kept.map(EventKind::Log));
    }

    impl Seen {
        const fn with_logs(self, logs: u32) -> Self {
            Self { logs, ..self }
        }
    }
}
