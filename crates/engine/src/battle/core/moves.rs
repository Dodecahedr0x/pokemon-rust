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
use crate::mon::{MoveEffect, MoveInfo, MoveTarget, Stat, Type};

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
        MoveUse {
            attacker,
            target,
            mov,
            slot,
            info,
            power: info.power,
            ty: info.ty,
            multiplier: 1,
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
        let r = self.deal(env, u, r);
        self.secondary(env, u, &r, secondary);
        self.try_faint(env, u.target);
        Some(r)
    }

    /// `attackstring` + `ppreduce`: "X used Y!" and one PP (none for Struggle). Call once per
    /// move.
    pub(crate) fn attack_string(&mut self, env: &mut Env, u: &MoveUse) {
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
        let (a, d) = (self.battler(u.attacker), self.battler(u.target));
        let eva = if d.foresight {
            0
        } else {
            d.stages[Stat::Evasion as usize]
        };
        let acc = a.stages[Stat::Accuracy as usize];
        calc::accuracy_hits_by(
            u.info.accuracy,
            acc,
            eva,
            a.ability,
            u.ty.is_physical(),
            env.rng.random(),
        )
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
        let m = if u.power > 0 && u.mov != env.t.struggle && self.immune(env, u) {
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

    /// Whether the chart gives the move's type no effect on the target.
    pub(crate) fn immune(&self, env: &Env, u: &MoveUse) -> bool {
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

    /// `critcalc` (no roll in a first battle), `damagecalc`, `typecalc` (skipped for Struggle)
    /// and `adjustnormaldamage` (one roll). `hp_dealt` is 0 until [`Battle::deal`].
    pub(crate) fn damage(&mut self, env: &mut Env, u: &MoveUse) -> HitResult {
        let (a, d) = (
            self.battler(u.attacker).fighter(),
            self.battler(u.target).fighter(),
        );
        let crit = !self.is_first_battle() && {
            let stage = calc::crit_chance_stage(self.battler(u.attacker).focus_energy);
            calc::is_crit(stage, env.rng.random())
        };
        let mut damage = calc::base_damage_in(&a, &d, u.power, u.ty, crit, self.field())
            * if crit { 2 } else { 1 }
            * u.multiplier as i32;
        // Cmd_damagecalc: STATUS3_CHARGED_UP doubles the move's damage when its base type
        // (`gBattleMoves[gCurrentMove].type`) is Electric.
        if self.battler(u.attacker).charge > 0 && u.info.ty == Type::Electric {
            damage *= 2;
        }
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
            if eff == Effectiveness::NoEffect {
                // `gProtectStructs.targetNotAffected` (`WasUnableToUseMove`).
                self.battler_mut(u.attacker).v.unable = true;
            }
        }
        damage = calc::random_damage(damage, env.rng.random());
        HitResult {
            damage,
            hp_dealt: 0,
            crit,
            eff,
        }
    }

    /// The hit's animations, HP drop and messages (`attackanimation` … `resultmessage`).
    /// Nothing but "It doesn't affect…" when the type has no effect.
    pub(crate) fn deal(&mut self, env: &mut Env, u: &MoveUse, mut r: HitResult) -> HitResult {
        if r.eff == Effectiveness::NoEffect {
            env.out
                .push(BattleEvent::Msg(u.msg_long(Str::ItDoesntAffect, u.target)));
            return r;
        }
        env.out.push(BattleEvent::MoveAnim {
            attacker: u.attacker,
            mov: u.mov,
        });
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
        r
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
        self.change_hp_passive(env, side, damage)
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
        let party = b.mon.party;
        self.clear_bonds(side);
        env.out.push(BattleEvent::Faint { side });
        // STRINGID_PKMNFAINTED names the target battler; B_BUFF1 also carries the party slot,
        // since a replacement may be out by the time the message is expanded.
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [Buff::Mon { side, party }, Buff::None, Buff::None],
            ..msg(Str::PkmnFainted, side, side, side)
        }));
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
