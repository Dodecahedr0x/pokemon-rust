//! Move execution: the `BattleScript_EffectHit` pipeline and the effects Task D implements
//! (`Hit`, `QuickAttack`, `StatUp`, `StatDown`, `Recoil`), plus the building blocks Task E2's
//! [`effects`] use (accuracy, damage, HP changes, stat stages, secondary effects, faints).
//!
//! Script order (`data/battle_scripts_1.s`): `attackcanceler` ([`effects::before_move`]),
//! `accuracycheck`, `attackstring` + `ppreduce`, `critcalc`, `damagecalc`, `typecalc`,
//! `adjustnormaldamage`, the animations and `healthbarupdate`/`datahpupdate`, `critmessage`,
//! `resultmessage`, `seteffectwithchance`, `tryfaintmon`.

use super::super::effects;
use super::{Action, Battle, Env, LONG, msg};
use crate::battle::calc;
use crate::battle::{BattleEvent, BattleReport, Buff, Effectiveness, Msg, Side, Str};
use crate::data::MoveId;
use crate::mon::{Ability, HoldEffect, MoveEffect, MoveFlags, MoveInfo, MoveTarget, Stat, Type};

/// The move being used (Gen3 `gCurrentMove`, `gBattlerAttacker`, `gBattlerTarget`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct MoveUse {
    pub attacker: Side,
    pub target: Side,
    pub mov: MoveId,
    /// The move slot, `None` for Struggle (no PP).
    pub slot: Option<u8>,
    pub info: &'static MoveInfo,
    /// `gBattleMovePower` / the move type, overridable (`gDynamicBasePower`, Bide).
    pub power: u8,
    pub ty: Type,
    /// `gBattleScripting.dmgMultiplier`: 1, or 2 for Pursuit on a switching foe.
    pub multiplier: u8,
    /// `HITMARKER_ATTACKSTRING_PRINTED | HITMARKER_NO_PPDEDUCT`: the attack string was printed
    /// and the PP paid already (a move bounced back by Magic Bounce).
    pub printed: bool,
    /// Parental Bond's second hit: a quarter of the damage (before the random roll).
    pub quarter: bool,
    /// `gDynamicBasePower` above 255 (Rollout / Ice Ball, Wave C): the power when non-zero.
    pub dynamic: u16,
}

impl MoveUse {
    /// A message about `active` in this move's context (`B_CURRENT_MOVE` = `mov`).
    pub fn msg(&self, id: Str, active: Side) -> Msg {
        Msg {
            mov: Some(self.mov),
            ..msg(id, self.attacker, self.target, active)
        }
    }

    /// The same message, waiting `B_WAIT_TIME_LONG`.
    pub fn msg_long(&self, id: Str, active: Side) -> Msg {
        Msg {
            wait: LONG,
            ..self.msg(id, active)
        }
    }
}

/// What a damaging hit did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct HitResult {
    /// `gBattleMoveDamage` after every multiplier.
    pub damage: i32,
    /// `gHpDealt`: the HP the target lost.
    pub hp_dealt: u16,
    pub crit: bool,
    pub eff: Effectiveness,
    /// `MOVE_RESULT_FOE_HUNG_ON`: the target's Focus Band left it at 1 HP.
    pub hung: bool,
}

/// A hit's secondary effect (`setmoveeffect` + `seteffectwithchance`): `certain` effects
/// (`MOVE_EFFECT_CERTAIN`) apply without a roll; others apply when `Random() % 100` is below
/// the move's `effect_chance`.
#[derive(Clone, Copy)]
pub(crate) struct Secondary {
    pub certain: bool,
    pub apply: fn(&mut Battle, &mut Env, &MoveUse, &HitResult),
}

impl Battle {
    /// `HandleAction_UseMove` for `side`'s chosen move (or Struggle).
    pub(crate) fn use_move(&mut self, env: &mut Env, side: Side, action: Action) {
        let b = self.battler(side);
        let (mov, slot) = match action {
            Action::Move(s) => match b.mon.moves[s as usize] {
                Some(m) => (m, Some(s)),
                None => (env.t.struggle, None),
            },
            _ => (env.t.struggle, None),
        };
        let u = self.move_use(env, side, mov, slot);
        self.sync = None;
        self.no_effect = false;
        // Mold Breaker: the target's breakable ability is off for the move (restored below).
        let broken = effects::break_mold(self, &u);
        // MOVEEND_UPDATE_LAST_MOVES: a move cancelled before `HITMARKER_OBEYS` (sleep,
        // paralysis, flinch, confusion…) leaves MOVE_UNAVAILABLE on its target.
        let landed = match effects::before_move(self, env, &u) {
            effects::Canceler::Use => {
                let hit = self.execute(env, &u);
                self.battler_mut(side).last_move = Some(mov);
                effects::after_move(self, env, &u, hit);
                self.landed(&u)
            }
            // The canceler ran the move itself (Bide's release): `moveend` still runs.
            effects::Canceler::Done(u, hit) => {
                self.battler_mut(side).last_move = Some(mov);
                effects::after_move(self, env, &u, hit);
                self.landed(&u)
            }
            effects::Canceler::Skip => None,
        };
        if let Some((side, party, ability)) = broken
            && self.battler(side).mon.party == party
        {
            self.battler_mut(side).ability = ability;
        }
        self.battler_mut(u.target).last_landed = landed;
        self.sync = None;
    }

    /// `gLastLandedMoves[target]` after `u` (see [`super::Battler::last_landed`]): the move
    /// when it had power and no `MOVE_RESULT_NO_EFFECT`. Bide's storing turns (power 1, no
    /// result flags in Gen3) count as nothing landed.
    fn landed(&self, u: &MoveUse) -> Option<MoveId> {
        (!self.no_effect && u.power > 0 && u.info.effect != MoveEffect::Bide).then_some(u.mov)
    }

    /// The `MoveUse` of `mov` by `attacker` (singles: user-targeting moves target the user,
    /// as the controllers set `gBattlerTarget` for `MOVE_TARGET_USER`).
    pub(crate) fn move_use(
        &self,
        env: &Env,
        attacker: Side,
        mov: MoveId,
        slot: Option<u8>,
    ) -> MoveUse {
        let info = env.t.move_info(mov);
        let target = match info.target {
            MoveTarget::User | MoveTarget::UserOrSelected => attacker,
            _ => attacker.other(),
        };
        // Aerilate / Pixilate / Refrigerate: Normal moves take the ability's type (Gen7).
        let ty = match (info.ty, ate_type(self.battler(attacker).ability)) {
            (Type::Normal, Some(t)) if mov != env.t.struggle => t,
            (t, _) => t,
        };
        MoveUse {
            attacker,
            target,
            mov,
            slot,
            info,
            power: info.power,
            ty,
            multiplier: 1,
            printed: false,
            quarter: false,
            dynamic: 0,
        }
    }

    /// Runs the move's battle script by effect. The hit's result when the move dealt one (for
    /// [`effects::after_move`]); `None` for a miss or a status move.
    pub(crate) fn execute(&mut self, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
        match u.info.effect {
            MoveEffect::Hit | MoveEffect::QuickAttack => self.hit(env, u, None, false),
            MoveEffect::Recoil => {
                let recoil = Secondary {
                    certain: true,
                    apply: recoil,
                };
                self.hit(env, u, Some(recoil), false)
            }
            MoveEffect::StatUp { stat, stages } => {
                self.stat_up_move(env, u, stat, stages);
                None
            }
            MoveEffect::StatDown { stat, stages } => {
                self.stat_down_move(env, u, stat, stages);
                None
            }
            _ => effects::apply(self, env, u),
        }
    }

    /// The fallback for effects the engine does not implement: a plain hit when the move has
    /// power (accuracy 0 = Gen3's always-hit moves), else "But it failed!".
    /// Reported as [`BattleReport::UnsupportedEffect`].
    pub(crate) fn unsupported(&mut self, env: &mut Env, u: &MoveUse) -> Option<HitResult> {
        env.out.push(BattleEvent::Report(match u.info.effect {
            MoveEffect::Unsupported(n) => BattleReport::UnsupportedEffect(n),
            _ => BattleReport::Degraded("unhandled move effect"),
        }));
        if u.power > 0 {
            self.hit(env, u, None, u.info.accuracy == 0)
        } else {
            self.attack_string(env, u);
            self.fail(env, u);
            None
        }
    }

    /// `BattleScript_EffectHit`: accuracy, damage, secondary effect, faint. `None` on a miss.
    /// Parental Bond (Silice, Gen7 25%): a second hit at a quarter of its damage, then "Hit 2
    /// times!", before the secondary effect ([`parental_bond_applies`]).
    pub(crate) fn hit(
        &mut self,
        env: &mut Env,
        u: &MoveUse,
        secondary: Option<Secondary>,
        sure_hit: bool,
    ) -> Option<HitResult> {
        if !sure_hit && !self.accuracy(env, u) {
            self.missed(env, u);
            return None;
        }
        self.attack_string(env, u);
        let r = self.damage(env, u);
        let mut r = self.deal(env, u, r);
        if self.battler(u.attacker).ability == Ability::ParentalBond
            && u.mov != env.t.struggle
            && parental_bond_applies(u.info.effect)
            && r.eff != Effectiveness::NoEffect
            && self.battler(u.target).mon.hp > 0
            && self.battler(u.attacker).mon.hp > 0
        {
            let u2 = MoveUse {
                quarter: true,
                ..*u
            };
            let r2 = self.damage(env, &u2);
            let r2 = self.deal(env, &u2, r2);
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [Buff::Number(2), Buff::None, Buff::None],
                ..u.msg_long(Str::HitXTimes, u.attacker)
            }));
            r = HitResult {
                hp_dealt: r.hp_dealt + r2.hp_dealt,
                ..r2
            };
        }
        self.secondary(env, u, &r, secondary);
        self.try_faint(env, u.target);
        Some(r)
    }

    /// `attackstring` + `ppreduce`: "X used Y!" and one PP (none for Struggle). Call once per
    /// move.
    pub(crate) fn attack_string(&mut self, env: &mut Env, u: &MoveUse) {
        if u.printed {
            return;
        }
        env.out
            .push(BattleEvent::Msg(u.msg(Str::UsedMove, u.attacker)));
        if let Some(s) = u.slot {
            let pp = &mut self.battler_mut(u.attacker).mon.pp[s as usize];
            *pp = pp.saturating_sub(1);
            self.write_back(u.attacker);
        }
    }

    /// `Cmd_accuracycheck` with `ACC_CURR_MOVE`: one roll. Under Foresight the target's
    /// evasion is ignored; Compound Eyes and Hustle change the odds. `AccuracyCalcHelper`'s
    /// always-hit effects (`ALWAYS_HIT`, `VITAL_THROW`) draw no roll: their scripts call
    /// [`Battle::hit`] with `sure_hit` instead.
    pub(crate) fn accuracy(&mut self, env: &mut Env, u: &MoveUse) -> bool {
        // AccuracyCalcHelper: Lock-On, the semi-invulnerable turn, Thunder in rain (Wave C).
        if let Some(hit) = effects::sure_accuracy(self, u) {
            return hit;
        }
        let (a, d) = (self.battler(u.attacker), self.battler(u.target));
        let eva = if d.foresight {
            0
        } else {
            d.stages[Stat::Evasion as usize]
        };
        let acc = a.stages[Stat::Accuracy as usize];
        calc::accuracy_hits_x(
            effects::move_accuracy(self, u),
            acc,
            eva,
            a.ability,
            u.ty.is_physical(),
            d.ability == Ability::SandVeil && self.weather() == super::Weather::Sand,
            d.x.hold,
            env.rng.random(),
        )
    }

    /// The weather, as moves and abilities see it.
    pub(crate) fn weather(&self) -> super::Weather {
        self.x.weather
    }

    /// Gen3 `CheckWonderGuardAndLevitate` / `Cmd_typecalc`: a Ground move with power against a
    /// Levitate target "misses" (`B_MSG_GROUND_MISS`).
    pub(crate) fn levitates(&self, env: &Env, u: &MoveUse) -> bool {
        u.ty == Type::Ground
            && u.power > 0
            && u.mov != env.t.struggle
            && self.battler(u.target).ability == Ability::Levitate
    }

    /// `ABILITYEFFECT_FIELD_SPORT`: Mud Sport / Water Sport on any battler.
    pub(crate) fn field(&self) -> calc::Field {
        let any = |f: fn(&super::Volatiles) -> bool| self.battlers.iter().any(|b| f(&b.v));
        calc::Field {
            mud_sport: any(|v| v.mud_sport),
            water_sport: any(|v| v.water_sport),
        }
    }

    /// `BattleScript_PrintMoveMissed`: "X used Y!" then "X's attack missed!", or "It doesn't
    /// affect Z…" for a damaging move against an immune type (`CheckWonderGuardAndLevitate`).
    pub(crate) fn missed(&mut self, env: &mut Env, u: &MoveUse) {
        self.attack_string(env, u);
        self.miss_message(env, u);
    }

    /// The `resultmessage` of a miss (after the attack string): "It doesn't affect Z…" when
    /// `CheckWonderGuardAndLevitate` found the type immune (`targetNotAffected`), else "X's
    /// attack missed!".
    pub(crate) fn miss_message(&mut self, env: &mut Env, u: &MoveUse) {
        self.no_effect = true;
        let m = if effects::protected(self, u) {
            // `gMissStringIds[B_MSG_PROTECTED]` (JumpIfMoveAffectedByProtect, before the
            // accuracy roll and `CheckWonderGuardAndLevitate`).
            u.msg_long(Str::PkmnProtectedItself, u.target)
        } else if self.levitates(env, u) {
            self.battler_mut(u.attacker).v.unable = true;
            levitate_msg(u)
        } else if u.power > 0 && u.mov != env.t.struggle && self.immune(env, u) {
            self.battler_mut(u.attacker).v.unable = true;
            u.msg_long(Str::ItDoesntAffect, u.target)
        } else {
            u.msg_long(Str::AttackMissed, u.attacker)
        };
        env.out.push(BattleEvent::Msg(m));
    }

    /// `BattleScript_ButItFailed` (after `attackstring`).
    pub(crate) fn fail(&mut self, env: &mut Env, u: &MoveUse) {
        self.no_effect = true;
        env.out
            .push(BattleEvent::Msg(u.msg_long(Str::ButItFailed, u.attacker)));
    }

    /// Whether the chart gives the move's type no effect on the target (or Levitate).
    pub(crate) fn immune(&self, env: &Env, u: &MoveUse) -> bool {
        if self.levitates(env, u) {
            return true;
        }
        let d = self.battler(u.target);
        let r = calc::type_effect(
            env.t.type_chart,
            env.t.foresight_split,
            d.foresight,
            u.ty,
            d.types,
            u.power,
            1,
        );
        r.eff == Effectiveness::NoEffect
    }

    /// `critcalc` (no roll in a first battle nor against Shell / Battle Armor), `damagecalc`,
    /// `typecalc` (skipped for Struggle) and `adjustnormaldamage` (its damage roll, then the
    /// target's Focus Band roll). `hp_dealt` is 0 until [`Battle::deal`].
    pub(crate) fn damage(&mut self, env: &mut Env, u: &MoveUse) -> HitResult {
        let crit = self.crit_roll(env, u);
        let damage = self.damage_calc(env, u, crit);
        let (damage, eff) = self.type_calc(env, u, damage);
        let mut damage = damage;
        damage = calc::random_damage(damage, env.rng.random());
        let hung;
        (damage, hung) = effects::hold_on(self, env, u, damage, eff);
        HitResult {
            damage,
            hp_dealt: 0,
            crit,
            eff,
            hung,
        }
    }

    /// `critcalc`: no roll in a first battle nor against Shell / Battle Armor.
    pub(crate) fn crit_roll(&mut self, env: &mut Env, u: &MoveUse) -> bool {
        let d = self.battler(u.target);
        !self.is_first_battle()
            && !matches!(d.ability, Ability::ShellArmor | Ability::BattleArmor)
            && {
                let stage = calc::crit_chance_stage(self.battler(u.attacker).focus_energy)
                    + effects::crit_bonus(u.info.effect);
                calc::is_crit(stage, env.rng.random())
            }
    }

    /// `damagecalc`: `CalculateBaseDamage`, the crit and `dmgMultiplier`, Charge.
    pub(crate) fn damage_calc(&mut self, env: &mut Env, u: &MoveUse, crit: bool) -> i32 {
        let (a, d) = (
            self.battler(u.attacker).fighter(),
            self.battler(u.target).fighter(),
        );
        let weather = match self.weather() {
            w if w.rain() => calc::DamageWeather::Rain,
            w if w.sun() => calc::DamageWeather::Sun,
            super::Weather::Sand => calc::DamageWeather::Sand,
            _ => calc::DamageWeather::None,
        };
        let power = self.power_with_abilities(env, u, weather);
        let x = effects::damage_extra(self, u, crit, weather);
        let mut damage = calc::base_damage_x(&a, &d, power, u.ty, crit, self.field(), x)
            * if crit { 2 } else { 1 }
            * u.multiplier as i32;
        if u.quarter {
            damage = (damage / 4).max(1);
        }
        // Cmd_damagecalc: STATUS3_CHARGED_UP doubles the move's damage when its base type
        // (`gBattleMoves[gCurrentMove].type`) is Electric.
        if self.battler(u.attacker).charge > 0 && u.info.ty == Type::Electric {
            damage *= 2;
        }
        damage
    }

    /// `typecalc` (skipped for Struggle): STAB, the type chart, Levitate, Filter. A type
    /// immunity sets `targetNotAffected` (`Volatiles::unable`).
    pub(crate) fn type_calc(
        &mut self,
        env: &mut Env,
        u: &MoveUse,
        damage: i32,
    ) -> (i32, Effectiveness) {
        let mut damage = damage;
        let a = self.battler(u.attacker).fighter();
        let d = self.battler(u.target).fighter();
        let mut eff = Effectiveness::Normal;
        if u.mov != env.t.struggle {
            if a.has_type(u.ty) {
                damage = calc::stab(damage);
            }
            let r = calc::type_effect(
                env.t.type_chart,
                env.t.foresight_split,
                self.battler(u.target).foresight,
                u.ty,
                d.types,
                u.power,
                damage,
            );
            damage = r.damage;
            eff = r.eff;
            if self.levitates(env, u) {
                eff = Effectiveness::NoEffect;
            }
            // Filter (Gen7): super-effective damage x3/4.
            if eff == Effectiveness::Super && d.ability == Ability::Filter {
                damage = damage * 3 / 4;
            }
            if eff == Effectiveness::NoEffect {
                // `gProtectStructs.targetNotAffected` (`WasUnableToUseMove`).
                self.battler_mut(u.attacker).v.unable = true;
            }
        }
        (damage, eff)
    }

    /// The Focus Band of `adjustnormaldamage` / `adjustnormaldamage2` / `adjustsetdamage`:
    /// the roll (drawn whenever `target` holds one), then the HP check. The damage (HP - 1
    /// when the target hangs on) and whether it did.
    pub(crate) fn focus_band(
        &self,
        env: &mut Env,
        target: Side,
        damage: i32,
        eff: Effectiveness,
    ) -> (i32, bool) {
        let d = self.battler(target);
        let banded = match d.x.hold {
            Some(HoldEffect::FocusBand(p)) => env.rng.random() % 100 < p as u16,
            _ => false,
        };
        let hp = d.mon.hp;
        if banded && eff != Effectiveness::NoEffect && hp > 0 && hp as i32 <= damage {
            (hp as i32 - 1, true)
        } else {
            (damage, false)
        }
    }

    /// The move's power with the attacker's power abilities (Silice; Gen7 modifiers as integer
    /// steps): Technician x1.5 at 60 or less, the -ate abilities x1.2 on the Normal moves they
    /// converted, Tough Claws x1.3 on contact, Sand Force x1.3 on Rock / Ground / Steel in a
    /// sandstorm.
    fn power_with_abilities(&self, env: &Env, u: &MoveUse, weather: calc::DamageWeather) -> u32 {
        let mut power = if u.dynamic > 0 {
            u.dynamic as u32
        } else {
            u.power as u32
        };
        match self.battler(u.attacker).ability {
            Ability::Technician if power <= 60 => power = power * 3 / 2,
            a if ate_type(a).is_some() && u.info.ty == Type::Normal && u.mov != env.t.struggle => {
                power = power * 6 / 5
            }
            Ability::ToughClaws if u.info.flags.has(MoveFlags::MAKES_CONTACT) => {
                power = power * 13 / 10
            }
            Ability::SandForce
                if weather == calc::DamageWeather::Sand
                    && matches!(u.ty, Type::Rock | Type::Ground | Type::Steel) =>
            {
                power = power * 13 / 10
            }
            _ => {}
        }
        power
    }

    /// The hit's animations, HP drop and messages (`attackanimation` … `resultmessage`).
    /// Nothing but "It doesn't affect…" when the type has no effect.
    pub(crate) fn deal(&mut self, env: &mut Env, u: &MoveUse, r: HitResult) -> HitResult {
        self.deal_with(env, u, r, true)
    }

    /// [`Battle::deal`]; `anim`: the attack animation plays (scripts that played it already,
    /// Explosion and Brick Break, pass false).
    pub(crate) fn deal_with(
        &mut self,
        env: &mut Env,
        u: &MoveUse,
        mut r: HitResult,
        anim: bool,
    ) -> HitResult {
        if r.eff == Effectiveness::NoEffect {
            let m = if self.levitates(env, u) {
                levitate_msg(u)
            } else {
                u.msg_long(Str::ItDoesntAffect, u.target)
            };
            env.out.push(BattleEvent::Msg(m));
            return r;
        }
        if anim {
            env.out.push(BattleEvent::MoveAnim {
                attacker: u.attacker,
                mov: u.mov,
            });
        }
        env.out.push(BattleEvent::Hit {
            target: u.target,
            eff: r.eff,
        });
        r.hp_dealt = self.change_hp(env, u.target, r.damage);
        if r.crit {
            env.out
                .push(BattleEvent::Msg(u.msg_long(Str::CriticalHit, u.attacker)));
        }
        let result = match r.eff {
            Effectiveness::Super => Some(Str::SuperEffective),
            Effectiveness::NotVery => Some(Str::NotVeryEffective),
            _ => None,
        };
        if let Some(id) = result {
            env.out.push(BattleEvent::Msg(u.msg_long(id, u.target)));
        }
        self.hung_on(env, u, &r);
        r
    }

    /// `resultmessage`'s `MOVE_RESULT_FOE_HUNG_ON`: `BattleScript_HangedOnMsg`.
    pub(crate) fn hung_on(&mut self, env: &mut Env, u: &MoveUse, r: &HitResult) {
        if !r.hung {
            return;
        }
        if effects::endured(self, u.target) {
            // MOVE_RESULT_FOE_ENDURED: STRINGID_PKMNENDUREDHIT.
            env.out
                .push(BattleEvent::Msg(u.msg_long(Str::PkmnEnduredHit, u.target)));
            return;
        }
        let item = self.battler(u.target).x.item;
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [Buff::None, Buff::None, item.map_or(Buff::None, Buff::Item)],
            ..u.msg_long(Str::HungOnWithX, u.target)
        }));
    }

    /// `seteffectwithchance`: a certain effect applies without a roll; otherwise one roll is
    /// always drawn (even for moves without a secondary effect, as Gen3).
    pub(crate) fn secondary(
        &mut self,
        env: &mut Env,
        u: &MoveUse,
        r: &HitResult,
        secondary: Option<Secondary>,
    ) {
        let no_effect = r.eff == Effectiveness::NoEffect;
        match secondary {
            Some(s) if s.certain && !no_effect => (s.apply)(self, env, u, r),
            _ => {
                let roll = env.rng.random();
                if let Some(s) = secondary
                    && !no_effect
                    && roll % 100 < u.info.effect_chance as u16
                {
                    (s.apply)(self, env, u, r);
                }
            }
        }
    }

    /// `healthbarupdate` + `datahpupdate`: positive `damage` lowers HP (to 0), negative heals
    /// (to max). Returns the HP lost (`gHpDealt`). Damage is added to the battler's `gBideDmg`
    /// (`bide_damage`); residual end-of-turn damage uses [`Battle::change_hp_passive`].
    pub(crate) fn change_hp(&mut self, env: &mut Env, side: Side, damage: i32) -> u16 {
        if damage > 0 {
            let b = self.battler_mut(side);
            b.bide_damage = b.bide_damage.saturating_add(damage as u32);
        }
        let dealt = self.change_hp_passive(env, side, damage);
        // `datahpupdate`'s Counter / Mirror Coat records (Wave C).
        effects::record_damage(self, side, dealt);
        dealt
    }

    /// [`Battle::change_hp`] under `HITMARKER_IGNORE_BIDE` (poison, burn, Leech Seed … at the
    /// end of the turn): Bide does not store the damage.
    pub(crate) fn change_hp_passive(&mut self, env: &mut Env, side: Side, damage: i32) -> u16 {
        let b = self.battler_mut(side);
        let from = b.mon.hp;
        let to = if damage >= 0 {
            from.saturating_sub(damage.min(u16::MAX as i32) as u16)
        } else {
            (from as i32 - damage).min(b.mon.max_hp as i32) as u16
        };
        b.mon.hp = to;
        env.out.push(BattleEvent::Hp { side, from, to });
        self.write_back(side);
        from.saturating_sub(to)
    }

    /// `tryfaintmon`: at 0 HP, the faint (once) and `cleareffectsonfaint`. True if it fainted.
    pub(crate) fn try_faint(&mut self, env: &mut Env, side: Side) -> bool {
        let b = self.battler_mut(side);
        if b.mon.hp > 0 || b.fainted {
            return false;
        }
        b.fainted = true;
        // BattleScript_DestinyBondTakesLife runs before BattleScript_FaintTarget (Wave C).
        effects::destiny_bond(self, env, side);
        let b = self.battler_mut(side);
        // FaintClearSetData: stages and volatile statuses.
        b.stages = [0; 8];
        b.confusion = 0;
        b.flinched = false;
        b.focus_energy = false;
        b.seeded = false;
        b.foresight = false;
        b.bide_turns = 0;
        b.charge = 0;
        b.bide_damage = 0;
        b.v = super::Volatiles::default();
        b.fx = super::BattlerFx::default();
        let (party, ability, level) = (b.mon.party, b.ability, b.mon.level);
        if side == Side::Player {
            // Gen3 `Cmd_tryfaintmon` → `AdjustFriendshipOnBattleFaint` (Task E).
            use crate::world::friendship::{FriendshipEvent, on_event};
            let foe_level = self.battler(Side::Opponent).mon.level;
            let event = FriendshipEvent::battle_faint(foe_level, level);
            let hold = self.x.holds[side as usize][party as usize];
            if let Some(m) = self.parties[side as usize].get_mut(party as usize) {
                on_event(m, event, hold, env.t, env.rng);
            }
        }
        self.clear_bonds(side);
        env.out.push(BattleEvent::Faint { side });
        // STRINGID_PKMNFAINTED names the target battler; B_BUFF1 also carries the party slot,
        // since a replacement may be out by the time the message is expanded.
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [Buff::Mon { side, party }, Buff::None, Buff::None],
            ..msg(Str::PkmnFainted, side, side, side)
        }));
        effects::left_field(self, env, ability);
        true
    }

    /// Moves `side`'s stage of `stat` by `delta`, clamped to ±6. False (and no change) when it
    /// is already at the limit in that direction (`B_MSG_STAT_WONT_INCREASE` / `_DECREASE`).
    pub(crate) fn change_stage(&mut self, side: Side, stat: Stat, delta: i8) -> bool {
        let s = &mut self.battler_mut(side).stages[stat as usize];
        if (delta > 0 && *s >= calc::MAX_STAGE) || (delta < 0 && *s <= calc::MIN_STAGE) {
            return false;
        }
        *s = (*s + delta).clamp(calc::MIN_STAGE, calc::MAX_STAGE);
        true
    }

    /// `gStatUpStringIds` / `gStatDownStringIds` for a change on `side`: "Y's STAT rose!"
    /// (sharply / harshly for 2 stages). The defender form when `side` is the move's target.
    pub(crate) fn stat_msg(
        &self,
        u: &MoveUse,
        side: Side,
        stat: Stat,
        stages: u8,
        up: bool,
    ) -> Msg {
        Msg {
            mov: Some(u.mov),
            ..self.stat_msg_sides(u.attacker, u.target, side, stat, stages, up)
        }
    }

    /// [`Battle::stat_msg`] without a move (Intimidate: `attacker` = the Intimidate user,
    /// `target` = the foe whose stat falls).
    pub(crate) fn stat_msg_sides(
        &self,
        attacker: Side,
        target: Side,
        side: Side,
        stat: Stat,
        stages: u8,
        up: bool,
    ) -> Msg {
        let id = match (target == side, up) {
            (true, true) => Str::DefenderStatRose,
            (true, false) => Str::DefenderStatFell,
            (false, true) => Str::AttackerStatRose,
            (false, false) => Str::AttackerStatFell,
        };
        let text = match (up, stages >= 2) {
            (true, false) => Str::StatRose,
            (true, true) => Str::StatSharplyRose,
            (false, false) => Str::StatFell,
            (false, true) => Str::StatHarshlyFell,
        };
        Msg {
            buffs: [Buff::Stat(stat), Buff::Text(text), Buff::None],
            wait: LONG,
            ..msg(id, attacker, target, side)
        }
    }

    /// `BattleScript_EffectStatUp` (`MOVE_EFFECT_AFFECTS_USER`).
    pub(crate) fn stat_up_move(&mut self, env: &mut Env, u: &MoveUse, stat: Stat, stages: u8) {
        self.attack_string(env, u);
        let side = u.attacker;
        if self.change_stage(side, stat, stages as i8) {
            env.out.push(BattleEvent::MoveAnim {
                attacker: u.attacker,
                mov: u.mov,
            });
            env.out.push(BattleEvent::StatAnim { side, up: true });
            env.out
                .push(BattleEvent::Msg(self.stat_msg(u, side, stat, stages, true)));
        } else {
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [Buff::Stat(stat), Buff::None, Buff::None],
                ..u.msg_long(Str::StatWontIncrease, side)
            }));
        }
    }

    /// `BattleScript_EffectStatDown` (no Substitute nor Mist in the engine; Clear Body and
    /// Keen Eye in `effects::abilities::blocks_stat_drop`).
    pub(crate) fn stat_down_move(&mut self, env: &mut Env, u: &MoveUse, stat: Stat, stages: u8) {
        if !self.accuracy(env, u) {
            self.missed(env, u);
            return;
        }
        self.attack_string(env, u);
        let side = u.target;
        if effects::blocks_stat_drop(self, env, u, side, stat) {
            return;
        }
        if self.change_stage(side, stat, -(stages as i8)) {
            env.out.push(BattleEvent::MoveAnim {
                attacker: u.attacker,
                mov: u.mov,
            });
            env.out.push(BattleEvent::StatAnim { side, up: false });
            env.out.push(BattleEvent::Msg(
                self.stat_msg(u, side, stat, stages, false),
            ));
        } else {
            env.out.push(BattleEvent::Msg(Msg {
                buffs: [Buff::Stat(stat), Buff::None, Buff::None],
                ..u.msg_long(Str::StatWontDecrease, side)
            }));
        }
    }
}

/// The type an -ate ability gives Normal moves.
/// Whether Parental Bond doubles a move of `effect` (Gen7 exclusions the engine can name):
/// not the multi-strike moves, Uproar, Explosion / Self-Destruct, the two-turn and
/// semi-invulnerable moves, Twineedle, Triple Kick, Rollout / Ice Ball, Endeavor, nor Beat Up
/// (`EFFECT_BEAT_UP` = 154, unsupported).
pub(crate) fn parental_bond_applies(effect: MoveEffect) -> bool {
    !matches!(
        effect,
        MoveEffect::MultiHit
            | MoveEffect::DoubleHit
            | MoveEffect::Uproar
            | MoveEffect::Explosion
            | MoveEffect::TwoTurn(_)
            | MoveEffect::SemiInvulnerable(_)
            | MoveEffect::Twineedle
            | MoveEffect::TripleKick
            | MoveEffect::Rollout
            | MoveEffect::Endeavor
            | MoveEffect::Unsupported(154)
    )
}

pub(crate) fn ate_type(a: Ability) -> Option<Type> {
    match a {
        Ability::Aerilate => Some(Type::Flying),
        Ability::Pixilate => Some(Type::Fairy),
        Ability::Refrigerate => Some(Type::Ice),
        _ => None,
    }
}

/// `B_MSG_GROUND_MISS`: "Z makes GROUND moves miss with Levitate!".
fn levitate_msg(u: &MoveUse) -> Msg {
    Msg {
        buffs: [
            Buff::None,
            Buff::None,
            Buff::Text(Str::AbilityName(Ability::Levitate)),
        ],
        ..u.msg_long(Str::MakesGroundMiss, u.target)
    }
}

/// `MOVE_EFFECT_RECOIL_25` → `BattleScript_MoveEffectRecoil`: a quarter of the HP dealt
/// (minimum 1) to the attacker, "X is hit with recoil!", then its faint check. Rock Head
/// prevents it, except for Struggle.
fn recoil(b: &mut Battle, env: &mut Env, u: &MoveUse, r: &HitResult) {
    if u.mov != env.t.struggle && b.battler(u.attacker).ability == crate::mon::Ability::RockHead {
        return;
    }
    let damage = (r.hp_dealt / 4).max(1);
    b.change_hp(env, u.attacker, damage as i32);
    env.out.push(BattleEvent::Msg(
        u.msg_long(Str::PkmnHitWithRecoil, u.attacker),
    ));
    b.try_faint(env, u.attacker);
}
