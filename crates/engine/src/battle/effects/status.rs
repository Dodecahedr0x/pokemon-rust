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

/// `CancelMultiTurnMoves`: ends Bide and Uproar, resets Fury Cutter.
pub(super) fn cancel_multi_turn(b: &mut Battle, side: Side) {
    let m = b.battler_mut(side);
    m.bide_turns = 0;
    m.v.uproar = 0;
    m.v.fury_cutter = 0;
    super::mech::cancel_locks(b, side);
}

/// `UproarWakeUpCheck`: some battler is making an uproar and `side` is not Soundproof. The
/// uproarer, if any.
pub(super) fn uproar_keeps_awake(b: &Battle, side: Side) -> Option<Side> {
    if b.battler(side).ability == Ability::Soundproof {
        return None;
    }
    [Side::Player, Side::Opponent]
        .into_iter()
        .find(|&s| b.battler(s).v.uproar > 0)
}

/// Insomnia / Vital Spirit.
pub(super) fn never_sleeps(a: Ability) -> bool {
    matches!(a, Ability::Insomnia | Ability::VitalSpirit)
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
    // Immunity, Water Veil and Limber come first: a primary effect names them
    // (`BattleScript_PSNPrevention` …), a chance one is blocked silently.
    let guard = match (kind, b.battler(effect).ability) {
        (StatusKind::Poison | StatusKind::Toxic, a @ Ability::Immunity) => {
            Some((a, Str::PkmnPreventsPoisoningWith))
        }
        (StatusKind::Burn, a @ Ability::WaterVeil) => Some((a, Str::PkmnSXPreventsBurns)),
        (StatusKind::Paralysis, a @ Ability::Limber) => Some((a, Str::PkmnPreventsParalysisWith)),
        _ => None,
    };
    if let Some((ability, id)) = guard {
        if cause.primary {
            ability_prevents(env, u, effect, ability, id, cause);
        }
        return false;
    }
    // Safeguard blocks a secondary status silently (`SIDE_STATUS_SAFEGUARD`, effect ≤ 7).
    if !cause.primary && cause.ability.is_none() && super::mech::safeguarded(b, effect) {
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
        // The sun (`WEATHER_HAS_EFFECT`) and Magma Armor prevent it, silently.
        StatusKind::Freeze => {
            has(Type::Ice)
                || matches!(
                    b.weather(),
                    crate::battle::core::Weather::Sun | crate::battle::core::Weather::HarshSun
                )
                || m.ability == Ability::MagmaArmor
        }
        StatusKind::Paralysis => false,
        // An uproar (unless Soundproof), Insomnia and Vital Spirit keep it awake, silently.
        StatusKind::Sleep => {
            if statused {
                return false;
            }
            uproar_keeps_awake(b, effect).is_some() || never_sleeps(m.ability)
        }
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

/// The `g*PreventionStringIds` message of `ability` on `effect` blocking a status: `id` for a
/// move (`B_MSG_ABILITY_PREVENTS_MOVE_STATUS`), `PkmnSXPreventsYSZ` for an ability's effect
/// (`B_MSG_ABILITY_PREVENTS_ABILITY_STATUS`: "ATK's ability prevents DEF's ability", with
/// `gBattlerAttacker` the effect battler and the cause's owner the target; both ability
/// placeholders read `buffs[2]`, so the cause's ability is not named).
fn ability_prevents(
    env: &mut Env,
    u: &MoveUse,
    effect: Side,
    ability: Ability,
    id: Str,
    cause: Cause,
) {
    let buffs = [
        Buff::None,
        Buff::None,
        Buff::Text(Str::AbilityName(ability)),
    ];
    let m = match cause.ability {
        Some((owner, _)) => {
            env.out
                .push(BattleEvent::Report(crate::battle::BattleReport::Degraded(
                    "prevents ability status: one ability name",
                )));
            msg(Str::PkmnSXPreventsYSZ, effect, owner, effect)
        }
        None => msg(id, u.attacker, effect, effect),
    };
    env.out.push(BattleEvent::Msg(Msg {
        mov: Some(u.mov),
        buffs,
        wait: LONG,
        ..m
    }));
}

/// `MOVE_EFFECT_CONFUSION` on `effect`: `Random() % 4 + 2` turns. Nothing when already
/// confused or Own Tempo (silently). True when inflicted.
pub(super) fn confuse(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    effect: Side,
    cause: Cause,
) -> bool {
    let m = b.battler(effect);
    if cause.shield_dust(b, effect)
        || m.mon.hp == 0
        || m.confusion > 0
        || m.ability == Ability::OwnTempo
    {
        return false;
    }
    if !cause.primary && cause.ability.is_none() && super::mech::safeguarded(b, effect) {
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
/// (`GetBattlerTurnOrderNum(gEffectBattler) > gCurrentTurnActionNumber`). Silent, but Inner
/// Focus names itself for a primary or `certain` (`MOVE_EFFECT_CERTAIN`) flinch
/// (`BattleScript_FlinchPrevention`); a chance flinch, even at 100%, is blocked silently.
pub(super) fn flinch(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    effect: Side,
    cause: Cause,
    certain: bool,
) {
    if cause.shield_dust(b, effect) || b.battler(effect).mon.hp == 0 {
        return;
    }
    if b.battler(effect).ability == Ability::InnerFocus {
        if cause.primary || certain {
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(Ability::InnerFocus)),
                ],
                ..u.msg_long(Str::PkmnSXPreventsFlinching, effect)
            }));
        }
        return;
    }
    if b.actions[effect.i()].is_some() {
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

/// `AtkCanceler_UnableToUseMove` (singles, the engine's statuses): sleep, freeze, Truant,
/// flinch, confusion, paralysis, infatuation, Bide. The immobilities that
/// `WasUnableToUseMove` reads set `Volatiles::unable`.
pub(super) fn canceler(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Canceler {
    let a = u.attacker;
    // CANCELLER_FLAGS.
    super::mech::canceler_flags(b, u);
    // CANCELLER_ASLEEP: an uproar wakes it (`UproarWakeUpCheck`); else the counter drops by
    // one (two with Early Bird) first.
    if let Status::Sleep(n) = b.battler(a).mon.status {
        if uproar_keeps_awake(b, a).is_some() {
            b.battler_mut(a).mon.status = Status::None;
            b.write_back(a);
            // BattleScript_MoveUsedWokeUp (B_MSG_WOKE_UP_UPROAR).
            env.out.push(about_attacker(u, Str::PkmnWokeUpInUproar));
            status_icon(b, env, a);
        } else {
            let sub = if b.battler(a).ability == Ability::EarlyBird {
                2
            } else {
                1
            };
            let n = n.saturating_sub(sub);
            b.battler_mut(a).mon.status = if n == 0 {
                Status::None
            } else {
                Status::Sleep(n)
            };
            b.write_back(a);
            // Snore and Sleep Talk go on (`gCurrentMove != MOVE_SNORE / MOVE_SLEEP_TALK`).
            if n > 0 {
                if !super::mech::sleep_usable(u) {
                    // BattleScript_MoveUsedIsAsleep.
                    env.out.push(about_attacker(u, Str::PkmnFastAsleep));
                    return Canceler::Skip;
                }
            } else {
                // BattleScript_MoveUsedWokeUp.
                env.out.push(about_attacker(u, Str::PkmnWokeUp));
                status_icon(b, env, a);
            }
        }
    }
    // CANCELLER_FROZEN: thaws when `Random() % 5 == 0`.
    if b.battler(a).mon.status == Status::Freeze {
        if !env.rng.random().is_multiple_of(5) {
            // A thawing move (Flame Wheel, Sacred Fire) goes on: CANCELLER_THAW.
            if u.info.effect != crate::mon::MoveEffect::ThawHit {
                env.out.push(about_attacker(u, Str::PkmnIsFrozen));
                return Canceler::Skip;
            }
        } else {
            b.battler_mut(a).mon.status = Status::None;
            b.write_back(a);
            // BattleScript_MoveUsedUnfroze.
            env.out.push(about_attacker(u, Str::PkmnWasDefrosted2));
            status_icon(b, env, a);
        }
    }
    // CANCELLER_TRUANT: BattleScript_MoveUsedLoafingAround.
    if b.battler(a).ability == Ability::Truant && b.battler(a).v.truant {
        cancel_multi_turn(b, a);
        env.out.push(about_attacker(u, Str::PkmnLoafing));
        return Canceler::Skip;
    }
    // CANCELLER_RECHARGE.
    if super::mech::canceler_recharge(b, env, u) {
        return Canceler::Skip;
    }
    // CANCELLER_FLINCH.
    if b.battler(a).flinched {
        b.battler_mut(a).flinched = false;
        b.battler_mut(a).v.unable = true;
        cancel_multi_turn(b, a);
        env.out.push(about_attacker(u, Str::PkmnFlinched));
        super::later::steadfast(b, env, a);
        return Canceler::Skip;
    }
    // CANCELLER_DISABLED, CANCELLER_TAUNTED.
    if super::mech::canceler_limits(b, env, u) {
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
                b.battler_mut(a).v.unable = true;
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
        b.battler_mut(a).v.unable = true;
        env.out.push(about_attacker(u, Str::PkmnIsParalyzed));
        cancel_multi_turn(b, a);
        return Canceler::Skip;
    }
    // CANCELLER_IN_LOVE: `Random() & 1`; BattleScript_MoveUsedIsInLove names the attacker and
    // the battler it loves (`gBattleScripting.battler`), then 0 immobilizes it.
    if b.battler(a).v.infatuated {
        let fine = env.rng.random() & 1 != 0;
        env.out.push(BattleEvent::Msg(Msg {
            mov: Some(u.mov),
            wait: LONG,
            ..msg(Str::PkmnInLove, a, u.target, a.other())
        }));
        if !fine {
            b.battler_mut(a).v.unable = true;
            env.out.push(about_attacker(u, Str::PkmnImmobilizedByLove));
            cancel_multi_turn(b, a);
            return Canceler::Skip;
        }
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
    // CANCELLER_THAW.
    super::mech::canceler_thaw(b, env, u);
    Canceler::Use
}

/// `BattleScript_DoSelfConfusionDmg`: `CalculateBaseDamage(attacker, attacker, MOVE_POUND, 40
/// power, Normal)` with `adjustnormaldamage2`'s random roll (no crit, no type) and the
/// attacker's own Focus Band (the target is the attacker), on the attacker. Recorded as move
/// damage (`gBideDmg`), as Gen3's `datahpupdate`.
fn confusion_self_hit(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let a = u.attacker;
    cancel_multi_turn(b, a);
    let f = b.battler(a).fighter();
    let damage = calc::random_damage(
        calc::base_damage(&f, &f, 40, Type::Normal, false),
        env.rng.random(),
    );
    let (damage, hung) = b.focus_band(env, a, damage, Effectiveness::Normal);
    env.out.push(about_attacker(u, Str::PkmnHurtItself));
    env.out.push(BattleEvent::Hit {
        target: a,
        eff: Effectiveness::Normal,
    });
    let hp_dealt = b.change_hp(env, a, damage);
    // resultmessage: MOVE_RESULT_FOE_HUNG_ON.
    let r = HitResult {
        damage,
        hp_dealt,
        crit: false,
        eff: Effectiveness::Normal,
        hung,
    };
    b.hung_on(env, &MoveUse { target: a, ..*u }, &r);
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
    // adjustsetdamage.
    let stored = stored.saturating_mul(2).min(i32::MAX as u32) as i32;
    let (damage, hung) = b.focus_band(env, u.target, stored, eff);
    let r = HitResult {
        damage,
        hp_dealt: 0,
        crit: false,
        eff,
        hung,
    };
    let r = b.deal(env, &u, r);
    b.try_faint(env, u.target);
    Canceler::Done(u, Some(r))
}

/// `max(1, max_hp / div)`.
fn fraction(max_hp: u16, div: u16) -> i32 {
    (max_hp / div).max(1) as i32
}

/// `DoBattlerEndTurnEffects` for each battler in turn order (the engine's subset): the end-turn
/// abilities (Shed Skin, Truant), Leech Seed, poison, toxic, burn, Uproar, the Charge timer,
/// then Yawn. Residual damage ignores Bide (`HITMARKER_IGNORE_BIDE`).
pub(super) fn end_turn(b: &mut Battle, env: &mut Env) {
    for side in b.turn_sides() {
        let foe = side.other();
        // ENDTURN_INGRAIN.
        super::mech::end_turn_ingrain(b, env, side);
        // ENDTURN_ABILITIES.
        super::abilities::end_turn(b, env, side);
        super::later::end_turn(b, env, side);
        // ENDTURN_ITEMS1.
        super::items::end_turn(b, env, side, true);
        // ENDTURN_LEECH_SEED: BattleScript_LeechSeedTurnDrain.
        let m = b.battler(side);
        if m.seeded && m.mon.hp != 0 && b.battler(foe).mon.hp != 0 {
            // Liquid Ooze on the seeded battler: the seeder loses what it would have gained.
            let ooze = m.ability == Ability::LiquidOoze;
            let dealt = b.change_hp_passive(env, side, fraction(m.mon.max_hp, 8)) as i32;
            let (hp, id) = if ooze {
                (dealt, Str::ItSuckedLiquidOoze)
            } else {
                (-dealt, Str::LeechSeedDrain)
            };
            b.change_hp_passive(env, foe, hp);
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(id, side, foe, side)
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
        // ENDTURN_NIGHTMARES, ENDTURN_CURSE, ENDTURN_WRAP.
        super::mech::end_turn_after_status(b, env, side);
        // ENDTURN_UPROAR.
        uproar_turn(b, env, side);
        // ENDTURN_THRASH, ENDTURN_DISABLE, ENDTURN_ENCORE, ENDTURN_LOCK_ON.
        super::mech::end_turn_timers(b, env, side);
        // ENDTURN_CHARGE.
        let m = b.battler_mut(side);
        m.charge = m.charge.saturating_sub(1);
        // ENDTURN_TAUNT.
        super::mech::end_turn_taunt(b, side);
        // ENDTURN_YAWN.
        yawn_turn(b, env, side);
        // ENDTURN_ITEMS2.
        super::items::end_turn(b, env, side, false);
    }
}

/// `ENDTURN_UPROAR` for an uproaring `side`: every sleeping battler but a Soundproof one wakes
/// (index order: player first; `BattleScript_MonWokeUpInUproar`), then the counter drops:
/// "calmed down" when it ends or the battler could not move this turn (`WasUnableToUseMove`,
/// which also ends it), else "is making an uproar".
fn uproar_turn(b: &mut Battle, env: &mut Env, side: Side) {
    if b.battler(side).v.uproar == 0 {
        return;
    }
    for s in [Side::Player, Side::Opponent] {
        let m = b.battler(s);
        if matches!(m.mon.status, Status::Sleep(_))
            && m.mon.hp > 0
            && m.ability != Ability::Soundproof
        {
            b.battler_mut(s).mon.status = Status::None;
            b.write_back(s);
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(Str::PkmnWokeUpInUproar, s, s.other(), s)
            }));
            status_icon(b, env, s);
        }
    }
    b.battler_mut(side).v.uproar -= 1;
    let m = b.battler(side);
    let id = if m.v.unable || m.v.uproar == 0 {
        cancel_multi_turn(b, side);
        Str::PkmnCalmedDown
    } else {
        Str::PkmnMakingUproar
    };
    // BattleScript_PrintUproarOverTurns.
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(id, side, side.other(), side)
    }));
}

/// `ENDTURN_YAWN`: the counter drops; at 0 a battler without a status, Insomnia, Vital
/// Spirit nor an uproar falls asleep for `(Random() & 3) + 2` turns
/// (`BattleScript_YawnMakesAsleep`).
fn yawn_turn(b: &mut Battle, env: &mut Env, side: Side) {
    if b.battler(side).v.yawn == 0 {
        return;
    }
    b.battler_mut(side).v.yawn -= 1;
    let m = b.battler(side);
    if m.v.yawn != 0
        || m.mon.status != Status::None
        || never_sleeps(m.ability)
        || uproar_keeps_awake(b, side).is_some()
    {
        return;
    }
    cancel_multi_turn(b, side);
    let status = Status::Sleep((env.rng.random() & 3) as u8 + 2);
    b.battler_mut(side).mon.status = status;
    b.write_back(side);
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(Str::PkmnFellAsleep, side, side, side)
    }));
    env.out.push(BattleEvent::StatusIcon { side, status });
}
