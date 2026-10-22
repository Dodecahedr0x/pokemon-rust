//! One job on a fresh session (Task A).

use crate::config::Config;
use crate::expand::{self, ExpandError, Expanded, Intents};
use crate::graph::Rng64;
use crate::guard::{self, Panicked};
use crate::macros;
use crate::model::{
    FindingDraft, FindingKind, Job, JobResult, Macro, MacroKind, Opts, Plan, StepResult, Stop,
};
use crate::observe::StepObserver;
use crate::path;
use crate::policy::Policy;
use crate::probe::Probe;
use crate::runs;
use engine::data::{GameData, MapId};
use game_native::Session;
use game_native::trace::rle;
use std::sync::Arc;

/// Candidates tried per `Plan::Random` step before the job is pruned.
pub const RANDOM_TRIES: usize = 8;

/// Runs one job on a fresh session built on this thread. Plan::Softlock is wired by Task G.
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
        },
    };
    if let Err(stop) = sim.run(job) {
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
}

type Live<G> = (Session<Probe<G>>, Box<dyn StepObserver<G>>);

impl<G: GameData + Clone> Sim<'_, G> {
    fn session(&mut self, start: Option<&engine::checkpoint::Checkpoint>) -> Result<Live<G>, Stop> {
        let probe = Probe::new(self.game.clone());
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
        if let Plan::Softlock { .. } = job.plan {
            return Err(Stop::Pruned("unwired".into()));
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
    /// skipped (the session restored) up to [`RANDOM_TRIES`] tries.
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
            let m = cands.remove(rng.below(cands.len() as u64) as usize);
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
        match expand::expand_with(s, m, self.intents, obs.as_mut(), &self.cfg.limits) {
            Ok(ex) => {
                let end = s
                    .checkpoint()
                    .map_err(|e| Err(Stop::Pruned(format!("{m:?}: ended {e:?}"))))?;
                self.res.steps.push(StepResult {
                    macro_: *m,
                    inputs: rle(ex.keys),
                    end: Arc::new(end),
                    summary: ex.summary,
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
            )))),
            Err((ExpandError::Flagged(f), ex)) => Err(Err(Stop::Finding(finding(
                s,
                &self.res.steps,
                m,
                (f.kind, f.message, None),
                ex,
            )))),
        }
    }
}

/// A finding at the session's current frame. Its tail runs from the job's start (its
/// parent's end state, or a new game) to that frame: the job's steps so far, then the failed
/// macro's inputs.
fn finding<G: GameData>(
    s: &Session<Probe<G>>,
    steps: &[StepResult],
    m: &Macro,
    (kind, message, location): (FindingKind, String, Option<String>),
    ex: Expanded,
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
        script: ex.last_script,
        log: w.log().lines().collect(),
    }
}
