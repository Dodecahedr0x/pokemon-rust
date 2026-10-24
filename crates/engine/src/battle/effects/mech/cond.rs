//! The status and stat scripts of Wave C (`data/battle_scripts_1.s`).

use super::super::abilities;
use super::super::status::{self, Cause};
use super::{anim, cancel_all, no_acc_check, protected, safeguarded, say, say_with, stage};
use crate::battle::calc;
use crate::battle::core::{Battle, Env, MoveUse, Weather};
use crate::battle::{BattleEvent, Buff, Environment, Side, Str};
use crate::mon::{Ability, MoveEffect, Stat, Status, Type};

/// Runs a status / stat script; false when `u`'s effect is not one of them.
pub(super) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) -> bool {
    use MoveEffect as E;
    match u.info.effect {
        E::Stockpile => stockpile(b, env, u),
        E::Swallow => swallow(b, env, u),
        E::Swagger | E::Flatter => swagger(b, env, u),
        E::TeeterDance => teeter_dance(b, env, u),
        E::Nightmare => nightmare(b, env, u),
        E::Curse => curse(b, env, u),
        E::Rest => rest(b, env, u),
        E::RestoreHp | E::WeatherHeal => restore_hp(b, env, u),
        E::HealBell(aroma) => heal_bell(b, env, u, aroma),
        E::Refresh => refresh(b, env, u),
        E::PainSplit => pain_split(b, env, u),
        E::BellyDrum => belly_drum(b, env, u),
        E::PsychUp => {
            // BattleScript_EffectPsychUp + `copyfoestats` (never fails).
            b.attack_string(env, u);
            let stages = b.battler(u.target).stages;
            b.battler_mut(u.attacker).stages = stages;
            anim(env, u);
            say(env, u, Str::PkmnCopiedStatChanges, u.attacker);
        }
        E::Haze => {
            // BattleScript_EffectHaze + `normalisebuffs`.
            b.attack_string(env, u);
            anim(env, u);
            for s in b.sides() {
                b.battler_mut(s).stages = [0; 8];
            }
            say(env, u, Str::StatChangesGone, u.attacker);
        }
        E::StatsUp(x, y) => stats_up(b, env, u, x, y),
        E::Tickle => tickle(b, env, u),
        E::Memento => memento(b, env, u),
        E::Minimize => {
            // BattleScript_EffectMinimize: `setminimize`, then the evasion +1 of a stat move.
            b.battler_mut(u.attacker).fx.minimized = true;
            b.stat_up_move(env, u, Stat::Evasion, 1);
        }
        E::Conversion => conversion(b, env, u),
        E::Camouflage => camouflage(b, env, u),
        E::Disable => disable(b, env, u),
        E::Encore => encore(b, env, u),
        E::Torment => {
            b.attack_string(env, u);
            if !b.accuracy(env, u) {
                b.miss_message(env, u);
            } else if b.battler(u.target).fx.tormented {
                b.fail(env, u);
            } else {
                b.battler_mut(u.target).fx.tormented = true;
                anim(env, u);
                say(env, u, Str::PkmnSubjectedToTorment, u.target);
            }
        }
        E::Taunt => {
            b.attack_string(env, u);
            if !b.accuracy(env, u) {
                b.miss_message(env, u);
            } else if b.battler(u.target).fx.taunt > 0 {
                b.fail(env, u);
            } else {
                b.battler_mut(u.target).fx.taunt = 2;
                anim(env, u);
                say(env, u, Str::PkmnFellForTaunt, u.target);
            }
        }
        E::Spite => spite(b, env, u),
        E::DestinyBond => {
            b.attack_string(env, u);
            b.battler_mut(u.attacker).fx.destiny_bond = true;
            anim(env, u);
            say(env, u, Str::PkmnTryingToTakeFoe, u.attacker);
        }
        E::LockOn => {
            // BattleScript_EffectLockOn + `setalwayshitflag` (STATUS3_ALWAYS_HITS_TURN(2)).
            b.attack_string(env, u);
            if super::sub::subbed(b, u) {
                b.fail(env, u);
            } else if !b.accuracy(env, u) {
                b.miss_message(env, u);
            } else {
                b.battler_mut(u.target).fx.lock_on = 2;
                anim(env, u);
                say(env, u, Str::PkmnTookAim, u.attacker);
            }
        }
        E::PerishSong => perish_song(b, env, u),
        _ => return false,
    }
    true
}

/// A `NO_ACC_CALC_CHECK_LOCK_ON` failure: a protected target's message, else "But it failed!".
pub(super) fn no_acc_fail(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    if protected(b, u) {
        b.miss_message(env, u);
    } else {
        b.fail(env, u);
    }
}

/// `statbuffchange STAT_CHANGE_ALLOW_PTR` lowering `side`'s `stat` by `n` from a status move:
/// Mist and Clear Body / Keen Eye say so once per move, the limit is quiet. True when it fell.
pub(super) fn drop(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    stat: Stat,
    n: i8,
) -> bool {
    if super::field::mist_blocks(b, env, u, side, true) {
        return false;
    }
    let lowered = b.fx.cur.is_some_and(|c| c.lowered[side.i()]);
    if lowered && abilities::blocks_secondary_drop(b, side, stat) {
        return false;
    }
    if abilities::blocks_stat_drop(b, env, u, side, stat) {
        if let Some(c) = &mut b.fx.cur {
            c.lowered[side.i()] = true;
        }
        return false;
    }
    stage(b, env, u, side, stat, -n, false)
}

/// `BattleScript_EffectStockpile`: up to 3.
fn stockpile(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let n = b.battler(u.attacker).fx.stockpile;
    if n >= 3 {
        b.no_effect = true;
        say(env, u, Str::PkmnCantStockpile, u.attacker);
        return;
    }
    b.battler_mut(u.attacker).fx.stockpile = n + 1;
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnStockpiled,
        u.attacker,
        [Buff::Number(n as u32 + 1), Buff::None, Buff::None],
    );
}

/// `BattleScript_EffectSwallow` + `stockpiletohpheal`: max HP / 4, / 2, or all.
fn swallow(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = *b.battler(a);
    if m.fx.stockpile == 0 {
        b.no_effect = true;
        say(env, u, Str::FailedToSwallow, a);
        return;
    }
    b.battler_mut(a).fx.stockpile = 0;
    if m.mon.hp == m.mon.max_hp {
        say(env, u, Str::PkmnHpFull, a);
        return;
    }
    let heal = (m.mon.max_hp / (1 << (3 - m.fx.stockpile.min(3)))).max(1) as i32;
    heal_target(b, env, u, a, heal);
}

/// `BattleScript_PresentHealTarget` on `side`.
fn heal_target(b: &mut Battle, env: &mut Env, u: &MoveUse, side: Side, heal: i32) {
    anim(env, u);
    b.change_hp_passive(env, side, -heal);
    let u = MoveUse { target: side, ..*u };
    say(env, &u, Str::PkmnRegainedHealth, side);
}

/// `BattleScript_EffectSwagger` / `Flatter`: accuracy, then the target's Attack +2 / Sp. Atk +1
/// and its confusion (`PkmnUsedSafeguard` under Safeguard).
fn swagger(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    // `jumpifstatus2 BS_TARGET, STATUS2_SUBSTITUTE, BattleScript_MakeMoveMissed`.
    if super::sub::subbed(b, u) {
        b.missed(env, u);
        return;
    }
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return;
    }
    b.attack_string(env, u);
    let (stat, n) = if u.info.effect == MoveEffect::Swagger {
        (Stat::Atk, 2)
    } else {
        (Stat::SpAtk, 1)
    };
    let t = b.battler(u.target);
    if t.confusion > 0 && t.stages[stat as usize] >= calc::MAX_STAGE {
        b.fail(env, u);
        return;
    }
    anim(env, u);
    stage(b, env, u, u.target, stat, n, false);
    if safeguarded(b, u.target) {
        say(env, u, Str::PkmnUsedSafeguard, u.target);
        return;
    }
    status::confuse(b, env, u, u.target, Cause::PRIMARY);
}

/// `BattleScript_EffectTeeterDance` on the foe (singles).
/// Doubles: every battler but the user, in battler order (the script's own loop, without
/// `moveend` per target).
fn teeter_dance(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if b.is_double() {
        for t in b.sides().filter(|&s| s != u.attacker) {
            teeter_one(b, env, &MoveUse { target: t, ..*u });
        }
    } else {
        teeter_one(b, env, u);
    }
}

fn teeter_one(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let t = u.target;
    // BattleScript_TeeterDanceSubstitutePrevents (after Own Tempo, which `status::confuse`
    // reports).
    if super::sub::subbed(b, u) && b.battler(t).ability != Ability::OwnTempo {
        b.fail(env, u);
        return;
    }
    if b.battler(t).confusion > 0 {
        say(env, u, Str::PkmnAlreadyConfused, t);
        return;
    }
    if b.battler(t).mon.hp == 0 {
        return;
    }
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    if safeguarded(b, t) {
        say(env, u, Str::PkmnUsedSafeguard, t);
        return;
    }
    anim(env, u);
    status::confuse(b, env, u, t, Cause::PRIMARY);
}

/// `BattleScript_EffectNightmare`.
fn nightmare(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let t = b.battler(u.target);
    if super::sub::subbed(b, u) || t.fx.nightmare || !matches!(t.mon.status, Status::Sleep(_)) {
        b.fail(env, u);
        return;
    }
    anim(env, u);
    b.battler_mut(u.target).fx.nightmare = true;
    say(env, u, Str::PkmnFellIntoNightmare, u.target);
}

/// `BattleScript_EffectCurse`: a Ghost user lays a curse on the foe for half its max HP;
/// another user trades a Speed stage for Attack and Defense.
fn curse(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let a = u.attacker;
    if b.battler(a).types.contains(&Type::Ghost) {
        let u = &MoveUse {
            target: a.other(),
            ..*u
        };
        b.attack_string(env, u);
        if super::sub::subbed(b, u) {
            b.fail(env, u);
            return;
        }
        if !no_acc_check(b, u) {
            no_acc_fail(b, env, u);
            return;
        }
        if b.battler(u.target).fx.cursed {
            b.fail(env, u);
            return;
        }
        b.battler_mut(u.target).fx.cursed = true;
        let damage = (b.battler(a).mon.max_hp / 2).max(1) as i32;
        anim(env, u);
        b.change_hp(env, a, damage);
        say(env, u, Str::PkmnLaidCurse, a);
        b.try_faint(env, a);
        return;
    }
    let u = &MoveUse { target: a, ..*u };
    b.attack_string(env, u);
    let s = b.battler(a).stages;
    if s[Stat::Speed as usize] <= calc::MIN_STAGE
        && s[Stat::Atk as usize] >= calc::MAX_STAGE
        && s[Stat::Def as usize] >= calc::MAX_STAGE
    {
        b.fail(env, u);
        return;
    }
    anim(env, u);
    stage(b, env, u, a, Stat::Speed, -1, true);
    stage(b, env, u, a, Stat::Atk, 1, true);
    stage(b, env, u, a, Stat::Def, 1, true);
}

/// `BattleScript_EffectRest` + `trysetrest`: 3 turns of sleep, then the HP to max.
fn rest(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = *b.battler(a);
    if matches!(m.mon.status, Status::Sleep(_)) {
        b.no_effect = true;
        say(env, u, Str::PkmnAlreadyAsleep2, a);
        return;
    }
    // `jumpifcantmakeasleep` → BattleScript_RestCantSleep (gUproarAwakeStringIds).
    if let Some(uproarer) = status::uproar_keeps_awake(b, a) {
        let id = if uproarer == u.target {
            Str::PkmnCantSleepInUproar
        } else {
            Str::UproarKeptPkmnAwake
        };
        say(env, u, id, u.target);
        return;
    }
    if status::never_sleeps(m.ability) {
        say_with(
            env,
            u,
            Str::PkmnStayedAwakeUsing,
            u.target,
            [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(m.ability)),
            ],
        );
        return;
    }
    if m.mon.hp == m.mon.max_hp {
        say(env, u, Str::PkmnHpFull, a);
        return;
    }
    let id = if m.mon.status != Status::None {
        Str::PkmnSleptHealthy
    } else {
        Str::PkmnWentToSleep
    };
    let status = Status::Sleep(3);
    b.battler_mut(a).mon.status = status;
    b.write_back(a);
    say(env, u, id, a);
    env.out.push(BattleEvent::StatusIcon { side: a, status });
    heal_target(b, env, u, a, m.mon.max_hp as i32);
}

/// Recover / Softboiled (`tryhealhalfhealth`) and Morning Sun / Synthesis / Moonlight
/// (`recoverbasedonsunlight`).
fn restore_hp(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = b.battler(a);
    if m.mon.hp == m.mon.max_hp {
        say(env, u, Str::PkmnHpFull, a);
        return;
    }
    let max = m.mon.max_hp as u32;
    let heal = if u.info.effect == MoveEffect::RestoreHp {
        max / 2
    } else {
        match b.weather() {
            Weather::None => max / 2,
            w if w.sun() => 20 * max / 30,
            _ => max / 4,
        }
    };
    heal_target(b, env, u, a, heal.max(1) as i32);
}

/// `BattleScript_EffectHealBell` + `healpartystatus`: the party's statuses are cured (Heal
/// Bell: not a Soundproof mon's; the user's Soundproof says it blocks the move).
fn heal_bell(b: &mut Battle, env: &mut Env, u: &MoveUse, aroma: bool) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = *b.battler(a);
    let blocked = !aroma && m.ability == Ability::Soundproof;
    let side = a.s();
    for i in 0..b.parties[side].len() {
        let Some(p) = b.parties[side].get(i) else {
            continue;
        };
        let ability = if i == m.mon.party as usize {
            m.ability
        } else {
            p.ability(env.t)
        };
        if (aroma || ability != Ability::Soundproof)
            && let Some(p) = b.parties[side].get_mut(i)
        {
            p.status = Status::None;
        }
    }
    if !blocked {
        let me = b.battler_mut(a);
        me.mon.status = Status::None;
        me.fx.nightmare = false;
    }
    b.write_back(a);
    anim(env, u);
    let id = if aroma {
        Str::SoothingAroma
    } else {
        Str::BellChimed
    };
    say(env, u, id, a);
    if blocked {
        say_with(
            env,
            u,
            Str::PkmnSXBlocksY,
            u.target,
            [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(Ability::Soundproof)),
            ],
        );
    }
    env.out.push(BattleEvent::StatusIcon {
        side: a,
        status: b.battler(a).mon.status,
    });
}

/// `BattleScript_EffectRefresh`: burn, paralysis and poison.
fn refresh(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    if !matches!(
        b.battler(a).mon.status,
        Status::Poison | Status::Burn | Status::Paralysis | Status::Toxic(_)
    ) {
        b.fail(env, u);
        return;
    }
    b.battler_mut(a).mon.status = Status::None;
    b.write_back(a);
    anim(env, u);
    say(env, u, Str::PkmnStatusNormal, a);
    env.out.push(BattleEvent::StatusIcon {
        side: a,
        status: Status::None,
    });
}

/// `BattleScript_EffectPainSplit`: both to the average HP (the user's bar first).
fn pain_split(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if !no_acc_check(b, u) {
        no_acc_fail(b, env, u);
        return;
    }
    let (ha, ht) = (
        b.battler(u.attacker).mon.hp as i32,
        b.battler(u.target).mon.hp as i32,
    );
    let avg = (ha + ht) / 2;
    anim(env, u);
    b.change_hp_passive(env, u.attacker, ha - avg);
    b.change_hp_passive(env, u.target, ht - avg);
    say(env, u, Str::SharedPain, u.attacker);
}

/// `BattleScript_EffectBellyDrum` + `maxattackhalvehp`.
fn belly_drum(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = b.battler(a);
    let half = (m.mon.max_hp / 2).max(1);
    if m.stages[Stat::Atk as usize] >= calc::MAX_STAGE || m.mon.hp <= half {
        b.fail(env, u);
        return;
    }
    b.battler_mut(a).stages[Stat::Atk as usize] = calc::MAX_STAGE;
    anim(env, u);
    b.change_hp(env, a, half as i32);
    say(env, u, Str::PkmnCutHpMaxedAttack, a);
}

/// Cosmic Power, Bulk Up, Calm Mind, Dragon Dance: both stats +1 (quiet at +6); both at +6:
/// `BattleScript_CantRaiseMultipleStats`.
fn stats_up(b: &mut Battle, env: &mut Env, u: &MoveUse, x: Stat, y: Stat) {
    b.attack_string(env, u);
    let a = u.attacker;
    let s = b.battler(a).stages;
    if s[x as usize] >= calc::MAX_STAGE && s[y as usize] >= calc::MAX_STAGE {
        b.no_effect = true;
        say(env, u, Str::StatsWontIncrease2, a);
        return;
    }
    anim(env, u);
    env.out.push(BattleEvent::StatAnim { side: a, up: true });
    for stat in [x, y] {
        if b.change_stage(a, stat, 1) {
            env.out
                .push(BattleEvent::Msg(b.stat_msg(u, a, stat, 1, true)));
        }
    }
}

/// `BattleScript_EffectTickle`: the target's Attack and Defense -1.
fn tickle(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let t = u.target;
    let s = b.battler(t).stages;
    if s[Stat::Atk as usize] <= calc::MIN_STAGE && s[Stat::Def as usize] <= calc::MIN_STAGE {
        b.no_effect = true;
        say(env, u, Str::StatsWontDecrease2, t);
        return;
    }
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    anim(env, u);
    drop(b, env, u, t, Stat::Atk, 1);
    drop(b, env, u, t, Stat::Def, 1);
}

/// `BattleScript_EffectMemento`: the user faints, the target's Attack and Sp. Atk -2.
fn memento(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let a = u.attacker;
    b.attack_string(env, u);
    let hp = b.battler(a).mon.hp as i32;
    if protected(b, u) {
        b.change_hp_passive(env, a, hp);
        b.miss_message(env, u);
        b.try_faint(env, a);
        return;
    }
    let s = b.battler(u.target).stages;
    if s[Stat::Atk as usize] <= calc::MIN_STAGE && s[Stat::SpAtk as usize] <= calc::MIN_STAGE {
        b.fail(env, u);
        return;
    }
    b.change_hp_passive(env, a, hp);
    anim(env, u);
    if super::sub::subbed(b, u) {
        // BattleScript_EffectMementoPrintNoEffect.
        say(env, u, Str::ButNoEffect, a);
    } else {
        drop(b, env, u, u.target, Stat::Atk, 2);
        drop(b, env, u, u.target, Stat::SpAtk, 2);
    }
    b.try_faint(env, a);
}

/// `tryconversiontypechange`: a random move's type (Mystery: Ghost for a Ghost, else Normal)
/// the user is not yet.
fn conversion(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let m = *b.battler(a);
    let valid = m.mon.moves.iter().take_while(|x| x.is_some()).count();
    let ty = |i: usize| {
        let t = m.mon.moves[i].map_or(Type::Normal, |mv| env.t.move_info(mv).ty);
        match t {
            Type::Mystery if m.types.contains(&Type::Ghost) => Type::Ghost,
            Type::Mystery => Type::Normal,
            t => t,
        }
    };
    if (0..valid).all(|i| m.types.contains(&ty(i))) {
        b.fail(env, u);
        return;
    }
    let new = loop {
        let i = (env.rng.random() % 4) as usize;
        if i >= valid {
            continue;
        }
        let t = ty(i);
        if !m.types.contains(&t) {
            break t;
        }
    };
    set_type(b, env, u, new);
}

fn set_type(b: &mut Battle, env: &mut Env, u: &MoveUse, ty: Type) {
    b.battler_mut(u.attacker).types = [ty, ty];
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnChangedType,
        u.attacker,
        [Buff::Type(ty), Buff::None, Buff::None],
    );
}

/// `settypetoenvironment` (`sEnvironmentToType`).
fn camouflage(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let ty = match b.setup().environment {
        Environment::Grass | Environment::LongGrass => Type::Grass,
        Environment::Sand => Type::Ground,
        Environment::Underwater | Environment::Water | Environment::Pond => Type::Water,
        Environment::Mountain | Environment::Cave => Type::Rock,
        Environment::Building | Environment::Plain => Type::Normal,
    };
    if b.battler(u.attacker).types.contains(&ty) {
        b.fail(env, u);
        return;
    }
    set_type(b, env, u, ty);
}

/// The slot of the target's last move (`gLastMoves`) among its moves.
fn last_slot(b: &Battle, side: Side) -> Option<usize> {
    let m = b.battler(side);
    let last = m.last_move?;
    (0..4).find(|&i| m.mon.moves[i] == Some(last))
}

/// `BattleScript_EffectDisable` + `disablelastusedattack`: `(Random() & 3) + 2` turns.
fn disable(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    let t = u.target;
    let slot = last_slot(b, t).filter(|&i| b.battler(t).mon.pp[i] > 0);
    let (Some(i), None) = (slot, b.battler(t).fx.disabled) else {
        b.fail(env, u);
        return;
    };
    let mov = b.battler(t).mon.moves[i];
    let timer = (env.rng.random() & 3) as u8 + 2;
    let fx = &mut b.battler_mut(t).fx;
    fx.disabled = mov;
    fx.disable_timer = timer;
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnMoveWasDisabled,
        t,
        mov.map_or([Buff::None; 3], super::move_buff),
    );
}

/// `BattleScript_EffectEncore` + `trysetencore`: `(Random() & 3) + 3` turns; not Struggle,
/// Encore nor Mirror Move.
fn encore(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return;
    }
    b.attack_string(env, u);
    let t = u.target;
    let m = b.battler(t);
    let excluded = m.last_move.is_some_and(|mv| {
        mv == env.t.struggle
            || matches!(
                env.t.move_info(mv).effect,
                MoveEffect::Encore | MoveEffect::MirrorMove
            )
    });
    let slot = last_slot(b, t).filter(|&i| !excluded && m.mon.pp[i] > 0);
    let (Some(i), None) = (slot, m.fx.encore) else {
        b.fail(env, u);
        return;
    };
    let Some(mov) = m.mon.moves[i] else {
        b.fail(env, u);
        return;
    };
    let timer = (env.rng.random() & 3) as u8 + 3;
    let fx = &mut b.battler_mut(t).fx;
    fx.encore = Some((mov, i as u8));
    fx.encore_timer = timer;
    anim(env, u);
    say(env, u, Str::PkmnGotEncore, t);
}

/// `BattleScript_EffectSpite` + `tryspiteppreduce`: `(Random() & 3) + 2` PP of the last move.
fn spite(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    let t = u.target;
    let Some(i) = last_slot(b, t).filter(|&i| b.battler(t).mon.pp[i] > 1) else {
        b.fail(env, u);
        return;
    };
    let pp = b.battler(t).mon.pp[i];
    let n = ((env.rng.random() & 3) as u8 + 2).min(pp);
    b.battler_mut(t).mon.pp[i] = pp - n;
    b.write_back(t);
    if pp == n {
        cancel_all(b, t);
    }
    let mov = b.battler(t).mon.moves[i];
    anim(env, u);
    say_with(
        env,
        u,
        Str::PkmnReducedPP,
        t,
        [
            mov.map_or(Buff::None, Buff::Move),
            Buff::Number(n as u32),
            Buff::None,
        ],
    );
}

/// `BattleScript_EffectPerishSong` + `trysetperishsong`: every battler without a count nor
/// Soundproof gets 3; Soundproof battlers say they block it.
fn perish_song(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let mut affected = false;
    for s in b.sides() {
        let m = b.battler_mut(s);
        if m.fx.perish.is_none() && m.ability != Ability::Soundproof {
            m.fx.perish = Some(3);
            affected = true;
        }
    }
    if !affected {
        b.fail(env, u);
        return;
    }
    anim(env, u);
    say(env, u, Str::FaintInThree, u.attacker);
    for s in b.sides() {
        if b.battler(s).ability == Ability::Soundproof {
            say_with(
                env,
                u,
                Str::PkmnSXBlocksY2,
                s,
                [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(Ability::Soundproof)),
                ],
            );
        }
    }
}
