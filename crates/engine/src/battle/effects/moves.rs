//! The move scripts of the engine's effects (`data/battle_scripts_1.s`).

use super::abilities;
use super::status::{self, Cause};
use super::{RunBlock, run_block};
use crate::battle::core::{Action, Battle, Env, HitResult, MoveUse, Secondary};
use crate::battle::{BattleEvent, BattleOutcome, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, MoveEffect, Stat, Status, StatusKind, Type};

/// Runs `u`'s script by effect; `Unsupported` falls back to the core's handling.
pub(super) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    match u.info.effect {
        MoveEffect::StatDownHit(_) => b.hit(env, u, with(stat_down_hit), false),
        MoveEffect::StatusHit(_) => b.hit(env, u, with(status_hit), false),
        MoveEffect::ConfuseHit => b.hit(env, u, with(confuse_hit), false),
        MoveEffect::FlinchHit | MoveEffect::FlinchMinimizeHit => {
            b.hit(env, u, with(flinch_hit), false)
        }
        MoveEffect::VitalThrow => b.hit(env, u, None, true),
        MoveEffect::Status(kind) => {
            status_move(b, env, u, kind);
            None
        }
        MoveEffect::Confuse => {
            confuse_move(b, env, u);
            None
        }
        MoveEffect::Absorb => absorb(b, env, u),
        MoveEffect::LeechSeed => {
            leech_seed(b, env, u);
            None
        }
        MoveEffect::Bide => {
            // BattleScript_EffectBide: `setbide` (two more turns, `gBideDmg` cleared).
            b.attack_string(env, u);
            anim(env, u);
            let m = b.battler_mut(u.attacker);
            m.bide_turns = 2;
            m.bide_damage = 0;
            None
        }
        MoveEffect::FocusEnergy => {
            focus_energy(b, env, u);
            None
        }
        MoveEffect::NaturePower => nature_power(b, env, u),
        MoveEffect::MultiHit => {
            multi_hit(b, env, u, None);
            None
        }
        MoveEffect::DoubleHit => {
            // BattleScript_EffectDoubleHit: `setmultihitcounter 2`.
            multi_hit(b, env, u, Some(2));
            None
        }
        // As a move, Pursuit is a plain hit (its double damage is `pursuit_on_switch`).
        MoveEffect::Pursuit => b.hit(env, u, None, false),
        MoveEffect::Attract => {
            attract(b, env, u);
            None
        }
        MoveEffect::MeanLook => {
            mean_look(b, env, u);
            None
        }
        MoveEffect::MudSport | MoveEffect::WaterSport => {
            sport(b, env, u);
            None
        }
        MoveEffect::Teleport => {
            teleport(b, env, u);
            None
        }
        MoveEffect::Uproar => uproar(b, env, u),
        MoveEffect::Yawn => {
            yawn(b, env, u);
            None
        }
        MoveEffect::Foresight => {
            foresight(b, env, u);
            None
        }
        MoveEffect::FuryCutter => fury_cutter(b, env, u),
        MoveEffect::Splash => {
            b.attack_string(env, u);
            anim(env, u);
            env.out.push(BattleEvent::Msg(
                u.msg_long(Str::ButNothingHappened, u.attacker),
            ));
            None
        }
        MoveEffect::DefenseCurl => {
            // BattleScript_EffectDefenseCurl: `setdefensecurlbit` (only Rollout reads it), then
            // the Defense +1 of BattleScript_EffectStatUp.
            b.stat_up_move(env, u, Stat::Def, 1);
            None
        }
        MoveEffect::SonicBoom => sonic_boom(b, env, u),
        MoveEffect::Charge => {
            // BattleScript_EffectCharge: `setcharge` (STATUS3_CHARGED_UP, chargeTimer = 2).
            b.attack_string(env, u);
            b.battler_mut(u.attacker).charge = 2;
            anim(env, u);
            env.out.push(BattleEvent::Msg(
                u.msg_long(Str::PkmnChargingPower, u.attacker),
            ));
            None
        }
        _ => b.unsupported(env, u),
    }
}

/// A `seteffectwithchance` secondary effect.
pub(super) fn with(apply: fn(&mut Battle, &mut Env, &MoveUse, &HitResult)) -> Option<Secondary> {
    Some(Secondary {
        certain: false,
        apply,
    })
}

fn anim(env: &mut Env, u: &MoveUse) {
    env.out.push(BattleEvent::MoveAnim {
        attacker: u.attacker,
        mov: u.mov,
    });
}

/// `MOVE_EFFECT_<STAT>_MINUS_1` (`EFFECT_<STAT>_DOWN_HIT`, Mud-Slap): `ChangeStatBuffs`
/// without `STAT_CHANGE_ALLOW_PTR`, so a block (Keen Eye, Shield Dust) or -6 is silent; else
/// `BattleScript_StatDown`.
fn stat_down_hit(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let MoveEffect::StatDownHit(stat) = u.info.effect else {
        return;
    };
    let side = u.target;
    if b.battler(side).mon.hp == 0 || abilities::blocks_secondary_drop(b, side, stat) {
        return;
    }
    if b.change_stage(side, stat, -1) {
        env.out.push(BattleEvent::StatAnim { side, up: false });
        env.out
            .push(BattleEvent::Msg(b.stat_msg(u, side, stat, 1, false)));
    }
}

/// `EFFECT_POISON_HIT` / `BURN_HIT` / `PARALYZE_HIT` / `FREEZE_HIT`.
pub(super) fn status_hit(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if let MoveEffect::StatusHit(kind) = u.info.effect {
        status::set_status(b, env, u, u.target, kind, Cause::SECONDARY);
    }
}

fn confuse_hit(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::confuse(b, env, u, u.target, Cause::SECONDARY);
}

fn flinch_hit(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::flinch(b, env, u, u.target, Cause::SECONDARY);
}

/// `BattleScript_EffectSleep` / `Poison` / `Toxic` / `Paralyze` (and Will-O-Wisp's burn):
/// attack string, the "already" checks, accuracy (a miss is "X's attack missed!"), the
/// animation, then `seteffectprimary`.
fn status_move(b: &mut Battle, env: &mut Env, u: &MoveUse, kind: StatusKind) {
    b.attack_string(env, u);
    let t = b.battler(u.target);
    let status = t.mon.status;
    let poison_type = t.types.contains(&Type::Poison) || t.types.contains(&Type::Steel);
    let poisoned = matches!(status, Status::Poison | Status::Toxic(_));
    let statused = status != Status::None;
    let target_msg = |id| Some(BattleEvent::Msg(u.msg_long(id, u.target)));
    let refusal = match kind {
        StatusKind::Sleep if matches!(status, Status::Sleep(_)) => {
            target_msg(Str::PkmnAlreadyAsleep)
        }
        // `jumpifcantmakeasleep` → BattleScript_CantMakeAsleep (gUproarAwakeStringIds).
        StatusKind::Sleep if status::uproar_keeps_awake(b, u.target).is_some() => {
            let uproarer = status::uproar_keeps_awake(b, u.target);
            target_msg(if uproarer == Some(u.target) {
                Str::PkmnCantSleepInUproar
            } else {
                Str::UproarKeptPkmnAwake
            })
        }
        StatusKind::Sleep if status::never_sleeps(t.ability) => Some(BattleEvent::Msg(Msg {
            buffs: [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(t.ability)),
            ],
            ..u.msg_long(Str::PkmnStayedAwakeUsing, u.target)
        })),
        StatusKind::Poison if poisoned => target_msg(Str::PkmnAlreadyPoisoned),
        StatusKind::Poison if poison_type => target_msg(Str::ItDoesntAffect),
        StatusKind::Toxic if poisoned => target_msg(Str::PkmnAlreadyPoisoned),
        StatusKind::Toxic if !statused && poison_type => target_msg(Str::ItDoesntAffect),
        // `typecalc` + `jumpifmovehadnoeffect` (Thunder Wave on Ground).
        StatusKind::Paralysis if b.immune(env, u) => target_msg(Str::ItDoesntAffect),
        StatusKind::Paralysis if status == Status::Paralysis => {
            target_msg(Str::PkmnIsAlreadyParalyzed)
        }
        StatusKind::Burn if status == Status::Burn => target_msg(Str::PkmnAlreadyHasBurn),
        StatusKind::Burn if t.types.contains(&Type::Fire) => target_msg(Str::ItDoesntAffect),
        // No status move freezes.
        StatusKind::Freeze => Some(BattleEvent::Msg(u.msg_long(Str::ButItFailed, u.attacker))),
        _ if statused => Some(BattleEvent::Msg(u.msg_long(Str::ButItFailed, u.attacker))),
        _ => None,
    };
    if let Some(e) = refusal {
        env.out.push(e);
        return;
    }
    if !b.accuracy(env, u) {
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::AttackMissed, u.attacker)));
        return;
    }
    anim(env, u);
    status::set_status(b, env, u, u.target, kind, Cause::PRIMARY);
}

/// `BattleScript_EffectConfuse`.
fn confuse_move(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if b.battler(u.target).confusion > 0 {
        env.out.push(BattleEvent::Msg(
            u.msg_long(Str::PkmnAlreadyConfused, u.target),
        ));
        return;
    }
    if !b.accuracy(env, u) {
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::AttackMissed, u.attacker)));
        return;
    }
    anim(env, u);
    status::confuse(b, env, u, u.target, Cause::PRIMARY);
}

/// `BattleScript_EffectAbsorb`: a hit without a secondary roll, then `negativedamage` (half
/// the HP dealt, at least 1) to the attacker with `STRINGID_PKMNENERGYDRAINED`, skipped when
/// the move had no effect; then the attacker's and the target's faint checks.
fn absorb(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let r = b.damage(env, u);
    let r = b.deal(env, u, r);
    // No effect (an immune target): `negativedamage` still sets -1, but `healthbarupdate` /
    // `datahpupdate BS_ATTACKER` do nothing under MOVE_RESULT_NO_EFFECT and
    // `jumpifmovehadnoeffect` skips the drain string, so neither heal nor message.
    // Liquid Ooze (BattleScript_AbsorbLiquidOoze): the same amount hurts the attacker.
    if r.eff != Effectiveness::NoEffect {
        let heal = (r.hp_dealt / 2).max(1) as i32;
        let (hp, id) = if b.battler(u.target).ability == Ability::LiquidOoze {
            (heal, Str::ItSuckedLiquidOoze)
        } else {
            (-heal, Str::AbsorbDrain)
        };
        b.change_hp_passive(env, u.attacker, hp);
        env.out.push(BattleEvent::Msg(u.msg_long(id, u.target)));
    }
    b.try_faint(env, u.attacker);
    b.try_faint(env, u.target);
    Some(r)
}

/// `BattleScript_EffectLeechSeed` + `Cmd_setseeded`: one accuracy roll; a miss or a seeded
/// target "evaded the attack", a Grass target is not affected.
fn leech_seed(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let hit = b.accuracy(env, u);
    let t = b.battler(u.target);
    let id = if !hit || t.seeded {
        Str::PkmnEvadedAttack
    } else if t.types.contains(&Type::Grass) {
        Str::ItDoesntAffect
    } else {
        b.battler_mut(u.target).seeded = true;
        anim(env, u);
        Str::PkmnSeeded
    };
    env.out.push(BattleEvent::Msg(u.msg_long(id, u.target)));
}

/// `BattleScript_EffectFocusEnergy`.
fn focus_energy(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if b.battler(u.attacker).focus_energy {
        b.fail(env, u);
        return;
    }
    b.battler_mut(u.attacker).focus_energy = true;
    anim(env, u);
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::GettingPumped, u.attacker)));
}

/// `BattleScript_EffectNaturePower` + `Cmd_callenvironmentattack`: "X used NATURE POWER!"
/// without PP, "NATURE POWER turned into Y!", then Y's script with the same move slot (its
/// `ppreduce` takes the PP) and Y's own target.
fn nature_power(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    env.out
        .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    let mov = env.t.nature_power[b.setup.environment as usize];
    let u = b.move_use(env, u.attacker, mov, u.slot);
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::NaturePowerTurnedInto, u.attacker),
    ));
    b.execute(env, &u)
}

/// `BattleScript_EffectSonicboom`: 20 HP; the type check keeps only "no effect".
fn sonic_boom(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let eff = if b.immune(env, u) {
        Effectiveness::NoEffect
    } else {
        Effectiveness::Normal
    };
    let r = HitResult {
        damage: 20,
        hp_dealt: 0,
        crit: false,
        eff,
    };
    let r = b.deal(env, u, r);
    b.secondary(env, u, &r, None);
    b.try_faint(env, u.target);
    Some(r)
}

/// `BattleScript_EffectMultiHit`: one accuracy check, `setmultihitcounter` (2–5: `Random() &
/// 3`, then `(Random() & 3) + 2` above 1; `fixed` for Double Kick's 2, no roll), then per
/// hit `critcalc` … `datahpupdate`,
/// `critmessage` and `moveendto MOVEEND_NEXT_TARGET` (defrost, Synchronize, contact
/// abilities); the loop stops when the attacker faints, the target faints or the attacker
/// falls asleep. Then the result message, "Hit N time(s)!", `seteffectwithchance` (its roll)
/// and the target's faint. Returns nothing: `moveend` already ran per hit.
fn multi_hit(b: &mut Battle, env: &mut Env, u: &MoveUse, fixed: Option<u16>) {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return;
    }
    b.attack_string(env, u);
    let count = fixed.unwrap_or_else(|| {
        let count = env.rng.random() & 3;
        if count > 1 {
            (env.rng.random() & 3) + 2
        } else {
            count + 2
        }
    });
    let mut hits = 0u32;
    let mut last: Option<HitResult> = None;
    let mut strings = true;
    for _ in 0..count {
        if b.battler(u.attacker).mon.hp == 0 {
            strings = false;
            break;
        }
        if b.battler(u.target).mon.hp == 0
            || matches!(b.battler(u.attacker).mon.status, Status::Sleep(_))
        {
            break;
        }
        if b.immune(env, u) {
            // `jumpifmovehadnoeffect` after `typecalc`: the crit roll only.
            if !b.is_first_battle() {
                env.rng.random();
            }
            b.battler_mut(u.attacker).v.unable = true;
            last = Some(HitResult {
                damage: 0,
                hp_dealt: 0,
                crit: false,
                eff: Effectiveness::NoEffect,
            });
            break;
        }
        let mut r = b.damage(env, u);
        anim(env, u);
        env.out.push(BattleEvent::Hit {
            target: u.target,
            eff: r.eff,
        });
        r.hp_dealt = b.change_hp(env, u.target, r.damage);
        if r.crit {
            env.out
                .push(BattleEvent::Msg(u.msg_long(Str::CriticalHit, u.attacker)));
        }
        hits += 1;
        last = Some(r);
        abilities::move_end(b, env, u, Some(r));
    }
    let r = last.unwrap_or(HitResult {
        damage: 0,
        hp_dealt: 0,
        crit: false,
        eff: Effectiveness::Normal,
    });
    if strings {
        let result = match r.eff {
            Effectiveness::Super => Some(Str::SuperEffective),
            Effectiveness::NotVery => Some(Str::NotVeryEffective),
            Effectiveness::NoEffect => Some(Str::ItDoesntAffect),
            _ => None,
        };
        if let Some(id) = result {
            env.out.push(BattleEvent::Msg(u.msg_long(id, u.target)));
        }
        if r.eff != Effectiveness::NoEffect {
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [Buff::Number(hits), Buff::None, Buff::None],
                ..u.msg_long(Str::HitXTimes, u.attacker)
            }));
        }
    }
    b.secondary(env, u, &r, None);
    b.try_faint(env, u.target);
}

/// A message naming `ability` (`B_*_ABILITY` = `buffs[2]`) about `active`, waiting long.
fn ability_msg(u: &MoveUse, id: Str, active: Side, ability: Ability) -> BattleEvent {
    BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(ability)),
        ],
        ..u.msg_long(id, active)
    })
}

/// `BattleScript_EffectAttract` + `Cmd_tryinfatuating`: the attack string, one accuracy roll
/// (a miss is "X's attack missed!"), the target's Oblivious ("prevents romance"), then "But it
/// failed!" for the same gender, a genderless mon or an infatuated target; else it falls in
/// love with the attacker.
fn attract(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    let (a, t) = (b.battler(u.attacker), b.battler(u.target));
    if t.ability == Ability::Oblivious {
        env.out.push(ability_msg(
            u,
            Str::PkmnPreventsRomanceWith,
            u.target,
            Ability::Oblivious,
        ));
        return;
    }
    if !abilities::opposite_genders(a.mon.gender, t.mon.gender) || t.v.infatuated {
        b.fail(env, u);
        return;
    }
    b.battler_mut(u.target).v.infatuated = true;
    anim(env, u);
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::PkmnFellInLove, u.target)));
}

/// `BattleScript_EffectMeanLook` (Mean Look, Block, Spider Web): no accuracy roll
/// (`NO_ACC_CALC`); "But it failed!" when the target already cannot escape.
fn mean_look(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if b.battler(u.target).v.escape_prevented {
        b.fail(env, u);
        return;
    }
    anim(env, u);
    b.battler_mut(u.target).v.escape_prevented = true;
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::TargetCantEscapeNow, u.target),
    ));
}

/// `BattleScript_EffectMudSport` / `WaterSport` + `Cmd_settypebasedhalvers`: once per user
/// (`STATUS3_MUDSPORT` / `_WATERSPORT`), "But it failed!" again.
fn sport(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let mud = u.info.effect == MoveEffect::MudSport;
    let v = &mut b.battler_mut(u.attacker).v;
    let set = if mud {
        &mut v.mud_sport
    } else {
        &mut v.water_sport
    };
    if *set {
        b.fail(env, u);
        return;
    }
    *set = true;
    anim(env, u);
    // gSportsUsedStringIds.
    let id = if mud {
        Str::ElectricityWeakened
    } else {
        Str::FireWeakened
    };
    env.out.push(BattleEvent::Msg(u.msg_long(id, u.attacker)));
}

/// `BattleScript_EffectTeleport`: "But it failed!" in a trainer battle or when the attacker
/// may not run (`BATTLE_RUN_FORBIDDEN`); a foe's trapping ability "made it ineffective"; else
/// the attacker flees and the battle ends (`setoutcomeonteleport`, no closing message).
fn teleport(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if b.is_trainer() {
        b.fail(env, u);
        return;
    }
    match run_block(b, u.attacker) {
        Some(RunBlock::Forbidden(_)) => b.fail(env, u),
        Some(RunBlock::Ability { owner, ability }) => {
            env.out
                .push(ability_msg(u, Str::PkmnSXMadeItIneffective, owner, ability))
        }
        None => {
            anim(env, u);
            env.out.push(BattleEvent::Msg(
                u.msg_long(Str::PkmnFledFromBattle, u.attacker),
            ));
            let outcome = match u.attacker {
                Side::Player => BattleOutcome::PlayerTeleported,
                Side::Opponent => BattleOutcome::MonTeleported,
            };
            b.finish(env, outcome);
        }
    }
}

/// `BattleScript_EffectUproar`: accuracy (a miss is `BattleScript_PrintMoveMissed`), the
/// attack string (PP only on the first turn: `STATUS2_MULTIPLETURNS` skips `ppreduce`), the
/// hit, then `seteffectwithchance` (its roll, chance 100): `MOVE_EFFECT_UPROAR` on the
/// attacker when it is not making one yet.
fn uproar(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    if b.battler(u.attacker).v.uproar > 0 {
        env.out
            .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    } else {
        b.attack_string(env, u);
    }
    let r = b.damage(env, u);
    let r = b.deal(env, u, r);
    b.secondary(env, u, &r, with(start_uproar));
    b.try_faint(env, u.target);
    Some(r)
}

/// `MOVE_EFFECT_UPROAR`: `(Random() & 3) + 2` turns, `gLockedMoves`, "X caused an UPROAR!".
fn start_uproar(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if b.battler(u.attacker).v.uproar > 0 {
        return;
    }
    b.battler_mut(u.attacker).v.uproar = (env.rng.random() & 3) as u8 + 2;
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::PkmnCausedUproar, u.attacker),
    ));
}

/// `BattleScript_EffectYawn`: Insomnia / Vital Spirit "made it ineffective"; no accuracy roll;
/// "But it failed!" during an uproar (`jumpifcantmakeasleep`), for a drowsy or statused target
/// (`setyawn`); else the target is drowsy (`STATUS3_YAWN_TURN(2)`).
fn yawn(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let t = b.battler(u.target);
    if status::never_sleeps(t.ability) {
        env.out.push(ability_msg(
            u,
            Str::PkmnSXMadeItIneffective,
            u.target,
            t.ability,
        ));
        return;
    }
    if status::uproar_keeps_awake(b, u.target).is_some()
        || t.v.yawn > 0
        || t.mon.status != Status::None
    {
        b.fail(env, u);
        return;
    }
    b.battler_mut(u.target).v.yawn = 2;
    anim(env, u);
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::PkmnWasMadeDrowsy, u.target),
    ));
}

/// `BattleScript_EffectForesight`: accuracy (a miss is `BattleScript_PrintMoveMissed`), then
/// `setforesight` (always) and "X identified Y!".
fn foresight(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return;
    }
    b.attack_string(env, u);
    b.battler_mut(u.target).foresight = true;
    anim(env, u);
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::PkmnIdentified, u.target)));
}

/// `BattleScript_EffectFuryCutter`: the attack string, then accuracy; `furycuttercalc` resets
/// the counter on a miss, else raises it (to 5) and doubles the power per earlier hit. A type
/// immunity (`jumpifmovehadnoeffect` after the crit roll) resets it too, with no damage roll.
fn fury_cutter(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.battler_mut(u.attacker).v.fury_cutter = 0;
        b.miss_message(env, u);
        return None;
    }
    let n = (b.battler(u.attacker).v.fury_cutter + 1).min(5);
    b.battler_mut(u.attacker).v.fury_cutter = n;
    let u = MoveUse {
        power: (u.info.power as u16 * (1 << (n - 1))).min(u8::MAX as u16) as u8,
        ..*u
    };
    if b.immune(env, &u) {
        if !b.is_first_battle() {
            env.rng.random();
        }
        let a = b.battler_mut(u.attacker);
        a.v.fury_cutter = 0;
        a.v.unable = true;
        b.no_effect = true;
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::ItDoesntAffect, u.target)));
        return None;
    }
    let r = b.damage(env, &u);
    let r = b.deal(env, &u, r);
    b.secondary(env, &u, &r, None);
    b.try_faint(env, u.target);
    Some(r)
}

/// See [`super::pursuit_on_switch`]: `jumpifnopursuitswitchdmg` (the foe chose Pursuit, is
/// neither asleep nor frozen nor loafing, and the switching battler has HP), then
/// `BattleScript_PursuitDmgOnSwitchOut` with `sDMG_MULTIPLIER` 2: the attack string and PP,
/// the damage (no accuracy check), the faint check and `moveendfromto
/// MOVEEND_ON_DAMAGE_ABILITIES`.
pub(super) fn pursuit_on_switch(b: &mut Battle, env: &mut Env, side: Side) -> bool {
    let foe = side.other();
    let Some(Action::Move(slot)) = b.actions[foe as usize] else {
        return false;
    };
    let f = b.battler(foe);
    let Some(mov) = f.mon.moves[slot as usize] else {
        return false;
    };
    if env.t.move_info(mov).effect != MoveEffect::Pursuit
        || matches!(f.mon.status, Status::Sleep(_) | Status::Freeze)
        || f.v.truant
        || b.battler(side).mon.hp == 0
    {
        return false;
    }
    b.actions[foe as usize] = None;
    let u = MoveUse {
        target: side,
        multiplier: 2,
        ..b.move_use(env, foe, mov, Some(slot))
    };
    b.attack_string(env, &u);
    let r = b.damage(env, &u);
    let r = b.deal(env, &u, r);
    b.try_faint(env, side);
    abilities::move_end_from_on_damage(b, env, &u, Some(r));
    true
}
