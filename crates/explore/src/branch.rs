//! Branches off the spine (`docs/plans/2026-10-23-explorer-spine.md` §3): from spine
//! positions, toward what the spine never touched; then back to the spine, or aborted.
//!
//! - [`origins`]: the positions branches start from.
//! - [`targets`]: per origin, the candidates the spine's coverage does not hold.
//! - [`run`]: one branch (explore, recover, verify), [`run_all`] many on a worker pool, results
//!   in target order.

use crate::config::{BranchConfig, Config};
use crate::findings;
use crate::key;
use crate::macros;
use crate::model::{FindingDraft, FindingKind, Macro, MacroKind, Opts, Stop};
use crate::path;
use crate::sim::{self, Live, StepFail};
use crate::softlock::Trap;
use crate::spine::{Coverage, Exec, Gain, Position, Progress, SpineRun, Step, StepOpts};
use crate::workers;
use engine::checkpoint::WorldSnapshot;
use engine::data::{BgKind, Dir, GameData, MapId, ScriptId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

/// What a branch does first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Action {
    /// A macro from the origin.
    Macro(Macro),
    /// The spine's next step (`step`) with another yes/no or starter `choice`.
    Choice { step: u32, choice: u8 },
}

impl Action {
    /// RON text (the db's `branch.action`).
    pub fn to_text(&self) -> String {
        ron::to_string(self).expect("an action serializes")
    }

    pub fn from_text(s: &str) -> Option<Action> {
        ron::from_str(s).ok()
    }
}

/// Why a target is new.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TargetKind {
    /// A warp to a map the spine never went to from here.
    Warp,
    /// A crossing the spine never took.
    Edge,
    /// An object whose script the spine never started (NPCs, item balls, trainers).
    Npc,
    /// A bg event whose script the spine never started.
    Bg,
    /// A coord event whose script the spine never started.
    Coord,
    /// A coarse block the spine never stood in.
    Cells,
    /// Wild battles on a map where the spine fought none.
    Grass,
    /// RUN from a wild battle on a map where the spine never ran.
    Run,
    /// The spine's next interaction with another choice.
    Choice,
}

impl TargetKind {
    pub const ALL: [TargetKind; 9] = [
        TargetKind::Warp,
        TargetKind::Edge,
        TargetKind::Npc,
        TargetKind::Bg,
        TargetKind::Coord,
        TargetKind::Cells,
        TargetKind::Grass,
        TargetKind::Run,
        TargetKind::Choice,
    ];

    /// [`TargetKind::name`] back.
    pub fn parse(s: &str) -> Option<TargetKind> {
        TargetKind::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            TargetKind::Warp => "warp",
            TargetKind::Edge => "edge",
            TargetKind::Npc => "npc",
            TargetKind::Bg => "bg",
            TargetKind::Coord => "coord",
            TargetKind::Cells => "cells",
            TargetKind::Grass => "grass",
            TargetKind::Run => "run",
            TargetKind::Choice => "choice",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Target {
    /// The origin's position index.
    pub origin: usize,
    pub action: Action,
    pub kind: TargetKind,
}

/// The branch origins: every checkpoint position and every `every`-th step's end, playing
/// steps only (a position that only set options repeats the one before it). With `from`, only
/// the positions of that checkpoint's segment (from it to the next checkpoint).
pub fn origins(spine: &SpineRun, cfg: &BranchConfig, from: Option<&str>) -> Vec<usize> {
    let mut out = Vec::new();
    for p in &spine.positions {
        if let Some(from) = from
            && p.segment != Some(from)
        {
            continue;
        }
        let step = p.index.checked_sub(1).map(|i| spine.steps[i]);
        let labelled = p.label.is_some();
        let plays = step.is_none_or(|s| s.plays());
        let every = cfg.every > 0 && (p.index as u32).is_multiple_of(cfg.every);
        if labelled || (plays && every) {
            out.push(p.index);
        }
    }
    out.dedup_by_key(|&mut i| spine.positions[i].end.header.frame);
    out
}

/// The script of object `local_id` on `map` (its template's).
fn object_script(map: &engine::data::MapData, local_id: u8) -> Option<ScriptId> {
    map.objects
        .iter()
        .find(|o| o.local_id == local_id)
        .and_then(|o| o.script)
}

/// The map a crossing toward `dir` leads to.
fn neighbour(map: &engine::data::MapData, dir: Dir) -> Option<MapId> {
    map.connections.iter().find(|c| c.dir == dir).map(|c| c.map)
}

/// Whether `m` from a state on `map` is new against the coverages (`a` ∪ `b`), and why.
fn novel<G: GameData>(
    game: &G,
    snap: &WorldSnapshot,
    m: &Macro,
    covs: &[&Coverage],
    shift: u8,
) -> Option<TargetKind> {
    let here = snap.field.map;
    let data = game.map(here);
    let any = |f: &dyn Fn(&Coverage) -> bool| covs.iter().any(|c| f(c));
    let script_new = |s: Option<ScriptId>| s.is_some_and(|s| !any(&|c| c.scripts.contains(&s)));
    match m.kind {
        MacroKind::Enter { x, y, .. } => {
            let dest = data.warps.iter().find(|w| (w.x, w.y) == (x, y))?.dest_map;
            (game.has_map(dest) && !any(&|c| c.edges.contains(&(here, dest))))
                .then_some(TargetKind::Warp)
        }
        MacroKind::Cross { dir, .. } => {
            let to = neighbour(data, dir)?;
            (!any(&|c| c.edges.contains(&(here, to)))).then_some(TargetKind::Edge)
        }
        MacroKind::Talk { local_id } => {
            script_new(object_script(data, local_id)).then_some(TargetKind::Npc)
        }
        MacroKind::Inspect { x, y, .. } => {
            let s = data.bgs.iter().find_map(|b| match b.kind {
                BgKind::Script { script, .. } if (b.x, b.y) == (x, y) => Some(script),
                _ => None,
            });
            script_new(s).then_some(TargetKind::Bg)
        }
        MacroKind::Grass { .. } => {
            let fought = any(&|c| c.wild_maps.contains(&here));
            let ran = any(&|c| c.ran_maps.contains(&here));
            if m.opts.run_wild {
                (!ran && !snap.party.is_empty()).then_some(TargetKind::Run)
            } else {
                (!fought).then_some(TargetKind::Grass)
            }
        }
        MacroKind::WalkTo { x, y } => {
            let coord = data.coords.iter().find(|c| (c.x, c.y) == (x, y));
            if let Some(c) = coord
                && script_new(Some(c.script))
            {
                return Some(TargetKind::Coord);
            }
            let s = shift.min(14);
            let block = (here, x >> s, y >> s);
            (!any(&|c| c.blocks.contains(&block))).then_some(TargetKind::Cells)
        }
        _ => None,
    }
}

/// The candidates of `snap` with the RUN variant of each grass candidate, in candidate order.
fn candidates<G: GameData>(game: &G, snap: &WorldSnapshot, shift: u8) -> Vec<Macro> {
    let reach = path::reach_all(game, snap);
    let mut out = Vec::new();
    for m in macros::candidates_with(game, snap, &reach, shift) {
        out.push(m);
        if matches!(m.kind, MacroKind::Grass { .. }) {
            out.push(Macro {
                kind: m.kind,
                opts: Opts {
                    run_wild: true,
                    choice: 0,
                    ..Opts::default()
                },
            });
        }
    }
    out
}

/// The choices a spine interaction did not make: the starter's other balls after a
/// `Choose(2)`, else the other yes/no answer.
fn other_choices(spine: &SpineRun, step: usize) -> Vec<u8> {
    let made = step
        .checked_sub(1)
        .and_then(|i| match spine.steps[i] {
            Step::Choose(n) => Some(n),
            _ => None,
        })
        .unwrap_or(0);
    match made {
        2 => vec![0, 1],
        0 => vec![1],
        _ => vec![0],
    }
}

/// The targets of origin `origin`, in candidate order (choices last), at most `max_targets`.
pub fn targets<G: GameData>(
    game: &G,
    spine: &SpineRun,
    origin: usize,
    cfg: &Config,
) -> Vec<Target> {
    let p = &spine.positions[origin];
    let snap = &p.end.world;
    let shift = cfg.key.cell_shift;
    let mut out: Vec<Target> = candidates(game, snap, shift)
        .into_iter()
        .filter_map(|m| {
            let kind = novel(game, snap, &m, &[&spine.coverage], shift)?;
            Some(Target {
                origin,
                action: Action::Macro(m),
                kind,
            })
        })
        .collect();
    // The spine's next interaction (past option steps), with the choices it did not make.
    let mut next = origin;
    while spine.steps.get(next).is_some_and(|s| !s.plays()) {
        next += 1;
    }
    if let Some(step) = spine.steps.get(next)
        && matches!(step, Step::Talk { .. } | Step::Use { .. } | Step::Interact)
    {
        for choice in other_choices(spine, next) {
            out.push(Target {
                origin,
                action: Action::Choice {
                    step: next as u32,
                    choice,
                },
                kind: TargetKind::Choice,
            });
        }
    }
    out.truncate(cfg.branch.max_targets as usize);
    out
}

/// The targets of every origin, in spine order, each `(place, action)` once (the first origin
/// keeps it).
pub fn all_targets<G: GameData>(
    game: &G,
    spine: &SpineRun,
    origins: &[usize],
    cfg: &Config,
) -> Vec<Target> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &o in origins {
        let place = key::place_region(&key::key_parts(&spine.positions[o].end.world, &cfg.key));
        for t in targets(game, spine, o, cfg) {
            if seen.insert((place, t.action)) {
                out.push(t);
            }
        }
    }
    out
}

/// How a branch ended.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Outcome {
    /// Back on the spine: the window ran and the progress state matches the spine's.
    Rejoined,
    /// The window failed or the progress state differs; `expected` when the branch had
    /// changed the progress state itself (or a battle of the window was lost).
    Diverged { expected: bool },
    /// No way back to the origin within the hops and frames.
    Stranded,
    /// Too little novelty for its frames (the rate fell below `min_novelty` past the target),
    /// or a macro timed out: no recovery.
    Aborted,
    /// A detector or a panic stopped it (a finding).
    Bug,
    /// The target could not be played from the origin (no path, no effect).
    Unplayable,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Rejoined => "rejoined",
            Outcome::Diverged { expected: true } => "diverged_expected",
            Outcome::Diverged { expected: false } => "diverged",
            Outcome::Stranded => "stranded",
            Outcome::Aborted => "aborted",
            Outcome::Bug => "bug",
            Outcome::Unplayable => "unplayable",
        }
    }

    pub const NAMES: [&'static str; 7] = [
        "rejoined",
        "diverged_expected",
        "diverged",
        "stranded",
        "aborted",
        "bug",
        "unplayable",
    ];
}

/// Why the exploration stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Explored {
    /// No new candidate left, or `max_steps` played.
    Exhausted,
    /// The novelty rate fell below `min_novelty` once the branch chained past its target
    /// (aborted: no recovery).
    LowNovelty,
    /// More than `budget_frames` (it then recovers).
    Budget,
}

/// One branch's result.
#[derive(Clone, Debug)]
pub struct BranchResult {
    pub target: Target,
    pub outcome: Outcome,
    pub explored: Option<Explored>,
    /// The macros the exploration played (the target's first).
    pub macros: Vec<Macro>,
    pub explore_frames: u64,
    pub recover_frames: u64,
    pub window_frames: u64,
    /// New against the spine.
    pub gain: Gain,
    pub points: i64,
    /// What the branch touched (exploration, recovery and window).
    pub coverage: Coverage,
    /// Bugs (tails from the origin).
    pub findings: Vec<FindingDraft>,
    /// Report-only findings (tails from the origin).
    pub reports: Vec<FindingDraft>,
    /// What diverged, or why it stopped.
    pub message: String,
}

impl BranchResult {
    pub fn frames(&self) -> u64 {
        self.explore_frames + self.recover_frames + self.window_frames
    }
}

/// Runs `targets` on `cfg.jobs` threads, results in target order.
pub fn run_all<G: GameData + Clone + Send + Sync + 'static>(
    game: &G,
    spine: &SpineRun,
    targets: &[Target],
    cfg: &Config,
) -> Vec<BranchResult> {
    workers::run_ordered(targets, cfg.jobs, |t| run(game, spine, t, cfg))
}

/// One branch from its origin: explore, then recover and verify (§3.3).
pub fn run<G: GameData + Clone + 'static>(
    game: &G,
    spine: &SpineRun,
    target: &Target,
    cfg: &Config,
) -> BranchResult {
    let cfg = &crate::spine::play_config(cfg);
    let origin = &spine.positions[target.origin];
    let mut make = crate::spine::detectors::<G>(cfg);
    let mut r = BranchResult {
        target: *target,
        outcome: Outcome::Unplayable,
        explored: None,
        macros: Vec::new(),
        explore_frames: 0,
        recover_frames: 0,
        window_frames: 0,
        gain: Gain::default(),
        points: 0,
        coverage: Coverage::default(),
        findings: Vec::new(),
        reports: Vec::new(),
        message: String::new(),
    };
    let mut live = match Live::start(game, cfg, Some(origin.end.clone()), &mut make) {
        Ok(l) => l,
        Err(stop) => {
            stopped(&mut r, stop);
            return r;
        }
    };
    let b = &cfg.branch;
    // ---- explore ----
    let first = play_action(game, spine, origin, cfg, &mut live, target.action);
    if let Err(e) = first {
        match e {
            StepFail::Skip(e) => r.message = format!("{e:?}"),
            StepFail::Stop => {
                if let Some(stop) = live.res.stop.take() {
                    stopped(&mut r, stop);
                }
            }
        }
        r.reports = std::mem::take(&mut live.res.reports);
        return r;
    }
    if let Action::Macro(m) = target.action {
        r.macros.push(m);
    }
    // Macros chained after the target.
    let mut chained = 0u32;
    let explored = loop {
        let frames = live.frames();
        r.explore_frames = frames;
        let cov = coverage_of(&live, origin, cfg);
        let gain = cov.gain_over(&spine.coverage);
        r.gain = gain;
        r.points = gain.points(&cfg.weights);
        // The target always plays; the rate judges the chain after it.
        if chained > 0 && (r.points.max(0) as u64) * 1000 < u64::from(b.min_novelty) * frames.max(1)
        {
            break Explored::LowNovelty;
        }
        if frames > u64::from(b.budget_frames) {
            break Explored::Budget;
        }
        if live.res.steps.len() as u32 >= b.max_steps {
            break Explored::Exhausted;
        }
        // The next new candidate (against the spine and the branch so far).
        let snap = &live.last_end().expect("a step").world;
        let next: Vec<Macro> = candidates(game, snap, cfg.key.cell_shift)
            .into_iter()
            .filter(|m| {
                novel(game, snap, m, &[&spine.coverage, &cov], cfg.key.cell_shift).is_some()
            })
            .take(4)
            .collect();
        let mut played = false;
        for m in next {
            match live.step_with(&m, &mut crate::policy::Policy::new()) {
                Ok(()) => {
                    r.macros.push(m);
                    chained += 1;
                    played = true;
                    break;
                }
                Err(StepFail::Skip(_)) => {
                    if let Err(stop) = live.rollback() {
                        stopped(&mut r, stop);
                        return r;
                    }
                }
                Err(StepFail::Stop) => {
                    r.explore_frames = live.frames();
                    if let Some(stop) = live.res.stop.take() {
                        stopped(&mut r, stop);
                    }
                    r.coverage = coverage_of(&live, origin, cfg);
                    r.reports = std::mem::take(&mut live.res.reports);
                    return r;
                }
            }
        }
        if !played {
            break Explored::Exhausted;
        }
    };
    r.explored = Some(explored);
    r.explore_frames = live.frames();
    // Too many frames for too little novelty: aborted, no recovery. A branch stopped by its
    // budget or its step cap while still finding things, or with nothing new left, recovers.
    if explored == Explored::LowNovelty {
        r.outcome = Outcome::Aborted;
        r.message = format!(
            "{} points in {} frames: under {} per 1,000 frames",
            r.points, r.explore_frames, b.min_novelty
        );
        r.coverage = coverage_of(&live, origin, cfg);
        r.reports = std::mem::take(&mut live.res.reports);
        return r;
    }
    let changed = {
        let now = Progress::of(
            game,
            &live.last_end().expect("a step").world,
            &spine.spine_flags,
            &cfg.key,
        );
        let at = spine.progress(game, target.origin, &cfg.key);
        let ahead = side_flags_ahead(
            spine,
            target.origin,
            &live.last_end().expect("a step").world,
        );
        let mut what: Vec<String> = (now != at).then(|| now.diff(&at)).into_iter().collect();
        what.extend(ahead.iter().take(4).map(|f| format!("side flag {f:#x}")));
        (!what.is_empty()).then(|| what.join(", "))
    };
    let lost_exploring = live.res.steps.iter().any(lost_battle);
    // ---- recover ----
    let rec_from = live.frames();
    let recovered = recover(game, spine, origin, cfg, &mut live, rec_from);
    r.recover_frames = live.frames() - rec_from;
    match recovered {
        Recovered::Back => {}
        Recovered::Bug => {
            if let Some(stop) = live.res.stop.take() {
                stopped(&mut r, stop);
            }
            r.coverage = coverage_of(&live, origin, cfg);
            r.reports = std::mem::take(&mut live.res.reports);
            return r;
        }
        Recovered::Stranded(why) if lost_exploring => {
            // A lost battle sent the player to the last heal place: expected, not stranded.
            r.outcome = Outcome::Diverged { expected: true };
            r.message = format!("a battle of the exploration was lost (whiteout); {why}");
            r.coverage = coverage_of(&live, origin, cfg);
            r.reports = std::mem::take(&mut live.res.reports);
            return r;
        }
        Recovered::Stranded(why) => {
            r.outcome = Outcome::Stranded;
            r.message = why;
            // A stranded state may be a softlock: probe it.
            if let Some(end) = live.last_end() {
                let k = sim::softlock_k(cfg, end.world.field.map);
                let probed = sim::trap(game, &end, k, cfg.detect.softlock_breadth, cfg);
                if let Trap::Trapped { reached } = probed.trap {
                    let mut f = sim::softlock_finding(&end, probed.place, k, reached);
                    f.tail = live.inputs();
                    r.findings.push(f);
                }
            }
            r.coverage = coverage_of(&live, origin, cfg);
            r.reports = std::mem::take(&mut live.res.reports);
            return r;
        }
    }
    // ---- verify: the spine's next steps ----
    let win_from = live.frames();
    let steps_from = live.res.steps.len();
    let mut x = Exec::at(game, cfg, origin.opts);
    let end = (target.origin + b.window as usize).min(spine.steps.len());
    let mut failed = None;
    let mut done = target.origin;
    let mut retried = false;
    for (i, step) in spine.steps.iter().enumerate().take(end).skip(target.origin) {
        let opts = x.opts;
        let Err(e) = x.play(&mut live, step) else {
            done = i + 1;
            continue;
        };
        // The step may only fail from where the branch left the player (a walk planned past a
        // trigger that pushes back, a one-way ledge): once per window, back to the spine's own
        // cell before it, around the coord triggers, then the step again.
        let retried = e.finding.is_none()
            && !std::mem::replace(&mut retried, true)
            && live.rollback().is_ok()
            && back_to(game, cfg, &mut live, &spine.positions[i]).is_ok()
            && {
                x.opts = opts;
                x.play(&mut live, step).is_ok()
            };
        if !retried {
            failed = Some((i, e));
            break;
        }
        done = i + 1;
    }
    r.window_frames = live.frames() - win_from;
    let lost = live.res.steps[steps_from..].iter().any(lost_battle);
    let (diverged, why) = match failed {
        Some((i, e)) => {
            if let Some(f) = e.finding {
                // A bug in the window is the branch's bug.
                r.findings.push(*f);
                r.outcome = Outcome::Bug;
                r.message = e.message;
                r.coverage = coverage_of(&live, origin, cfg);
                r.reports = std::mem::take(&mut live.res.reports);
                return r;
            }
            (
                true,
                format!("spine step {i} {:?} failed: {}", spine.steps[i], e.message),
            )
        }
        None => {
            let now = Progress::of(
                game,
                &live.last_end().expect("a step").world,
                &spine.spine_flags,
                &cfg.key,
            );
            let want = spine.progress(game, done, &cfg.key);
            if now == want {
                (false, String::new())
            } else {
                (true, format!("after step {done}: {}", now.diff(&want)))
            }
        }
    };
    r.coverage = coverage_of(&live, origin, cfg);
    r.reports = std::mem::take(&mut live.res.reports);
    if !diverged {
        r.outcome = Outcome::Rejoined;
        return r;
    }
    let expected = changed.is_some() || lost || lost_exploring;
    r.outcome = Outcome::Diverged { expected };
    r.message = match &changed {
        Some(c) => format!("{why} (the branch changed {c})"),
        None if lost => format!("{why} (a battle of the window was lost)"),
        None if lost_exploring => format!("{why} (a battle of the exploration was lost)"),
        None => why,
    };
    if !expected {
        let w = live.s.world();
        r.findings.push(FindingDraft {
            kind: FindingKind::Divergence,
            message: divergence_message(origin, target, &r.message),
            location: None,
            macro_: r.macros.first().copied(),
            tail: live.inputs(),
            map: w.map(),
            pos: w.objects().player().pos(),
            script: None,
            log: Vec::new(),
        });
    }
    r
}

/// The side flags ([`key::side_flag`]) set in `w` but not at position `origin` that the spine
/// sets later: a trainer the spine fights later beaten early, an item it picks up taken
/// first. They do not keep a branch from rejoining, but a window step that needed them clear
/// (talking to that trainer) failing is expected.
fn side_flags_ahead(spine: &SpineRun, origin: usize, w: &WorldSnapshot) -> Vec<u16> {
    let (Some(at), Some(last)) = (spine.positions.get(origin), spine.positions.last()) else {
        return Vec::new();
    };
    let set = |s: &WorldSnapshot, f: engine::data::FlagId| s.vars.flags.binary_search(&f).is_ok();
    (w.vars.flags.iter())
        .filter(|&&f| key::side_flag(f.0) && !set(&at.end.world, f) && set(&last.end.world, f))
        .map(|f| f.0)
        .collect()
}

/// Whether a battle of step `st` was lost (Gen3 `B_OUTCOME_LOST`, `B_OUTCOME_DREW`).
fn lost_battle(st: &crate::model::StepResult) -> bool {
    (st.summary.battles.iter()).any(|b| matches!(b.outcome, Some(2 | 3)))
}

/// A `Divergence` finding's message: the origin's segment, the target and what diverged
/// (digits stripped by the signature: step indices and cells do not split it).
pub fn divergence_message(origin: &Position, t: &Target, why: &str) -> String {
    format!(
        "after {}: {} {:?} did not rejoin: {why}",
        origin.segment.unwrap_or("the new game"),
        t.kind.name(),
        t.action
    )
}

fn stopped(r: &mut BranchResult, stop: Stop) {
    match stop {
        Stop::Finding(f) => {
            r.outcome = Outcome::Bug;
            r.message = format!("{:?}: {}", f.kind, f.message);
            r.findings.push(f);
        }
        Stop::Pruned(m) => {
            r.outcome = Outcome::Aborted;
            r.message = m;
        }
        Stop::NotTrapped => {}
    }
}

/// The branch's coverage: its steps from the origin.
fn coverage_of<G: GameData + Clone>(
    live: &Live<'_, '_, G>,
    origin: &Position,
    cfg: &Config,
) -> Coverage {
    let mut c = Coverage::default();
    let mut from = origin.map();
    for st in &live.res.steps {
        c.add_step(from, st, &cfg.key);
        from = st.end.world.field.map;
    }
    c
}

/// Plays a branch's first action.
fn play_action<G: GameData + Clone>(
    game: &G,
    spine: &SpineRun,
    origin: &Position,
    cfg: &Config,
    live: &mut Live<'_, '_, G>,
    a: Action,
) -> Result<(), StepFail> {
    match a {
        Action::Macro(m) => live.step_with(&m, &mut crate::policy::Policy::new()),
        Action::Choice { step, choice } => {
            let mut x = Exec::at(
                game,
                cfg,
                StepOpts {
                    choice: Some(choice),
                    ..origin.opts
                },
            );
            // Past the option steps up to the interaction.
            for s in &spine.steps[origin.index..step as usize] {
                if !s.plays()
                    && !matches!(s, Step::Choose(_))
                    && let Err(e) = x.play(live, s)
                {
                    return Err(StepFail::Skip(crate::expand::ExpandError::NoEffect(leak(
                        e.message,
                    ))));
                }
            }
            x.opts.choice = Some(choice);
            x.play(live, &spine.steps[step as usize]).map_err(|e| {
                if e.finding.is_some() {
                    StepFail::Stop
                } else {
                    StepFail::Skip(crate::expand::ExpandError::NoEffect("choice"))
                }
            })
        }
    }
}

/// A `&'static str` for an error text (rare: a choice target's option step failing).
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

enum Recovered {
    Back,
    Stranded(String),
    Bug,
}

/// The maps one warp or crossing away from `map`.
fn exits<G: GameData>(game: &G, map: MapId) -> Vec<MapId> {
    let d = game.map(map);
    let mut out: Vec<MapId> = (d.warps.iter().map(|w| w.dest_map))
        .chain(d.connections.iter().map(|c| c.map))
        .filter(|&m| game.has_map(m))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether the player on `origin`'s map reaches the origin's cell or a cell next to it (an
/// object may stand on it).
fn near_origin<G: GameData + Clone>(game: &G, live: &Live<'_, '_, G>, origin: &Position) -> bool {
    let Some(end) = live.last_end() else {
        return true;
    };
    if end.world.field.map != origin.map() {
        return false;
    }
    let reach = path::reach(game, &end.world);
    let (x, y) = origin.pos();
    [(0, 0), (0, 1), (0, -1), (1, 0), (-1, 0)]
        .iter()
        .any(|(dx, dy)| reach.dist(x + dx, y + dy).is_some())
}

/// Every crossing from `snap`'s map into `origin`'s map whose edge cell the player reaches
/// and whose arrival cell reaches the origin's cell (on the origin's map with its objects,
/// carrying the player's elevation over: a surfer crossing onto land is ashore, Gen3
/// `CanStopSurfing`), nearest edge cell first.
fn crossings_into<G: GameData>(game: &G, snap: &WorldSnapshot, origin: &Position) -> Vec<Macro> {
    let here = game.map(snap.field.map);
    let there_map = game.map(origin.map());
    let reach = path::reach_all(game, snap);
    let e = snap.objects.objs[0].current_elevation();
    let to = origin.pos();
    let (w, h) = (here.width as i16, here.height as i16);
    let mut out: Vec<(u16, Macro)> = Vec::new();
    for c in here.connections.iter().filter(|c| c.map == origin.map()) {
        let edge: Vec<(i16, i16)> = match c.dir {
            Dir::Up => (0..w).map(|x| (x, 0)).collect(),
            Dir::Down => (0..w).map(|x| (x, h - 1)).collect(),
            Dir::Left => (0..h).map(|y| (0, y)).collect(),
            Dir::Right => (0..h).map(|y| (w - 1, y)).collect(),
        };
        let (dx, dy) = c.dir.delta();
        for (x, y) in edge {
            let Some(d) = reach.dist(x, y) else { continue };
            let Some(n) = engine::grid::neighbor(game, here, x + dx, y + dy) else {
                continue;
            };
            let block = there_map.blocks[n.y as usize * there_map.width as usize + n.x as usize];
            let en = engine::data::block_elevation(block);
            let start = if en == 0 || en == 15 { e } else { en };
            let there = path::Grid::of_snapshot(game, &origin.end.world).with_elevation(start);
            if n.map != origin.map() || !there.walkable((n.x, n.y), false) {
                continue;
            }
            let back = there.distances((n.x, n.y), false);
            let i = to.1 as usize * there.map.width as usize + to.0 as usize;
            if back.get(i).is_some_and(|&b| b != u16::MAX) {
                out.push((d, Macro::of(MacroKind::Cross { x, y, dir: c.dir })));
            }
        }
    }
    out.sort_by_key(|&(d, _)| d);
    out.into_iter().map(|(_, m)| m).collect()
}

/// The maps one warp or crossing from `from` that lead to `to` over the static map graph,
/// nearest to `to` first (ties in map order), the ones in `visited` after the others.
fn toward<G: GameData>(game: &G, from: MapId, to: MapId, visited: &BTreeSet<MapId>) -> Vec<MapId> {
    let dist = |start: MapId| -> Option<usize> {
        let mut q = VecDeque::from([(start, 0usize)]);
        let mut seen = BTreeSet::from([start]);
        while let Some((m, d)) = q.pop_front() {
            if m == to {
                return Some(d);
            }
            for n in exits(game, m) {
                if seen.insert(n) {
                    q.push_back((n, d + 1));
                }
            }
        }
        None
    };
    let mut out: Vec<(bool, usize, MapId)> = (exits(game, from).into_iter())
        .filter(|&n| n != from)
        .filter_map(|n| Some((visited.contains(&n), dist(n)?, n)))
        .collect();
    out.sort();
    out.into_iter().map(|(_, _, n)| n).collect()
}

/// Back to the origin's map (warps and crossings toward it), then to its cell and facing when
/// the spine's next step needs them.
fn recover<G: GameData + Clone>(
    game: &G,
    spine: &SpineRun,
    origin: &Position,
    cfg: &Config,
    live: &mut Live<'_, '_, G>,
    from_frames: u64,
) -> Recovered {
    let b = &cfg.branch;
    let over = |live: &Live<'_, '_, G>| live.frames() - from_frames > u64::from(b.recover_frames);
    let mut hops = 0;
    let mut visited = BTreeSet::new();
    // Off the origin's map, or on it where its cell is out of reach (another water body,
    // below a ledge): out and back in by a way that arrives on the origin's side.
    while live.s.world().map() != origin.map() || !near_origin(game, live, origin) {
        hops += 1;
        if hops > b.recover_hops || over(live) {
            return Recovered::Stranded(format!(
                "not back on map {:#06x} after {} hops",
                origin.map().0,
                hops - 1
            ));
        }
        let here = live.s.world().map();
        visited.insert(here);
        // The neighbours nearest to the origin's map first (maps this recovery has not been
        // on first): the nearest may only be reachable from another part of this map.
        let nexts = toward(game, here, origin.map(), &visited);
        if nexts.is_empty() {
            return Recovered::Stranded(format!("no way from map {:#06x}", here.0));
        }
        let snap = live.last_end().expect("a step").world.clone();
        let data = game.map(here);
        let mut all = candidates(game, &snap, cfg.key.cell_shift);
        // Into the origin's map: first the crossings that arrive where the origin's cell is
        // reachable (a candidate crosses at the middle of the edge only, maybe into another
        // water body or ledge side than the origin's).
        if nexts.contains(&origin.map()) {
            let mut into = crossings_into(game, &snap, origin);
            into.append(&mut all);
            all = into;
        }
        let mut moved = false;
        'next: for &next in &nexts {
            let ways = all.iter().filter(|m| match m.kind {
                MacroKind::Enter { x, y, .. } => data
                    .warps
                    .iter()
                    .any(|w| (w.x, w.y) == (x, y) && w.dest_map == next),
                MacroKind::Cross { dir, .. } => neighbour(data, dir) == Some(next),
                _ => false,
            });
            for m in ways {
                match live.step_with(m, &mut crate::policy::Policy::new()) {
                    Ok(()) => {
                        moved = true;
                        break 'next;
                    }
                    Err(StepFail::Skip(_)) => {
                        if live.rollback().is_err() {
                            return Recovered::Bug;
                        }
                    }
                    Err(StepFail::Stop) => {
                        return match &live.res.stop {
                            Some(Stop::Finding(_)) => Recovered::Bug,
                            _ => Recovered::Stranded("a recovery macro timed out".into()),
                        };
                    }
                }
            }
        }
        if !moved {
            return Recovered::Stranded(format!(
                "no reachable way from map {:#06x} toward {:#06x}",
                here.0, nexts[0].0
            ));
        }
    }
    let next_step = (spine.steps[origin.index..].iter()).find(|s| s.plays());
    if !next_step.is_some_and(|s| s.needs_cell()) {
        return Recovered::Back;
    }
    let mut x = Exec::new(game, cfg);
    let (cx, cy) = origin.pos();
    let at = Step::Goto {
        map: origin.map(),
        x: cx,
        y: cy,
    };
    for s in [at, Step::Face(origin.facing())] {
        if over(live) {
            return Recovered::Stranded("over the recovery budget".into());
        }
        if let Err(e) = x.play(live, &s) {
            return match e.finding {
                Some(f) => {
                    live.res.stop = Some(Stop::Finding(*f));
                    Recovered::Bug
                }
                None => Recovered::Stranded(format!("{s:?}: {}", e.message)),
            };
        }
    }
    Recovered::Back
}

/// The cells of `map`'s coord events that would run now (their var holds their value): a walk
/// back to the spine steps around them (Route 111's sandstorm pushes the player back from the
/// desert, forever).
fn active_triggers<G: GameData>(game: &G, snap: &WorldSnapshot) -> Vec<(i16, i16)> {
    let var = |v: engine::data::VarId| {
        (snap.vars.vars.iter())
            .find(|(id, _)| *id == v)
            .map_or(0, |&(_, x)| x)
    };
    (game.map(snap.field.map).coords.iter())
        .filter(|c| var(c.var) == c.value)
        .map(|c| (c.x, c.y))
        .collect()
}

/// Walks to position `p`'s cell around the active coord triggers ([`active_triggers`]; a
/// plain `Goto` when no path avoids them), then faces its way. On `p`'s map only.
fn back_to<G: GameData + Clone>(
    game: &G,
    cfg: &Config,
    live: &mut Live<'_, '_, G>,
    p: &Position,
) -> Result<(), crate::spine::ExecError> {
    let no = |message: String| crate::spine::ExecError {
        message,
        finding: None,
    };
    if live.s.world().map() != p.map() {
        return Err(no("not on the position's map".into()));
    }
    let mut x = Exec::new(game, cfg);
    let snap = (live.s.world().snapshot(game)).map_err(|e| no(format!("{e:?}")))?;
    let avoid: Vec<(i16, i16)> = (active_triggers(game, &snap).into_iter())
        .filter(|&c| c != p.pos())
        .collect();
    let around = !avoid.is_empty()
        && x.goto_avoiding(live, p.map(), p.pos(), &avoid, Opts::default())
            .is_ok();
    if !around {
        let (gx, gy) = p.pos();
        x.play(
            live,
            &Step::Goto {
                map: p.map(),
                x: gx,
                y: gy,
            },
        )?;
    }
    x.play(live, &Step::Face(p.facing()))
}

/// The signature of a branch finding, with its kind (for tests and repro).
pub fn signature(f: &FindingDraft) -> u64 {
    findings::signature(f)
}
