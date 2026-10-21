//! Move effects beyond `Hit` / `StatUp` / `StatDown` / `Recoil`, statuses and abilities.
//!
//! Task D ships these seams as stubs; **Task E2** (Wave 2) fills them in, following each
//! `BattleScript_Effect*` and `Cmd_seteffect*` in Gen3 order. They work on the core's
//! `pub(crate)` state (`Battle`, `Battler`) with the building blocks of `core/moves.rs`:
//! `accuracy`, `attack_string`, `missed`, `fail`, `damage` / `deal` / `hit` (with a
//! [`Secondary`](super::core::Secondary) for `*_HIT` effects), `secondary`, `change_hp`,
//! `change_stage` / `stat_msg`, `try_faint`, `move_use` / `execute` (Nature Power) and
//! `unsupported`.

use super::Side;
use super::core::{Battle, Env, HitResult, MoveUse};
use crate::mon::Stat;

/// Runs an effect the core does not handle itself; the hit's result when it dealt one. Stub:
/// the `Unsupported` handling (a plain hit when the move has power, else "But it failed!").
pub(crate) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.unsupported(env, u)
}

/// After the move's script (`moveend`): contact abilities (Static, Flame Body, Poison Point,
/// Effect Spore, Rough Skin…) on a `hit`, and Synchronize for a status the move inflicted.
/// `hit` is `None` for a miss or a status move. Stub: nothing.
pub(crate) fn after_move(_b: &mut Battle, _env: &mut Env, _u: &MoveUse, _hit: Option<HitResult>) {}

/// `attackcanceler`'s status checks (sleep, freeze, flinch, confusion, paralysis, Bide's
/// turns), before the move's script. False when the move is not used. Stub: always used.
pub(crate) fn before_move(_b: &mut Battle, _env: &mut Env, _u: &MoveUse) -> bool {
    true
}

/// `DoBattlerEndTurnEffects` (Leech Seed, poison, burn …), before faints are handled. Stub:
/// nothing.
pub(crate) fn end_turn(_b: &mut Battle, _env: &mut Env) {}

/// Switch-in abilities (Intimidate, Trace) of the battler just sent out on `side`, also run in
/// speed order before the first turn. Stub: nothing.
pub(crate) fn switch_in(_b: &mut Battle, _env: &mut Env, _side: Side) {}

/// `ChangeStatBuffs`' refusals for a drop on `side` (Keen Eye for accuracy, Shield Dust for
/// secondary drops …); true when blocked, after printing its own message. Stub: never.
pub(crate) fn blocks_stat_drop(
    _b: &mut Battle,
    _env: &mut Env,
    _u: &MoveUse,
    _side: Side,
    _stat: Stat,
) -> bool {
    false
}

/// The player's Poké Ball action (`Cmd_handleballthrow`, `calc::catch_shakes`), wild battles
/// only: the core refuses `Choice::Ball` in a trainer battle (E2 may instead play
/// `BattleScript_TrainerBallBlock`). Stub: nothing.
pub(crate) fn ball(_b: &mut Battle, _env: &mut Env) {}
