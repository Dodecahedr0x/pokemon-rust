//! Abilities: `moveend`'s defrost, Synchronize and contact abilities
//! (`ABILITYEFFECT_ON_DAMAGE`), Intimidate and Trace on switch-in, Keen Eye.
//!
//! Shield Dust is in [`super::status`] (`SetMoveEffect`) and [`super::moves`] (secondary stat
//! drops); Guts is in `calc::base_damage`. Pickup, the weather abilities and Early Bird have no
//! effect in the slice (no items after battle, no weather, Early Bird is `Ability::Other`).

use super::status::{self, Cause};
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, MoveFlags, Stat, Status, StatusKind, Type};

/// `Cmd_moveend` cases `MOVEEND_DEFROST` … `MOVEEND_SYNCHRONIZE_ATTACKER`.
pub(super) fn move_end(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    defrost(b, env, u, hit);
    synchronize(b, env, u, u.target, u.attacker);
    on_damage(b, env, u, hit);
    synchronize(b, env, u, u.attacker, u.target);
}

/// `MOVEEND_DEFROST`: a Fire move that dealt damage thaws its frozen target
/// (`BattleScript_DefrostedViaFireMove`).
fn defrost(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    let Some(r) = hit else { return };
    let t = b.battler(u.target);
    if t.mon.status == Status::Freeze
        && t.mon.hp != 0
        && u.attacker != u.target
        && r.hp_dealt > 0
        && r.eff != Effectiveness::NoEffect
        && u.ty == Type::Fire
    {
        b.battler_mut(u.target).mon.status = Status::None;
        b.write_back(u.target);
        env.out.push(BattleEvent::Msg(
            u.msg_long(Str::PkmnWasDefrosted, u.target),
        ));
        env.out.push(BattleEvent::StatusIcon {
            side: u.target,
            status: Status::None,
        });
    }
}

/// `ABILITYEFFECT_SYNCHRONIZE` (owner = target) / `ABILITYEFFECT_ATK_SYNCHRONIZE` (owner =
/// attacker): the status the action inflicted (toxic as poison) goes to `to`, as a primary
/// ability effect (`BattleScript_SynchronizeActivates`).
fn synchronize(b: &mut Battle, env: &mut Env, u: &MoveUse, owner: Side, to: Side) {
    if b.battler(owner).ability != Ability::Synchronize {
        return;
    }
    let Some(kind) = b.sync.take() else { return };
    let kind = match kind {
        StatusKind::Toxic => StatusKind::Poison,
        k => k,
    };
    let cause = Cause {
        primary: true,
        ability: Some((owner, Ability::Synchronize)),
    };
    status::set_status(b, env, u, to, kind, cause);
}

/// `ABILITYEFFECT_ON_DAMAGE` of the target's ability on a contact move that damaged it: Effect
/// Spore (10%: sleep, poison or paralysis), Static and Poison Point (1/3) on the attacker, as a
/// secondary ability effect (`BattleScript_ApplySecondaryEffect`).
fn on_damage(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    let Some(r) = hit else { return };
    let owner = u.target;
    let ability = b.battler(owner).ability;
    if !matches!(
        ability,
        Ability::EffectSpore | Ability::Static | Ability::PoisonPoint
    ) {
        return;
    }
    if r.eff == Effectiveness::NoEffect
        || owner == u.attacker
        || b.battler(u.attacker).mon.hp == 0
        || r.hp_dealt == 0
        || !u.info.flags.has(MoveFlags::MAKES_CONTACT)
    {
        return;
    }
    let kind = match ability {
        Ability::EffectSpore => {
            if !env.rng.random().is_multiple_of(10) {
                return;
            }
            // `Random() & 3` until non-zero: sleep, poison, burn → paralysis.
            let mut k = 0;
            while k == 0 {
                k = env.rng.random() & 3;
            }
            match k {
                1 => StatusKind::Sleep,
                2 => StatusKind::Poison,
                _ => StatusKind::Paralysis,
            }
        }
        Ability::Static if env.rng.random().is_multiple_of(3) => StatusKind::Paralysis,
        Ability::PoisonPoint if env.rng.random().is_multiple_of(3) => StatusKind::Poison,
        _ => return,
    };
    let cause = Cause {
        primary: false,
        ability: Some((owner, ability)),
    };
    status::set_status(b, env, u, u.attacker, kind, cause);
}

/// `ABILITYEFFECT_INTIMIDATE1` → `BattleScript_IntimidateActivates`: the foe's Attack -1
/// (nothing at -6), with `STRINGID_PKMNCUTSATTACKWITH`.
pub(super) fn intimidate(b: &mut Battle, env: &mut Env, side: Side) {
    let foe = side.other();
    if b.battler(side).ability != Ability::Intimidate
        || b.battler(side).mon.hp == 0
        || b.battler(foe).mon.hp == 0
    {
        return;
    }
    if !b.change_stage(foe, Stat::Atk, -1) {
        return;
    }
    env.out.push(BattleEvent::StatAnim {
        side: foe,
        up: false,
    });
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::Intimidate)),
        ],
        wait: LONG,
        ..msg(Str::PkmnCutsAttackWith, side, foe, side)
    }));
}

/// `ABILITYEFFECT_TRACE` → `BattleScript_TraceActivates`: copies the foe's ability when it has
/// one and HP.
pub(super) fn trace(b: &mut Battle, env: &mut Env, side: Side) {
    let foe = side.other();
    let f = b.battler(foe);
    if b.battler(side).ability != Ability::Trace
        || b.battler(side).mon.hp == 0
        || f.ability == Ability::None
        || f.mon.hp == 0
    {
        return;
    }
    let (ability, party) = (f.ability, f.mon.party);
    b.battler_mut(side).ability = ability;
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::Mon { side: foe, party },
            Buff::Text(Str::AbilityName(ability)),
            Buff::Text(Str::AbilityName(Ability::Trace)),
        ],
        wait: LONG,
        ..msg(Str::PkmnTraced, side, foe, side)
    }));
}

/// `ChangeStatBuffs` with `STAT_CHANGE_ALLOW_PTR` (a status move's drop): Keen Eye keeps its
/// accuracy (`BattleScript_AbilityNoSpecificStatLoss`). True when blocked.
pub(super) fn blocks_stat_drop(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    stat: Stat,
) -> bool {
    if b.battler(side).ability == Ability::KeenEye && stat == Stat::Accuracy {
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::Stat(stat),
                Buff::None,
                Buff::Text(Str::AbilityName(Ability::KeenEye)),
            ],
            ..u.msg_long(Str::PkmnSXPreventsYLoss, side)
        }));
        return true;
    }
    false
}

/// `ChangeStatBuffs` without `STAT_CHANGE_ALLOW_PTR` (a hit's secondary drop): Keen Eye
/// (accuracy) and Shield Dust block it silently.
pub(super) fn blocks_secondary_drop(b: &Battle, side: Side, stat: Stat) -> bool {
    let a = b.battler(side).ability;
    (a == Ability::KeenEye && stat == Stat::Accuracy) || a == Ability::ShieldDust
}
