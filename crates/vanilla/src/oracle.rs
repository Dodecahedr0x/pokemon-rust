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
use explore::semantic::{Delta, Semantic, Tolerance, diff};
use explore::sim::Live as OurLive;
use explore::spine::{Exec, SpineRun, Step};

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
    /// Every delta under the tolerance at this boundary.
    pub deltas: Vec<Delta>,
    /// The ones not already there at the boundary before (the findings).
    pub new: Vec<Delta>,
    /// Vanilla could not play the step (a driver problem, or a divergence).
    pub error: Option<String>,
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
}

/// Our engine's semantic view at position `i` of `spine`.
pub fn ours_at<G: GameData>(game: &G, spine: &SpineRun, i: usize) -> Result<Semantic> {
    let w = World::restore(game, &spine.positions[i].end)
        .map_err(|e| anyhow::anyhow!("restore position {i}: {e:?}"))?;
    Ok(Semantic::from_world(&w, game))
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
    mut progress: impl FnMut(&Row),
) -> Result<OracleRun> {
    let mut out = OracleRun {
        rows: Vec::new(),
        log: ParityLog::default(),
        start_frames: live.v.frames,
        states: Vec::new(),
        history: Vec::new(),
        stopped: None,
    };
    let mut h = History::default();
    let mut failed = 0;
    let mut battles = live.v.battles.len();
    // Position 0, then one per step ours played.
    for i in 0..spine.positions.len() {
        let step = i.checked_sub(1).map(|s| spine.steps[s]);
        let before = live.v.frames;
        live.rerolls = 0;
        let error = match step {
            Some(s) => live.play(&s).err().map(|e| format!("{e:#}")),
            None => None,
        };
        // A failed step may leave a script running: settle what can be settled.
        if error.is_some() {
            let _ = live.v.settle(crate::live::MACRO_FRAMES);
        }
        update(&mut h, &mut live.v, battles);
        battles = live.v.battles.len();
        let p = &spine.positions[i];
        let ours = ours_at(game, spine, i)?;
        let theirs = live.v.state()?.semantic();
        let tol = tolerance(h);
        let deltas = diff(&ours, &theirs, tol);
        let new = out
            .log
            .record(i, p.segment, step.as_ref(), p.map(), p.pos(), &deltas);
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
            error,
            rerolls: live.rerolls,
            tolerance: tol,
        };
        progress(&row);
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
    if ours_error.is_none() {
        for m in b.macros {
            if let Err(e) = ours.step_with(m, &mut explore::policy::Policy::new()) {
                ours_error = Some(format!("{m:?}: {e:?}"));
                break;
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
    live.opts = origin.opts;
    let from = live.v.frames;
    let battles = live.v.battles.len();
    let mut vanilla_error = None;
    if let Action::Choice { step, choice } = b.action {
        live.opts.choice = Some(choice);
        if let Err(e) = live.play(&spine.steps[step as usize]) {
            vanilla_error = Some(format!("{e:#}"));
        }
    }
    if vanilla_error.is_none() {
        for m in chained {
            if let Err(e) = live.play_macro(m) {
                vanilla_error = Some(format!("{m:?}: {e:#}"));
                let _ = live.v.settle(crate::live::MACRO_FRAMES);
                break;
            }
        }
    }
    let mut h = oracle.history.get(b.origin).copied().unwrap_or_default();
    update(&mut h, &mut live.v, battles);
    let theirs = live.v.state()?.semantic();
    let tol = tolerance(h);
    // The deltas the origin already had are the spine's findings.
    let at_origin: Vec<Delta> = oracle
        .rows
        .get(b.origin)
        .map(|r| r.deltas.clone())
        .unwrap_or_default();
    let deltas: Vec<Delta> = diff(&ours_sem, &theirs, tol)
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
