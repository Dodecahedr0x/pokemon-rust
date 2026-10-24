//! Held items in battle (Silice; Gen3 `ItemBattleEffects`, `battle_util.c`, with the
//! `HOLD_EFFECT_*` of `include/constants/hold_effects.h`).
//!
//! - [`end_turn`]: `ITEMEFFECT_NORMAL` at `ENDTURN_ITEMS1` (`moveTurn` FALSE: the HP, PP,
//!   pinch stat and flavour berries, Leftovers and the cures) and `ENDTURN_ITEMS2` (`moveTurn`
//!   TRUE: the cures, White Herb and Mental Herb only).
//! - [`move_end`]: `MOVEEND_ITEM_EFFECTS_ALL` (`ITEMEFFECT_MOVE_END`: the cure berries and
//!   herbs of every battler, in index order, until none acts) then
//!   `MOVEEND_KINGSROCK_SHELLBELL` (King's Rock, Shell Bell).
//! - [`on_switch_in`]: `ITEMEFFECT_ON_SWITCH_IN` (Amulet Coin, White Herb).
//! - Elsewhere: Quick Claw and Macho Brace in `Battle::strike_order`, BrightPowder in
//!   `calc::accuracy_hits_x`, the type items, Choice Band and the species items (Soul Dew,
//!   Deep Sea Tooth / Scale, Light Ball, Metal Powder, Thick Club) in `calc::base_damage_x`,
//!   Scope Lens, Lucky Punch and Stick in `calc::crit_item_stage`, Focus Band in
//!   `Battle::damage` (`Cmd_adjustnormaldamage`), Smoke Ball in `effects::run_block`, Amulet
//!   Coin in `Battle::prize`, Exp. Share, Lucky Egg and Macho Brace in `Cmd_getexp`
//!   (`Battle::gain_exp`), Everstone in `world::evolve`. Soothe Bell goes to
//!   `world::friendship`; Dragon Scale and Up-Grade only matter to trade evolutions.
//!
//! A berry is consumed (`removeitem`): the battler and its party mon lose the item. The
//! `B_ANIM_HELD_ITEM_EFFECT` animation has no event. Lum Berry's "cured its {B_BUFF1}
//! problem" leaves the status name empty (`gStatusConditionString_*Jpn`, as Shed Skin).

use super::status;
use crate::battle::calc;
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, BattleReport, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, HoldEffect, MoveFlags, Stat, Status};

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
    // `usedHeldItems` (Recycle).
    super::mech::record_used_item(b, side);
    let party = b.battler(side).mon.party as usize;
    let m = b.battler_mut(side);
    m.x.item = None;
    m.x.hold = None;
    b.x.holds[side.s()][party] = None;
    if let Some(p) = b.parties[side.s()].get_mut(party) {
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

/// The herbs (any `moveTurn`, and at `ITEMEFFECT_MOVE_END`): White Herb when a stat stage is
/// negative, Mental Herb when infatuated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Herb {
    White,
    Mental,
}

fn herb(b: &Battle, side: Side) -> Option<Herb> {
    let m = b.battler(side);
    match m.x.hold? {
        HoldEffect::RestoreStats if m.stages.iter().any(|&s| s < 0) => Some(Herb::White),
        HoldEffect::CureAttract if m.v.infatuated => Some(Herb::Mental),
        _ => None,
    }
}

/// `BattleScript_WhiteHerbRet` (the negative stages back to 0, "X's ITEM restored its
/// status!") / `BattleScript_BerryCureChosenStatus*` for Mental Herb ("cured its {B_BUFF1}
/// problem", the status name empty). The herb is used up.
fn apply_herb(b: &mut Battle, env: &mut Env, side: Side, h: Herb) {
    let m = b.battler_mut(side);
    let id = match h {
        Herb::White => {
            for s in &mut m.stages {
                *s = (*s).max(0);
            }
            Str::ItemRestoredStatus
        }
        Herb::Mental => {
            m.v.infatuated = false;
            env.out.push(BattleEvent::Report(BattleReport::Degraded(
                "mental herb status name",
            )));
            Str::ItemCuredProblem
        }
    };
    env.out.push(BattleEvent::Msg(item_msg(b, id, side)));
    if h == Herb::Mental {
        env.out.push(BattleEvent::StatusIcon {
            side,
            status: b.battler(side).mon.status,
        });
    }
    consume(b, side);
}

/// `ITEMEFFECT_ON_SWITCH_IN` for `side`: Amulet Coin on the player's side doubles the prize
/// (`moneyMultiplier`); White Herb (`BattleScript_WhiteHerbEnd2`).
pub(super) fn on_switch_in(b: &mut Battle, env: &mut Env, side: Side) {
    match b.battler(side).x.hold {
        Some(HoldEffect::DoublePrize) if side == Side::Player => b.x.double_prize = true,
        Some(HoldEffect::RestoreStats) if herb(b, side).is_some() => {
            apply_herb(b, env, side, Herb::White)
        }
        _ => {}
    }
}

/// `max_hp / param` (a 0 param is treated as 1).
fn pinch(max: u16, param: u8) -> u16 {
    max / (param.max(1) as u16)
}

/// `BattleScript_BerryStatRaiseEnd2`: `stat` up by `stages` ("Using ITEM, the STAT of X
/// rose!"), the berry used up.
fn stat_berry(b: &mut Battle, env: &mut Env, side: Side, stat: Stat, stages: u8) {
    b.change_stage(side, stat, stages as i8);
    env.out.push(BattleEvent::StatAnim { side, up: true });
    let text = if stages >= 2 {
        Str::StatSharplyRose
    } else {
        Str::StatRose
    };
    let mut m = item_msg(b, Str::UsingItemStatRose, side);
    m.buffs[0] = Buff::Stat(stat);
    m.buffs[1] = Buff::Text(text);
    env.out.push(BattleEvent::Msg(m));
    consume(b, side);
}

/// `TRY_EAT_CONFUSE_BERRY`: heals max HP / `param` (at least 1); when the holder's nature
/// dislikes the flavour, `BattleScript_BerryConfuseHealEnd2` also says so and confuses it
/// (`MOVE_EFFECT_CONFUSION | MOVE_EFFECT_AFFECTS_USER`, primary: Own Tempo and a current
/// confusion block it silently).
fn confuse_berry(b: &mut Battle, env: &mut Env, side: Side, flavor: u8, param: u8) {
    let m = b.battler(side);
    let (hp, max) = (m.mon.hp, m.mon.max_hp);
    let heal = pinch(max, param).max(1).min(max - hp);
    // gPokeblockFlavorCompatibilityTable: spicy, dry, sweet, bitter, sour = Atk, SpAtk,
    // Speed, SpDef, Def; a nature lowering that stat dislikes it.
    let stat = [Stat::Atk, Stat::SpAtk, Stat::Speed, Stat::SpDef, Stat::Def]
        .get(flavor as usize)
        .copied();
    let party = m.mon.party as usize;
    let dislikes = match (stat, b.parties[side.s()].get(party)) {
        (Some(s), Some(p)) => p.nature().effect(s) < 0,
        _ => false,
    };
    env.out
        .push(BattleEvent::Msg(item_msg(b, Str::ItemRestoredHealth, side)));
    b.change_hp_passive(env, side, -(heal as i32));
    if dislikes {
        // "For X, the ITEM was {B_BUFF1}": the flavour text (gPokeblockWasTooXStringTable)
        // is not extracted.
        env.out.push(BattleEvent::Report(BattleReport::Degraded(
            "berry flavor text",
        )));
        env.out
            .push(BattleEvent::Msg(item_msg(b, Str::ForXCommaYZ, side)));
        let m = b.battler(side);
        if m.ability != Ability::OwnTempo && m.confusion == 0 && m.mon.hp > 0 {
            b.battler_mut(side).confusion = (env.rng.random() % 4) as u8 + 2;
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..msg(Str::PkmnBecameConfused, side, side, side)
            }));
        }
    }
    consume(b, side);
}

/// `HOLD_EFFECT_RESTORE_PP`: the first move at 0 PP gets `param` back (up to its max PP,
/// `BattleScript_BerryPPHealEnd2`). False when no move is at 0.
fn leppa(b: &mut Battle, env: &mut Env, side: Side, param: u8) -> bool {
    let m = b.battler(side);
    let Some(i) = (0..4).find(|&i| m.mon.moves[i].is_some() && m.mon.pp[i] == 0) else {
        return false;
    };
    let mov = m.mon.moves[i].expect("move");
    let pp = param.min(m.mon.max_pp[i]);
    b.battler_mut(side).mon.pp[i] = pp;
    b.write_back(side);
    let mut msg = item_msg(b, Str::ItemRestoredPp, side);
    msg.buffs[0] = Buff::Move(mov);
    env.out.push(BattleEvent::Msg(msg));
    consume(b, side);
    true
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
        HoldEffect::RestorePp(p) if items1 => {
            leppa(b, env, side, p);
        }
        HoldEffect::ConfuseBerry { flavor, param } if items1 && hp <= max / 2 => {
            confuse_berry(b, env, side, flavor, param)
        }
        HoldEffect::StatUp(stat, p)
            if items1
                && hp <= pinch(max, p)
                && b.battler(side).stages[stat as usize] < calc::MAX_STAGE =>
        {
            stat_berry(b, env, side, stat, 1)
        }
        // BattleScript_BerryFocusEnergyEnd2.
        HoldEffect::CriticalUp(p)
            if items1 && hp <= pinch(max, p) && !b.battler(side).focus_energy =>
        {
            b.battler_mut(side).focus_energy = true;
            env.out
                .push(BattleEvent::Msg(item_msg(b, Str::UsedXToGetPumped, side)));
            consume(b, side);
        }
        // Starf Berry: `Random() % 5` until a stat (Attack … Sp. Defense) below +6, +2.
        HoldEffect::RandomStatUp(p) if items1 && hp <= pinch(max, p) => {
            let stages = b.battler(side).stages;
            if (1..6).any(|i| stages[i] < calc::MAX_STAGE) {
                let stat = loop {
                    let i = (env.rng.random() % 5) as usize + 1;
                    if stages[i] != calc::MAX_STAGE {
                        break Stat::ALL[i];
                    }
                };
                stat_berry(b, env, side, stat, 2);
            }
        }
        _ if herb(b, side).is_some() => {
            let h = herb(b, side).expect("herb");
            apply_herb(b, env, side, h);
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
            if b.battler(s).mon.hp == 0 {
                return None;
            }
            match (cure(b, s, true), herb(b, s)) {
                (Some(id), _) => Some((s, Ok(id))),
                (None, Some(h)) => Some((s, Err(h))),
                _ => None,
            }
        });
        let Some((side, found)) = found else { break };
        let id = match found {
            Ok(id) => id,
            // BattleScript_WhiteHerbRet returns at once (`return effect`).
            Err(h) => {
                apply_herb(b, env, side, h);
                if h == Herb::White {
                    break;
                }
                continue;
            }
        };
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
    if let Some(HoldEffect::ShellBell(p)) = b.battler(u.attacker).x.hold {
        shell_bell(b, env, u, r, p);
        return;
    }
    let Some(HoldEffect::KingsRock(p)) = b.battler(u.attacker).x.hold else {
        return;
    };
    // TARGET_TURN_DAMAGED: not a hit on a substitute.
    if r.damage == 0
        || r.eff == Effectiveness::NoEffect
        || r.hp_dealt == 0
        || super::mech::sub_absorbed(b, u.target)
    {
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

/// `HOLD_EFFECT_SHELL_BELL`: the attacker (not at full HP, with HP) gets the HP it dealt /
/// `param` back (at least 1), with `BattleScript_ItemHealHP_Ret`. Kept.
fn shell_bell(b: &mut Battle, env: &mut Env, u: &MoveUse, r: HitResult, p: u8) {
    let a = b.battler(u.attacker);
    let (hp, max) = (a.mon.hp, a.mon.max_hp);
    if r.damage == 0
        || r.eff == Effectiveness::NoEffect
        || r.hp_dealt == 0
        || u.attacker == u.target
        || hp == max
        || hp == 0
    {
        return;
    }
    let heal = (r.hp_dealt / p.max(1) as u16).max(1).min(max - hp);
    env.out.push(BattleEvent::Msg(item_msg(
        b,
        Str::ItemRestoredHpALittle,
        u.attacker,
    )));
    b.change_hp_passive(env, u.attacker, -(heal as i32));
}
