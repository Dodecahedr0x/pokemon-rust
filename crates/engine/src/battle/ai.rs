//! Opponent move choice (`OpponentHandleChooseMove`, `battle_ai_script_commands.c`,
//! `data/battle_ai_scripts.s`).
//!
//! - Wild battles: `Random() & 3` until a slot with a move and PP.
//! - Trainer battles and the first battle go through the score model
//!   (`BattleAI_SetupAIData` + `BattleAI_ChooseMoveOrAction`): every slot starts at 100 (0 for
//!   an empty slot or one without PP), four `simulatedRNG` rolls (`100 - Random() % 16`) are
//!   drawn, then each AI script of the flags runs over the slots, bit 0 first. The first battle
//!   uses `AI_SCRIPT_FIRST_BATTLE` alone; trainers use `Battle::trainer_ai`.
//! - Implemented scripts (every `Random()` of the implemented branches is drawn in order):
//!   - `AI_CheckBadMove` (bit 0): the type immunity of damaging moves and the branches of the
//!     engine's effects (sleep, stat ups/downs at the cap, Keen Eye, Bide / Sonic Boom, poison
//!     and toxic, paralysis, Focus Energy, confusion (-5), Leech Seed, Defense Curl);
//!   - `AI_TryToFaint` (bit 1): exact;
//!   - `AI_CheckViability` (bit 2): Sleep, Absorb, the stat up/down scripts, Swift's
//!     `AI_CV_AlwaysHit`, Bide, Toxic / Leech Seed, Confuse, Poison, Paralyze, Vital Throw;
//!   - `AI_FirstBattle` (bit 31): flee at ≤ 20% target HP → `Choice::Run`.
//! - Not modelled: the other flag bits (`AI_SCRIPT_SETUP_FIRST_TURN`, `RISKY`, … — their
//!   scripts' rolls are not drawn), abilities outside `mon::Ability` (Soundproof, Volt Absorb,
//!   Levitate, Wonder Guard, Clear Body, Hyper Cutter, Insomnia, Own Tempo, Limber …), the
//!   target's revealed moves (`BATTLE_HISTORY->usedMoves`: Dream Eater / Nightmare checks see
//!   none), `AI_CV_SpeedDownFromChance` (it names moves), `AI_TrySwitchOrUseItem`, and the four
//!   rolls of `BattleAI_HandleItemUseBeforeAISetup` at battle setup.

use super::calc;
use super::core::Battle;
use super::{Choice, Effectiveness, Side};
use crate::data::MoveId;
use crate::mon::{Ability, BattleTables, MoveEffect, MoveInfo, Stat, Status, StatusKind, Type};
use crate::rng::Rng;

/// `AI_SCRIPT_*` bits.
pub const AI_SCRIPT_CHECK_BAD_MOVE: u32 = 1 << 0;
pub const AI_SCRIPT_TRY_TO_FAINT: u32 = 1 << 1;
pub const AI_SCRIPT_CHECK_VIABILITY: u32 = 1 << 2;
pub const AI_SCRIPT_FIRST_BATTLE: u32 = 1 << 31;

/// `AI_EFFECTIVENESS_*` (`Cmd_if_type_effectiveness`).
const X0: i32 = 0;
const X0_25: i32 = 10;
const X0_5: i32 = 20;
const X4: i32 = 160;

/// `sIgnoredPowerfulMoveEffects`, as raw `EFFECT_*` (none is implemented by the engine).
const IGNORED_POWERFUL: [u8; 12] = [7, 8, 39, 75, 80, 145, 151, 161, 170, 182, 190, 204];
/// `EFFECT_EXPLOSION`, `EFFECT_ALWAYS_HIT`, `EFFECT_PROTECT`, `EFFECT_SPECIAL_DEFENSE_UP`.
const EFFECT_EXPLOSION: u8 = 7;
const EFFECT_ALWAYS_HIT: u8 = 17;
const EFFECT_PROTECT: u8 = 111;

/// The opponent's action for this turn, chosen when the turn starts (Gen3 asks the opponent's
/// controller during action selection, before the player's choice resolves). The core only
/// asks when a move is usable; it maps `Choice::Run` to fleeing (`B_ACTION_RUN`) and
/// `Choice::Switch` to a switch.
pub fn choose(battle: &Battle, t: &BattleTables, rng: &mut Rng) -> Choice {
    let flags = if battle.is_first_battle() {
        AI_SCRIPT_FIRST_BATTLE
    } else if battle.is_trainer() {
        battle.trainer_ai
    } else {
        return wild(battle, rng);
    };
    let mut ai = Ai::setup(battle, t, rng);
    for bit in 0..32 {
        let flag = 1u32 << bit;
        if flags & flag == 0 {
            continue;
        }
        for slot in 0..4 {
            let Some(mov) = ai.considered(slot) else {
                ai.score[slot] = 0;
                continue;
            };
            match flag {
                AI_SCRIPT_CHECK_BAD_MOVE => ai.check_bad_move(slot, mov),
                AI_SCRIPT_TRY_TO_FAINT => ai.try_to_faint(slot, mov),
                AI_SCRIPT_CHECK_VIABILITY => ai.check_viability(slot, mov),
                AI_SCRIPT_FIRST_BATTLE => {
                    // AI_FirstBattle: `flee` sets AI_ACTION_DO_NOT_ATTACK, ending the loop.
                    let hp = ai.hp(Side::Player);
                    if hp <= 20 {
                        return Choice::Run;
                    }
                }
                _ => {}
            }
        }
    }
    ai.pick()
}

/// `OpponentHandleChooseMove` outside the AI branch; a slot without PP is refused by
/// `TrySetCantSelectMoveBattleScript` and chosen again.
fn wild(battle: &Battle, rng: &mut Rng) -> Choice {
    let m = battle.mon(Side::Opponent);
    if (0..4).all(|i| m.moves[i].is_none() || m.pp[i] == 0) {
        return Choice::Move(0);
    }
    loop {
        let slot = (rng.random() & 3) as usize;
        if m.moves[slot].is_some() && m.pp[slot] > 0 {
            return Choice::Move(slot as u8);
        }
    }
}

/// `AI_THINKING_STRUCT` for the opponent (`sBattler_AI`) against the player (`gBattlerTarget`).
struct Ai<'a> {
    b: &'a Battle,
    t: &'a BattleTables,
    rng: &'a mut Rng,
    /// `s8 score[4]`, kept ≥ 0 by `Cmd_score`.
    score: [i16; 4],
    /// `simulatedRNG`.
    sim: [i32; 4],
}

#[cfg(test)]
mod tests;

const USER: Side = Side::Opponent;
const TARGET: Side = Side::Player;

impl<'a> Ai<'a> {
    /// `BattleAI_SetupAIData(ALL_MOVES_MASK)`.
    fn setup(b: &'a Battle, t: &'a BattleTables, rng: &'a mut Rng) -> Self {
        let m = b.battler(USER);
        let mut score = [0; 4];
        let mut sim = [0; 4];
        for i in 0..4 {
            // CheckMoveLimitations: no move or no PP.
            score[i] = if m.mon.moves[i].is_some() && m.mon.pp[i] > 0 {
                100
            } else {
                0
            };
            sim[i] = 100 - (rng.random() % 16) as i32;
        }
        Ai {
            b,
            t,
            rng,
            score,
            sim,
        }
    }

    /// `moveConsidered`: `None` without PP.
    fn considered(&self, slot: usize) -> Option<MoveId> {
        let m = &self.b.battler(USER).mon;
        if m.pp[slot] == 0 { None } else { m.moves[slot] }
    }

    /// `ChooseMoveOrAction_Singles`: slot 0, then the slots with a move; ties broken by
    /// `Random() % n`.
    fn pick(&mut self) -> Choice {
        let m = &self.b.battler(USER).mon;
        let mut best = [0u8; 4];
        let mut n = 1;
        let mut top = self.score[0];
        for i in 1..4 {
            if m.moves[i].is_none() {
                continue;
            }
            if top == self.score[i] {
                best[n] = i as u8;
                n += 1;
            }
            if top < self.score[i] {
                n = 1;
                top = self.score[i];
                best[0] = i as u8;
            }
        }
        Choice::Move(best[(self.rng.random() % n as u16) as usize])
    }

    /// `Cmd_score`.
    fn add(&mut self, slot: usize, d: i16) {
        self.score[slot] = (self.score[slot] + d).clamp(0, 127);
    }

    /// `Random() % 256 < n` (`if_random_less_than`).
    fn random_lt(&mut self, n: u16) -> bool {
        self.rng.random() % 256 < n
    }

    fn info(&self, mov: MoveId) -> &'static MoveInfo {
        self.t.move_info(mov)
    }

    /// `100 * hp / maxHP` (`if_hp_*`).
    fn hp(&self, side: Side) -> u32 {
        let m = &self.b.battler(side).mon;
        100 * m.hp as u32 / m.max_hp.max(1) as u32
    }

    /// `statStages[stat]` in Gen3's 0..=12 (6 = default).
    fn stage(&self, side: Side, stat: Stat) -> i8 {
        self.b.battler(side).stages[stat as usize] + 6
    }

    fn has_type(&self, side: Side, ty: Type) -> bool {
        self.b.battler(side).types.contains(&ty)
    }

    fn status(&self, side: Side) -> Status {
        self.b.battler(side).mon.status
    }

    /// `if_target_faster` = `GetWhoStrikesFirst(AI, target, TRUE) == 1`: a speed tie draws
    /// `Random() & 1` and is never "faster".
    fn target_faster(&mut self) -> bool {
        let speed = |s: Side| calc::effective_speed(&self.b.battler(s).fighter());
        let (u, t) = (speed(USER), speed(TARGET));
        if u == t {
            self.rng.random();
            return false;
        }
        u < t
    }

    /// `Cmd_if_type_effectiveness` (`TypeCalc` on `AI_EFFECTIVENESS_x1` = 40): STAB first,
    /// then the chart; Struggle skips it. 120 / 240 / 30 / 15 map to x2 / x4 / x0.5 / x0.25,
    /// so a neutral STAB move stays at 60, as Gen3.
    fn effectiveness(&self, mov: MoveId) -> i32 {
        if mov == self.t.struggle {
            return 40;
        }
        let info = self.info(mov);
        let mut d = 40;
        if self.has_type(USER, info.ty) {
            d = calc::stab(d);
        }
        let target = self.b.battler(TARGET);
        let r = calc::type_effect(
            self.t.type_chart,
            self.t.foresight_split,
            target.foresight,
            info.ty,
            target.types,
            info.power,
            d,
        );
        if r.eff == Effectiveness::NoEffect {
            return X0;
        }
        match r.damage {
            120 => 80,
            240 => X4,
            30 => X0_5,
            15 => X0_25,
            d => d,
        }
    }

    /// `AI_CalcDmg` + `TypeCalc`, × `simulatedRNG[slot]` / 100, at least 1.
    fn damage(&self, slot: usize, mov: MoveId) -> i32 {
        let info = self.info(mov);
        let (user, target) = (self.b.battler(USER), self.b.battler(TARGET));
        let (a, d) = (user.fighter(), target.fighter());
        let mut dmg = calc::base_damage(&a, &d, info.power, info.ty, false);
        if user.charge > 0 && info.ty == Type::Electric {
            dmg *= 2;
        }
        if mov != self.t.struggle {
            if a.has_type(info.ty) {
                dmg = calc::stab(dmg);
            }
            dmg = calc::type_effect(
                self.t.type_chart,
                self.t.foresight_split,
                target.foresight,
                info.ty,
                d.types,
                info.power,
                dmg,
            )
            .damage;
        }
        (dmg * self.sim[slot] / 100).max(1)
    }

    fn ignored_powerful(info: &MoveInfo) -> bool {
        matches!(info.effect, MoveEffect::Unsupported(e) if IGNORED_POWERFUL.contains(&e))
    }

    /// `Cmd_get_how_powerful_move_is`: `None` = `MOVE_POWER_OTHER`, else whether `slot` is
    /// the most powerful (no other move strictly stronger).
    fn most_powerful(&self, slot: usize, mov: MoveId) -> Option<bool> {
        let info = self.info(mov);
        if info.power <= 1 || Self::ignored_powerful(info) {
            return None;
        }
        let moves = self.b.battler(USER).mon.moves;
        let dmgs: [i32; 4] = core::array::from_fn(|i| match moves[i] {
            Some(m) if !Self::ignored_powerful(self.info(m)) && self.info(m).power > 1 => {
                self.damage(i, m)
            }
            _ => 0,
        });
        Some(dmgs.iter().all(|&d| d <= dmgs[slot]))
    }

    /// `AI_CheckBadMove` (the engine's subset).
    fn check_bad_move(&mut self, slot: usize, mov: MoveId) {
        if self.most_powerful(slot, mov).is_some() && self.effectiveness(mov) == X0 {
            self.add(slot, -10);
            return;
        }
        let info = self.info(mov);
        let target = self.b.battler(TARGET);
        let statused = target.mon.status != Status::None;
        let bad = match info.effect {
            MoveEffect::Status(StatusKind::Sleep) => statused,
            MoveEffect::StatUp { stat, .. } => self.stage(USER, stat) == 12,
            MoveEffect::DefenseCurl => self.stage(USER, Stat::Def) == 12,
            MoveEffect::StatDown { stat, .. } => {
                self.stage(TARGET, stat) == 0
                    || (stat == Stat::Accuracy && target.ability == Ability::KeenEye)
            }
            // AI_CBM_HighRiskForDamage.
            MoveEffect::Bide | MoveEffect::SonicBoom => self.effectiveness(mov) == X0,
            // AI_CBM_Toxic (also EFFECT_POISON).
            MoveEffect::Status(StatusKind::Poison | StatusKind::Toxic) => {
                self.has_type(TARGET, Type::Steel)
                    || self.has_type(TARGET, Type::Poison)
                    || statused
            }
            MoveEffect::Status(StatusKind::Paralysis) => self.effectiveness(mov) == X0 || statused,
            MoveEffect::FocusEnergy => self.b.battler(USER).focus_energy,
            MoveEffect::Confuse => {
                if target.confusion > 0 {
                    self.add(slot, -5);
                }
                false
            }
            MoveEffect::LeechSeed => target.seeded || self.has_type(TARGET, Type::Grass),
            _ => false,
        };
        if bad {
            self.add(slot, -10);
        }
    }

    /// `AI_TryToFaint`.
    fn try_to_faint(&mut self, slot: usize, mov: MoveId) {
        let info = self.info(mov);
        // if_can_faint.
        if info.power >= 2 && self.b.battler(TARGET).mon.hp as i32 <= self.damage(slot, mov) {
            match info.effect {
                MoveEffect::Unsupported(EFFECT_EXPLOSION) => {}
                MoveEffect::QuickAttack => self.add(slot, 6),
                _ => self.add(slot, 4),
            }
            return;
        }
        if self.most_powerful(slot, mov) == Some(false) {
            self.add(slot, -1);
            return;
        }
        if self.effectiveness(mov) == X4 && !self.random_lt(80) {
            self.add(slot, 2);
        }
    }

    /// `AI_CheckViability` (the engine's subset).
    fn check_viability(&mut self, slot: usize, mov: MoveId) {
        let info = self.info(mov);
        match info.effect {
            // AI_CV_Sleep: the target's revealed moves are not tracked (no Dream Eater).
            MoveEffect::Absorb => self.cv_absorb(slot, mov),
            MoveEffect::StatUp { stat, .. } => match stat {
                Stat::Atk => self.cv_stat_up(slot, Stat::Atk, 70, 40),
                Stat::Def => self.cv_def_up(slot, Stat::Def),
                Stat::Speed => self.cv_speed_up(slot),
                Stat::SpAtk => self.cv_stat_up(slot, Stat::SpAtk, 70, 70),
                Stat::SpDef => self.cv_def_up(slot, Stat::SpDef),
                Stat::Accuracy => self.cv_accuracy_up(slot),
                Stat::Evasion => self.cv_evasion_up(slot),
                Stat::Hp => {}
            },
            MoveEffect::Unsupported(EFFECT_ALWAYS_HIT) => self.cv_always_hit(slot),
            MoveEffect::StatDown { stat, .. } => match stat {
                Stat::Atk => self.cv_attack_down(slot, Stat::Atk, &PHYSICAL_TYPE_LIST),
                Stat::SpAtk => self.cv_attack_down(slot, Stat::SpAtk, &SPECIAL_TYPE_LIST),
                Stat::Def | Stat::SpDef | Stat::Evasion => self.cv_defense_down(slot, stat),
                Stat::Speed => self.cv_speed_down(slot),
                Stat::Accuracy => self.cv_accuracy_down(slot),
                Stat::Hp => {}
            },
            // AI_CV_Bide.
            MoveEffect::Bide if self.hp(USER) <= 90 => self.add(slot, -2),
            MoveEffect::Status(StatusKind::Toxic) | MoveEffect::LeechSeed => self.cv_toxic(slot),
            MoveEffect::Confuse => self.cv_confuse(slot),
            // AI_CV_Poison.
            MoveEffect::Status(StatusKind::Poison)
                if self.hp(USER) < 50 || self.hp(TARGET) <= 50 =>
            {
                self.add(slot, -1)
            }
            MoveEffect::Status(StatusKind::Paralysis) => {
                if self.target_faster() {
                    if !self.random_lt(20) {
                        self.add(slot, 3);
                    }
                } else if self.hp(USER) <= 70 {
                    self.add(slot, -1);
                }
            }
            MoveEffect::VitalThrow => self.cv_vital_throw(slot),
            _ => {}
        }
    }

    fn cv_absorb(&mut self, slot: usize, mov: MoveId) {
        let e = self.effectiveness(mov);
        if (e == X0_5 || e == X0_25) && !self.random_lt(50) {
            self.add(slot, -3);
        }
    }

    /// AI_CV_AttackUp / AI_CV_SpAtkUp (`keep` = the roll of the 40–70% HP branch).
    fn cv_stat_up(&mut self, slot: usize, stat: Stat, high: u32, keep: u16) {
        if self.stage(USER, stat) < 9 {
            if self.hp(USER) == 100 && !self.random_lt(128) {
                self.add(slot, 2);
            }
        } else if !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.hp(USER) > high {
            return;
        }
        if self.hp(USER) < 40 || !self.random_lt(keep) {
            self.add(slot, -2);
        }
    }

    /// AI_CV_DefenseUp / AI_CV_SpDefUp: the target's last move (`gLastMoves`) decides.
    fn cv_def_up(&mut self, slot: usize, stat: Stat) {
        if self.stage(USER, stat) < 9 {
            if self.hp(USER) == 100 && !self.random_lt(128) {
                self.add(slot, 2);
            }
        } else if !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.hp(USER) >= 70 && self.random_lt(200) {
            return;
        }
        if self.hp(USER) < 40 {
            self.add(slot, -2);
            return;
        }
        let last = self.b.battler(TARGET).last_move.map(|m| self.info(m));
        let power = last.map_or(0, |i| i.power);
        if power != 0 {
            let ty = last.map_or(Type::Normal, |i| i.ty);
            let physical = DEF_UP_PHYSICAL_TYPES.contains(&ty);
            // DefenseUp discourages against a special last move, SpDefUp against a physical one.
            let discourage = if stat == Stat::Def {
                !physical
            } else {
                physical
            };
            if discourage || !self.random_lt(60) {
                // The first `if_random_less_than 60` falls through to the second.
                if discourage || !self.random_lt(60) {
                    self.add(slot, -2);
                }
            }
            return;
        }
        if !self.random_lt(60) {
            self.add(slot, -2);
        }
    }

    fn cv_speed_up(&mut self, slot: usize) {
        if self.target_faster() {
            if !self.random_lt(70) {
                self.add(slot, 3);
            }
        } else {
            self.add(slot, -3);
        }
    }

    fn cv_accuracy_up(&mut self, slot: usize) {
        if self.stage(USER, Stat::Accuracy) >= 9 && !self.random_lt(50) {
            self.add(slot, -2);
        }
        if self.hp(USER) <= 70 {
            self.add(slot, -2);
        }
    }

    fn cv_evasion_up(&mut self, slot: usize) {
        if self.hp(USER) >= 90 && !self.random_lt(100) {
            self.add(slot, 3);
        }
        if self.stage(USER, Stat::Evasion) >= 9 && !self.random_lt(128) {
            self.add(slot, -1);
        }
        if matches!(self.status(TARGET), Status::Toxic(_)) {
            let go = self.hp(USER) > 50 || !self.random_lt(80);
            if go && !self.random_lt(50) {
                self.add(slot, 3);
            }
        }
        if self.b.battler(TARGET).seeded && !self.random_lt(70) {
            self.add(slot, 3);
        }
        // STATUS3_ROOTED and STATUS2_CURSED are not modelled (never set).
        self.cv_low_hp_tail(slot, USER, Stat::Evasion);
    }

    /// The common tail of AI_CV_EvasionUp / AI_CV_AccuracyDown: at ≤ 70% HP with `stat` of
    /// `side` changed, -2 unless a 70/256 roll keeps it (always -2 under 40% HP either side).
    fn cv_low_hp_tail(&mut self, slot: usize, side: Side, stat: Stat) {
        if self.hp(USER) > 70 || self.stage(side, stat) == 6 {
            return;
        }
        if self.hp(USER) < 40 || self.hp(TARGET) < 40 || !self.random_lt(70) {
            self.add(slot, -2);
        }
    }

    fn cv_always_hit(&mut self, slot: usize) {
        let (eva, acc) = (
            self.stage(TARGET, Stat::Evasion),
            self.stage(USER, Stat::Accuracy),
        );
        if eva > 10 || acc < 2 {
            self.add(slot, 1);
        } else if !(eva > 8 || acc < 4) {
            return;
        }
        if !self.random_lt(100) {
            self.add(slot, 1);
        }
    }

    /// AI_CV_AttackDown (Atk, `AI_CV_AttackDown_PhysicalTypeList`) / AI_CV_SpAtkDown (whose
    /// first check reads the target's **Attack**, as Gen3).
    fn cv_attack_down(&mut self, slot: usize, stat: Stat, types: &[Type]) {
        if self.stage(TARGET, Stat::Atk) != 6 {
            self.add(slot, -1);
            if self.hp(USER) <= 90 {
                self.add(slot, -1);
            }
            if self.stage(TARGET, stat) <= 3 && !self.random_lt(50) {
                self.add(slot, -2);
            }
        }
        if self.hp(TARGET) <= 70 {
            self.add(slot, -2);
        }
        let [t1, t2] = self.b.battler(TARGET).types;
        if types.contains(&t1) || types.contains(&t2) {
            return;
        }
        if !self.random_lt(50) {
            self.add(slot, -2);
        }
    }

    /// AI_CV_DefenseDown / SpDefDown / EvasionDown.
    fn cv_defense_down(&mut self, slot: usize, stat: Stat) {
        if (self.hp(USER) < 70 || self.stage(TARGET, stat) <= 3) && !self.random_lt(50) {
            self.add(slot, -2);
        }
        if self.hp(TARGET) <= 70 {
            self.add(slot, -2);
        }
    }

    fn cv_speed_down(&mut self, slot: usize) {
        if self.target_faster() {
            if !self.random_lt(70) {
                self.add(slot, 2);
            }
        } else {
            self.add(slot, -3);
        }
    }

    fn cv_accuracy_down(&mut self, slot: usize) {
        if (self.hp(USER) < 70 || self.hp(TARGET) <= 70) && !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.stage(USER, Stat::Accuracy) <= 4 && !self.random_lt(80) {
            self.add(slot, -2);
        }
        if matches!(self.status(TARGET), Status::Toxic(_)) && !self.random_lt(70) {
            self.add(slot, 2);
        }
        if self.b.battler(TARGET).seeded && !self.random_lt(70) {
            self.add(slot, 2);
        }
        // STATUS3_ROOTED and STATUS2_CURSED are not modelled; the tail reads the target's
        // accuracy, as Gen3.
        self.cv_low_hp_tail(slot, TARGET, Stat::Accuracy);
    }

    /// AI_CV_Toxic (also Leech Seed).
    fn cv_toxic(&mut self, slot: usize) {
        let moves = self.b.battler(USER).mon.moves;
        let attacking = moves.iter().flatten().any(|&m| self.info(m).power != 0);
        if attacking {
            if self.hp(USER) <= 50 && !self.random_lt(50) {
                self.add(slot, -3);
            }
            if self.hp(TARGET) <= 50 && !self.random_lt(50) {
                self.add(slot, -3);
            }
        }
        let encouraged = moves.iter().flatten().any(|&m| {
            matches!(
                self.info(m).effect,
                MoveEffect::StatUp {
                    stat: Stat::SpDef,
                    stages: 1
                } | MoveEffect::Unsupported(EFFECT_PROTECT)
            )
        });
        if encouraged && !self.random_lt(60) {
            self.add(slot, 2);
        }
    }

    fn cv_confuse(&mut self, slot: usize) {
        if self.hp(TARGET) > 70 {
            return;
        }
        if !self.random_lt(128) {
            self.add(slot, -1);
        }
        if self.hp(TARGET) > 50 {
            return;
        }
        self.add(slot, -1);
        if self.hp(TARGET) <= 30 {
            self.add(slot, -1);
        }
    }

    fn cv_vital_throw(&mut self, slot: usize) {
        if self.target_faster() || self.hp(USER) > 60 {
            return;
        }
        if self.hp(USER) >= 40 && self.random_lt(180) {
            return;
        }
        if !self.random_lt(50) {
            self.add(slot, -1);
        }
    }
}

/// `AI_CV_DefenseUp_PhysicalTypes` / `AI_CV_SpDefUp_PhysicalTypes`.
const DEF_UP_PHYSICAL_TYPES: [Type; 9] = [
    Type::Normal,
    Type::Fighting,
    Type::Poison,
    Type::Ground,
    Type::Flying,
    Type::Rock,
    Type::Bug,
    Type::Ghost,
    Type::Steel,
];

/// `AI_CV_AttackDown_PhysicalTypeList` (no Flying, Poison nor Ghost, as Gen3).
const PHYSICAL_TYPE_LIST: [Type; 6] = [
    Type::Normal,
    Type::Fighting,
    Type::Ground,
    Type::Rock,
    Type::Bug,
    Type::Steel,
];

/// `AI_CV_SpAtkDown_SpecialTypeList`.
const SPECIAL_TYPE_LIST: [Type; 8] = [
    Type::Fire,
    Type::Water,
    Type::Grass,
    Type::Electric,
    Type::Psychic,
    Type::Ice,
    Type::Dragon,
    Type::Dark,
];
