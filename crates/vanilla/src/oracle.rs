//! Both spines side by side (Task O2): our engine's [`SpineRun`] and BPEE's [`Live`] on the
//! same steps, with the [`Semantic`] diff at every step boundary, the frames each game took
//! per step, and the diffs outside the tolerances as `Parity` findings ([`ParityLog`]).
//! Then the branches: each stored branch's macros replayed on both games from its origin, and
//! the end states compared.
//!
//! Vanilla starts earlier than our new game: it is booted ([`Vanilla::boot`]), driven through
//! the intro ([`Vanilla::intro`]) and aligned on our position 0 ([`Vanilla::align`]).
//! A step vanilla cannot play is a driver problem (an oracle row with its error); the run goes
//! on from wherever vanilla is, and stops after [`MAX_FAILED`] failures in a row.

use crate::live::Live;
use crate::state::Vanilla;
use anyhow::{Context, Result};
use engine::World;
use engine::data::GameData;
use explore::branch::Action;
use explore::model::{FindingDraft, FindingKind, Macro};
use explore::oracle::{History, ParityLog, parity_message, tolerance};
use explore::semantic::{Delta, Semantic, Tolerance, diff, map_name};
use explore::sim::Live as OurLive;
use explore::spine::{Exec, SpineRun, Step};
use std::collections::{BTreeMap, BTreeSet};

/// Failed vanilla steps in a row after which the spine comparison stops.
pub const MAX_FAILED: usize = 4;

/// One step boundary.
#[derive(Clone, Debug)]
pub struct Row {
    /// The position after the step (0: the start).
    pub index: usize,
    pub step: Option<Step>,
    pub segment: Option<&'static str>,
    pub ours_frames: u64,
    pub vanilla_frames: u64,
    /// Every delta under the tolerance at this boundary, but the [`Row::noise`].
    pub deltas: Vec<Delta>,
    /// The fields not already differing at the boundary before. They become findings when
    /// they still hold after the next step that moves the player ([`run_spine_range`]).
    pub new: Vec<Delta>,
    /// Deltas that follow from vanilla's RNG or timing ([`split_noise`]): never findings.
    pub noise: Vec<Delta>,
    /// Vanilla could not play the step (a driver problem, or a divergence).
    pub error: Option<String>,
    /// What vanilla played natively for a `Bypass` step ([`Live::note`]).
    pub note: Option<String>,
    /// Replays after a battle vanilla lost ([`Live::play`]).
    pub rerolls: u32,
    pub tolerance: Tolerance,
}

/// A side-by-side run.
pub struct OracleRun {
    pub rows: Vec<Row>,
    pub log: ParityLog,
    /// Vanilla's frames from power-on to the aligned start.
    pub start_frames: u64,
    /// Vanilla's save states at the positions, when asked for (branches start from them).
    pub states: Vec<Option<Vec<u8>>>,
    /// The vanilla history at each position (the tolerance a branch from there starts with).
    pub history: Vec<History>,
    pub stopped: Option<String>,
    /// [`trainer_flags`] of the spine's maps.
    pub trainers: BTreeMap<u16, bool>,
}

/// Our engine's semantic view at position `i` of `spine`.
pub fn ours_at<G: GameData>(game: &G, spine: &SpineRun, i: usize) -> Result<Semantic> {
    let w = World::restore(game, &spine.positions[i].end)
        .map_err(|e| anyhow::anyhow!("restore position {i}: {e:?}"))?;
    Ok(Semantic::from_world(&w, game))
}

/// The trainer flags of the trainer objects on the maps `spine` visits, each with whether
/// the trainer's sight depends on vanilla's RNG or frame timing: a movement type that turns or
/// walks (`LookAround`, `Wander*`, `FaceTwo`, `Rotate`, `WalkBackAndForth`…), so whether it
/// sees the player passing by differs between the games for reasons outside the rules.
pub fn trainer_flags<G: GameData>(game: &G, spine: &SpineRun) -> BTreeMap<u16, bool> {
    use engine::data::MovementType as M;
    let maps: BTreeSet<_> = spine.positions.iter().map(|p| p.map()).collect();
    let mut out = BTreeMap::new();
    for m in maps.into_iter().filter(|&m| game.has_map(m)) {
        for o in game.map(m).objects.iter().filter(|o| o.trainer_type != 0) {
            let Some(flag) = o.script.and_then(|s| game.trainer_flag_of(s)) else {
                continue;
            };
            let fixed = matches!(
                o.movement,
                M::Face(_)
                    | M::WalkInPlace(_)
                    | M::JogInPlace(_)
                    | M::RunInPlace(_)
                    | M::WalkSlowlyInPlace(_)
                    | M::Invisible
            );
            *out.entry(flag.0).or_insert(false) |= !fixed;
        }
    }
    out
}

/// Splits `deltas` into (parity deltas, noise). The noise: when the games beat different
/// trainers whose sight depends on the RNG or timing ([`trainer_flags`]), those trainer flags,
/// and what follows from those battles: the money, the Match Call registrations
/// (`FLAG_REGISTERED_*`, set on beating a rematch trainer) and the lead's exp, level and HP.
pub fn split_noise(
    ours: &Semantic,
    theirs: &Semantic,
    deltas: Vec<Delta>,
    trainers: &BTreeMap<u16, bool>,
) -> (Vec<Delta>, Vec<Delta>) {
    let timed: Vec<String> = (ours.flags.symmetric_difference(&theirs.flags))
        .filter(|f| trainers.get(f) == Some(&true))
        .map(|&f| format!("flag {}", explore::semantic::flag_name(f)))
        .collect();
    if timed.is_empty() {
        return (deltas, Vec::new());
    }
    deltas.into_iter().partition(|d| {
        let f = d.field.as_str();
        let noise = timed.iter().any(|t| t == f)
            || f == "money"
            || f.starts_with("flag FLAG_REGISTERED_")
            || (f.starts_with("party[")
                && (f.ends_with(".exp") || f.ends_with(".level") || f.ends_with(".hp")));
        !noise
    })
}

/// Splits `deltas` into (parity deltas, noise) once levels are RNG-dependent (`tol.levels`
/// off: a random wild battle was fought): the lead's species when one evolves into the other
/// (an evolution reached at another battle), and its moves when every differing slot holds a
/// level-up move of either species or nothing (a move learnt at another level-up).
pub fn split_leveling<G: GameData>(
    game: &G,
    ours: &Semantic,
    theirs: &Semantic,
    deltas: Vec<Delta>,
    tol: Tolerance,
) -> (Vec<Delta>, Vec<Delta>) {
    use engine::data::SpeciesId;
    if tol.levels {
        return (deltas, Vec::new());
    }
    let t = game.battle();
    let learns = |sp: u16, m: u16| {
        m == 0
            || (usize::from(sp) < t.species.len()
                && (t.species_info(SpeciesId(sp)).learnset.iter()).any(|l| l.mov.0 == m))
    };
    let evolves = |a: u16, b: u16| {
        (game.evolutions(SpeciesId(a)).iter()).any(|e| e.target.0 == b)
            || (game.evolutions(SpeciesId(b)).iter()).any(|e| e.target.0 == a)
    };
    deltas.into_iter().partition(|d| {
        let Some(i) = (d.field.strip_prefix("party["))
            .and_then(|r| r.split(']').next())
            .and_then(|n| n.parse::<usize>().ok())
        else {
            return true;
        };
        let (Some(a), Some(b)) = (ours.party.get(i), theirs.party.get(i)) else {
            return true;
        };
        let noise = if d.field.ends_with(".species") {
            evolves(a.species, b.species)
        } else if d.field.ends_with(".moves") {
            (a.moves.iter().zip(&b.moves)).all(|(&x, &y)| {
                x == y
                    || ((learns(a.species, x) || learns(b.species, x))
                        && (learns(a.species, y) || learns(b.species, y)))
            })
        } else {
            false
        };
        !noise
    })
}

/// The deltas of `a` whose field still differs in `b`: with the same values for the player's
/// place (a walk ending elsewhere is realigned by the next walk), with any values for the rest
/// (a counter only one game counts goes on counting).
fn kept(a: &[Delta], b: &[Delta]) -> Vec<Delta> {
    let place = |f: &str| matches!(f, "map" | "cell" | "facing");
    (a.iter())
        .filter(|d| {
            b.iter()
                .any(|x| x.field == d.field && (!place(&d.field) || x == *d))
        })
        .cloned()
        .collect()
}

/// A boundary whose deltas wait for the next boundary of a step that moves the player
/// ([`run_spine_range`]) to become findings.
struct Pending {
    at: usize,
    segment: Option<&'static str>,
    step: Option<Step>,
    map: engine::data::MapId,
    pos: (i16, i16),
    deltas: Vec<Delta>,
}

impl Pending {
    /// Records the deltas that still hold in `still` (`None`: all of them), the player's
    /// place against `place` (the last boundary before the moving step: where a scene left the
    /// player, before the next walk moves them on).
    fn record(self, log: &mut ParityLog, still: Option<(&[Delta], &[Delta])>) {
        let d = match still {
            Some((next, place)) => {
                let is_place = |d: &Delta| matches!(d.field.as_str(), "map" | "cell" | "facing");
                let (p, rest): (Vec<Delta>, Vec<Delta>) =
                    self.deltas.iter().cloned().partition(is_place);
                let mut d = kept(&rest, next);
                d.extend(kept(&p, place));
                d
            }
            None => self.deltas,
        };
        log.record(
            self.at,
            self.segment,
            self.step.as_ref(),
            self.map,
            self.pos,
            &d,
        );
    }
}

/// Updates the history with the battles vanilla fought since `from` (an index into
/// [`Vanilla::battles`]) and its party now.
fn update(h: &mut History, v: &mut Vanilla, from: usize) {
    for &flags in &v.battles[from..] {
        h.hurt_since_heal = true;
        let wild = flags & crate::state::layout::BATTLE_TYPE_TRAINER == 0;
        if wild && flags & crate::state::layout::BATTLE_TYPE_FIRST_BATTLE == 0 {
            h.random_battle = true;
        }
    }
    let party = v.party();
    if !party.is_empty() && party.iter().all(|m| m.hp >= m.max_hp) {
        h.hurt_since_heal = false;
    }
}

/// Boots BPEE, plays the intro and aligns it on our spine's position 0.
pub fn start<'g, G: GameData>(v: Vanilla, game: &'g G, spine: &SpineRun) -> Result<Live<'g, G>> {
    let mut live = Live::new(v, game);
    live.v.boot(20_000).context("boot")?;
    live.v.intro(game).context("intro")?;
    let p = &spine.positions[0];
    live.v
        .align(game, p.map(), p.pos(), p.facing())
        .context("align on our new game")?;
    Ok(live)
}

/// Plays `spine.steps` on vanilla (from [`start`]) and diffs every boundary with ours.
/// `keep_states`: save vanilla's state at every position (for [`branches`]).
pub fn run_spine<G: GameData>(
    game: &G,
    spine: &SpineRun,
    live: &mut Live<'_, G>,
    keep_states: bool,
    progress: impl FnMut(&Row),
) -> Result<OracleRun> {
    run_spine_range(game, spine, live, keep_states, Resume::default(), progress)
}

/// Where [`run_spine_range`] starts and stops: from position `from` (vanilla already stands
/// there, with history `history`: a [`Resume::load`]ed state), up to position `to`, saving
/// every position's vanilla state into `cache` ([`Resume::save`]). For iterating on the
/// driver: the whole spine takes minutes.
#[derive(Clone, Debug, Default)]
pub struct Resume {
    pub from: usize,
    pub to: Option<usize>,
    pub history: History,
    pub cache: Option<std::path::PathBuf>,
}

impl Resume {
    fn paths(dir: &std::path::Path, i: usize) -> (std::path::PathBuf, std::path::PathBuf) {
        (
            dir.join(format!("{i}.state")),
            dir.join(format!("{i}.meta")),
        )
    }

    /// Saves vanilla at position `i` into `dir`: the machine state, and the frames, the step
    /// options and the history as text.
    pub fn save<G: GameData>(
        dir: &std::path::Path,
        i: usize,
        live: &mut Live<'_, G>,
        h: History,
    ) -> Result<()> {
        let (st, meta) = Self::paths(dir, i);
        std::fs::create_dir_all(dir)?;
        std::fs::write(st, live.v.emu.save_state()?)?;
        let o = live.opts;
        std::fs::write(
            meta,
            format!(
                "{} {} {} {} {} {} {}",
                live.v.frames,
                o.run_wild as u8,
                o.best_move as u8,
                o.learn as u8,
                o.choice.map_or(255, |c| c),
                h.random_battle as u8,
                h.hurt_since_heal as u8
            ),
        )?;
        Ok(())
    }

    /// Loads position `i` saved by [`Resume::save`] into `live`; returns the history.
    pub fn load<G: GameData>(
        dir: &std::path::Path,
        i: usize,
        live: &mut Live<'_, G>,
    ) -> Result<History> {
        let (st, meta) = Self::paths(dir, i);
        live.v
            .emu
            .load_state(&std::fs::read(&st).with_context(|| format!("{}", st.display()))?)?;
        let m: Vec<u64> = (std::fs::read_to_string(meta)?.split_whitespace())
            .map(|t| t.parse())
            .collect::<Result<_, _>>()?;
        anyhow::ensure!(m.len() == 7, "bad meta at {i}");
        live.v.frames = m[0];
        live.opts = explore::spine::StepOpts {
            run_wild: m[1] != 0,
            best_move: m[2] != 0,
            learn: m[3] != 0,
            choice: (m[4] != 255).then_some(m[4] as u8),
        };
        Ok(History {
            random_battle: m[5] != 0,
            hurt_since_heal: m[6] != 0,
        })
    }
}

impl OracleRun {
    /// Puts the cache's states and histories of the positions before `from` (a cache a whole
    /// `--cache` run wrote) ahead of this `--from` run's, so a branch can start at any origin.
    pub fn prepend_cached(&mut self, dir: &std::path::Path, from: usize) -> Result<()> {
        let mut states = Vec::with_capacity(from + self.states.len());
        let mut history = Vec::with_capacity(from + self.history.len());
        for i in 0..from {
            let (st, meta) = Resume::paths(dir, i);
            states.push(Some(
                std::fs::read(&st).with_context(|| format!("{}", st.display()))?,
            ));
            let m: Vec<u64> = (std::fs::read_to_string(meta)?.split_whitespace())
                .map(|t| t.parse())
                .collect::<Result<_, _>>()?;
            anyhow::ensure!(m.len() == 7, "bad meta at {i}");
            history.push(History {
                random_battle: m[5] != 0,
                hurt_since_heal: m[6] != 0,
            });
        }
        states.append(&mut self.states);
        history.append(&mut self.history);
        (self.states, self.history) = (states, history);
        Ok(())
    }
}

/// [`run_spine`] over [`Resume`]'s range.
pub fn run_spine_range<G: GameData>(
    game: &G,
    spine: &SpineRun,
    live: &mut Live<'_, G>,
    keep_states: bool,
    resume: Resume,
    mut progress: impl FnMut(&Row),
) -> Result<OracleRun> {
    let mut out = OracleRun {
        rows: Vec::new(),
        log: ParityLog::default(),
        start_frames: live.v.frames,
        states: Vec::new(),
        history: Vec::new(),
        stopped: None,
        trainers: trainer_flags(game, spine),
    };
    let mut h = resume.history;
    let trainers = out.trainers.clone();
    // A boundary's deltas are findings only when they still hold at the boundary after the next
    // moving step (not a wait): vanilla's settle runs a trigger or frame script the step ends next to, which
    // ours runs as the next step (boundary timing), and a walk or a talk can end a cell apart
    // where the next step realigns them.
    let mut pending: Vec<Pending> = Vec::new();
    let mut prev_fields: BTreeSet<String> = BTreeSet::new();
    let mut prev_exp: Option<u32> = None;
    let mut talked = false;
    let mut failed = 0;
    let mut battles = live.v.battles.len();
    let to = resume.to.map_or(spine.positions.len(), |t| {
        (t + 1).min(spine.positions.len())
    });
    // Position 0, then one per step ours played.
    for i in resume.from..to {
        let step = i
            .checked_sub(1)
            .filter(|_| i > resume.from || resume.from == 0)
            .map(|s| spine.steps[s]);
        let before = live.v.frames;
        live.rerolls = 0;
        let ours = ours_at(game, spine, i)?;
        live.ours_lead_level = ours.party.first().map(|m| m.level);
        let error = match step {
            Some(s) => live.play(&s).err().map(|e| format!("{e:#}")),
            None => None,
        };
        // A failed step may leave a script running: settle what can be settled.
        if error.is_some() {
            let _ = live.v.settle(crate::live::MACRO_FRAMES);
        }
        let fought = live.v.battles.len() > battles;
        update(&mut h, &mut live.v, battles);
        battles = live.v.battles.len();
        let p = &spine.positions[i];
        // Our lead gained exp in a step vanilla fought nothing: our engine met a wild mon
        // vanilla did not (its RNG), so levels are RNG-dependent from here on.
        let ours_exp = ours.party.first().map(|m| m.exp);
        if !fought && step.is_some() && ours_exp > prev_exp && prev_exp.is_some() {
            h.random_battle = true;
        }
        prev_exp = ours_exp;
        let theirs = live.v.state()?.semantic_in(game);
        let tol = tolerance(h);
        let (deltas, noise) = split_noise(&ours, &theirs, diff(&ours, &theirs, tol), &trainers);
        let (deltas, leveling) = split_leveling(game, &ours, &theirs, deltas, tol);
        let mut noise: Vec<Delta> = noise.into_iter().chain(leveling).collect();
        // A talk approached from another side (the object or the way to it differed): the
        // player's cell and facing next to it are the driver's choice, not the game's, until
        // a step moves the player on (the scene's waits after the talk keep them).
        let (dx, dy) = (ours.cell.0 - theirs.cell.0, ours.cell.1 - theirs.cell.1);
        let near = ours.map == theirs.map && dx.abs() + dy.abs() <= 2;
        talked = match step {
            Some(Step::Talk { .. }) => near,
            Some(s) if !s.plays() => talked,
            Some(Step::WaitScript | Step::Wait(_) | Step::WaitFor { .. }) => talked && near,
            _ => false,
        };
        let deltas = if talked {
            let (place, rest): (Vec<Delta>, Vec<Delta>) =
                (deltas.into_iter()).partition(|d| matches!(d.field.as_str(), "cell" | "facing"));
            noise.extend(place);
            rest
        } else {
            deltas
        };
        let new: Vec<Delta> = (deltas.iter())
            .filter(|d| !prev_fields.contains(&d.field))
            .cloned()
            .collect();
        prev_fields = deltas.iter().map(|d| d.field.clone()).collect();
        if step.is_none_or(|s| s.plays()) {
            // A step that only waits (a scene our engine plays over several `WaitScript`s,
            // vanilla's settle in one) does not settle the boundaries before it: they wait for
            // the next step that moves the player.
            let waits = live.ahead
                || matches!(
                    step,
                    Some(Step::WaitScript | Step::Wait(_) | Step::WaitFor { .. })
                );
            if !waits {
                // Waits since the first pending boundary: the place after them, else now.
                let place = match pending.as_slice() {
                    [_, .., last] => last.deltas.clone(),
                    _ => deltas.clone(),
                };
                for p in pending.drain(..) {
                    p.record(&mut out.log, Some((&deltas, &place)));
                }
            }
            pending.push(Pending {
                at: i,
                segment: p.segment,
                step,
                map: p.map(),
                pos: p.pos(),
                deltas: deltas.clone(),
            });
        }
        let row = Row {
            index: i,
            step,
            segment: p.segment,
            ours_frames: p.frame
                - i.checked_sub(1)
                    .map_or(p.frame, |j| spine.positions[j].frame),
            vanilla_frames: live.v.frames - before,
            deltas,
            new,
            noise,
            error,
            rerolls: live.rerolls,
            note: live.note.take(),
            tolerance: tol,
        };
        progress(&row);
        if let Some(dir) = &resume.cache {
            Resume::save(dir, i, live, h)?;
        }
        out.history.push(h);
        out.states
            .push(keep_states.then(|| live.v.emu.save_state()).transpose()?);
        failed = if row.error.is_some() { failed + 1 } else { 0 };
        out.rows.push(row);
        if failed >= MAX_FAILED {
            out.stopped = Some(format!("{MAX_FAILED} vanilla steps failed in a row at {i}"));
            break;
        }
    }
    for p in pending.drain(..) {
        p.record(&mut out.log, None);
    }
    Ok(out)
}

/// One branch replayed on both games.
#[derive(Clone, Debug)]
pub struct BranchRow {
    pub id: i64,
    pub origin: usize,
    pub kind: String,
    pub outcome: String,
    pub macros: usize,
    /// Our replay's error (it should replay as the branch run saw it).
    pub ours_error: Option<String>,
    pub vanilla_error: Option<String>,
    pub ours_frames: u64,
    pub vanilla_frames: u64,
    pub deltas: Vec<Delta>,
}

/// What a branch plays: its action, then the macros it chained (`explore::db::BranchRow`).
pub struct BranchPlan<'a> {
    pub id: i64,
    pub origin: usize,
    pub action: Action,
    pub kind: &'a str,
    pub outcome: &'a str,
    /// The exploration's macros (the target's first for a macro action).
    pub macros: &'a [Macro],
}

/// Replays `b` on both games from its origin (vanilla from the saved state), and diffs the end
/// states: start-relative deltas only (the deltas the origin already had are left out), and
/// as `Parity` findings.
pub fn branch<G: GameData + Clone + 'static>(
    game: &G,
    spine: &SpineRun,
    cfg: &explore::config::Config,
    oracle: &OracleRun,
    live: &mut Live<'_, G>,
    b: &BranchPlan<'_>,
) -> Result<(BranchRow, Vec<FindingDraft>)> {
    let origin = &spine.positions[b.origin];
    let state = (oracle.states.get(b.origin).cloned().flatten())
        .context("no vanilla state at the origin")?;
    let cfg = &explore::spine::play_config(cfg);
    // ---- ours ----
    let mut make = explore::spine::detectors::<G>(cfg);
    let mut ours = OurLive::start(game, cfg, Some(origin.end.clone()), &mut make)
        .map_err(|e| anyhow::anyhow!("our live from {}: {e:?}", b.origin))?;
    let mut ours_error = None;
    let mut x = Exec::at(game, cfg, origin.opts);
    let mut chained = b.macros;
    match b.action {
        Action::Choice { step, choice } => {
            x.opts.choice = Some(choice);
            if let Err(e) = x.play(&mut ours, &spine.steps[step as usize]) {
                ours_error = Some(e.message);
            }
        }
        Action::Macro(_) => {}
    }
    // Our map before and after every macro played: an `Enter` or `Cross` vanilla's settle already
    // made (a frame script ours runs as its own macro) is skipped there.
    let our_map = |o: &OurLive<'_, '_, G>| o.last_end().map(|c| c.world.field.map);
    let mut maps: Vec<(Option<engine::data::MapId>, Option<engine::data::MapId>)> = Vec::new();
    if ours_error.is_none() {
        for m in b.macros {
            let before = our_map(&ours);
            let played = ours.step_with(m, &mut explore::policy::Policy::new());
            maps.push((before, our_map(&ours)));
            if let Err(e) = played {
                ours_error = Some(format!("{m:?}: {e:?}"));
                break;
            }
            if live.v.trace
                && let Some(w) = (ours.last_end()).and_then(|s| World::restore(game, &s).ok())
            {
                let s = Semantic::from_world(&w, game);
                println!(
                    "  ours macro {:?}: on {} at {:?}",
                    m.kind,
                    map_name(s.map),
                    s.cell
                );
            }
        }
    } else {
        chained = &[];
    }
    let ours_frames = ours.frames();
    let ours_end = ours.last_end().context("our replay played nothing")?;
    let ours_sem = Semantic::from_world(
        &World::restore(game, &ours_end).map_err(|e| anyhow::anyhow!("restore: {e:?}"))?,
        game,
    );
    // ---- vanilla ----
    live.v.emu.load_state(&state)?;
    // The driver's own state belongs to the run that ended (the spine's last step left it at
    // the Hall of Fame, where `settle` returns at once): the origin's state starts afresh.
    live.v.hall_of_fame = false;
    live.v.bumped = None;
    live.v.buying = None;
    live.ahead = false;
    live.note = None;
    anyhow::ensure!(
        !live.v.at_hall_of_fame(),
        "vanilla is at the Hall of Fame at origin {} (GameClear's screens): nothing to play",
        b.origin
    );
    live.opts = origin.opts;
    let from = live.v.frames;
    let battles = live.v.battles.len();
    let mut vanilla_error = None;
    live.ours_lead_level = None;
    // Vanilla at the origin, for the origin's deltas when this run has no row there (a
    // `--from` run).
    let theirs_at_origin = live.v.state()?.semantic_in(game);
    if let Action::Choice { step, choice } = b.action {
        live.opts.choice = Some(choice);
        if let Err(e) = live.play(&spine.steps[step as usize]) {
            vanilla_error = Some(format!("{e:#}"));
        }
    }
    if vanilla_error.is_none() {
        for (i, m) in chained.iter().enumerate() {
            let on = live.v.current_map().ok();
            if matches!(
                m.kind,
                explore::MacroKind::Enter { .. } | explore::MacroKind::Cross { .. }
            ) && let Some(&(before, after)) = maps.get(i)
                && on != before
                && on == after
            {
                if live.v.trace {
                    println!("  macro {:?}: vanilla already there", m.kind);
                }
                continue;
            }
            if let Err(e) = live.play_macro(m) {
                vanilla_error = Some(format!("{m:?}: {e:#}"));
                let _ = live.v.settle(crate::live::MACRO_FRAMES);
                break;
            }
            if live.v.trace {
                let map = live.v.current_map().ok().map(map_name);
                let at = live.v.player_object().map(|o| o.cell);
                println!(
                    "  [{}] macro {:?}: on {map:?} at {at:?}",
                    live.v.frames, m.kind
                );
            }
        }
    }
    let mut h = oracle.history.get(b.origin).copied().unwrap_or_default();
    update(&mut h, &mut live.v, battles);
    let theirs = live.v.state()?.semantic_in(game);
    let tol = tolerance(h);
    // The deltas the origin already had are the spine's findings.
    let at_origin: Vec<Delta> = match oracle.rows.iter().find(|r| r.index == b.origin) {
        Some(r) => r.deltas.clone(),
        None => diff(&ours_at(game, spine, b.origin)?, &theirs_at_origin, tol),
    };
    let (deltas, _noise) = split_noise(
        &ours_sem,
        &theirs,
        diff(&ours_sem, &theirs, tol),
        &oracle.trainers,
    );
    let (deltas, _leveling) = split_leveling(game, &ours_sem, &theirs, deltas, tol);
    let deltas: Vec<Delta> = deltas
        .into_iter()
        .filter(|d| !at_origin.contains(d))
        .collect();
    let findings = deltas
        .iter()
        .map(|d| FindingDraft {
            kind: FindingKind::Parity,
            message: format!(
                "branch {} {}: {}",
                b.kind,
                b.outcome,
                parity_message(origin.segment, None, d)
            ),
            location: None,
            macro_: b.macros.first().copied(),
            tail: Vec::new(),
            map: ours_sem.map,
            pos: ours_sem.cell,
            script: None,
            log: Vec::new(),
        })
        .collect();
    Ok((
        BranchRow {
            id: b.id,
            origin: b.origin,
            kind: b.kind.to_string(),
            outcome: b.outcome.to_string(),
            macros: b.macros.len(),
            ours_error,
            vanilla_error,
            ours_frames,
            vanilla_frames: live.v.frames - from,
            deltas,
        },
        findings,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::data::{Dir, MapId};

    fn sem(flags: &[u16], money: u32) -> Semantic {
        Semantic {
            map: MapId(1),
            cell: (0, 0),
            facing: Dir::Down,
            flags: flags.iter().copied().collect(),
            vars: BTreeMap::new(),
            party: Vec::new(),
            bag: BTreeMap::new(),
            money,
            badges: 0,
            respawn: None,
            objects: BTreeMap::new(),
            homes: BTreeMap::new(),
            layout: None,
        }
    }

    #[test]
    fn a_wandering_trainers_battle_is_noise_with_its_money() {
        // 0x600: a trainer that looks around (RNG); 0x601: one facing a fixed way.
        let trainers = BTreeMap::from([(0x600, true), (0x601, false)]);
        let ours = sem(&[], 100);
        let theirs = sem(&[0x600], 300);
        let d = diff(&ours, &theirs, Tolerance::default());
        let (real, noise) = split_noise(&ours, &theirs, d, &trainers);
        assert!(real.is_empty(), "{real:?}");
        assert_eq!(noise.len(), 2);
        // A fixed trainer beaten in one game only stays a parity delta, and so does money.
        let theirs = sem(&[0x601], 300);
        let d = diff(&ours, &theirs, Tolerance::default());
        let (real, noise) = split_noise(&ours, &theirs, d, &trainers);
        assert_eq!(real.len(), 2);
        assert!(noise.is_empty());
    }

    #[test]
    fn a_delta_gone_at_the_next_boundary_is_no_finding() {
        let d = |f: &str| Delta {
            field: f.into(),
            ours: "0".into(),
            vanilla: "1".into(),
        };
        let mut log = ParityLog::default();
        let p = Pending {
            at: 3,
            segment: None,
            step: None,
            map: MapId(1),
            pos: (0, 0),
            deltas: vec![d("var A"), d("var B")],
        };
        p.record(&mut log, Some((&[d("var B")], &[])));
        assert_eq!(log.findings.len(), 1);
        assert!(log.findings[0].1.message.contains("var B"));
        assert_eq!(log.findings[0].0, 3);
    }
}
