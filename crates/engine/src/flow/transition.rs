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
//! - `Loading`: nothing; the loaded map is still uploading over frames (see
//!   [`Transition::update_loading`]) and the screen stays black.
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
    /// Start the player jumping one cell `Dir` (a [`Behavior::PopOutWarp`] arrival).
    Jump(Dir),
    Walking,
    FadingOut,
    Load(MapId, i16, i16),
    /// The loaded map is still uploading; the screen stays black.
    Loading,
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
    /// The destination is a door, stairs or an escalator: the player walks out one cell
    /// `facing` (hidden until then, but on an escalator).
    exit_walk: bool,
    player_hidden: bool,
    /// The exit is a one-cell jump ([`Behavior::PopOutWarp`]); set only from the arrival to the
    /// end of the jump, so an idle transition never has it (not snapshotted).
    exit_jump: bool,
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
            exit_jump: false,
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
    /// (Gen3 `GetAdjustedInitialDirection`, `SetUpWarpExitTask`). `warped_facing` is the
    /// player's facing when the warp started (Gen3 `StoreInitialPlayerAvatarState`).
    /// - `Door` (animated door, `Task_ExitDoor`): face down, hidden during the fade-in, then walk
    ///   one cell down.
    /// - `NonAnimDoor(d)` (stairs, `MB_DEEP_SOUTH_WARP` exits, `Task_ExitNonAnimDoor`): face
    ///   `d`, hidden during the fade-in, then walk one cell `d`.
    /// - `Escalator(d)` (`Task_EscalatorWarpIn`, without the ride): face `d`, then walk one
    ///   cell `d`, visible.
    /// - `Ladder`: keep `warped_facing`, no exit walk.
    /// - `ArrowWarp(d)`: face away from `d`, no exit walk (`Task_ExitNonDoor`).
    /// - `PopOutWarp(d)` (`FieldCB_LavaridgeGymB1FWarpExit`): face `d`, hidden during the
    ///   fade-in, then jump one cell `d`.
    /// - anything else (`Warp` included): face down, no exit walk.
    ///
    /// - the water door (`MB_WATER_DOOR`, a non-animated door): as `NonAnimDoor(Down)`.
    ///
    /// Without this call the arrival is a plain one (face down, no exit walk).
    pub fn arrive(&mut self, behavior: Behavior, warped_facing: Dir) -> Dir {
        self.arrive_swapping(behavior, warped_facing, false)
    }

    /// [`arrive`](Self::arrive) where `swapped` says the player went from surfing to
    /// underwater or back (a dive): then the facing is kept wherever no door or arrow warp
    /// decides it (Gen3 `GetAdjustedInitialDirection`'s transition-flags case).
    pub fn arrive_swapping(
        &mut self,
        behavior: Behavior,
        warped_facing: Dir,
        swapped: bool,
    ) -> Dir {
        let (facing, walk, hidden) = match behavior {
            Behavior::Door => (Dir::Down, true, true),
            Behavior::NonAnimDoor(d) => (d, true, true),
            Behavior::Water(crate::data::Water::Door) => (Dir::Down, true, true),
            Behavior::Escalator(d) => (d, true, false),
            Behavior::Ladder => (warped_facing, false, false),
            Behavior::ArrowWarp(d) => (d.opposite(), false, false),
            Behavior::Water(crate::data::Water::SouthArrowWarp) => (Dir::Up, false, false),
            Behavior::PopOutWarp(d) => (d, true, true),
            _ if swapped => (warped_facing, false, false),
            _ => (Dir::Down, false, false),
        };
        self.exit_jump = matches!(behavior, Behavior::PopOutWarp(_));
        self.facing = facing;
        self.exit_walk = walk;
        self.player_hidden = hidden;
        facing
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
    /// during a door or non-animated door exit fade-in).
    pub const fn player_hidden(&self) -> bool {
        self.player_hidden
    }

    /// Runs one frame. `player_moving` is whether the player is still in the middle of a step.
    pub fn update(&mut self, fader: &mut Fader, player_moving: bool) -> TransitionStep {
        self.update_loading(fader, player_moving, false)
    }

    /// [`Transition::update`] while the loaded map may still be uploading (`loading`, Milestone 8
    /// Task F): the fade-in waits on the black screen until it is done ([`TransitionStep::Loading`]),
    /// as Gen3 `LoadMapInStepsLocal` finishes before `FieldCB_DefaultWarpExit` fades in.
    pub fn update_loading(
        &mut self,
        fader: &mut Fader,
        player_moving: bool,
        loading: bool,
    ) -> TransitionStep {
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
                Stage::FadeIn(false) if loading => return TransitionStep::Loading,
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
                        if self.exit_jump {
                            return TransitionStep::Jump(self.facing);
                        }
                        return TransitionStep::Walk(self.facing);
                    }
                    self.stage = Stage::Idle;
                    return TransitionStep::Done;
                }
                Stage::WalkOut => {
                    if player_moving {
                        return TransitionStep::Walking;
                    }
                    self.exit_jump = false;
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
        // Wave E2: only during a pop-out arrival, so other hashes are unchanged.
        if self.exit_jump {
            h.write(&[0xE7]);
        }
    }
}

// Milestone 5 Task B.
impl Transition {
    /// The leftover fields of an idle transition (they stay hashed). The stage is not kept:
    /// only idle transitions are captured.
    pub fn snapshot(&self) -> crate::checkpoint::TransitionSnap {
        debug_assert!(!self.is_active(), "only an idle transition is captured");
        crate::checkpoint::TransitionSnap {
            map: self.map,
            x: self.x,
            y: self.y,
            facing: self.facing,
            exit_walk: self.exit_walk,
            player_hidden: self.player_hidden,
        }
    }

    /// An idle transition with these leftover fields.
    pub fn from_snapshot(s: &crate::checkpoint::TransitionSnap) -> Transition {
        Transition {
            stage: Stage::Idle,
            map: s.map,
            x: s.x,
            y: s.y,
            facing: s.facing,
            exit_walk: s.exit_walk,
            player_hidden: s.player_hidden,
            exit_jump: false,
        }
    }
}

#[cfg(test)]
mod tests;
