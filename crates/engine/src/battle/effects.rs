//! Move effects beyond `Hit` / `StatUp` / `StatDown` / `Recoil`, statuses, abilities and
//! catching. **Task E2.**
//!
//! Each effect follows its `BattleScript_Effect*` (`data/battle_scripts_1.s`) and
//! `SetMoveEffect` / `Cmd_seteffect*` (`battle_script_commands.c`) in Gen3 order, on the core's
//! `pub(crate)` state (`Battle`, `Battler`) with the building blocks of `core/moves.rs`.
//! - [`moves`]: the move scripts (status moves, `*_HIT` secondaries, Absorb, Leech Seed, Bide,
//!   Focus Energy, Nature Power).
//! - [`status`]: `SetMoveEffect` for statuses, confusion and flinch; `attackcanceler`
//!   (`AtkCanceler_UnableToUseMove`); `DoBattlerEndTurnEffects`.
//! - [`abilities`]: `moveend`'s Synchronize, contact abilities and defrost; Intimidate and
//!   Trace on switch-in; Keen Eye.
//! - [`ball`]: `Cmd_handleballthrow` for a Poké Ball.
//!
//! # Not modelled
//! - Abilities outside `mon::Ability` (`Ability::Other`): Immunity, Limber, Insomnia, Own
//!   Tempo, Inner Focus, Clear Body, Hyper Cutter, Water Veil, Liquid Ooze, Early Bird, Wonder
//!   Guard, Levitate … act as no ability. Pickup, weather abilities and Early Bird have no effect
//!   in the slice.
//! - Substitute, Safeguard, Mist, Uproar, Minimize (Astonish never doubles), Destiny Bond,
//!   held items (Focus Band, King's Rock, Quick Claw), the `pause` commands.
//! - `statusanimation` / `status2animation` (no event; `StatusIcon` updates the healthbox).
//! - The toxic counter is not reset on a switch.
//! - Catching: `STRINGID_PLAYERUSEDITEM` (no item names), the Pokédex and nickname prompts, the
//!   PC (a mon caught with a full party is dropped, see `Battle::caught_without_room`).

mod abilities;
mod ball;
mod moves;
mod status;
#[cfg(test)]
mod tests;

use super::Side;
use super::core::{Action, Battle, Env, HitResult, MoveUse};
use crate::mon::{BattleTables, MoveEffect, Stat};

/// The result of `attackcanceler` (`AtkCanceler_UnableToUseMove`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Canceler {
    /// The move's script runs.
    Use,
    /// The canceler ran a script in its place that still ends in `moveend` (Bide's release:
    /// `BattleScript_BideAttack`), with that script's move use and hit.
    Done(MoveUse, Option<HitResult>),
    /// The move is not used (asleep, frozen, flinched, confused self-hit, fully paralysed,
    /// storing energy).
    Skip,
}

/// Runs an effect the core does not handle itself; the hit's result when it dealt one.
pub(crate) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    moves::apply(b, env, u)
}

/// `moveend`: defrost by a Fire move, Synchronize (target, then attacker) around the contact
/// abilities (Effect Spore, Static, Poison Point). `hit` is `None` for a miss or a status move.
pub(crate) fn after_move(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    abilities::move_end(b, env, u, hit);
}

/// `attackcanceler`'s status checks, in `AtkCanceler_UnableToUseMove` order: sleep, freeze,
/// flinch, confusion, paralysis, Bide.
pub(crate) fn before_move(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    status::canceler(b, env, u)
}

/// `DoBattlerEndTurnEffects` for each battler in turn order: Leech Seed, poison, toxic, burn.
pub(crate) fn end_turn(b: &mut Battle, env: &mut Env) {
    status::end_turn(b, env);
}

/// `TryDoEventsBeforeFirstTurn`'s switch-in abilities: `ABILITYEFFECT_INTIMIDATE1` then
/// `ABILITYEFFECT_TRACE`, each scanning the battlers in index order (player first).
pub(crate) fn first_turn(b: &mut Battle, env: &mut Env) {
    for side in [Side::Player, Side::Opponent] {
        abilities::intimidate(b, env, side);
    }
    for side in [Side::Player, Side::Opponent] {
        abilities::trace(b, env, side);
    }
}

/// Switch-in abilities (Intimidate, then Trace) of the battler just sent out on `side`.
pub(crate) fn switch_in(b: &mut Battle, env: &mut Env, side: Side) {
    abilities::intimidate(b, env, side);
    abilities::trace(b, env, side);
}

/// `ChangeStatBuffs`' refusals for a status move's drop on `side` (`STAT_CHANGE_ALLOW_PTR`):
/// Keen Eye for accuracy, with its message. True when blocked.
pub(crate) fn blocks_stat_drop(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    stat: Stat,
) -> bool {
    abilities::blocks_stat_drop(b, env, u, side, stat)
}

/// The player's Poké Ball (`Cmd_handleballthrow`), wild battles only: the core refuses
/// `Choice::Ball` in a trainer battle.
pub(crate) fn ball(b: &mut Battle, env: &mut Env) {
    ball::throw(b, env);
}

/// The action of a battler locked into a move (`STATUS2_MULTIPLETURNS`: Bide's two turns), so
/// it is not asked (`HandleTurnActionSelectionState` skips it). `None` when free.
pub(crate) fn locked_action(b: &Battle, t: &BattleTables, side: Side) -> Option<Action> {
    let m = b.battler(side);
    if m.bide_turns == 0 || m.mon.hp == 0 {
        return None;
    }
    let slot = (0..4)
        .find(|&i| m.mon.moves[i].is_some_and(|mv| t.move_info(mv).effect == MoveEffect::Bide))?;
    Some(Action::Move(slot as u8))
}
