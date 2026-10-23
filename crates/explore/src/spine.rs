//! The spine: one authored main path per flavour, as high-level [`Step`]s that the macro
//! driver expands into inputs (`docs/plans/2026-10-23-explorer-spine.md` §2).
//!
//! [`run`] plays the build's steps ([`steps`]) from a new game on one session, with the
//! detectors on. Every step ends on a capturable frame, so the state after each step is a
//! [`Position`]: where branches start, and where they rejoin.

pub mod emerald;
pub mod silice;
pub mod stub;

use crate::config::{Config, KeyConfig, Weights};
use crate::detect::Detectors;
use crate::goals;
use crate::key;
use crate::model::{FindingDraft, Macro, MacroKind, Opts, StepResult, Stop};
use crate::observe::StepObserver;
use crate::sim::{Live, StepFail};
use engine::checkpoint::{Checkpoint, WorldSnapshot};
use engine::data::{Dir, FlagId, GameData, MapId, ScriptId, VarId};
use game_native::trace::Run;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The map field of a step that runs on the new game's map, whatever its id (the stub's).
pub const NEW_GAME_MAP: MapId = MapId(u16::MAX);

/// How wild battles go from here on (sticky).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BattlePlan {
    /// The policy's first damaging move (the default).
    Fight,
    /// The move of the best expected damage (power, STAB, type matchups); wild battles fought.
    Best,
    /// RUN from wild battles (never the first battle, never trainers).
    RunWild,
    /// [`BattlePlan::Best`], learning the level-up moves that hit harder than the weakest
    /// known one (`Opts::learn`).
    BestLearn,
}

/// One step of the spine. Map fields are asserted before the step plays (a wrong map fails
/// the step at once); [`NEW_GAME_MAP`] stands for the new game's map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Walk to (x, y).
    Goto { map: MapId, x: i16, y: i16 },
    /// Walk to (x, y) one step at a time on a shortest path that never enters the `avoid`
    /// cells (trainers' sight lines), planned again after each step.
    GotoAvoiding {
        map: MapId,
        x: i16,
        y: i16,
        avoid: &'static [(i16, i16)],
    },
    /// Walk to warp cell (x, y) and enter it moving `dir`; the map must change.
    Enter {
        map: MapId,
        x: i16,
        y: i16,
        dir: Dir,
    },
    /// Walk to edge cell (x, y) and hold `dir` until the map changes.
    Cross {
        map: MapId,
        x: i16,
        y: i16,
        dir: Dir,
    },
    /// Walk next to object `local_id`, face it, press A, resolve.
    Talk { map: MapId, local_id: u8 },
    /// Stand on (x, y), face `dir`, press A, resolve (signs, counters, hidden items, beds).
    Use {
        map: MapId,
        x: i16,
        y: i16,
        dir: Dir,
    },
    /// Turn in place.
    Face(Dir),
    /// One step toward `dir` (a bump into a scripted cell, or across a cell a script opened).
    Walk(Dir),
    /// Hold `dir` from the current cell until the map changes; the new map must be `to`.
    Exit { dir: Dir, to: MapId },
    /// Press A on what is ahead, resolve.
    Interact,
    /// Hold `Buttons(keys)` for `frames` frames, resolve.
    Press(u16, u16),
    /// Press nothing for this many frames, resolve.
    Wait(u16),
    /// Idle a few frames ([`WAIT_SCRIPT_FRAMES`]: a frame script starts), then resolve
    /// whatever runs.
    WaitScript,
    /// Idle until object `local_id` stands on (x, y) (a wandering object's timing; at most
    /// [`WAIT_FOR_FRAMES`]).
    WaitFor { local_id: u8, x: i16, y: i16 },
    /// Pace on tall-grass cell (x, y) until a wild battle starts; the battle per [`BattlePlan`].
    Grass { map: MapId, x: i16, y: i16 },
    /// [`Step::Grass`] (fighting) until the lead's level is at least `level`.
    Train {
        map: MapId,
        x: i16,
        y: i16,
        level: u8,
    },
    /// Sticky from here on.
    Battle(BattlePlan),
    /// The yes/no answer (0 yes, 1 no) and the starter ball for the **next** playing step.
    Choose(u8),
    /// The named goal holds now: one of the config's act, else of the build's later acts (the
    /// spine runs on past its act's end).
    Expect(&'static str),
    /// Labels the current position (a branch origin, an exported node).
    Checkpoint(&'static str),
    /// Steps over an engine gap (`docs/plans/2026-10-23-emerald-parity.md` §2, "Spine
    /// blockers"): writes the state the missing feature would have produced, so the spine goes
    /// on and later gaps get recorded too. Explorer-side only; fixing the gap deletes the step.
    Bypass(&'static Bypass),
}

/// What a [`Step::Bypass`] writes: flags, vars and a warp, outside the game's rules. It plays
/// no input, so a finding past the spine's first bypass does not reproduce from a new game
/// (its origin checkpoint does).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bypass {
    /// The blocker's id in the plan's "Spine blockers" table.
    pub issue: &'static str,
    /// The decomp features the engine lacks there (script commands, specials, scripts): the
    /// parity ledger reads these literals (`parity::story::bypassed`), so keep them right
    /// after `issue`.
    pub features: &'static [&'static str],
    pub set: &'static [FlagId],
    pub clear: &'static [FlagId],
    /// `(var, value)`; 0 clears.
    pub vars: &'static [(VarId, u16)],
    /// `(map, x, y)`: a debug warp onto that cell (`World::debug_warp`), waited out.
    pub warp: Option<(MapId, i16, i16)>,
}

impl std::fmt::Debug for Bypass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.issue)
    }
}

impl Bypass {
    /// No change, for `..Bypass::NONE`.
    pub const NONE: Bypass = Bypass {
        issue: "",
        features: &[],
        set: &[],
        clear: &[],
        vars: &[],
        warp: None,
    };
}

impl Step {
    /// The step's map, if it names one.
    pub fn map(&self) -> Option<MapId> {
        match *self {
            Step::Goto { map, .. }
            | Step::GotoAvoiding { map, .. }
            | Step::Enter { map, .. }
            | Step::Cross { map, .. }
            | Step::Talk { map, .. }
            | Step::Use { map, .. }
            | Step::Grass { map, .. }
            | Step::Train { map, .. } => Some(map),
            Step::Bypass(_) => None,
            _ => None,
        }
    }

    /// Whether the step needs the player's exact cell and facing (recovery walks back to them).
    pub fn needs_cell(&self) -> bool {
        matches!(
            self,
            Step::Face(_)
                | Step::Walk(_)
                | Step::Exit { .. }
                | Step::Interact
                | Step::Press(..)
                | Step::Wait(_)
                | Step::WaitScript
                | Step::WaitFor { .. }
        )
    }

    /// Whether the step plays inputs (the others only set options or check).
    pub fn plays(&self) -> bool {
        !matches!(
            self,
            Step::Battle(_) | Step::Choose(_) | Step::Expect(_) | Step::Checkpoint(_)
        )
    }
}

/// The build's spine ([`content::FLAVOR`]).
pub fn steps() -> &'static [Step] {
    match content::FLAVOR {
        content::Flavor::Stub => stub::STEPS,
        content::Flavor::Emerald => emerald::STEPS,
        content::Flavor::Silice => silice::STEPS,
    }
}

/// The option state between steps: the sticky battle plan and a pending `Choose`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct StepOpts {
    pub run_wild: bool,
    /// [`BattlePlan::Best`].
    pub best_move: bool,
    /// [`BattlePlan::BestLearn`].
    pub learn: bool,
    /// Set by `Choose`, used (and cleared) by the next playing step.
    pub choice: Option<u8>,
}

/// The state after a spine step.
#[derive(Clone, Debug)]
pub struct Position {
    /// Steps done: 0 is the settled new game, `i` the end of step `i - 1`.
    pub index: usize,
    /// The label of the last `Checkpoint` step at this position.
    pub label: Option<&'static str>,
    /// The label of the last checkpoint at or before this position (a finding's segment).
    pub segment: Option<&'static str>,
    /// Frames from the new game.
    pub frame: u64,
    pub end: Arc<Checkpoint>,
    pub opts: StepOpts,
}

impl Position {
    pub fn map(&self) -> MapId {
        self.end.world.field.map
    }

    pub fn pos(&self) -> (i16, i16) {
        self.end.world.objects.objs[0].pos
    }

    pub fn facing(&self) -> Dir {
        self.end.world.objects.objs[0].facing
    }
}

/// What the spine (or a branch) touched: the novelty baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub maps: BTreeSet<MapId>,
    /// Map changes (from, to): warps and crossings taken.
    pub edges: BTreeSet<(MapId, MapId)>,
    pub cells: BTreeSet<(MapId, i16, i16)>,
    /// Coarse blocks (`KeyConfig::cell_shift`) stood in.
    pub blocks: BTreeSet<(MapId, i16, i16)>,
    pub scripts: BTreeSet<ScriptId>,
    /// Story flags ever set.
    pub flags: BTreeSet<u16>,
    /// Story var values ever held (non-zero).
    pub var_values: BTreeSet<(u16, u16)>,
    /// Maps with a wild battle fought.
    pub wild_maps: BTreeSet<MapId>,
    /// Maps with a wild battle run from.
    pub ran_maps: BTreeSet<MapId>,
}

/// Counts of new things.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gain {
    pub maps: u32,
    pub scripts: u32,
    pub flags: u32,
    pub var_values: u32,
    pub cells: u32,
}

impl Gain {
    pub fn total(&self) -> u32 {
        self.maps + self.scripts + self.flags + self.var_values + self.cells
    }

    pub fn points(&self, w: &Weights) -> i64 {
        i64::from(self.maps) * w.map
            + i64::from(self.scripts) * w.script
            + i64::from(self.flags) * w.flag
            + i64::from(self.var_values) * w.var_value
            + i64::from(self.cells) * w.cell
    }
}

impl Coverage {
    /// Adds a step played from `from` (the map it started on).
    pub fn add_step(&mut self, from: MapId, st: &StepResult, cfg: &KeyConfig) {
        let s = &st.summary;
        self.maps.insert(from);
        let mut prev = from;
        for &m in &s.maps {
            self.maps.insert(m);
            if m != prev {
                self.edges.insert((prev, m));
            }
            prev = m;
        }
        let shift = cfg.cell_shift.min(14);
        for &(m, x, y) in &s.cells {
            self.cells.insert((m, x, y));
            self.blocks.insert((m, x >> shift, y >> shift));
        }
        self.scripts.extend(s.scripts.iter().copied());
        for (b, &m) in s.battles.iter().zip(&s.battle_maps) {
            if b.kind != 2 {
                if b.outcome == Some(engine::battle::BattleOutcome::Ran as u8) {
                    self.ran_maps.insert(m);
                } else {
                    self.wild_maps.insert(m);
                }
            }
        }
        self.add_state(&st.end.world, cfg);
    }

    /// Adds the story flags and var values of a state.
    pub fn add_state(&mut self, w: &WorldSnapshot, cfg: &KeyConfig) {
        self.maps.insert(w.field.map);
        for &f in &w.vars.flags {
            if key::story_flag(f, cfg) {
                self.flags.insert(f.0);
            }
        }
        for &(v, value) in &w.vars.vars {
            if value != 0 && key::story_var(v.0, cfg) {
                self.var_values.insert((v.0, value));
            }
        }
    }

    /// What `self` has that `base` has not.
    pub fn gain_over(&self, base: &Coverage) -> Gain {
        Gain {
            maps: self.maps.difference(&base.maps).count() as u32,
            scripts: self.scripts.difference(&base.scripts).count() as u32,
            flags: self.flags.difference(&base.flags).count() as u32,
            var_values: self.var_values.difference(&base.var_values).count() as u32,
            cells: self.cells.difference(&base.cells).count() as u32,
        }
    }
}

/// The progress state a branch must leave as it found it to rejoin the spine (§3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Story vars (non-zero), ascending.
    pub vars: Vec<(u16, u16)>,
    /// The spine flags set ([`SpineRun::spine_flags`]), ascending.
    pub flags: Vec<u16>,
    pub templates: u64,
    pub key_items: u64,
    pub respawn: u8,
    /// A mon with HP left (or no party at all).
    pub usable: bool,
}

/// Gen3 trainer types that look for the player (`TRAINER_TYPE_NORMAL`, `_BURIED`).
const TRAINER_TYPES_LOOKING: [u8; 2] = [1, 3];

impl Progress {
    /// The progress state of `w`. Template deltas of trainers that look for the player are
    /// left out: a trainer that spots the player stays where it stopped for the rest of the
    /// visit (Gen3 `OverrideTemplateCoordsForObjectEvent`), so which ones moved depends on the
    /// path a walk took past their sight lines, not on progress (their defeat flags are not
    /// spine flags either).
    pub fn of<G: GameData>(
        game: &G,
        w: &WorldSnapshot,
        spine_flags: &BTreeSet<u16>,
        cfg: &KeyConfig,
    ) -> Progress {
        let objects = game.map(w.field.map).objects;
        let looks = |local: u8| {
            (objects.iter())
                .any(|o| o.local_id == local && TRAINER_TYPES_LOOKING.contains(&o.trainer_type))
        };
        let templates: Vec<_> = (w.templates.iter())
            .filter(|t| !looks(t.local_id))
            .copied()
            .collect();
        Progress {
            vars: (w.vars.vars.iter())
                .filter(|(v, value)| *value != 0 && key::story_var(v.0, cfg))
                .map(|(v, value)| (v.0, *value))
                .collect(),
            flags: (w.vars.flags.iter())
                .filter(|f| spine_flags.contains(&f.0))
                .map(|f| f.0)
                .collect(),
            templates: key::templates_digest(&templates),
            key_items: key::key_items_digest(&w.bag),
            respawn: w.respawn,
            usable: w.party.is_empty() || w.party.iter().any(|m| m.hp > 0),
        }
    }

    /// What differs from `other`, in words (empty when equal).
    pub fn diff(&self, other: &Progress) -> String {
        let mut out = Vec::new();
        let vars: BTreeSet<_> = self.vars.iter().collect();
        let ovars: BTreeSet<_> = other.vars.iter().collect();
        for (v, x) in ovars.symmetric_difference(&vars).take(4) {
            out.push(format!("var {v:#x}={x}"));
        }
        let flags: BTreeSet<_> = self.flags.iter().collect();
        let oflags: BTreeSet<_> = other.flags.iter().collect();
        for f in oflags.symmetric_difference(&flags).take(4) {
            out.push(format!("flag {f:#x}"));
        }
        if self.templates != other.templates {
            out.push("object templates".into());
        }
        if self.key_items != other.key_items {
            out.push("key items".into());
        }
        if self.respawn != other.respawn {
            out.push("respawn".into());
        }
        if self.usable != other.usable {
            out.push("party usable".into());
        }
        out.join(", ")
    }
}

/// Why the spine stopped before its end.
#[derive(Clone, Debug)]
pub struct SpineError {
    /// The failing step's index.
    pub step: usize,
    pub message: String,
    /// The finding, when a detector or a panic stopped it.
    pub finding: Option<FindingDraft>,
}

impl std::fmt::Display for SpineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "spine step {}: {}", self.step, self.message)
    }
}

/// A spine run.
#[derive(Clone, Debug)]
pub struct SpineRun {
    pub steps: &'static [Step],
    /// `positions[i]`: after `i` steps (0: the settled new game). As many as the steps done + 1.
    pub positions: Vec<Position>,
    /// Every input from the new game.
    pub inputs: Vec<Run>,
    pub frames: u64,
    pub coverage: Coverage,
    /// Story flags the spine sets and never clears ([`Progress`]).
    pub spine_flags: BTreeSet<u16>,
    /// Report-only findings along the spine (tails from the new game).
    pub reports: Vec<FindingDraft>,
    pub notes: Vec<String>,
    pub error: Option<SpineError>,
    /// The macros played, with the step each belongs to.
    pub played: Vec<(usize, Macro)>,
}

impl SpineRun {
    /// The position labelled `label`.
    pub fn labelled(&self, label: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.label == Some(label))
    }

    /// The labels, in order.
    pub fn labels(&self) -> Vec<&'static str> {
        self.positions.iter().filter_map(|p| p.label).collect()
    }

    /// Whether every step ran.
    pub fn complete(&self) -> bool {
        self.error.is_none() && self.positions.len() == self.steps.len() + 1
    }

    /// The [`Step::Bypass`] steps played: `(step index, issue)`, in order.
    pub fn bypasses(&self) -> Vec<(usize, &'static str)> {
        (self.steps.iter().enumerate())
            .take(self.positions.len().saturating_sub(1))
            .filter_map(|(i, s)| match s {
                Step::Bypass(b) => Some((i, b.issue)),
                _ => None,
            })
            .collect()
    }

    /// The frame of the first bypass played: inputs reproduce the spine only before it.
    pub fn first_bypass_frame(&self) -> Option<u64> {
        let (i, _) = *self.bypasses().first()?;
        Some(self.positions[i + 1].frame)
    }

    /// The progress state at position `i`.
    pub fn progress<G: GameData>(&self, game: &G, i: usize, cfg: &KeyConfig) -> Progress {
        Progress::of(game, &self.positions[i].end.world, &self.spine_flags, cfg)
    }
}

/// The detectors' observer factory for `cfg`.
pub fn detectors<G: GameData + Clone + 'static>(
    cfg: &Config,
) -> impl FnMut() -> Box<dyn StepObserver<G>> + use<G> {
    let d = cfg.detect.clone();
    move || Box::new(Detectors::<G>::new(&d))
}

/// The macro frame cap of spine and branch sessions, at least: a scripted scene on the main
/// path may run long (Silice's morning, the starter and the gifts, is ~6,000 frames in one
/// `WaitScript`). Branch budgets are checked between macros, so this bounds one macro only.
pub const PLAY_MACRO_FRAMES: u32 = 12_000;

/// The pacing frame cap of a `Grass` macro in spine and branch sessions, at least: a cave's
/// encounter rate (Granite Cave's 10, a 5.5% roll per step) can go 150 steps dry, and a dry
/// `Train` retry replays the same rolls (the RNG does not advance on idle frames).
pub const PLAY_GRASS_FRAMES: u32 = 9_600;

/// `cfg` with [`PLAY_MACRO_FRAMES`] and [`PLAY_GRASS_FRAMES`] (spine and branch sessions).
pub fn play_config(cfg: &Config) -> Config {
    let mut c = cfg.clone();
    c.limits.macro_frames = c.limits.macro_frames.max(PLAY_MACRO_FRAMES);
    c.limits.grass_frames = c.limits.grass_frames.max(PLAY_GRASS_FRAMES);
    c
}

/// Plays [`steps`] (or `steps`) from a new game.
pub fn run<G: GameData + Clone + 'static>(
    game: &G,
    steps: &'static [Step],
    cfg: &Config,
) -> SpineRun {
    let cfg = &play_config(cfg);
    let mut make = detectors::<G>(cfg);
    let mut out = SpineRun {
        steps,
        positions: Vec::new(),
        inputs: Vec::new(),
        frames: 0,
        coverage: Coverage::default(),
        spine_flags: BTreeSet::new(),
        reports: Vec::new(),
        notes: Vec::new(),
        error: None,
        played: Vec::new(),
    };
    let mut live = match Live::start(game, cfg, None, &mut make) {
        Ok(l) => l,
        Err(stop) => {
            out.error = Some(stop_error(0, "the new game", stop));
            return out;
        }
    };
    // The settled new game is position 0.
    if let Err(e) = live.step(&Macro::of(MacroKind::Settle)) {
        out.error = Some(fail_error(&mut live, 0, "the new game's settle", e));
        return finish(out, live, cfg);
    }
    let mut x = Exec::new(game, cfg);
    out.positions.push(Position {
        index: 0,
        label: None,
        segment: None,
        frame: live.frames(),
        end: live.last_end().expect("settled"),
        opts: x.opts,
    });
    for (i, step) in steps.iter().enumerate() {
        let played = live.res.steps.len();
        let r = x.play(&mut live, step);
        for st in &live.res.steps[played..] {
            out.played.push((i, st.macro_));
        }
        if let Err(e) = r {
            out.error = Some(SpineError {
                step: i,
                message: format!("{step:?}: {}", e.message),
                finding: e.finding.map(|f| *f),
            });
            break;
        }
        let prev = out.positions.last().expect("position 0");
        let label = match step {
            Step::Checkpoint(l) => Some(*l),
            _ => None,
        };
        let segment = label.or(prev.segment);
        out.positions.push(Position {
            index: i + 1,
            label,
            segment,
            frame: live.frames(),
            end: live.last_end().expect("a step was played"),
            opts: x.opts,
        });
    }
    finish(out, live, cfg)
}

fn finish<G: GameData + Clone>(mut out: SpineRun, live: Live<'_, '_, G>, cfg: &Config) -> SpineRun {
    out.inputs = live.inputs();
    out.frames = live.frames();
    let res = live.finish();
    let mut from = res
        .steps
        .first()
        .map(|st| st.end.world.field.map)
        .unwrap_or(MapId(0));
    for st in &res.steps {
        out.coverage.add_step(from, st, &cfg.key);
        from = st.end.world.field.map;
    }
    out.spine_flags = spine_flags(&out.positions, &cfg.key);
    out.reports = res.reports;
    out.notes = res.notes;
    out
}

/// The story flags set at the last position, clear at the first, and never cleared once set
/// along the positions (on-load toggles such as hide flags are left out).
fn spine_flags(positions: &[Position], cfg: &KeyConfig) -> BTreeSet<u16> {
    let (Some(first), Some(last)) = (positions.first(), positions.last()) else {
        return BTreeSet::new();
    };
    let set = |p: &Position, f: FlagId| p.end.world.vars.flags.binary_search(&f).is_ok();
    (last.end.world.vars.flags.iter())
        .filter(|&&f| key::story_flag(f, cfg) && !set(first, f))
        .filter(|&&f| {
            let mut seen = false;
            positions.iter().all(|p| {
                let s = set(p, f);
                let ok = s || !seen;
                seen |= s;
                ok
            })
        })
        .map(|f| f.0)
        .collect()
}

/// A failed step.
#[derive(Clone, Debug)]
pub struct ExecError {
    pub message: String,
    /// Boxed: a draft is large and errors are rare.
    pub finding: Option<Box<FindingDraft>>,
}

fn stop_error(step: usize, what: &str, stop: Stop) -> SpineError {
    let (message, finding) = stop_text(stop);
    SpineError {
        step,
        message: format!("{what}: {message}"),
        finding,
    }
}

fn stop_text(stop: Stop) -> (String, Option<FindingDraft>) {
    match stop {
        Stop::Pruned(m) => (m, None),
        Stop::Finding(f) => (format!("{:?}: {}", f.kind, f.message), Some(f)),
        Stop::NotTrapped => ("not trapped".into(), None),
    }
}

fn fail_error<G: GameData + Clone>(
    live: &mut Live<'_, '_, G>,
    step: usize,
    what: &str,
    e: StepFail,
) -> SpineError {
    let e = fail(live, e);
    SpineError {
        step,
        message: format!("{what}: {}", e.message),
        finding: e.finding.map(|f| *f),
    }
}

fn fail<G: GameData + Clone>(live: &mut Live<'_, '_, G>, e: StepFail) -> ExecError {
    match e {
        StepFail::Skip(e) => ExecError {
            message: format!("{e:?}"),
            finding: None,
        },
        StepFail::Stop => {
            let (message, finding) = live
                .res
                .stop
                .take()
                .map_or(("stopped".into(), None), stop_text);
            ExecError {
                message,
                finding: finding.map(Box::new),
            }
        }
    }
}

/// Plays steps on a [`Live`] session: the spine's runner, and the branches' verification
/// window.
pub struct Exec<'g, G: GameData> {
    game: &'g G,
    cfg: &'g Config,
    pub opts: StepOpts,
}

/// Macros a `Goto`, `Enter` or `Cross` plays at most (a battle or a script may interrupt
/// each).
pub const RETRIES: u32 = 12;

/// Idle frames a `WaitScript` step waits before resolving.
pub const WAIT_SCRIPT_FRAMES: u16 = 60;

/// Frames a `WaitFor` step idles at most.
pub const WAIT_FOR_FRAMES: u32 = 1_200;

/// Times a `Train` step's pacing may find no encounter.
pub const TRAIN_DRY: u32 = 8;

/// Wild battles a `Train` step fights at most.
pub const TRAIN_BATTLES: u32 = 80;

impl<'g, G: GameData + Clone> Exec<'g, G> {
    pub fn new(game: &'g G, cfg: &'g Config) -> Self {
        Exec {
            game,
            cfg,
            opts: StepOpts::default(),
        }
    }

    /// From a position's options.
    pub fn at(game: &'g G, cfg: &'g Config, opts: StepOpts) -> Self {
        Exec { game, cfg, opts }
    }

    fn resolve_map(&self, m: MapId) -> MapId {
        if m == NEW_GAME_MAP {
            self.game.new_game().map
        } else {
            m
        }
    }

    /// Plays one step.
    pub fn play(&mut self, live: &mut Live<'_, '_, G>, step: &Step) -> Result<(), ExecError> {
        let err = |message: String| ExecError {
            message,
            finding: None,
        };
        if let Some(want) = step.map() {
            let want = self.resolve_map(want);
            let on = live.s.world().map();
            if on != want {
                return Err(err(format!(
                    "on map {:#06x} at {:?}, not {:#06x}",
                    on.0,
                    live.s.world().objects().player().pos(),
                    want.0
                )));
            }
        }
        match *step {
            Step::Battle(p) => {
                match p {
                    BattlePlan::Fight => {
                        self.opts.run_wild = false;
                        self.opts.best_move = false;
                        self.opts.learn = false;
                    }
                    BattlePlan::Best => {
                        self.opts.run_wild = false;
                        self.opts.best_move = true;
                        self.opts.learn = false;
                    }
                    BattlePlan::BestLearn => {
                        self.opts.run_wild = false;
                        self.opts.best_move = true;
                        self.opts.learn = true;
                    }
                    BattlePlan::RunWild => self.opts.run_wild = true,
                }
                return Ok(());
            }
            Step::Choose(n) => {
                self.opts.choice = Some(n);
                return Ok(());
            }
            Step::Checkpoint(_) => return Ok(()),
            Step::Bypass(b) => return bypass(self.game, live, b).map_err(err),
            Step::Expect(name) => {
                let goal = goals::find(self.cfg.act, name)
                    .ok_or_else(|| err(format!("no goal {name:?} in {:?}", self.cfg.act)))?;
                let snap = live
                    .s
                    .world()
                    .snapshot(live.s.game())
                    .map_err(|e| err(format!("not capturable: {e:?}")))?;
                return if (goal.reached)(&snap) {
                    Ok(())
                } else {
                    let w = live.s.world();
                    Err(err(format!(
                        "goal {name:?} not reached (frame {}, map {:?} at {:?}, script {}, \
                         transition {})",
                        w.frame(),
                        w.map(),
                        w.objects().player().pos(),
                        w.script_running(),
                        w.transition_active()
                    )))
                };
            }
            _ => {}
        }
        let opts = Opts {
            run_wild: self.opts.run_wild,
            choice: self.opts.choice.take().unwrap_or(0),
            best_move: self.opts.best_move,
            learn: self.opts.learn,
        };
        let start_map = live.s.world().map();
        let mk = |kind| Macro { kind, opts };
        match *step {
            Step::Goto { x, y, .. } => self.goto(live, start_map, x, y, opts)?,
            Step::GotoAvoiding { x, y, avoid, .. } => {
                self.goto_avoiding(live, start_map, (x, y), avoid, opts)?
            }
            Step::Enter { x, y, dir, .. } => {
                for _ in 0..RETRIES {
                    if live.s.world().map() != start_map {
                        break;
                    }
                    self.macro_(live, mk(MacroKind::Enter { x, y, dir }))?;
                }
                let want = (self.game.map(start_map).warps.iter())
                    .find(|w| (w.x, w.y) == (x, y))
                    .map(|w| w.dest_map)
                    .filter(|&m| self.game.has_map(m));
                let on = live.s.world().map();
                if want.is_some_and(|m| m != on) || (want.is_none() && on == start_map) {
                    return Err(err(format!(
                        "entered ({x}, {y}) but on map {:#06x}, not {want:?}",
                        on.0
                    )));
                }
            }
            Step::Cross { x, y, dir, .. } => {
                for _ in 0..RETRIES {
                    if live.s.world().map() != start_map {
                        break;
                    }
                    self.macro_(live, mk(MacroKind::Cross { x, y, dir }))?;
                }
                if live.s.world().map() == start_map {
                    return Err(err(format!(
                        "crossed at ({x}, {y}) but the map is the same"
                    )));
                }
            }
            // A wild battle on the way (a cave floor) ends the macro before the talk: again.
            Step::Talk { local_id, .. } => {
                // Gen3 spawns objects only near the camera: walk next to an unspawned one's
                // template cell first (the first reachable side).
                if live.s.world().objects().get(local_id).is_none()
                    && let Some(t) =
                        (self.game.map(start_map).objects.iter()).find(|t| t.local_id == local_id)
                    && let Ok(snap) = live.s.world().snapshot(live.s.game())
                {
                    let reach = crate::path::reach(self.game, &snap);
                    let side = Dir::ALL.into_iter().find_map(|d| {
                        let (dx, dy) = d.delta();
                        let c = (t.x - dx, t.y - dy);
                        reach.dist(c.0, c.1).map(|_| c)
                    });
                    if let Some((x, y)) = side {
                        self.goto(live, start_map, x, y, opts)?;
                    }
                }
                for _ in 0..RETRIES {
                    self.macro_(live, mk(MacroKind::Talk { local_id }))?;
                    let s = &live.res.steps.last().expect("a step").summary;
                    let wild_only = !s.battles.is_empty() && s.battles.iter().all(|b| b.kind == 0);
                    if !(wild_only && s.scripts.is_empty()) {
                        break;
                    }
                }
            }
            Step::Use { x, y, dir, .. } => {
                self.goto(live, start_map, x, y, Opts { choice: 0, ..opts })?;
                if live.s.world().objects().player().facing != dir {
                    self.macro_(live, Macro::of(MacroKind::Face { dir }))?;
                }
                self.macro_(live, mk(MacroKind::Interact))?;
            }
            Step::Face(dir) => {
                if live.s.world().objects().player().facing != dir {
                    self.macro_(live, mk(MacroKind::Face { dir }))?;
                }
            }
            Step::Interact => self.macro_(live, mk(MacroKind::Interact))?,
            Step::Walk(dir) => self.macro_(live, mk(MacroKind::Step { dir }))?,
            Step::Exit { dir, to } => {
                for _ in 0..RETRIES {
                    if live.s.world().map() != start_map {
                        break;
                    }
                    self.macro_(live, mk(MacroKind::Push { dir }))?;
                }
                let to = self.resolve_map(to);
                let on = live.s.world().map();
                if on != to {
                    return Err(err(format!(
                        "exited {dir:?} to map {:#06x}, not {:#06x}",
                        on.0, to.0
                    )));
                }
            }
            Step::Press(keys, frames) => {
                self.macro_(live, mk(MacroKind::Press { keys, frames }))?
            }
            Step::Wait(frames) => self.macro_(live, mk(MacroKind::Wait { frames }))?,
            // A few idle frames first: a map's frame script starts on the field's next frame.
            Step::WaitScript => self.macro_(
                live,
                mk(MacroKind::Wait {
                    frames: WAIT_SCRIPT_FRAMES,
                }),
            )?,
            Step::WaitFor { local_id, x, y } => {
                let mut n = 0;
                while (live.s.world().objects().get(local_id)).is_none_or(|o| o.pos() != (x, y)) {
                    n += 1;
                    if n > WAIT_FOR_FRAMES {
                        return Err(err(format!(
                            "object {local_id} never on ({x}, {y}) in {WAIT_FOR_FRAMES} frames"
                        )));
                    }
                    self.macro_(live, mk(MacroKind::Wait { frames: 1 }))?;
                }
            }
            Step::Grass { x, y, .. } => self.macro_(live, mk(MacroKind::Grass { x, y }))?,
            Step::Train { x, y, level, .. } => {
                let fight = mk(MacroKind::Grass { x, y });
                let (mut n, mut dry) = (0, 0);
                while live
                    .s
                    .world()
                    .party()
                    .get(0)
                    .is_some_and(|m| m.level < level)
                {
                    n += 1;
                    if n > TRAIN_BATTLES {
                        return Err(err(format!(
                            "not level {level} after {TRAIN_BATTLES} tries"
                        )));
                    }
                    match live.step_with(&fight, &mut crate::policy::Policy::new()) {
                        Ok(()) => dry = 0,
                        // No encounter: idle a little (another random state) and pace again.
                        Err(StepFail::Skip(e @ crate::expand::ExpandError::NoEffect(_))) => {
                            dry += 1;
                            if dry > TRAIN_DRY {
                                let w = live.s.world();
                                return Err(err(format!(
                                    "{e:?} {dry} times in a row (after {n} tries; at {:?}, lead \
                                     {:?})",
                                    w.objects().player().pos(),
                                    w.party().get(0).map(|m| (m.level, m.hp))
                                )));
                            }
                            live.rollback().map_err(|stop| {
                                let (message, finding) = stop_text(stop);
                                ExecError {
                                    message,
                                    finding: finding.map(Box::new),
                                }
                            })?;
                            self.macro_(live, mk(MacroKind::Wait { frames: 31 }))?;
                        }
                        Err(e) => return Err(fail(live, e)),
                    }
                    // A lost battle (a whiteout) leaves the grass.
                    let w = live.s.world();
                    if w.map() != start_map {
                        return Err(err(format!(
                            "left the map while training (on {:#06x} at {:?}, lead {:?} after {n} \
                             tries)",
                            w.map().0,
                            w.objects().player().pos(),
                            w.party().get(0).map(|m| (m.level, m.hp))
                        )));
                    }
                }
            }
            Step::Battle(_)
            | Step::Choose(_)
            | Step::Expect(_)
            | Step::Checkpoint(_)
            | Step::Bypass(_) => {
                unreachable!("handled above")
            }
        }
        Ok(())
    }

    /// Walks to (x, y) on `map`: a battle or a script can interrupt the walk, which walks on.
    fn goto(
        &self,
        live: &mut Live<'_, '_, G>,
        map: MapId,
        x: i16,
        y: i16,
        opts: Opts,
    ) -> Result<(), ExecError> {
        let err = |message: String| ExecError {
            message,
            finding: None,
        };
        for _ in 0..RETRIES {
            if live.s.world().objects().player().pos() == (x, y) {
                break;
            }
            if live.s.world().map() != map {
                let w = live.s.world();
                return Err(err(format!(
                    "left the map walking to ({x}, {y}): on {:#06x} at {:?}, lead {:?}",
                    w.map().0,
                    w.objects().player().pos(),
                    w.party().get(0).map(|m| (m.level, m.hp))
                )));
            }
            let walk = Macro {
                kind: MacroKind::WalkTo { x, y },
                opts,
            };
            match live.step_with(&walk, &mut crate::policy::Policy::new()) {
                Ok(()) => {}
                // A wandering object on the way (or on the cell): let it move on.
                Err(StepFail::Skip(crate::expand::ExpandError::NoPath)) => {
                    live.rollback().map_err(|stop| {
                        let (message, finding) = stop_text(stop);
                        ExecError {
                            message,
                            finding: finding.map(Box::new),
                        }
                    })?;
                    self.macro_(live, Macro::of(MacroKind::Wait { frames: 16 }))?;
                }
                Err(e) => return Err(fail(live, e)),
            }
        }
        if live.s.world().map() != map || live.s.world().objects().player().pos() != (x, y) {
            let w = live.s.world();
            return Err(err(format!(
                "never reached ({x}, {y}): on {:#06x} at {:?}, lead {:?}",
                w.map().0,
                w.objects().player().pos(),
                w.party().get(0).map(|m| (m.level, m.hp))
            )));
        }
        Ok(())
    }

    /// [`Step::GotoAvoiding`]: a `Step` macro per cell, toward the first direction of a
    /// shortest path around `avoid` (re-planned from wherever each step left the player).
    fn goto_avoiding(
        &self,
        live: &mut Live<'_, '_, G>,
        map: MapId,
        to: (i16, i16),
        avoid: &[(i16, i16)],
        opts: Opts,
    ) -> Result<(), ExecError> {
        let err = |message: String| ExecError {
            message,
            finding: None,
        };
        let data = self.game.map(map);
        let most = 4 * u32::from(data.width) * u32::from(data.height);
        for _ in 0..most {
            let w = live.s.world();
            if w.map() != map {
                return Err(err(format!("left the map walking to {to:?}")));
            }
            let at = w.objects().player().pos();
            if at == to {
                return Ok(());
            }
            let snap = w
                .snapshot(live.s.game())
                .map_err(|e| err(format!("not capturable walking to {to:?}: {e:?}")))?;
            let grid = crate::path::Grid::of_snapshot(self.game, &snap);
            let dir = first_dir_avoiding(&grid, at, to, avoid)
                .ok_or_else(|| err(format!("no path from {at:?} to {to:?} around {avoid:?}")))?;
            self.macro_(
                live,
                Macro {
                    kind: MacroKind::Step { dir },
                    opts,
                },
            )?;
        }
        Err(err(format!("never reached {to:?}")))
    }

    fn macro_(&self, live: &mut Live<'_, '_, G>, m: Macro) -> Result<(), ExecError> {
        live.step_with(&m, &mut crate::policy::Policy::new())
            .map_err(|e| fail(live, e))
    }
}

/// Frames a bypass's warp may take at most.
const BYPASS_WARP_FRAMES: u32 = 1_200;

/// Plays [`Step::Bypass`]: writes its flags and vars into the current state, restores it
/// (another build header, so the hash is not checked against the edited state), warps if asked
/// and waits the warp out, then loads the result into the session as a step with no input.
fn bypass<G: GameData + Clone>(
    game: &G,
    live: &mut Live<'_, '_, G>,
    b: &Bypass,
) -> Result<(), String> {
    let mut cp = (live.s.checkpoint()).map_err(|e| format!("bypass {}: {e:?}", b.issue))?;
    let v = &mut cp.world.vars;
    for &f in b.set {
        if let Err(i) = v.flags.binary_search(&f) {
            v.flags.insert(i, f);
        }
    }
    for f in b.clear {
        if let Ok(i) = v.flags.binary_search(f) {
            v.flags.remove(i);
        }
    }
    for &(var, value) in b.vars {
        match (v.vars.binary_search_by_key(&var, |&(k, _)| k), value) {
            (Ok(i), 0) => {
                v.vars.remove(i);
            }
            (Ok(i), _) => v.vars[i].1 = value,
            (Err(_), 0) => {}
            (Err(i), _) => v.vars.insert(i, (var, value)),
        }
    }
    cp.header.build = engine::checkpoint::BuildHash {
        engine: 0,
        content: 0,
    };
    let mut world = engine::world::World::restore(game, &cp)
        .map_err(|e| format!("bypass {}: restore {e:?}", b.issue))?;
    if let Some((map, x, y)) = b.warp {
        if !world.debug_warp(game, map, None, x, y) {
            return Err(format!(
                "bypass {}: warp to {:#06x} refused",
                b.issue, map.0
            ));
        }
        let mut n = 0;
        loop {
            world.step(game, engine::Buttons::NONE);
            n += 1;
            if world.map() == map && !world.transition_active() && world.capturable().is_ok() {
                break;
            }
            if n > BYPASS_WARP_FRAMES {
                return Err(format!("bypass {}: the warp never settled", b.issue));
            }
        }
    }
    let end = (world.checkpoint(game)).map_err(|e| format!("bypass {}: {e:?}", b.issue))?;
    live.load_bypass(end)
        .map_err(|e| format!("bypass {}: {e}", b.issue))
}

/// The first direction of a shortest path from `from` to `to` on `grid` that never enters an
/// `avoid` cell (ledges jumped toward their direction; `to` entered whatever it is). Grass is
/// avoided when a path without it exists.
fn first_dir_avoiding<G: GameData>(
    grid: &crate::path::Grid<'_, G>,
    from: (i16, i16),
    to: (i16, i16),
    avoid: &[(i16, i16)],
) -> Option<Dir> {
    use engine::data::Behavior;
    let step = |c: (i16, i16), d: Dir, grass: bool| -> Option<(i16, i16)> {
        if grid.behavior(c) == Behavior::ArrowWarp(d) {
            return None;
        }
        let (dx, dy) = d.delta();
        let mut n = (c.0 + dx, c.1 + dy);
        if !grid.in_bounds(n) || Behavior::walled(grid.behavior(c), grid.behavior(n), d) {
            return None;
        }
        if let Behavior::Ledge(ld) = grid.behavior(n) {
            if ld != d {
                return None;
            }
            n = (n.0 + dx, n.1 + dy);
        }
        let ok = grid.in_bounds(n) && !avoid.contains(&n) && (n == to || grid.walkable(n, grass));
        ok.then_some(n)
    };
    for grass in [true, false] {
        // BFS from `from`, remembering each cell's first direction.
        let mut first: std::collections::BTreeMap<(i16, i16), Option<Dir>> =
            std::collections::BTreeMap::from([(from, None)]);
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(c) = queue.pop_front() {
            if c == to {
                return first[&c];
            }
            for d in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                if let Some(n) = step(c, d, grass)
                    && !first.contains_key(&n)
                {
                    first.insert(n, first[&c].or(Some(d)));
                    queue.push_back(n);
                }
            }
        }
    }
    None
}
