//! Playing macros on a live session: [`Live`] (the spine's and the branches' player) and
//! [`simulate`] (one job of given macros, or a softlock probe), plus the softlock probe
//! [`trap`].

use crate::config::Config;
use crate::expand::{self, ExpandError, Expanded, Intents};
use crate::findings;
use crate::guard::{self, Panicked};
use crate::key;
use crate::macros;
use crate::model::{
    FindingDraft, FindingKind, Job, JobResult, Macro, MacroKind, Opts, Plan, StepResult, Stop,
};
use crate::observe::{NoObserver, StepObserver};
use crate::path;
use crate::policy::Policy;
use crate::runs;
use crate::softlock::{self, Trap};
use engine::checkpoint::Checkpoint;
use engine::data::{GameData, MapId, ScriptId};
use game_native::Session;
use game_native::trace::{Run, rle};
use std::sync::Arc;

/// Makes the observer of a fresh session (the detectors, or nothing).
pub type MakeObserver<'a, G> = dyn FnMut() -> Box<dyn StepObserver<G>> + 'a;

/// Runs one job on a fresh session built on this thread. `Plan::Softlock` probes the start
/// state with [`trap`] (no step: `Stop::NotTrapped` or a `Softlock` finding).
pub fn simulate<G: GameData + Clone>(
    game: &G,
    job: &Job,
    cfg: &Config,
    obs: &mut MakeObserver<'_, G>,
) -> JobResult {
    simulate_with(game, job, cfg, obs, &mut Policy::new())
}

/// [`simulate`] with any [`Intents`] (a test seam: fake policies).
pub fn simulate_with<G: GameData + Clone>(
    game: &G,
    job: &Job,
    cfg: &Config,
    obs: &mut MakeObserver<'_, G>,
    intents: &mut dyn Intents,
) -> JobResult {
    let ms = match &job.plan {
        &Plan::Softlock { k, breadth } => {
            let mut res = empty_result(job.index);
            res.stop = Some(match job.start.as_deref() {
                None => Stop::Pruned("softlock probe without a start state".into()),
                Some(start) => {
                    let probed = trap(game, start, k, breadth, cfg);
                    res.probe_frames = probed.frames;
                    match probed.trap {
                        Trap::Trapped { reached } => {
                            Stop::Finding(softlock_finding(start, probed.place, k, reached))
                        }
                        _ => Stop::NotTrapped,
                    }
                }
            });
            return res;
        }
        Plan::Given(ms) => ms,
    };
    let mut live = match Live::start(game, cfg, job.start.clone(), obs) {
        Ok(l) => l,
        Err(stop) => {
            let mut res = empty_result(job.index);
            res.stop = Some(stop);
            return res;
        }
    };
    live.res.index = job.index;
    // A new game is not capturable: settle it first (its own step).
    if job.start.is_none() && ms.first().map(|m| m.kind) != Some(MacroKind::Settle) {
        let settle = Macro::of(MacroKind::Settle);
        if let Err(e) = live.step_with(&settle, intents) {
            live.stop_on(&settle, e);
            return live.finish();
        }
    }
    for m in ms {
        if let Err(e) = live.step_with(m, intents) {
            live.stop_on(m, e);
            break;
        }
    }
    live.finish()
}

fn empty_result(index: u32) -> JobResult {
    JobResult {
        index,
        steps: Vec::new(),
        stop: None,
        probe_frames: 0,
        slow_frames: 0,
        notes: Vec::new(),
        reports: Vec::new(),
    }
}

/// Why [`Live::step`] failed.
#[derive(Clone, Debug)]
pub enum StepFail {
    /// The macro does not apply here or had no effect (`NoPath`/`NoEffect`): the session is
    /// mid-expansion; [`Live::rollback`] to go on from the last step's end.
    Skip(ExpandError),
    /// A finding or a prune (a timeout, an end that is not capturable), now the job's stop
    /// (`JobResult::stop`).
    Stop,
}

/// A session at a start state (a checkpoint, or a new game) playing macros one after the other,
/// each a [`StepResult`], with the observer, the reports and the findings of [`simulate`]'s
/// jobs. Built on the thread that uses it (`World` is `!Send`).
pub struct Live<'a, 'o, G: GameData + Clone> {
    game: &'a G,
    cfg: &'a Config,
    obs_make: &'o mut MakeObserver<'o, G>,
    /// The start state (`None`: a new game).
    pub start: Option<Arc<Checkpoint>>,
    pub s: Session<G>,
    obs: Box<dyn StepObserver<G>>,
    pub res: JobResult,
    /// The last script started over the steps (see `findings` on `script`).
    last_script: Option<ScriptId>,
    /// The options [`Live::step`] applies to every macro (the spine's sticky `Choose` and
    /// `Battle` steps).
    pub opts: Opts,
}

impl<'a, 'o, G: GameData + Clone> Live<'a, 'o, G> {
    /// A session at `start` (a new game for `None`) with a fresh observer. A panic while
    /// building the world is a `Panic` stop on the start map (the new game's for `World::new`).
    pub fn start(
        game: &'a G,
        cfg: &'a Config,
        start: Option<Arc<Checkpoint>>,
        obs_make: &'o mut MakeObserver<'o, G>,
    ) -> Result<Self, Stop> {
        let s = session(game, start.as_deref(), &[])?;
        let obs = obs_make();
        Ok(Live {
            game,
            cfg,
            obs_make,
            start,
            s,
            obs,
            res: empty_result(0),
            last_script: None,
            opts: Opts::default(),
        })
    }

    /// The end state of the last step (or the start; `None`: a new game not stepped yet).
    pub fn last_end(&self) -> Option<Arc<Checkpoint>> {
        self.res
            .steps
            .last()
            .map(|st| st.end.clone())
            .or_else(|| self.start.clone())
    }

    /// Frames of the steps so far.
    pub fn frames(&self) -> u64 {
        self.res
            .steps
            .iter()
            .map(|st| u64::from(st.summary.frames))
            .sum()
    }

    /// Every input from the start, the steps' in order.
    pub fn inputs(&self) -> Vec<Run> {
        rle(self.res.steps.iter().flat_map(|st| runs::keys(&st.inputs)))
    }

    /// Back to the last step's end (or the start) on a fresh session and observer, after a
    /// [`StepFail::Skip`].
    pub fn rollback(&mut self) -> Result<(), Stop> {
        let tail = self.inputs();
        let at = self.last_end();
        self.s = session(self.game, at.as_deref(), &tail)?;
        self.obs = (self.obs_make)();
        Ok(())
    }

    /// Plays `m`'s kind with [`Live::opts`] and the policy.
    pub fn step(&mut self, m: &Macro) -> Result<(), StepFail> {
        let m = Macro {
            kind: m.kind,
            opts: self.opts,
        };
        self.step_with(&m, &mut Policy::new())
    }

    /// Plays `m` as given (its own options) with `intents`. On `StepFail::Stop` the job's stop
    /// is set (a finding marks the last step `bug`).
    pub fn step_with(&mut self, m: &Macro, intents: &mut dyn Intents) -> Result<(), StepFail> {
        match self.try_step(m, intents) {
            Ok(()) => Ok(()),
            Err(Ok(e)) => Err(StepFail::Skip(e)),
            Err(Err(stop)) => {
                if let (Stop::Finding(_), Some(last)) = (&stop, self.res.steps.last_mut()) {
                    last.bug = true;
                }
                self.res.stop = Some(stop);
                Err(StepFail::Stop)
            }
        }
    }

    /// Replaces the session's world by `end` (a spine bypass, `spine::Bypass`): a step of no
    /// input and no frame, so the inputs stay those played.
    pub fn load_bypass(&mut self, end: Checkpoint) -> Result<(), String> {
        self.s.load(&end).map_err(|e| format!("{e:?}"))?;
        let map = end.world.field.map;
        let pos = end.world.objects.objs[0].pos;
        self.res.steps.push(StepResult {
            macro_: Macro::of(MacroKind::Wait { frames: 0 }),
            inputs: Vec::new(),
            end: Arc::new(end),
            summary: crate::model::SimSummary {
                maps: vec![map],
                cells: [(map, pos.0, pos.1)].into(),
                ..Default::default()
            },
            bug: false,
        });
        self.obs = (self.obs_make)();
        Ok(())
    }

    /// Records a skipped `m` as the job's stop (a prune), as [`simulate`] does.
    pub fn stop_on(&mut self, m: &Macro, e: StepFail) {
        if let StepFail::Skip(e) = e {
            self.res.stop = Some(Stop::Pruned(format!("{m:?}: {e:?}")));
        }
    }

    /// The result so far (the steps, the stop, the reports).
    pub fn finish(self) -> JobResult {
        self.res
    }

    /// Expands `m` into a step. `Err(Ok(e))`: a skippable `NoPath`/`NoEffect`; `Err(Err(_))`:
    /// the job stops (a finding, or a prune).
    fn try_step(
        &mut self,
        m: &Macro,
        intents: &mut dyn Intents,
    ) -> Result<(), Result<ExpandError, Stop>> {
        let r = expand::expand_with(&mut self.s, m, intents, self.obs.as_mut(), &self.cfg.limits);
        // Every expansion starts at the end of the job's steps, so a report's tail is valid
        // even when the expansion fails and the session is rolled back.
        let ex = match &r {
            Ok(ex) | Err((_, ex)) => ex,
        };
        for rep in &ex.reports {
            let f = report_finding(&self.res.steps, m, ex, rep, self.last_script);
            let sig = findings::signature(&f);
            if !self
                .res
                .reports
                .iter()
                .any(|g| findings::signature(g) == sig)
            {
                self.res.reports.push(f);
            }
        }
        match r {
            Ok(ex) => {
                self.last_script = ex.last_script.or(self.last_script);
                let end = self
                    .s
                    .checkpoint()
                    .map_err(|e| Err(Stop::Pruned(format!("{m:?}: ended {e:?}"))))?;
                self.res.slow_frames += ex.slow.len() as u32;
                self.res.notes.extend(ex.notes);
                self.res.steps.push(StepResult {
                    macro_: *m,
                    inputs: rle(ex.keys),
                    end: Arc::new(end),
                    summary: ex.summary,
                    bug: false,
                });
                Ok(())
            }
            Err((e @ (ExpandError::NoPath | ExpandError::NoEffect(_)), _)) => Err(Ok(e)),
            Err((ExpandError::Timeout(t), _)) => {
                let w = self.s.world();
                Err(Err(Stop::Pruned(format!(
                    "{m:?}: timeout ({t}) on {:#06x} at {:?}, frame {}, script {:?} waiting on \
                     {:?}, battle {}, transition {}, text open {}",
                    w.map().0,
                    w.objects().player().pos(),
                    w.frame(),
                    w.last_script(),
                    w.script_waiting(),
                    w.in_battle(),
                    w.transition_active(),
                    w.text().is_open()
                ))))
            }
            Err((ExpandError::Panicked(p), ex)) => Err(Err(Stop::Finding(finding(
                &self.s,
                &self.res.steps,
                m,
                (FindingKind::Panic, p.message, p.location),
                ex,
                self.last_script,
            )))),
            Err((ExpandError::Flagged(f), ex)) => Err(Err(Stop::Finding(finding(
                &self.s,
                &self.res.steps,
                m,
                (f.kind, f.message, None),
                ex,
                self.last_script,
            )))),
        }
    }
}

/// A session at `start` (a new game for `None`) under the guard. A panic is a `Panic` stop on
/// the start map (the new game's for `World::new`, as `findings::reproduce` expects), whose
/// tail is `tail` (the inputs that led to this start within the job).
fn session<G: GameData + Clone>(
    game: &G,
    start: Option<&Checkpoint>,
    tail: &[Run],
) -> Result<Session<G>, Stop> {
    let panicked = |p: Panicked, map: MapId, pos: (i16, i16)| {
        Stop::Finding(FindingDraft {
            kind: FindingKind::Panic,
            message: p.message,
            location: p.location,
            macro_: None,
            tail: tail.to_vec(),
            map,
            pos,
            script: None,
            log: Vec::new(),
        })
    };
    let g = game.clone();
    match start {
        Some(cp) => {
            let at = (cp.world.field.map, cp.world.objects.objs[0].pos);
            guard::catch(|| Session::restore(g, cp))
                .map_err(|p| panicked(p, at.0, at.1))?
                .map_err(|e| Stop::Pruned(format!("restore failed: {e:?}")))
        }
        None => guard::catch(|| Session::new(g)).map_err(|p| {
            let ng = game.new_game();
            panicked(p, ng.map, (ng.x, ng.y))
        }),
    }
}

/// The softlock probe's depth on `map`: `softlock_k_per_map[map]`, else `softlock_k`.
pub fn softlock_k(cfg: &Config, map: MapId) -> u32 {
    cfg.detect
        .softlock_k_per_map
        .get(&map.0)
        .copied()
        .unwrap_or(cfg.detect.softlock_k)
}

/// Frames of the probe's [`MacroKind::Wait`]: long enough for a wandering NPC to step off a
/// cell it blocks (the movement types pause one to four seconds between steps).
pub const PROBE_WAIT_FRAMES: u16 = 300;

/// Whether the probe resumes `m` after a battle interrupted it, the player standing at `pos`:
/// a walk whose target is not reached yet. `WalkTo` until the player stands on its cell,
/// `Inspect` until the player stands on its approach cell, `Enter`/`Cross` while the map is
/// the same (the caller resumes only inside the start's place). `Talk` is never resumed: a
/// trainer's battle is what the talk was for, and talking again would only repeat it.
fn resumable(m: &Macro, pos: (i16, i16)) -> bool {
    match m.kind {
        MacroKind::WalkTo { x, y } => pos != (x, y),
        MacroKind::Inspect { x, y, dir } => {
            let (dx, dy) = dir.delta();
            pos != (x - dx, y - dy)
        }
        MacroKind::Enter { .. } | MacroKind::Cross { .. } => true,
        _ => false,
    }
}

/// A softlock probe's verdict, the start's place (`key::place_region`), the frames it
/// stepped and the re-probes it needed (`softlock::probe_waiting`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Probed {
    pub trap: Trap,
    pub place: u64,
    pub frames: u64,
    pub reprobes: u32,
}

/// The softlock probe of `start` (`softlock::probe` with the start's place and its map's
/// exits). The probe compares places (`key::place_region`: map, story flags and vars), not
/// regions: a level-up or a faint inside a sealed pocket changes the party bucket, not the
/// way out. From each state, a [`MacroKind::Wait`] of [`PROBE_WAIT_FRAMES`] (objects that
/// wander may free a way out), then every candidate macro, each expanded from a fresh session
/// restored there (the policy, no detectors), at most `remaining` successful ones per state.
///
/// A walk that a battle interrupted before its target (a wild encounter in tall grass, a
/// trainer's sight; see `resumable`) is resumed up to `cfg.detect.probe_resumes` times while
/// it stays in the start's place (Task H: on Routes 101 and 102 every walk from a cell by the
/// grass met an encounter after a step or two, so three macros never got out and the probe
/// reported those cells as traps). Each resume runs under its own panic guard: a panic keeps
/// the last good end state.
///
/// A trapped start whose map has an object that walks by itself is probed again after waits
/// (Task N, `softlock::probe_waiting`: up to `cfg.detect.reprobes` waits of
/// `cfg.detect.reprobe_wait_frames`), so an object that blocks the way out for a while is not
/// a softlock. Worst case, one call expands `(1 + reprobes) x (1 + probe_resumes) x breadth`
/// macros (1,600 by default) and steps `reprobes x reprobe_wait_frames` frames of waits.
///
/// A wait that ends in another place counts as a way out. A place includes the story flags
/// and vars, so a script that loops while the player waits and changes a story var (an
/// `on_frame` script, a timer) frees every trapped start of its map: such a softlock is
/// missed (a false negative), never invented.
pub fn trap<G: GameData + Clone>(
    game: &G,
    start: &Checkpoint,
    k: u32,
    breadth: u32,
    cfg: &Config,
) -> Probed {
    let place = key::place_region(&key::key_parts(&start.world, &cfg.key));
    let map = start.world.field.map;
    let exits = softlock::map_exits(game, map);
    let mut frames = 0u64;
    let mut expand_all = |cp: &Checkpoint, remaining: u32| {
        expand_all(game, cp, remaining, (place, map), cfg, &mut frames)
    };
    let mut waited = 0u64;
    let mut wait = |cp: &Checkpoint| wait_from(game, cp, cfg, &mut waited);
    let (trap, reprobes) = softlock::probe_waiting(
        start,
        place,
        exits,
        k,
        breadth,
        cfg.detect.reprobes,
        &mut expand_all,
        &mut wait,
    );
    Probed {
        trap,
        place,
        frames: frames + waited,
        reprobes,
    }
}

/// A [`MacroKind::Wait`] of `cfg.detect.reprobe_wait_frames` from `cp` (the policy resolves anything
/// that starts), under a panic guard: (place, map, end state). Its frames count in `frames`.
fn wait_from<G: GameData + Clone>(
    game: &G,
    cp: &Checkpoint,
    cfg: &Config,
    frames: &mut u64,
) -> Option<(u64, MapId, Checkpoint)> {
    let mut s = guard::catch(|| Session::restore(game.clone(), cp))
        .ok()?
        .ok()?;
    let start = s.world().frame();
    let wait = Macro {
        kind: MacroKind::Wait {
            frames: cfg.detect.reprobe_wait_frames,
        },
        opts: Opts::default(),
    };
    let ok = guard::catch(|| {
        expand::expand(
            &mut s,
            &wait,
            &mut Policy::new(),
            &mut NoObserver,
            &cfg.limits,
        )
        .is_ok()
    });
    *frames += u64::from(s.world().frame().wrapping_sub(start));
    if ok != Ok(true) {
        return None;
    }
    let end = s.checkpoint().ok()?;
    let place = key::place_region(&key::key_parts(&end.world, &cfg.key));
    Some((place, end.world.field.map, end))
}

/// The states one successful macro away from `cp`: (place, map, end state), the wait then
/// the candidates in order, at most `remaining`. A panic or a failed expansion skips the macro.
/// Every stepped frame counts in `frames`.
///
/// The list stops after the first state that leaves `start` (its (place, map)):
/// `softlock::probe` returns `Free` on that element and never reads the rest, so the verdict
/// is the same, without expanding the remaining candidates (Task H: most probes are free, and
/// the full first level cost up to `breadth` expansions each).
fn expand_all<G: GameData + Clone>(
    game: &G,
    cp: &Checkpoint,
    remaining: u32,
    start: (u64, MapId),
    cfg: &Config,
    frames: &mut u64,
) -> Vec<(u64, MapId, Checkpoint)> {
    let restore = || {
        guard::catch(|| Session::restore(game.clone(), cp))
            .ok()
            .and_then(Result::ok)
    };
    let Some(s) = restore() else {
        return Vec::new();
    };
    let Ok(snap) = s.world().snapshot(s.game()) else {
        return Vec::new();
    };
    let reach = path::reach(game, &snap);
    let wait = Macro {
        kind: MacroKind::Wait {
            frames: PROBE_WAIT_FRAMES,
        },
        opts: Opts::default(),
    };
    let cands = std::iter::once(wait).chain(macros::candidates_with(
        game,
        &snap,
        &reach,
        cfg.key.cell_shift,
    ));
    let mut first = Some(s);
    let mut out = Vec::new();
    for m in cands {
        if out.len() as u32 >= remaining {
            break;
        }
        let Some(mut s) = first.take().or_else(restore) else {
            break;
        };
        let start_frame = s.world().frame();
        // Each expansion under its own guard: a panic in a resume keeps the last good end.
        let mut end = None;
        for _ in 0..=cfg.detect.probe_resumes {
            // Only whether a battle interrupted it matters (the error is dropped).
            let Ok(Some(battled)) = guard::catch(|| {
                expand::expand(&mut s, &m, &mut Policy::new(), &mut NoObserver, &cfg.limits)
                    .ok()
                    .map(|ex| !ex.summary.battles.is_empty())
            }) else {
                break;
            };
            let Ok(cp) = s.checkpoint() else {
                break;
            };
            let here = (
                key::place_region(&key::key_parts(&cp.world, &cfg.key)),
                cp.world.field.map,
            );
            let pos = cp.world.objects.objs[0].pos;
            let resume = battled && resumable(&m, pos);
            end = Some((here, cp));
            if !resume || here != start {
                break;
            }
        }
        // A panicked session's frame counter is still readable (the step that panicked did
        // not count).
        *frames += u64::from(s.world().frame().wrapping_sub(start_frame));
        if let Some(((place, map), end)) = end {
            out.push((place, map, end));
            if (place, map) != start {
                break;
            }
        }
    }
    out
}

/// The `Softlock` finding of a trapped start state: an empty tail (the node itself).
pub fn softlock_finding(start: &Checkpoint, place: u64, k: u32, reached: u32) -> FindingDraft {
    let map = start.world.field.map;
    FindingDraft {
        kind: FindingKind::Softlock,
        message: findings::softlock_message(place, k, reached),
        location: None,
        macro_: None,
        tail: Vec::new(),
        map,
        pos: start.world.objects.objs.first().map_or((0, 0), |o| o.pos),
        script: None,
        log: Vec::new(),
    }
}

/// A finding at the session's current frame. Its tail runs from the job's start (its
/// parent's end state, or a new game) to that frame: the job's steps so far, then the failed
/// macro's inputs. Its script is the last one started over that tail (`job_script`: the
/// steps').
fn finding<G: GameData>(
    s: &Session<G>,
    steps: &[StepResult],
    m: &Macro,
    (kind, message, location): (FindingKind, String, Option<String>),
    ex: Expanded,
    job_script: Option<ScriptId>,
) -> FindingDraft {
    let w = s.world();
    let keys = steps.iter().flat_map(|st| runs::keys(&st.inputs));
    FindingDraft {
        kind,
        message,
        location,
        macro_: Some(*m),
        tail: rle(keys.chain(ex.keys)),
        map: w.map(),
        pos: w.objects().player().pos(),
        script: ex.last_script.or(job_script),
        log: w.log().lines().collect(),
    }
}

/// The report-only `Unsupported` finding of a script's report during `m` (expanded as `ex`):
/// its tail ends on the reporting frame.
fn report_finding(
    steps: &[StepResult],
    m: &Macro,
    ex: &Expanded,
    rep: &expand::Reported,
    job_script: Option<ScriptId>,
) -> FindingDraft {
    let keys = steps.iter().flat_map(|st| runs::keys(&st.inputs));
    let upto = (rep.frames as usize).min(ex.keys.len());
    FindingDraft {
        kind: FindingKind::Unsupported,
        message: findings::report_message(rep.report),
        location: None,
        macro_: Some(*m),
        tail: rle(keys.chain(ex.keys[..upto].iter().copied())),
        map: rep.map,
        pos: rep.pos,
        script: rep.script.or(job_script),
        log: Vec::new(),
    }
}
