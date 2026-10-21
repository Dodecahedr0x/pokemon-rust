//! The warp state machine (Gen3 `DoWarp` / `DoDoorWarp` → `Task_WarpAndLoadMap` →
//! `FieldCB_DefaultWarpExit` → `Task_ExitDoor` / `Task_ExitNonAnimDoor` / `Task_ExitNonDoor`).
//!
//! Each frame the caller runs [`Transition::update`] and acts on the returned step:
//!
//! - `Walk(dir)`: start the player walking one cell `dir` this frame.
//! - `Walking`: the player is still walking; keep updating.
//! - `FadingOut` / `FadingIn`: nothing; the shared [`Fader`] was advanced.
//! - `Load(map, x, y)`: load `map` (field, objects, temp vars, `on_transition` / `on_load` map
//!   scripts) with the player on (x, y), then call [`Transition::arrive`] with the live behavior
//!   of that cell and face the player the way it returns. Returned once, on a black screen.
//! - `Done`: the player has control again.
//!
//! Integration notes:
//! - `Walk` steps are forced moves (Gen3 held movements): they ignore collision and input.
//! - While a transition is active it drives the shared [`Fader`]; the World must not update the
//!   fader itself on those frames.
//! - No player control and no triggers (coord, warp, on_frame) until `Done`, including during
//!   the exit walk; NPCs stay frozen until `Done` (Gen3 `FreezeObjectEvents` in the exit tasks).
//! - Gen3 `ClearTempFieldEventData` also clears five system flags (`FLAG_SYS_ENC_UP_ITEM`,
//!   `FLAG_SYS_ENC_DOWN_ITEM`, `FLAG_SYS_USE_STRENGTH`, `FLAG_SYS_CTRL_OBJ_DELETE`,
//!   `FLAG_NURSE_UNION_ROOM_REMINDER`); `Vars::clear_temp` is game-agnostic and does not, so
//!   content or the integration must clear them on `Load` and on connection crossing.

use super::fade::Fader;
use crate::data::{Behavior, Dir, GameData, MapId};
use crate::hash::Fnv64;
use crate::script::Fade;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransitionStep {
    Walk(Dir),
    Walking,
    FadingOut,
    Load(MapId, i16, i16),
    FadingIn,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    Idle,
    /// Walking into a door; `bool` = walk started.
    WalkIn(bool),
    /// `bool` = fade started.
    FadeOut(bool),
    Load,
    FadeIn(bool),
    WalkOut,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transition {
    stage: Stage,
    map: MapId,
    x: i16,
    y: i16,
    facing: Dir,
    /// The destination is a door or stairs: the player appears hidden and walks out.
    exit_walk: bool,
    player_hidden: bool,
}

impl Default for Transition {
    fn default() -> Self {
        Self::new()
    }
}

impl Transition {
    pub const fn new() -> Self {
        Self {
            stage: Stage::Idle,
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            exit_walk: false,
            player_hidden: false,
        }
    }

    /// Starts a warp to `map`: to its warp `warp` if that exists, else to (x, y) if both are
    /// non-negative, else to the map center (Gen3 `SetPlayerCoordsFromWarp`).
    ///
    /// The arrival facing and exit walk are decided after the load by [`arrive`](Self::arrive).
    pub fn start(&mut self, game: &impl GameData, map: MapId, warp: Option<u8>, x: i16, y: i16) {
        let m = game.map(map);
        let (x, y) = match warp.and_then(|w| m.warps.get(w as usize)) {
            Some(w) => (w.x, w.y),
            None if x >= 0 && y >= 0 => (x, y),
            None => ((m.width / 2) as i16, (m.height / 2) as i16),
        };
        *self = Self {
            stage: Stage::FadeOut(false),
            map,
            x,
            y,
            ..Self::new()
        };
    }

    /// Reports the behavior of the arrival cell, read from the live field after the load and its
    /// `on_load` script (Gen3 reads it then too), and returns the facing to give the player
    /// (Gen3 `GetAdjustedInitialDirection`, `SetUpWarpExitTask`):
    /// - `Door` or `Warp` (Gen3 animated and non-animated doors: house doors, stairs): face down,
    ///   hidden during the fade-in, then walk one cell down. `Warp` also covers ladders, which
    ///   Gen3 does not walk out of; that needs a future `Behavior` split.
    /// - `ArrowWarp(d)`: face away from `d`.
    /// - anything else: face down.
    ///
    /// Without this call the arrival is a plain one (face down, no exit walk).
    pub fn arrive(&mut self, behavior: Behavior) -> Dir {
        self.exit_walk = matches!(behavior, Behavior::Door | Behavior::Warp);
        self.player_hidden = self.exit_walk;
        self.facing = match behavior {
            Behavior::ArrowWarp(d) => d.opposite(),
            _ => Dir::Down,
        };
        self.facing
    }

    /// [`start`](Self::start) after first walking the player one cell up into the door in front
    /// and hiding them (Gen3 `DoDoorWarp` / `Task_DoDoorWarp`, without the door animation).
    pub fn start_door(
        &mut self,
        game: &impl GameData,
        map: MapId,
        warp: Option<u8>,
        x: i16,
        y: i16,
    ) {
        self.start(game, map, warp, x, y);
        self.stage = Stage::WalkIn(false);
    }

    pub const fn is_active(&self) -> bool {
        !matches!(self.stage, Stage::Idle)
    }

    /// Whether the player sprite should be hidden (inside the door before the fade-out, and
    /// during a door or stairs exit fade-in).
    pub const fn player_hidden(&self) -> bool {
        self.player_hidden
    }

    /// Runs one frame. `player_moving` is whether the player is still in the middle of a step.
    pub fn update(&mut self, fader: &mut Fader, player_moving: bool) -> TransitionStep {
        loop {
            match self.stage {
                Stage::Idle => return TransitionStep::Done,
                Stage::WalkIn(false) => {
                    self.stage = Stage::WalkIn(true);
                    return TransitionStep::Walk(Dir::Up);
                }
                Stage::WalkIn(true) => {
                    if player_moving {
                        return TransitionStep::Walking;
                    }
                    self.player_hidden = true;
                    self.stage = Stage::FadeOut(false);
                }
                Stage::FadeOut(false) => {
                    fader.start(Fade::ToBlack);
                    self.stage = Stage::FadeOut(true);
                }
                Stage::FadeOut(true) => {
                    fader.update();
                    if fader.is_done() {
                        self.stage = Stage::Load;
                    }
                    return TransitionStep::FadingOut;
                }
                Stage::Load => {
                    self.exit_walk = false;
                    self.player_hidden = false;
                    self.facing = Dir::Down;
                    self.stage = Stage::FadeIn(false);
                    return TransitionStep::Load(self.map, self.x, self.y);
                }
                Stage::FadeIn(false) => {
                    fader.start(Fade::FromBlack);
                    self.stage = Stage::FadeIn(true);
                }
                Stage::FadeIn(true) => {
                    fader.update();
                    if !fader.is_done() {
                        return TransitionStep::FadingIn;
                    }
                    // Gen3 starts the exit walk (or returns control) the frame the fade ends.
                    if self.exit_walk {
                        self.player_hidden = false;
                        self.stage = Stage::WalkOut;
                        return TransitionStep::Walk(Dir::Down);
                    }
                    self.stage = Stage::Idle;
                    return TransitionStep::Done;
                }
                Stage::WalkOut => {
                    if player_moving {
                        return TransitionStep::Walking;
                    }
                    self.stage = Stage::Idle;
                    return TransitionStep::Done;
                }
            }
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        let stage = match self.stage {
            Stage::Idle => 0,
            Stage::WalkIn(s) => 1 + s as u8,
            Stage::FadeOut(s) => 3 + s as u8,
            Stage::Load => 5,
            Stage::FadeIn(s) => 6 + s as u8,
            Stage::WalkOut => 8,
        };
        h.write(&[
            stage,
            self.facing as u8,
            self.exit_walk as u8,
            self.player_hidden as u8,
        ]);
        h.write_u16(self.map.0);
        h.write_u16(self.x as u16);
        h.write_u16(self.y as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_data::*;
    use super::*;
    use crate::flow::{FADE_MAX, behavior_at};
    use alloc::vec::Vec;

    /// Runs to `Done`, simulating a player whose walks take `walk` frames. Returns every step and
    /// the fader level after it.
    fn run(t: &mut Transition, fader: &mut Fader, walk: u32) -> Vec<(TransitionStep, u8, bool)> {
        let mut out = Vec::new();
        let mut walking: u32 = 0;
        for _ in 0..200 {
            let s = t.update(fader, walking > 0);
            walking = walking.saturating_sub(1);
            if let TransitionStep::Walk(_) = s {
                walking = walk;
            }
            if let TransitionStep::Load(m, x, y) = s {
                // Arrival facing, checked through `t.facing` by the tests.
                t.arrive(behavior_at(&Game, Game.map(m), x, y));
            }
            out.push((s, fader.level(), t.player_hidden()));
            if s == TransitionStep::Done {
                return out;
            }
        }
        panic!("transition never finished");
    }

    fn count(steps: &[(TransitionStep, u8, bool)], s: TransitionStep) -> usize {
        steps.iter().filter(|x| x.0 == s).count()
    }

    #[test]
    fn idle_is_done() {
        let mut t = Transition::new();
        assert!(!t.is_active());
        assert_eq!(t.update(&mut Fader::new(), false), TransitionStep::Done);
    }

    #[test]
    fn plain_warp_fades_out_loads_once_and_fades_in() {
        let mut t = Transition::new();
        let mut f = Fader::new();
        // House warp 1 is on a normal cell.
        t.start(&Game, HOUSE, Some(1), -1, -1);
        assert!(t.is_active());
        let steps = run(&mut t, &mut f, 0);
        assert_eq!(count(&steps, TransitionStep::FadingOut), 16);
        assert_eq!(steps[15].1, FADE_MAX, "black before the load");
        assert_eq!(steps[16].0, TransitionStep::Load(HOUSE, 2, 0));
        assert_eq!(steps[16].1, FADE_MAX, "load happens on a black screen");
        assert_eq!(count(&steps, TransitionStep::FadingIn), 15);
        assert_eq!(steps.last().unwrap().0, TransitionStep::Done);
        assert_eq!(steps.last().unwrap().1, 0);
        assert_eq!(steps.len(), 16 + 1 + 16);
        assert!(steps.iter().all(|s| !s.2), "player never hidden");
        assert_eq!(t.facing, Dir::Down);
        assert!(!t.is_active());
    }

    #[test]
    fn door_destination_walks_out_south_hidden_during_fade_in() {
        let mut t = Transition::new();
        let mut f = Fader::new();
        // House warp 0 is on a door.
        t.start(&Game, HOUSE, Some(0), -1, -1);
        let steps = run(&mut t, &mut f, 16);
        let load = steps
            .iter()
            .position(|s| matches!(s.0, TransitionStep::Load(..)));
        let load = load.unwrap();
        assert_eq!(steps[load].0, TransitionStep::Load(HOUSE, 1, 3));
        let walk = steps
            .iter()
            .position(|s| s.0 == TransitionStep::Walk(Dir::Down))
            .unwrap();
        assert_eq!(walk, load + 16, "walk starts the frame the fade-in ends");
        assert_eq!(steps[walk].1, 0);
        assert!(
            steps[load..walk].iter().all(|s| s.2),
            "hidden while fading in"
        );
        assert!(steps[walk..].iter().all(|s| !s.2));
        assert_eq!(count(&steps, TransitionStep::Walking), 16);
        assert_eq!(steps.len(), walk + 1 + 16 + 1);
    }

    #[test]
    fn stairs_destination_also_walks_out() {
        let mut t = Transition::new();
        // Town (4, 4) is a `Warp` cell (Gen3 stairs are MB_NON_ANIMATED_DOOR).
        t.start(&Game, TOWN, Some(1), -1, -1);
        let steps = run(&mut t, &mut Fader::new(), 1);
        assert!(steps.contains(&(TransitionStep::Load(TOWN, 4, 4), FADE_MAX, true)));
        assert_eq!(count(&steps, TransitionStep::Walk(Dir::Down)), 1);
    }

    #[test]
    fn arrow_warp_destination_faces_away_without_walking() {
        let mut t = Transition::new();
        // House warp 2 is a south arrow warp: arrive facing north.
        t.start(&Game, HOUSE, Some(2), -1, -1);
        let steps = run(&mut t, &mut Fader::new(), 1);
        assert!(
            steps
                .iter()
                .any(|s| s.0 == TransitionStep::Load(HOUSE, 3, 3))
        );
        assert_eq!(t.facing, Dir::Up);
        assert!(!steps.iter().any(|s| matches!(s.0, TransitionStep::Walk(_))));
    }

    #[test]
    fn destination_falls_back_to_xy_then_center() {
        let load = |warp, x, y| {
            let mut t = Transition::new();
            t.start(&Game, TOWN, warp, x, y);
            run(&mut t, &mut Fader::new(), 1)
                .into_iter()
                .find_map(|s| match s.0 {
                    TransitionStep::Load(m, x, y) => Some((m, x, y)),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(load(Some(0), 5, 5), (TOWN, 2, 1), "warp wins over xy");
        assert_eq!(load(None, 3, 2), (TOWN, 3, 2));
        assert_eq!(
            load(Some(9), 0, 4),
            (TOWN, 0, 4),
            "invalid warp falls back to xy"
        );
        assert_eq!(load(None, -1, 2), (TOWN, 3, 3), "map center");
    }

    #[test]
    fn door_entry_walks_up_hides_then_fades() {
        let mut t = Transition::new();
        let mut f = Fader::new();
        t.start_door(&Game, HOUSE, Some(1), -1, -1);
        let steps = run(&mut t, &mut f, 16);
        assert_eq!(steps[0], (TransitionStep::Walk(Dir::Up), 0, false));
        assert!(
            steps[1..17]
                .iter()
                .all(|s| s.0 == TransitionStep::Walking && !s.2)
        );
        assert_eq!(
            steps[17],
            (TransitionStep::FadingOut, 1, true),
            "fade starts as walk ends"
        );
        assert!(
            steps
                .iter()
                .any(|s| s.0 == TransitionStep::Load(HOUSE, 2, 0))
        );
        assert!(
            !steps.last().unwrap().2,
            "visible again on a normal destination"
        );
    }

    #[test]
    fn arrive_uses_the_live_behavior_and_defaults_to_plain() {
        let mut t = Transition::new();
        let mut f = Fader::new();
        t.start(&Game, HOUSE, Some(1), -1, -1);
        let mut step = t.update(&mut f, false);
        while step == TransitionStep::FadingOut {
            step = t.update(&mut f, false);
        }
        assert_eq!(step, TransitionStep::Load(HOUSE, 2, 0));
        // The static cell is plain, but a live override made it a door.
        assert_eq!(t.arrive(Behavior::Door), Dir::Down);
        assert!(t.player_hidden());
        assert_eq!(t.arrive(Behavior::ArrowWarp(Dir::Left)), Dir::Right);
        assert!(!t.player_hidden());
        assert_eq!(t.arrive(Behavior::Door), Dir::Down);
        let mut last = step;
        while last != TransitionStep::Done && !matches!(last, TransitionStep::Walk(_)) {
            last = t.update(&mut f, false);
        }
        assert_eq!(last, TransitionStep::Walk(Dir::Down));

        // No `arrive` call: plain arrival even onto a static door.
        let mut t = Transition::new();
        t.start(&Game, HOUSE, Some(0), -1, -1);
        let mut n = 0;
        loop {
            let s = t.update(&mut f, false);
            assert!(!matches!(s, TransitionStep::Walk(_)));
            n += 1;
            if s == TransitionStep::Done {
                break;
            }
        }
        assert_eq!(n, 33);
    }

    #[test]
    fn a_blocked_exit_walk_still_finishes() {
        let mut t = Transition::new();
        t.start(&Game, HOUSE, Some(0), -1, -1);
        // The walk never starts moving (e.g. blocked): the next frame is Done.
        let steps = run(&mut t, &mut Fader::new(), 0);
        let n = steps.len();
        assert_eq!(steps[n - 2].0, TransitionStep::Walk(Dir::Down));
        assert_eq!(steps[n - 1].0, TransitionStep::Done);
    }

    #[test]
    fn hash_changes_with_progress_and_is_deterministic() {
        let h = |t: &Transition| {
            let mut h = Fnv64::new();
            t.hash(&mut h);
            h.finish()
        };
        let mut a = Transition::new();
        let idle = h(&a);
        a.start(&Game, HOUSE, Some(0), -1, -1);
        let started = h(&a);
        assert_ne!(idle, started);
        let mut b = Transition::new();
        b.start(&Game, HOUSE, Some(0), -1, -1);
        assert_eq!(started, h(&b));
        let mut f = Fader::new();
        for _ in 0..17 {
            a.update(&mut f, false);
        }
        assert_ne!(h(&a), started);
    }
}
