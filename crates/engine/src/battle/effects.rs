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
//! - [`abilities`]: `moveend`'s Synchronize, contact abilities (Cute Charm, Rough Skin, Color
//!   Change included), the status-curing abilities and defrost; the absorbing abilities, Wonder
//!   Guard, Forecast; Intimidate and Trace on switch-in; Keen Eye, Hyper Cutter, Clear Body,
//!   White Smoke; Soundproof; Shed Skin and Truant at the end of the turn.
//! - [`run_block`]: `IsRunningFromBattleImpossible` (Smoke Ball, Run Away, Magnet Pull, Mean
//!   Look, Shadow Tag, Arena Trap).
//! - [`ball`]: `Cmd_handleballthrow` for a Poké Ball.
//! - [`items`]: the held items' switch-in, end-of-turn and `moveend` effects (berries, herbs,
//!   Leftovers, King's Rock, Shell Bell, Amulet Coin).
//! - [`later`] (Silice): the weather, its abilities, and the post-Gen3 abilities (Mold Breaker,
//!   Magic Bounce, Prankster, Lightning Rod, Steadfast, Speed Boost, Solar Power …).
//!
//! # Not modelled
//! - Pickup has no effect (no item gain after battle yet); Sturdy only matters against OHKO
//!   moves. Shed Skin's, Lum Berry's, Mental Herb's and the curing abilities' messages leave
//!   the status name empty, a flavour berry's message its flavour (Gen3 buffers
//!   `gStatusConditionString_*Jpn` / `gPokeblockWasTooXStringTable` strings the content does
//!   not extract). Forecast's form sprite has no event (only the type changes). Two-ability
//!   messages (`STRINGID_PKMNSXPREVENTSYSZ`) name one ability. Damp, Sticky Hold and Suction
//!   Cups are helpers for Explosion / Self-Destruct, Thief / Covet / Trick and Roar / Whirlwind.
//!   The absorbing abilities (`ABILITYEFFECT_ABSORBING`) act in `Battle::hit` and the status
//!   moves only; Wonder Guard in `Battle::deal`.
//! - The `pause` commands; the substitute doll and Transform's new sprite have no event.
//!   Items: the Choice Band's move lock; the held-item animation (`B_ANIM_HELD_ITEM_EFFECT`)
//!   has no event; `HandleFaintedMonActions`' item check
//!   (`ItemBattleEffects(ITEMEFFECT_NORMAL, 0, TRUE)` after a faint) is skipped.
//! - `statusanimation` / `status2animation` (no event; `StatusIcon` updates the healthbox).
//! - The toxic counter is not reset on a switch.
//! - Catching: `STRINGID_PLAYERUSEDITEM` (no item names), the Pokédex and nickname prompts, the
//!   PC (a mon caught with a full party is dropped, see `Battle::caught_without_room`).

mod abilities;
mod ball;
mod items;
mod later;
mod mech;
mod moves;
mod status;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_g;
#[cfg(test)]
mod tests_s;
#[cfg(test)]
mod tests_w;
#[cfg(test)]
mod tests_wc;
#[cfg(test)]
mod tests_wd;

use super::core::{Action, Battle, Env, HitResult, MoveUse, msg};
use super::{BattleEvent, Buff, Msg, Side, Str};
use crate::mon::{Ability, BattleTables, HoldEffect, MoveEffect, Stat, Type};
pub(crate) use abilities::opposite_genders;
// For Explosion / Self-Destruct, Thief / Covet / Trick and Roar / Whirlwind (move effects).
#[allow(unused_imports)]
pub(crate) use abilities::{damp, sticky_hold, suction_cups};
pub(crate) use later::{break_mold, prankster_boost};
pub(crate) use mech::{
    BattlerFx, FieldFx, all_moves_unusable, baton_pass_in, before_turn, clear_bonds, crit_bonus,
    damage_extra, destiny_bond, encored_slot, endured, hold_on, move_accuracy, protected,
    reads_first_turn, record_damage, selection_block, slot_limited, spikes, sub_blocks_secondary,
    sub_takes, subbed, sure_accuracy, wish_perish, wish_perish_pending,
};

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
/// abilities and the status-curing abilities, then the items (cure berries, herbs, King's
/// Rock, Shell Bell). `hit` is `None` for a miss or a status move. Wave C: `MOVEEND_RAGE`
/// first, `MOVEEND_MIRROR_MOVE` last; a calling move's `moveend` runs for the move it called.
pub(crate) fn after_move(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    let called = mech::called_use(b, env, u);
    let u = called.as_ref().unwrap_or(u);
    mech::move_end_first(b, env, u);
    abilities::move_end(b, env, u, hit);
    let last = b.multi_last.take();
    items::move_end(b, env, u, hit.or(last));
    mech::move_end_last(b, u);
}

/// `attackcanceler`: the status checks in `AtkCanceler_UnableToUseMove` order (sleep, freeze,
/// Truant, flinch, confusion, paralysis, infatuation, Bide), then the target's Soundproof
/// (`ABILITYEFFECT_MOVES_BLOCK`).
/// Wave C: a locked (Rollout, Thrash, the two-turn moves) or encored battler uses that move
/// (`HandleAction_UseMove`), which then runs in the canceler's place.
pub(crate) fn before_move(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    let rerouted = mech::reroute(b, env, u);
    let uu = rerouted.as_ref().unwrap_or(u);
    mech::start_move(b, uu);
    let c = match status::canceler(b, env, uu) {
        Canceler::Use => match later::before_move(b, env, uu) {
            Some(c) => c,
            None => abilities::moves_block(b, env, uu),
        },
        c => c,
    };
    // Magic Coat, then Snatch (`Cmd_attackcanceler`, after `ABILITYEFFECT_MOVES_BLOCK`).
    if let Canceler::Use = c
        && let Some((back, hit)) = mech::bounce(b, env, uu).or_else(|| mech::steal(b, env, uu))
    {
        return Canceler::Done(back, hit);
    }
    // `lightningRodRedirected`: `BattleScript_TookAttack` (the attack string, then the
    // ability's message), before the protection check.
    if core::mem::take(&mut b.fx.rod)
        && let Canceler::Use = c
    {
        b.attack_string(env, uu);
        let t = uu.target;
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(b.battler(t).ability)),
            ],
            ..uu.msg_long(Str::PkmnsXTookAttack, t)
        }));
        let printed = MoveUse {
            printed: true,
            ..*uu
        };
        return Canceler::Done(printed, b.execute(env, &printed));
    }
    match c {
        Canceler::Use if rerouted.is_some() => Canceler::Done(*uu, b.execute(env, uu)),
        Canceler::Skip => {
            mech::skipped(b);
            Canceler::Skip
        }
        c => c,
    }
}

/// `DoBattlerEndTurnEffects` for each battler in turn order: Leech Seed, poison, toxic, burn.
/// Wave C: `TurnValuesCleanUp`, the side timers, Wish. Future Sight and Perish Song come after
/// the faints are handled ([`wish_perish`], `Step::WishPerish`).
pub(crate) fn end_turn(b: &mut Battle, env: &mut Env) {
    mech::turn_passed(b);
    mech::field_end_turn(b, env);
    later::field_end_turn(b, env);
    status::end_turn(b, env);
    mech::turn_clean_up(b);
}

/// `ABILITYEFFECT_ON_SWITCH_IN` of `side`'s battler: the weather abilities, Mold Breaker,
/// Forecast (its own; Cloud Nine and Air Lock update the first Forecast battler that changes).
pub(crate) fn on_switch_in(b: &mut Battle, env: &mut Env, side: Side) {
    later::on_switch_in(b, env, side);
    match b.battler(side).ability {
        Ability::Forecast => {
            abilities::forecast(b, env, side);
        }
        Ability::CloudNine | Ability::AirLock if b.battler(side).mon.hp > 0 => {
            abilities::forecast_all(b, env);
        }
        _ => {}
    }
}

/// `ITEMEFFECT_ON_SWITCH_IN` of `side`'s battler: Amulet Coin, White Herb.
pub(crate) fn items_on_switch_in(b: &mut Battle, env: &mut Env, side: Side) {
    items::on_switch_in(b, env, side);
}

/// `HandleFaintedMonActions` case 6 after an action with no faint: `ABILITYEFFECT_FORECAST`.
pub(crate) fn after_action(b: &mut Battle, env: &mut Env) {
    abilities::forecast_all(b, env);
}

/// `ABILITYEFFECT_ABSORBING` once a move hit: true when the target's ability took it.
pub(crate) fn absorbs(b: &mut Battle, env: &mut Env, u: &MoveUse) -> bool {
    abilities::absorbs(b, env, u)
}

/// `Cmd_typecalc`'s Wonder Guard: true when the move "missed", with the message.
pub(crate) fn wonder_guard(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    eff: crate::battle::Effectiveness,
) -> bool {
    abilities::wonder_guard(b, env, u, eff)
}

/// A battler with `ability` left the field: a primal weather may end.
pub(crate) fn left_field(b: &mut Battle, env: &mut Env, ability: Ability) {
    later::left_field(b, env, ability);
}

/// `TryDoEventsBeforeFirstTurn`'s switch-in abilities: `ABILITYEFFECT_INTIMIDATE1` then
/// `ABILITYEFFECT_TRACE`, each scanning the battlers in index order (player first).
pub(crate) fn first_turn(b: &mut Battle, env: &mut Env) {
    for side in b.sides() {
        abilities::intimidate(b, env, side);
    }
    for side in b.sides() {
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
    mech::mist_blocks(b, env, u, side, true) || abilities::blocks_stat_drop(b, env, u, side, stat)
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
    if let Some(a) = mech::locked_action(b, side) {
        return Some(a);
    }
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

/// Why `IsRunningFromBattleImpossible` refuses `side` (no Wrap / Ingrain in the engine).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RunBlock {
    /// `BATTLE_RUN_FORBIDDEN`: escape prevention (`B_MSG_CANT_ESCAPE`) or the first battle
    /// (`B_MSG_DONT_LEAVE_BIRCH`).
    Forbidden(Str),
    /// `BATTLE_RUN_FAILURE`: the foe's Shadow Tag, Arena Trap (a grounded battler) or Magnet
    /// Pull (a Steel battler) holds it (`B_MSG_PREVENTS_ESCAPE`).
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
    // A Smoke Ball (`HOLD_EFFECT_CAN_ALWAYS_RUN`) first, then Run Away.
    if me.x.hold == Some(HoldEffect::CanAlwaysRun) || me.ability == Ability::RunAway {
        return None;
    }
    if shadow_tagged(b, side) || arena_trapped(b, side) {
        return Some(RunBlock::Ability {
            owner: side.other(),
            ability: foe.ability,
        });
    }
    if foe.ability == Ability::MagnetPull && me.types.contains(&Type::Steel) {
        return Some(RunBlock::Ability {
            owner: side.other(),
            ability: Ability::MagnetPull,
        });
    }
    if me.v.escape_prevented || mech::trapped(b, side) {
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
    me.v.escape_prevented
        || mech::trapped(b, side)
        || (foe.ability == Ability::MagnetPull && me.types.contains(&Type::Steel))
        || shadow_tagged(b, side)
        || arena_trapped(b, side)
}

/// The foe's Arena Trap holds `side` unless it is Flying or Levitates.
pub(crate) fn arena_trapped(b: &Battle, side: Side) -> bool {
    let me = b.battler(side);
    b.battler(side.other()).ability == Ability::ArenaTrap
        && !me.types.contains(&Type::Flying)
        && me.ability != Ability::Levitate
}

/// The foe's Shadow Tag traps `side` (Gen3: always; Gen4: unless it has Shadow Tag too).
pub(crate) fn shadow_tagged(b: &Battle, side: Side) -> bool {
    match b.battler(side.other()).ability {
        Ability::ShadowTag => true,
        Ability::ShadowTagGen4 => b.battler(side).ability != Ability::ShadowTagGen4,
        _ => false,
    }
}

/// `jumpifnopursuitswitchdmg` + `BattleScript_PursuitDmgOnSwitchOut` when `side` switches out
/// and the foe chose Pursuit: the foe's action is cancelled and Pursuit hits first at double
/// damage. True when it struck (the core then gives the exp and finishes the switch).
pub(crate) fn pursuit_on_switch(b: &mut Battle, env: &mut Env, side: Side) -> bool {
    moves::pursuit_on_switch(b, env, side)
}
