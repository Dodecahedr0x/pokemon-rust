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
//!
//! Running does not apply (`B` only changes keypad steps). Gen3's `PLAYER_AVATAR_FLAG_CONTROLLABLE`
//! ([`super::Objects::hold`]) holds forced movement off after a warp load until the player
//! first moves.
//!
//! Differences from Gen3:
//! - `Ice` slips the way the player faces; Gen3 uses `movementDirection`, which differs only
//!   after a `Slide` (facing kept) lands on ice;
//! - no bikes: the muddy slope always slides (Gen3 lets the Mach Bike at top speed climb it),
//!   and the muddy slope's metatile animation (`Task_MuddySlope`) is not drawn.

use super::ActionKind;
use crate::data::{Behavior, Dir};
use crate::script::Speed;

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
        _ => return None,
    };
    Some(ForcedStep {
        dir,
        kind: ActionKind::Walk(speed),
        keep_facing,
        pause_anim,
    })
}
