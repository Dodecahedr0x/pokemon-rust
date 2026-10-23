//! Role Play, Skill Swap, Grudge, Imprison, Magic Coat and Trick, with their hooks (the
//! Grudge on a faint, Imprison's selection and canceler limit, Magic Coat's bounce in
//! `Cmd_attackcanceler`).

use super::{anim, no_acc_check, say, say_with};
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, Buff, Msg, Side, Str};
use crate::data::MoveId;
use crate::mon::{Ability, HoldEffect, MoveEffect, MoveFlags};

/// The scripts (`BattleScript_EffectRolePlay` …).
pub(super) fn apply(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let (a, t) = (u.attacker, u.target);
    match u.info.effect {
        // `accuracycheck BattleScript_ButItFailed, NO_ACC_CALC_CHECK_LOCK_ON` +
        // `trycopyability`: not no ability nor Wonder Guard.
        MoveEffect::RolePlay => {
            b.attack_string(env, u);
            let ability = b.battler(t).ability;
            if !no_acc_check(b, u) || matches!(ability, Ability::None | Ability::WonderGuard) {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).ability = ability;
            anim(env, u);
            say_with(
                env,
                u,
                Str::PkmnCopiedFoe,
                a,
                [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(ability)),
                ],
            );
        }
        // `tryswapabilities`: not both none, nor Wonder Guard on either side.
        MoveEffect::SkillSwap => {
            b.attack_string(env, u);
            let (x, y) = (b.battler(a).ability, b.battler(t).ability);
            if !no_acc_check(b, u)
                || (x == Ability::None && y == Ability::None)
                || x == Ability::WonderGuard
                || y == Ability::WonderGuard
            {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).ability = y;
            b.battler_mut(t).ability = x;
            anim(env, u);
            say(env, u, Str::PkmnSwappedAbilities, a);
        }
        // `trysetgrudge`.
        MoveEffect::Grudge => {
            b.attack_string(env, u);
            if b.battler(a).fx.grudge {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).fx.grudge = true;
            anim(env, u);
            say(env, u, Str::PkmnWantsGrudge, a);
        }
        // `tryimprison`: Gen3 fails unless the foe knows one of the user's moves.
        MoveEffect::Imprison => {
            b.attack_string(env, u);
            let mine = b.battler(a).mon.moves;
            let theirs = b.battler(a.other()).mon.moves;
            let shared = mine.iter().flatten().any(|m| theirs.contains(&Some(*m)));
            if b.battler(a).fx.imprison || !shared {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).fx.imprison = true;
            anim(env, u);
            say(env, u, Str::PkmnSealedOpponentMove, a);
        }
        // `trysetmagiccoat BattleScript_FailedFromAtkString`: not as the turn's last action.
        MoveEffect::MagicCoat => {
            super::copy::no_pressure(b);
            b.attack_string(env, u);
            if b.order[1] == a {
                b.fail(env, u);
                return;
            }
            b.battler_mut(a).fx.bounce = true;
            anim(env, u);
            say(env, u, Str::PkmnShroudedItself, a);
        }
        MoveEffect::Trick => trick(b, env, u),
        _ => {}
    }
}

/// `BattleScript_EffectTrick` + `tryswapitems`: not behind a substitute; never an opponent's
/// (a regular battle), not after Knock Off, not when neither holds an item, not with Mail or
/// the Enigma Berry on either side (`HoldEffect::NoSwap`); Sticky Hold.
fn trick(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let (a, t) = (u.attacker, u.target);
    b.attack_string(env, u);
    if super::sub::subbed(b, u) || !b.accuracy(env, u) {
        b.fail(env, u);
        return;
    }
    let (am, tm) = (*b.battler(a), *b.battler(t));
    let knocked = |side: Side, party: u8| b.fx.knocked_off[side as usize] & (1 << party) != 0;
    let no_swap = |h: Option<HoldEffect>| h == Some(HoldEffect::NoSwap);
    if a == Side::Opponent
        || knocked(a, am.mon.party)
        || knocked(t, tm.mon.party)
        || (am.x.item.is_none() && tm.x.item.is_none())
        || no_swap(am.x.hold)
        || no_swap(tm.x.hold)
    {
        b.fail(env, u);
        return;
    }
    if super::super::abilities::sticky_hold(b, t) {
        super::ability_blocks(env, u, Str::PkmnSXMadeYIneffective, Ability::StickyHold);
        return;
    }
    for (side, party, item, hold) in [
        (a, am.mon.party, tm.x.item, tm.x.hold),
        (t, tm.mon.party, am.x.item, am.x.hold),
    ] {
        let m = b.battler_mut(side);
        m.x.item = item;
        m.x.hold = hold;
        b.x.holds[side as usize][party as usize] = hold;
        if let Some(p) = b.parties[side as usize].get_mut(party as usize) {
            p.held = item;
        }
    }
    anim(env, u);
    say(env, u, Str::PkmnSwitchedItems, a);
    // gItemSwapStringIds.
    let item = |i: Option<crate::data::ItemId>| i.map_or(Buff::None, Buff::Item);
    let id = match (am.x.item, tm.x.item) {
        (Some(_), Some(_)) => Str::PkmnObtainedXYObtainedZ,
        (None, Some(_)) => Str::PkmnObtainedX,
        _ => Str::PkmnObtainedX2,
    };
    say_with(
        env,
        u,
        id,
        a,
        [item(tm.x.item), item(am.x.item), Buff::None],
    );
}

/// `IsImprisoned`: the foe of `side` imprisoned and knows `mov`.
pub(in crate::battle::effects) fn imprisoned(b: &Battle, side: Side, mov: MoveId) -> bool {
    let foe = b.battler(side.other());
    foe.fx.imprison && foe.mon.hp > 0 && foe.mon.moves.contains(&Some(mov))
}

/// `Cmd_tryfaintmon`'s Grudge: the battler fainting to a foe's move (not Struggle) takes the
/// PP of the move that knocked it out (`chosenMovePositions`).
pub(in crate::battle::effects) fn grudge(b: &mut Battle, env: &mut Env, side: Side) {
    let Some(c) = b.fx.cur else {
        return;
    };
    let a = side.other();
    if !b.battler(side).fx.grudge || c.target != side || c.attacker != a {
        return;
    }
    let m = b.battler(a);
    let slot = c.slot.unwrap_or(m.fx.slot) as usize;
    let Some(mov) = m.mon.moves.get(slot).copied().flatten() else {
        return;
    };
    if m.mon.hp == 0 || c.mov == env.t.struggle {
        return;
    }
    b.battler_mut(a).mon.pp[slot] = 0;
    b.write_back(a);
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [Buff::Move(mov), Buff::None, Buff::None],
        wait: LONG,
        ..msg(Str::PkmnLostPPGrudge, a, side, a)
    }));
}

/// `Cmd_attackcanceler`'s Magic Coat (after the canceler and `ABILITYEFFECT_MOVES_BLOCK`):
/// the target's coat sends a `FLAG_MAGIC_COAT_AFFECTED` move back
/// (`BattleScript_MagicCoatBounce`: the attack string and PP, "was bounced back", then the
/// move by the target, without PP). The bounced move's use, when it was.
pub(in crate::battle::effects) fn bounce(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
) -> Option<(MoveUse, Option<HitResult>)> {
    let t = u.target;
    if t == u.attacker
        || !b.battler(t).fx.bounce
        || !u.info.flags.has(MoveFlags::MAGIC_COAT_AFFECTED)
    {
        return None;
    }
    // PressurePPLose(attacker, target, MOVE_MAGIC_COAT).
    super::copy::pressure_pp_lose(b, env, u.attacker, t, MoveEffect::MagicCoat);
    b.battler_mut(t).fx.bounce = false;
    b.attack_string(env, u);
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::PkmnMoveBounced, u.attacker),
    ));
    let back = MoveUse {
        attacker: t,
        target: u.attacker,
        slot: None,
        printed: true,
        ..*u
    };
    super::start_move(b, &back);
    let hit = b.execute(env, &back);
    Some((back, hit))
}
