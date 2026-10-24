//! Wave D: Substitute and Baton Pass, with their hooks into the shared flow
//! (`Cmd_healthbarupdate` / `Cmd_datahpupdate` on a substitute, `SetMoveEffect`'s refusal,
//! `MOVEEND_SUBSTITUTE`, `SwitchInClearSetData` after Baton Pass).

use super::{anim, say};
use crate::battle::core::{Battle, Battler, Env, MoveUse, msg};
use crate::battle::{BattleEvent, Msg, Side, Str};
use crate::mon::MoveEffect;

/// `BattleScript_EffectSubstitute` + `Cmd_setsubstitute`: max HP / 4 (at least 1) becomes the
/// substitute's HP; "too weak" at that HP or less. Clears `STATUS2_WRAPPED`. The user's HP
/// drops through `datahpupdate BS_ATTACKER` (Bide and the damage records see it, as Gen3).
pub(super) fn substitute(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    if b.battler(a).fx.substitute {
        // BattleScript_AlreadyHasSubstitute.
        say(env, u, Str::PkmnHasSubstitute, a);
        return;
    }
    let m = b.battler(a);
    let cost = (m.mon.max_hp / 4).max(1);
    if m.mon.hp <= cost {
        // B_MSG_SUBSTITUTE_FAILED (no MOVE_RESULT_FAILED).
        say(env, u, Str::TooWeakForSubstitute, a);
        return;
    }
    let fx = &mut b.battler_mut(a).fx;
    fx.substitute = true;
    // `gDisableStructs.substituteHP` is a u8 (max HP / 4 of a Gen3 mon fits).
    fx.sub_hp = cost.min(u8::MAX as u16) as u8;
    fx.wrapped = 0;
    fx.wrap_move = None;
    anim(env, u);
    b.change_hp(env, a, cost as i32);
    say(env, u, Str::PkmnMadeSubstitute, a);
}

/// Whether `u`'s target hides behind a substitute (`jumpifstatus2 BS_TARGET,
/// STATUS2_SUBSTITUTE`).
pub(crate) fn subbed(b: &Battle, u: &MoveUse) -> bool {
    u.target != u.attacker && b.battler(u.target).fx.substitute
}

/// `Cmd_healthbarupdate` + `Cmd_datahpupdate` on a battler whose substitute has HP (move
/// damage, not the attacker's own: its scripts set `HITMARKER_IGNORE_SUBSTITUTE`): the
/// substitute takes the damage ("The SUBSTITUTE took damage for X!"), and fades at 0
/// (`BattleScript_SubstituteFade`). No Bide nor Counter record, no `TARGET_TURN_DAMAGED`.
/// `gHpDealt` (the substitute's HP lost), or `None` when the battler takes it.
pub(crate) fn sub_takes(b: &mut Battle, env: &mut Env, side: Side, damage: i32) -> Option<u16> {
    let c = b.fx.cur?;
    let fx = b.battler(side).fx;
    if damage <= 0 || c.attacker == side || !fx.substitute || fx.sub_hp == 0 {
        return None;
    }
    let about = |id| {
        BattleEvent::Msg(Msg {
            mov: Some(c.mov),
            ..msg(id, c.attacker, side, side)
        })
    };
    env.out.push(about(Str::SubstituteDamaged));
    let dealt = damage.min(fx.sub_hp as i32);
    let left = fx.sub_hp - dealt as u8;
    b.battler_mut(side).fx.sub_hp = left;
    if left == 0 {
        env.out.push(about(Str::PkmnSubstituteFaded));
    }
    Some(dealt as u16)
}

/// `SetMoveEffect`: a secondary effect on a target behind a substitute does nothing, unless
/// it is `MOVE_EFFECT_AFFECTS_USER` (recoil, recharge, Rapid Spin, Superpower, Overheat, the
/// user's stat raises, Thrash, Uproar). The bit is read, so the hit that breaks the
/// substitute still blocks it (`MOVEEND_SUBSTITUTE` clears it after).
pub(crate) fn sub_blocks_secondary(b: &Battle, u: &MoveUse) -> bool {
    use MoveEffect as E;
    subbed(b, u)
        && !matches!(
            u.info.effect,
            E::Recoil
                | E::DoubleEdge
                | E::Recharge
                | E::RapidSpin
                | E::Superpower
                | E::Overheat
                | E::StatUpHit(_)
                | E::AllStatsUpHit
                | E::Rampage
                | E::Uproar
        )
}

/// A hit on `side` went to its substitute (`TARGET_TURN_DAMAGED` stays false): the contact
/// abilities, Color Change, the defrost and King's Rock do not trigger.
pub(crate) fn sub_absorbed(b: &Battle, side: Side) -> bool {
    b.battler(side).fx.substitute && !b.fx.cur.is_some_and(|c| c.hit[side.i()])
}

/// `MOVEEND_SUBSTITUTE`: a substitute at 0 HP is gone.
pub(in crate::battle::effects) fn sub_end(b: &mut Battle) {
    for side in [Side::Player, Side::Opponent] {
        let fx = &mut b.battler_mut(side).fx;
        if fx.substitute && fx.sub_hp == 0 {
            fx.substitute = false;
        }
    }
}

/// `BattleScript_EffectBatonPass`: `jumpifcantswitch SWITCH_IGNORE_ESCAPE_PREVENTION` (no
/// other mon that can fight: "But it failed!"), the animation, then `openpartyscreen`: the
/// player chooses (`Need::Switch`, the turn goes on after), the opponent sends its first usable
/// mon at once (the simplified `GetMostSuitableMonToSwitchInto` of `Battle::replace`).
pub(super) fn baton_pass(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    b.attack_string(env, u);
    let a = u.attacker;
    let n = b.parties[a.s()].len() as u8;
    let Some(next) = (0..n).find(|&i| b.can_switch_to(a, i)) else {
        b.fail(env, u);
        return;
    };
    anim(env, u);
    b.battler_mut(a).fx.baton = true;
    if b.player_chooses_switch(a) {
        b.ask_switch(a);
    } else {
        b.switch_out(env, a);
        b.switch_in(env, a, next);
    }
}

/// `Cmd_switchindataupdate` + `SwitchInClearSetData` after Baton Pass: the new battler on
/// `side` keeps `old`'s stat stages, confusion, Focus Energy, substitute, escape prevention,
/// curse, Leech Seed, Lock-On on it, Perish Song count, Ingrain and the sports. On the foe:
/// its infatuation and wrap by `old` end, its escape prevention stays, a Lock-On by `old`
/// gets `STATUS3_ALWAYS_HITS_TURN(2)`.
pub(crate) fn baton_pass_in(b: &mut Battle, old: &Battler, side: Side) {
    let m = b.battler_mut(side);
    m.stages = old.stages;
    m.confusion = old.confusion;
    m.focus_energy = old.focus_energy;
    m.seeded = old.seeded;
    m.v.escape_prevented = old.v.escape_prevented;
    m.v.mud_sport = old.v.mud_sport;
    m.v.water_sport = old.v.water_sport;
    m.fx.substitute = old.fx.substitute;
    m.fx.sub_hp = old.fx.sub_hp;
    m.fx.cursed = old.fx.cursed;
    m.fx.lock_on = old.fx.lock_on;
    m.fx.perish = old.fx.perish;
    m.fx.rooted = old.fx.rooted;
    let foe = b.battler_mut(side.other());
    foe.v.infatuated = false;
    foe.fx.wrapped = 0;
    foe.fx.wrap_move = None;
    if foe.fx.lock_on > 0 {
        foe.fx.lock_on = 2;
    }
}
