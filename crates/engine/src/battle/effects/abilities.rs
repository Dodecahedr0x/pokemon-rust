//! Abilities: `moveend`'s defrost, Synchronize and contact abilities
//! (`ABILITYEFFECT_ON_DAMAGE`: Effect Spore, Static, Poison Point, Flame Body, Rough Skin,
//! Color Change, Cute Charm), the status-curing abilities (`ABILITYEFFECT_IMMUNITY`:
//! Immunity, Own Tempo, Limber, Insomnia, Vital Spirit, Water Veil, Magma Armor, Oblivious),
//! the absorbing abilities (`ABILITYEFFECT_ABSORBING`: Volt / Water Absorb, Flash Fire),
//! Wonder Guard (`Cmd_typecalc`), Forecast (`CastformDataTypeChange`), Damp
//! (`Cmd_tryexplosion`), Sticky Hold, Suction Cups, Intimidate and Trace on switch-in, Keen
//! Eye and Hyper Cutter, Soundproof (`ABILITYEFFECT_MOVES_BLOCK`), the end-turn abilities
//! (Shed Skin, Truant), Clear Body and White Smoke (stat drops, Intimidate).
//!
//! Elsewhere: Immunity, Limber, Water Veil, Magma Armor and Own Tempo's blocks in
//! [`super::status`] and [`super::moves`]; Pressure in `Battle::attack_string`; Serene Grace
//! in `Battle::secondary`; Cloud Nine and Air Lock in `Battle::weather`; Plus, Minus and
//! Flash Fire's boost in `calc::base_damage_x`; Arena Trap in `effects::run_block`.
//! Cacophony does nothing in Gen3; Lightning Rod (Gen3) only redirects in doubles.
//!
//! Elsewhere: Shield Dust and Inner Focus in [`super::status`] (`SetMoveEffect`) and
//! [`super::moves`] (secondary stat drops); Insomnia / Vital Spirit / Early Bird / Truant's
//! canceler in [`super::status`]; Oblivious in Attract; Magnet Pull in `effects::run_block`;
//! Rock Head in the core's recoil; Natural Cure in the core's switch-out; Liquid Ooze in
//! Absorb and Leech Seed; Huge Power, Pure Power, Hustle, Guts, Thick Fat and Marvel
//! Scale in `calc::base_damage_x`; Compound Eyes and Hustle in `calc::accuracy_hits_x`;
//! Swift Swim and Chlorophyll in `Battle::strike_order`; the weather and post-Gen3 abilities
//! in [`super::later`]. Pickup and Rain Dish have no effect.

use super::status::{self, Cause};
use crate::battle::core::{Battle, Env, HitResult, LONG, MoveUse, msg};
use crate::battle::{BattleEvent, BattleReport, Buff, Effectiveness, Msg, Side, Str};
use crate::mon::{Ability, Gender, MoveFlags, Stat, Status, StatusKind, Type};

/// `Cmd_moveend` cases `MOVEEND_DEFROST` … `MOVEEND_SYNCHRONIZE_ATTACKER`.
pub(super) fn move_end(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    defrost(b, env, u, hit);
    synchronize(b, env, u, u.target, u.attacker);
    on_damage(b, env, u, hit);
    immunity(b, env);
    synchronize(b, env, u, u.attacker, u.target);
}

/// `MOVEEND_IMMUNITY_ABILITIES` (`ABILITYEFFECT_IMMUNITY`): the first battler (index order)
/// whose ability cures what it has is cured (`BattleScript_AbilityCuredStatus`), again until
/// none. `{B_BUFF1}` is a `gStatusConditionString_*Jpn` the content does not carry: empty.
pub(super) fn immunity(b: &mut Battle, env: &mut Env) {
    loop {
        let found = b.sides().find(|&s| {
            let m = b.battler(s);
            match m.ability {
                Ability::Immunity => matches!(m.mon.status, Status::Poison | Status::Toxic(_)),
                Ability::OwnTempo => m.confusion > 0,
                Ability::Limber => m.mon.status == Status::Paralysis,
                Ability::Insomnia | Ability::VitalSpirit => {
                    matches!(m.mon.status, Status::Sleep(_))
                }
                Ability::WaterVeil => m.mon.status == Status::Burn,
                Ability::MagmaArmor => m.mon.status == Status::Freeze,
                Ability::Oblivious => m.v.infatuated,
                _ => false,
            }
        });
        let Some(side) = found else { return };
        let m = b.battler_mut(side);
        let ability = m.ability;
        match ability {
            Ability::OwnTempo => m.confusion = 0,
            Ability::Oblivious => m.v.infatuated = false,
            _ => m.mon.status = Status::None,
        }
        b.write_back(side);
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(ability)),
            ],
            wait: LONG,
            ..msg(Str::PkmnSXCuredItsYProblem, side, side, side)
        }));
        env.out.push(BattleEvent::Report(BattleReport::Degraded(
            "ability cure status name",
        )));
        env.out.push(BattleEvent::StatusIcon {
            side,
            status: b.battler(side).mon.status,
        });
    }
}

/// `ABILITYEFFECT_ABSORBING` (`JumpIfMoveFailed`, once the move hit): the target's Volt
/// Absorb / Water Absorb take a damaging move of their type (`BattleScript_MoveHPDrain`: max
/// HP / 4, at least 1; at full HP `BattleScript_MonMadeMoveUseless`); Flash Fire takes a Fire
/// move unless frozen (`BattleScript_FlashFireBoost`, `RESOURCE_FLAG_FLASH_FIRE`). Each prints
/// the attack string (and pays the PP) first. True when absorbed: the move does nothing more.
pub(super) fn absorbs(b: &mut Battle, env: &mut Env, u: &MoveUse) -> bool {
    let side = u.target;
    if side == u.attacker {
        return false;
    }
    let t = b.battler(side);
    let (ability, hp, max, frozen) = (
        t.ability,
        t.mon.hp,
        t.mon.max_hp,
        t.mon.status == Status::Freeze,
    );
    let drain = match ability {
        Ability::VoltAbsorb => u.ty == Type::Electric && u.info.power != 0,
        Ability::WaterAbsorb => u.ty == Type::Water && u.info.power != 0,
        Ability::FlashFire if u.ty == Type::Fire && !frozen => {
            b.attack_string(env, u);
            let boosted = b.battler(side).x.flash_fire;
            b.battler_mut(side).x.flash_fire = true;
            let id = if boosted {
                Str::PkmnSXMadeYIneffective
            } else {
                Str::PkmnRaisedFirePowerWith
            };
            env.out
                .push(BattleEvent::Msg(ability_msg(u, id, side, ability)));
            return true;
        }
        _ => false,
    };
    if !drain {
        return false;
    }
    b.attack_string(env, u);
    if hp == max {
        env.out.push(BattleEvent::Msg(ability_msg(
            u,
            Str::PkmnSXMadeYUseless,
            side,
            ability,
        )));
    } else {
        let heal = (max / 4).max(1).min(max - hp);
        b.change_hp_passive(env, side, -(heal as i32));
        env.out.push(BattleEvent::Msg(ability_msg(
            u,
            Str::PkmnRestoredHpUsing,
            side,
            ability,
        )));
    }
    true
}

/// A message about `side` in the move's context naming `ability` (`buffs[2]`).
fn ability_msg(u: &MoveUse, id: Str, side: Side, ability: Ability) -> Msg {
    Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(ability)),
        ],
        ..u.msg_long(id, side)
    }
}

/// `Cmd_typecalc`'s Wonder Guard: a move with power (not Struggle) that is not super effective
/// on the target "misses" (`B_MSG_AVOIDED_DMG`, which wins over "doesn't affect"). True when
/// it did, with the message.
pub(super) fn wonder_guard(b: &mut Battle, env: &mut Env, u: &MoveUse, eff: Effectiveness) -> bool {
    if b.battler(u.target).ability != Ability::WonderGuard
        || u.target == u.attacker
        || u.info.power == 0
        || u.mov == env.t.struggle
        || eff == Effectiveness::Super
        || b.levitates(env, u)
    {
        return false;
    }
    env.out.push(BattleEvent::Msg(ability_msg(
        u,
        Str::AvoidedDamage,
        u.target,
        Ability::WonderGuard,
    )));
    true
}

/// `CastformDataTypeChange` for `side`'s Forecast: Normal without weather (or in a
/// sandstorm), Fire in the sun, Water in the rain (the engine has no hail), then
/// `BattleScript_CastformChange` ("X transformed!"; the form's sprite has no event). True when
/// it changed.
pub(super) fn forecast(b: &mut Battle, env: &mut Env, side: Side) -> bool {
    use crate::battle::core::Weather;
    let m = b.battler(side);
    if m.ability != Ability::Forecast || m.mon.hp == 0 {
        return false;
    }
    let to = match b.weather() {
        Weather::Sun | Weather::HarshSun => Type::Fire,
        Weather::Rain | Weather::HeavyRain => Type::Water,
        // CastformDataTypeChange: hail is Ice.
        Weather::Hail => Type::Ice,
        Weather::None | Weather::Sand => Type::Normal,
    };
    if m.types.contains(&to) {
        return false;
    }
    b.battler_mut(side).types = [to, to];
    env.out.push(BattleEvent::Report(BattleReport::Degraded(
        "castform form sprite",
    )));
    env.out.push(BattleEvent::Msg(Msg {
        wait: LONG,
        ..msg(Str::PkmnTransformed, side, side, side)
    }));
    true
}

/// `ABILITYEFFECT_FORECAST`: the first Forecast battler (index order) that changes.
pub(super) fn forecast_all(b: &mut Battle, env: &mut Env) {
    for side in b.sides() {
        if forecast(b, env, side) {
            return;
        }
    }
}

/// `Cmd_tryexplosion`'s Damp: a battler with Damp (and any HP) on the field prevents the
/// move (`BattleScript_DampStopsExplosion`: "X's DAMP prevents Y from using Z!"). True when
/// stopped, with the message.
#[allow(dead_code)] // for the move effects that call it
pub(crate) fn damp(b: &mut Battle, env: &mut Env, u: &MoveUse) -> bool {
    let Some(owner) = b
        .sides()
        .into_iter()
        .find(|&s| b.battler(s).ability == Ability::Damp && b.battler(s).mon.hp > 0)
    else {
        return false;
    };
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::Damp)),
        ],
        wait: LONG,
        mov: Some(u.mov),
        ..msg(Str::PkmnPreventsUsage, u.attacker, owner, owner)
    }));
    true
}

/// Sticky Hold keeps `side`'s item from Thief / Covet / Trick / Knock Off
/// (`MOVE_EFFECT_STEAL_ITEM`, `Cmd_tryswapitems`, `BattleScript_StickyHoldActivates`).
#[allow(dead_code)] // for the move effects that call it
pub(crate) fn sticky_hold(b: &Battle, side: Side) -> bool {
    b.battler(side).ability == Ability::StickyHold
}

/// Suction Cups keeps `side` from Roar / Whirlwind (`Cmd_forcerandomswitch`'s
/// `BattleScript_AbilityPreventsPhasingOut`).
#[allow(dead_code)] // for the move effects that call it
pub(crate) fn suction_cups(b: &Battle, side: Side) -> bool {
    b.battler(side).ability == Ability::SuctionCups
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
        && !super::mech::sub_absorbed(b, u.target)
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

/// `Cmd_moveend` from `MOVEEND_ON_DAMAGE_ABILITIES` to `MOVEEND_SYNCHRONIZE_ATTACKER`
/// (`BattleScript_PursuitDmgOnSwitchOut`'s `moveendfromto`).
pub(super) fn move_end_from_on_damage(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    hit: Option<HitResult>,
) {
    on_damage(b, env, u, hit);
    immunity(b, env);
    synchronize(b, env, u, u.attacker, u.target);
}

/// `ABILITYEFFECT_ON_DAMAGE` of the target's ability on a contact move that damaged it: Effect
/// Spore (10%: sleep, poison or paralysis), Static, Poison Point and Flame Body (1/3) on the
/// attacker, as a secondary ability effect (`BattleScript_ApplySecondaryEffect`); Cute Charm
/// (1/3) infatuates it; Rough Skin hurts it (max HP / 16). Color Change (any damaging move but
/// Struggle) turns the target into the move's type.
fn on_damage(b: &mut Battle, env: &mut Env, u: &MoveUse, hit: Option<HitResult>) {
    let Some(r) = hit else { return };
    let owner = u.target;
    let ability = b.battler(owner).ability;
    if ability == Ability::ColorChange {
        color_change(b, env, u, r);
        return;
    }
    if !matches!(
        ability,
        Ability::EffectSpore
            | Ability::Static
            | Ability::PoisonPoint
            | Ability::CuteCharm
            | Ability::FlameBody
            | Ability::RoughSkin
    ) {
        return;
    }
    if r.eff == Effectiveness::NoEffect
        || owner == u.attacker
        || b.battler(u.attacker).mon.hp == 0
        || r.hp_dealt == 0
        || super::mech::sub_absorbed(b, u.target)
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
        Ability::FlameBody if env.rng.random().is_multiple_of(3) => StatusKind::Burn,
        Ability::RoughSkin => {
            rough_skin(b, env, u);
            return;
        }
        Ability::CuteCharm => {
            cute_charm(b, env, u);
            return;
        }
        _ => return,
    };
    let cause = Cause {
        primary: false,
        ability: Some((owner, ability)),
    };
    status::set_status(b, env, u, u.attacker, kind, cause);
}

/// Color Change: a damaging move (power, not Struggle) of a type the target does not have
/// makes it that type (`BattleScript_ColorChangeActivates`).
fn color_change(b: &mut Battle, env: &mut Env, u: &MoveUse, r: HitResult) {
    let t = b.battler(u.target);
    if r.eff == Effectiveness::NoEffect
        || u.target == u.attacker
        || u.mov == env.t.struggle
        || u.info.power == 0
        || r.hp_dealt == 0
        || super::mech::sub_absorbed(b, u.target)
        || t.types.contains(&u.ty)
        || t.mon.hp == 0
    {
        return;
    }
    b.battler_mut(u.target).types = [u.ty, u.ty];
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::Type(u.ty),
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::ColorChange)),
        ],
        ..u.msg_long(Str::PkmnChangedTypeWith, u.target)
    }));
}

/// Rough Skin (`BattleScript_RoughSkinActivates`): the attacker loses max HP / 16 (at least
/// 1), "X's ROUGH SKIN hurt Y!", then its faint check.
fn rough_skin(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    let dmg = (b.battler(u.attacker).mon.max_hp / 16).max(1);
    b.change_hp_passive(env, u.attacker, dmg as i32);
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::RoughSkin)),
        ],
        ..u.msg_long(Str::PkmnHurtsWith, u.target)
    }));
    b.try_faint(env, u.attacker);
}

/// Cute Charm (the target's, on a damaging contact move): `Random() % 3 == 0`, then a
/// non-Oblivious attacker of the other gender (neither genderless) that is not infatuated yet
/// falls in love with the target (`BattleScript_CuteCharmActivates`).
fn cute_charm(b: &mut Battle, env: &mut Env, u: &MoveUse) {
    if b.battler(u.target).mon.hp == 0 || !env.rng.random().is_multiple_of(3) {
        return;
    }
    let (a, t) = (b.battler(u.attacker), b.battler(u.target));
    if a.ability == Ability::Oblivious
        || !opposite_genders(a.mon.gender, t.mon.gender)
        || a.v.infatuated
    {
        return;
    }
    b.battler_mut(u.attacker).v.infatuated = true;
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::CuteCharm)),
        ],
        ..u.msg_long(Str::PkmnSXInfatuatedY, u.target)
    }));
}

/// Two battlers that can fall for each other: different genders, neither genderless.
pub(crate) fn opposite_genders(x: Gender, y: Gender) -> bool {
    x != y && x != Gender::Genderless && y != Gender::Genderless
}

/// `ABILITYEFFECT_MOVES_BLOCK`: the target's Soundproof stops a sound move
/// (`BattleScript_SoundproofProtected`: the attack string and PP, unless the move is locked
/// in, then "X's SOUNDPROOF blocks Y!").
pub(super) fn moves_block(b: &mut Battle, env: &mut Env, u: &MoveUse) -> super::Canceler {
    if b.battler(u.target).ability != Ability::Soundproof || !u.info.flags.has(MoveFlags::SOUND) {
        return super::Canceler::Use;
    }
    if b.battler(u.attacker).v.uproar > 0 {
        env.out
            .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
    } else {
        b.attack_string(env, u);
    }
    env.out.push(BattleEvent::Msg(Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::Soundproof)),
        ],
        ..u.msg_long(Str::PkmnSXBlocksY, u.target)
    }));
    super::Canceler::Done(*u, None)
}

/// `ABILITYEFFECT_ENDTURN` for a battler with HP: Shed Skin cures its status when
/// `Random() % 3 == 0` (`BattleScript_ShedSkinActivates`; `B_BUFF1` would name the status
/// with Gen3's `gStatusConditionString_*Jpn`, which the engine does not have: empty);
/// Truant flips its counter.
pub(super) fn end_turn(b: &mut Battle, env: &mut Env, side: Side) {
    let m = b.battler(side);
    if m.mon.hp == 0 {
        return;
    }
    match m.ability {
        Ability::ShedSkin if m.mon.status != Status::None => {
            if !env.rng.random().is_multiple_of(3) {
                return;
            }
            b.battler_mut(side).mon.status = Status::None;
            b.write_back(side);
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [
                    Buff::None,
                    Buff::None,
                    Buff::Text(Str::AbilityName(Ability::ShedSkin)),
                ],
                wait: LONG,
                ..msg(Str::PkmnSXCuredYProblem, side, side.other(), side)
            }));
            // The message's {B_BUFF1} is the status name (`gStatusConditionString_*`), which
            // the engine does not carry: it prints empty.
            env.out.push(BattleEvent::Report(BattleReport::Degraded(
                "shed skin status name",
            )));
            env.out.push(BattleEvent::StatusIcon {
                side,
                status: Status::None,
            });
        }
        Ability::Truant => {
            let v = &mut b.battler_mut(side).v;
            v.truant = !v.truant;
        }
        _ => {}
    }
}

/// `ABILITYEFFECT_INTIMIDATE1` → `BattleScript_IntimidateActivates`: the foe's Attack -1
/// (nothing at -6), with `STRINGID_PKMNCUTSATTACKWITH`.
/// Doubles: each foe on the field in battler order (`trygetintimidatetarget`).
pub(super) fn intimidate(b: &mut Battle, env: &mut Env, side: Side) {
    if !b.is_double() {
        intimidate_one(b, env, side, side.other());
        return;
    }
    for foe in b.sides() {
        if foe.side() != side.side() && b.is_present(foe) {
            intimidate_one(b, env, side, foe);
        }
    }
}

fn intimidate_one(b: &mut Battle, env: &mut Env, side: Side, foe: Side) {
    // `jumpifstatus2 BS_TARGET, STATUS2_SUBSTITUTE`: a substitute (Baton Pass) is skipped.
    if b.battler(side).ability != Ability::Intimidate
        || b.battler(side).mon.hp == 0
        || b.battler(foe).mon.hp == 0
        || b.battler(foe).fx.substitute
    {
        return;
    }
    // BattleScript_IntimidatePrevented (Clear Body, Hyper Cutter, White Smoke), before
    // `statbuffchange`.
    let blocker = b.battler(foe).ability;
    if matches!(
        blocker,
        Ability::ClearBody | Ability::HyperCutter | Ability::WhiteSmoke
    ) {
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::Text(Str::AbilityName(Ability::Intimidate)),
                Buff::None,
                Buff::Text(Str::AbilityName(blocker)),
            ],
            wait: LONG,
            ..msg(Str::PreventedFromWorking, side, foe, side)
        }));
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
    let ok =
        |b: &Battle, s: Side| b.battler(s).ability != Ability::None && b.battler(s).mon.hp != 0;
    let left = side.other().side();
    // Doubles: a random one of the two foes when both qualify (`Random() & 1`: the right).
    let foe = if !b.is_double()
        || b.battler(side).ability != Ability::Trace
        || b.battler(side).mon.hp == 0
    {
        left
    } else {
        match (ok(b, left), ok(b, left.partner())) {
            (true, true) if env.rng.random() & 1 != 0 => left.partner(),
            (false, true) => left.partner(),
            _ => left,
        }
    };
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

/// `ChangeStatBuffs` with `STAT_CHANGE_ALLOW_PTR` (a status move's drop): Clear Body and White
/// Smoke keep every stat (`BattleScript_AbilityNoStatLoss`), Keen Eye its accuracy and Hyper
/// Cutter its attack (`BattleScript_AbilityNoSpecificStatLoss`). True when blocked.
pub(super) fn blocks_stat_drop(
    b: &mut Battle,
    env: &mut Env,
    u: &MoveUse,
    side: Side,
    stat: Stat,
) -> bool {
    let ability = b.battler(side).ability;
    if matches!(ability, Ability::ClearBody | Ability::WhiteSmoke) {
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::None,
                Buff::None,
                Buff::Text(Str::AbilityName(ability)),
            ],
            ..u.msg_long(Str::PkmnPreventsStatLossWith, side)
        }));
        return true;
    }
    if (ability == Ability::KeenEye && stat == Stat::Accuracy)
        || (ability == Ability::HyperCutter && stat == Stat::Atk)
    {
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::Stat(stat),
                Buff::None,
                Buff::Text(Str::AbilityName(ability)),
            ],
            ..u.msg_long(Str::PkmnSXPreventsYLoss, side)
        }));
        return true;
    }
    false
}

/// `ChangeStatBuffs` without `STAT_CHANGE_ALLOW_PTR` (a hit's secondary drop): Clear Body,
/// White Smoke, Keen Eye (accuracy), Hyper Cutter (attack) and Shield Dust block it silently.
pub(super) fn blocks_secondary_drop(b: &Battle, side: Side, stat: Stat) -> bool {
    let a = b.battler(side).ability;
    (a == Ability::KeenEye && stat == Stat::Accuracy)
        || (a == Ability::HyperCutter && stat == Stat::Atk)
        || a == Ability::ShieldDust
        || a == Ability::ClearBody
        || a == Ability::WhiteSmoke
}
