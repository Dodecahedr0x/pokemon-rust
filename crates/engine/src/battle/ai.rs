//! Opponent move choice (`OpponentHandleChooseMove`, `battle_ai_script_commands.c`,
//! `data/battle_ai_scripts.s`).
//!
//! - Wild battles: `Random() & 3` until a slot with a move and PP.
//! - Trainer battles and the first battle go through the score model
//!   (`BattleAI_SetupAIData` + `BattleAI_ChooseMoveOrAction`): every slot starts at 100 (0 for
//!   an empty slot or one without PP), four `simulatedRNG` rolls (`100 - Random() % 16`) are
//!   drawn, then each AI script of the flags runs over the slots, bit 0 first. The first battle
//!   uses `AI_SCRIPT_FIRST_BATTLE` alone; trainers use `Battle::trainer_ai`.
//! - Trainer battles first ask [`switch`] (`AI_TrySwitchOrUseItem`, before the move scores).
//! - Implemented scripts (every `Random()` of the implemented branches is drawn in order):
//!   - `AI_CheckBadMove` (bit 0): the type immunity of damaging moves, Soundproof, and the
//!     branches of the engine's effects (sleep with Insomnia / Vital Spirit, stat ups/downs at
//!     the cap, Keen Eye, Bide / Sonic Boom, poison and toxic, paralysis, Focus Energy,
//!     confusion (-5), Leech Seed, Defense Curl, Mean Look, Foresight, Attract, Teleport, the
//!     sports);
//!   - `AI_TryToFaint` (bit 1): exact;
//!   - `AI_CheckViability` (bit 2): Sleep, Absorb, the stat up/down scripts, Swift's
//!     `AI_CV_AlwaysHit`, Bide, Toxic / Leech Seed, Confuse, Poison, Paralyze, Vital Throw,
//!     `AI_CV_SpeedDownFromChance`, `AI_CV_Trap`, `AI_CV_Foresight` (as built, without
//!     `BUGFIX`), the sports;
//!   - `AI_SetupFirstTurn` (bit 3): exact;
//!   - `AI_FirstBattle` (bit 31): flee at ≤ 20% target HP → `Choice::Run`.
//! - Not modelled: the other flag bits (`RISKY`, `PREFER_POWER_EXTREMES`, … — their scripts'
//!   rolls are not drawn; no slice trainer uses them), abilities outside `mon::Ability`
//!   (Volt Absorb, Levitate, Wonder Guard, Hyper Cutter, Own Tempo, Limber …), the target's revealed moves (`BATTLE_HISTORY->usedMoves`: Dream Eater /
//!   Nightmare checks see none) and `AI_CV_Pursuit` (it reads `isFirstTurn`, which the engine
//!   does not keep; its roll is not drawn). The four rolls of
//!   `BattleAI_HandleItemUseBeforeAISetup` at battle setup are drawn by `Battle::new`.

use super::calc;
use super::core::Battle;
use super::effects::opposite_genders;
use super::{Choice, Effectiveness, Side};
use crate::data::MoveId;
use crate::mon::{
    Ability, BattleTables, MoveEffect, MoveFlags, MoveInfo, Stat, Status, StatusKind, Type,
};
use crate::rng::Rng;

/// `AI_SCRIPT_*` bits.
pub const AI_SCRIPT_CHECK_BAD_MOVE: u32 = 1 << 0;
pub const AI_SCRIPT_TRY_TO_FAINT: u32 = 1 << 1;
pub const AI_SCRIPT_CHECK_VIABILITY: u32 = 1 << 2;
pub const AI_SCRIPT_SETUP_FIRST_TURN: u32 = 1 << 3;
pub const AI_SCRIPT_FIRST_BATTLE: u32 = 1 << 31;

/// `AI_EFFECTIVENESS_*` (`Cmd_if_type_effectiveness`).
const X0: i32 = 0;
const X0_25: i32 = 10;
const X0_5: i32 = 20;
const X4: i32 = 160;

/// `AI_SCRIPT_DOUBLE_BATTLE`: added to the flags in doubles (`BattleAI_SetupAIData`).
pub const AI_SCRIPT_DOUBLE_BATTLE: u32 = 1 << 7;

/// The action of the AI-controlled battler on `side` (an opponent, or the in-game partner)
/// for this turn, chosen when the turn starts (Gen3 asks its controller during action
/// selection). The core only asks when a move is usable; it maps `Choice::Run` to fleeing
/// (`B_ACTION_RUN`) and `Choice::Switch` to a switch. In doubles the move comes with the
/// AI's target (`Choice::MoveAt`, `ChooseMoveOrAction_Doubles`).
pub fn choose(battle: &Battle, t: &BattleTables, rng: &mut Rng, side: Side) -> Choice {
    let flags = if battle.is_first_battle() {
        AI_SCRIPT_FIRST_BATTLE
    } else if battle.is_trainer() || (side == Side::PlayerRight && battle.has_partner()) {
        // A wild double's in-game partner: its own trainer's flags (`trainer_ai`).
        battle.trainer_ai
    } else {
        return wild(battle, t, rng, side);
    };
    if !battle.is_double() {
        let mut ai = Ai::setup(battle, t, rng, side, side.other());
        if let Some(c) = ai.run(flags) {
            return c;
        }
        let (slot, _) = ai.pick();
        return Choice::Move(slot);
    }
    doubles(battle, t, rng, side, flags | AI_SCRIPT_DOUBLE_BATTLE)
}

/// `ChooseMoveOrAction_Doubles`, after the controller's own `BattleAI_SetupAIData` (its rolls
/// are drawn, its results overwritten): for each other battler standing, the scripts against
/// it from fresh AI data and its best move (`Random() % n` among ties); an ally only counts
/// with 100 points or more. Then the target among the best (`Random() % n`).
fn doubles(battle: &Battle, t: &BattleTables, rng: &mut Rng, side: Side, flags: u32) -> Choice {
    let _ = Ai::setup(battle, t, rng, side, side.other());
    let mut points = [-1i16; 4];
    let mut index = [None::<Choice>; 4];
    for target in battle.sides() {
        let i = target.i();
        if target == side || battle.mon(target).hp == 0 {
            continue;
        }
        let mut ai = Ai::setup(battle, t, rng, side, target);
        if let Some(c) = ai.run(flags) {
            // AI_CHOICE_FLEE: `bestMovePointsForTarget[i]` is left unset (-1 here).
            index[i] = Some(c);
            continue;
        }
        let (slot, top) = ai.pick();
        index[i] = Some(Choice::MoveAt { slot, target });
        points[i] = if target == side.partner() && top < 100 {
            -1
        } else {
            top
        };
    }
    let mut best = [0usize; 4];
    let (mut n, mut top) = (1, points[0]);
    #[allow(clippy::needless_range_loop)]
    for i in 1..4 {
        if top == points[i] {
            best[n] = i;
            n += 1;
        }
        if top < points[i] {
            top = points[i];
            best[0] = i;
            n = 1;
        }
    }
    let target = best[(rng.random() % n as u16) as usize];
    index[target].unwrap_or(Choice::MoveAt {
        slot: 0,
        target: side.other(),
    })
}

/// `OpponentHandleChooseMove` outside the AI branch; a slot without PP is refused by
/// `TrySetCantSelectMoveBattleScript` and chosen again. In a double (a game's wild double),
/// a move that does not target its user aims at `GetBattlerAtPosition(Random() & 2)`: either
/// player battler.
fn wild(battle: &Battle, t: &BattleTables, rng: &mut Rng, side: Side) -> Choice {
    let m = battle.mon(side);
    if (0..4).all(|i| m.moves[i].is_none() || m.pp[i] == 0) {
        return Choice::Move(0);
    }
    loop {
        let slot = (rng.random() & 3) as usize;
        let Some(mv) = m.moves[slot].filter(|_| m.pp[slot] > 0) else {
            continue;
        };
        let user = matches!(
            t.move_info(mv).target,
            crate::mon::MoveTarget::User | crate::mon::MoveTarget::UserOrSelected
        );
        if !battle.is_double() || user {
            return Choice::Move(slot as u8);
        }
        let target = if rng.random() & 2 == 0 {
            Side::Player
        } else {
            Side::PlayerRight
        };
        return Choice::MoveAt {
            slot: slot as u8,
            target,
        };
    }
}

/// `AI_THINKING_STRUCT` for `sBattler_AI` (`user`) against `gBattlerTarget` (`target`).
struct Ai<'a> {
    b: &'a Battle,
    t: &'a BattleTables,
    rng: &'a mut Rng,
    user: Side,
    target: Side,
    /// `s8 score[4]`: `Cmd_score` adds in s8 (127 + 1 wraps to -128) and floors at 0.
    score: [i16; 4],
    /// `simulatedRNG`.
    sim: [i32; 4],
}

#[cfg(test)]
mod tests;

impl<'a> Ai<'a> {
    /// `BattleAI_SetupAIData(ALL_MOVES_MASK)`; in doubles it also draws a random target
    /// (`(Random() & BIT_FLANK) + BATTLE_OPPOSITE(side)`), which the callers here overwrite.
    fn setup(
        b: &'a Battle,
        t: &'a BattleTables,
        rng: &'a mut Rng,
        user: Side,
        target: Side,
    ) -> Self {
        let mut score = [0; 4];
        let mut sim = [0; 4];
        for i in 0..4 {
            // CheckMoveLimitations: no move, no PP, Disable, Torment, Taunt.
            score[i] = if !super::effects::slot_limited(b, t, user, i) {
                100
            } else {
                0
            };
            sim[i] = 100 - (rng.random() % 16) as i32;
        }
        if b.is_double() {
            rng.random();
        }
        Ai {
            b,
            t,
            rng,
            user,
            target,
            score,
            sim,
        }
    }

    /// The scripts of `flags`, bit 0 first, over the slots; `Some` when one ends the choice
    /// (`AI_FirstBattle`'s `flee`).
    fn run(&mut self, flags: u32) -> Option<Choice> {
        for bit in 0..32 {
            let flag = 1u32 << bit;
            if flags & flag == 0 {
                continue;
            }
            for slot in 0..4 {
                let Some(mov) = self.considered(slot) else {
                    self.score[slot] = 0;
                    continue;
                };
                match flag {
                    // `if_target_is_ally AI_Ret` opens each of these scripts.
                    AI_SCRIPT_CHECK_BAD_MOVE if !self.on_ally() => self.check_bad_move(slot, mov),
                    AI_SCRIPT_TRY_TO_FAINT if !self.on_ally() => self.try_to_faint(slot, mov),
                    AI_SCRIPT_CHECK_VIABILITY if !self.on_ally() => self.check_viability(slot, mov),
                    AI_SCRIPT_SETUP_FIRST_TURN if !self.on_ally() => {
                        self.setup_first_turn(slot, mov)
                    }
                    AI_SCRIPT_DOUBLE_BATTLE => self.double_battle(slot, mov),
                    // AI_FirstBattle: `flee` sets AI_ACTION_DO_NOT_ATTACK, ending the loop.
                    AI_SCRIPT_FIRST_BATTLE if self.hp(self.target) <= 20 => {
                        return Some(Choice::Run);
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// `if_target_is_ally`.
    fn on_ally(&self) -> bool {
        self.target.side() == self.user.side()
    }

    /// `moveConsidered`: `None` without PP.
    fn considered(&self, slot: usize) -> Option<MoveId> {
        let m = &self.b.battler(self.user).mon;
        if m.pp[slot] == 0 { None } else { m.moves[slot] }
    }

    /// `ChooseMoveOrAction_Singles` (and the per-target pick of `_Doubles`): slot 0, then the
    /// slots with a move; ties broken by `Random() % n`. The slot and its score.
    fn pick(&mut self) -> (u8, i16) {
        let m = &self.b.battler(self.user).mon;
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
        (best[(self.rng.random() % n as u16) as usize], top)
    }

    /// `AI_DoubleBattle` (its move checks by effect, the engine being game-agnostic: Earthquake
    /// and Magnitude are the Ground moves hitting the ally). Not modelled: Flash Fire's boost
    /// (`if_flash_fired`, never set) and the Persim Berry of `AI_TrySwaggerOnAlly`.
    fn double_battle(&mut self, slot: usize, mov: MoveId) {
        use crate::mon::MoveTarget;
        let info = self.info(mov);
        let partner = self.user.partner();
        if self.on_ally() {
            return self.try_on_ally(slot, mov);
        }
        if info.effect == MoveEffect::SkillSwap {
            let (u, t) = (self.b.battler(self.user), self.b.battler(self.target));
            if u.ability == Ability::Truant {
                self.add(slot, 5);
            } else if matches!(t.ability, Ability::ShadowTag | Ability::PurePower) {
                self.add(slot, 2);
            }
            return;
        }
        if info.ty == Type::Ground && info.target == MoveTarget::FoesAndAlly {
            // AI_DoubleBattleAllHittingGroundMove.
            let p = self.b.battler(partner);
            let d = if p.ability == Ability::Levitate || self.has_type(partner, Type::Flying) {
                2
            } else if [Type::Fire, Type::Electric, Type::Poison, Type::Rock]
                .iter()
                .any(|&ty| self.has_type(partner, ty))
            {
                -10
            } else {
                -3
            };
            return self.add(slot, d);
        }
        if info.ty == Type::Electric {
            // AI_DoubleBattleElectricMove.
            let tp = self.target.partner();
            if matches!(
                self.b.battler(tp).ability,
                Ability::LightningRod | Ability::LightningRodGen3
            ) {
                self.add(slot, -2);
                if self.has_type(tp, Type::Ground) {
                    self.add(slot, -8);
                }
            }
            return;
        }
        if info.ty == Type::Fire {
            return;
        }
        if self.b.battler(self.user).ability != Ability::Guts {
            // AI_DoubleBattleCheckUserStatus.
            if self.status(self.user) != Status::None {
                match self.most_powerful(slot, mov) {
                    None => self.add(slot, -5),
                    Some(true) => self.add(slot, 3),
                    Some(false) => self.add(slot, 1),
                }
            }
            return;
        }
        // AI_DoubleBattlePartnerHasHelpingHand.
        let helping = self
            .b
            .battler(partner)
            .mon
            .moves
            .into_iter()
            .flatten()
            .any(|m| self.info(m).effect == MoveEffect::HelpingHand);
        if helping && self.most_powerful(slot, mov).is_some() {
            self.add(slot, 1);
        }
    }

    /// `AI_TryOnAlly`.
    fn try_on_ally(&mut self, slot: usize, mov: MoveId) {
        let info = self.info(mov);
        if self.most_powerful(slot, mov).is_some() {
            if info.ty != Type::Fire {
                return self.add(slot, -30);
            }
            // AI_TryFireMoveOnAlly (Flash Fire's boost is not tracked: never fired yet).
            let d = if self.b.battler(self.target).ability == Ability::FlashFire {
                3
            } else {
                -30
            };
            return self.add(slot, d);
        }
        // AI_TryStatusMoveOnAlly.
        let target = self.b.battler(self.target);
        let d = match info.effect {
            MoveEffect::SkillSwap => {
                let ua = self.b.battler(self.user).ability;
                if target.ability == Ability::Truant {
                    10
                } else if ua == Ability::Levitate {
                    if target.ability == Ability::Levitate {
                        -30
                    } else {
                        // A type that is not Electric falls into AI_TrySkillSwapOnAlly2 (-30).
                        match target.types {
                            [Type::Electric, Type::Electric] => 2,
                            [Type::Electric, _] => -29,
                            _ => -30,
                        }
                    }
                } else {
                    // AI_TrySkillSwapOnAlly2: Compound Eyes with the partner's inaccurate
                    // moves (named by move id: not modelled).
                    -30
                }
            }
            MoveEffect::Status(StatusKind::Burn | StatusKind::Toxic) => {
                if target.ability != Ability::Guts
                    || self.status(self.target) != Status::None
                    || self.hp(self.user) < 91
                {
                    -30
                } else {
                    5
                }
            }
            MoveEffect::HelpingHand => {
                if self.random_lt(64) {
                    -1
                } else {
                    2
                }
            }
            _ => -30,
        };
        self.add(slot, d);
    }

    /// `Cmd_score`: `score[i] += d` stored as s8, then `if (score[i] < 0) score[i] = 0`.
    fn add(&mut self, slot: usize, d: i16) {
        let v = (self.score[slot] as i8).wrapping_add(d as i8);
        self.score[slot] = v.max(0) as i16;
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
        let (u, t) = (speed(self.user), speed(self.target));
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
        if self.has_type(self.user, info.ty) {
            d = calc::stab(d);
        }
        let target = self.b.battler(self.target);
        let r = calc::type_effect_in(
            self.t.type_chart,
            self.t.foresight_split,
            target.foresight,
            info.ty,
            target.types,
            info.power,
            d,
            self.b.setup.rules.inverse,
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
        let (user, target) = (self.b.battler(self.user), self.b.battler(self.target));
        let (a, d) = (user.fighter(), target.fighter());
        let mut dmg = calc::base_damage_in(&a, &d, info.power, info.ty, false, self.b.field());
        if user.charge > 0 && info.ty == Type::Electric {
            dmg *= 2;
        }
        if mov != self.t.struggle {
            if a.has_type(info.ty) {
                dmg = calc::stab(dmg);
            }
            dmg = calc::type_effect_in(
                self.t.type_chart,
                self.t.foresight_split,
                target.foresight,
                info.ty,
                d.types,
                info.power,
                dmg,
                self.b.setup.rules.inverse,
            )
            .damage;
        }
        (dmg * self.sim[slot] / 100).max(1)
    }

    fn ignored_powerful(info: &MoveInfo) -> bool {
        // `sIgnoredPowerfulMoveEffects`.
        matches!(
            info.effect,
            MoveEffect::Explosion
                | MoveEffect::DreamEater
                | MoveEffect::TwoTurn(_)
                | MoveEffect::Recharge
                | MoveEffect::SpitUp
                | MoveEffect::FocusPunch
                | MoveEffect::Superpower
                | MoveEffect::Eruption
                | MoveEffect::Overheat
        )
    }

    /// `Cmd_get_how_powerful_move_is`: `None` = `MOVE_POWER_OTHER`, else whether `slot` is
    /// the most powerful (no other move strictly stronger).
    fn most_powerful(&self, slot: usize, mov: MoveId) -> Option<bool> {
        let info = self.info(mov);
        if info.power <= 1 || Self::ignored_powerful(info) {
            return None;
        }
        let moves = self.b.battler(self.user).mon.moves;
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
        let target = self.b.battler(self.target);
        let user = self.b.battler(self.user);
        // AI_CheckBadMove_CheckSoundproof names Growl, Roar, Sing, Supersonic, Screech, Snore,
        // Uproar, Metal Sound and Grass Whistle: every sound move but Hyper Voice, the only
        // one with `EFFECT_HIT`.
        if target.ability == Ability::Soundproof
            && info.flags.has(MoveFlags::SOUND)
            && info.effect != MoveEffect::Hit
        {
            self.add(slot, -10);
            return;
        }
        let statused = target.mon.status != Status::None;
        let bad = match info.effect {
            MoveEffect::Status(StatusKind::Sleep) => {
                matches!(target.ability, Ability::Insomnia | Ability::VitalSpirit) || statused
            }
            MoveEffect::StatUp { stat, .. } => self.stage(self.user, stat) == 12,
            MoveEffect::DefenseCurl => self.stage(self.user, Stat::Def) == 12,
            MoveEffect::StatDown { stat, .. } => {
                // CheckIfAbilityBlocksStatChange: Clear Body.
                self.stage(self.target, stat) == 0
                    || (stat == Stat::Accuracy && target.ability == Ability::KeenEye)
                    || target.ability == Ability::ClearBody
            }
            // AI_CBM_HighRiskForDamage.
            MoveEffect::Bide | MoveEffect::SonicBoom => self.effectiveness(mov) == X0,
            // AI_CBM_Toxic (also EFFECT_POISON).
            MoveEffect::Status(StatusKind::Poison | StatusKind::Toxic) => {
                self.has_type(self.target, Type::Steel)
                    || self.has_type(self.target, Type::Poison)
                    || statused
            }
            MoveEffect::Status(StatusKind::Paralysis) => self.effectiveness(mov) == X0 || statused,
            MoveEffect::FocusEnergy => self.b.battler(self.user).focus_energy,
            MoveEffect::Confuse => {
                if target.confusion > 0 {
                    self.add(slot, -5);
                }
                false
            }
            MoveEffect::LeechSeed => target.seeded || self.has_type(self.target, Type::Grass),
            MoveEffect::MeanLook => target.v.escape_prevented,
            MoveEffect::Foresight => target.foresight,
            MoveEffect::Attract => {
                target.v.infatuated
                    || target.ability == Ability::Oblivious
                    || !opposite_genders(user.mon.gender, target.mon.gender)
            }
            MoveEffect::Teleport => true,
            MoveEffect::MudSport => user.v.mud_sport,
            MoveEffect::WaterSport => user.v.water_sport,
            _ => false,
        };
        if bad {
            self.add(slot, -10);
        }
    }

    /// `AI_SetupFirstTurn`: on the battle's first turn (`get_turn_count` =
    /// `gBattleResults.battleTurnCounter`), a move whose effect is in
    /// `AI_SetupFirstTurn_SetupEffectsToEncourage` gets +2 unless `if_random_less_than 80`.
    fn setup_first_turn(&mut self, slot: usize, mov: MoveId) {
        if self.b.turn != 0 || !setup_effect(self.info(mov).effect) {
            return;
        }
        if !self.random_lt(80) {
            self.add(slot, 2);
        }
    }

    /// `AI_TryToFaint`.
    fn try_to_faint(&mut self, slot: usize, mov: MoveId) {
        let info = self.info(mov);
        // if_can_faint.
        if info.power >= 2 && self.b.battler(self.target).mon.hp as i32 <= self.damage(slot, mov) {
            match info.effect {
                MoveEffect::Explosion => {}
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
            MoveEffect::AlwaysHit => self.cv_always_hit(slot),
            MoveEffect::StatDown { stat, .. } => match stat {
                Stat::Atk => self.cv_attack_down(slot, Stat::Atk, &PHYSICAL_TYPE_LIST),
                Stat::SpAtk => self.cv_attack_down(slot, Stat::SpAtk, &SPECIAL_TYPE_LIST),
                Stat::Def | Stat::SpDef | Stat::Evasion => self.cv_defense_down(slot, stat),
                Stat::Speed => self.cv_speed_down(slot),
                Stat::Accuracy => self.cv_accuracy_down(slot),
                Stat::Hp => {}
            },
            // AI_CV_Bide.
            MoveEffect::Bide if self.hp(self.user) <= 90 => self.add(slot, -2),
            MoveEffect::Status(StatusKind::Toxic) | MoveEffect::LeechSeed => self.cv_toxic(slot),
            MoveEffect::Confuse => self.cv_confuse(slot),
            // AI_CV_Poison.
            MoveEffect::Status(StatusKind::Poison)
                if self.hp(self.user) < 50 || self.hp(self.target) <= 50 =>
            {
                self.add(slot, -1)
            }
            MoveEffect::Status(StatusKind::Paralysis) => {
                if self.target_faster() {
                    if !self.random_lt(20) {
                        self.add(slot, 3);
                    }
                } else if self.hp(self.user) <= 70 {
                    self.add(slot, -1);
                }
            }
            MoveEffect::VitalThrow => self.cv_vital_throw(slot),
            // AI_CV_SpeedDownFromChance names Icy Wind, Rock Tomb and Mud Shot: the only
            // `EFFECT_SPEED_DOWN_HIT` moves with a 100% chance.
            MoveEffect::StatDownHit(Stat::Speed) if info.effect_chance == 100 => {
                self.cv_speed_down(slot)
            }
            // AI_CV_Trap.
            MoveEffect::MeanLook => {
                let t = self.b.battler(self.target);
                if (matches!(t.mon.status, Status::Toxic(_)) || t.v.infatuated)
                    && !self.random_lt(128)
                {
                    self.add(slot, 1);
                }
            }
            MoveEffect::Foresight => self.cv_foresight(slot),
            MoveEffect::MudSport => self.cv_sport(slot, Type::Electric),
            MoveEffect::WaterSport => self.cv_sport(slot, Type::Fire),
            _ => {}
        }
    }

    /// `AI_CV_Foresight` as built (no `BUGFIX`): the user's Ghost type and evasion, not the
    /// target's.
    fn cv_foresight(&mut self, slot: usize) {
        if self.has_type(self.user, Type::Ghost) {
            if self.random_lt(80) {
                return;
            }
        } else if self.stage(self.user, Stat::Evasion) <= 8 {
            self.add(slot, -2);
            return;
        }
        if !self.random_lt(80) {
            self.add(slot, 2);
        }
    }

    /// `AI_CV_MudSport` / `AI_CV_WaterSport`: +1 against a target of `ty` while the user has
    /// at least half its HP, else -1.
    fn cv_sport(&mut self, slot: usize, ty: Type) {
        if self.hp(self.user) >= 50 && self.has_type(self.target, ty) {
            self.add(slot, 1);
        } else {
            self.add(slot, -1);
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
        if self.stage(self.user, stat) < 9 {
            if self.hp(self.user) == 100 && !self.random_lt(128) {
                self.add(slot, 2);
            }
        } else if !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.hp(self.user) > high {
            return;
        }
        if self.hp(self.user) < 40 || !self.random_lt(keep) {
            self.add(slot, -2);
        }
    }

    /// AI_CV_DefenseUp / AI_CV_SpDefUp: the target's last move (`gLastMoves`) decides.
    fn cv_def_up(&mut self, slot: usize, stat: Stat) {
        if self.stage(self.user, stat) < 9 {
            if self.hp(self.user) == 100 && !self.random_lt(128) {
                self.add(slot, 2);
            }
        } else if !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.hp(self.user) >= 70 && self.random_lt(200) {
            return;
        }
        if self.hp(self.user) < 40 {
            self.add(slot, -2);
            return;
        }
        let last = self.b.battler(self.target).last_move.map(|m| self.info(m));
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
        if self.stage(self.user, Stat::Accuracy) >= 9 && !self.random_lt(50) {
            self.add(slot, -2);
        }
        if self.hp(self.user) <= 70 {
            self.add(slot, -2);
        }
    }

    fn cv_evasion_up(&mut self, slot: usize) {
        if self.hp(self.user) >= 90 && !self.random_lt(100) {
            self.add(slot, 3);
        }
        if self.stage(self.user, Stat::Evasion) >= 9 && !self.random_lt(128) {
            self.add(slot, -1);
        }
        if matches!(self.status(self.target), Status::Toxic(_)) {
            let go = self.hp(self.user) > 50 || !self.random_lt(80);
            if go && !self.random_lt(50) {
                self.add(slot, 3);
            }
        }
        if self.b.battler(self.target).seeded && !self.random_lt(70) {
            self.add(slot, 3);
        }
        // STATUS3_ROOTED and STATUS2_CURSED are not modelled (never set).
        self.cv_low_hp_tail(slot, self.user, Stat::Evasion);
    }

    /// The common tail of AI_CV_EvasionUp / AI_CV_AccuracyDown: at ≤ 70% HP with `stat` of
    /// `side` changed, -2 unless a 70/256 roll keeps it (always -2 under 40% HP either side).
    fn cv_low_hp_tail(&mut self, slot: usize, side: Side, stat: Stat) {
        if self.hp(self.user) > 70 || self.stage(side, stat) == 6 {
            return;
        }
        if self.hp(self.user) < 40 || self.hp(self.target) < 40 || !self.random_lt(70) {
            self.add(slot, -2);
        }
    }

    fn cv_always_hit(&mut self, slot: usize) {
        let (eva, acc) = (
            self.stage(self.target, Stat::Evasion),
            self.stage(self.user, Stat::Accuracy),
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
        if self.stage(self.target, Stat::Atk) != 6 {
            self.add(slot, -1);
            if self.hp(self.user) <= 90 {
                self.add(slot, -1);
            }
            if self.stage(self.target, stat) <= 3 && !self.random_lt(50) {
                self.add(slot, -2);
            }
        }
        if self.hp(self.target) <= 70 {
            self.add(slot, -2);
        }
        let [t1, t2] = self.b.battler(self.target).types;
        if types.contains(&t1) || types.contains(&t2) {
            return;
        }
        if !self.random_lt(50) {
            self.add(slot, -2);
        }
    }

    /// AI_CV_DefenseDown / SpDefDown / EvasionDown.
    fn cv_defense_down(&mut self, slot: usize, stat: Stat) {
        if (self.hp(self.user) < 70 || self.stage(self.target, stat) <= 3) && !self.random_lt(50) {
            self.add(slot, -2);
        }
        if self.hp(self.target) <= 70 {
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
        if (self.hp(self.user) < 70 || self.hp(self.target) <= 70) && !self.random_lt(100) {
            self.add(slot, -1);
        }
        if self.stage(self.user, Stat::Accuracy) <= 4 && !self.random_lt(80) {
            self.add(slot, -2);
        }
        if matches!(self.status(self.target), Status::Toxic(_)) && !self.random_lt(70) {
            self.add(slot, 2);
        }
        if self.b.battler(self.target).seeded && !self.random_lt(70) {
            self.add(slot, 2);
        }
        // STATUS3_ROOTED and STATUS2_CURSED are not modelled; the tail reads the target's
        // accuracy, as Gen3.
        self.cv_low_hp_tail(slot, self.target, Stat::Accuracy);
    }

    /// AI_CV_Toxic (also Leech Seed).
    fn cv_toxic(&mut self, slot: usize) {
        let moves = self.b.battler(self.user).mon.moves;
        let attacking = moves.iter().flatten().any(|&m| self.info(m).power != 0);
        if attacking {
            if self.hp(self.user) <= 50 && !self.random_lt(50) {
                self.add(slot, -3);
            }
            if self.hp(self.target) <= 50 && !self.random_lt(50) {
                self.add(slot, -3);
            }
        }
        let encouraged = moves.iter().flatten().any(|&m| {
            matches!(
                self.info(m).effect,
                MoveEffect::StatUp {
                    stat: Stat::SpDef,
                    stages: 1
                } | MoveEffect::Protect
            )
        });
        if encouraged && !self.random_lt(60) {
            self.add(slot, 2);
        }
    }

    fn cv_confuse(&mut self, slot: usize) {
        if self.hp(self.target) > 70 {
            return;
        }
        if !self.random_lt(128) {
            self.add(slot, -1);
        }
        if self.hp(self.target) > 50 {
            return;
        }
        self.add(slot, -1);
        if self.hp(self.target) <= 30 {
            self.add(slot, -1);
        }
    }

    fn cv_vital_throw(&mut self, slot: usize) {
        if self.target_faster() || self.hp(self.user) > 60 {
            return;
        }
        if self.hp(self.user) >= 40 && self.random_lt(180) {
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

/// Whether `e` is in `AI_SetupFirstTurn_SetupEffectsToEncourage`: every stat up / down (1 or
/// 2 stages), Focus Energy, Confuse, Poison, Paralyze, Leech Seed, Defense Curl, Yawn,
/// Substitute and Imprison (not Sleep nor Toxic).
fn setup_effect(e: MoveEffect) -> bool {
    match e {
        MoveEffect::StatUp { .. }
        | MoveEffect::StatDown { .. }
        | MoveEffect::FocusEnergy
        | MoveEffect::Confuse
        | MoveEffect::Status(StatusKind::Poison | StatusKind::Paralysis)
        | MoveEffect::LeechSeed
        | MoveEffect::DefenseCurl
        | MoveEffect::Yawn
        | MoveEffect::Status(StatusKind::Burn)
        | MoveEffect::Conversion
        | MoveEffect::LightScreen
        | MoveEffect::Reflect
        | MoveEffect::Minimize
        | MoveEffect::Curse
        | MoveEffect::Swagger
        | MoveEffect::Torment
        | MoveEffect::Flatter
        | MoveEffect::Ingrain
        | MoveEffect::TeeterDance
        | MoveEffect::Camouflage
        | MoveEffect::Tickle
        | MoveEffect::Substitute
        | MoveEffect::Imprison => true,
        // COSMIC_POWER, BULK_UP, CALM_MIND (not DRAGON_DANCE).
        MoveEffect::StatsUp(a, b) => (a, b) != (Stat::Atk, Stat::Speed),
        _ => false,
    }
}

/// `AI_TrySwitchOrUseItem` for the opponent (`battle_ai_switch_items.c`): the party slot to
/// switch to, or `None` to choose a move. Trainer battles only. `ShouldSwitch` as the engine
/// can reach it, drawing every `Random()` Gen3 draws there:
/// - trapped (Mean Look's `STATUS2_ESCAPE_PREVENTION`, Shadow Tag, the foe's Arena Trap
///   (without Gen3's Flying / Levitate check), Magnet Pull on the field and a Steel user): no
///   switch, no draw (Wrap and Ingrain are not in the engine);
/// - nothing to do without another mon that can fight (`availableToSwitch == 0`);
/// - `ShouldSwitchIfPerishSong`: no Perish Song;
/// - `ShouldSwitchIfWonderGuard`: against Wonder Guard with no super effective move, the first
///   party mon with one (`Random() % 3 < 2` per such move);
/// - `FindMonThatAbsorbsOpponentsMove`: `HasSuperEffectiveMoveAgainstOpponents(TRUE) &&
///   Random() % 3 != 0` keeps the mon in; otherwise the last move that landed on it (with
///   power) is Fire / Water / Electric and a party mon has Flash Fire / Water Absorb / Volt
///   Absorb (`Random() & 1` each);
/// - `ShouldSwitchIfNaturalCure` is not modelled: an asleep Natural Cure trainer mon at half
///   HP or more would draw `Random() & 1` there and switch through
///   `GetMostSuitableMonToSwitchInto` (no Act 1 trainer mon has Natural Cure);
/// - `HasSuperEffectiveMoveAgainstOpponents(FALSE)` (`Random() % 10 != 0` per super effective
///   move) and `AreStatsRaised` keep the mon in;
/// - `FindMonWithFlagsAndSuperEffective(DOESNT_AFFECT_FOE, 2)`, then `(NOT_VERY_EFFECTIVE, 3)`.
///
/// `ShouldUseItem` is out: the slice trainers have no items (`gTrainers[].items` all
/// `ITEM_NONE`), so it never draws.
pub fn switch(b: &Battle, t: &BattleTables, rng: &mut Rng, side: Side) -> Option<u8> {
    if !b.is_trainer() {
        return None;
    }
    let me = b.battler(side);
    // STATUS2_ESCAPE_PREVENTION; `ABILITY_ON_FIELD2(ABILITY_MAGNET_PULL)` (any battler, the
    // user included, fainted or not) with a Steel user.
    let magnet = b
        .sides()
        .any(|s| b.battler(s).ability == Ability::MagnetPull);
    // ABILITY_ON_OPPOSING_FIELD(ABILITY_ARENA_TRAP).
    let arena = foes(b, side).any(|s| b.battler(s).ability == Ability::ArenaTrap);
    if me.v.escape_prevented
        || (magnet && me.types.contains(&Type::Steel))
        || super::effects::shadow_tagged(b, side)
        || arena
    {
        return None;
    }
    // availableToSwitch: the party range, not out on the side nor chosen by the partner.
    if !b.party_range(side).any(|i| b.can_switch_to(side, i)) {
        return None;
    }
    if let Some(i) = switch_if_wonder_guard(b, t, rng, side) {
        return Some(i);
    }
    // FindMonThatAbsorbsOpponentsMove (its `&&` draws only after a hit).
    let keep = super_effective_move(b, t, rng, side, true) && !rng.random().is_multiple_of(3);
    if !keep && let Some(i) = absorber(b, t, rng, side) {
        return Some(i);
    }
    if super_effective_move(b, t, rng, side, false) {
        return None;
    }
    // AreStatsRaised: the raised stages add up to more than 3.
    let raised: i32 = me.stages.iter().map(|&s| s.max(0) as i32).sum();
    if raised > 3 {
        return None;
    }
    find_mon(b, t, rng, side, Effectiveness::NoEffect, 2)
        .or_else(|| find_mon(b, t, rng, side, Effectiveness::NotVery, 3))
}

/// The opposing battlers on the field.
fn foes(b: &Battle, side: Side) -> impl Iterator<Item = Side> + use<'_> {
    [side.other(), side.other().partner()]
        .into_iter()
        .filter(|&s| b.is_present(s))
}

/// The party mons `side` may switch to (`firstId..lastId`, not `battlerIn1`/`battlerIn2` nor
/// their `monToSwitchIntoId`, standing).
fn candidates<'b>(
    b: &'b Battle,
    side: Side,
) -> impl Iterator<Item = (usize, &'b crate::mon::Pokemon)> + use<'b> {
    let party = b.party(side);
    b.party_range(side)
        .filter(move |&i| b.can_switch_to(side, i))
        .filter_map(move |i| party.get(i as usize).map(|p| (i as usize, p)))
}

/// `ShouldSwitchIfWonderGuard`: the player's battler has Wonder Guard and no move of the AI's
/// battler is super effective on it; the first other party mon (with HP) with a super
/// effective move passing `Random() % 3 < 2`.
fn switch_if_wonder_guard(b: &Battle, t: &BattleTables, rng: &mut Rng, side: Side) -> Option<u8> {
    if b.is_double() {
        return None;
    }
    let (me, foe) = (b.battler(side), b.battler(side.other()));
    if foe.ability != Ability::WonderGuard {
        return None;
    }
    let (species, ability) = (foe.mon.species, foe.ability);
    let super_eff = |m: MoveId| type_calc(b, t, m, species, ability) == Effectiveness::Super;
    if me.mon.moves.into_iter().flatten().any(super_eff) {
        return None;
    }
    for (i, p) in candidates(b, side) {
        for m in p.moves.into_iter().flatten() {
            if super_eff(m) && rng.random() % 3 < 2 {
                return Some(i as u8);
            }
        }
    }
    None
}

/// `FindMonThatAbsorbsOpponentsMove` after its first line: the move that last landed on the
/// AI's battler (with power) is Fire / Water / Electric, the battler lacks the absorbing
/// ability, and an other party mon (with HP) has it, passing `Random() & 1`.
fn absorber(b: &Battle, t: &BattleTables, rng: &mut Rng, side: Side) -> Option<u8> {
    let me = b.battler(side);
    let info = t.move_info(me.last_landed?);
    if info.power == 0 {
        return None;
    }
    let absorbing = match info.ty {
        Type::Fire => Ability::FlashFire,
        Type::Water => Ability::WaterAbsorb,
        Type::Electric => Ability::VoltAbsorb,
        _ => return None,
    };
    if me.ability == absorbing {
        return None;
    }
    for (i, p) in candidates(b, side) {
        if p.ability(t) == absorbing && rng.random() & 1 != 0 {
            return Some(i as u8);
        }
    }
    None
}

/// `AI_TypeCalc(move, species, ability)`: the chart on the species' types (Foresight ignored);
/// a Ground move on Levitate and, on Wonder Guard, a move with power that is not super
/// effective do not affect it; Struggle has no flags.
fn type_calc(
    b: &Battle,
    t: &BattleTables,
    mov: MoveId,
    species: crate::data::SpeciesId,
    ability: Ability,
) -> Effectiveness {
    if mov == t.struggle {
        return Effectiveness::Normal;
    }
    let info = t.move_info(mov);
    if ability == Ability::Levitate && info.ty == Type::Ground {
        return Effectiveness::NoEffect;
    }
    let types = t.species_info(species).types;
    let eff = calc::type_effect_in(
        t.type_chart,
        t.foresight_split,
        false,
        info.ty,
        types,
        info.power,
        1,
        b.setup.rules.inverse,
    )
    .eff;
    if ability == Ability::WonderGuard && eff != Effectiveness::Super && info.power != 0 {
        return Effectiveness::NoEffect;
    }
    eff
}

/// `HasSuperEffectiveMoveAgainstOpponents(noRng)` (singles): a move of the AI's battler super
/// effective on the player's species; with RNG each one passes on `Random() % 10 != 0`.
fn super_effective_move(
    b: &Battle,
    t: &BattleTables,
    rng: &mut Rng,
    side: Side,
    no_rng: bool,
) -> bool {
    // The opposite battler, then (doubles) its partner, when present.
    let opposite = side.other();
    for foe in [opposite, opposite.partner()] {
        if !b.is_present(foe) {
            continue;
        }
        let foe = b.battler(foe);
        let (foe, ability) = (foe.mon.species, foe.ability);
        for m in b.battler(side).mon.moves.into_iter().flatten() {
            if type_calc(b, t, m, foe, ability) == Effectiveness::Super
                && (no_rng || !rng.random().is_multiple_of(10))
            {
                return true;
            }
        }
    }
    false
}

/// `FindMonWithFlagsAndSuperEffective(flags, moduloPercent)`: the move that last hit the AI's
/// battler (with power) has `flags` on a party mon, which has a move super effective on the
/// attacker (`gLastHitBy`, the player) passing `Random() % moduloPercent == 0`.
fn find_mon(
    b: &Battle,
    t: &BattleTables,
    rng: &mut Rng,
    side: Side,
    flags: Effectiveness,
    modulo: u16,
) -> Option<u8> {
    let me = b.battler(side);
    let last = me.last_landed?;
    if t.move_info(last).power == 0 {
        return None;
    }
    // gLastHitBy is not kept: the opposite battler.
    let foe = b.battler(side.other());
    let (foe, ability) = (foe.mon.species, foe.ability);
    for (i, p) in candidates(b, side) {
        if type_calc(b, t, last, p.species, p.ability(t)) != flags {
            continue;
        }
        for m in p.moves.into_iter().flatten() {
            if type_calc(b, t, m, foe, ability) == Effectiveness::Super
                && rng.random().is_multiple_of(modulo)
            {
                return Some(i as u8);
            }
        }
    }
    None
}
