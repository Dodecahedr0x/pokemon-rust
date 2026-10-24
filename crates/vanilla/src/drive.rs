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
use engine::data::{Dir, GameData, MapData, MapId, MoveId, SpeciesId};
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

/// The key that moves a 2-column grid menu's cursor (`menu.c`, row-major) from `c` to
/// `want`, or A once it is there.
fn grid_key(c: u8, want: u8) -> Buttons {
    match c.cmp(&want) {
        std::cmp::Ordering::Equal => Buttons::A,
        std::cmp::Ordering::Greater if c - want >= 2 => Buttons::UP,
        std::cmp::Ordering::Greater => Buttons::LEFT,
        std::cmp::Ordering::Less if want - c >= 2 => Buttons::DOWN,
        std::cmp::Ordering::Less => Buttons::RIGHT,
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
            if self.hall_of_fame || self.at_hall_of_fame() {
                // `GameClear`'s screens: the Hall of Fame, the credits, then a reset to the
                // title. The run ends at the Hall of Fame.
                self.hall_of_fame = true;
                return Ok(());
            }
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
            let mart = self.mart_key();
            let keys = match mode {
                _ if mart.is_some() => mart.unwrap_or(Buttons::NONE),
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
        let random = wild && flags & BATTLE_TYPE_FIRST_BATTLE == 0;
        // `Opts::catch`: a ball (BAG) at that species, RUN from every other wild mon.
        let catch = self.answers.catch;
        let foe = self
            .emu
            .read_u16(s.battle_mons + BATTLE_MON_SIZE + BM_SPECIES);
        let ball = random && catch != 0 && foe == catch;
        // A random encounter on a walk (no script waits on it: not a scripted wild battle)
        // with the lead under half its HP: RUN. Vanilla's RNG meets more wild mons than our
        // spine on the same walks; fighting them all wears a one-mon party down to a lost
        // trainer battle further on (Rusturf Tunnel). Not while training or catching.
        let scripted = self.emu.read_u8(s.script_context_status) != CONTEXT_SHUTDOWN;
        let worn = {
            let me = s.battle_mons;
            2 * self.emu.read_u16(me + BM_HP) < self.emu.read_u16(me + BM_MAX_HP)
        };
        let flee = !self.training && !scripted && worn;
        let run = random && (self.answers.run_wild || catch != 0 || flee) && !ball;
        match mode {
            Mode::BattleAction => toward(
                self.emu.read_u8(s.action_selection_cursor),
                if run {
                    3
                } else if ball {
                    1
                } else {
                    0
                },
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
                    // "Use next Pokémon?" (a wild battle): YES; "<foe> is about to use <mon>.
                    // Will you change Pokémon?" (a trainer battle, `BATTLE_STYLE_SHIFT`): NO,
                    // the lead stays in (`BattleScript_HandleFaintedMon`).
                    B_SCR_OP_YESNOBOX => {
                        let want = u8::from(!wild);
                        match self.emu.read_u8(s.battle_communication + 1).cmp(&want) {
                            std::cmp::Ordering::Equal => Buttons::A,
                            std::cmp::Ordering::Less => Buttons::DOWN,
                            std::cmp::Ordering::Greater => Buttons::UP,
                        }
                    }
                    // "Give a nickname to the caught <mon>?": no (B).
                    B_SCR_OP_TRYGIVECAUGHTMONNICK => Buttons::B,
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
        forget_slot_of(SpeciesId(lead.species), &moves, MoveId(new), t, &|_| false)
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
        if self.trace {
            eprintln!(
                "  battle move: {species:?} L{} hp {}/{} moves {moves:?} pp {pp:?} vs {} L{} hp {} best {}",
                self.emu.read_u8(me + BM_LEVEL),
                self.emu.read_u16(me + BM_HP),
                self.emu.read_u16(me + BM_MAX_HP),
                self.emu.read_u16(foe + BM_SPECIES),
                self.emu.read_u8(foe + BM_LEVEL),
                self.emu.read_u16(foe + BM_HP),
                self.answers.best_move,
            );
        }
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
        if self.callback2() == s.cb2_bag_menu_run && self.answers.catch != 0 {
            // The battle's bag (only a catch opens it): the POKé BALLS pocket's first ball,
            // USE (`Opts::catch`; our battle scene's degraded BAG throws the same ball).
            return self.ball_key();
        }
        if self.callback2() == s.cb2_pokenav {
            return self.pokenav_key();
        }
        if self.callback2() == s.cb2_update_party_menu
            && tasks.contains(&s.task_handle_choose_mon_input)
        {
            // A fainted lead's replacement (or a script's choice): the first mon that can
            // fight. A on a fainted one only prints "no will to fight" and asks again.
            let mut want = (self.party().iter()).position(|m| m.hp > 0).unwrap_or(0) as u8;
            if self.emu.read_u8(s.party_menu + PM_MENU_TYPE) & 0xF == PARTY_MENU_TYPE_CHOOSE_HALF {
                // The choose-half menu (Steven's multi battle, `ChooseHalfPartyForBattle`):
                // ENTER the mons that can fight in party order (up to 3: `max`, then the
                // menu moves to CONFIRM itself), then CONFIRM (slot `PARTY_SIZE`), as our
                // explorer's policy does.
                let order = self.emu.read_bytes(s.selected_order_from_party, 3);
                let party = self.party();
                let next = (party.iter().enumerate())
                    .find(|(i, m)| m.hp > 0 && !order.contains(&(*i as u8 + 1)))
                    .map(|(i, _)| i as u8);
                want = match next {
                    Some(i) if order.contains(&0) => i,
                    _ => PARTY_SIZE as u8,
                };
            }
            let slot = self.emu.read_u8(s.party_menu + PM_SLOT_ID);
            if self.trace {
                let hp: Vec<_> = self.party().iter().map(|m| (m.species, m.hp)).collect();
                eprintln!("  party menu: slot {slot} want {want} party {hp:?}");
            }
            return if slot == want {
                Buttons::A
            } else {
                Buttons::DOWN
            };
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
            // Gen3 collides with invisible objects too (`ObjectEventCollidesWithObjectAt`
            // checks `active` only): Route 120's hidden Kecleon blocks its cell.
            .filter(|o| !o.is_player && o.map == map)
            .map(|o| o.cell)
            .collect()
    }

    /// The rotating gate puzzle of the current map and the gates' orientations (Gen3
    /// `GetCurrentMapRotatingGatePuzzleType`: Fortree's Gym, `MAP_FORTREE_CITY_GYM` 12.1, and
    /// the Trick House's sixth puzzle, `MAP_ROUTE110_TRICK_HOUSE_PUZZLE6` 29.8), read from
    /// RAM and the ROM's tables ([`crate::gates`]); `None` elsewhere.
    pub fn rotating_gates(&mut self) -> Option<(crate::gates::Gates, u32)> {
        let map = self.current_map().ok()?;
        if !matches!(map.0, 0x0C01 | 0x1D08) {
            return None;
        }
        let config = self.emu.read_u32(self.sym.rotating_gate_puzzle_config);
        let count = self.emu.read_u8(self.sym.rotating_gate_puzzle_count) as usize;
        if config >> 24 != 8 || count == 0 || count > 16 {
            return None;
        }
        // `struct RotatingGatePuzzle`: x, y (s16), shape, orientation; 8 bytes with agbcc.
        let raw = self.emu.read_bytes(config, 8 * count);
        let gates: Vec<crate::gates::Gate> = (raw.chunks(8))
            .map(|g| crate::gates::Gate {
                x: i16::from_le_bytes([g[0], g[1]]) + MAP_OFFSET,
                y: i16::from_le_bytes([g[2], g[3]]) + MAP_OFFSET,
                shape: g[4],
            })
            .collect();
        let shapes = 1 + gates.iter().map(|g| g.shape as usize).max()?;
        let layout = self
            .emu
            .read_bytes(self.sym.rotating_gate_arm_layout, 8 * shapes);
        let arm_layout = layout
            .chunks(8)
            .map(|c| <[u8; 8]>::try_from(c).unwrap_or_default())
            .collect();
        let mut rotation = [[0u8; 16]; 4];
        // By `gates::table`'s order: up (north), down (south), left (west), right (east).
        let tables = [
            self.sym.rotating_gate_rotation_north,
            self.sym.rotating_gate_rotation_south,
            self.sym.rotating_gate_rotation_west,
            self.sym.rotating_gate_rotation_east,
        ];
        for (r, at) in rotation.iter_mut().zip(tables) {
            self.emu.read_into(at, r);
        }
        let mut arm_positions = [[(0i8, 0i8); 8]; 2];
        let tables = [
            self.sym.rotating_gate_arms_anticlockwise,
            self.sym.rotating_gate_arms_clockwise,
        ];
        for (p, at) in arm_positions.iter_mut().zip(tables) {
            // `struct Coords8` (two s8): 4 bytes apart with agbcc (the symbol is 0x20 long).
            let b = self.emu.read_bytes(at, 32);
            for (i, c) in p.iter_mut().enumerate() {
                *c = (b[4 * i] as i8, b[4 * i + 1] as i8);
            }
        }
        let at = self.sym.backup_map_layout;
        let (w, h) = (
            self.emu.read_u32(at) as usize,
            self.emu.read_u32(at + 4) as usize,
        );
        let ptr = self.emu.read_u32(at + 8);
        if !EWRAM.contains(&ptr) || w * h > 0x8000 {
            return None;
        }
        let bytes = self.emu.read_bytes(ptr, 2 * w * h);
        let collision = (bytes.chunks(2))
            .map(|b| ((u16::from_le_bytes([b[0], b[1]]) & 0x0C00) >> 10) as u8)
            .collect();
        // The orientations: the temp vars from `VAR_TEMP_0`, a byte per gate.
        let sb1 = self.save_block1().ok()?;
        let o = self.emu.read_bytes(sb1 + SB1_VARS, count);
        let orientations =
            (o.iter().enumerate()).fold(0u32, |acc, (i, &b)| acc | (u32::from(b & 3) << (2 * i)));
        let g = crate::gates::Gates {
            gates,
            arm_layout,
            rotation,
            arm_positions,
            collision,
            w: w as i16,
            h: h as i16,
        };
        Some((g, orientations))
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
        let data = self.layout_data(game, map);
        overrides_from_grid(data, w, h, &grid).unwrap_or_default()
    }

    /// `gSaveBlock1Ptr->mapLayoutId`: the layout the current map is loaded with (its own, or
    /// a `setmaplayoutindex` override).
    pub fn layout_id(&mut self) -> Option<u16> {
        let sb1 = self.save_block1().ok()?;
        Some(self.emu.read_u16(sb1 + SB1_MAP_LAYOUT_ID))
    }

    /// Our data for map `map` as vanilla has it loaded: its layout override's variant
    /// ([`Vanilla::layout_id`], `GameData::map_layout`), else its own.
    pub fn layout_data<G: GameData>(&mut self, game: &G, map: MapId) -> &'static MapData {
        let own = game.map(map);
        match self.layout_id() {
            Some(l) => game.map_layout(map, l).unwrap_or(own),
            None => own,
        }
    }

    /// The player's elevation (`ObjectEvent.currentElevation`): on a bridge or under it.
    pub fn player_elevation(&mut self) -> Option<u8> {
        let id = u32::from(
            self.emu
                .read_u8(self.sym.player_avatar + PA_OBJECT_EVENT_ID),
        );
        if id >= OBJECT_EVENTS {
            return None;
        }
        let at = self.sym.object_events + id * OBJECT_EVENT_SIZE + OE_ELEVATION;
        Some(self.emu.read_u8(at) & 0xF)
    }

    /// Our planner on `map` as vanilla has it now: its layout (the override), the player's
    /// elevation, `overrides` and the `occupied` cells.
    pub fn planner<'a, G: GameData>(
        &mut self,
        game: &'a G,
        map: MapId,
        overrides: &'a [((i16, i16), u16)],
        occupied: Vec<(i16, i16)>,
    ) -> Planner<'a, G> {
        let layout = self.layout_id();
        let elevation = self.player_elevation();
        Planner::new(game, map, overrides, occupied)
            .with_layout(map, layout)
            .with_elevation(elevation)
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
            let planner = self.planner(game, start, &overrides, occupied);
            // Rotating gates (our engine has none): a walk that pushes them where they turn.
            let gated = self.rotating_gates().and_then(|(g, o)| {
                let at = |(x, y): (i16, i16)| (x + MAP_OFFSET, y + MAP_OFFSET);
                g.first_dir(at(me.cell), at(to), o, |(x, y)| {
                    planner.walkable((x - MAP_OFFSET, y - MAP_OFFSET))
                })
            });
            let dir = gated
                .or_else(|| planner.first_dir(me.cell, to))
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
                // A trigger on the target itself: its scene may move the player off it (Meteor
                // Falls' Magma scene pushes the player aside); the walk did reach it.
                let at = self.player_object().map(|o| o.cell);
                self.settle(end.saturating_sub(self.frames))?;
                let now = self.player_object().ok_or_else(|| anyhow!("no player"))?;
                let map = self.current_map()?;
                if map != start {
                    return Ok(Walk::MapChanged(map));
                }
                if at == Some(to) {
                    return Ok(Walk::Reached);
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

/// Frames a [`Vanilla::use_item`] may take (the start menu, the bag, the party menu, the
/// summary screen, their fades and messages).
pub const USE_ITEM_FRAMES: u64 = 6_000;

/// The key from bag list position `pos` toward `at` (the list shows the pocket's non-empty
/// slots in order), A on it.
fn list_key(pos: u16, at: u16) -> Buttons {
    match pos.cmp(&at) {
        std::cmp::Ordering::Less => Buttons::DOWN,
        std::cmp::Ordering::Greater => Buttons::UP,
        std::cmp::Ordering::Equal => Buttons::A,
    }
}

impl Vanilla {
    /// The bag pocket (0-based `*_POCKET`) and list position of `item`, read from the save
    /// block's pockets (the bag sorts its TM/HM and berry pockets in place when it opens).
    fn bag_slot(&mut self, item: u16) -> Option<(u8, u16)> {
        let sb1 = self.save_block1().ok()?;
        for (i, &(off, len)) in SB1_POCKETS.iter().enumerate() {
            let b = self.emu.read_bytes(sb1 + off, 4 * len);
            let at = (b.chunks(4))
                .map(|s| u16::from_le_bytes([s[0], s[1]]))
                .filter(|&it| it != 0)
                .position(|it| it == item);
            if let Some(at) = at {
                return Some((BAG_POCKET_OF_SB1[i], at as u16));
            }
        }
        None
    }

    /// The key [`Vanilla::use_item`] presses on this frame, from RAM: START on the field, the
    /// start menu's cursor to BAG (`sCurrentStartMenuActions`), the bag's pocket
    /// (`gBagPosition.pocket`) and list position (scroll + cursor) to `item`, USE (the context
    /// menu's first cell), the party menu's `gPartyMenu.slotId` to `mon`, the summary's forget
    /// cursor to `forget`, YES and A through the prompts. Once the party menu was open (`used`),
    /// B out of the bag and the start menu. `None`: the field is back after the use.
    fn use_item_key(
        &mut self,
        used: &mut bool,
        item: u16,
        mon: u8,
        forget: u8,
    ) -> Result<Option<Buttons>> {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        let tasks = self.tasks();
        if cb2 == s.cb2_update_party_menu || tasks.contains(&s.task_replace_move) {
            *used = true;
        }
        if tasks.contains(&s.task_replace_move) {
            let pss = self.emu.read_u32(s.mon_summary_screen);
            if pss == 0 {
                return Ok(Some(Buttons::NONE));
            }
            let at = self.emu.read_u8(pss + PSS_FIRST_MOVE_INDEX);
            return Ok(Some(vertical(at, forget)));
        }
        if cb2 == s.cb2_overworld {
            let mode = self.mode();
            let start_menu = mode != Mode::FieldIdle
                && (self.emu.read_u32(s.menu_callback) & !1) == s.handle_start_menu_input
                && !self.fade_active();
            if start_menu {
                if *used {
                    return Ok(Some(Buttons::B));
                }
                let n = self.emu.read_u8(s.num_start_menu_actions).min(9);
                let actions = (self.emu).read_bytes(s.current_start_menu_actions, usize::from(n));
                let want = (actions.iter().position(|&a| a == MENU_ACTION_BAG))
                    .ok_or_else(|| anyhow!("use_item: no BAG in the start menu {actions:?}"))?;
                let at = self.emu.read_u8(s.start_menu_cursor_pos);
                return Ok(Some(vertical(at, want as u8)));
            }
            return Ok(match mode {
                Mode::FieldIdle if *used => None,
                Mode::FieldIdle => Some(Buttons::START),
                Mode::Message => Some(Buttons::A),
                Mode::YesNo => Some(self.yes()),
                _ => Some(Buttons::NONE),
            });
        }
        if cb2 == s.cb2_bag_menu_run {
            if self.fade_active() {
                return Ok(Some(Buttons::NONE));
            }
            if tasks.contains(&s.task_bag_menu_handle_input) {
                if *used {
                    return Ok(Some(Buttons::B));
                }
                let (pocket, at) = (self.bag_slot(item))
                    .ok_or_else(|| anyhow!("use_item: item {item} is not in the bag"))?;
                let bp = s.bag_position;
                let cur = self.emu.read_u8(bp + BP_POCKET);
                if cur != pocket {
                    return Ok(Some(if cur < pocket {
                        Buttons::RIGHT
                    } else {
                        Buttons::LEFT
                    }));
                }
                let p = u32::from(pocket);
                let pos = self.emu.read_u16(bp + BP_SCROLL_POSITION + 2 * p)
                    + self.emu.read_u16(bp + BP_CURSOR_POSITION + 2 * p);
                return Ok(Some(list_key(pos, at)));
            }
            if tasks.contains(&s.task_item_context_multiple_rows)
                || tasks.contains(&s.task_item_context_single_row)
            {
                if *used {
                    return Ok(Some(Buttons::B));
                }
                // USE is the first cell of the context menus that have it (`item_menu.c`
                // `sContextMenuItems_*Pocket`, a 2-column grid), but the berries' (CHECK TAG,
                // -, USE, GIVE, TOSS, CANCEL: the third).
                let berries = self
                    .bag_slot(item)
                    .is_some_and(|(p, _)| p == BAG_POCKET_BERRIES);
                let want = if berries { 2 } else { 0 };
                return Ok(Some(grid_key(self.emu.read_u8(s.menu + 2), want)));
            }
            if tasks.contains(&s.task_yes_no) {
                return Ok(Some(self.yes()));
            }
            return Ok(Some(Buttons::A));
        }
        if cb2 == s.cb2_update_party_menu {
            if tasks.contains(&s.task_handle_choose_mon_input) {
                let slot = self.emu.read_u8(s.party_menu + PM_SLOT_ID);
                return Ok(Some(if slot == mon {
                    Buttons::A
                } else {
                    Buttons::DOWN
                }));
            }
            if tasks.contains(&s.task_yes_no) {
                return Ok(Some(self.yes()));
            }
            // The party menu's messages and its yes/no boxes (YES is their first cursor).
            return Ok(Some(if self.fade_active() {
                Buttons::NONE
            } else {
                Buttons::A
            }));
        }
        // Fades and set-ups between the screens.
        Ok(Some(if self.fade_active() {
            Buttons::NONE
        } else {
            Buttons::A
        }))
    }

    /// Uses `item` from the bag on party slot `mon` (START → BAG → the pocket → the item →
    /// USE → the party menu), forgetting move slot `forget` when the mon knows four moves
    /// (Gen3 `ItemUseOutOfBattle_TMHM` → `ItemUseCB_TMHM`), then backs out to the field. Every
    /// input is read from RAM ([`Vanilla::use_item_key`]).
    pub fn use_item(&mut self, item: u16, mon: u8, forget: u8, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        let mut used = false;
        let mut idle = 0;
        loop {
            if self.frames >= end {
                let inputs = self.mode_inputs();
                return Err(anyhow!(
                    "use_item {item}: not back on the field after {max_frames} frames \
                     (used {used}): {inputs}"
                ));
            }
            match self.use_item_key(&mut used, item, mon, forget)? {
                None => {
                    idle += 1;
                    if idle > SETTLE_IDLE {
                        return self.settle(max_frames);
                    }
                    self.step(Buttons::NONE)?;
                }
                Some(Buttons::NONE) => {
                    idle = 0;
                    self.step(Buttons::NONE)?;
                }
                Some(k) => {
                    idle = 0;
                    self.tap(k)?;
                }
            }
        }
    }
}

/// Frames a [`Vanilla::fly`] may take (the start menu, the party menu, the fly map, the
/// flight).
pub const FLY_FRAMES: u64 = 6_000;

impl Vanilla {
    /// Whether `GameClear`'s screens run: the Hall of Fame (`CB2_DoHallOfFameScreen`,
    /// `CB2_HallOfFame`) or the credits after it.
    pub fn at_hall_of_fame(&mut self) -> bool {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        [
            s.cb2_do_hall_of_fame_screen,
            s.cb2_hall_of_fame,
            s.cb2_start_credits_sequence,
            s.cb2_credits,
        ]
        .contains(&cb2)
    }

    /// The quantity of `item` in the bag (every slot).
    pub fn bag_count(&mut self, item: u16) -> u16 {
        self.state().map_or(0, |st| {
            (st.bag.iter())
                .filter(|&&(_, it, _)| it == item)
                .map(|&(_, _, q)| q)
                .sum()
        })
    }

    /// The Poké Mart's key (`shop.c`) while it is open, per [`Vanilla::buying`]: BUY on the
    /// shop menu (`Task_ShopMenu`, `sMenu`), the item in the list (`sMartInfo.itemList`, the
    /// list's `sShopData` scroll and row), the quantity (`tItemCount`: UP +1, DOWN -1), A
    /// through the confirm and the messages; once bought (or with nothing to buy), B out of the
    /// list and QUIT. `None`: no mart screen.
    pub fn mart_key(&mut self) -> Option<Buttons> {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        if cb2 != s.cb2_overworld && cb2 != s.cb2_buy_menu {
            return None;
        }
        let tasks = self.tasks();
        let shop_menu = cb2 == s.cb2_overworld && tasks.contains(&s.task_shop_menu);
        if !shop_menu && cb2 != s.cb2_buy_menu {
            return None;
        }
        if self.fade_active() {
            return Some(Buttons::NONE);
        }
        let pending = match self.buying {
            Some((item, qty, had)) => {
                (self.bag_count(item) < had.saturating_add(qty)).then_some((item, qty))
            }
            None => None,
        };
        if shop_menu {
            // BUY / SELL / QUIT.
            let at = self.emu.read_u8(s.menu + 2);
            return Some(vertical(at, if pending.is_some() { 0 } else { 2 }));
        }
        if tasks.contains(&s.task_buy_menu) {
            let Some((item, _)) = pending else {
                return Some(Buttons::B);
            };
            let mi = s.mart_info;
            let list = self.emu.read_u32(mi + MI_ITEM_LIST);
            let n = self.emu.read_u16(mi + MI_ITEM_COUNT);
            let at = (0..n).find(|&i| self.emu.read_u16(list + 2 * u32::from(i)) == item);
            let Some(at) = at else {
                // Not sold here: leave.
                self.buying = None;
                return Some(Buttons::B);
            };
            let sd = self.emu.read_u32(s.shop_data);
            let pos =
                self.emu.read_u16(sd + SD_SCROLL_OFFSET) + self.emu.read_u16(sd + SD_SELECTED_ROW);
            return Some(list_key(pos, at));
        }
        if let Some(d) = self.task_data(s.task_buy_how_many) {
            let Some((_, qty)) = pending else {
                return Some(Buttons::B);
            };
            return Some(match (d[1] as u16).cmp(&qty) {
                std::cmp::Ordering::Less => Buttons::UP,
                std::cmp::Ordering::Greater => Buttons::DOWN,
                std::cmp::Ordering::Equal => Buttons::A,
            });
        }
        Some(Buttons::A)
    }

    /// The battle bag's key for a catch: the POKé BALLS pocket, its first ball, USE.
    fn ball_key(&mut self) -> Buttons {
        let s = self.sym.clone();
        if self.fade_active() {
            return Buttons::NONE;
        }
        let tasks = self.tasks();
        if tasks.contains(&s.task_bag_menu_handle_input) {
            let bp = s.bag_position;
            let cur = self.emu.read_u8(bp + BP_POCKET);
            if cur != BAG_POCKET_BALLS {
                return if cur < BAG_POCKET_BALLS {
                    Buttons::RIGHT
                } else {
                    Buttons::LEFT
                };
            }
            let p = u32::from(BAG_POCKET_BALLS);
            let pos = self.emu.read_u16(bp + BP_SCROLL_POSITION + 2 * p)
                + self.emu.read_u16(bp + BP_CURSOR_POSITION + 2 * p);
            return list_key(pos, 0);
        }
        if tasks.contains(&s.task_item_context_multiple_rows)
            || tasks.contains(&s.task_item_context_single_row)
        {
            return match self.emu.read_u8(s.menu + 2) {
                0 => Buttons::A,
                1 => Buttons::LEFT,
                _ => Buttons::UP,
            };
        }
        Buttons::A
    }

    /// The key [`Vanilla::fly`] presses on this frame, from RAM: START, the start menu's
    /// POKéMON, party slot `mon`, FLY in its action list (`sPartyMenuInternal->actions`), then
    /// the fly map's cursor (`sFlyMap->regionMap.cursorPos`) onto `MAPSEC` `dest`'s cell
    /// (`gRegionMapEntries`) and A. `None`: back on the field after the flight.
    fn fly_key(&mut self, flown: &mut bool, mon: u8, dest: u8) -> Result<Option<Buttons>> {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        let tasks = self.tasks();
        if cb2 == s.cb2_overworld {
            let mode = self.mode();
            let start_menu = mode != Mode::FieldIdle
                && (self.emu.read_u32(s.menu_callback) & !1) == s.handle_start_menu_input
                && !self.fade_active();
            if start_menu {
                if *flown {
                    return Ok(Some(Buttons::B));
                }
                let n = self.emu.read_u8(s.num_start_menu_actions).min(9);
                let actions = (self.emu).read_bytes(s.current_start_menu_actions, usize::from(n));
                let want = (actions.iter().position(|&a| a == MENU_ACTION_POKEMON))
                    .ok_or_else(|| anyhow!("fly: no POKéMON in the start menu {actions:?}"))?;
                let at = self.emu.read_u8(s.start_menu_cursor_pos);
                return Ok(Some(vertical(at, want as u8)));
            }
            return Ok(match mode {
                Mode::FieldIdle if *flown => None,
                Mode::FieldIdle => Some(Buttons::START),
                Mode::Message => Some(Buttons::A),
                _ => Some(Buttons::NONE),
            });
        }
        if self.fade_active() {
            return Ok(Some(Buttons::NONE));
        }
        if cb2 == s.cb2_update_party_menu {
            if *flown {
                return Ok(Some(Buttons::B));
            }
            if tasks.contains(&s.task_handle_choose_mon_input) {
                let slot = self.emu.read_u8(s.party_menu + PM_SLOT_ID);
                return Ok(Some(if slot == mon {
                    Buttons::A
                } else {
                    Buttons::DOWN
                }));
            }
            if tasks.contains(&s.task_handle_selection_menu_input) {
                let pmi = self.emu.read_u32(s.party_menu_internal);
                let n = self.emu.read_u8(pmi + PMI_NUM_ACTIONS).min(8);
                let actions = self.emu.read_bytes(pmi + PMI_ACTIONS, usize::from(n));
                let want = (actions.iter().position(|&a| a == MENU_FIELD_MOVE_FLY))
                    .ok_or_else(|| anyhow!("fly: party slot {mon} has no FLY {actions:?}"))?;
                return Ok(Some(vertical(self.emu.read_u8(s.menu + 2), want as u8)));
            }
            // "Can't use that here" and the like.
            return Ok(Some(Buttons::A));
        }
        if cb2 == s.cb2_fly_map {
            let fm = self.emu.read_u32(s.fly_map);
            if fm == 0 || self.emu.read_u16(fm + FM_STATE) != 0 {
                return Ok(Some(Buttons::NONE));
            }
            let rm = fm + FM_REGION_MAP;
            if self.emu.read_u32(rm + RM_INPUT_CALLBACK) & !1 != s.process_region_map_input_full {
                // The cursor is moving.
                return Ok(Some(Buttons::NONE));
            }
            let e = s.region_map_entries + 8 * u32::from(dest);
            let want = (
                u16::from(self.emu.read_u8(e)) + MAPCURSOR_X_MIN,
                u16::from(self.emu.read_u8(e + 1)) + MAPCURSOR_Y_MIN,
            );
            let at = (
                self.emu.read_u16(rm + RM_CURSOR_X),
                self.emu.read_u16(rm + RM_CURSOR_Y),
            );
            let key = match (at.0.cmp(&want.0), at.1.cmp(&want.1)) {
                (std::cmp::Ordering::Less, _) => Buttons::RIGHT,
                (std::cmp::Ordering::Greater, _) => Buttons::LEFT,
                (_, std::cmp::Ordering::Less) => Buttons::DOWN,
                (_, std::cmp::Ordering::Greater) => Buttons::UP,
                _ => {
                    let sec = self.emu.read_u16(rm + RM_MAP_SEC_ID);
                    if sec != u16::from(dest) {
                        return Err(anyhow!(
                            "fly: the cursor at {at:?} is on MAPSEC {sec}, not {dest}"
                        ));
                    }
                    *flown = true;
                    Buttons::A
                }
            };
            return Ok(Some(key));
        }
        // The flight's screens and set-ups.
        Ok(Some(Buttons::NONE))
    }

    /// Flies with party slot `mon` to fly destination `dest` (its `MAPSEC`), from the field:
    /// START → POKéMON → the slot → FLY → the fly map (Gen3 `CB2_OpenFlyMap`), every input read
    /// from RAM ([`Vanilla::fly_key`]), then settles.
    pub fn fly(&mut self, mon: u8, dest: u8, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        let mut flown = false;
        let mut idle = 0;
        loop {
            if self.frames >= end {
                let inputs = self.mode_inputs();
                return Err(anyhow!(
                    "fly to {dest}: not back on the field after {max_frames} frames (flown \
                     {flown}): {inputs}"
                ));
            }
            match self.fly_key(&mut flown, mon, dest)? {
                None => {
                    idle += 1;
                    if idle > SETTLE_IDLE {
                        return self.settle(max_frames);
                    }
                    self.step(Buttons::NONE)?;
                }
                Some(Buttons::NONE) => {
                    idle = 0;
                    self.step(Buttons::NONE)?;
                }
                Some(k) => {
                    idle = 0;
                    self.tap(k)?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_cursor_walks_rows_then_columns() {
        // Berries' context menu: CHECK TAG (0), -, USE (2), GIVE, TOSS, CANCEL.
        assert_eq!(grid_key(0, 2), Buttons::DOWN);
        assert_eq!(grid_key(1, 2), Buttons::RIGHT);
        assert_eq!(grid_key(3, 2), Buttons::LEFT);
        assert_eq!(grid_key(5, 2), Buttons::UP);
        assert_eq!(grid_key(2, 2), Buttons::A);
    }

    #[test]
    fn list_cursors_step_toward_the_target() {
        assert_eq!(vertical(0, 2), Buttons::DOWN);
        assert_eq!(vertical(3, 2), Buttons::UP);
        assert_eq!(vertical(2, 2), Buttons::A);
        assert_eq!(list_key(4, 1), Buttons::UP);
    }
}
