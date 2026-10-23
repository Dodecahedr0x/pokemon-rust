//! Friendship (Gen3 `AdjustFriendship`, `sFriendshipEventModifiers`; Milestone 9 Task E owns
//! the file).
//!
//! Wave 0 adds the hooks at Gen3's call sites, all no-ops until Task E fills them:
//! - [`on_step`], once per finished player step that started no coord or warp script, before
//!   the repel counter and the wild encounter (Gen3 `TryStartStepBasedScript` →
//!   `TryStartStepCountScript` → `UpdateFriendshipStepCounter`, `field_control_avatar.c`);
//! - [`on_event`] on a level-up (`battle/core.rs` exp gain, Gen3 `Cmd_getexp`) and on a faint
//!   (`battle/core/moves.rs` `try_faint`, Gen3 `AdjustFriendshipOnBattleFaint`); Tasks B and BM
//!   call it for vitamins, TMs and battle items.
//!
//! [`on_event`] takes the mon's hold effect (Soothe Bell) instead of the game: the battle core,
//! one of its callers, has no `GameData`, and keeps each party mon's hold effect already.

use super::State;
use crate::data::GameData;
use crate::mon::{BattleTables, HoldEffect, Pokemon};
use crate::rng::Rng;

/// Gen3 `FRIENDSHIP_EVENT_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(
    dead_code,
    reason = "Milestone 9 Wave 0: Tasks E, B and BM raise the other events"
)]
pub enum FriendshipEvent {
    GrowLevel,
    Vitamin,
    BattleItem,
    LeagueBattle,
    LearnTmHm,
    Walking,
    FaintSmall,
    FaintFieldPsn,
    FaintLarge,
}

impl FriendshipEvent {
    /// Gen3 `AdjustFriendshipOnBattleFaint` (singles): `FaintLarge` when the opponent is 30 or
    /// more levels above the fainted mon, else `FaintSmall`.
    pub const fn battle_faint(foe_level: u8, level: u8) -> FriendshipEvent {
        if foe_level > level && foe_level - level > 29 {
            FriendshipEvent::FaintLarge
        } else {
            FriendshipEvent::FaintSmall
        }
    }
}

/// Gen3 `AdjustFriendship(mon, event)` (Task E). Wave 0: nothing.
pub(crate) fn on_event(
    mon: &mut Pokemon,
    event: FriendshipEvent,
    hold: Option<HoldEffect>,
    t: &BattleTables,
    rng: &mut Rng,
) {
    let _ = (mon, event, hold, t, rng);
}

/// Once per finished player step, before the step triggers (Task E: the walking counter,
/// `VAR_FRIENDSHIP_STEP_COUNTER`, every 128 steps). Wave 0: nothing.
pub(crate) fn on_step(s: &mut State, game: &impl GameData) {
    let _ = (s, game);
}
