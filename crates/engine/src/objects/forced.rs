//! Forced movement: what ice, arrows, slides and muddy slopes do to the player (Gen3
//! `TryDoMetatileBehaviorForcedMovement`, `sForcedMovementFuncs`, `field_player_avatar.c`).
//!
//! Gen3 runs it at the start of `PlayerStep`, on every frame of player control, before the
//! keypad: when the player's cell (the destination of a step in progress) is a forced-movement
//! cell, it tests the collision one cell on (`CheckForPlayerAvatarCollision`):
//! - free (or a ledge, jumped): the player is `MOVING` and the forced step starts as soon as no
//!   step plays (`PlayerSetAnimId` ignores it until then), so each forced step follows the last
//!   one with no frame between, as walking does; the keypad is not read;
//! - blocked: the forced movement ends (`ForcedMovement_None`) and the keypad moves the player
//!   as on plain floor, so the player stopped on ice can walk off it.
//!
//! The steps (Gen3 `DoForcedMovement`):
//!
//! | Behavior | Step | Direction | Facing | Walk animation |
//! |---|---|---|---|---|
//! | `Ice` (`ForcedMovement_Slip`) | `walk_fast` (8 frames) | the player's | turns | paused (`disableAnim`) |
//! | `ForcedWalk(d)` (`ForcedMovement_Walk*`) | `walk_normal` (16) | `d` | turns | plays |
//! | `Slide(d)` (`ForcedMovement_Slide*`) | `walk_fast` | `d` | kept (`facingDirectionLocked`) | paused |
//! | `MuddySlope` (`ForcedMovement_MuddySlope`) | `walk_fast` | south | kept | plays |
//! | `Water(Current(d))` (`ForcedMovement_Pushed*ByCurrent`) | `walk_fast` (Gen3's is 6 frames) | `d` | turns | plays |
//!
//! Running does not apply (`B` only changes keypad steps). Gen3's `PLAYER_AVATAR_FLAG_CONTROLLABLE`
//! ([`ForcedState::Held`]) holds forced movement off after a warp load until the player
//! first moves.
//!
//! Gen3's `T_TILE_CENTER` ([`ForcedState::Centered`]) is the frame after a step ended, until
//! the next `PlayerStep` clears the finished step; on a forced cell `FieldGetPlayerInput` reads
//! no input on it. A bump (walk in place) is not a step: pushing into an obstacle from a forced
//! cell reads input (Gen3 `T_NOT_MOVING`), so A talks and doors open.
//!
//! Differences from Gen3:
//! - `Ice` slips the way the player faces; Gen3 uses `movementDirection`, which differs only
//!   after a `Slide` (facing kept) lands on ice;
//! - no bikes: the muddy slope always slides (Gen3 lets the Mach Bike at top speed climb it),
//!   and the muddy slope's metatile animation (`Task_MuddySlope`) is not drawn.

use super::ActionKind;
use crate::data::{Behavior, Dir, Water};
use crate::script::Speed;

/// The player's forced-movement state beside its run state. Both are set only on a forced
/// cell, so they are hashed and snapshotted only when set (`Objects::hash`, the appended
/// `RunStateSnap` variants), and never together: a hold lifts when the player's run state is
/// `MOVING`, which every step the player takes from a hold starts with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ForcedState {
    Free,
    /// Gen3 `PLAYER_AVATAR_FLAG_CONTROLLABLE`: forced movement is held off until the run state
    /// is `MOVING` (`PlayerAllowForcedMovementIfMovingSameDirection`). Gen3 sets it on every
    /// warp load (`InitPlayerAvatar`); here [`super::Objects::hold_forced`] sets it only when
    /// the player arrives on a forced-movement cell, so it is never set on maps without one.
    /// Never set while the run state is `MOVING`.
    Held,
    /// Gen3 `T_TILE_CENTER` on a forced cell: a player step ended there on the last frame
    /// `update_player` ran, and none started since.
    Centered,
}

impl ForcedState {
    /// The next `PlayerStep` (or anything that moves the player otherwise) ends the tile
    /// center.
    pub(super) fn end_center(&mut self) {
        if *self == ForcedState::Centered {
            *self = ForcedState::Free;
        }
    }
}

/// One forced step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct ForcedStep {
    pub dir: Dir,
    pub kind: ActionKind,
    /// The facing stays (Gen3 `facingDirectionLocked`).
    pub keep_facing: bool,
    /// The walk animation is paused (Gen3 `disableAnim`).
    pub pause_anim: bool,
}

/// The forced step a cell of behavior `b` gives a player facing `facing`, if any (Gen3
/// `GetForcedMovementByMetatileBehavior`; `CrackedFloor` is a forced-movement tile without a
/// movement).
pub(super) const fn forced_step(b: Behavior, facing: Dir) -> Option<ForcedStep> {
    let (dir, speed, keep_facing, pause_anim) = match b {
        Behavior::Ice => (facing, Speed::Fast, false, true),
        Behavior::ForcedWalk(d) => (d, Speed::Normal, false, false),
        Behavior::Slide(d) => (d, Speed::Fast, true, true),
        Behavior::MuddySlope => (Dir::Down, Speed::Fast, true, false),
        // Gen3 `MOVE_SPEED_FAST_2` (6 frames); the engine's nearest step is `walk_fast` (8).
        Behavior::Water(Water::Current(d)) => (d, Speed::Fast, false, false),
        _ => return None,
    };
    Some(ForcedStep {
        dir,
        kind: ActionKind::Walk(speed),
        keep_facing,
        pause_anim,
    })
}
