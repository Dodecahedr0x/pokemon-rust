//! The damaging scripts of Wave C (`data/battle_scripts_1.s`, `battle_script_commands.c`).

use super::super::abilities;
use super::super::status::{self, Cause};
use super::{
    Future, anim, cancel_all, certain, chance, no_acc_check, protected, say, say_with, stage,
};
use crate::battle::calc;
use crate::battle::core::{Battle, Env, HitResult, MoveUse, Secondary};
use crate::battle::{BattleEvent, Buff, Effectiveness, Environment, Side, Str};
use crate::mon::{
    Ability, HoldEffect, MoveEffect, SemiKind, Stat, Status, StatusKind, TrapKind, TwoTurnKind,
    Type,
};

fn result(damage: i32, eff: Effectiveness, hung: bool) -> HitResult {
    HitResult {
        damage,
        hp_dealt: 0,
        crit: false,
        eff,
        hung,
    }
}

/// `ppreduce` without `attackstring` (a charging turn, Focus Punch's lost focus).
pub(super) fn pay_pp(b: &mut Battle, u: &MoveUse) {
    if u.printed {
        return;
    }
    if let Some(s) = u.slot {
        let pp = &mut b.battler_mut(u.attacker).mon.pp[s as usize];
        *pp = pp.saturating_sub(1);
        b.write_back(u.attacker);
    }
}

/// `BattleScript_HitFromAtkAnimation` on a computed hit: the animations, the HP, the result
/// message, `seteffectwithchance` and `tryfaintmon`.
fn from_anim(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    r: HitResult,
    sec: Option<Secondary>,
) -> HitResult {
    let r = b.deal(env, u, r);
    b.secondary(env, u, &r, sec);
    b.try_faint(env, u.target);
    r
}

fn in_air(semi: Option<SemiKind>) -> bool {
    matches!(semi, Some(SemiKind::Fly | SemiKind::Bounce))
}

/// `BattleScript_EffectHit` with a `setmoveeffect`, a damage multiplier or a power formula.
pub(super) fn plain(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    use MoveEffect as E;
    let mut u = *u;
    let a = *b.battler(u.attacker);
    let t = *b.battler(u.target);
    let sec = match u.info.effect {
        E::HighCritical(Some(_)) | E::Thunder | E::ThawHit | E::PoisonFang => chance(status_sec),
        E::TriAttack => chance(tri_attack),
        // BattleScript_EffectGust / Twister: double damage against a flying target.
        E::Gust => {
            if in_air(t.fx.semi) {
                u.multiplier = 2;
            }
            None
        }
        E::Twister => {
            if in_air(t.fx.semi) {
                u.multiplier = 2;
            }
            chance(flinch)
        }
        E::StatUpHit(_) => chance(stat_up_sec),
        E::AllStatsUpHit => chance(all_stats_up),
        E::FakeOut => {
            // `jumpifnotfirstturn BattleScript_FailedFromAtkString`.
            if a.x.first_turn == 0 {
                b.attack_string(env, &u);
                b.fail(env, &u);
                return None;
            }
            certain(flinch_certain)
        }
        E::PayDay => chance(pay_day),
        E::RapidSpin => certain(rapid_spin),
        E::SecretPower => match b.setup().environment {
            Environment::Grass | Environment::LongGrass => chance(status_sec),
            Environment::Mountain => chance(confuse_sec),
            Environment::Cave => chance(flinch),
            Environment::Sand
            | Environment::Underwater
            | Environment::Water
            | Environment::Pond => chance(stat_down_sec),
            _ => chance(status_sec),
        },
        E::Superpower => certain(superpower),
        E::Overheat => certain(overheat),
        E::KnockOff => chance(knock_off),
        E::Thief => chance(thief),
        // `jumpifstatus2 BS_TARGET, STATUS2_SUBSTITUTE, BattleScript_EffectHit`.
        E::SmellingSalt if t.fx.substitute => None,
        E::SmellingSalt => {
            if t.mon.status == Status::Paralysis {
                u.multiplier = 2;
            }
            certain(remove_paralysis)
        }
        E::Facade => {
            if matches!(
                a.mon.status,
                Status::Poison | Status::Burn | Status::Paralysis | Status::Toxic(_)
            ) {
                u.multiplier = 2;
            }
            None
        }
        // `doubledamagedealtifdamaged`: the target damaged the attacker this turn.
        E::Revenge => {
            let rec = a.fx.rec;
            let by = |r: Option<(u16, Side)>| r.is_some_and(|(_, s)| s == u.target);
            if by(rec.physical) || by(rec.special) {
                u.multiplier = 2;
            }
            None
        }
        E::DoubleEdge => certain(recoil_33),
        E::Recharge => certain(recharge),
        E::Trap(kind) => {
            if kind == TrapKind::Whirlpool && t.fx.semi == Some(SemiKind::Dive) {
                u.multiplier = 2;
            }
            chance(wrap)
        }
        E::Flail => {
            // `remaininghptopower` (`GetScaledHPFraction` out of 48).
            let mut f = (a.mon.hp as u32 * 48 / a.mon.max_hp.max(1) as u32) as u8;
            if f == 0 && a.mon.hp > 0 {
                f = 1;
            }
            u.power = match f {
                0..=1 => 200,
                2..=4 => 150,
                5..=9 => 100,
                10..=16 => 80,
                17..=32 => 40,
                _ => 20,
            };
            None
        }
        E::Eruption => {
            // `scaledamagebyhealthratio`.
            let p = a.mon.hp as u32 * u.info.power as u32 / a.mon.max_hp.max(1) as u32;
            u.power = p.max(1) as u8;
            None
        }
        E::Return | E::Frustration => {
            // `friendshiptodamagecalculation` (`gDynamicBasePower` 0 keeps the table power).
            let f = b.parties[u.attacker as usize]
                .get(a.mon.party as usize)
                .map_or(0, |p| p.friendship) as u32;
            let f = if u.info.effect == E::Return {
                f
            } else {
                255 - f
            };
            let p = 10 * f / 25;
            if p > 0 {
                u.power = p as u8;
            }
            None
        }
        E::HiddenPower => {
            let ivs = b.parties[u.attacker as usize]
                .get(a.mon.party as usize)
                .map_or([0; 6], |p| p.ivs);
            let (power, ty) = hidden_power(ivs);
            u.power = power;
            u.ty = ty;
            None
        }
        E::WeatherBall => {
            use crate::battle::core::Weather as W;
            let w = b.weather();
            if w != W::None {
                u.multiplier = 2;
            }
            u.ty = match w {
                _ if w.rain() => Type::Water,
                W::Sand => Type::Rock,
                _ if w.sun() => Type::Fire,
                W::Hail => Type::Ice,
                _ => u.ty,
            };
            None
        }
        E::Rage => {
            // BattleScript_EffectRage: the miss clears STATUS2_RAGE, the hit sets it.
            if !b.accuracy(env, &u) {
                b.battler_mut(u.attacker).fx.rage = false;
                b.missed(env, &u);
                return None;
            }
            b.battler_mut(u.attacker).fx.rage = true;
            return b.hit(env, &u, None, true);
        }
        _ => None,
    };
    b.hit(env, &u, sec, false)
}

/// `hiddenpowercalc`: the power and type from the IVs' low bits (`[hp, atk, def, spe, spa,
/// spd]`), skipping Normal and Mystery.
pub(super) fn hidden_power(ivs: [u8; 6]) -> (u8, Type) {
    let order = [0usize, 1, 2, 3, 4, 5];
    let mut power_bits = 0u32;
    let mut type_bits = 0u32;
    for (i, &k) in order.iter().enumerate() {
        power_bits |= (((ivs[k] & 2) >> 1) as u32) << i;
        type_bits |= ((ivs[k] & 1) as u32) << i;
    }
    let power = (40 * power_bits / 63 + 30) as u8;
    let mut ty = (15 * type_bits / 63 + 1) as u8;
    if ty >= Type::Mystery as u8 {
        ty += 1;
    }
    (power, Type::from_u8(ty).unwrap_or(Type::Normal))
}

// ---- secondary effects (`SetMoveEffect`) ----

fn status_sec(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    use MoveEffect as E;
    let kind = match u.info.effect {
        E::HighCritical(Some(k)) => k,
        E::Thunder | E::SemiInvulnerable(SemiKind::Bounce) => StatusKind::Paralysis,
        E::ThawHit => StatusKind::Burn,
        E::PoisonFang => StatusKind::Toxic,
        E::SecretPower => match b.setup().environment {
            Environment::Grass => StatusKind::Poison,
            Environment::LongGrass => StatusKind::Sleep,
            _ => StatusKind::Paralysis,
        },
        _ => return,
    };
    status::set_status(b, env, u, u.target, kind, Cause::SECONDARY);
}

/// `MOVE_EFFECT_TRI_ATTACK`: nothing on a statused target; else `Random() % 3`.
fn tri_attack(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if b.battler(u.target).mon.status != Status::None {
        return;
    }
    let kind = match env.rng.random() % 3 {
        0 => StatusKind::Burn,
        1 => StatusKind::Freeze,
        _ => StatusKind::Paralysis,
    };
    status::set_status(b, env, u, u.target, kind, Cause::SECONDARY);
}

fn flinch(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::flinch(b, env, u, u.target, Cause::SECONDARY, false);
}

fn flinch_certain(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::flinch(b, env, u, u.target, Cause::SECONDARY, true);
}

fn confuse_sec(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::confuse(b, env, u, u.target, Cause::SECONDARY);
}

/// Secret Power's stat drop by environment (`MOVE_EFFECT_*_MINUS_1`): silent at -6, under Mist
/// or a blocking ability.
fn stat_down_sec(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let stat = match b.setup().environment {
        Environment::Sand => Stat::Accuracy,
        Environment::Underwater => Stat::Def,
        Environment::Water => Stat::Atk,
        _ => Stat::Speed,
    };
    let side = u.target;
    if b.battler(side).mon.hp == 0
        || super::field::mist_blocks(b, env, u, side, false)
        || abilities::blocks_secondary_drop(b, side, stat)
    {
        return;
    }
    stage(b, env, u, side, stat, -1, false);
}

/// `MOVE_EFFECT_ATK_PLUS_1` / `DEF_PLUS_1` on the attacker: silent at +6.
fn stat_up_sec(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if let MoveEffect::StatUpHit(stat) = u.info.effect {
        stage(b, env, u, u.attacker, stat, 1, false);
    }
}

/// `BattleScript_AllStatsUp`: one animation, then each stat +1 ("won't go higher" at +6);
/// nothing when all are at +6.
fn all_stats_up(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let side = u.attacker;
    let stats = [Stat::Atk, Stat::Def, Stat::Speed, Stat::SpAtk, Stat::SpDef];
    if stats
        .iter()
        .all(|&s| b.battler(side).stages[s as usize] >= calc::MAX_STAGE)
    {
        return;
    }
    env.out.push(BattleEvent::StatAnim { side, up: true });
    for stat in stats {
        if b.change_stage(side, stat, 1) {
            env.out
                .push(BattleEvent::Msg(b.stat_msg(u, side, stat, 1, true)));
        } else {
            say_with(
                env,
                u,
                Str::StatWontIncrease,
                side,
                [Buff::Stat(stat), Buff::None, Buff::None],
            );
        }
    }
}

/// `MOVE_EFFECT_PAYDAY`: the player's attacker adds level x 5 to `gPaydayMoney` (0xFFFF on
/// overflow), paid after a win; "Coins scattered everywhere!".
fn pay_day(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if u.attacker == Side::Player {
        let level = b.battler(u.attacker).mon.level as u16;
        b.fx.pay_day = b.fx.pay_day.saturating_add(level * 5);
    }
    say(env, u, Str::CoinsScattered, u.attacker);
}

/// `rapidspinfree`: the first of the wrap, Leech Seed, then Spikes on the attacker's side.
fn rapid_spin(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let a = u.attacker;
    let m = b.battler(a);
    if m.fx.wrapped > 0 {
        let mov = m.fx.wrap_move;
        let fx = &mut b.battler_mut(a).fx;
        fx.wrapped = 0;
        fx.wrap_move = None;
        say_with(
            env,
            u,
            Str::PkmnGotFree,
            a,
            mov.map_or([Buff::None; 3], super::move_buff),
        );
    } else if m.seeded {
        b.battler_mut(a).seeded = false;
        say(env, u, Str::PkmnShedLeechSeed, a);
    } else if b.fx.spikes[a as usize] > 0 {
        b.fx.spikes[a as usize] = 0;
        say(env, u, Str::PkmnBlewAwaySpikes, a);
    }
}

/// `BattleScript_AtkDefDown`: the attacker's Attack, then Defense -1 (quiet at -6).
fn superpower(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    stage(b, env, u, u.attacker, Stat::Atk, -1, false);
    stage(b, env, u, u.attacker, Stat::Def, -1, false);
}

/// `BattleScript_SAtkDown2`.
fn overheat(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    stage(b, env, u, u.attacker, Stat::SpAtk, -2, false);
}

/// `MOVE_EFFECT_KNOCK_OFF`: the target's item is gone for the battle (`knockedOffMons`).
fn knock_off(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let t = u.target;
    let Some(item) = b.battler(t).x.item else {
        return;
    };
    if abilities::sticky_hold(b, t) {
        super::ability_blocks(env, u, Str::PkmnSXMadeYIneffective, Ability::StickyHold);
        return;
    }
    let party = b.battler(t).mon.party;
    let m = b.battler_mut(t);
    m.x.item = None;
    m.x.hold = None;
    b.fx.knocked_off[t as usize] |= 1 << party;
    say_with(
        env,
        u,
        Str::PkmnKnockedOff,
        u.attacker,
        [Buff::None, Buff::None, Buff::Item(item)],
    );
}

/// `MOVE_EFFECT_STEAL_ITEM`: the player's mon only (an opponent never steals), not knocked
/// off; Sticky Hold; then without an item of its own, and not Mail nor the Enigma Berry; the
/// target loses its item.
fn thief(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let (a, t) = (u.attacker, u.target);
    if a == Side::Opponent {
        return;
    }
    let am = *b.battler(a);
    if b.fx.knocked_off[a as usize] & (1 << am.mon.party) != 0 {
        return;
    }
    let tm = *b.battler(t);
    if tm.x.item.is_some() && abilities::sticky_hold(b, t) {
        super::ability_blocks(env, u, Str::PkmnSXMadeYIneffective, Ability::StickyHold);
        return;
    }
    // Not with an item of its own, nor Mail or the Enigma Berry (`HoldEffect::NoSwap`).
    let Some(item) = tm.x.item else {
        return;
    };
    if am.x.item.is_some() || tm.x.hold == Some(HoldEffect::NoSwap) {
        return;
    }
    let hold = tm.x.hold;
    for (side, party, item, hold) in [
        (a, am.mon.party, Some(item), hold),
        (t, tm.mon.party, None, None),
    ] {
        let m = b.battler_mut(side);
        m.x.item = item;
        m.x.hold = hold;
        b.x.holds[side as usize][party as usize] = hold;
        if let Some(p) = b.parties[side as usize].get_mut(party as usize) {
            p.held = item;
        }
    }
    say_with(
        env,
        u,
        Str::PkmnStoleItem,
        a,
        [Buff::None, Buff::None, Buff::Item(item)],
    );
}

/// `MOVE_EFFECT_REMOVE_PARALYSIS` (SmellingSalt).
fn remove_paralysis(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let t = u.target;
    if b.battler(t).mon.status != Status::Paralysis || b.battler(t).mon.hp == 0 {
        return;
    }
    b.battler_mut(t).mon.status = Status::None;
    b.write_back(t);
    say(env, u, Str::PkmnHealedParalysis, t);
    env.out.push(BattleEvent::StatusIcon {
        side: t,
        status: Status::None,
    });
}

/// `MOVE_EFFECT_RECOIL_33`: a third of the HP dealt (at least 1), Rock Head prevents it.
fn recoil_33(b: &mut Battle, env: &mut Env, u: &MoveUse, r: &HitResult) {
    if b.battler(u.attacker).ability == Ability::RockHead {
        return;
    }
    let damage = (r.hp_dealt / 3).max(1);
    b.change_hp(env, u.attacker, damage as i32);
    say(env, u, Str::PkmnHitWithRecoil, u.attacker);
    b.try_faint(env, u.attacker);
}

/// `MOVE_EFFECT_RECHARGE`: `rechargeTimer` 2 (the next turn is lost).
fn recharge(b: &mut Battle, _env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let fx = &mut b.battler_mut(u.attacker).fx;
    fx.recharge = 2;
    fx.slot = u.slot.unwrap_or(fx.slot);
}

/// `MOVE_EFFECT_WRAP`: `(Random() & 3) + 3` turns unless already wrapped.
fn wrap(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    let t = u.target;
    let MoveEffect::Trap(kind) = u.info.effect else {
        return;
    };
    if b.battler(t).fx.wrapped > 0 || b.battler(t).mon.hp == 0 {
        return;
    }
    let turns = (env.rng.random() & 3) as u8 + 3;
    let fx = &mut b.battler_mut(t).fx;
    fx.wrapped = turns;
    fx.wrap_move = Some(u.mov);
    let id = match kind {
        TrapKind::Bind => Str::PkmnSqueezedByBind,
        TrapKind::Wrap => Str::PkmnWrappedBy,
        TrapKind::FireSpin | TrapKind::Whirlpool => Str::PkmnTrappedInVortex,
        TrapKind::Clamp => Str::PkmnClamped,
        TrapKind::SandTomb => Str::PkmnTrappedBySandTomb,
    };
    say(env, u, id, t);
}

// ---- the scripts ----

/// `BattleScript_EffectEarthquake` / `Magnitude`: the attack string first; Magnitude's roll
/// and message; double damage against a digging target; no secondary roll.
pub(super) fn earthquake(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    magnitude: bool,
) -> Option<HitResult> {
    b.attack_string(env, u);
    let mut u = *u;
    if magnitude {
        // `magnitudedamagecalculation`.
        let (m, power) = match env.rng.random() % 100 {
            0..5 => (4, 10),
            5..15 => (5, 30),
            15..35 => (6, 50),
            35..65 => (7, 70),
            65..85 => (8, 90),
            85..95 => (9, 110),
            _ => (10, 150),
        };
        u.power = power;
        say_with(
            env,
            &u,
            Str::MagnitudeStrength,
            u.attacker,
            [Buff::Number(m), Buff::None, Buff::None],
        );
    }
    if b.battler(u.target).fx.semi == Some(SemiKind::Dig) {
        u.multiplier = 2;
    }
    if !b.accuracy(env, &u) {
        b.miss_message(env, &u);
        return None;
    }
    let r = b.damage(env, &u);
    let r = b.deal(env, &u, r);
    b.try_faint(env, u.target);
    Some(r)
}

/// `BattleScript_EffectBrickBreak`: `removelightscreenreflect` on the target's side; the
/// animation plays (even against an immune target) when a wall shattered.
pub(super) fn brick_break(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let side = u.target as usize;
    let had = b.fx.reflect[side].is_some() || b.fx.light_screen[side].is_some();
    b.fx.reflect[side] = None;
    b.fx.light_screen[side] = None;
    let r = b.damage(env, u);
    if had || r.eff != Effectiveness::NoEffect {
        anim(env, u);
    }
    if had {
        say(env, u, Str::TheWallShattered, u.attacker);
    }
    let r = b.deal_with(env, u, r, false);
    b.secondary(env, u, &r, None);
    b.try_faint(env, u.target);
    Some(r)
}

/// `BattleScript_EffectRecoilIfMiss`: a miss (not on an immune target) crashes for half the
/// damage it would have dealt (no crit), at most half the target's max HP.
pub(super) fn recoil_if_miss(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if b.accuracy(env, u) {
        return b.hit(env, u, None, true);
    }
    b.attack_string(env, u);
    let immune = !protected(b, u) && (b.levitates(env, u) || b.immune(env, u));
    b.miss_message(env, u);
    if immune {
        return None;
    }
    say(env, u, Str::PkmnCrashed, u.attacker);
    let d = b.damage_calc(env, u, false);
    let (d, eff) = b.type_calc(env, u, d);
    let d = calc::random_damage(d, env.rng.random());
    let (d, _) = super::hold_on(b, env, u, d, eff);
    let cap = (b.battler(u.target).mon.max_hp / 2) as i32;
    let d = (d / 2).max(1).min(cap);
    b.change_hp(env, u.attacker, d);
    b.try_faint(env, u.attacker);
    None
}

/// `BattleScript_EffectRampage`: PP on the first turn only; `MOVE_EFFECT_THRASH` (2 or 3
/// turns) by chance when not locked yet.
pub(super) fn rampage(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    let sec = if b.battler(u.attacker).fx.rampage == 0 {
        chance(thrash)
    } else {
        None
    };
    b.hit(env, u, sec, true)
}

fn thrash(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    if b.battler(u.attacker).fx.rampage > 0 {
        return;
    }
    let turns = (env.rng.random() & 1) as u8 + 2;
    let fx = &mut b.battler_mut(u.attacker).fx;
    fx.lock = Some(u.mov);
    fx.slot = u.slot.unwrap_or(fx.slot);
    fx.rampage = turns;
}

/// `BattleScript_EffectTwineedle`: two hits, then the poison chance.
pub(super) fn twineedle(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    super::super::moves::multi_hit_with(b, env, u, Some(2), chance(poison_sec));
    None
}

fn poison_sec(b: &mut Battle, env: &mut Env, u: &MoveUse, _r: &HitResult) {
    status::set_status(b, env, u, u.target, StatusKind::Poison, Cause::SECONDARY);
}

/// `BattleScript_EffectTripleKick`: up to three hits of 10, 20, 30 power, each with its own
/// accuracy check; a miss or the target's faint stops it.
pub(super) fn triple_kick(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    let sleep_talk = super::via_sleep_talk(b);
    let mut hits = 0u32;
    let mut last: Option<HitResult> = None;
    let mut strings = true;
    let mut missed = false;
    for i in 1..=3u8 {
        if b.battler(u.attacker).mon.hp == 0 {
            strings = false;
            break;
        }
        if b.battler(u.target).mon.hp == 0
            || (!sleep_talk && matches!(b.battler(u.attacker).mon.status, Status::Sleep(_)))
        {
            break;
        }
        if !b.accuracy(env, u) {
            missed = true;
            break;
        }
        let ui = MoveUse {
            power: 10 * i,
            ..*u
        };
        let r = b.damage(env, &ui);
        if r.eff == Effectiveness::NoEffect {
            last = Some(r);
            break;
        }
        let r = b.deal_with(env, &ui, HitResult { ..r }, true);
        hits += 1;
        last = Some(r);
        abilities::move_end(b, env, &ui, Some(r));
        super::super::items::move_end(b, env, &ui, Some(r));
        if r.hung && super::endured(b, u.target) {
            break;
        }
    }
    if strings {
        match last {
            None if missed => b.miss_message(env, u),
            Some(r) if r.eff == Effectiveness::NoEffect => {
                b.no_effect = true;
                env.out
                    .push(BattleEvent::Msg(u.msg_long(Str::ItDoesntAffect, u.target)));
            }
            _ => {}
        }
        if hits > 0 && last.is_some_and(|r| r.eff != Effectiveness::NoEffect) {
            say_with(
                env,
                u,
                Str::HitXTimes,
                u.attacker,
                [Buff::Number(hits), Buff::None, Buff::None],
            );
        }
    }
    let r = last.unwrap_or(result(0, Effectiveness::Normal, false));
    b.secondary(env, u, &r, None);
    b.try_faint(env, u.target);
    None
}

/// `BattleScript_EffectDreamEater`: only on a sleeping target; half the HP dealt heals the
/// attacker.
pub(super) fn dream_eater(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    // BattleScript_DreamEaterNoEffect also behind a substitute.
    if super::sub::subbed(b, u) || !matches!(b.battler(u.target).mon.status, Status::Sleep(_)) {
        b.attack_string(env, u);
        b.no_effect = true;
        say(env, u, Str::PkmnWasntAffected, u.target);
        return None;
    }
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let r = b.damage(env, u);
    let r = b.deal(env, u, r);
    if r.eff != Effectiveness::NoEffect {
        let heal = (r.hp_dealt / 2).max(1) as i32;
        b.change_hp_passive(env, u.attacker, -heal);
        say(env, u, Str::PkmnDreamEaten, u.target);
    }
    b.try_faint(env, u.target);
    Some(r)
}

/// `BattleScript_EffectExplosion`: the user's HP drops to 0 first, the animation always plays,
/// then the damage and the accuracy check (a protected target is missed); the user faints
/// last. No secondary roll.
pub(super) fn explosion(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    // `faintifabilitynotdamp`: BattleScript_DampStopsExplosion.
    if abilities::damp(b, env, u) {
        b.no_effect = true;
        return None;
    }
    let hp = b.battler(u.attacker).mon.hp as i32;
    b.change_hp_passive(env, u.attacker, hp);
    anim(env, u);
    let r = b.damage(env, u);
    let hit = if b.accuracy(env, u) {
        let r = b.deal_with(env, u, r, false);
        b.try_faint(env, u.target);
        Some(r)
    } else {
        b.miss_message(env, u);
        None
    };
    b.try_faint(env, u.attacker);
    hit
}

/// `BattleScript_EffectOHKO` + `Cmd_tryKO`.
pub(super) fn ohko(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    if !no_acc_check(b, u) {
        if protected(b, u) {
            b.miss_message(env, u);
        } else {
            b.fail(env, u);
        }
        return None;
    }
    if b.immune(env, u) {
        b.battler_mut(u.attacker).v.unable = true;
        let r = from_anim(b, env, u, result(0, Effectiveness::NoEffect, false), None);
        return Some(r);
    }
    let t = *b.battler(u.target);
    let banded = match t.x.hold {
        Some(crate::mon::HoldEffect::FocusBand(p)) => env.rng.random() % 100 < p as u16,
        _ => false,
    };
    if t.ability == Ability::Sturdy {
        b.no_effect = true;
        say_with(
            env,
            u,
            Str::PkmnProtectedBy,
            u.target,
            [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(Ability::Sturdy)),
            ],
        );
        return None;
    }
    let (la, lt) = (b.battler(u.attacker).mon.level, t.mon.level);
    let hits = if t.fx.lock_on > 0 && la >= lt {
        true
    } else {
        let chance = u.info.accuracy as i32 + la as i32 - lt as i32;
        ((env.rng.random() % 100) as i32 + 1) < chance && la >= lt
    };
    if !hits {
        b.no_effect = true;
        if la >= lt {
            say(env, u, Str::AttackMissed, u.attacker);
        } else {
            say(env, u, Str::PkmnUnaffected, u.target);
        }
        return None;
    }
    let hp = t.mon.hp as i32;
    let held = t.fx.endured || banded;
    let r = result(if held { hp - 1 } else { hp }, Effectiveness::Normal, held);
    let r = b.deal(env, u, r);
    if !held {
        say(env, u, Str::OneHitKo, u.attacker);
    }
    b.secondary(env, u, &r, None);
    b.try_faint(env, u.target);
    Some(r)
}

/// Super Fang, Dragon Rage, Night Shade / Seismic Toss, Psywave: accuracy, then a set damage
/// (`typecalc` keeps only "no effect"); `adjustsetdamage` but for Super Fang.
pub(super) fn fixed(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let eff = if b.immune(env, u) {
        b.battler_mut(u.attacker).v.unable = true;
        Effectiveness::NoEffect
    } else {
        Effectiveness::Normal
    };
    let level = b.battler(u.attacker).mon.level as i32;
    let damage = match u.info.effect {
        MoveEffect::SuperFang => (b.battler(u.target).mon.hp as i32 / 2).max(1),
        MoveEffect::DragonRage => 40,
        MoveEffect::LevelDamage => level,
        _ => {
            // `psywavedamageeffect`.
            let mut n;
            loop {
                n = (env.rng.random() & 0xF) as i32;
                if n <= 10 {
                    break;
                }
            }
            level * (n * 10 + 50) / 100
        }
    };
    let r = if u.info.effect == MoveEffect::SuperFang {
        result(damage, eff, false)
    } else {
        let (damage, hung) = super::hold_on(b, env, u, damage, eff);
        result(damage, eff, hung)
    };
    Some(from_anim(b, env, u, r, None))
}

/// `BattleScript_EffectCounter` / `MirrorCoat`: twice the physical / special damage the foe
/// dealt this turn, else "But it failed!".
pub(super) fn counter(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    let rec = b.battler(u.attacker).fx.rec;
    let rec = if u.info.effect == MoveEffect::Counter {
        rec.physical
    } else {
        rec.special
    };
    let dealt = match rec {
        Some((d, src)) if src != u.attacker && b.battler(src).mon.hp > 0 && d > 0 => d,
        _ => {
            super::copy::no_pressure(b);
            b.attack_string(env, u);
            b.fail(env, u);
            return None;
        }
    };
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let eff = if b.immune(env, u) {
        b.battler_mut(u.attacker).v.unable = true;
        Effectiveness::NoEffect
    } else {
        Effectiveness::Normal
    };
    let (damage, hung) = super::hold_on(b, env, u, dealt as i32 * 2, eff);
    Some(from_anim(b, env, u, result(damage, eff, hung), None))
}

/// `BattleScript_EffectEndeavor`: the target's HP down to the attacker's.
pub(super) fn endeavor(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    let (hp, thp) = (b.battler(u.attacker).mon.hp, b.battler(u.target).mon.hp);
    if thp <= hp {
        b.fail(env, u);
        return None;
    }
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return None;
    }
    let damage = (thp - hp) as i32;
    if b.immune(env, u) {
        b.battler_mut(u.attacker).v.unable = true;
        let r = result(damage, Effectiveness::NoEffect, false);
        return Some(from_anim(b, env, u, r, None));
    }
    let (damage, hung) = super::hold_on(b, env, u, damage, Effectiveness::Normal);
    Some(from_anim(
        b,
        env,
        u,
        result(damage, Effectiveness::Normal, hung),
        None,
    ))
}

/// `BattleScript_EffectPresent` + `presentdamagecalculation`.
pub(super) fn present(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !b.accuracy(env, u) {
        b.missed(env, u);
        return None;
    }
    b.attack_string(env, u);
    let roll = env.rng.random() & 0xFF;
    let power = match roll {
        0..102 => 40,
        102..178 => 80,
        178..204 => 120,
        _ => 0,
    };
    if power > 0 {
        let u = MoveUse { power, ..*u };
        let r = b.damage(env, &u);
        return Some(from_anim(b, env, &u, r, None));
    }
    let t = b.battler(u.target);
    if t.mon.hp == t.mon.max_hp {
        say(env, u, Str::PkmnHpFull, u.target);
        return None;
    }
    let heal = (t.mon.max_hp / 4).max(1) as i32;
    anim(env, u);
    b.change_hp_passive(env, u.target, -heal);
    say(env, u, Str::PkmnRegainedHealth, u.target);
    None
}

/// `BattleScript_EffectRollout` + `rolloutdamagecalculation`: five turns locked, the power
/// doubling each hit (and with Defense Curl); a miss ends it.
pub(super) fn rollout(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    b.attack_string(env, u);
    let hit = b.accuracy(env, u);
    if !hit || b.immune(env, u) {
        cancel_all(b, u.attacker);
        b.miss_message(env, u);
        return None;
    }
    let a = u.attacker;
    if b.battler(a).fx.lock.is_none() {
        let fx = &mut b.battler_mut(a).fx;
        fx.rollout = 5;
        fx.lock = Some(u.mov);
        fx.slot = u.slot.unwrap_or(fx.slot);
    }
    let fx = &mut b.battler_mut(a).fx;
    fx.rollout = fx.rollout.saturating_sub(1);
    if fx.rollout == 0 {
        fx.lock = None;
    }
    let mut power = (u.info.power as u16) << (4 - fx.rollout.min(4));
    if fx.defense_curl {
        power *= 2;
    }
    let u = MoveUse {
        dynamic: power,
        ..*u
    };
    let r = b.damage(env, &u);
    Some(from_anim(b, env, &u, r, None))
}

/// `BattleScript_EffectSnore`: only while asleep ("fast asleep" first unless called by Sleep
/// Talk); flinch chance.
pub(super) fn snore(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    if !matches!(b.battler(u.attacker).mon.status, Status::Sleep(_)) {
        b.attack_string(env, u);
        b.fail(env, u);
        return None;
    }
    if !super::via_sleep_talk(b) {
        say(env, u, Str::PkmnFastAsleep, u.attacker);
    }
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return None;
    }
    b.hit(env, u, chance(flinch), true)
}

/// `BattleScript_EffectFocusPunch`: damaged this turn, it loses its focus (PP paid).
pub(super) fn focus_punch(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    let rec = b.battler(u.attacker).fx.rec;
    if rec.physical.is_none() && rec.special.is_none() {
        return b.hit(env, u, None, false);
    }
    pay_pp(b, u);
    b.no_effect = true;
    say(env, u, Str::PkmnLostFocus, u.attacker);
    None
}

fn charge_turn(b: &mut Battle, env: &mut Env, u: &MoveUse, id: Str) {
    pay_pp(b, u);
    anim(env, u);
    let fx = &mut b.battler_mut(u.attacker).fx;
    fx.lock = Some(u.mov);
    fx.slot = u.slot.unwrap_or(fx.slot);
    say(env, u, id, u.attacker);
}

/// Razor Wind, Sky Attack, Skull Bash, Solar Beam: `BattleScriptFirstChargingTurn` (PP, the
/// animation, the turn-1 message; Skull Bash's Defense +1), then
/// `BattleScript_TwoTurnMovesSecondTurn` (no PP; Sky Attack's flinch chance). Solar Beam
/// fires at once in the sun.
pub(super) fn two_turn(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    kind: TwoTurnKind,
) -> Option<HitResult> {
    let a = u.attacker;
    let sec = if kind == TwoTurnKind::SkyAttack {
        chance(flinch)
    } else {
        None
    };
    if b.battler(a).fx.lock.is_some() {
        b.battler_mut(a).fx.lock = None;
        return b.hit(env, u, sec, false);
    }
    if kind == TwoTurnKind::SolarBeam && b.weather().sun() {
        pay_pp(b, u);
        let u = MoveUse { slot: None, ..*u };
        return b.hit(env, &u, sec, false);
    }
    let id = match kind {
        TwoTurnKind::RazorWind => Str::PkmnWhippedWhirlwind,
        TwoTurnKind::SkyAttack => Str::PkmnIsGlowing,
        TwoTurnKind::SkullBash => Str::PkmnLoweredHead,
        TwoTurnKind::SolarBeam => Str::PkmnTookSunlight,
    };
    charge_turn(b, env, u, id);
    if kind == TwoTurnKind::SkullBash {
        stage(b, env, u, a, Stat::Def, 1, false);
    }
    None
}

/// Fly, Dig, Dive, Bounce: the first turn hides the user (`setsemiinvulnerablebit`); the second
/// clears it and strikes (Bounce's paralysis chance).
pub(super) fn semi_invulnerable(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    kind: SemiKind,
) -> Option<HitResult> {
    let a = u.attacker;
    if b.battler(a).fx.lock.is_some() {
        b.battler_mut(a).fx.lock = None;
        let sec = if kind == SemiKind::Bounce {
            chance(status_sec)
        } else {
            None
        };
        let hit = b.accuracy(env, u);
        b.battler_mut(a).fx.semi = None;
        if !hit {
            b.missed(env, u);
            return None;
        }
        return b.hit(env, u, sec, true);
    }
    let id = match kind {
        SemiKind::Fly => Str::PkmnFlewHigh,
        SemiKind::Bounce => Str::PkmnSprangUp,
        SemiKind::Dig => Str::PkmnDugHole,
        SemiKind::Dive => Str::PkmnHidUnderwater,
    };
    charge_turn(b, env, u, id);
    b.battler_mut(a).fx.semi = Some(kind);
    None
}

/// `BattleScript_EffectSpitUp` + `stockpiletobasedamage`: the stockpiled count times the base
/// damage (no crit, no random roll), then the type and `adjustsetdamage`; the count resets.
pub(super) fn spit_up(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
    let a = u.attacker;
    let count = b.battler(a).fx.stockpile;
    if protected(b, u) {
        b.attack_string(env, u);
        if count == 0 {
            b.no_effect = true;
            say(env, u, Str::FailedToSpitUp, a);
            return None;
        }
        b.battler_mut(a).fx.stockpile = 0;
        b.miss_message(env, u);
        return None;
    }
    b.attack_string(env, u);
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return None;
    }
    if count == 0 {
        b.no_effect = true;
        say(env, u, Str::FailedToSpitUp, a);
        return None;
    }
    let d = b.damage_calc(env, u, false) * count as i32;
    b.battler_mut(a).fx.stockpile = 0;
    let (d, eff) = b.type_calc(env, u, d);
    let (d, hung) = super::hold_on(b, env, u, d, eff);
    Some(from_anim(b, env, u, result(d, eff, hung), None))
}

/// `BattleScript_EffectFutureSight` + `trysetfutureattack`: the damage is computed now
/// (`CalculateBaseDamage`), it lands at the end of the third turn.
pub(super) fn future_sight(b: &mut Battle, env: &mut Env, u: &MoveUse, doom: bool) {
    b.attack_string(env, u);
    let t = u.target as usize;
    if b.fx.future[t].is_some() {
        b.fail(env, u);
        return;
    }
    let damage = b.damage_calc(env, u, false);
    b.fx.future[t] = Some(Future {
        turns: 3,
        damage,
        mov: u.mov,
        attacker: u.attacker,
    });
    anim(env, u);
    let id = if doom {
        Str::PkmnChoseXAsDestiny
    } else {
        Str::PkmnForesawAttack
    };
    say(env, u, id, u.attacker);
}
