//! The driver: expands a macro to inputs on a live session (Task A).
//!
//! Adapted from the scenario recorder (`game_native::scenario`, decision 1): BFS walking
//! re-planned every frame, the tap rule, the battle cursor walk. A macro always ends on the
//! first frame where `World::capturable()` is `Ok` and the player is settled; a script,
//! battle or starter screen that starts during a walk ends the walk and is resolved with the
//! policy and the macro's `Opts`.

use crate::balance::BattleTracker;
use crate::config::Limits;
use crate::guard::{self, Panicked};
use crate::macros;
use crate::model::{FindingKind, Macro, MacroKind, Opts, SimSummary};
use crate::observe::{Flag, NoObserver, StepObserver};
use crate::path::{self, Grid, Terrain, live_grid};
use crate::policy::Policy;
use engine::data::{Behavior, Dir, GameData, MapId, ScriptId};
use engine::mon::BattleTables;
use engine::script::Report;
use engine::world::Event;
use engine::{Buttons, World};
use game_native::Session;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, PartialEq, Debug)]
pub enum ExpandError {
    NoPath,
    NoEffect(&'static str),
    Timeout(&'static str),
    Panicked(Panicked),
    Flagged(Flag),
}

pub struct Expanded {
    pub keys: Vec<Buttons>,
    pub summary: SimSummary,
    /// The last script started during the expansion (`Event::ScriptStarted`, the frame that
    /// panicked included), for findings.
    pub last_script: Option<ScriptId>,
    /// Scripts' `Request::Report`s, the first of each (report, map), at most [`MAX_REPORTS`]
    /// (report-only: `FindingKind::Unsupported`).
    pub reports: Vec<Reported>,
    /// Report-only flags (`FindingKind::SlowFrame`) raised during the expansion, at most
    /// [`MAX_SLOW`]; they never end it.
    pub slow: Vec<Flag>,
    /// What the driver noticed beyond the macro itself, e.g. `"3 engine events dropped
    /// (EVENTS_MAX)"`.
    pub notes: Vec<String>,
}

/// Report-only flags kept per expansion.
pub const MAX_SLOW: usize = 16;
/// Reports kept per expansion.
pub const MAX_REPORTS: usize = 16;

/// A script's `Request::Report` during an expansion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reported {
    pub report: Report,
    /// The expansion's frames up to and including the reporting one.
    pub frames: u32,
    /// `World::map()` after the reporting frame: a report raised on a warp frame (a map
    /// script of the destination, or the warping script itself) is attributed to the map the
    /// player is on after the warp.
    pub map: MapId,
    pub pos: (i16, i16),
    /// The last script started by then (in this expansion).
    pub script: Option<ScriptId>,
}

/// Frames the player may hold one planned direction toward a cell the planner calls walkable
/// without moving before the driver flags a planner/engine divergence.
pub const STALL_FRAMES: u32 = 64;
/// Frames the player may wait on one cell for its way to clear (no path, or an object in the
/// planned cell) before the walk gives up with `NoPath`.
pub const BLOCKED_FRAMES: u32 = 240;
/// Frames after pressing A for the interaction's script to start.
const INTERACT_FRAMES: u32 = 4;

/// What drives battles and scripts: the policy's intents (the driver applies the tap rule).
pub trait Intents {
    fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons;
    /// [`Intents::battle`], never forgetting a move `is_hm` names (HM moves).
    fn battle_with(
        &mut self,
        w: &World,
        t: &BattleTables,
        opts: Opts,
        is_hm: &dyn Fn(engine::data::MoveId) -> bool,
    ) -> Buttons {
        let _ = is_hm;
        self.battle(w, t, opts)
    }
    fn script(&mut self, w: &World, opts: Opts) -> Buttons;
}

impl Intents for Policy {
    fn battle(&mut self, w: &World, t: &BattleTables, opts: Opts) -> Buttons {
        Policy::battle(self, w, t, opts)
    }
    fn battle_with(
        &mut self,
        w: &World,
        t: &BattleTables,
        opts: Opts,
        is_hm: &dyn Fn(engine::data::MoveId) -> bool,
    ) -> Buttons {
        Policy::battle_with(self, w, t, opts, is_hm)
    }
    fn script(&mut self, w: &World, opts: Opts) -> Buttons {
        Policy::script(self, w, opts)
    }
}

/// Plays `m` on `s` to the next capturable frame. On error, `keys`/`summary` so far are
/// returned for the repro.
#[allow(clippy::result_large_err)] // the partial expansion is the repro
pub fn expand<G: GameData>(
    s: &mut Session<G>,
    m: &Macro,
    policy: &mut Policy,
    obs: &mut dyn StepObserver<G>,
    limits: &Limits,
) -> Result<Expanded, (ExpandError, Expanded)> {
    expand_with(s, m, policy, obs, limits)
}

/// [`expand`] with any [`Intents`] (a test seam: fake policies).
#[allow(clippy::result_large_err)] // the partial expansion is the repro
pub fn expand_with<G: GameData>(
    s: &mut Session<G>,
    m: &Macro,
    intents: &mut dyn Intents,
    obs: &mut dyn StepObserver<G>,
    limits: &Limits,
) -> Result<Expanded, (ExpandError, Expanded)> {
    let mut d = Driver::new(s, intents, obs, limits, m.opts);
    let r = d.plan(m).and_then(|()| d.run(m.kind));
    let ex = d.finish();
    match r {
        Ok(()) => Ok(ex),
        Err(e) => Err((e, ex)),
    }
}

/// Root: a new game settled to its first capturable frame (`MacroKind::Settle`).
///
/// Panics if the new game never settles (default limits): that is a broken build, not an
/// exploration result.
pub fn new_game<G: GameData>(game: G) -> Session<G> {
    let mut s = Session::new(game);
    let settle = Macro {
        kind: MacroKind::Settle,
        opts: Opts::default(),
    };
    if let Err((e, _)) = expand_with(
        &mut s,
        &settle,
        &mut Policy::new(),
        &mut NoObserver,
        &Limits::default(),
    ) {
        panic!("a new game does not settle: {e:?}");
    }
    s
}

pub(crate) fn dir_key(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

/// Watches a walk for frames without moving.
#[derive(Default)]
struct Stall {
    pos: Option<(i16, i16)>,
    dir: Option<Dir>,
    /// Frames holding the same direction toward a cell the planner calls walkable.
    open: u32,
    /// Frames without a way (no path, or the planned cell occupied).
    blocked: u32,
}

impl Stall {
    /// `Some(NoPath)` after [`BLOCKED_FRAMES`] blocked frames on one cell; `Some(Flagged)`
    /// (the caller builds the flag) after [`STALL_FRAMES`] frames holding one direction toward
    /// a walkable cell without moving.
    fn tick(&mut self, pos: (i16, i16), dir: Option<Dir>, open: bool) -> Option<ExpandError> {
        if self.pos != Some(pos) {
            *self = Stall {
                pos: Some(pos),
                ..Stall::default()
            };
        }
        if self.dir != dir || !open {
            self.open = 0;
        }
        self.dir = dir;
        if open {
            self.open += 1;
        } else {
            self.blocked += 1;
        }
        if self.blocked > BLOCKED_FRAMES {
            Some(ExpandError::NoPath)
        } else if self.open > STALL_FRAMES {
            Some(ExpandError::NoEffect("stalled"))
        } else {
            None
        }
    }
}

/// A [`Driver::next_dir`] question: with the map's terrain (fixed by the map and the driver's
/// overrides), everything the answer depends on.
#[derive(PartialEq, Eq)]
struct DirQuery {
    map: MapId,
    from: (i16, i16),
    /// The player's elevation (the searches carry it).
    elevation: u8,
    to: (i16, i16),
    occupied: Vec<(i16, i16)>,
}

/// How a walk ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Walk {
    Arrived,
    /// A script, battle, starter screen or warp took over.
    Interrupted,
}

struct Driver<'a, G: GameData> {
    s: &'a mut Session<G>,
    intents: &'a mut dyn Intents,
    obs: &'a mut dyn StepObserver<G>,
    limits: &'a Limits,
    opts: Opts,
    keys: Vec<Buttons>,
    summary: SimSummary,
    last_script: Option<ScriptId>,
    reports: Vec<Reported>,
    /// New (report, map) pairs past [`MAX_REPORTS`], not kept.
    reports_dropped: u32,
    /// `World::events_dropped` at the start.
    dropped_start: u32,
    /// The battle's outcome from `Event::BattleEnded`, until the tracker finishes.
    ended: Option<u8>,
    /// The buttons of the last step.
    last: Buttons,
    /// The start snapshot's `setmetatile` overrides, valid until the map first changes.
    overrides: Vec<((i16, i16), u16)>,
    /// The planner's terrain of the live map under `overrides` (built by the first
    /// [`Driver::grid`], dropped with the overrides on a map change).
    terrain: RefCell<Option<Arc<Terrain>>>,
    /// The last [`Driver::next_dir`] question and its answer: a walk asks the same one on
    /// every frame of a step.
    dir_memo: Option<(DirQuery, Option<Dir>)>,
    slow: Vec<Flag>,
    notes: Vec<String>,
    map: MapId,
    log_start: u32,
    battle: Option<BattleTracker>,
    /// Frames stepped outside battles (`Limits::macro_frames`).
    nonbattle: u32,
    /// Frames of the current battle (`Limits::battle_frames`).
    battle_frames: u32,
}

impl<'a, G: GameData> Driver<'a, G> {
    fn new(
        s: &'a mut Session<G>,
        intents: &'a mut dyn Intents,
        obs: &'a mut dyn StepObserver<G>,
        limits: &'a Limits,
        opts: Opts,
    ) -> Self {
        let w = s.world();
        let (map, log_start, dropped_start) = (w.map(), w.log().count(), w.events_dropped());
        let overrides = w
            .snapshot(s.game())
            .map(|snap| snap.field.overrides)
            .unwrap_or_default();
        Driver {
            s,
            intents,
            obs,
            limits,
            opts,
            keys: Vec::new(),
            summary: SimSummary::default(),
            last_script: None,
            reports: Vec::new(),
            reports_dropped: 0,
            dropped_start,
            ended: None,
            last: Buttons::NONE,
            overrides,
            terrain: RefCell::new(None),
            dir_memo: None,
            slow: Vec::new(),
            notes: Vec::new(),
            map,
            log_start,
            battle: None,
            nonbattle: 0,
            battle_frames: 0,
        }
    }

    fn w(&self) -> &World {
        self.s.world()
    }

    fn finish(self) -> Expanded {
        let mut summary = self.summary;
        summary.frames = self.keys.len() as u32;
        summary.log_lines = self.s.world().log().count().wrapping_sub(self.log_start);
        let mut notes = self.notes;
        let dropped = self
            .s
            .world()
            .events_dropped()
            .wrapping_sub(self.dropped_start);
        summary.events_dropped = dropped;
        if dropped > 0 {
            notes.push(format!("{dropped} engine events dropped (EVENTS_MAX)"));
        }
        if self.reports_dropped > 0 {
            notes.push(format!(
                "{} script reports dropped (MAX_REPORTS)",
                self.reports_dropped
            ));
        }
        Expanded {
            keys: self.keys,
            summary,
            last_script: self.last_script,
            reports: self.reports,
            slow: self.slow,
            notes,
        }
    }

    /// The overrides that apply to the live map: the start snapshot's, cleared (latched off)
    /// on the first map change, since a new map starts without overrides.
    fn overrides(&self) -> &[((i16, i16), u16)] {
        &self.overrides
    }

    /// The planner's view of the live world ([`live_grid`]) with the cached terrain table.
    fn grid(&self) -> Grid<'_, G> {
        let cached = self.terrain.borrow().clone();
        let grid = live_grid(self.s.game(), self.w(), self.overrides()).with_terrain(cached);
        *self.terrain.borrow_mut() = Some(grid.terrain().clone());
        grid
    }

    /// The first step toward `to` from the live world, as [`path::next_dir`] (tall grass only
    /// when there is no other way), memoized on its question.
    fn next_dir(&mut self, to: (i16, i16)) -> Option<Dir> {
        let (q, dir) = {
            let grid = self.grid();
            let q = DirQuery {
                map: self.w().map(),
                from: self.w().objects().player().pos(),
                elevation: self.w().objects().player().elevation,
                to,
                occupied: grid.occupied_cells().to_vec(),
            };
            if let Some((k, d)) = &self.dir_memo
                && *k == q
            {
                return *d;
            }
            let d =
                (grid.first_dir(q.from, to, true)).or_else(|| grid.first_dir(q.from, to, false));
            (q, d)
        };
        self.dir_memo = Some((q, dir));
        dir
    }

    /// Up-front planning: `NoPath` when `m` is not applicable to a capturable start.
    fn plan(&mut self, m: &Macro) -> Result<(), ExpandError> {
        if matches!(
            m.kind,
            MacroKind::Settle
                | MacroKind::Wait { .. }
                | MacroKind::Face { .. }
                | MacroKind::Step { .. }
                | MacroKind::Push { .. }
                | MacroKind::Interact
                | MacroKind::Press { .. }
        ) {
            return Ok(());
        }
        let Ok(snap) = self.w().snapshot(self.s.game()) else {
            return Ok(());
        };
        let reach = path::reach(self.s.game(), &snap);
        if macros::applicable(self.s.game(), &snap, &reach, m) {
            Ok(())
        } else {
            Err(ExpandError::NoPath)
        }
    }

    /// One frame: guarded step, observers, summary.
    fn press(&mut self, keys: Buttons) -> Result<(), ExpandError> {
        if self.w().in_battle() {
            self.battle_frames += 1;
            if self.battle_frames > self.limits.battle_frames {
                return Err(ExpandError::Timeout("battle"));
            }
        } else {
            self.nonbattle += 1;
            if self.nonbattle > self.limits.macro_frames {
                return Err(ExpandError::Timeout("macro"));
            }
        }
        let t = Instant::now();
        let stepped = guard::step_guarded(self.s, keys);
        let ns = t.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.keys.push(keys);
        self.last = keys;
        if stepped.is_err() {
            // The panicking frame's scripts (read-only: the session is dropped after this).
            for e in self.s.world().events() {
                if let &Event::ScriptStarted(id) = e {
                    self.last_script = Some(id);
                }
            }
        }
        stepped.map_err(ExpandError::Panicked)?;
        let flag = self.obs.after_step(self.s, keys, ns);
        self.observe(ns);
        match flag {
            // Report-only (`observe` module docs): recorded, never fatal.
            Some(f) if f.kind == FindingKind::SlowFrame => {
                if self.slow.len() < MAX_SLOW {
                    self.slow.push(f);
                }
                Ok(())
            }
            Some(f) => Err(ExpandError::Flagged(f)),
            None => Ok(()),
        }
    }

    /// The summary from this frame's `World::events()`: maps entered, scripts started,
    /// battles (tracked from `BattleStarted` to the frame the field is back, with the
    /// `BattleEnded` outcome) and reports.
    fn observe(&mut self, ns: u64) {
        let sum = &mut self.summary;
        sum.max_step_ns = sum.max_step_ns.max(ns);
        let w = self.s.world();
        let (map, pos) = (w.map(), w.objects().player().pos());
        if map != self.map {
            self.map = map;
            self.overrides.clear();
            self.terrain.get_mut().take();
            self.dir_memo = None;
        }
        sum.cells.insert((map, pos.0, pos.1));
        let mut started = false;
        for &e in w.events() {
            match e {
                Event::MapEntered(m) => {
                    if sum.maps.last() != Some(&m) {
                        sum.maps.push(m);
                    }
                }
                Event::ScriptStarted(id) => {
                    sum.scripts.insert(id);
                    self.last_script = Some(id);
                }
                Event::BattleStarted => started = true,
                Event::BattleEnded(o) => self.ended = Some(o as u8),
                Event::Report(report) => {
                    let new = !self
                        .reports
                        .iter()
                        .any(|r| r.report == report && r.map == map);
                    if new && self.reports.len() >= MAX_REPORTS {
                        self.reports_dropped += 1;
                    } else if new {
                        self.reports.push(Reported {
                            report,
                            frames: self.keys.len() as u32,
                            map,
                            pos,
                            script: self.last_script,
                        });
                    }
                }
                // Milestone 9 screens, saves and evolutions: not summarized yet.
                Event::ScreenOpened(_)
                | Event::ScreenClosed
                | Event::Saved { .. }
                | Event::Evolved(_) => {}
            }
        }
        if started {
            self.battle_frames = 0;
            self.ended = None;
            if self.battle.is_none() {
                self.battle = BattleTracker::start(w);
                if self.battle.is_some() {
                    sum.battle_maps.push(map);
                }
            }
        } else if w.in_battle() {
            if let Some(t) = &mut self.battle {
                t.observe(w);
            }
        } else if let Some(t) = self.battle.take() {
            sum.battles.push(t.finish(w, self.ended.take()));
        }
    }

    /// Presses `keys` as a new press: a frame of nothing first if something is held, and
    /// nothing while a battle or warp has the screen black (`scenario.rs` tap rule).
    fn tap(&mut self, keys: Buttons) -> Result<(), ExpandError> {
        let w = self.w();
        let black = (w.in_battle() || w.transition_active()) && self.s.frame().is_black();
        if self.last != Buttons::NONE || black {
            self.press(Buttons::NONE)
        } else {
            self.press(keys)
        }
    }

    /// A script, battle, starter screen or warp owns the frame.
    fn interrupted(&self) -> bool {
        let w = self.w();
        w.script_running() || w.in_battle() || w.starter().is_some() || w.transition_active()
    }

    /// Resolves whatever runs (battles with the policy, scripts and the starter screen with
    /// the policy) until the first capturable frame with the player settled. Anything else
    /// is waited out with no input: a crossing's upload (`NotCapturable::StaleView`) ends by
    /// itself within a few frames (Milestone 8 Task F).
    fn resolve(&mut self) -> Result<(), ExpandError> {
        let mut idle = 0u32;
        loop {
            let w = self.s.world();
            if w.capturable().is_ok() && w.objects().player().is_settled() {
                return Ok(());
            }
            if w.in_battle() {
                idle = 0;
                let game = self.s.game();
                let k = (self.intents)
                    .battle_with(w, game.battle(), self.opts, &|m| game.is_hm_move(m));
                self.tap(k)?;
            } else if w.script_running() || w.starter().is_some() {
                idle = 0;
                let k = self.intents.script(w, self.opts);
                self.tap(k)?;
            } else {
                idle += 1;
                if idle > self.limits.settle_frames {
                    return Err(ExpandError::Timeout("settle"));
                }
                self.press(Buttons::NONE)?;
            }
        }
    }

    /// Walks to `to` (re-planned every frame), finishing the last step.
    fn walk_to(&mut self, to: (i16, i16)) -> Result<Walk, ExpandError> {
        let mut stall = Stall::default();
        loop {
            if self.interrupted() {
                return Ok(Walk::Interrupted);
            }
            let p = self.w().objects().player();
            let (pos, settled) = (p.pos(), p.is_settled());
            if pos == to {
                if settled {
                    return Ok(Walk::Arrived);
                }
                self.press(Buttons::NONE)?;
                continue;
            }
            let dir = self.next_dir(to);
            self.step_toward(&mut stall, dir, to)?;
        }
    }

    /// Holds `dir` (nothing if `None`) for a frame, watching for a stalled walk.
    fn step_toward(
        &mut self,
        stall: &mut Stall,
        dir: Option<Dir>,
        to: (i16, i16),
    ) -> Result<(), ExpandError> {
        let pos = self.w().objects().player().pos();
        let ahead = dir.and_then(|d| self.ahead_walkable(d));
        if let Some(e) = stall.tick(pos, dir, ahead.is_some()) {
            return Err(match e {
                ExpandError::NoPath => ExpandError::NoPath,
                _ => ExpandError::Flagged(Flag {
                    kind: FindingKind::PlannerDivergence,
                    message: format!(
                        "map {:#x}: held {:?} at {pos:?} toward walkable {:?} (going to {to:?}) \
                         for {STALL_FRAMES} frames without moving",
                        self.w().map().0,
                        dir.expect("a direction"),
                        ahead.expect("walkable"),
                    ),
                }),
            });
        }
        self.press(dir.map_or(Buttons::NONE, dir_key))
    }

    /// The cell a step toward `d` lands on (over a ledge), if the planner calls it walkable
    /// right now.
    fn ahead_walkable(&self, d: Dir) -> Option<(i16, i16)> {
        let w = self.w();
        let grid = self.grid();
        let (dx, dy) = d.delta();
        let p = w.objects().player().pos();
        let mut ahead = (p.0 + dx, p.1 + dy);
        if grid.behavior(ahead) == Behavior::Ledge(d) {
            ahead = (ahead.0 + dx, ahead.1 + dy);
        }
        grid.walkable(ahead, false).then_some(ahead)
    }

    /// Settled on the right cell: turn to `d`, then press A until a script starts.
    fn face_and_press(&mut self, d: Dir, what: &'static str) -> Result<(), ExpandError> {
        let p = self.w().objects().player();
        if p.facing != d {
            return self.press(dir_key(d));
        }
        if !p.is_settled() || self.last != Buttons::NONE {
            return self.press(Buttons::NONE);
        }
        self.press(Buttons::A)?;
        for _ in 0..INTERACT_FRAMES {
            if self.interrupted() {
                return Ok(());
            }
            self.press(Buttons::NONE)?;
        }
        if self.interrupted() {
            Ok(())
        } else {
            Err(ExpandError::NoEffect(what))
        }
    }

    fn run(&mut self, kind: MacroKind) -> Result<(), ExpandError> {
        match kind {
            MacroKind::Settle => {}
            MacroKind::WalkTo { x, y } => {
                self.walk_to((x, y))?;
            }
            MacroKind::Enter { x, y, dir } => self.enter((x, y), dir)?,
            MacroKind::Cross { x, y, dir } => self.cross((x, y), dir)?,
            MacroKind::Talk { local_id } => self.talk(local_id)?,
            MacroKind::Inspect { x, y, dir } => {
                let (dx, dy) = dir.delta();
                if self.walk_to((x - dx, y - dy))? == Walk::Arrived {
                    while !self.interrupted() {
                        self.face_and_press(dir, "inspect")?;
                    }
                }
            }
            MacroKind::Grass { x, y } => self.grass((x, y))?,
            MacroKind::Wait { frames } => {
                for _ in 0..frames {
                    if self.interrupted() {
                        break;
                    }
                    self.press(Buttons::NONE)?;
                }
            }
            MacroKind::Face { dir } => self.face(dir)?,
            MacroKind::Step { dir } => {
                self.resolve()?;
                let at = self.w().objects().player().pos();
                let mut n = 0;
                while self.w().objects().player().pos() == at && !self.interrupted() {
                    n += 1;
                    if n > self.limits.enter_frames {
                        return Err(ExpandError::NoEffect("step: never moved"));
                    }
                    self.press(dir_key(dir))?;
                }
                self.press(Buttons::NONE)?;
            }
            MacroKind::Push { dir } => {
                self.resolve()?;
                let from = self.w().map();
                let mut n = 0;
                while self.w().map() == from && !self.interrupted() {
                    n += 1;
                    if n > self.limits.enter_frames {
                        return Err(ExpandError::NoEffect("push: the map never changed"));
                    }
                    self.press(dir_key(dir))?;
                }
            }
            MacroKind::Interact => {
                self.resolve()?;
                if self.last != Buttons::NONE {
                    self.press(Buttons::NONE)?;
                }
                self.press(Buttons::A)?;
                for _ in 0..INTERACT_FRAMES {
                    if self.interrupted() {
                        break;
                    }
                    self.press(Buttons::NONE)?;
                }
                if !self.interrupted() {
                    return Err(ExpandError::NoEffect("interact"));
                }
            }
            MacroKind::Press { keys, frames } => {
                let keys = Buttons(keys);
                for _ in 0..frames {
                    self.press(keys)?;
                }
            }
        }
        self.resolve()
    }

    /// Settled, turns in place to `d`: holds `d` until the player faces it (a tap turns
    /// without stepping), then releases. Moving off the cell is `NoEffect`.
    fn face(&mut self, d: Dir) -> Result<(), ExpandError> {
        self.resolve()?;
        let at = self.w().objects().player().pos();
        let mut n = 0;
        while self.w().objects().player().facing != d {
            n += 1;
            if n > self.limits.enter_frames {
                return Err(ExpandError::NoEffect("face: never turned"));
            }
            self.press(dir_key(d))?;
        }
        self.press(Buttons::NONE)?;
        self.resolve()?;
        if self.w().objects().player().pos() != at {
            return Err(ExpandError::NoEffect("face: moved while turning"));
        }
        Ok(())
    }

    fn enter(&mut self, at: (i16, i16), dir: Dir) -> Result<(), ExpandError> {
        let grid = self.grid();
        let approach = macros::enter_approach(grid.behavior(at), at, dir);
        if self.walk_to(approach)? == Walk::Interrupted {
            return Ok(());
        }
        let mut n = 0;
        while !self.interrupted() {
            n += 1;
            if n > self.limits.enter_frames {
                return Err(ExpandError::NoEffect("enter: no warp"));
            }
            self.press(dir_key(dir))?;
        }
        Ok(())
    }

    fn cross(&mut self, at: (i16, i16), dir: Dir) -> Result<(), ExpandError> {
        if self.walk_to(at)? == Walk::Interrupted {
            return Ok(());
        }
        let map = self.w().map();
        let mut n = 0;
        while self.w().map() == map {
            if self.interrupted() {
                return Ok(());
            }
            n += 1;
            if n > self.limits.enter_frames {
                return Err(ExpandError::NoEffect("cross: no map change"));
            }
            self.press(dir_key(dir))?;
        }
        Ok(())
    }

    fn talk(&mut self, local_id: u8) -> Result<(), ExpandError> {
        let mut stall = Stall::default();
        while !self.interrupted() {
            let w = self.w();
            let Some(o) = w.objects().get(local_id) else {
                return Err(ExpandError::NoEffect("talk: object gone"));
            };
            let grid = self.grid();
            let p = w.objects().player();
            let approaches = macros::talk_approaches(&grid, o.pos());
            if let Some(&(_, d)) = approaches.iter().find(|(c, _)| *c == p.pos()) {
                // A wandering object mid-step is not where its cell says: wait for it.
                if p.is_settled() && o.is_settled() {
                    self.face_and_press(d, "talk")?;
                } else {
                    self.press(Buttons::NONE)?;
                }
                continue;
            }
            // The first approach the player can walk to (the object may wander).
            let (from, at) = (p.pos(), o.pos());
            let dir = approaches.iter().find_map(|&(c, _)| {
                grid.first_dir(from, c, true)
                    .or_else(|| grid.first_dir(from, c, false))
            });
            self.step_toward(&mut stall, dir, at)?;
        }
        Ok(())
    }

    fn grass(&mut self, at: (i16, i16)) -> Result<(), ExpandError> {
        let start = self.keys.len() as u32;
        if self.walk_to(at)? == Walk::Interrupted {
            return Ok(());
        }
        let other = {
            let grid = self.grid();
            Dir::ALL.into_iter().find_map(|d| {
                let (dx, dy) = d.delta();
                let c = (at.0 + dx, at.1 + dy);
                let ledge = matches!(grid.behavior(c), Behavior::Ledge(_));
                (grid.walkable(c, false) && !ledge).then_some(c)
            })
        };
        let Some(other) = other else {
            return Err(ExpandError::NoEffect("grass: no walkable neighbour"));
        };
        let mut to = other;
        while !self.interrupted() {
            if self.keys.len() as u32 - start > self.limits.grass_frames {
                return Err(ExpandError::NoEffect("grass: no encounter"));
            }
            let p = self.w().objects().player();
            if p.pos() == to && p.is_settled() {
                to = if to == at { other } else { at };
            }
            let dir = self.next_dir(to);
            self.press(dir.map_or(Buttons::NONE, dir_key))?;
        }
        Ok(())
    }
}
