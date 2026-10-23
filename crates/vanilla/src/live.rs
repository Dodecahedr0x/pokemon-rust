//! [`Live`]: the spine's [`Step`]s and the branches' [`Macro`]s expanded on BPEE, closed-loop
//! on RAM, as `explore::spine::Exec` and `explore::expand` expand them on our engine:
//!
//! - walks with our planner (`setmetatile` overrides and objects read from RAM), around the
//!   cells a `GotoAvoiding` names;
//! - `Choose` and the macros' `Opts` answer yes/no and the starter, `Battle` sets the battle
//!   policy (FIGHT, the first damaging move or `Best`, RUN from wild battles), read from
//!   `gBattleMons` and the cursors ([`Vanilla::settle`]);
//! - `Train` paces on the grass until the lead's level holds;
//! - `Expect` is our engine's goal check: not played on vanilla (the diff covers it);
//! - `Bypass` steps over an engine gap on our side: vanilla plays the feature natively
//!   ([`Live::bypass`]), and [`Live::note`] says what it did.

use crate::drive::Walk;
use crate::state::Vanilla;
use crate::state::layout::{B_OUTCOME_DREW, B_OUTCOME_LOST};
use anyhow::{Result, anyhow, bail, ensure};
use engine::Buttons;
use engine::data::{Dir, GameData, MapId};
use explore::model::{Macro, MacroKind, Opts};
use explore::oracle::Planner;
use explore::semantic::map_name;
use explore::spine::{
    BattlePlan, Bypass, NEW_GAME_MAP, RETRIES, Step, StepOpts, TRAIN_BATTLES, TRAIN_DRY,
    WAIT_FOR_FRAMES, WAIT_SCRIPT_FRAMES,
};

/// Frames one macro may take (`explore::spine::PLAY_MACRO_FRAMES`, and battles on top).
pub const MACRO_FRAMES: u64 = 30_000;
/// Frames a `Grass` macro paces before it gives up (no encounter).
pub const GRASS_FRAMES: u64 = 3_000;

/// BPEE playing spine steps and macros.
pub struct Live<'g, G: GameData> {
    pub v: Vanilla,
    game: &'g G,
    /// The sticky battle plan and the pending `Choose`.
    pub opts: StepOpts,
    /// Times the last [`Live::play`] replayed its step after a lost battle.
    pub rerolls: u32,
    /// What the last step played natively for a [`Step::Bypass`].
    pub note: Option<String>,
}

/// The answer a bypassed scripted talk takes natively (the spine has no `Choose` before a
/// bypass): `(issue, choice)`. Mr. Briney: yes to Dewford; Slateport (1) in his multichoice.
const NATIVE_CHOICE: &[(&str, u8)] = &[
    ("briney-sail-camera", 0),
    ("briney-sail-camera-slateport", 1),
];

/// Floor triggers pressed at most to open a way ([`Live::bypass`], Mauville's gym).
const TRIGGER_DEPTH: u32 = 3;

/// Replays of a step whose battle vanilla lost ([`Live::play`]).
pub const REROLLS: u32 = 6;

impl<'g, G: GameData> Live<'g, G> {
    pub fn new(mut v: Vanilla, game: &'g G) -> Self {
        v.tables = Some(game.battle());
        Live {
            v,
            game,
            opts: StepOpts::default(),
            rerolls: 0,
            note: None,
        }
    }

    fn resolve_map(&self, m: MapId) -> MapId {
        if m == NEW_GAME_MAP {
            self.game.new_game().map
        } else {
            m
        }
    }

    fn map(&mut self) -> Result<MapId> {
        self.v.current_map()
    }

    fn cell(&mut self) -> Result<(i16, i16)> {
        Ok(self
            .v
            .player_object()
            .ok_or_else(|| anyhow!("no player"))?
            .cell)
    }

    /// Plays one spine step ([`Live::play_once`]). A step whose battle vanilla lost (a
    /// whiteout our engine's spine never has) is played again from the state before it with
    /// another RNG (a few idle frames first), up to [`REROLLS`] times: vanilla's RNG is not
    /// ours, and a lost battle derails the rest of the comparison. [`Live::rerolls`] counts
    /// the replays of the last step.
    pub fn play(&mut self, step: &Step) -> Result<()> {
        if !step.plays() {
            return self.play_once(step);
        }
        let state = self.v.emu.save_state()?;
        let (frames, battles, outcomes, opts) = (
            self.v.frames,
            self.v.battles.len(),
            self.v.outcomes.len(),
            self.opts,
        );
        self.rerolls = 0;
        self.note = None;
        loop {
            let r = self.play_once(step);
            let lost = (self.v.outcomes[outcomes..].iter())
                .any(|&o| o == B_OUTCOME_LOST || o == B_OUTCOME_DREW);
            if !lost || self.rerolls >= REROLLS {
                return r;
            }
            self.rerolls += 1;
            self.v.emu.load_state(&state)?;
            self.v.frames = frames;
            self.v.battles.truncate(battles);
            self.v.outcomes.truncate(outcomes);
            self.opts = opts;
            self.v
                .hold(Buttons::NONE, 1 + 17 * self.rerolls as u16, MACRO_FRAMES)?;
        }
    }

    /// Plays one spine step (`explore::spine::Exec::play`'s rules).
    pub fn play_once(&mut self, step: &Step) -> Result<()> {
        if let Some(want) = step.map() {
            let want = self.resolve_map(want);
            let on = self.map()?;
            ensure!(
                on == want,
                "on {} at {:?}, not {}",
                map_name(on),
                self.cell()?,
                map_name(want)
            );
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
            Step::Checkpoint(_) | Step::Expect(_) => return Ok(()),
            _ => {}
        }
        // Vanilla's random wild battles on the way are not ours (another RNG): RUN from them
        // outside the steps that fight wild battles on purpose, so that their damage does not
        // derail the comparison (an oracle alignment, `oracle` module docs).
        let pacing = matches!(step, Step::Grass { .. } | Step::Train { .. });
        let opts = Opts {
            run_wild: self.opts.run_wild || !pacing,
            choice: self.opts.choice.take().unwrap_or(0),
            best_move: self.opts.best_move,
            learn: self.opts.learn,
        };
        self.answer(opts);
        let start = self.map()?;
        match *step {
            Step::Goto { x, y, .. } => self.goto(start, (x, y), &[])?,
            Step::GotoAvoiding { x, y, avoid, .. } => self.goto(start, (x, y), avoid)?,
            Step::Enter { x, y, dir, .. } => {
                for _ in 0..RETRIES {
                    if self.map()? != start {
                        break;
                    }
                    self.enter((x, y), dir)?;
                }
                let want = (self.game.map(start).warps.iter())
                    .find(|w| (w.x, w.y) == (x, y))
                    .map(|w| w.dest_map)
                    .filter(|&m| self.game.has_map(m));
                let on = self.map()?;
                if want.is_some_and(|m| m != on) || (want.is_none() && on == start) {
                    bail!(
                        "entered ({x}, {y}) but on {}, not {:?}",
                        map_name(on),
                        want.map(map_name)
                    );
                }
            }
            Step::Cross { x, y, dir, .. } => {
                for _ in 0..RETRIES {
                    if self.map()? != start {
                        break;
                    }
                    self.cross((x, y), dir)?;
                }
                ensure!(
                    self.map()? != start,
                    "crossed at ({x}, {y}) but the map is the same"
                );
            }
            Step::Talk { local_id, .. } => self.talk(local_id)?,
            Step::Use { x, y, dir, .. } => {
                self.answer(Opts { choice: 0, ..opts });
                self.goto(start, (x, y), &[])?;
                self.answer(opts);
                self.v.interact(dir, MACRO_FRAMES)?;
            }
            Step::Face(dir) => self.v.face(dir, MACRO_FRAMES)?,
            Step::Interact => {
                self.v.settle(MACRO_FRAMES)?;
                self.v.tap(Buttons::A)?;
                self.v.settle(MACRO_FRAMES)?;
            }
            Step::Walk(dir) => self.v.walk_dir(dir, MACRO_FRAMES)?,
            Step::Exit { dir, to } => {
                for _ in 0..RETRIES {
                    if self.map()? != start {
                        break;
                    }
                    self.v.push(dir, MACRO_FRAMES)?;
                }
                let to = self.resolve_map(to);
                let on = self.map()?;
                ensure!(
                    on == to,
                    "exited {dir:?} to {}, not {}",
                    map_name(on),
                    map_name(to)
                );
            }
            Step::Press(keys, frames) => self.v.hold(Buttons(keys), frames, MACRO_FRAMES)?,
            Step::Wait(frames) => self.v.hold(Buttons::NONE, frames, MACRO_FRAMES)?,
            Step::WaitScript => self
                .v
                .hold(Buttons::NONE, WAIT_SCRIPT_FRAMES, MACRO_FRAMES)?,
            Step::WaitFor { local_id, x, y } => {
                let mut n = 0;
                while self.object(local_id)?.is_none_or(|o| o.cell != (x, y)) {
                    n += 1;
                    ensure!(
                        n <= WAIT_FOR_FRAMES,
                        "object {local_id} never on ({x}, {y}) in {WAIT_FOR_FRAMES} frames"
                    );
                    self.v.hold(Buttons::NONE, 1, MACRO_FRAMES)?;
                }
            }
            Step::Bypass(b) => self.bypass(b, opts)?,
            Step::Grass { x, y, .. } => {
                self.grass((x, y))?;
            }
            Step::Train { x, y, level, .. } => {
                let (mut n, mut dry) = (0, 0);
                while self.v.party().first().is_some_and(|m| m.level < level) {
                    n += 1;
                    ensure!(
                        n <= TRAIN_BATTLES,
                        "not level {level} after {TRAIN_BATTLES} tries"
                    );
                    if !self.grass((x, y))? {
                        dry += 1;
                        ensure!(dry <= TRAIN_DRY, "no encounter {dry} times");
                        self.v.hold(Buttons::NONE, 31, MACRO_FRAMES)?;
                    }
                    let on = self.map()?;
                    ensure!(
                        on == start,
                        "left the map while training (on {} at {:?}, lead {:?} after {n} tries)",
                        map_name(on),
                        self.cell()?,
                        self.v.party().first().map(|m| (m.level, m.hp))
                    );
                }
            }
            Step::Battle(_) | Step::Choose(_) | Step::Expect(_) | Step::Checkpoint(_) => {
                unreachable!("handled above")
            }
        }
        Ok(())
    }

    /// Plays one macro (`explore::expand`'s rules) with its options.
    pub fn play_macro(&mut self, m: &Macro) -> Result<()> {
        self.answer(m.opts);
        let start = self.map()?;
        match m.kind {
            MacroKind::Settle => self.v.settle(MACRO_FRAMES)?,
            MacroKind::WalkTo { x, y } => {
                self.v.walk_to(self.game, (x, y), MACRO_FRAMES)?;
                self.v.settle(MACRO_FRAMES)?;
            }
            MacroKind::Enter { x, y, dir } => self.enter((x, y), dir)?,
            MacroKind::Cross { x, y, dir } => self.cross((x, y), dir)?,
            MacroKind::Talk { local_id } => self.talk(local_id)?,
            MacroKind::Inspect { x, y, dir } => {
                let (dx, dy) = dir.delta();
                if self.v.walk_to(self.game, (x - dx, y - dy), MACRO_FRAMES)? == Walk::Reached {
                    self.v.interact(dir, MACRO_FRAMES)?;
                }
            }
            MacroKind::Grass { x, y } => {
                ensure!(self.grass((x, y))?, "grass: no encounter");
            }
            MacroKind::Wait { frames } => self.v.hold(Buttons::NONE, frames, MACRO_FRAMES)?,
            MacroKind::Face { dir } => self.v.face(dir, MACRO_FRAMES)?,
            MacroKind::Interact => {
                self.v.settle(MACRO_FRAMES)?;
                self.v.tap(Buttons::A)?;
                self.v.settle(MACRO_FRAMES)?;
            }
            MacroKind::Press { keys, frames } => {
                self.v.hold(Buttons(keys), frames, MACRO_FRAMES)?
            }
            MacroKind::Step { dir } => self.v.walk_dir(dir, MACRO_FRAMES)?,
            MacroKind::Push { dir } => {
                if self.map()? == start {
                    self.v.push(dir, MACRO_FRAMES)?;
                }
            }
        }
        Ok(())
    }

    /// Object `local_id` of the current map, when spawned and visible.
    fn object(&mut self, local_id: u8) -> Result<Option<crate::state::Object>> {
        let map = self.map()?;
        Ok((0..crate::state::layout::OBJECT_EVENTS)
            .filter_map(|i| self.v.object_event(i))
            .find(|o| !o.is_player && o.visible && o.map == map && o.local_id == local_id))
    }

    /// Plays a [`Step::Bypass`] natively, the feature our engine lacks (it writes the state
    /// the feature leaves instead):
    ///
    /// - no warp: the bypass only writes flags and vars, which vanilla's feature wrote when it
    ///   ran (nothing to play: the boundary's diff says whether they agree);
    /// - a warp on the current map (a gym puzzle): walk to the cell, pressing the map's floor
    ///   triggers (up to [`TRIGGER_DEPTH`], searched on save states) until a way opens;
    /// - a warp to another map (a scripted trip): A on what the player faces, answering per
    ///   [`NATIVE_CHOICE`], and the script runs to the end.
    ///
    /// [`Live::note`] says what was played and where vanilla stands.
    pub fn bypass(&mut self, b: &Bypass, opts: Opts) -> Result<()> {
        let features = b.features.join(", ");
        let Some((m, x, y)) = b.warp else {
            self.v.settle(MACRO_FRAMES)?;
            self.note = Some(format!(
                "{}: nothing to play ({features} ran natively)",
                b.issue
            ));
            return Ok(());
        };
        let m = self.resolve_map(m);
        let start = self.map()?;
        let from = self.v.frames;
        if start == m {
            let mut pressed = Vec::new();
            ensure!(
                self.reach_by_triggers(m, (x, y), TRIGGER_DEPTH, &mut pressed)?,
                "{}: no way to ({x}, {y}) pressing up to {TRIGGER_DEPTH} triggers",
                b.issue
            );
            self.note = Some(format!(
                "{}: {features} played natively: triggers {pressed:?} pressed, ({x}, {y}) reached \
                 in {} frames",
                b.issue,
                self.v.frames - from
            ));
            return Ok(());
        }
        let choice = (NATIVE_CHOICE.iter())
            .find(|(i, _)| *i == b.issue)
            .map_or(0, |&(_, c)| c);
        self.answer(Opts { choice, ..opts });
        self.v.settle(MACRO_FRAMES)?;
        self.v.tap(Buttons::A)?;
        self.v.settle(MACRO_FRAMES)?;
        let on = self.map()?;
        let cell = self.cell()?;
        self.note = Some(format!(
            "{}: {features} played natively (choice {choice}): on {} at {cell:?} after {} frames \
             (the bypass warps to {:?})",
            b.issue,
            map_name(on),
            self.v.frames - from,
            (map_name(m), x, y)
        ));
        ensure!(
            on == m,
            "{}: the native script left the player on {}, not {}",
            b.issue,
            map_name(on),
            map_name(m)
        );
        Ok(())
    }

    /// Whether `to` is reachable on `map` now (objects in the way).
    fn reachable(&mut self, map: MapId, to: (i16, i16)) -> Result<bool> {
        let me = self.cell()?;
        let overrides = self.v.overrides(self.game, map);
        let occupied = self.v.occupied(map);
        let p = Planner::new(self.game, map, &overrides, occupied);
        Ok(me == to || p.first_dir(me, to).is_some())
    }

    /// Walks to `to`, stepping on the map's floor triggers first (depth-first over save
    /// states, `depth` presses at most) while it is out of reach. `pressed` gets the
    /// triggers' cells.
    fn reach_by_triggers(
        &mut self,
        map: MapId,
        to: (i16, i16),
        depth: u32,
        pressed: &mut Vec<(i16, i16)>,
    ) -> Result<bool> {
        if self.reachable(map, to)? {
            self.goto(map, to, &[])?;
            return Ok(true);
        }
        if depth == 0 {
            return Ok(false);
        }
        let mut triggers: Vec<(i16, i16)> = (self.game.map(map).coords.iter())
            .map(|c| (c.x, c.y))
            .collect();
        triggers.dedup();
        for t in triggers {
            if self.cell()? == t || !self.reachable(map, t)? {
                continue;
            }
            let state = self.v.emu.save_state()?;
            let (frames, battles, outcomes) =
                (self.v.frames, self.v.battles.len(), self.v.outcomes.len());
            let ok = self
                .v
                .walk_to(self.game, t, MACRO_FRAMES)
                .is_ok_and(|w| w == Walk::Reached)
                && self.v.settle(MACRO_FRAMES).is_ok()
                && self.map()? == map;
            if ok {
                pressed.push(t);
                if self.reach_by_triggers(map, to, depth - 1, pressed)? {
                    return Ok(true);
                }
                pressed.pop();
            }
            self.v.emu.load_state(&state)?;
            self.v.frames = frames;
            self.v.battles.truncate(battles);
            self.v.outcomes.truncate(outcomes);
        }
        Ok(false)
    }

    /// Sets the answers (and the starter ball) for what the next inputs trigger, on the
    /// current map ([`Vanilla::answers_map`]).
    fn answer(&mut self, opts: Opts) {
        self.v.answers = opts;
        self.v.starter = opts.choice % 3;
        self.v.answers_map = self.v.current_map().ok();
    }

    /// Walks to `to` on `map` (avoiding `avoid`): a battle or a script can interrupt the walk,
    /// which walks on.
    fn goto(&mut self, map: MapId, to: (i16, i16), avoid: &[(i16, i16)]) -> Result<()> {
        for _ in 0..RETRIES {
            if self.cell()? == to {
                break;
            }
            ensure!(
                self.map()? == map,
                "left the map walking to {to:?} (on {})",
                map_name(self.map()?)
            );
            self.v
                .walk_to_avoiding(self.game, to, avoid, MACRO_FRAMES)?;
        }
        self.v.settle(MACRO_FRAMES)?;
        ensure!(
            self.map()? == map && self.cell()? == to,
            "never reached {to:?} (at {:?} on {})",
            self.cell()?,
            map_name(self.map()?)
        );
        Ok(())
    }

    /// Walks to the warp's approach, then moves `dir` into it.
    fn enter(&mut self, at: (i16, i16), dir: Dir) -> Result<()> {
        let map = self.map()?;
        let overrides = self.v.overrides(self.game, map);
        let approach = Planner::new(self.game, map, &overrides, Vec::new()).enter_approach(at, dir);
        if self.v.walk_to(self.game, approach, MACRO_FRAMES)? != Walk::Reached {
            return self.v.settle(MACRO_FRAMES);
        }
        self.v.push(dir, MACRO_FRAMES)
    }

    /// Walks to the edge cell, then holds `dir` until the map changes.
    fn cross(&mut self, at: (i16, i16), dir: Dir) -> Result<()> {
        if self.v.walk_to(self.game, at, MACRO_FRAMES)? != Walk::Reached {
            return self.v.settle(MACRO_FRAMES);
        }
        self.v.push(dir, MACRO_FRAMES)
    }

    /// Walks next to object `local_id` of the current map, faces it and presses A.
    fn talk(&mut self, local_id: u8) -> Result<()> {
        for _ in 0..RETRIES {
            self.v.settle(MACRO_FRAMES)?;
            let map = self.map()?;
            let o = (0..crate::state::layout::OBJECT_EVENTS)
                .filter_map(|i| self.v.object_event(i))
                .find(|o| !o.is_player && o.visible && o.map == map && o.local_id == local_id)
                .ok_or_else(|| anyhow!("talk: no object {local_id} on {}", map_name(map)))?;
            let overrides = self.v.overrides(self.game, map);
            let occupied = self.v.occupied(map);
            let me = self.cell()?;
            let (approaches, reachable) = {
                let p = Planner::new(self.game, map, &overrides, occupied);
                let a = p.talk_approaches(o.cell);
                let r: Vec<_> = (a.iter())
                    .filter(|&&(c, _)| c == me || p.first_dir(me, c).is_some())
                    .copied()
                    .collect();
                (a, r)
            };
            if let Some(&(_, d)) = approaches.iter().find(|(c, _)| *c == me) {
                return self.v.interact(d, MACRO_FRAMES);
            }
            let &(c, _) = reachable
                .first()
                .ok_or_else(|| anyhow!("talk: no way next to object {local_id} at {:?}", o.cell))?;
            self.v.walk_to(self.game, c, MACRO_FRAMES)?;
        }
        bail!("talk: never next to object {local_id}")
    }

    /// Paces between encounter cell `at` and a walkable neighbour until a battle starts, then
    /// settles it. `false` when no encounter came within [`GRASS_FRAMES`].
    fn grass(&mut self, at: (i16, i16)) -> Result<bool> {
        let battles = self.v.battles.len();
        if self.v.walk_to(self.game, at, MACRO_FRAMES)? != Walk::Reached {
            self.v.settle(MACRO_FRAMES)?;
            if self.v.battles.len() > battles {
                return Ok(true);
            }
            bail!("grass: never reached {at:?}");
        }
        let map = self.map()?;
        let overrides = self.v.overrides(self.game, map);
        let occupied = self.v.occupied(map);
        let other = Planner::new(self.game, map, &overrides, occupied)
            .grass_partner(at)
            .ok_or_else(|| anyhow!("grass: no walkable neighbour of {at:?}"))?;
        let end = self.v.frames + GRASS_FRAMES;
        let mut to = other;
        while self.v.battles.len() == battles {
            if self.v.frames >= end {
                return Ok(false);
            }
            self.v.walk_to(self.game, to, MACRO_FRAMES)?;
            to = if to == at { other } else { at };
        }
        self.v.settle(MACRO_FRAMES)?;
        Ok(true)
    }
}
