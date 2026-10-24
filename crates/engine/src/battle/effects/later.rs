//! The weather and the abilities Silice adds beyond Gen3's (`mon::Ability` after
//! `NaturalCure`). Gen3 order and RNG where Gen3 has the ability (Drizzle, Drought, Sand
//! Stream, Speed Boost, Levitate, Shadow Tag, Shell / Battle Armor); later abilities follow
//! their Gen7 behaviour fitted into Gen3's script order, without new RNG draws.
//!
//! - Weather: `ABILITYEFFECT_ON_SWITCH_IN` sets it ([`on_switch_in`]); `DoFieldEndTurnEffects`
//!   (`ENDTURN_RAIN` / `_SUN` / `_SANDSTORM`) prints it each turn and the sandstorm hurts
//!   ([`field_end_turn`]). Every weather ability gives a permanent weather (Gen3). Primordial
//!   Sea and Desolate Land (Gen6) give heavy rain / harsh sun, which the other weather
//!   abilities cannot replace, which make Fire / Water moves fail, and which end when no
//!   battler with the ability is left ([`left_field`]).
//! - Speed Boost and Solar Power at `ENDTURN_ABILITIES` ([`end_turn`]).
//! - [`before_move`] (after `attackcanceler`'s status checks): the primal weathers' failed
//!   moves, Prankster against a Dark target, Magic Bounce, Lightning Rod.
//! - Mold Breaker ([`break_mold`]): the target's breakable ability is off during the move.
//! - Steadfast in `status::canceler`, Shadow Tag in `effects::run_block` / `switch_block` and
//!   the AI's switch; Prankster in `Battle::strike_order`; the damage abilities in
//!   `Battle::damage` / `calc`; Skill Link in `moves::multi_hit`; Parental Bond in
//!   `Battle::hit`.

use super::Canceler;
use crate::battle::core::{Battle, Env, LONG, MoveUse, Weather, msg};
use crate::battle::{BattleEvent, Buff, Msg, Side, Str};
use crate::mon::{Ability, MoveFlags, MoveInfo, Stat, Type};

/// A message about `side` naming `ability` (`B_SCR_ACTIVE_ABILITY`, in `buffs[2]`).
fn ability_about(id: Str, side: Side, ability: Ability) -> BattleEvent {
    BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(ability)),
        ],
        wait: LONG,
        ..msg(id, side, side, side)
    })
}

/// `ABILITYEFFECT_ON_SWITCH_IN` for `side`: the weather abilities (Gen3: a permanent weather
/// is not set again; Gen6: nothing replaces a primal weather but the other primal one) and
/// Mold Breaker's announcement.
pub(super) fn on_switch_in(b: &mut Battle, env: &mut Env, side: Side) {
    let m = b.battler(side);
    if m.mon.hp == 0 {
        return;
    }
    let ability = m.ability;
    let current = b.x.weather;
    let (weather, id) = match ability {
        Ability::Drizzle => (Weather::Rain, Str::MadeItRain),
        Ability::Drought => (Weather::Sun, Str::IntensifiedSun),
        Ability::SandStream => (Weather::Sand, Str::WhippedUpSandstorm),
        Ability::PrimordialSea => (Weather::HeavyRain, Str::HeavyRainBegan),
        Ability::DesolateLand => (Weather::HarshSun, Str::HarshSunBegan),
        Ability::MoldBreaker => {
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(Str::BreaksTheMold, side, side, side)
            }));
            return;
        }
        _ => return,
    };
    // A move's timed weather is replaced (Gen3: only the same permanent weather is skipped).
    if (current == weather && b.fx.weather_turns == 0) || (current.primal() && !weather.primal()) {
        return;
    }
    b.x.weather = weather;
    b.fx.weather_turns = 0;
    env.out.push(ability_about(id, side, ability));
}

/// A battler with `ability` left the field (switched out or fainted): a primal weather ends
/// when no battler with HP has its ability.
pub(super) fn left_field(b: &mut Battle, env: &mut Env, ability: Ability) {
    let (source, id) = match (b.x.weather, ability) {
        (Weather::HeavyRain, Ability::PrimordialSea) => {
            (Ability::PrimordialSea, Str::HeavyRainLifted)
        }
        (Weather::HarshSun, Ability::DesolateLand) => (Ability::DesolateLand, Str::HarshSunFaded),
        _ => return,
    };
    let kept = b
        .sides()
        .any(|s| b.battler(s).ability == source && b.battler(s).mon.hp > 0);
    if kept {
        return;
    }
    b.x.weather = Weather::None;
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(id, Side::Player, Side::Player, Side::Player)
    }));
}

/// `DoFieldEndTurnEffects`' weather: "Rain continues to fall." / "The sunlight is strong." /
/// "The sandstorm rages." (permanent weathers never end), then the sandstorm's damage to each
/// battler in turn order (`BattleScript_DamagingWeatherContinues`: max HP / 16, at least 1;
/// not Rock / Ground / Steel, nor Sand Force, Gen5, nor Sand Veil, Gen4; none under Cloud Nine
/// / Air Lock).
/// Wave C: a move's weather lasts 5 turns (`B_WEATHER_*_TEMPORARY`, "The rain stopped." …);
/// Hail pelts all but Ice battlers; Dig and Dive dodge both.
pub(super) fn field_end_turn(b: &mut Battle, env: &mut Env) {
    let ended = b.fx.weather_turns > 0 && {
        b.fx.weather_turns -= 1;
        b.fx.weather_turns == 0
    };
    let id = match (b.x.weather, ended) {
        (Weather::None, _) => return,
        (Weather::Rain, true) => Str::RainStopped,
        (Weather::Sun, true) => Str::SunlightFaded,
        (Weather::Sand, true) => Str::SandstormSubsided,
        (Weather::Hail, true) => Str::HailStopped,
        (Weather::Rain | Weather::HeavyRain, _) => Str::RainContinues,
        (Weather::Sun | Weather::HarshSun, _) => Str::SunlightStrong,
        (Weather::Sand, _) => Str::SandstormRages,
        (Weather::Hail, _) => Str::HailContinues,
    };
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(id, Side::Player, Side::Player, Side::Player)
    }));
    if ended {
        b.x.weather = Weather::None;
        return;
    }
    let hail = b.x.weather == Weather::Hail;
    // Cmd_weatherdamage: no damage while Cloud Nine / Air Lock is out (the message stays).
    if (b.x.weather != Weather::Sand && !hail) || b.weather() != b.x.weather {
        return;
    }
    for side in b.turn_sides() {
        let m = b.battler(side);
        let immune = if hail {
            m.types.contains(&Type::Ice)
        } else {
            m.types
                .iter()
                .any(|t| matches!(t, Type::Rock | Type::Ground | Type::Steel))
                || matches!(m.ability, Ability::SandForce | Ability::SandVeil)
        };
        let hidden = matches!(
            m.fx.semi,
            Some(crate::mon::SemiKind::Dig | crate::mon::SemiKind::Dive)
        );
        if m.mon.hp == 0 || immune || hidden {
            continue;
        }
        let damage = (m.mon.max_hp / 16).max(1) as i32;
        let id = if hail {
            Str::PkmnPeltedByHail
        } else {
            Str::BuffetedBySandstorm
        };
        env.out.push(BattleEvent::Msg(Msg {
            wait: LONG,
            ..msg(id, side, side, side)
        }));
        b.change_hp_passive(env, side, damage);
        b.try_faint(env, side);
    }
}

/// `ENDTURN_ABILITIES` for `side` (with HP): Speed Boost (+1 Speed below +6, not on the turn
/// it came in: `isFirstTurn != 2`; `BattleScript_SpeedBoostActivates`), Rain Dish (max HP / 16
/// in rain below max HP, `BattleScript_RainDishActivates`) and Solar Power (Gen4: max HP / 8
/// in the sun, with [`Str::SolarPowerHurt`]).
pub(super) fn end_turn(b: &mut Battle, env: &mut Env, side: Side) {
    let m = *b.battler(side);
    if m.mon.hp == 0 {
        return;
    }
    match m.ability {
        Ability::SpeedBoost if m.x.first_turn != 2 && b.change_stage(side, Stat::Speed, 1) => {
            env.out.push(BattleEvent::StatAnim { side, up: true });
            env.out
                .push(ability_about(Str::RaisedSpeed, side, Ability::SpeedBoost));
        }
        Ability::RainDish if b.weather().rain() && m.mon.hp < m.mon.max_hp => {
            let heal = (m.mon.max_hp / 16).max(1).min(m.mon.max_hp - m.mon.hp);
            env.out.push(ability_about(
                Str::AbilityRestoredHp,
                side,
                Ability::RainDish,
            ));
            b.change_hp_passive(env, side, -(heal as i32));
        }
        Ability::SolarPower if b.weather().sun() => {
            let damage = (m.mon.max_hp / 8).max(1) as i32;
            env.out.push(ability_about(
                Str::SolarPowerHurt,
                side,
                Ability::SolarPower,
            ));
            b.change_hp_passive(env, side, damage);
            b.try_faint(env, side);
        }
        _ => {}
    }
}

/// Steadfast (Gen4): a flinch raises the battler's Speed.
pub(super) fn steadfast(b: &mut Battle, env: &mut Env, side: Side) {
    if b.battler(side).ability == Ability::Steadfast && b.change_stage(side, Stat::Speed, 1) {
        env.out.push(BattleEvent::StatAnim { side, up: true });
        env.out
            .push(ability_about(Str::RaisedSpeed, side, Ability::Steadfast));
    }
}

/// Prankster (Gen5): a status move (no power) gets +1 priority.
pub(crate) fn prankster_boost(ability: Ability, info: &MoveInfo) -> bool {
    ability == Ability::Prankster && info.power == 0
}

/// The abilities Mold Breaker ignores (the engine's subset of Gen7's breakable abilities).
fn breakable(a: Ability) -> bool {
    matches!(
        a,
        Ability::Levitate
            | Ability::Filter
            | Ability::ThickFat
            | Ability::MarvelScale
            | Ability::Soundproof
            | Ability::Sturdy
            | Ability::ClearBody
            | Ability::KeenEye
            | Ability::ShieldDust
            | Ability::InnerFocus
            | Ability::MagicBounce
            | Ability::LightningRod
            | Ability::ShellArmor
            | Ability::BattleArmor
            | Ability::Oblivious
            | Ability::Insomnia
            | Ability::VitalSpirit
    )
}

/// Mold Breaker: the target's breakable ability is set to none for the move; returns what to
/// restore (side, party slot, ability).
pub(crate) fn break_mold(b: &mut Battle, u: &MoveUse) -> Option<(Side, u8, Ability)> {
    if u.attacker == u.target || b.battler(u.attacker).ability != Ability::MoldBreaker {
        return None;
    }
    let t = b.battler_mut(u.target);
    if !breakable(t.ability) {
        return None;
    }
    let ability = core::mem::replace(&mut t.ability, Ability::None);
    Some((u.target, t.mon.party, ability))
}

/// The checks after the status cancelers: the primal weathers make damaging Fire / Water moves
/// fail (after the attack string); Prankster's status moves do not affect a Dark target
/// (Gen7); Magic Bounce sends a `MAGIC_COAT_AFFECTED` move back (the attacker's PP is spent,
/// the bounced move is used by the target without PP); Lightning Rod absorbs Electric moves
/// for Sp. Atk +1 ("made it ineffective" at +6). `None`: the move goes on.
pub(super) fn before_move(b: &mut Battle, env: &mut Env, u: &MoveUse) -> Option<Canceler> {
    let weather = b.x.weather;
    let fizzle = match (weather, u.ty) {
        (Weather::HeavyRain, Type::Fire) if u.power > 0 => Some(Str::FizzledInRain),
        (Weather::HarshSun, Type::Water) if u.power > 0 => Some(Str::EvaporatedInSun),
        _ => None,
    };
    if let Some(id) = fizzle {
        b.attack_string(env, u);
        b.no_effect = true;
        env.out.push(BattleEvent::Msg(u.msg_long(id, u.attacker)));
        return Some(Canceler::Done(*u, None));
    }
    if u.target == u.attacker {
        return None;
    }
    let attacker = b.battler(u.attacker);
    let target = b.battler(u.target);
    if prankster_boost(attacker.ability, u.info) && target.types.contains(&Type::Dark) {
        b.attack_string(env, u);
        b.no_effect = true;
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::ItDoesntAffect, u.target)));
        return Some(Canceler::Done(*u, None));
    }
    if target.ability == Ability::MagicBounce
        && !u.printed
        && u.info.flags.has(MoveFlags::MAGIC_COAT_AFFECTED)
    {
        b.attack_string(env, u);
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(Ability::MagicBounce)),
            ],
            ..u.msg_long(Str::BouncedBack, u.target)
        }));
        let back = MoveUse {
            attacker: u.target,
            target: u.attacker,
            slot: None,
            printed: true,
            ..*u
        };
        b.execute(env, &back);
        return Some(Canceler::Done(*u, None));
    }
    if target.ability == Ability::LightningRod && u.ty == Type::Electric {
        b.attack_string(env, u);
        b.no_effect = true;
        if b.change_stage(u.target, Stat::SpAtk, 1) {
            env.out.push(BattleEvent::StatAnim {
                side: u.target,
                up: true,
            });
            env.out.push(BattleEvent::Msg(b.stat_msg(
                u,
                u.target,
                Stat::SpAtk,
                1,
                true,
            )));
        } else {
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(Ability::LightningRod)),
                ],
                ..u.msg_long(Str::PkmnSXMadeItIneffective, u.target)
            }));
        }
        return Some(Canceler::Done(*u, None));
    }
    None
}
