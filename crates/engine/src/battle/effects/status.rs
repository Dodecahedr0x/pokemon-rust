//! Statuses: `SetMoveEffect`'s status, confusion and flinch branches, `attackcanceler`'s
//! checks (`AtkCanceler_UnableToUseMove`) and `DoBattlerEndTurnEffects`.

use super::Canceler;
use crate::battle::calc;
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, Status, StatusKind, Type};

/// Where a `SetMoveEffect` comes from: `primary` (`seteffectprimary`: the status moves,
/// Synchronize) or not (`seteffectwithchance` / `seteffectsecondary`), and the ability that
/// caused it (`HITMARKER_STATUS_ABILITY_EFFECT`, with its owner = `gBattleScripting.battler`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Cause {
    pub primary: bool,
    pub ability: Option<(Side, Ability)>,
}

impl Cause {
    /// A move's primary effect (`seteffectprimary`).
    pub const PRIMARY: Cause = Cause {
        primary: true,
        ability: None,
    };
    /// A hit's secondary effect (`seteffectwithchance`).
    pub const SECONDARY: Cause = Cause {
        primary: false,
        ability: None,
    };

    /// Shield Dust blocks the non-primary effects up to `MOVE_EFFECT_FLINCH` (id ≤ 9) that no
    /// ability caused.
    fn shield_dust(self, b: &Battle, side: Side) -> bool {
        b.battler(side).ability == Ability::ShieldDust && self.ability.is_none() && !self.primary
    }
}

/// The message of an effect landing on `effect` (`B_EFF_NAME` = `msg.target`); an ability
/// effect names its owner (`B_SCR_ACTIVE_*` = `msg.active`) and the ability (`buffs[2]`).
fn effect_msg(u: &MoveUse, id: Str, effect: Side, cause: Cause) -> Msg {
    let (active, ability) = match cause.ability {
        Some((owner, a)) => (owner, Buff::Text(Str::AbilityName(a))),
        None => (effect, Buff::None),
    };
    Msg {
        mov: Some(u.mov),
        buffs: [Buff::None, Buff::None, ability],
        wait: LONG,
        ..msg(id, u.attacker, effect, active)
    }
}

/// `CancelMultiTurnMoves`: ends Bide.
pub(super) fn cancel_multi_turn(b: &mut Battle, side: Side) {
    b.battler_mut(side).bide_turns = 0;
}

/// `SetMoveEffect` for a primary status on `effect`. True when it was inflicted.
pub(super) fn set_status(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    effect: Side,
    kind: StatusKind,
    cause: Cause,
) -> bool {
    if cause.shield_dust(b, effect) || b.battler(effect).mon.hp == 0 {
        return false;
    }
    let m = b.battler(effect);
    let has = |t: Type| m.types.contains(&t);
    let statused = m.mon.status != Status::None;
    // An ability's primary effect on an immune type says so (B_MSG_STATUS_HAD_NO_EFFECT).
    let no_effect_msg = cause.ability.is_some() && cause.primary;
    let immune = match kind {
        StatusKind::Poison | StatusKind::Toxic => has(Type::Poison) || has(Type::Steel),
        StatusKind::Burn => has(Type::Fire),
        StatusKind::Freeze => has(Type::Ice),
        StatusKind::Sleep | StatusKind::Paralysis => false,
    };
    if immune && no_effect_msg && kind != StatusKind::Freeze {
        env.out.push(BattleEvent::Msg(effect_msg(
            u,
            Str::PkmnSXHadNoEffectOnY,
            effect,
            cause,
        )));
        return false;
    }
    if immune || statused {
        return false;
    }
    if matches!(kind, StatusKind::Sleep | StatusKind::Freeze) {
        cancel_multi_turn(b, effect);
    }
    let status = match kind {
        // STATUS1_SLEEP_TURN((Random() & 3) + 2).
        StatusKind::Sleep => Status::Sleep((env.rng.random() & 3) as u8 + 2),
        StatusKind::Poison => Status::Poison,
        StatusKind::Burn => Status::Burn,
        StatusKind::Freeze => Status::Freeze,
        StatusKind::Paralysis => Status::Paralysis,
        StatusKind::Toxic => Status::Toxic(0),
    };
    b.battler_mut(effect).mon.status = status;
    b.write_back(effect);
    let by_ability = cause.ability.is_some();
    // sMoveEffectBS_Ptrs: gFellAsleepStringIds, gGotPoisonedStringIds, … (statused / by ability).
    let id = match (kind, by_ability) {
        (StatusKind::Sleep, false) => Str::PkmnFellAsleep,
        (StatusKind::Sleep, true) => Str::PkmnMadeSleep,
        (StatusKind::Poison, false) => Str::PkmnWasPoisoned,
        (StatusKind::Poison, true) => Str::PkmnPoisonedBy,
        (StatusKind::Burn, false) => Str::PkmnWasBurned,
        (StatusKind::Burn, true) => Str::PkmnBurnedBy,
        (StatusKind::Freeze, false) => Str::PkmnWasFrozen,
        (StatusKind::Freeze, true) => Str::PkmnFrozenBy,
        (StatusKind::Paralysis, false) => Str::PkmnWasParalyzed,
        (StatusKind::Paralysis, true) => Str::PkmnWasParalyzedBy,
        (StatusKind::Toxic, _) => Str::PkmnBadlyPoisoned,
    };
    env.out
        .push(BattleEvent::Msg(effect_msg(u, id, effect, cause)));
    env.out.push(BattleEvent::StatusIcon {
        side: effect,
        status,
    });
    // HITMARKER_SYNCHRONIZE_EFFECT for poison, toxic, paralysis and burn.
    if matches!(
        kind,
        StatusKind::Poison | StatusKind::Toxic | StatusKind::Paralysis | StatusKind::Burn
    ) {
        b.sync = Some(kind);
    }
    true
}

/// `MOVE_EFFECT_CONFUSION` on `effect`: `Random() % 4 + 2` turns. Nothing when already
/// confused. True when inflicted.
pub(super) fn confuse(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    effect: Side,
    cause: Cause,
) -> bool {
    let m = b.battler(effect);
    if cause.shield_dust(b, effect) || m.mon.hp == 0 || m.confusion > 0 {
        return false;
    }
    b.battler_mut(effect).confusion = (env.rng.random() % 4) as u8 + 2;
    // BattleScript_MoveEffectConfusion: STRINGID_PKMNWASCONFUSED.
    env.out.push(BattleEvent::Msg(effect_msg(
        u,
        Str::PkmnBecameConfused,
        effect,
        cause,
    )));
    true
}

/// `MOVE_EFFECT_FLINCH` on `effect`: only when it has not acted yet this turn
/// (`GetBattlerTurnOrderNum(gEffectBattler) > gCurrentTurnActionNumber`). Silent.
pub(super) fn flinch(b: &mut Battle, effect: Side, cause: Cause) {
    if cause.shield_dust(b, effect) || b.battler(effect).mon.hp == 0 {
        return;
    }
    if b.actions[effect as usize].is_some() {
        b.battler_mut(effect).flinched = true;
    }
}

/// A message about the attacker in the move's context, waiting `B_WAIT_TIME_LONG`.
fn about_attacker(u: &MoveUse, id: Str) -> BattleEvent {
    BattleEvent::Msg(u.msg_long(id, u.attacker))
}

fn status_icon(b: &Battle, env: &mut Env, side: Side) {
    env.out.push(BattleEvent::StatusIcon {
        side,
        status: b.battler(side).mon.status,
    });
}

/// `AtkCanceler_UnableToUseMove` (singles, the engine's statuses): sleep, freeze, flinch,
/// confusion, paralysis, Bide.
pub(super) fn canceler(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    let a = u.attacker;
    // CANCELLER_ASLEEP: the counter drops first (no Early Bird: it is `Ability::Other`).
    if let Status::Sleep(n) = b.battler(a).mon.status {
        let n = n.saturating_sub(1);
        b.battler_mut(a).mon.status = if n == 0 {
            Status::None
        } else {
            Status::Sleep(n)
        };
        b.write_back(a);
        if n > 0 {
            // BattleScript_MoveUsedIsAsleep.
            env.out.push(about_attacker(u, Str::PkmnFastAsleep));
            return Canceler::Skip;
        }
        // BattleScript_MoveUsedWokeUp.
        env.out.push(about_attacker(u, Str::PkmnWokeUp));
        status_icon(b, env, a);
    }
    // CANCELLER_FROZEN: thaws when `Random() % 5 == 0`.
    if b.battler(a).mon.status == Status::Freeze {
        if !env.rng.random().is_multiple_of(5) {
            env.out.push(about_attacker(u, Str::PkmnIsFrozen));
            return Canceler::Skip;
        }
        b.battler_mut(a).mon.status = Status::None;
        b.write_back(a);
        // BattleScript_MoveUsedUnfroze.
        env.out.push(about_attacker(u, Str::PkmnWasDefrosted2));
        status_icon(b, env, a);
    }
    // CANCELLER_FLINCH.
    if b.battler(a).flinched {
        b.battler_mut(a).flinched = false;
        cancel_multi_turn(b, a);
        env.out.push(about_attacker(u, Str::PkmnFlinched));
        return Canceler::Skip;
    }
    // CANCELLER_CONFUSED: the counter drops first; then a 50% self-hit.
    if b.battler(a).confusion > 0 {
        b.battler_mut(a).confusion -= 1;
        if b.battler(a).confusion > 0 {
            let fine = env.rng.random() & 1 != 0;
            // BattleScript_MoveUsedIsConfused.
            env.out.push(about_attacker(u, Str::PkmnIsConfused));
            if !fine {
                confusion_self_hit(b, env, u);
                return Canceler::Skip;
            }
        } else {
            // BattleScript_MoveUsedIsConfusedNoMore.
            env.out.push(about_attacker(u, Str::PkmnHealedConfusion));
        }
    }
    // CANCELLER_PARALYSED: `Random() % 4 == 0` (drawn only when paralysed).
    if b.battler(a).mon.status == Status::Paralysis && env.rng.random().is_multiple_of(4) {
        // BattleScript_MoveUsedIsParalyzed.
        env.out.push(about_attacker(u, Str::PkmnIsParalyzed));
        cancel_multi_turn(b, a);
        return Canceler::Skip;
    }
    // CANCELLER_BIDE.
    if b.battler(a).bide_turns > 0 {
        b.battler_mut(a).bide_turns -= 1;
        if b.battler(a).bide_turns > 0 {
            // BattleScript_BideStoringEnergy (then BattleScript_MoveEnd).
            env.out.push(about_attacker(u, Str::PkmnStoringEnergy));
            return Canceler::Done(*u, None);
        }
        return bide_release(b, env, u);
    }
    Canceler::Use
}

/// `BattleScript_DoSelfConfusionDmg`: `CalculateBaseDamage(attacker, attacker, MOVE_POUND, 40
/// power, Normal)` with `adjustnormaldamage2`'s random roll (no crit, no type), on the
/// attacker. Recorded as move damage (`gBideDmg`), as Gen3's `datahpupdate`.
fn confusion_self_hit(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let a = u.attacker;
    cancel_multi_turn(b, a);
    let f = b.battler(a).fighter();
    let damage = calc::random_damage(
        calc::base_damage(&f, &f, 40, Type::Normal, false),
        env.rng.random(),
    );
    env.out.push(about_attacker(u, Str::PkmnHurtItself));
    env.out.push(BattleEvent::Hit {
        target: a,
        eff: Effectiveness::Normal,
    });
    b.change_hp(env, a, damage);
    b.try_faint(env, a);
}

/// `BattleScript_BideAttack` / `BattleScript_BideNoEnergyToAttack`: no attack string nor PP;
/// twice the stored damage on the foe (`gBideTarget`, the other battler in singles), only
/// `NoEffect` kept from the type check.
fn bide_release(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    let u = MoveUse {
        target: u.attacker.other(),
        ..*u
    };
    let stored = b.battler(u.attacker).bide_damage;
    env.out.push(about_attacker(&u, Str::PkmnUnleashedEnergy));
    if stored == 0 {
        b.fail(env, &u);
        return Canceler::Done(u, None);
    }
    if !b.accuracy(env, &u) {
        // BattleScript_MoveMissed.
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::AttackMissed, u.attacker)));
        return Canceler::Done(u, None);
    }
    let eff = if b.immune(env, &u) {
        Effectiveness::NoEffect
    } else {
        Effectiveness::Normal
    };
    let r = HitResult {
        damage: stored.saturating_mul(2).min(i32::MAX as u32) as i32,
        hp_dealt: 0,
        crit: false,
        eff,
    };
    let r = b.deal(env, &u, r);
    b.try_faint(env, u.target);
    Canceler::Done(u, Some(r))
}

/// `max(1, max_hp / div)`.
fn fraction(max_hp: u16, div: u16) -> i32 {
    (max_hp / div).max(1) as i32
}

/// `DoBattlerEndTurnEffects` for each battler in turn order (the engine's subset): Leech Seed,
/// poison, toxic, burn, then the Charge timer. Residual damage ignores Bide
/// (`HITMARKER_IGNORE_BIDE`).
pub(super) fn end_turn(b: &mut Battle, env: &mut Env) {
    for side in b.order {
        let foe = side.other();
        // ENDTURN_LEECH_SEED: BattleScript_LeechSeedTurnDrain.
        let m = b.battler(side);
        if m.seeded && m.mon.hp != 0 && b.battler(foe).mon.hp != 0 {
            let dealt = b.change_hp_passive(env, side, fraction(m.mon.max_hp, 8));
            b.change_hp_passive(env, foe, -(dealt as i32));
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(Str::LeechSeedDrain, side, foe, side)
            }));
            b.try_faint(env, side);
            b.try_faint(env, foe);
        }
        let m = b.battler(side);
        let max_hp = m.mon.max_hp;
        let residual = match m.mon.status {
            _ if m.mon.hp == 0 => None,
            // ENDTURN_POISON.
            Status::Poison => Some((Str::PkmnHurtByPoison, fraction(max_hp, 8))),
            // ENDTURN_BAD_POISON: the counter rises (to 15) before the damage.
            Status::Toxic(n) => {
                let n = (n + 1).min(15);
                b.battler_mut(side).mon.status = Status::Toxic(n);
                Some((Str::PkmnHurtByPoison, fraction(max_hp, 16) * n as i32))
            }
            // ENDTURN_BURN.
            Status::Burn => Some((Str::PkmnHurtByBurn, fraction(max_hp, 8))),
            _ => None,
        };
        if let Some((id, damage)) = residual {
            // BattleScript_PoisonTurnDmg / BurnTurnDmg.
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(id, side, side, side)
            }));
            b.change_hp_passive(env, side, damage);
            b.try_faint(env, side);
        }
        // ENDTURN_CHARGE.
        let m = b.battler_mut(side);
        m.charge = m.charge.saturating_sub(1);
    }
}
