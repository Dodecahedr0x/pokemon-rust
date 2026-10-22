//! One job on a fresh session (Task A).

use crate::config::Config;
use crate::expand::{self, ExpandError, Expanded, Intents};
use crate::findings;
use crate::graph::Rng64;
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
use game_native::trace::rle;
use std::sync::Arc;

/// Candidates tried per `Plan::Random` step before the job is pruned.
pub const RANDOM_TRIES: usize = 8;

/// Runs one job on a fresh session built on this thread. `Plan::Softlock` probes the start
/// state with [`trap`] (no step: `Stop::NotTrapped` or a `Softlock` finding).
pub fn simulate<G: GameData + Clone>(
    game: &G,
    job: &Job,
    cfg: &Config,
    obs: &mut dyn FnMut() -> Box<dyn StepObserver<G>>,
) -> JobResult {
    simulate_with(game, job, cfg, obs, &mut Policy::new())
}

/// [`simulate`] with any [`Intents`] (a test seam: fake policies).
pub fn simulate_with<G: GameData + Clone>(
    game: &G,
    job: &Job,
    cfg: &Config,
    obs: &mut dyn FnMut() -> Box<dyn StepObserver<G>>,
    intents: &mut dyn Intents,
) -> JobResult {
    let mut sim = Sim {
        game,
        cfg,
        obs,
        intents,
        res: JobResult {
            index: job.index,
            parent: job.parent,
            steps: Vec::new(),
            stop: None,
            probe_frames: 0,
            slow_frames: 0,
            notes: Vec::new(),
            reports: Vec::new(),
        },
        last_script: None,
    };
    if let Err(stop) = sim.run(job) {
        // The bug weight goes to the step the finding follows (`StepResult::bug`).
        if let (Stop::Finding(_), Some(last)) = (&stop, sim.res.steps.last_mut()) {
            last.bug = true;
        }
        sim.res.stop = Some(stop);
    }
    sim.res
}

struct Sim<'a, G: GameData + Clone> {
    game: &'a G,
    cfg: &'a Config,
    obs: &'a mut dyn FnMut() -> Box<dyn StepObserver<G>>,
    intents: &'a mut dyn Intents,
    res: JobResult,
    /// The last script started over the job's steps (see `findings` on `script`).
    last_script: Option<ScriptId>,
}

type Live<G> = (Session<G>, Box<dyn StepObserver<G>>);

impl<G: GameData + Clone> Sim<'_, G> {
    fn session(&mut self, start: Option<&Checkpoint>) -> Result<Live<G>, Stop> {
        let probe = self.game.clone();
        // A panic while building the world is a finding on the start map (the new game's for `World::new`, as `findings::reproduce` expects).
        // The tail is the job's steps so far (empty for the job's own start), as `finding`.
        let tail = rle(self.res.steps.iter().flat_map(|st| runs::keys(&st.inputs)));
        let panicked = |p: Panicked, map: MapId, pos: (i16, i16)| {
            Stop::Finding(FindingDraft {
                kind: FindingKind::Panic,
                message: p.message,
                location: p.location,
                macro_: None,
                tail,
                map,
                pos,
                script: None,
                log: Vec::new(),
            })
        };
        let s = match start {
            Some(cp) => {
                let at = (cp.world.field.map, cp.world.objects.objs[0].pos);
                guard::catch(|| Session::restore(probe, cp))
                    .map_err(|p| panicked(p, at.0, at.1))?
                    .map_err(|e| Stop::Pruned(format!("restore failed: {e:?}")))?
            }
            None => guard::catch(|| Session::new(probe)).map_err(|p| {
                let ng = self.game.new_game();
                panicked(p, ng.map, (ng.x, ng.y))
            })?,
        };
        Ok((s, (self.obs)()))
    }

    fn run(&mut self, job: &Job) -> Result<(), Stop> {
        if let Plan::Softlock { k, breadth } = job.plan {
            let Some(start) = job.start.as_deref() else {
                return Err(Stop::Pruned("softlock probe without a start state".into()));
            };
            let probed = trap(self.game, start, k, breadth, self.cfg);
            self.res.probe_frames = probed.frames;
            return Err(match probed.trap {
                Trap::Trapped { reached } => {
                    Stop::Finding(softlock_finding(start, probed.place, k, reached))
                }
                _ => Stop::NotTrapped,
            });
        }
        let mut live = self.session(job.start.as_deref())?;
        let first = match &job.plan {
            Plan::Given(ms) => ms.first().map(|m| m.kind),
            _ => None,
        };
        // A new game is not capturable: settle it first (its own step, like the root).
        if job.start.is_none() && first != Some(MacroKind::Settle) {
            let settle = Macro {
                kind: MacroKind::Settle,
                opts: Opts::default(),
            };
            self.step(&mut live, &settle)?;
        }
        match &job.plan {
            Plan::Given(ms) => {
                for m in ms {
                    self.step(&mut live, m)?;
                }
                Ok(())
            }
            &Plan::Random { len, seed } => {
                for i in 0..len {
                    live = self.random_step(live, seed ^ u64::from(i))?;
                }
                Ok(())
            }
            Plan::Softlock { .. } => unreachable!("handled above"),
        }
    }

    /// Picks among the candidates with `Rng64(seed)`; a `NoPath`/`NoEffect` candidate is
    /// skipped (the session restored) up to [`RANDOM_TRIES`] tries. A block `WalkTo` walks to
    /// a cell of its block drawn from the same RNG (`macros::block_mates`), so the walks
    /// cover every cell, not only the block centres.
    fn random_step(&mut self, mut live: Live<G>, seed: u64) -> Result<Live<G>, Stop> {
        let s = &live.0;
        let snap = s
            .world()
            .snapshot(s.game())
            .map_err(|e| Stop::Pruned(format!("not capturable: {e:?}")))?;
        let start = s
            .checkpoint()
            .map_err(|e| Stop::Pruned(format!("not capturable: {e:?}")))?;
        let reach = path::reach(self.game, &snap);
        let mut cands = macros::candidates_with(self.game, &snap, &reach, self.cfg.key.cell_shift);
        let mut rng = Rng64(seed);
        let mut last = String::from("no candidate");
        for _ in 0..RANDOM_TRIES {
            if cands.is_empty() {
                break;
            }
            let mut m = cands.remove(rng.below(cands.len() as u64) as usize);
            if let MacroKind::WalkTo { x, y } = m.kind {
                let mates =
                    macros::block_mates(self.game, &snap, &reach, self.cfg.key.cell_shift, (x, y));
                if !mates.is_empty() {
                    let (x, y) = mates[rng.below(mates.len() as u64) as usize];
                    m.kind = MacroKind::WalkTo { x, y };
                }
            }
            match self.try_step(&mut live, &m) {
                Ok(()) => return Ok(live),
                Err(Ok(e)) => {
                    last = format!("{m:?}: {e:?}");
                    live = self.session(Some(&start))?;
                }
                Err(Err(stop)) => return Err(stop),
            }
        }
        Err(Stop::Pruned(last))
    }

    /// Expands `m`; every failure stops the job.
    fn step(&mut self, live: &mut Live<G>, m: &Macro) -> Result<(), Stop> {
        self.try_step(live, m).map_err(|e| match e {
            Ok(e) => Stop::Pruned(format!("{m:?}: {e:?}")),
            Err(stop) => stop,
        })
    }

    /// Expands `m` into a step. `Err(Ok(e))`: a skippable `NoPath`/`NoEffect`; `Err(Err(_))`:
    /// the job stops (a finding, or a prune).
    fn try_step(&mut self, live: &mut Live<G>, m: &Macro) -> Result<(), Result<ExpandError, Stop>> {
        let (s, obs) = live;
        let r = expand::expand_with(s, m, self.intents, obs.as_mut(), &self.cfg.limits);
        // Every expansion starts at the end of the job's steps, so a report's tail is valid
        // even when the expansion fails and the session is restored.
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
                let end = s
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
                Err(Err(Stop::Pruned(format!("{m:?}: timeout ({t})"))))
            }
            Err((ExpandError::Panicked(p), ex)) => Err(Err(Stop::Finding(finding(
                s,
                &self.res.steps,
                m,
                (FindingKind::Panic, p.message, p.location),
                ex,
                self.last_script,
            )))),
            Err((ExpandError::Flagged(f), ex)) => Err(Err(Stop::Finding(finding(
                s,
                &self.res.steps,
                m,
                (f.kind, f.message, None),
                ex,
                self.last_script,
            )))),
        }
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

/// A softlock probe's verdict, the start's place (`key::place_region`) and the frames it
/// stepped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Probed {
    pub trap: Trap,
    pub place: u64,
    pub frames: u64,
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
/// the last good end state. Worst case, a probe expands `(1 + probe_resumes) x breadth`
/// macros.
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
    let trap = softlock::probe(start, place, exits, k, breadth, &mut expand_all);
    Probed {
        trap,
        place,
        frames,
    }
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
fn softlock_finding(start: &Checkpoint, place: u64, k: u32, reached: u32) -> FindingDraft {
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
