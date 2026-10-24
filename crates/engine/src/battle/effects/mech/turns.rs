//! Wave C's hooks into the shared battle flow: the action selection (`gLockedMoves`, Encore,
//! `CheckMoveLimitations`), `attackcanceler`, `moveend`, the switch-in (Spikes), the faint
//! (Destiny Bond), `datahpupdate`'s damage records and the end of the turn
//! (`DoBattlerEndTurnEffects`, `HandleWishPerishSongOnTurnEnd`, `TurnValuesCleanUp`).

use super::{Current, cancel_all, tell};
use crate::battle::calc;
use crate::battle::core::{Action, Battle, Env, LONG, MoveUse, Step, msg};
use crate::battle::{BattleEvent, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, BattleTables, MoveEffect, MoveFlags, Status, Type};

// ---- action selection ----

/// `gLockedMoves` / `STATUS2_RECHARGE`: the action of a locked or recharging battler (not
/// asked, `HandleTurnActionSelectionState`).
pub(crate) fn locked_action(b: &Battle, side: Side) -> Option<Action> {
    let m = b.battler(side);
    if m.mon.hp == 0 || (m.fx.lock.is_none() && m.fx.recharge == 0) {
        return None;
    }
    Some(Action::Move(m.fx.slot))
}

/// `CheckMoveLimitations` without the PP check for slot `i` of `side`: no move, a disabled
/// move, the tormented repeat, a taunted status move.
pub(in crate::battle::effects) fn limited_except_pp(
    b: &Battle,
    t: &BattleTables,
    side: Side,
    i: usize,
) -> bool {
    let m = b.battler(side);
    let Some(mv) = m.mon.moves[i] else {
        return true;
    };
    m.fx.disabled == Some(mv)
        || (m.fx.tormented && m.last_move == Some(mv))
        || (m.fx.taunt > 0 && t.move_info(mv).power == 0)
        || super::imprisoned(b, side, mv)
}

/// `CheckMoveLimitations(MOVE_LIMITATIONS_ALL)` for slot `i` of `side` (the AI's score 0).
pub(crate) fn slot_limited(b: &Battle, t: &BattleTables, side: Side, i: usize) -> bool {
    b.battler(side).mon.pp[i] == 0 || limited_except_pp(b, t, side, i)
}

/// Gen3 `AreAllMovesUnusable`.
pub(crate) fn all_moves_unusable(b: &Battle, t: &BattleTables, side: Side) -> bool {
    (0..4).all(|i| slot_limited(b, t, side, i))
}

/// An encored battler's move slot (its move is not asked).
pub(crate) fn encored_slot(b: &Battle, side: Side) -> Option<u8> {
    b.battler(side).fx.encore.map(|e| e.1)
}

/// `TrySetCantSelectMoveBattleScript` for slot `slot`: Disable, Torment, Taunt.
pub(crate) fn selection_block(b: &Battle, t: &BattleTables, side: Side, slot: u8) -> Option<Msg> {
    let m = b.battler(side);
    let mv = m.mon.moves.get(slot as usize).copied().flatten()?;
    let id = if m.fx.disabled == Some(mv) {
        Str::PkmnMoveIsDisabled
    } else if m.fx.tormented && m.last_move == Some(mv) && mv != t.struggle {
        Str::PkmnCantUseMoveTorment
    } else if m.fx.taunt > 0 && t.move_info(mv).power == 0 {
        Str::PkmnCantUseMoveTaunt
    } else if super::imprisoned(b, side, mv) {
        Str::PkmnCantUseMoveSealed
    } else {
        return None;
    };
    Some(Msg {
        mov: Some(mv),
        ..msg(id, side, side.other(), side)
    })
}

/// Whether a battler knows a move that reads `isFirstTurn` (Fake Out), so its battler keeps
/// the counter.
pub(crate) fn reads_first_turn(t: &BattleTables, b: &crate::battle::core::Battler) -> bool {
    b.mon
        .moves
        .iter()
        .flatten()
        .any(|&m| t.move_info(m).effect == MoveEffect::FakeOut)
}

/// `CheckFocusPunch_ClearVarsBeforeTurnStarts`: "tightening its focus" for each battler about
/// to use Focus Punch (not asleep, not loafing), then `TryClearRageStatuses`.
pub(crate) fn before_turn(b: &mut Battle, env: &mut Env) {
    for side in b.sides() {
        if b.is_absent(side) {
            continue;
        }
        let m = b.battler(side);
        let chosen = match b.actions[side.i()] {
            Some(Action::Move(s)) => m.mon.moves[s as usize],
            _ => None,
        };
        let effect = chosen.map(|mv| env.t.move_info(mv).effect);
        if effect == Some(MoveEffect::FocusPunch)
            && !matches!(m.mon.status, Status::Sleep(_))
            && !m.v.truant
            && m.fx.lock.is_none()
        {
            tell(env, Str::PkmnTighteningFocus, side, [Buff::None; 3]);
        }
        if effect != Some(MoveEffect::Rage) {
            b.battler_mut(side).fx.rage = false;
        }
    }
}

/// `HandleAction_UseMove`: a locked battler uses `gLockedMoves` (no PP); an encored one its
/// encored move (the encore ends when that slot changed).
pub(in crate::battle::effects) fn reroute(
    b: &mut Battle,
    env: &Env,
    u: &MoveUse,
) -> Option<MoveUse> {
    let a = u.attacker;
    let m = *b.battler(a);
    if let Some(lock) = m.fx.lock {
        return Some(b.move_use(env, a, lock, None));
    }
    let (mov, pos) = m.fx.encore?;
    if m.mon.moves[pos as usize] != Some(mov) {
        let fx = &mut b.battler_mut(a).fx;
        fx.encore = None;
        fx.encore_timer = 0;
        return None;
    }
    if u.slot.is_none() || u.slot == Some(pos) {
        return None;
    }
    Some(b.move_use(env, a, mov, Some(pos)))
}

// ---- the move ----

/// `IS_TYPE_PHYSICAL` as `datahpupdate` reads it: Hidden Power's base type
/// (`F_DYNAMIC_TYPE_IGNORE_PHYSICALITY`), else the move's type.
pub(in crate::battle::effects) fn physical_of(u: &MoveUse) -> bool {
    if u.info.effect == MoveEffect::HiddenPower {
        u.info.ty.is_physical()
    } else {
        u.ty.is_physical()
    }
}

/// The move starts (`gCurrentMove`, `gSpecialStatuses` cleared).
pub(in crate::battle::effects) fn start_move(b: &mut Battle, u: &MoveUse) {
    b.fx.cur = Some(Current {
        attacker: u.attacker,
        target: u.target,
        physical: physical_of(u),
        hit: [false; 4],
        sleep_talk: false,
        lowered: [false; 4],
        called: None,
        mov: u.mov,
        slot: u.slot,
        printed: false,
        unavailable: false,
        no_pressure: false,
    });
}

/// The move was used through Sleep Talk (`gChosenMove == MOVE_SLEEP_TALK`).
pub(in crate::battle::effects) fn via_sleep_talk(b: &Battle) -> bool {
    b.fx.cur.is_some_and(|c| c.sleep_talk)
}

/// Snore and Sleep Talk go on while asleep (`CANCELLER_ASLEEP`).
pub(in crate::battle::effects) fn sleep_usable(u: &MoveUse) -> bool {
    matches!(u.info.effect, MoveEffect::Snore | MoveEffect::SleepTalk)
}

/// `CANCELLER_FLAGS`: Destiny Bond ends when its user moves again.
pub(in crate::battle::effects) fn canceler_flags(b: &mut Battle, u: &MoveUse) {
    let fx = &mut b.battler_mut(u.attacker).fx;
    fx.destiny_bond = false;
    fx.grudge = false;
}

/// `CANCELLER_RECHARGE`: true when the battler must recharge (the move is skipped).
pub(in crate::battle::effects) fn canceler_recharge(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
) -> bool {
    let a = u.attacker;
    if b.battler(a).fx.recharge == 0 {
        return false;
    }
    b.battler_mut(a).fx.recharge = 0;
    cancel_all(b, a);
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::PkmnMustRecharge, a)));
    true
}

/// `CANCELLER_DISABLED` / `CANCELLER_TAUNTED`: true when the move is refused.
pub(in crate::battle::effects) fn canceler_limits(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
) -> bool {
    let a = u.attacker;
    let m = b.battler(a);
    let id = if m.fx.disabled == Some(u.mov) {
        Str::PkmnMoveIsDisabled
    } else if m.fx.taunt > 0 && u.info.power == 0 {
        Str::PkmnCantUseMoveTaunt
    } else if super::imprisoned(b, a, u.mov) {
        // CANCELLER_IMPRISONED.
        Str::PkmnCantUseMoveSealed
    } else {
        return false;
    };
    cancel_all(b, a);
    b.battler_mut(a).v.unable = true;
    env.out.push(BattleEvent::Msg(u.msg_long(id, a)));
    true
}

/// `CANCELLER_THAW`: a frozen user of a thawing move (Flame Wheel …) thaws.
pub(in crate::battle::effects) fn canceler_thaw(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let a = u.attacker;
    if b.battler(a).mon.status != Status::Freeze || u.info.effect != MoveEffect::ThawHit {
        return;
    }
    b.battler_mut(a).mon.status = Status::None;
    b.write_back(a);
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::PkmnWasDefrostedBy, a)));
    env.out.push(BattleEvent::StatusIcon {
        side: a,
        status: Status::None,
    });
}

/// The canceler skipped the move.
pub(in crate::battle::effects) fn skipped(b: &mut Battle) {
    // MOVEEND_UPDATE_LAST_MOVES without HITMARKER_OBEYS: MOVE_UNAVAILABLE lands.
    if let Some(c) = b.fx.cur {
        b.battler_mut(c.target).fx.rec.landed = None;
    }
    b.fx.cur = None;
}

/// The move a calling move (Metronome …) used, for `moveend`'s abilities and items.
pub(in crate::battle::effects) fn called_use(
    b: &Battle,
    env: &Env,
    u: &MoveUse,
) -> Option<MoveUse> {
    let c = b.fx.cur?;
    let mov = c.called?;
    Some(MoveUse {
        target: c.target,
        ..b.move_use(env, u.attacker, mov, None)
    })
}

/// `datahpupdate`'s records of move damage (`gProtectStructs.physicalDmg` / `specialDmg` and
/// the battler that dealt it: the attacker when its target takes it, else the target).
pub(crate) fn record_damage(b: &mut Battle, side: Side, dealt: u16) {
    let Some(c) = b.fx.cur else {
        return;
    };
    if dealt == 0 {
        return;
    }
    let source = if side == c.target {
        c.attacker
    } else {
        c.target
    };
    let rec = &mut b.battler_mut(side).fx.rec;
    if c.physical {
        rec.physical = Some((dealt, source));
    } else {
        rec.special = Some((dealt, source));
    }
    if let Some(c) = &mut b.fx.cur {
        c.hit[side.i()] = true;
    }
}

/// `tryfaintmon` of a battler with Destiny Bond: the move's attacker goes down with it
/// (`BattleScript_DestinyBondTakesLife`, before the target's faint).
pub(crate) fn destiny_bond(b: &mut Battle, env: &mut Env, side: Side) {
    super::grudge(b, env, side);
    let Some(c) = b.fx.cur else {
        return;
    };
    let a = side.other();
    if !b.battler(side).fx.destiny_bond
        || c.target != side
        || c.attacker != a
        || b.battler(a).mon.hp == 0
    {
        return;
    }
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(Str::PkmnTookFoe, a, side, side)
    }));
    let hp = b.battler(a).mon.hp as i32;
    b.change_hp_passive(env, a, hp);
    b.try_faint(env, a);
}

/// `MOVEEND_RAGE`: a raging target hit by a damaging move gains Attack.
pub(in crate::battle::effects) fn move_end_first(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let t = u.target;
    let m = b.battler(t);
    let hit = b.fx.cur.is_some_and(|c| c.hit[t.i()]);
    if !m.fx.rage
        || m.mon.hp == 0
        || u.attacker == t
        || b.no_effect
        || u.power == 0
        || !hit
        || m.stages[crate::mon::Stat::Atk as usize] >= calc::MAX_STAGE
    {
        return;
    }
    b.change_stage(t, crate::mon::Stat::Atk, 1);
    env.out.push(BattleEvent::StatAnim { side: t, up: true });
    env.out
        .push(BattleEvent::Msg(u.msg_long(Str::PkmnRageBuilding, t)));
}

/// `MOVEEND_MIRROR_MOVE` (`lastTakenMove`), then the move's bookkeeping ends.
pub(in crate::battle::effects) fn move_end_last(b: &mut Battle, u: &MoveUse) {
    let t = u.target;
    super::sub::sub_end(b);
    // MOVEEND_UPDATE_LAST_MOVES: `gLastPrintedMoves` (the chosen move), and, but for Baton
    // Pass, `gLastLandedMoves` / `gLastHitByType` of the target (Wave D).
    if let Some(c) = b.fx.cur {
        let moved = if c.unavailable { None } else { Some(c.mov) };
        if c.printed {
            b.battler_mut(u.attacker).fx.rec.printed = moved;
        }
        if u.info.effect != MoveEffect::BatonPass {
            let landed = moved.is_some() && !b.no_effect;
            let rec = &mut b.battler_mut(t).fx.rec;
            rec.landed = landed.then_some(u.mov);
            if landed {
                rec.hit_type = Some(u.ty);
            }
        }
    }
    if u.info.flags.has(MoveFlags::MIRROR_MOVE_AFFECTED)
        && u.attacker != t
        && b.battler(t).mon.hp > 0
        && !b.no_effect
    {
        b.battler_mut(t).fx.rec.last_taken = Some(u.mov);
    }
    b.fx.cur = None;
}

// ---- switch-in, faint ----

/// `SwitchInClearSetData` / `FaintClearSetData` for the battler leaving `side`: what it held
/// on the other battler (its wrap, its Lock-On) ends.
pub(crate) fn clear_bonds(b: &mut Battle, side: Side) {
    let fx = &mut b.battler_mut(side.other()).fx;
    fx.wrapped = 0;
    fx.wrap_move = None;
    fx.lock_on = 0;
}

/// `switchineffects`' Spikes: max HP / ((5 - layers) * 2), not on a Flying or Levitate
/// battler (`BattleScript_SpikesOn*`: the HP, then the message).
pub(crate) fn spikes(b: &mut Battle, env: &mut Env, side: Side) {
    let n = b.fx.spikes[side.s()];
    let m = b.battler(side);
    if n == 0 || m.mon.hp == 0 || m.types.contains(&Type::Flying) || m.ability == Ability::Levitate
    {
        return;
    }
    let damage = (m.mon.max_hp / ((5 - n as u16) * 2)).max(1) as i32;
    b.change_hp_passive(env, side, damage);
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(Str::PkmnHurtBySpikes, side, side, side)
    }));
    if b.try_faint(env, side) {
        b.steps.push_front(Step::Fainted);
    }
}

/// `IsRunningFromBattleImpossible` / the party menu: a wrapped or rooted battler cannot leave.
pub(in crate::battle::effects) fn trapped(b: &Battle, side: Side) -> bool {
    let fx = &b.battler(side).fx;
    fx.wrapped > 0 || fx.rooted
}

/// The side's Safeguard is up.
pub(in crate::battle::effects) fn safeguarded(b: &Battle, side: Side) -> bool {
    b.fx.safeguard[side.s()] > 0
}

// ---- end of turn ----

/// `TurnValuesCleanUp(TRUE)` and `HandleEndTurn_ContinueBattle`: the protections end; a
/// sleeping battler's lock ends.
pub(in crate::battle::effects) fn turn_passed(b: &mut Battle) {
    for side in [Side::Player, Side::Opponent] {
        let m = b.battler_mut(side);
        m.fx.protected = false;
        m.fx.endured = false;
        if matches!(m.mon.status, Status::Sleep(_)) && (m.fx.lock.is_some() || m.v.uproar > 0) {
            cancel_all(b, side);
        }
    }
}

/// `TurnValuesCleanUp(FALSE)`: the damage records clear, the recharge counts down.
pub(in crate::battle::effects) fn turn_clean_up(b: &mut Battle) {
    for side in [Side::Player, Side::Opponent] {
        let fx = &mut b.battler_mut(side).fx;
        fx.rec.physical = None;
        fx.rec.special = None;
        fx.recharge = fx.recharge.saturating_sub(1);
        fx.bounce = false;
        fx.snatch = false;
    }
}

fn hurt(b: &mut Battle, env: &mut Env, side: Side, div: u16, id: Str, buffs: [Buff; 3]) {
    let damage = (b.battler(side).mon.max_hp / div).max(1) as i32;
    tell(env, id, side, buffs);
    b.change_hp_passive(env, side, damage);
    b.try_faint(env, side);
}

/// `ENDTURN_INGRAIN` (before the abilities).
pub(in crate::battle::effects) fn end_turn_ingrain(b: &mut Battle, env: &mut Env, side: Side) {
    let m = b.battler(side);
    if !m.fx.rooted || m.mon.hp == 0 || m.mon.hp == m.mon.max_hp {
        return;
    }
    let heal = (m.mon.max_hp / 16).max(1) as i32;
    tell(env, Str::PkmnAbsorbedNutrients, side, [Buff::None; 3]);
    b.change_hp_passive(env, side, -heal);
}

/// `ENDTURN_NIGHTMARES`, `ENDTURN_CURSE`, `ENDTURN_WRAP` (after the burn).
pub(in crate::battle::effects) fn end_turn_after_status(b: &mut Battle, env: &mut Env, side: Side) {
    let m = b.battler(side);
    if m.fx.nightmare && m.mon.hp > 0 {
        if matches!(m.mon.status, Status::Sleep(_)) {
            hurt(b, env, side, 4, Str::PkmnLockedInNightmare, [Buff::None; 3]);
        } else {
            b.battler_mut(side).fx.nightmare = false;
        }
    }
    let m = b.battler(side);
    if m.fx.cursed && m.mon.hp > 0 {
        hurt(b, env, side, 4, Str::PkmnAfflictedByCurse, [Buff::None; 3]);
    }
    let m = b.battler(side);
    if m.fx.wrapped > 0 && m.mon.hp > 0 {
        let buffs = m.fx.wrap_move.map_or([Buff::None; 3], super::move_buff);
        let fx = &mut b.battler_mut(side).fx;
        fx.wrapped -= 1;
        if fx.wrapped > 0 {
            hurt(b, env, side, 16, Str::PkmnHurtBy, buffs);
        } else {
            fx.wrap_move = None;
            tell(env, Str::PkmnFreedFrom, side, buffs);
        }
    }
}

/// `ENDTURN_THRASH`, `ENDTURN_DISABLE`, `ENDTURN_ENCORE`, `ENDTURN_LOCK_ON` (after the
/// uproar).
pub(in crate::battle::effects) fn end_turn_timers(b: &mut Battle, env: &mut Env, side: Side) {
    let m = *b.battler(side);
    if m.fx.rampage > 0 {
        b.battler_mut(side).fx.rampage -= 1;
        if m.v.unable {
            cancel_all(b, side);
        } else if m.fx.rampage == 1 && m.fx.lock.is_some() {
            b.battler_mut(side).fx.lock = None;
            if b.battler(side).confusion == 0 {
                b.battler_mut(side).confusion = (env.rng.random() % 4) as u8 + 2;
                tell(env, Str::PkmnFatigueConfusion, side, [Buff::None; 3]);
            }
        }
    }
    if m.fx.disable_timer > 0 {
        let has =
            m.fx.disabled
                .is_some_and(|d| m.mon.moves.contains(&Some(d)));
        let fx = &mut b.battler_mut(side).fx;
        if !has {
            fx.disabled = None;
            fx.disable_timer = 0;
        } else {
            fx.disable_timer -= 1;
            if fx.disable_timer == 0 {
                fx.disabled = None;
                tell(env, Str::PkmnMoveDisabledNoMore, side, [Buff::None; 3]);
            }
        }
    }
    if m.fx.encore_timer > 0 {
        let fx = &mut b.battler_mut(side).fx;
        match m.fx.encore {
            Some((mv, pos)) if m.mon.moves[pos as usize] == Some(mv) => {
                fx.encore_timer -= 1;
                if fx.encore_timer == 0 || m.mon.pp[pos as usize] == 0 {
                    fx.encore = None;
                    fx.encore_timer = 0;
                    tell(env, Str::PkmnEncoreEnded, side, [Buff::None; 3]);
                }
            }
            _ => {
                fx.encore = None;
                fx.encore_timer = 0;
            }
        }
    }
    let fx = &mut b.battler_mut(side).fx;
    fx.lock_on = fx.lock_on.saturating_sub(1);
}

/// `ENDTURN_TAUNT` (after the charge).
pub(in crate::battle::effects) fn end_turn_taunt(b: &mut Battle, side: Side) {
    let fx = &mut b.battler_mut(side).fx;
    fx.taunt = fx.taunt.saturating_sub(1);
}

/// Whether `HandleWishPerishSongOnTurnEnd` has anything to do (a future attack or a perish
/// count pending).
pub(crate) fn wish_perish_pending(b: &Battle) -> bool {
    b.fx.future.iter().any(Option::is_some) || b.battlers.iter().any(|m| m.fx.perish.is_some())
}

/// One event of `HandleWishPerishSongOnTurnEnd` from `state` (0..n: the future attacks by
/// battler index; n..2n: the perish counts in turn order; n = `gBattlersCount`). The state to go on from after an
/// event, whose faints are handled first (`BattleTurnPassed` runs `HandleFaintedMonActions`
/// again before it); `None` when done.
pub(crate) fn wish_perish(b: &mut Battle, env: &mut Env, state: u8) -> Option<u8> {
    let mut i = state;
    let n = b.count() as u8;
    while i < n {
        let side = Side::at(i as usize);
        i += 1;
        let s = side.i();
        let Some(mut f) = b.fx.future[s] else {
            continue;
        };
        f.turns = f.turns.saturating_sub(1);
        if f.turns > 0 {
            b.fx.future[s] = Some(f);
            continue;
        }
        b.fx.future[s] = None;
        if b.battler(side).mon.hp == 0 {
            continue;
        }
        future_attack(b, env, side, f);
        return Some(i);
    }
    while i < 2 * n {
        let side = b.order[(i - n) as usize];
        i += 1;
        let m = b.battler(side);
        let Some(n) = m.fx.perish else {
            continue;
        };
        if m.mon.hp == 0 {
            continue;
        }
        let buffs = [Buff::Number(n as u32), Buff::None, Buff::None];
        if n == 0 {
            b.battler_mut(side).fx.perish = None;
            tell(env, Str::PkmnPerishCountFell, side, buffs);
            let hp = b.battler(side).mon.hp as i32;
            b.change_hp_passive(env, side, hp);
            b.try_faint(env, side);
        } else {
            tell(env, Str::PkmnPerishCountFell, side, buffs);
            b.battler_mut(side).fx.perish = Some(n - 1);
        }
        return Some(i);
    }
    None
}

/// `BattleScript_MonTookFutureAttack`: the move's accuracy, `adjustnormaldamage2` (the random
/// roll, the Focus Band), the hit.
fn future_attack(b: &mut Battle, env: &mut Env, side: Side, f: super::Future) {
    let u = MoveUse {
        target: side,
        ..b.move_use(env, f.attacker, f.mov, None)
    };
    start_move(b, &u);
    env.out.push(BattleEvent::Msg(Msg {
        buffs: super::move_buff(f.mov),
        wait: LONG,
        ..msg(Str::PkmnTookAttack, f.attacker, side, side)
    }));
    if !b.accuracy(env, &u) {
        b.fail(env, &u);
        b.fx.cur = None;
        return;
    }
    let d = calc::random_damage(f.damage, env.rng.random());
    let (d, hung) = super::hold_on(b, env, &u, d, Effectiveness::Normal);
    env.out.push(BattleEvent::MoveAnim {
        attacker: f.attacker,
        mov: f.mov,
    });
    let r = crate::battle::core::HitResult {
        damage: d,
        hp_dealt: 0,
        crit: false,
        eff: Effectiveness::Normal,
        hung,
    };
    b.deal_with(env, &u, r, false);
    b.try_faint(env, side);
    super::sub::sub_end(b);
    b.fx.cur = None;
}
