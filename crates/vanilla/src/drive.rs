//! First driving primitives on BPEE, all closed-loop on RAM (a foretaste of Task O2's
//! `vanilla::Live`): [`Vanilla::settle`] answers what waits for input until the player has
//! control, [`Vanilla::walk_to`] walks with our planner (`explore::path::first_dir_on` over our map
//! data: Gen3 map ids are ours) around the objects read from RAM, and [`Vanilla::interact`]
//! faces a cell and presses A.
//!
//! Known simplifications: the planner sees our map data without `setmetatile` overrides; the
//! answers are YES, the first multichoice item, FIGHT and the first move; the starter is
//! [`Vanilla::starter`]; a nickname is declined by confirming the offered name.

use crate::state::{Mode, Vanilla};
use anyhow::{Result, anyhow};
use engine::Buttons;
use engine::data::{Dir, GameData, MapId};
use explore::path::first_dir_on;

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

fn button(d: Dir) -> Buttons {
    match d {
        Dir::Down => Buttons::DOWN,
        Dir::Up => Buttons::UP,
        Dir::Left => Buttons::LEFT,
        Dir::Right => Buttons::RIGHT,
    }
}

impl Vanilla {
    /// One frame `keys`, one frame released (a new press each call).
    pub fn tap(&mut self, keys: Buttons) -> Result<()> {
        self.step(keys)?;
        self.step(Buttons::NONE)
    }

    /// Steps until [`Mode::FieldIdle`] holds over a frame without input (within `max_frames`),
    /// answering every wait with A:
    /// messages, yes/no (YES), multichoices (the first item), other screens (the clock), the
    /// battle's menus (FIGHT, then the first move: the cursors are moved to 0 first) and its
    /// messages. Scripts and transitions run without input.
    pub fn settle(&mut self, max_frames: u64) -> Result<()> {
        let end = self.frames + max_frames;
        let mut idle = false;
        loop {
            let mode = self.mode();
            let keys = match mode {
                // Confirmed on a second frame: the field runs on-frame map scripts and triggers
                // only once it processes the player's (here: no) input.
                Mode::FieldIdle if idle => return Ok(()),
                Mode::FieldIdle => Buttons::NONE,
                Mode::Script | Mode::Transition => Buttons::NONE,
                Mode::BattleAction => match self.emu.read_u8(self.sym.action_selection_cursor) {
                    0 => Buttons::A,
                    1 => Buttons::LEFT,
                    _ => Buttons::UP,
                },
                Mode::BattleMove => match self.emu.read_u8(self.sym.move_selection_cursor) {
                    0 => Buttons::A,
                    1 => Buttons::LEFT,
                    _ => Buttons::UP,
                },
                // YES: the menu cursor (`sMenu.cursorPos`) up to 0 first.
                Mode::YesNo => self.yes(),
                Mode::Screen { .. } => self.screen_key(),
                Mode::Message | Mode::Multichoice | Mode::BattleTarget | Mode::BattleWaiting => {
                    Buttons::A
                }
            };
            if self.frames >= end {
                let inputs = self.mode_inputs();
                return Err(anyhow!(
                    "settle: still {mode:?} after {max_frames} frames (frame {}): {inputs}",
                    self.frames
                ));
            }
            idle = mode == Mode::FieldIdle;
            if keys == Buttons::NONE {
                self.step(keys)?;
            } else {
                self.tap(keys)?;
            }
        }
    }

    /// The key for a screen outside the field and the battle: the wall clock's yes/no (YES),
    /// the starter screen ([`Vanilla::starter`], then YES), the naming screen (the name it
    /// offers: START to OK, then A), A elsewhere.
    fn screen_key(&mut self) -> Buttons {
        let s = self.sym.clone();
        let tasks = self.tasks();
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
        Buttons::A
    }

    /// The key toward YES on a `menu.c` yes/no.
    fn yes(&mut self) -> Buttons {
        match self.emu.read_u8(self.sym.menu + 2) {
            0 => Buttons::A,
            _ => Buttons::UP,
        }
    }

    /// The cells the visible objects of the current map (but the player) stand on.
    fn occupied(&mut self, map: MapId) -> Vec<(i16, i16)> {
        (0..crate::state::layout::OBJECT_EVENTS)
            .filter_map(|i| self.object_event(i))
            .filter(|o| !o.is_player && o.visible && o.map == map)
            .map(|o| o.cell)
            .collect()
    }

    /// Walks to `to` on the current map (`game`'s collision data), settling whenever a script
    /// takes over. Ends on the target, on a map change, or when a trigger moved the player.
    pub fn walk_to<G: GameData>(
        &mut self,
        game: &G,
        to: (i16, i16),
        max_frames: u64,
    ) -> Result<Walk> {
        let end = self.frames + max_frames;
        self.settle(max_frames)?;
        let start = self.current_map()?;
        loop {
            let me = self.player_object().ok_or_else(|| anyhow!("no player"))?;
            let map = self.current_map()?;
            if map != start {
                return Ok(Walk::MapChanged(map));
            }
            if me.cell == to {
                return Ok(Walk::Reached);
            }
            let occupied = self.occupied(start);
            let dir = first_dir_on(game, start, &[], occupied, me.cell, to).ok_or_else(|| {
                anyhow!("walk_to: no path from {:?} to {to:?} on {start:?}", me.cell)
            })?;
            // Hold the direction until the player leaves the cell, or the field stops idling.
            loop {
                if self.frames >= end {
                    return Err(anyhow!("walk_to {to:?}: stuck at {:?}", me.cell));
                }
                self.step(button(dir))?;
                let now = self.player_object().ok_or_else(|| anyhow!("no player"))?;
                if now.cell != me.cell || self.current_map()? != start {
                    break;
                }
                if !matches!(self.mode(), Mode::FieldIdle | Mode::Transition) {
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

    /// Faces `dir` (from the field) and presses A, then settles.
    pub fn interact(&mut self, dir: Dir, max_frames: u64) -> Result<()> {
        self.settle(max_frames)?;
        let end = self.frames + max_frames;
        while self.player_object().map(|o| o.facing) != Some(dir) {
            if self.frames >= end {
                return Err(anyhow!("interact: cannot face {dir:?}"));
            }
            self.tap(button(dir))?;
        }
        self.tap(Buttons::A)?;
        self.settle(end.saturating_sub(self.frames))
    }
}
