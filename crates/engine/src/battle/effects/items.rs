//! Held items in battle (Silice; Gen3 `ItemBattleEffects`, `battle_util.c`, with the
//! `HOLD_EFFECT_*` of `include/constants/hold_effects.h`).
//!
//! - [`end_turn`]: `ITEMEFFECT_NORMAL` at `ENDTURN_ITEMS1` (`moveTurn` FALSE: the HP berries,
//!   Leftovers and the cures) and `ENDTURN_ITEMS2` (`moveTurn` TRUE: the cures only).
//! - [`move_end`]: `MOVEEND_ITEM_EFFECTS_ALL` (`ITEMEFFECT_MOVE_END`: the cure berries of every
//!   battler, in index order, until none acts) then `MOVEEND_KINGSROCK_SHELLBELL`.
//! - Elsewhere: Quick Claw in `Battle::strike_order`, BrightPowder in
//!   `calc::accuracy_hits_x`, the type items and Choice Band in `calc::base_damage_x`, Focus
//!   Band in `Battle::damage` (`Cmd_adjustnormaldamage`).
//!
//! A berry is consumed (`removeitem`): the battler and its party mon lose the item. The
//! `B_ANIM_HELD_ITEM_EFFECT` animation has no event. Lum Berry's "cured its {B_BUFF1}
//! problem" leaves the status name empty (`gStatusConditionString_*Jpn`, as Shed Skin).

use super::status;
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{HoldEffect, MoveFlags, Status};

/// A message about `side` naming its item (`B_LAST_ITEM`, in `buffs[2]`).
fn item_msg(b: &Battle, id: Str, side: Side) -> Msg {
    Msg {
        buffs: [
            Buff::None,
            Buff::None,
            b.battler(side).x.item.map_or(Buff::None, Buff::Item),
        ],
        wait: LONG,
        ..msg(id, side, side, side)
    }
}

/// `removeitem`: the battler and its party mon lose their item.
fn consume(b: &mut Battle, side: Side) {
    let party = b.battler(side).mon.party as usize;
    let m = b.battler_mut(side);
    m.x.item = None;
    m.x.hold = None;
    b.x.holds[side as usize][party] = None;
    if let Some(p) = b.parties[side as usize].get_mut(party) {
        p.held = None;
    }
}

/// The cure a berry gives `side` now, with its message (`None`: nothing to cure). `move_end`
/// is `ITEMEFFECT_MOVE_END`, where Lum Berry always says "cured its problem".
fn cure(b: &Battle, side: Side, move_end: bool) -> Option<Str> {
    let m = b.battler(side);
    let st = m.mon.status;
    match m.x.hold? {
        HoldEffect::CurePar if st == Status::Paralysis => Some(Str::ItemCuredParalysis),
        HoldEffect::CurePsn if matches!(st, Status::Poison | Status::Toxic(_)) => {
            Some(Str::ItemCuredPoison)
        }
        HoldEffect::CureBrn if st == Status::Burn => Some(Str::ItemHealedBurn),
        HoldEffect::CureFrz if st == Status::Freeze => Some(Str::ItemDefrostedIt),
        HoldEffect::CureSlp if matches!(st, Status::Sleep(_)) => Some(Str::ItemWokeIt),
        HoldEffect::CureConfusion if m.confusion > 0 => Some(Str::ItemSnappedOut),
        HoldEffect::CureStatus if st != Status::None || m.confusion > 0 => {
            let several = st != Status::None && m.confusion > 0;
            Some(if several && !move_end {
                Str::ItemNormalizedStatus
            } else {
                Str::ItemCuredProblem
            })
        }
        _ => None,
    }
}

/// Applies the cure [`cure`] found: the status (and confusion) goes, the message, the status
/// icon (`updatestatusicon`, not for Persim), the berry is eaten.
fn apply_cure(b: &mut Battle, env: &mut Env, side: Side, id: Str) {
    let confusion_only = id == Str::ItemSnappedOut;
    let m = b.battler_mut(side);
    if id == Str::ItemSnappedOut || id == Str::ItemCuredProblem || id == Str::ItemNormalizedStatus {
        m.confusion = 0;
    }
    if !confusion_only {
        m.mon.status = Status::None;
    }
    b.write_back(side);
    env.out.push(BattleEvent::Msg(item_msg(b, id, side)));
    if !confusion_only {
        env.out.push(BattleEvent::StatusIcon {
            side,
            status: Status::None,
        });
    }
    consume(b, side);
}

/// `ITEMEFFECT_NORMAL` for `side` (with HP). `items1` is `ENDTURN_ITEMS1` (`!moveTurn`):
/// only then do the HP berries (at half HP or less) and Leftovers act.
pub(super) fn end_turn(b: &mut Battle, env: &mut Env, side: Side, items1: bool) {
    let m = b.battler(side);
    if m.mon.hp == 0 {
        return;
    }
    let Some(hold) = m.x.hold else { return };
    let (hp, max) = (m.mon.hp, m.mon.max_hp);
    match hold {
        // BattleScript_ItemHealHP_RemoveItem: the message, the HP, `removeitem`.
        HoldEffect::RestoreHp(p) if items1 && hp <= max / 2 => {
            let heal = (p as u16).min(max - hp);
            env.out
                .push(BattleEvent::Msg(item_msg(b, Str::ItemRestoredHealth, side)));
            b.change_hp_passive(env, side, -(heal as i32));
            consume(b, side);
        }
        // BattleScript_ItemHealHP_End2: max HP / 16 (at least 1), kept.
        HoldEffect::Leftovers if items1 && hp < max => {
            let heal = (max / 16).max(1).min(max - hp);
            env.out.push(BattleEvent::Msg(item_msg(
                b,
                Str::ItemRestoredHpALittle,
                side,
            )));
            b.change_hp_passive(env, side, -(heal as i32));
        }
        _ => {
            if let Some(id) = cure(b, side, false) {
                if id == Str::ItemCuredProblem || id == Str::ItemNormalizedStatus {
                    env.out
                        .push(BattleEvent::Report(crate::battle::BattleReport::Degraded(
                            "lum berry status name",
                        )));
                }
                apply_cure(b, env, side, id);
            }
        }
    }
}

/// `MOVEEND_ITEM_EFFECTS_ALL` then `MOVEEND_KINGSROCK_SHELLBELL`.
pub(super) fn move_end(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    // ITEMEFFECT_MOVE_END: the first battler (index order) with a cure, again until none.
    // Gen3 does not check HP here; a fainted battler's status is irrelevant, so the engine
    // skips it.
    loop {
        let found = [Side::Player, Side::Opponent].into_iter().find_map(|s| {
            (b.battler(s).mon.hp > 0)
                .then(|| cure(b, s, true))
                .flatten()
                .map(|id| (s, id))
        });
        let Some((side, id)) = found else { break };
        if id == Str::ItemCuredProblem {
            env.out
                .push(BattleEvent::Report(crate::battle::BattleReport::Degraded(
                    "lum berry status name",
                )));
        }
        apply_cure(b, env, side, id);
    }
    kings_rock(b, env, u, hit);
}

/// `ITEMEFFECT_KINGSROCK_SHELLBELL`, `HOLD_EFFECT_FLINCH`: when the move did damage (and had
/// an effect), `Random() % 100 < param` is drawn; then a `KINGS_ROCK_AFFECTED` move flinches
/// a target with HP (`SetMoveEffect(FALSE, 0)`: Shield Dust blocks it, Inner Focus silently).
fn kings_rock(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    let Some(r) = hit else { return };
    let Some(HoldEffect::KingsRock(p)) = b.battler(u.attacker).x.hold else {
        return;
    };
    if r.damage == 0 || r.eff == Effectiveness::NoEffect || r.hp_dealt == 0 {
        return;
    }
    if env.rng.random() % 100 >= p as u16
        || !u.info.flags.has(MoveFlags::KINGS_ROCK_AFFECTED)
        || b.battler(u.target).mon.hp == 0
    {
        return;
    }
    status::flinch(b, env, u, u.target, status::Cause::SECONDARY, false);
}
