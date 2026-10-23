//! Protect-likes, screens, Mist, Safeguard, Spikes, the weather moves, Ingrain, Wish, Roar,
//! Helping Hand / Follow Me, and `DoFieldEndTurnEffects`' side timers and Wish.

use super::cond::no_acc_fail;
use super::{Timer, anim, no_acc_check, say, tell};
use crate::battle::core::{Battle, Env, MoveUse, Weather};
use crate::battle::{BattleEvent, BattleOutcome, Buff, Side, Str};
use crate::mon::{MoveEffect, WeatherKind};

/// `ChangeStatBuffs`' Mist check for a drop on `side` (not `MOVE_EFFECT_CERTAIN`): blocked;
/// with `STAT_CHANGE_ALLOW_PTR` (`loud`) `BattleScript_MistProtected` once per move.
pub(in crate::battle::effects) fn mist_blocks(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    loud: bool,
) -> bool {
    if b.fx.mist[side as usize].is_none() {
        return false;
    }
    if loud {
        let first = match &mut b.fx.cur {
            Some(c) => !core::mem::replace(&mut c.lowered[side as usize], true),
            None => true,
        };
        if first {
            say(env, u, Str::PkmnProtectedByMist, side);
        }
    }
    true
}

/// The protect-likes, screens, Mist, Safeguard, Spikes, the weather moves, Ingrain, Wish,
/// Helping Hand and Follow Me.
pub(super) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    use MoveEffect as E;
    let a = u.attacker;
    let side = a as usize;
    match u.info.effect {
        E::Protect | E::Endure => protect(b, env, u),
        E::Spikes => {
            // `trysetspikes BattleScript_FailedFromAtkString`.
            let foe = a.other() as usize;
            if b.fx.spikes[foe] >= 3 {
                super::copy::no_pressure(b);
            }
            b.attack_string(env, u);
            if b.fx.spikes[foe] >= 3 {
                b.fail(env, u);
                return;
            }
            b.fx.spikes[foe] += 1;
            anim(env, u);
            say(env, u, Str::SpikesScattered, a);
        }
        E::Reflect | E::LightScreen => {
            b.attack_string(env, u);
            let reflect = u.info.effect == E::Reflect;
            let slot = if reflect {
                &mut b.fx.reflect[side]
            } else {
                &mut b.fx.light_screen[side]
            };
            if slot.is_some() {
                b.fail(env, u);
                return;
            }
            *slot = Some(Timer {
                turns: 5,
                mov: u.mov,
            });
            anim(env, u);
            let id = if reflect {
                Str::PkmnRaisedDef
            } else {
                Str::PkmnRaisedSpDef
            };
            say(env, u, id, a);
        }
        E::Mist => {
            b.attack_string(env, u);
            if b.fx.mist[side].is_some() {
                b.fail(env, u);
                return;
            }
            b.fx.mist[side] = Some(Timer {
                turns: 5,
                mov: u.mov,
            });
            anim(env, u);
            say(env, u, Str::PkmnShroudedInMist, a);
        }
        E::Safeguard => {
            b.attack_string(env, u);
            if b.fx.safeguard[side] > 0 {
                b.fail(env, u);
                return;
            }
            b.fx.safeguard[side] = 5;
            anim(env, u);
            say(env, u, Str::PkmnCoveredByVeil, a);
        }
        E::SetWeather(kind) => {
            b.attack_string(env, u);
            let w = b.weather();
            let (new, same, id) = match kind {
                WeatherKind::Rain => (Weather::Rain, w.rain(), Str::StartedToRain),
                WeatherKind::Sun => (Weather::Sun, w.sun(), Str::SunlightGotBright),
                WeatherKind::Sandstorm => (Weather::Sand, w == Weather::Sand, Str::SandstormBrewed),
                WeatherKind::Hail => (Weather::Hail, w == Weather::Hail, Str::StartedHail),
            };
            // Gen6's primal weathers cannot be replaced by a move.
            if same || w.primal() {
                b.fail(env, u);
                return;
            }
            b.x.weather = new;
            b.fx.weather_turns = 5;
            anim(env, u);
            say(env, u, id, a);
        }
        E::Ingrain => {
            b.attack_string(env, u);
            if b.battler(a).fx.rooted {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).fx.rooted = true;
            anim(env, u);
            say(env, u, Str::PkmnPlantedRoots, a);
        }
        E::Wish => {
            b.attack_string(env, u);
            if b.fx.wish[side] != 0 {
                b.fail(env, u);
                return;
            }
            b.fx.wish[side] = 2;
            b.fx.wish_party[side] = b.battler(a).mon.party;
            anim(env, u);
        }
        // `trysethelpinghand`: no partner in a single battle.
        E::HelpingHand => {
            b.attack_string(env, u);
            b.fail(env, u);
        }
        E::FollowMe => {
            b.attack_string(env, u);
            anim(env, u);
            say(env, u, Str::PkmnCenterAttention, a);
        }
        _ => {}
    }
}

/// `setprotectlike`: `sProtectSuccessRates[protectUses] >= Random()` (always drawn), and not
/// the turn's last action; the uses reset unless the last move was a protect-like.
fn protect(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let last_protect = b.battler(a).last_move.is_some_and(|m| {
        matches!(
            env.t.move_info(m).effect,
            MoveEffect::Protect | MoveEffect::Endure
        )
    });
    if !last_protect {
        b.battler_mut(a).fx.protect_uses = 0;
    }
    let last_turn = b.order[1] == a;
    let rate = protect_rate(env.t, b.battler(a).fx.protect_uses);
    if rate >= env.rng.random() && !last_turn {
        let fx = &mut b.battler_mut(a).fx;
        let id = if u.info.effect == MoveEffect::Protect {
            fx.protected = true;
            Str::PkmnProtectedItself2
        } else {
            fx.endured = true;
            Str::PkmnBracedItself
        };
        fx.protect_uses = fx.protect_uses.saturating_add(1);
        anim(env, u);
        say(env, u, id, a);
    } else {
        b.battler_mut(a).fx.protect_uses = 0;
        b.fail(env, u);
    }
}

/// `sProtectSuccessRates[protectUses]`. Past its 4 rates Gen3 reads on into the next table,
/// `sMovesForbiddenToCopy` (move ids, named here by effect: the n-th move with it in id
/// order); past that one (`protectUses` 24 and up, after 20 more successes) the ROM data is
/// not modelled and the rate is 0.
pub(in crate::battle::effects) fn protect_rate(t: &crate::mon::BattleTables, uses: u8) -> u16 {
    use MoveEffect as E;
    const RATES: [u16; 4] = [0xFFFF, 0x7FFF, 0x3FFF, 0x1FFF];
    if let Some(&r) = RATES.get(uses as usize) {
        return r;
    }
    let nth = |e: MoveEffect, n: usize| {
        t.moves
            .iter()
            .enumerate()
            .filter(|(_, m)| m.effect == e)
            .nth(n)
            .map_or(0, |(i, _)| i as u16)
    };
    match uses - 4 {
        0 => nth(E::Metronome, 0),
        1 => t.struggle.0,
        2 => nth(E::Sketch, 0),
        3 => nth(E::Mimic, 0),
        4 => 0xFFFE,
        5 => nth(E::Counter, 0),
        6 => nth(E::MirrorCoat, 0),
        7 => nth(E::Protect, 0),
        8 => nth(E::Protect, 1),
        9 => nth(E::Endure, 0),
        10 => nth(E::DestinyBond, 0),
        11 => nth(E::SleepTalk, 0),
        12 => nth(E::Thief, 0),
        13 => nth(E::FollowMe, 0),
        14 => nth(E::Snatch, 0),
        15 => nth(E::HelpingHand, 0),
        16 => nth(E::Thief, 1),
        17 => nth(E::Trick, 0),
        18 => nth(E::FocusPunch, 0),
        19 => 0xFFFF,
        _ => 0,
    }
}

/// `BattleScript_EffectRoar` + `forcerandomswitch`: Ingrain anchors the target; a trainer's
/// target is replaced by a random other mon ("was dragged out!"); a wild battle ends.
pub(super) fn roar(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let t = u.target;
    if super::super::abilities::suction_cups(b, t) {
        super::ability_blocks(
            env,
            u,
            Str::PkmnAnchorsItselfWith,
            crate::mon::Ability::SuctionCups,
        );
        return;
    }
    if b.battler(t).fx.rooted {
        say(env, u, Str::PkmnAnchoredItself, t);
        return;
    }
    if !no_acc_check(b, u) {
        no_acc_fail(b, env, u);
        return;
    }
    if !b.accuracy(env, u) {
        b.miss_message(env, u);
        return;
    }
    let party = &b.parties[t as usize];
    let valid = |i: usize| party.get(i).is_some_and(|p| p.hp > 0);
    if b.is_trainer() && (0..6).filter(|&i| valid(i)).count() <= 1 {
        b.fail(env, u);
        return;
    }
    // TryDoForceSwitchOut.
    let (la, lt) = (
        b.battler(u.attacker).mon.level as u32,
        b.battler(t).mon.level as u32,
    );
    if la < lt {
        let r = (env.rng.random() & 0xFF) as u32;
        // `(random * (atk + def) >> 8) + 1 <= def / 4`.
        if ((r * (la + lt)) >> 8) < lt / 4 {
            b.fail(env, u);
            return;
        }
    }
    let current = b.battler(t).mon.party as usize;
    let next = if b.is_trainer() {
        let party = &b.parties[t as usize];
        let valid = |i: usize| party.get(i).is_some_and(|p| p.hp > 0);
        Some(loop {
            let i = (env.rng.random() % 6) as usize;
            if i != current && valid(i) {
                break i as u8;
            }
        })
    } else {
        None
    };
    anim(env, u);
    b.switch_out(env, t);
    match next {
        Some(i) => {
            let m = crate::battle::core::msg(Str::PkmnWasDraggedOut, t.other(), t, t);
            b.switch_in_with(env, t, i, Some(m));
        }
        None => {
            let outcome = match u.attacker {
                Side::Player => BattleOutcome::PlayerTeleported,
                Side::Opponent => BattleOutcome::MonTeleported,
            };
            b.finish(env, outcome);
        }
    }
}

/// `DoFieldEndTurnEffects` up to the weather: Reflect, Light Screen, Mist, Safeguard (each
/// for both sides in turn), then Wish in turn order.
pub(super) fn end_turn(b: &mut Battle, env: &mut Env) {
    for which in 0..3 {
        for side in [Side::Player, Side::Opponent] {
            let s = side as usize;
            let slot = match which {
                0 => &mut b.fx.reflect[s],
                1 => &mut b.fx.light_screen[s],
                _ => &mut b.fx.mist[s],
            };
            let Some(t) = slot else {
                continue;
            };
            t.turns = t.turns.saturating_sub(1);
            if t.turns == 0 {
                let mov = t.mov;
                *slot = None;
                tell(env, Str::PkmnSXWoreOff, side, super::move_buff(mov));
            }
        }
    }
    for side in [Side::Player, Side::Opponent] {
        let s = side as usize;
        if b.fx.safeguard[s] > 0 {
            b.fx.safeguard[s] -= 1;
            if b.fx.safeguard[s] == 0 {
                tell(env, Str::PkmnSafeguardExpired, side, [Buff::None; 3]);
            }
        }
    }
    for side in b.order {
        let s = side as usize;
        if b.fx.wish[s] == 0 {
            continue;
        }
        b.fx.wish[s] -= 1;
        let m = *b.battler(side);
        if b.fx.wish[s] != 0 || m.mon.hp == 0 {
            continue;
        }
        let buffs = [
            Buff::Mon {
                side,
                party: b.fx.wish_party[s],
            },
            Buff::None,
            Buff::None,
        ];
        let about = |id| {
            BattleEvent::Msg(crate::battle::Msg {
                buffs,
                wait: crate::battle::core::LONG,
                ..crate::battle::core::msg(id, side.other(), side, side)
            })
        };
        if m.mon.hp == m.mon.max_hp {
            env.out.push(about(Str::PkmnWishCameTrue));
            env.out.push(about(Str::PkmnHpFull));
            continue;
        }
        env.out.push(about(Str::PkmnWishCameTrue));
        let heal = (m.mon.max_hp / 2).max(1) as i32;
        b.change_hp_passive(env, side, -heal);
        env.out.push(about(Str::PkmnRegainedHealth));
    }
}
