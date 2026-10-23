//! Driving primitives on BPEE, all closed-loop on RAM (the ground floor of [`crate::live`]):
//! [`Vanilla::settle`] answers what waits for input until the player has control,
//! [`Vanilla::walk_to`] walks with our planner (`explore::oracle::Planner` over our map data:
//! Gen3 map ids are ours) around the objects read from RAM, and [`Vanilla::interact`] faces a
//! cell and presses A.
//!
//! The planner sees our map data with the `setmetatile` overrides read from `gBackupMapLayout`.
//! The answers follow [`Vanilla::answers`] (`explore::Opts`), as our engine's policy
//! (`explore::policy`): yes/no by `choice` (B for no), the first multichoice item, the starter
//! [`Vanilla::starter`], FIGHT (RUN from non-first wild battles with `run_wild`) and the first
//! damaging move (or the best one with `best_move`) read from `gBattleMons`, B on "delete a move
//! to learn?" and YES on "stop learning?"; a nickname is declined by confirming the offered
//! name.

use crate::state::{Mode, Vanilla, layout::*};
use anyhow::{Result, anyhow};
use engine::Buttons;
use engine::data::{Dir, GameData, MapId, MoveId, SpeciesId};
use explore::oracle::{Planner, overrides_from_grid};
use explore::policy::{
    YES_ONCE, best_damaging_slot_of, first_damaging_slot_of, forget_slot_of, toward,
};

/// The HMs (`IsHMMove`): Cut, Fly, Surf, Strength, Flash, Rock Smash, Waterfall, Dive.
const HM_MOVES: [u16; 8] = [15, 19, 57, 70, 148, 249, 127, 291];

/// The key that moves a one-column list cursor from `c` to `want`, or A once it is there.
fn vertical(c: u8, want: u8) -> Buttons {
    match c.cmp(&want) {
        std::cmp::Ordering::Less => Buttons::DOWN,
        std::cmp::Ordering::Greater => Buttons::UP,
        std::cmp::Ordering::Equal => Buttons::A,
    }
}

/// How a [`Vanilla::walk_to`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    /// The player stands on the target.
    Reached,
    /// A warp (or a script) took the player to another map.
    MapChanged(MapId),
    /// A script took over on the way (a trigger); settled, the player is elsewhere.
    Interrupted((i16, i16)),
}

pub fn button(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

/// Frames a held direction may bump without moving before the walk re-plans.
const BUMP_FRAMES: u32 = 48;
/// Re-plans after bumps a walk allows.
const MAX_STALLS: u32 = 40;
/// Input-less [`Mode::FieldIdle`] frames in a row that end a [`Vanilla::settle`]: the field
/// runs on-frame map scripts and triggers only once it processes the player's input, and after
/// a connection crossing the player may still finish its step (a few frames of `Transition`)
/// before a trigger's script starts.
pub const SETTLE_IDLE: u32 = 4;

impl Vanilla {
    /// One frame `keys`, one frame released (a new press each call).
    pub fn tap(&mut self, keys: Buttons) -> Result<()> {
        self.step(keys)?;
        self.step(Buttons::NONE)
    }

    /// Steps until [`Mode::FieldIdle`] holds over [`SETTLE_IDLE`] frames without input (within
    /// `max_frames`),
    /// answering every wait per [`Vanilla::answers`] (module docs): messages with A, yes/no,
    /// multichoices (the choice's item, the next one when the same script asks again), other screens (the clock, the starter, the naming
    /// screen), the battle's menus and prompts. Scripts and transitions run without input.
    /// Every battle seen starting is logged in [`Vanilla::battles`].
    pub fn settle(&mut self, max_frames: u64) -> Result<()> {
        let end = self.frames + max_frames;
        let mut idle = 0;
        let mut in_battle = false;
        let mut last = None;
        // The script position of the last multichoice, and the times it was asked again.
        let (mut asked, mut reasked, mut in_multichoice) = (None, 0u8, false);
        let mut in_yes_no = false;
        loop {
            let mode = self.mode();
            if self.trace && last != Some(mode) {
                let inputs = self.mode_inputs();
                eprintln!("  [{:>7}] settle {mode:?}: {inputs}", self.frames);
                last = Some(mode);
            }
            if mode == Mode::Multichoice && !in_multichoice {
                let at = self.emu.read_u32(self.sym.script_context + CTX_SCRIPT_PTR);
                if asked == Some(at) {
                    reasked += 1;
                } else {
                    (asked, reasked) = (Some(at), 0);
                }
            }
            in_multichoice = mode == Mode::Multichoice;
            if mode == Mode::YesNo && !in_yes_no {
                self.yes_nos += 1;
            }
            in_yes_no = mode == Mode::YesNo;
            let battle = matches!(
                mode,
                Mode::BattleAction | Mode::BattleMove | Mode::BattleTarget | Mode::BattleWaiting
            );
            if battle && !in_battle {
                let flags = self.emu.read_u32(self.sym.battle_type_flags);
                self.battles.push(flags);
            }
            if in_battle && !battle && mode != Mode::Transition {
                let outcome = self.emu.read_u8(self.sym.battle_outcome);
                self.outcomes.push(outcome);
            }
            in_battle = battle || (in_battle && mode == Mode::Transition);
            let keys = match mode {
                // Confirmed on a second frame: the field runs on-frame map scripts and triggers
                // only once it processes the player's (here: no) input.
                Mode::FieldIdle if idle >= SETTLE_IDLE => return Ok(()),
                Mode::FieldIdle => Buttons::NONE,
                Mode::Script | Mode::Transition => Buttons::NONE,
                Mode::BattleAction
                | Mode::BattleMove
                | Mode::BattleTarget
                | Mode::BattleWaiting => self.battle_key(mode),
                Mode::YesNo
                    if self.answers.choice == 0
                        || !self.choice_holds()
                        || (self.answers.choice == YES_ONCE && self.yes_nos == 1) =>
                {
                    self.yes()
                }
                // B answers NO wherever the cursor is (`menu.c`: MENU_B_PRESSED).
                Mode::YesNo => Buttons::B,
                Mode::Screen { .. } => self.screen_key(),
                // The choice's item where it applies (`Choose(n)`), else the first; the next
                // item each time the same script asks again (a tutorial's "please select X").
                Mode::Multichoice => {
                    let at = self.emu.read_u8(self.sym.menu + 2);
                    let max = self.emu.read_u8(self.sym.menu + 4);
                    let base = if self.choice_holds() {
                        self.answers.choice
                    } else {
                        0
                    };
                    vertical(at, (base + reasked) % (max.saturating_add(1).max(1)))
                }
                Mode::Message => Buttons::A,
            };
            if self.frames >= end {
                let inputs = self.mode_inputs();
                return Err(anyhow!(
                    "settle: still {mode:?} after {max_frames} frames (frame {}): {inputs}",
                    self.frames
                ));
            }
            idle = if mode == Mode::FieldIdle { idle + 1 } else { 0 };
            if keys == Buttons::NONE {
                self.step(keys)?;
            } else {
                self.tap(keys)?;
            }
        }
    }

    /// Whether [`Vanilla::answers`]' choice applies on the current map.
    fn choice_holds(&mut self) -> bool {
        match self.answers_map {
            None => true,
            Some(m) => self.current_map().is_ok_and(|on| on == m),
        }
    }

    /// The battle's key (our `policy::Policy::battle`): FIGHT or RUN on the action menu, the
    /// policy's move, B on "delete a move?", YES on "stop learning?", A through the rest.
    fn battle_key(&mut self, mode: Mode) -> Buttons {
        let s = self.sym.clone();
        let flags = self.emu.read_u32(s.battle_type_flags);
        let wild = flags & BATTLE_TYPE_TRAINER == 0;
        let run = self.answers.run_wild && wild && flags & BATTLE_TYPE_FIRST_BATTLE == 0;
        match mode {
            Mode::BattleAction => toward(
                self.emu.read_u8(s.action_selection_cursor),
                if run { 3 } else { 0 },
            ),
            Mode::BattleMove if run => Buttons::B,
            Mode::BattleMove => {
                let slot = self.move_slot();
                toward(self.emu.read_u8(s.move_selection_cursor), slot)
            }
            _ => {
                let instr = self.emu.read_u32(s.battlescript_curr_instr);
                let learning = self.emu.read_u16(s.move_to_learn);
                match self.emu.read_u8(instr) {
                    // Learn over the weakest known move when the new one is worth more
                    // (`Opts::learn`, our `policy::forget_slot`); else keep the known moves.
                    B_SCR_OP_YESNOBOXLEARNMOVE if self.forget_slot(learning).is_some() => {
                        if self.emu.read_u8(s.battle_communication + 1) == 0 {
                            Buttons::A
                        } else {
                            Buttons::UP
                        }
                    }
                    B_SCR_OP_YESNOBOXLEARNMOVE => Buttons::B,
                    B_SCR_OP_YESNOBOXSTOPLEARNINGMOVE => {
                        if self.emu.read_u8(s.battle_communication + 1) == 0 {
                            Buttons::A
                        } else {
                            Buttons::UP
                        }
                    }
                    _ => Buttons::A,
                }
            }
        }
    }

    /// The slot to forget for `gMoveToLearn` (`Opts::learn` only), from `gBattleMons[0]`.
    fn forget_slot(&mut self, new: u16) -> Option<u8> {
        if !self.answers.learn {
            return None;
        }
        let t = self.tables?;
        // The lead (`gPlayerParty[0]`: the learner in a single battle, and after an
        // evolution); HMs cannot be forgotten in Gen3 (`IsHMMove`): never picked.
        let lead = self.party().into_iter().next()?;
        let moves: [Option<MoveId>; 4] = std::array::from_fn(|i| {
            let m = lead.moves[i];
            (m != 0 && !HM_MOVES.contains(&m)).then_some(MoveId(m))
        });
        forget_slot_of(SpeciesId(lead.species), &moves, MoveId(new), t)
    }

    /// The summary screen's forget-a-move cursor: toward [`Vanilla::forget_slot`] for the
    /// screen's `newMove`, then A; B (keep the known moves) without one.
    fn replace_move_key(&mut self) -> Buttons {
        let pss = self.emu.read_u32(self.sym.mon_summary_screen);
        if pss == 0 {
            return Buttons::NONE;
        }
        let new = self.emu.read_u16(pss + PSS_NEW_MOVE);
        let Some(slot) = self.forget_slot(new) else {
            return Buttons::B;
        };
        vertical(self.emu.read_u8(pss + PSS_FIRST_MOVE_INDEX), slot)
    }

    /// The policy's move slot, from `gBattleMons[0]` (ours) and `[1]` (the foe).
    fn move_slot(&mut self) -> u8 {
        let Some(t) = self.tables else {
            return 0;
        };
        let me = self.sym.battle_mons;
        let foe = me + BATTLE_MON_SIZE;
        let species = SpeciesId(self.emu.read_u16(me + BM_SPECIES));
        let moves: [Option<MoveId>; 4] = std::array::from_fn(|i| {
            let m = self.emu.read_u16(me + BM_MOVES + 2 * i as u32);
            (m != 0).then_some(MoveId(m))
        });
        let pp: [u8; 4] = std::array::from_fn(|i| self.emu.read_u8(me + BM_PP + i as u32));
        if self.answers.best_move {
            let theirs = SpeciesId(self.emu.read_u16(foe + BM_SPECIES));
            best_damaging_slot_of(species, &moves, &pp, theirs, t)
        } else {
            first_damaging_slot_of(&moves, &pp, t)
        }
    }

    /// The key for a screen outside the field and the battle: the wall clock's yes/no (YES),
    /// the starter screen ([`Vanilla::starter`], then YES), the naming screen (the name it
    /// offers: START to OK, then A), A elsewhere.
    fn screen_key(&mut self) -> Buttons {
        let s = self.sym.clone();
        let tasks = self.tasks();
        if tasks.contains(&s.task_replace_move) {
            return self.replace_move_key();
        }
        if tasks.contains(&s.task_set_clock_confirm) || tasks.contains(&s.task_starter_confirm) {
            return self.yes();
        }
        if let Some(d) = self.task_data(s.task_starter_choose) {
            return match d[0].cmp(&i16::from(self.starter)) {
                std::cmp::Ordering::Less => Buttons::RIGHT,
                std::cmp::Ordering::Greater => Buttons::LEFT,
                std::cmp::Ordering::Equal => Buttons::A,
            };
        }
        if tasks.contains(&s.task_naming_screen) {
            return match self.naming() {
                None => Buttons::NONE,
                Some((_, (8, 2), _)) => Buttons::A,
                Some(_) => Buttons::START,
            };
        }
        if self.callback2() == s.cb2_pokenav {
            return self.pokenav_key();
        }
        Buttons::A
    }

    /// Rustboro's PokéNav tutorial (`OpenPokenavForTutorial`): before the call
    /// (`POKENAV_MODE_FORCE_CALL_READY`) A on MATCH CALL (only it opens: A taps, then a DOWN
    /// once the cursor's animation is long over, until it does), A on the first entry, A on
    /// CALL; after it, B out of every menu.
    fn pokenav_key(&mut self) -> Buttons {
        let res = self.emu.read_u32(self.sym.pokenav_resources);
        if res == 0 {
            return Buttons::NONE;
        }
        match self.emu.read_u16(res + PN_MODE) {
            POKENAV_MODE_FORCE_CALL_READY if (self.frames / 2) % 48 == 47 => Buttons::DOWN,
            POKENAV_MODE_FORCE_CALL_READY => Buttons::A,
            _ => Buttons::B,
        }
    }

    /// The key toward YES on a `menu.c` yes/no.
    fn yes(&mut self) -> Buttons {
        match self.emu.read_u8(self.sym.menu + 2) {
            0 => Buttons::A,
            _ => Buttons::UP,
        }
    }

    /// The cells the visible objects of the current map (but the player) stand on.
    pub fn occupied(&mut self, map: MapId) -> Vec<(i16, i16)> {
        (0..OBJECT_EVENTS)
            .filter_map(|i| self.object_event(i))
            .filter(|o| !o.is_player && o.visible && o.map == map)
            .map(|o| o.cell)
            .collect()
    }

    /// The `setmetatile` overrides of the current map: the cells of `gBackupMapLayout` whose
    /// block differs from our map data (none when the grid is not `map`'s).
    pub fn overrides<G: GameData>(&mut self, game: &G, map: MapId) -> Vec<((i16, i16), u16)> {
        let at = self.sym.backup_map_layout;
        let (w, h) = (
            self.emu.read_u32(at) as usize,
            self.emu.read_u32(at + 4) as usize,
        );
        let ptr = self.emu.read_u32(at + 8);
        if !EWRAM.contains(&ptr) || w * h > 0x8000 || !game.has_map(map) {
            return Vec::new();
        }
        let bytes = self.emu.read_bytes(ptr, 2 * w * h);
        let grid: Vec<u16> = (bytes.chunks(2))
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        overrides_from_grid(game.map(map), w, h, &grid).unwrap_or_default()
    }

    /// Walks to `to` on the current map (`game`'s collision data), settling whenever a script
    /// takes over. Ends on the target, on a map change, or when a trigger moved the player.
    pub fn walk_to<G: GameData>(
        &mut self,
        game: &G,
        to: (i16, i16),
        max_frames: u64,
    ) -> Result<Walk> {
        self.walk_to_avoiding(game, to, &[], max_frames)
    }

    /// [`Vanilla::walk_to`] never entering an `avoid` cell (a trainer's sight line), planned
    /// again from every cell.
    pub fn walk_to_avoiding<G: GameData>(
        &mut self,
        game: &G,
        to: (i16, i16),
        avoid: &[(i16, i16)],
        max_frames: u64,
    ) -> Result<Walk> {
        let end = self.frames + max_frames;
        self.settle(max_frames)?;
        let start = self.current_map()?;
        let mut stalls = 0;
        loop {
            let me = self.player_object().ok_or_else(|| anyhow!("no player"))?;
            let map = self.current_map()?;
            if map != start {
                return Ok(Walk::MapChanged(map));
            }
            if me.cell == to {
                return Ok(Walk::Reached);
            }
            let overrides = self.overrides(game, start);
            let mut occupied = self.occupied(start);
            occupied.extend_from_slice(avoid);
            let dir = Planner::new(game, start, &overrides, occupied)
                .first_dir(me.cell, to)
                .ok_or_else(|| {
                    anyhow!("walk_to: no path from {:?} to {to:?} on {start:?}", me.cell)
                })?;
            // Hold the direction until the player leaves the cell, or the field stops idling;
            // a bump (an object stepped in the way) re-plans after a while.
            let mut held = 0;
            loop {
                if self.frames >= end {
                    return Err(anyhow!("walk_to {to:?}: stuck at {:?}", me.cell));
                }
                self.step(button(dir))?;
                held += 1;
                let now = self.player_object().ok_or_else(|| anyhow!("no player"))?;
                if now.cell != me.cell || self.current_map()? != start {
                    break;
                }
                if !matches!(self.mode(), Mode::FieldIdle | Mode::Transition) {
                    break;
                }
                if held > BUMP_FRAMES {
                    stalls += 1;
                    if stalls > MAX_STALLS {
                        return Err(anyhow!(
                            "walk_to {to:?}: bumping {dir:?} at {:?} (planner divergence?)",
                            me.cell
                        ));
                    }
                    self.step(Buttons::NONE)?;
                    break;
                }
            }
            if !matches!(self.mode(), Mode::FieldIdle | Mode::Transition) {
                self.settle(end.saturating_sub(self.frames))?;
                let now = self.player_object().ok_or_else(|| anyhow!("no player"))?;
                let map = self.current_map()?;
                if map != start {
                    return Ok(Walk::MapChanged(map));
                }
                if now.cell != to {
                    return Ok(Walk::Interrupted(now.cell));
                }
            }
        }
    }

    /// Holds `dir` until the map changes or the field stops idling (arrow warps, such as the
    /// truck's door, need the direction pressed on the warp cell), then settles.
    pub fn push(&mut self, dir: Dir, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        let map = self.current_map()?;
        while self.mode() == Mode::FieldIdle && self.current_map()? == map {
            if self.frames >= end {
                return Err(anyhow!("push {dir:?}: nothing happened"));
            }
            self.step(button(dir))?;
        }
        self.settle(end.saturating_sub(self.frames))
    }

    /// Turns to `dir` in place (a tap turns without stepping), then settles.
    pub fn face(&mut self, dir: Dir, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        while self.player_object().map(|o| o.facing) != Some(dir) {
            if self.frames >= end {
                return Err(anyhow!("face: cannot face {dir:?}"));
            }
            self.tap(button(dir))?;
        }
        self.settle(end.saturating_sub(self.frames))
    }

    /// One step toward `dir`: holds it until the player leaves the cell or something takes
    /// over (a bump into a scripted cell), then settles.
    pub fn walk_dir(&mut self, dir: Dir, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        let at = self.player_object().map(|o| o.cell);
        let map = self.current_map()?;
        while self.mode() == Mode::FieldIdle
            && self.player_object().map(|o| o.cell) == at
            && self.current_map()? == map
        {
            if self.frames >= end {
                return Err(anyhow!("step {dir:?}: never moved"));
            }
            self.step(button(dir))?;
        }
        self.settle(end.saturating_sub(self.frames))
    }

    /// Holds `keys` for `frames` frames, then settles.
    pub fn hold(&mut self, keys: Buttons, frames: u16, max_frames: u64) -> Result<()> {
        for _ in 0..frames {
            self.step(keys)?;
        }
        self.settle(max_frames)
    }

    /// Faces `dir` (from the field) and presses A, then settles.
    pub fn interact(&mut self, dir: Dir, max_frames: u64) -> Result<()> {
        let end = self.frames + max_frames;
        self.face(dir, max_frames)?;
        self.tap(Buttons::A)?;
        self.settle(end.saturating_sub(self.frames))
    }
}
