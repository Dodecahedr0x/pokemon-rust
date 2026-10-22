//! Move effects beyond `Hit` / `StatUp` / `StatDown` / `Recoil`, statuses, abilities and
//! catching. **Task E2.**
//!
//! Each effect follows its `BattleScript_Effect*` (`data/battle_scripts_1.s`) and
//! `SetMoveEffect` / `Cmd_seteffect*` (`battle_script_commands.c`) in Gen3 order, on the core's
//! `pub(crate)` state (`Battle`, `Battler`) with the building blocks of `core/moves.rs`.
//! - [`moves`]: the move scripts (status moves, `*_HIT` secondaries, Absorb, Leech Seed, Bide,
//!   Focus Energy, Nature Power; Task G: Attract, Mean Look, the sports, Teleport, Uproar,
//!   Yawn, Double Kick, Foresight, Fury Cutter, Pursuit and its switch-out hit).
//! - [`status`]: `SetMoveEffect` for statuses, confusion and flinch; `attackcanceler`
//!   (`AtkCanceler_UnableToUseMove`); `DoBattlerEndTurnEffects`.
//! - [`abilities`]: `moveend`'s Synchronize, contact abilities (Cute Charm included) and
//!   defrost; Intimidate and Trace on switch-in; Keen Eye; Soundproof; Shed Skin and Truant at
//!   the end of the turn.
//! - [`run_block`]: `IsRunningFromBattleImpossible` (Run Away, Magnet Pull, Mean Look).
//! - [`ball`]: `Cmd_handleballthrow` for a Poké Ball.
//!
//! # Not modelled
//! - Abilities outside `mon::Ability` (`Ability::Other`): Immunity, Limber, Own Tempo, White
//!   Smoke, Hyper Cutter, Water Veil, Wonder Guard, Levitate … act as no ability. Pickup,
//!   Rain Dish, Swift Swim and Chlorophyll have no effect (no items after battle, no
//!   weather). Sturdy only matters against OHKO moves, which the engine does not have yet
//!   (G2). Shed Skin's message leaves the status name empty (Gen3 buffers a
//!   `gStatusConditionString_*Jpn` string the content does not extract).
//! - Substitute, Safeguard, Mist, Minimize (Astonish never doubles), Destiny Bond, held items
//!   (Focus Band, King's Rock, Quick Claw), the `pause` commands.
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
#[cfg(test)]
mod tests_g;

use super::core::{Action, Battle, Env, HitResult, MoveUse, msg};
use super::{Buff, Msg, Side, Str};
use crate::mon::{Ability, BattleTables, MoveEffect, Stat, Type};
pub(crate) use abilities::opposite_genders;

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

/// `attackcanceler`: the status checks in `AtkCanceler_UnableToUseMove` order (sleep, freeze,
/// Truant, flinch, confusion, paralysis, infatuation, Bide), then the target's Soundproof
/// (`ABILITYEFFECT_MOVES_BLOCK`).
pub(crate) fn before_move(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    match status::canceler(b, env, u) {
        Canceler::Use => abilities::moves_block(b, env, u),
        c => c,
    }
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

/// The action of a battler locked into a move (`STATUS2_MULTIPLETURNS`: Bide's two turns,
/// Uproar's `gLockedMoves`), so it is not asked (`HandleTurnActionSelectionState` skips it).
/// `None` when free.
pub(crate) fn locked_action(b: &Battle, t: &BattleTables, side: Side) -> Option<Action> {
    let m = b.battler(side);
    let effect = if m.bide_turns > 0 {
        MoveEffect::Bide
    } else if m.v.uproar > 0 {
        MoveEffect::Uproar
    } else {
        return None;
    };
    if m.mon.hp == 0 {
        return None;
    }
    let slot =
        (0..4).find(|&i| m.mon.moves[i].is_some_and(|mv| t.move_info(mv).effect == effect))?;
    Some(Action::Move(slot as u8))
}

/// Why `IsRunningFromBattleImpossible` refuses `side` (no items, no Shadow Tag / Arena Trap /
/// Wrap / Ingrain in the engine).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RunBlock {
    /// `BATTLE_RUN_FORBIDDEN`: escape prevention (`B_MSG_CANT_ESCAPE`) or the first battle
    /// (`B_MSG_DONT_LEAVE_BIRCH`).
    Forbidden(Str),
    /// `BATTLE_RUN_FAILURE`: the foe's Magnet Pull holds a Steel battler
    /// (`B_MSG_PREVENTS_ESCAPE`).
    Ability { owner: Side, ability: Ability },
}

impl RunBlock {
    /// The `gNoEscapeStringIds` message about `side`.
    pub fn msg(self, side: Side) -> Msg {
        match self {
            RunBlock::Forbidden(id) => msg(id, side, side, side),
            RunBlock::Ability { owner, ability } => Msg {
                buffs: [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(ability)),
                ],
                ..msg(Str::PreventsEscape, side, side, owner)
            },
        }
    }
}

/// Gen3 `IsRunningFromBattleImpossible` for `side`: `None` when it may run.
pub(crate) fn run_block(b: &Battle, side: Side) -> Option<RunBlock> {
    let (me, foe) = (b.battler(side), b.battler(side.other()));
    if me.ability == Ability::RunAway {
        return None;
    }
    if foe.ability == Ability::MagnetPull && me.types.contains(&Type::Steel) {
        return Some(RunBlock::Ability {
            owner: side.other(),
            ability: Ability::MagnetPull,
        });
    }
    if me.v.escape_prevented {
        return Some(RunBlock::Forbidden(Str::CantEscape));
    }
    if b.is_first_battle() {
        return Some(RunBlock::Forbidden(Str::DontLeaveBirch));
    }
    None
}

/// `HandleTurnActionSelectionState`, `B_ACTION_SWITCH`: whether `side` may not switch
/// (`PARTY_ACTION_CANT_SWITCH` / `PARTY_ACTION_ABILITY_PREVENTS`). Unlike running, Run Away
/// does not help, and the first battle does not forbid it.
pub(crate) fn switch_block(b: &Battle, side: Side) -> bool {
    let (me, foe) = (b.battler(side), b.battler(side.other()));
    me.v.escape_prevented || (foe.ability == Ability::MagnetPull && me.types.contains(&Type::Steel))
}

/// `jumpifnopursuitswitchdmg` + `BattleScript_PursuitDmgOnSwitchOut` when `side` switches out
/// and the foe chose Pursuit: the foe's action is cancelled and Pursuit hits first at double
/// damage. True when it struck (the core then gives the exp and finishes the switch).
pub(crate) fn pursuit_on_switch(b: &mut Battle, env: &mut Env, side: Side) -> bool {
    moves::pursuit_on_switch(b, env, side)
}
