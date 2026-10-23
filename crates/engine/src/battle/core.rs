//! The turn engine. **Task D.**
//!
//! Gen3 references: `battle_main.c` (`GetWhoStrikesFirst`, `SetActionsAndBattlersTurnOrder`,
//! `TryDoEventsBeforeFirstTurn`, `BattleTurnPassed`, `HandleEndTurn_*`), `battle_util.c`
//! (`HandleFaintedMonActions`, `TryRunFromBattle`), `battle_script_commands.c` and
//! `data/battle_scripts_1.s`. Pure and video-free: takes `Choice`s, pushes `BattleEvent`s, and
//! draws every random roll from the `Rng` it is handed, one `rng.random()` per Gen3 `Random()`
//! at the same point of the sequence (the per-VBlank `Random()` excepted).
//!
//! # Flow
//! The battle is a queue of [`Step`]s run until one needs the player ([`Need`]):
//! - `new` draws Gen3's `BattleStartClearSetData` roll (`gRandomTurnNumber`).
//! - `start` pushes the intro, then [`Step::FirstTurn`] (switch-in effects in speed order,
//!   `TryDoEventsBeforeFirstTurn`) and [`Step::NewTurn`] (`gRandomTurnNumber`, the opponent's
//!   choice from [`super::ai::choose`], then `Need::Action`).
//! - A chosen action orders the turn (`SetActionsAndBattlersTurnOrder`) into
//!   `Act(first), Fainted, Act(second), Fainted, EndTurn`. `Fainted` is Gen3
//!   `HandleAction_TryFinish` → `HandleFaintedMonActions`: exp for a fainted opponent, then the
//!   win/loss check (`checkteamslost`) and replacements, which cancel the turn's remaining
//!   actions (`cancelallactions`).
//! - `EndTurn` runs end-of-turn effects ([`super::effects::end_turn`]), faints, then a new turn.
//!
//! # Seams for Task E2
//! Move effects other than `Hit`/`QuickAttack`/`StatUp`/`StatDown`/`Recoil` go through
//! [`super::effects::apply`]; `effects` also gets `before_move` (attackcanceler statuses),
//! `end_turn`, `switch_in`, `blocks_stat_drop` and `ball`. They work on the `pub(crate)` state
//! below with the building blocks of `core/moves.rs` (accuracy, damage, HP changes, stat
//! changes, faints, messages).
//!
//! # Messages
//! `MsgWait` follows the script's `waitmessage`: `Frames(64)` after `waitmessage
//! B_WAIT_TIME_LONG`, `Frames(0)` for a `printstring` without one (a `\p` in the text still
//! waits), `Button` for the texts that end the battle. Fields: `attacker`/`target` are Gen3's
//! `gBattlerAttacker`/`gBattlerTarget` of the script, `active` the battler the message is about.
//! `slots` is stamped with each side's battler party slot when the message is emitted
//! ([`Battle::stamp`]), so the text names the mons out at that point.
//!
//! # Outcomes
//! `Drew` (both last battlers fainted together) plays the whiteout texts like `Lost`: Gen3
//! `IsPlayerDefeated(B_OUTCOME_DREW)`, so Task J whites out on it too. Teleport ends a wild
//! battle with `PlayerTeleported` / `MonTeleported` and no closing message
//! (`HandleEndTurn_FinishBattle`); the world should treat them as `Ran` / `MonFled`.
//!
//! # Not modelled
//! - Friendship changes (fainting, level ups, `AdjustFriendship` at the battle's end).
//! - The `STRINGID_ENEMYABOUTTOSWITCHPKMN` "Will you change Pokémon?" prompt when a trainer
//!   sends its next mon (skipped, per the milestone's decision 4).
//! - Wild held items (`SetWildMonHeldItem` keeps only its `Random()`), badge boosts.
//!
//! # Silice engine (items, weather, forms)
//! A battler carries its party mon's held item and its `GameData::hold_effect`
//! ([`BattlerExt`], hashed only when set); Quick Claw is in [`Battle::strike_order`], the other
//! items in `effects::items`, `calc` and `Battle::damage`. [`BattleExt`] (hashed only when set)
//! holds the weather and the form changes ([`forms`]: Mega Evolution, Primal Reversion). A
//! Gen3 battle without items, forms or weather abilities draws and hashes as before.

mod forms;
mod items;
mod moves;
#[cfg(test)]
pub(crate) mod tests;

use super::calc::{self, Fighter};
use super::{
    BattleEvent, BattleKind, BattleMon, BattleOutcome, BattleReport, BattleSetup, Buff, Choice,
    Msg, MsgWait, Need, Side, Str,
};
use crate::data::{FlagId, FormChange, GameData, ItemId, MoveId, SpeciesId};
use crate::hash::Fnv64;
use crate::mon::{
    Ability, BattleTables, HoldEffect, Learn, PARTY_SIZE, Party, Pokemon, StatusKind, Type,
};
use crate::rng::Rng;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

// The building blocks of Task E2's effects.
#[allow(unused_imports)]
pub(crate) use moves::{HitResult, MoveUse, Secondary};

/// What the core hands its building blocks: the tables, the RNG and the event sink.
pub(crate) struct Env<'a> {
    pub t: &'a BattleTables,
    pub rng: &'a mut Rng,
    pub out: &'a mut Vec<BattleEvent>,
    /// `out[..stamped]` has its messages' `Msg::slots` set (see [`Battle::stamp`]).
    stamped: usize,
}

impl<'a> Env<'a> {
    pub(crate) fn new(
        t: &'a BattleTables,
        rng: &'a mut Rng,
        out: &'a mut Vec<BattleEvent>,
    ) -> Self {
        let stamped = out.len();
        Env {
            t,
            rng,
            out,
            stamped,
        }
    }
}

/// Core-private per-battler state (Gen3 `struct BattlePokemon` beyond [`BattleMon`], the
/// STATUS2/STATUS3 subset, `gDisableStructs` bits) next to the public view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Battler {
    pub mon: BattleMon,
    /// `Stat` order, copied from the party mon (`stats[0]` = max HP).
    pub stats: [u16; 6],
    pub types: [Type; 2],
    pub ability: Ability,
    /// Stage per `Stat`, -6..=6 (`stages[0]` unused).
    pub stages: [i8; 8],
    /// `STATUS2_CONFUSION` turns left.
    pub confusion: u8,
    /// `STATUS2_FLINCHED`, cleared at the end of the turn.
    pub flinched: bool,
    /// `STATUS2_FOCUS_ENERGY`.
    pub focus_energy: bool,
    /// `STATUS3_LEECHSEED`.
    pub seeded: bool,
    /// `STATUS2_FORESIGHT`.
    pub foresight: bool,
    /// `STATUS2_BIDE` turns left.
    pub bide_turns: u8,
    /// `gDisableStructs.chargeTimer` (`STATUS3_CHARGED_UP` while non-zero): Electric damage ×2.
    pub charge: u8,
    /// `gBideDmg`: every move damage this battler took (`gBattleMoveDamage`, before the HP
    /// clamp), recorded by [`Battle::change_hp`] (not by `change_hp_passive`). `gBideTarget` is
    /// always the other battler in singles. Bide itself (E2) clears it when it starts.
    pub bide_damage: u32,
    /// `gLastMoves`: the last move this battler used.
    pub last_move: Option<MoveId>,
    /// `gLastLandedMoves`, as the switch AI reads it: the last move with power that hit this
    /// battler (`MOVEEND_UPDATE_LAST_MOVES` with no `MOVE_RESULT_NO_EFFECT`). `None` stands
    /// for `MOVE_NONE`, `MOVE_UNAVAILABLE` (a cancelled, missed or failed move) and moves
    /// without power alike: `AI_TrySwitchOrUseItem` treats them the same.
    /// `gLastHitBy` is the other battler in singles.
    pub last_landed: Option<MoveId>,
    /// `HITMARKER_FAINTED`: the faint was played.
    pub fainted: bool,
    /// Task G's volatile state (hashed only when set, see [`Volatiles`]).
    pub v: Volatiles,
    /// The Silice engine's state (hashed only when set, see [`BattlerExt`]).
    pub x: BattlerExt,
}

/// A battler's held item and the Silice engine's per-battler state. All default for a Gen3
/// battle without items or the new abilities, so it is hashed only when set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct BattlerExt {
    /// `gBattleMons[].item`.
    pub item: Option<ItemId>,
    /// `GetBattlerHoldEffect` of `item` (`GameData::hold_effect`).
    pub hold: Option<HoldEffect>,
    /// `gDisableStructs.isFirstTurn` for Speed Boost: 2 on entry, decremented when a turn
    /// passes (set only for a Speed Boost battler, so other battles hash as before).
    pub first_turn: u8,
}

impl BattlerExt {
    fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.item.map_or(0xFFFF, |i| i.0));
        h.write(&self.hold.map_or([0xFF, 0], HoldEffect::code));
        h.write(&[self.first_turn]);
    }
}

/// Task G's `STATUS2` / `STATUS3` / `gDisableStructs` / `gProtectStructs` bits of a battler.
/// In singles the other battler is always the source (`STATUS2_INFATUATED_WITH`,
/// `battlerPreventingEscape`), so its switch-out or faint clears them
/// ([`Battle::clear_bonds`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Volatiles {
    /// `STATUS2_INFATUATED_WITH(other)`.
    pub infatuated: bool,
    /// `STATUS2_ESCAPE_PREVENTION` by the other battler.
    pub escape_prevented: bool,
    /// `STATUS3_MUDSPORT`.
    pub mud_sport: bool,
    /// `STATUS3_WATERSPORT`.
    pub water_sport: bool,
    /// `STATUS2_UPROAR` turns left (`STATUS2_MULTIPLETURNS` while set).
    pub uproar: u8,
    /// `STATUS3_YAWN` turns left.
    pub yawn: u8,
    /// `gDisableStructs.furyCutterCounter`.
    pub fury_cutter: u8,
    /// `gDisableStructs.truantCounter`: loafs this turn.
    pub truant: bool,
    /// `WasUnableToUseMove`: a `gProtectStructs` immobility flag of this turn (paralysis,
    /// love, flinch, confusion self-hit, a type immunity), cleared when the turn passes.
    pub unable: bool,
}

impl Volatiles {
    fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            self.infatuated as u8,
            self.escape_prevented as u8,
            self.mud_sport as u8,
            self.water_sport as u8,
            self.uproar,
            self.yawn,
            self.fury_cutter,
            self.truant as u8,
            self.unable as u8,
        ]);
    }
}

impl Battler {
    /// Gen3 `CopyMonToBattleMon` + `SwitchInClearSetData` for party slot `party`.
    pub fn new(t: &BattleTables, p: &Pokemon, party: u8) -> Battler {
        let max_pp = core::array::from_fn(|slot| p.max_pp(t, slot as u8));
        Battler {
            mon: BattleMon {
                party,
                species: p.species,
                nickname: p.nickname,
                level: p.level,
                hp: p.hp,
                max_hp: p.stats[0],
                status: p.status,
                gender: p.gender(t),
                moves: p.moves,
                pp: p.pp,
                max_pp,
            },
            stats: p.stats,
            types: t.species_info(p.species).types,
            ability: p.ability(t),
            stages: [0; 8],
            confusion: 0,
            flinched: false,
            focus_energy: false,
            seeded: false,
            foresight: false,
            bide_turns: 0,
            charge: 0,
            bide_damage: 0,
            last_move: None,
            last_landed: None,
            fainted: false,
            v: Volatiles::default(),
            x: BattlerExt {
                item: p.held,
                ..BattlerExt::default()
            },
        }
    }

    /// What the formulas read.
    pub fn fighter(&self) -> Fighter {
        Fighter {
            level: self.mon.level,
            hp: self.mon.hp,
            max_hp: self.mon.max_hp,
            stats: self.stats,
            stages: self.stages,
            types: self.types,
            ability: self.ability,
            status: self.mon.status,
            hold: self.x.hold,
        }
    }

    /// Gen3 `AreAllMovesUnusable` (no Disable/Taunt yet): every known move is out of PP.
    pub fn all_moves_unusable(&self) -> bool {
        (0..4).all(|i| self.mon.moves[i].is_none() || self.mon.pp[i] == 0)
    }

    fn hash(&self, h: &mut Fnv64) {
        let m = &self.mon;
        h.write(&[m.party, m.level, m.gender as u8]);
        h.write_u16(m.species.0);
        h.write(&m.nickname);
        h.write_u16(m.hp);
        h.write_u16(m.max_hp);
        let (tag, n) = m.status.code();
        h.write(&[tag, n]);
        for mv in m.moves {
            h.write_u16(mv.map_or(0xFFFF, |m| m.0));
        }
        h.write(&m.pp);
        h.write(&m.max_pp);
        for s in self.stats {
            h.write_u16(s);
        }
        h.write(&[self.types[0] as u8, self.types[1] as u8, self.ability as u8]);
        for s in self.stages {
            h.write(&[s as u8]);
        }
        h.write(&[
            self.confusion,
            self.flinched as u8,
            self.focus_energy as u8,
            self.seeded as u8,
            self.foresight as u8,
            self.bide_turns,
            self.charge,
            self.fainted as u8,
        ]);
        h.write_u32(self.bide_damage);
        h.write_u16(self.last_move.map_or(0xFFFF, |m| m.0));
        h.write_u16(self.last_landed.map_or(0xFFFF, |m| m.0));
        // Task G: a tagged block only while any is set. Some are set mid-turn by effects that
        // existed before Task G (`unable` by a flinch, full paralysis, a miss on an immune
        // target…), so such turns hash differently than before.
        if self.v != Volatiles::default() {
            h.write(&[0xA7]);
            self.v.hash(h);
        }
        if self.x != BattlerExt::default() {
            h.write(&[0xA8]);
            self.x.hash(h);
        }
    }
}

/// Gen3 `gBattleWeather` (the engine's subset), plus the Gen6 primal weathers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Weather {
    #[default]
    None,
    /// `B_WEATHER_RAIN_PERMANENT` (Drizzle).
    Rain,
    /// `B_WEATHER_SUN_PERMANENT` (Drought).
    Sun,
    /// `B_WEATHER_SANDSTORM_PERMANENT` (Sand Stream).
    Sand,
    /// Primordial Sea: rain, and Fire moves fail. Ends when no battler has the ability.
    HeavyRain,
    /// Desolate Land: sun, and Water moves fail.
    HarshSun,
}

impl Weather {
    pub fn rain(self) -> bool {
        matches!(self, Weather::Rain | Weather::HeavyRain)
    }

    pub fn sun(self) -> bool {
        matches!(self, Weather::Sun | Weather::HarshSun)
    }

    pub fn primal(self) -> bool {
        matches!(self, Weather::HeavyRain | Weather::HarshSun)
    }
}

/// The Silice engine's battle state: held-item effects and form changes by party slot, the
/// Mega Evolution bookkeeping and the weather. Default for a Gen3 battle without items, forms
/// or weather abilities, so it is hashed only when set.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct BattleExt {
    /// `GameData::hold_effect` of each party mon's item (`[side][party slot]`).
    pub holds: [[Option<HoldEffect>; PARTY_SIZE]; 2],
    /// The form change each party mon may undergo (its species and held item match one of
    /// `GameData::form_changes`; the player's also need their `enable_flag`).
    pub forms: [[Option<FormChange>; PARTY_SIZE]; 2],
    /// Party slots in their battle form (a bit each): kept until the battle ends.
    pub formed: [u8; 2],
    /// One Mega Evolution per side per battle.
    pub mega_used: [bool; 2],
    /// Mega Evolution chosen for this turn (the player's toggle, the AI's choice).
    pub mega_pending: [bool; 2],
    pub weather: Weather,
}

impl BattleExt {
    fn hash(&self, h: &mut Fnv64) {
        for side in 0..2 {
            for slot in 0..PARTY_SIZE {
                h.write(&self.holds[side][slot].map_or([0xFF, 0], HoldEffect::code));
                match self.forms[side][slot] {
                    None => h.write(&[0xFF]),
                    Some(f) => {
                        h.write(&[f.kind as u8]);
                        h.write_u16(f.base.0);
                        h.write_u16(f.item.0);
                        h.write_u16(f.form.0);
                    }
                }
            }
            h.write(&[
                self.formed[side],
                self.mega_used[side] as u8,
                self.mega_pending[side] as u8,
            ]);
        }
        h.write(&[self.weather as u8]);
    }
}

/// A battler's action for the turn (Gen3 `B_ACTION_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    /// `B_ACTION_USE_MOVE` with a move slot.
    Move(u8),
    /// `B_ACTION_USE_MOVE` with no usable move (`noValidMoves`).
    Struggle,
    /// `B_ACTION_SWITCH` to a party slot.
    Switch(u8),
    /// `B_ACTION_RUN` (for the opponent: a wild mon fleeing).
    Run,
    /// `B_ACTION_USE_ITEM` with a Poké Ball (Task E2).
    Ball,
}

impl Action {
    fn code(self) -> [u8; 2] {
        match self {
            Action::Move(s) => [0, s],
            Action::Struggle => [1, 0],
            Action::Switch(s) => [2, s],
            Action::Run => [3, 0],
            Action::Ball => [4, 0],
        }
    }
}

/// One unit of the battle flow (see the module docs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Step {
    /// `TryDoEventsBeforeFirstTurn`.
    FirstTurn,
    /// `gRandomTurnNumber`, the opponent's choice, then `Need::Action`.
    NewTurn,
    /// The chosen action of a side.
    Act(Side),
    /// `HandleFaintedMonActions`.
    Fainted,
    /// `BattleScript_HandleFaintedMon` for one side (`checkteamslost`, replacement).
    Replace(Side),
    /// `Cmd_getexp` for player party slot `party` and the following slots.
    GainExp {
        party: u8,
        exp: u32,
        sent: u8,
        defeated: SpeciesId,
    },
    /// One `add_exp_step` of party slot `party` with `rest` exp left.
    AddExp { party: u8, rest: u32 },
    /// `handlelearnnewmove`: the learnset moves of `level`, from index `next`.
    Learn { party: u8, level: u8, next: u8 },
    /// `BattleTurnPassed`.
    EndTurn,
    /// The rest of `BattleScript_ActionSwitch` after a Pursuit hit (and its exp).
    SwitchOut { side: Side, party: u8 },
    /// The pending Mega Evolutions before the first move action (Silice; queued only when
    /// one is pending).
    Megas,
}

impl Step {
    fn hash(&self, h: &mut Fnv64) {
        match *self {
            Step::FirstTurn => h.write(&[0]),
            Step::NewTurn => h.write(&[1]),
            Step::Act(s) => h.write(&[2, s as u8]),
            Step::Fainted => h.write(&[3]),
            Step::Replace(s) => h.write(&[4, s as u8]),
            Step::GainExp {
                party,
                exp,
                sent,
                defeated,
            } => {
                h.write(&[5, party, sent]);
                h.write_u32(exp);
                h.write_u16(defeated.0);
            }
            Step::AddExp { party, rest } => {
                h.write(&[6, party]);
                h.write_u32(rest);
            }
            Step::Learn { party, level, next } => h.write(&[7, party, level, next]),
            Step::EndTurn => h.write(&[8]),
            Step::SwitchOut { side, party } => h.write(&[9, side as u8, party]),
            Step::Megas => h.write(&[10]),
        }
    }
}

/// A running battle (singles). See the module docs for the flow.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Battle {
    pub(crate) setup: BattleSetup,
    /// `[Player, Opponent]`, kept in sync with the battlers.
    pub(crate) parties: [Party; 2],
    /// `[Player, Opponent]`.
    pub(crate) battlers: [Battler; 2],
    pub(crate) need: Need,
    pub(crate) steps: VecDeque<Step>,
    /// This turn's actions, `None` once done or cancelled.
    pub(crate) actions: [Option<Action>; 2],
    /// `gBattleStruct->runTries`.
    pub(crate) run_tries: u8,
    /// `gBattleResults.battleTurnCounter`.
    pub(crate) turn: u8,
    /// `gSentPokesToOpponent`: player party slots that faced the current opponent.
    pub(crate) sent_in: u8,
    /// `gBattleStruct->givenExpMons`: opponent party slots whose exp was given.
    pub(crate) exp_given: u8,
    /// `gRandomTurnNumber` (Quick Claw; drawn as Gen3, unused without items).
    pub(crate) random_turn: u16,
    /// The trainer's Gen3 `AI_SCRIPT_*` bits (0 in wild battles), for `ai`.
    pub(crate) trainer_ai: u32,
    /// `gBattlerByTurnOrder` of this turn (end-of-turn effects run in this order).
    pub(crate) order: [Side; 2],
    /// `HITMARKER_SYNCHRONIZE_EFFECT` + `synchronizeMoveEffect`: the status the current action
    /// inflicted, for Synchronize (cleared when the action ends, `HandleAction_ActionFinished`).
    pub(crate) sync: Option<StatusKind>,
    /// A mon caught with a full party (no PC yet): dropped, kept here for the caller to log.
    pub(crate) caught_without_room: Option<Pokemon>,
    /// `gMoveResultFlags & MOVE_RESULT_NO_EFFECT` of the move being used: set by
    /// [`Battle::missed`] (miss or type immunity) and [`Battle::fail`].
    pub(crate) no_effect: bool,
    /// The last hit of a multi-hit move, for its final `moveend`'s King's Rock (set by the
    /// move, taken by `after_move`: always `None` between steps, so not hashed).
    pub(crate) multi_last: Option<HitResult>,
    /// `GetTrainerMoneyToGive` for a trainer battle (`None` in a wild battle or when the game
    /// gives no money table entry): paid by the caller on a win.
    pub(crate) prize: Option<u32>,
    /// `gLeveledUpInBattle`: player party slots that gained a level (evolution candidates).
    pub(crate) leveled_up: u8,
    /// The Silice engine's state (hashed only when set, see [`BattleExt`]).
    pub(crate) x: BattleExt,
}

/// Gen3 `GetTrainerMoneyToGive` (`moneyMultiplier` 1: no Amulet Coin without held items):
/// `4 × the last party mon's level × gTrainerMoneyTable[class]`, doubled for a double battle.
pub fn trainer_money(last_level: u8, table: u8, double: bool) -> u32 {
    4 * last_level as u32 * table as u32 * if double { 2 } else { 1 }
}

/// A message about `active` with the move context of `attacker` → `target`, no buffers, no
/// wait. `slots` is filled when the message is stamped ([`Battle::stamp`]).
pub(crate) fn msg(id: Str, attacker: Side, target: Side, active: Side) -> Msg {
    Msg {
        id,
        attacker,
        target,
        active,
        mov: None,
        buffs: [Buff::None; 3],
        wait: MsgWait::Frames(0),
        slots: [0; 2],
    }
}

/// `waitmessage B_WAIT_TIME_LONG`.
pub(crate) const LONG: MsgWait = MsgWait::Frames(calc::WAIT_LONG);

/// A message about one side (attacker = target = active).
fn about(id: Str, side: Side) -> Msg {
    msg(id, side, side, side)
}

/// Gen3 `SetUpBattleVarsAndBirchZigzagoon`'s `BattleAI_HandleItemUseBeforeAISetup(0xF)`: the
/// four `simulatedRNG` rolls of `BattleAI_SetupAIData` (the AI data is set up again before
/// each choice, so only the draws matter). [`Battle::new`] draws them first.
pub(crate) fn setup_ai_draws(rng: &mut Rng) {
    for _ in 0..4 {
        rng.random();
    }
}

impl Battle {
    /// A battle between `player` (cloned; the first mon with HP leads) and the setup's
    /// opponents. Draws Gen3 `BattleStartClearSetData`'s `gRandomTurnNumber`. Both parties must
    /// have a mon.
    pub fn new(game: &impl GameData, setup: BattleSetup, player: &Party, rng: &mut Rng) -> Self {
        let t = game.battle();
        let parties = [player.clone(), setup.opponents.clone()];
        let lead = |p: &Party| p.first_usable().unwrap_or(0);
        let battler = |p: &Party| {
            let i = lead(p);
            Battler::new(t, p.get(i as usize).expect("a battle needs a mon"), i)
        };
        let battlers = [battler(&parties[0]), battler(&parties[1])];
        let trainer_ai = match setup.kind {
            BattleKind::Trainer { trainer, .. } => game.trainer(trainer).map_or(0, |t| t.ai),
            BattleKind::Wild { .. } => 0,
        };
        let prize = match setup.kind {
            BattleKind::Trainer { trainer, .. } => {
                let last = setup.opponents.iter().last().map_or(0, |m| m.level);
                let double = game.trainer(trainer).is_some_and(|t| t.double);
                game.trainer_money(trainer)
                    .map(|m| trainer_money(last, m, double))
            }
            BattleKind::Wild { .. } => None,
        };
        // SetUpBattleVarsAndBirchZigzagoon → BattleAI_HandleItemUseBeforeAISetup(0xF) →
        // BattleAI_SetupAIData: four `simulatedRNG` rolls, in every battle (battle_controllers.c,
        // battle_ai_script_commands.c). CB2_InitBattleInternal runs it before
        // CreateNPCTrainerParty / SetWildMonHeldItem, and BattleStartClearSetData's
        // gRandomTurnNumber comes later (CB2_HandleStartBattle).
        setup_ai_draws(rng);
        if !matches!(setup.kind, BattleKind::Trainer { .. }) {
            // SetWildMonHeldItem: `Random() % 100` for every wild battle (no held items in the
            // engine yet, so only the draw is kept).
            rng.random();
        }
        let random_turn = rng.random();
        let x = BattleExt {
            holds: [&parties[0], &parties[1]].map(|p| {
                core::array::from_fn(|i| {
                    p.get(i)
                        .and_then(|m| m.held)
                        .and_then(|it| game.hold_effect(it))
                })
            }),
            ..BattleExt::default()
        };
        let mut b = Battle {
            sent_in: 1 << battlers[0].mon.party,
            setup,
            parties,
            battlers,
            need: Need::Busy,
            steps: VecDeque::new(),
            actions: [None; 2],
            run_tries: 0,
            turn: 0,
            exp_given: 0,
            random_turn,
            trainer_ai,
            order: [Side::Player, Side::Opponent],
            sync: None,
            caught_without_room: None,
            no_effect: false,
            multi_last: None,
            prize,
            leveled_up: 0,
            x,
        };
        b.find_forms(game, Side::Player, |_| false);
        b.find_forms(game, Side::Opponent, |_| false);
        for side in [Side::Player, Side::Opponent] {
            let party = b.battler(side).mon.party;
            b.battlers[side as usize] = b.battler_for(t, side, party);
        }
        b
    }

    /// Lets the player's mons change form when their `FormChange::enable_flag` is set
    /// (`is_set`, the World's flags; the Mega Ring). Call before [`Battle::start`]: a
    /// battle without it only allows the form changes without a flag.
    pub fn enable_player_forms(&mut self, game: &impl GameData, is_set: impl Fn(FlagId) -> bool) {
        self.find_forms(game, Side::Player, is_set);
    }

    /// The `Battler` for party slot `i` of `side`: `CopyMonToBattleMon`, with its battle form
    /// (stats, types and ability of the form species) and its hold effect.
    pub(crate) fn battler_for(&self, t: &BattleTables, side: Side, i: u8) -> Battler {
        let p = *self.parties[side as usize]
            .get(i as usize)
            .expect("a battler's party slot");
        let mut b = match self.form_of(side, i) {
            Some(form) => {
                let mut q = Pokemon { species: form, ..p };
                q.recalc_stats(t);
                // The form keeps the mon's HP and max HP (Gen6: a form change never changes
                // the HP stat).
                q.stats[0] = p.stats[0];
                q.hp = p.hp;
                Battler::new(t, &q, i)
            }
            None => Battler::new(t, &p, i),
        };
        b.x.hold = self.x.holds[side as usize][i as usize];
        if b.ability == Ability::SpeedBoost {
            b.x.first_turn = 2;
        }
        b
    }

    /// The battle form of party slot `i` of `side`, once it changed.
    pub(crate) fn form_of(&self, side: Side, i: u8) -> Option<SpeciesId> {
        if self.x.formed[side as usize] & (1 << i) == 0 {
            return None;
        }
        self.x.forms[side as usize][i as usize].map(|f| f.form)
    }

    /// Pushes the intro events (wild or trainer) and runs to the first `Need::Action`.
    pub fn start(&mut self, game: &impl GameData, rng: &mut Rng, out: &mut Vec<BattleEvent>) {
        if self.need != Need::Busy || !self.steps.is_empty() {
            return;
        }
        let mut env = Env::new(game.battle(), rng, out);
        let (p, o) = (Side::Player, Side::Opponent);
        // BattleIntroPrintWildMonAttacked / PrintTrainerWantsToBattle (STRINGID_INTROMSG), then
        // STRINGID_INTROSENDOUT for the opponent (trainer) and the player.
        if self.is_trainer() {
            env.out.push(BattleEvent::TrainerAppear);
            env.out
                .push(BattleEvent::Msg(msg(Str::IntroTrainer, p, o, o)));
            self.send_out_events(&mut env, o, true);
        } else {
            env.out.push(BattleEvent::WildAppear);
            env.out.push(BattleEvent::Msg(msg(Str::Intro1, p, o, o)));
        }
        self.send_out_events(&mut env, p, true);
        self.steps.push_back(Step::FirstTurn);
        self.run(&mut env);
        self.stamp(&mut env);
    }

    /// What the core waits for.
    pub fn need(&self) -> Need {
        self.need
    }

    /// Answers the current `Need` and runs until the next one. A choice that does not answer
    /// the need (or names an empty move slot, a fainted or active mon) is ignored.
    pub fn choose(
        &mut self,
        game: &impl GameData,
        rng: &mut Rng,
        c: Choice,
        out: &mut Vec<BattleEvent>,
    ) {
        let mut env = Env::new(game.battle(), rng, out);
        self.answer(&mut env, c);
        self.stamp(&mut env);
    }

    fn answer(&mut self, env: &mut Env, c: Choice) {
        match (self.need, c) {
            (Need::Action, c) => {
                let Some(a) = self.player_action(env, c) else {
                    return;
                };
                self.actions[Side::Player as usize] = Some(a);
                self.need = Need::Busy;
                self.queue_turn(env);
            }
            (Need::Switch, Choice::Switch(i)) if self.can_switch_to(Side::Player, i) => {
                self.need = Need::Busy;
                self.switch_in(env, Side::Player, i);
                // BattleScript_FaintedMonSendOutNew ends with `cancelallactions`.
                self.actions = [None; 2];
            }
            (Need::LearnMove { party, mov }, Choice::LearnMove { forget }) => {
                self.need = Need::Busy;
                self.answer_learn(env, party, mov, forget);
            }
            _ => return,
        }
        self.run(env);
    }

    /// Sets `Msg::slots` (each side's battler party slot) on the messages pushed since the last
    /// stamp. Called before a battler changes ([`Battle::switch_in`]) and when `start` /
    /// `choose` return, so every message names the battlers out when it was emitted. E2 code
    /// that replaces a battler must go through `switch_in`.
    pub(crate) fn stamp(&self, env: &mut Env) {
        let slots = [
            self.battler(Side::Player).mon.party,
            self.battler(Side::Opponent).mon.party,
        ];
        for e in &mut env.out[env.stamped..] {
            if let BattleEvent::Msg(m) = e {
                m.slots = slots;
            }
        }
        env.stamped = env.out.len();
    }

    /// The battler on `side`.
    pub fn mon(&self, side: Side) -> &BattleMon {
        &self.battlers[side as usize].mon
    }

    /// The party of `side`. Player: written back to the World at the end.
    pub fn party(&self, side: Side) -> &Party {
        &self.parties[side as usize]
    }

    pub fn setup(&self) -> &BattleSetup {
        &self.setup
    }

    /// Feeds every piece of state into `h`.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        match self.setup.kind {
            BattleKind::Wild { first_battle } => h.write(&[0, first_battle as u8]),
            BattleKind::Trainer { trainer, defeat } => {
                h.write(&[1]);
                h.write_u16(trainer.0);
                h.write_u16(defeat.0);
            }
        }
        h.write(&[self.setup.environment as u8]);
        for p in &self.parties {
            p.hash(h);
        }
        for b in &self.battlers {
            b.hash(h);
        }
        match self.need {
            Need::Busy => h.write(&[0]),
            Need::Action => h.write(&[1]),
            Need::Switch => h.write(&[2]),
            Need::LearnMove { party, mov } => {
                h.write(&[3, party]);
                h.write_u16(mov.0);
            }
            Need::Ended(o) => h.write(&[4, o as u8]),
        }
        h.write_u16(self.steps.len() as u16);
        for s in &self.steps {
            s.hash(h);
        }
        for a in self.actions {
            h.write(&a.map_or([0xFF, 0], Action::code));
        }
        h.write(&[self.run_tries, self.turn, self.sent_in, self.exp_given]);
        h.write_u16(self.random_turn);
        h.write_u32(self.trainer_ai);
        h.write(&[
            self.order[0] as u8,
            self.sync.map_or(0xFF, |k| k as u8),
            self.caught_without_room.is_some() as u8,
        ]);
        if let Some(p) = &self.caught_without_room {
            p.hash(h);
        }
        h.write(&[self.leveled_up]);
        if self.x != BattleExt::default() {
            h.write(&[0xA9]);
            self.x.hash(h);
        }
    }

    /// The prize money of a trainer battle (Gen3 `getmoneyreward`), paid on a win; `None` in a
    /// wild battle or without a money table entry.
    pub fn prize(&self) -> Option<u32> {
        self.prize
    }

    /// Gen3 `gLeveledUpInBattle`: a bit per player party slot that gained a level.
    pub fn leveled_up(&self) -> u8 {
        self.leveled_up
    }

    /// The mon caught while the party was full (Gen3 sends it to the PC, which the engine does
    /// not have yet): it is dropped; the caller may log it.
    pub fn caught_without_room(&self) -> Option<&Pokemon> {
        self.caught_without_room.as_ref()
    }

    // ---- helpers shared with `moves`, `effects` and `ai` ----

    pub(crate) fn battler(&self, side: Side) -> &Battler {
        &self.battlers[side as usize]
    }

    pub(crate) fn battler_mut(&mut self, side: Side) -> &mut Battler {
        &mut self.battlers[side as usize]
    }

    pub(crate) fn is_trainer(&self) -> bool {
        matches!(self.setup.kind, BattleKind::Trainer { .. })
    }

    /// `BATTLE_TYPE_FIRST_BATTLE`.
    pub(crate) fn is_first_battle(&self) -> bool {
        matches!(self.setup.kind, BattleKind::Wild { first_battle: true })
    }

    /// Copies the battler's HP, status and PP into its party mon (Gen3 `SetMonData` after
    /// `datahpupdate`, `ppreduce`, status changes).
    pub(crate) fn write_back(&mut self, side: Side) {
        let b = self.battlers[side as usize];
        if let Some(p) = self.parties[side as usize].get_mut(b.mon.party as usize) {
            p.hp = b.mon.hp;
            p.status = b.mon.status;
            p.pp = b.mon.pp;
        }
    }

    /// Runs steps until one needs the player.
    fn run(&mut self, env: &mut Env) {
        while self.need == Need::Busy {
            let Some(step) = self.steps.pop_front() else {
                debug_assert!(false, "battle flow ran dry");
                return;
            };
            self.step(env, step);
            self.write_back(Side::Player);
            self.write_back(Side::Opponent);
        }
    }

    fn step(&mut self, env: &mut Env, step: Step) {
        match step {
            Step::FirstTurn => {
                // TryDoEventsBeforeFirstTurn: the battlers sorted by speed (a tie draws a roll),
                // then `ABILITYEFFECT_ON_SWITCH_IN` for each in that order (Primal Reversion
                // first, Gen6; the weather abilities), then Intimidate and Trace.
                let order = self.strike_order(env, true);
                for side in order {
                    self.primal_reversion(env, side);
                    super::effects::on_switch_in(self, env, side);
                }
                super::effects::first_turn(self, env);
                self.steps.push_front(Step::NewTurn);
            }
            Step::NewTurn => self.new_turn(env),
            Step::Act(side) => self.act(env, side),
            Step::Fainted => self.fainted(env),
            Step::Replace(side) => self.replace(env, side),
            Step::GainExp {
                party,
                exp,
                sent,
                defeated,
            } => self.gain_exp(env, party, exp, sent, defeated),
            Step::AddExp { party, rest } => self.exp_step(env, party, rest),
            Step::Learn { party, level, next } => self.learn(env, party, level, next),
            Step::SwitchOut { side, party } => {
                self.switch_out(env, side);
                self.switch_in(env, side, party);
            }
            Step::Megas => self.mega_evolutions(env),
            Step::EndTurn => {
                // HandleEndTurn_ContinueBattle + BattleTurnPassed.
                for b in &mut self.battlers {
                    b.flinched = false;
                }
                super::effects::end_turn(self, env);
                // TurnValuesCleanUp(FALSE): this turn's gProtectStructs, `isFirstTurn`.
                for b in &mut self.battlers {
                    b.v.unable = false;
                }
                self.x.mega_pending = [false; 2];
                self.turn = self.turn.saturating_add(1);
                self.steps.push_front(Step::NewTurn);
                self.steps.push_front(Step::Fainted);
            }
        }
    }

    /// `gRandomTurnNumber = Random()`, then `HandleTurnActionSelectionState`: the opponent
    /// chooses at once (its `Random()`s come before the player's turn runs).
    /// A battler locked into a move (`STATUS2_MULTIPLETURNS`, Bide) is not asked: neither the
    /// AI nor the player chooses, and the turn starts at once when the player is locked.
    fn new_turn(&mut self, env: &mut Env) {
        // TurnValuesCleanUp(FALSE) right before HandleTurnActionSelectionState: at the end of
        // TryDoEventsBeforeFirstTurn, and in BattleTurnPassed after HandleFaintedMonActions
        // (so a replacement sent in at the end of a turn counts that turn).
        for b in &mut self.battlers {
            b.x.first_turn = b.x.first_turn.saturating_sub(1);
        }
        self.random_turn = env.rng.random();
        let o = self.battler(Side::Opponent);
        let action = if let Some(a) = super::effects::locked_action(self, env.t, Side::Opponent) {
            a
        } else if let Some(i) = super::ai::switch(self, env.t, env.rng)
            .filter(|&i| self.can_switch_to(Side::Opponent, i))
        {
            // OpponentHandleChooseAction → AI_TrySwitchOrUseItem, before the move choice
            // (and before the all-moves-unusable check of B_ACTION_USE_MOVE).
            Action::Switch(i)
        } else if o.all_moves_unusable() {
            if self.is_trainer() && self.can_mega(Side::Opponent) {
                self.x.mega_pending[Side::Opponent as usize] = true;
            }
            Action::Struggle
        } else {
            if self.is_trainer()
                && self.trainer_ai & super::ai::AI_SCRIPT_CHECK_VIABILITY != 0
                && o.mon
                    .moves
                    .iter()
                    .flatten()
                    .any(|&m| env.t.move_info(m).effect == crate::mon::MoveEffect::Pursuit)
            {
                // AI_CV_Pursuit reads `isFirstTurn` and the foe's predicted switch, which the
                // engine does not model: the score may differ.
                env.out.push(BattleEvent::Report(BattleReport::Degraded(
                    "ai pursuit viability",
                )));
            }
            match super::ai::choose(self, env.t, env.rng) {
                Choice::Move(s) if (s as usize) < 4 && o.mon.pp[s as usize] > 0 => {
                    // A trainer Mega Evolves at the first opportunity (with a move).
                    if self.is_trainer() && self.can_mega(Side::Opponent) {
                        self.x.mega_pending[Side::Opponent as usize] = true;
                    }
                    Action::Move(s)
                }
                Choice::Switch(i) if self.can_switch_to(Side::Opponent, i) => Action::Switch(i),
                Choice::Run => Action::Run,
                _ => Action::Struggle,
            }
        };
        self.actions = [None, Some(action)];
        match super::effects::locked_action(self, env.t, Side::Player) {
            Some(a) => {
                self.actions[Side::Player as usize] = Some(a);
                self.queue_turn(env);
            }
            None => self.need = Need::Action,
        }
    }

    /// Both actions are chosen: the turn order (`SetActionsAndBattlersTurnOrder`).
    /// A pending Mega Evolution happens after the switches, items and running and before the
    /// first move action (expansion `TryDoGimmicksBeforeMoves`); when both actions are moves,
    /// the turn is then ordered with the new speeds.
    fn queue_turn(&mut self, env: &mut Env) {
        let moves = |a: Option<Action>| matches!(a, Some(Action::Move(_) | Action::Struggle));
        let pending = self.x.mega_pending != [false; 2];
        if pending && self.actions.iter().all(|&a| moves(a)) {
            self.mega_evolutions(env);
        }
        let [first, second] = self.turn_order(env);
        self.order = [first, second];
        self.steps.push_back(Step::Act(first));
        self.steps.push_back(Step::Fainted);
        if pending && !moves(self.actions[first as usize]) {
            self.steps.push_back(Step::Megas);
        }
        self.steps
            .extend([Step::Act(second), Step::Fainted, Step::EndTurn]);
    }

    /// The player's choice at `Need::Action`, or `None` when it does not start the turn
    /// (selection-time messages as Gen3's selection scripts).
    fn player_action(&mut self, env: &mut Env, c: Choice) -> Option<Action> {
        let p = Side::Player;
        let b = self.battler(p);
        match c {
            Choice::Mega(slot) => {
                let a = self.player_action(env, Choice::Move(slot))?;
                if matches!(a, Action::Move(_) | Action::Struggle) && self.can_mega(p) {
                    self.x.mega_pending[p as usize] = true;
                }
                Some(a)
            }
            Choice::Move(slot) => {
                let slot = slot as usize;
                if slot >= 4 || b.mon.moves[slot].is_none() {
                    return None;
                }
                if b.all_moves_unusable() {
                    // BattleScript_NoMovesLeft (STRINGID_PKMNHASNOMOVESLEFT), then Struggle.
                    env.out.push(BattleEvent::Msg(about(Str::Struggle, p)));
                    return Some(Action::Struggle);
                }
                if b.mon.pp[slot] == 0 {
                    // BattleScript_SelectingMoveWithNoPP.
                    env.out.push(BattleEvent::Msg(about(Str::NoPPLeft, p)));
                    return None;
                }
                Some(Action::Move(slot as u8))
            }
            // The party menu refuses a trapped battler (PARTY_ACTION_CANT_SWITCH /
            // PARTY_ACTION_ABILITY_PREVENTS: party menu texts, not battle strings).
            Choice::Switch(i) if self.can_switch_to(p, i) => {
                if super::effects::switch_block(self, p) {
                    return None;
                }
                Some(Action::Switch(i))
            }
            Choice::Run => {
                // Selection-time refusals: BattleScript_PrintCantRunFromTrainer, then
                // IsRunningFromBattleImpossible (BattleScript_PrintCantEscapeFromBattle).
                if self.is_trainer() {
                    env.out
                        .push(BattleEvent::Msg(about(Str::NoRunningFromTrainers, p)));
                    return None;
                }
                if let Some(r) = super::effects::run_block(self, p) {
                    env.out.push(BattleEvent::Msg(r.msg(p)));
                    return None;
                }
                Some(Action::Run)
            }
            // Refused in trainer battles for now. E2: Gen3 throws the ball anyway and plays
            // BattleScript_TrainerBallBlock ("The TRAINER blocked the BALL!"), using the turn.
            Choice::Ball if self.is_trainer() => None,
            Choice::Ball => Some(Action::Ball),
            Choice::Item { item, party } => items::choose(self, env, item, party),
            Choice::Switch(_) | Choice::LearnMove { .. } => None,
        }
    }

    /// Gen3 `SetActionsAndBattlersTurnOrder` (singles, not a link battle).
    fn turn_order(&mut self, env: &mut Env) -> [Side; 2] {
        let (p, o) = (Side::Player, Side::Opponent);
        let first_class = |a: Option<Action>| matches!(a, Some(Action::Switch(_) | Action::Ball));
        let [pa, oa] = self.actions;
        // Only the player's side (battlers 0 / 2) runs first. The opponent's B_ACTION_RUN (a
        // fleeing wild mon) is ordered by `GetWhoStrikesFirst` like a move of priority 0.
        if pa == Some(Action::Run) {
            return [p, o];
        }
        match (first_class(pa), first_class(oa)) {
            (true, _) => [p, o],
            (false, true) => [o, p],
            _ => self.strike_order(env, false),
        }
    }

    /// Gen3 `GetWhoStrikesFirst(player, opponent, ignoreChosenMoves)`: priority, then speed
    /// with Swift Swim / Chlorophyll, stages and paralysis, and Quick Claw (`gRandomTurnNumber`
    /// below `0xFFFF * param / 100`: `UINT_MAX`); a speed tie draws `Random() & 1` (only on a
    /// tie). Prankster adds 1 to a status move's priority.
    pub(crate) fn strike_order(&mut self, env: &mut Env, ignore_moves: bool) -> [Side; 2] {
        let random_turn = self.random_turn as u32;
        let speed = |s: Side| match self.battler(s).x.hold {
            Some(HoldEffect::QuickClaw(p)) if random_turn < 0xFFFF * p as u32 / 100 => u32::MAX,
            _ => self.full_speed(s),
        };
        let (s1, s2) = (speed(Side::Player), speed(Side::Opponent));
        let priority = |s: Side| -> i8 {
            let b = self.battler(s);
            let mov = match self.actions[s as usize] {
                _ if ignore_moves => None,
                Some(Action::Move(slot)) => b.mon.moves[slot as usize],
                Some(Action::Struggle) => Some(env.t.struggle),
                _ => None,
            };
            mov.map_or(0, |m| {
                let info = env.t.move_info(m);
                info.priority + super::effects::prankster_boost(b.ability, info) as i8
            })
        };
        let (p1, p2) = (priority(Side::Player), priority(Side::Opponent));
        let opponent_first = if (p1 != 0 || p2 != 0) && p1 != p2 {
            p1 < p2
        } else {
            (s1 == s2 && env.rng.random() & 1 != 0) || s1 < s2
        };
        if opponent_first {
            [Side::Opponent, Side::Player]
        } else {
            [Side::Player, Side::Opponent]
        }
    }

    /// A battler's speed with Swift Swim / Chlorophyll (`speedMultiplierBattler`), stages
    /// and paralysis.
    pub(crate) fn full_speed(&self, s: Side) -> u32 {
        let weather = self.weather();
        let b = self.battler(s);
        let doubled = match b.ability {
            Ability::SwiftSwim => weather.rain(),
            Ability::Chlorophyll => weather.sun(),
            _ => false,
        };
        calc::effective_speed_x(&b.fighter(), if doubled { 2 } else { 1 })
    }

    fn act(&mut self, env: &mut Env, side: Side) {
        let Some(action) = self.actions[side as usize].take() else {
            return;
        };
        if self.battler(side).mon.hp == 0 {
            return;
        }
        match action {
            Action::Move(_) | Action::Struggle => self.use_move(env, side, action),
            Action::Switch(i) => {
                // BattleScript_ActionSwitch (no STRINGID_RETURNMON string in `Str`): the foe's
                // Pursuit strikes first (`jumpifnopursuitswitchdmg`), then its exp.
                if super::effects::pursuit_on_switch(self, env, side) {
                    self.steps.push_front(Step::SwitchOut { side, party: i });
                    if let Some(exp) = self.exp_for_fainted_foe(env) {
                        self.steps.push_front(exp);
                    }
                    return;
                }
                self.switch_out(env, side);
                self.switch_in(env, side, i);
            }
            Action::Run => self.run_action(env, side),
            Action::Ball => {
                // HandleAction_UseItem: ClearFuryCutterDestinyBondGrudge.
                self.battler_mut(side).v.fury_cutter = 0;
                super::effects::ball(self, env)
            }
        }
    }

    /// Gen3 `HandleAction_Run` / `TryRunFromBattle` (singles, wild).
    fn run_action(&mut self, env: &mut Env, side: Side) {
        if side == Side::Opponent {
            // A wild mon fleeing (B_OUTCOME_MON_FLED).
            self.finish(env, BattleOutcome::MonFled);
            return;
        }
        let (me, foe) = (self.battler(side), self.battler(side.other()));
        let fled = if me.ability == Ability::RunAway {
            // FLEE_ABILITY: Gen3 prints STRINGID_PKMNFLEDUSING; `GotAwaySafely` here.
            true
        } else {
            let (speed, foe_speed) = (me.stats[3], foe.stats[3]);
            let ok = match calc::run_threshold(speed, foe_speed, self.run_tries) {
                None => true,
                Some(var) => var > (env.rng.random() & 0xFF) as u8,
            };
            self.run_tries = self.run_tries.saturating_add(1);
            ok
        };
        if fled {
            self.finish(env, BattleOutcome::Ran);
        } else {
            // ClearFuryCutterDestinyBondGrudge, then BattleScript_PrintFailedToRunString.
            self.battler_mut(side).v.fury_cutter = 0;
            env.out.push(BattleEvent::Msg(Msg {
                wait: LONG,
                ..about(Str::CantEscape, side)
            }));
        }
    }

    /// Whether `side` can send out party slot `i` (it can fight and is not out).
    pub(crate) fn can_switch_to(&self, side: Side, i: u8) -> bool {
        self.battler(side).mon.party != i
            && self.parties[side as usize]
                .get(i as usize)
                .is_some_and(|p| p.hp > 0)
    }

    /// The send-out message and event of the battler now on `side` (STRINGID_INTROSENDOUT /
    /// STRINGID_SWITCHINMON); at the intro a trainer slides out as it throws the ball.
    fn send_out_events(&mut self, env: &mut Env, side: Side, intro: bool) {
        let party = self.battler(side).mon.party;
        // B_BUFF1 = PREPARE_MON_NICK_BUFFER (sText_GoPkmn2, sText_Trainer1SentOutPkmn2): the
        // party slot, so the text names it even when expanded after later events.
        let buffs = [Buff::MonNick { side, party }, Buff::None, Buff::None];
        let id = match side {
            Side::Player => Str::GoPkmn,
            Side::Opponent => Str::PkmnSentOut,
        };
        env.out.push(BattleEvent::Msg(Msg {
            buffs,
            ..about(id, side)
        }));
        if intro && side == Side::Opponent && self.is_trainer() {
            env.out.push(BattleEvent::TrainerSlideOut);
        }
        env.out.push(BattleEvent::SendOut { side, party });
    }

    /// Sends out party slot `i` on `side` (`switchindataupdate`, STRINGID_SWITCHINMON,
    /// `switchinanim`, `switchineffects`).
    pub(crate) fn switch_in(&mut self, env: &mut Env, side: Side, i: u8) {
        self.stamp(env);
        self.write_back(side);
        let left = self.battler(side).ability;
        self.battlers[side as usize] = self.battler_for(env.t, side, i);
        // A primal weather ends when its source leaves.
        super::effects::left_field(self, env, left);
        // SwitchInClearSetData: the foe's escape prevention and infatuation by this battler.
        self.clear_bonds(side);
        // `switchineffects`: a Truant mon sent out mid-battle starts with the counter set (the
        // end of the turn clears it before its next move).
        if self.battler(side).ability == Ability::Truant {
            self.battler_mut(side).v.truant = true;
        }
        let player_party = self.battler(Side::Player).mon.party;
        match side {
            // UpdateSentPokesToOpponentValue.
            Side::Player => self.sent_in |= 1 << i,
            // OpponentSwitchInResetSentPokesToOpponentValue.
            Side::Opponent => self.sent_in = 1 << player_party,
        }
        self.send_out_events(env, side, false);
        // Primal Reversion on entry (Gen6), then the switch-in abilities.
        self.primal_reversion(env, side);
        super::effects::on_switch_in(self, env, side);
        super::effects::switch_in(self, env, side);
    }

    /// Gen3 `HandleFaintedMonActions`: exp for a fainted opponent (once per party slot), then
    /// `BattleScript_HandleFaintedMon` for each fainted battler, player first.
    fn fainted(&mut self, env: &mut Env) {
        let mut next: Vec<Step> = Vec::new();
        if let Some(exp) = self.exp_for_fainted_foe(env) {
            next.push(exp);
        }
        for side in [Side::Player, Side::Opponent] {
            if self.battler(side).mon.hp == 0 {
                next.push(Step::Replace(side));
            }
        }
        for s in next.into_iter().rev() {
            self.steps.push_front(s);
        }
    }

    /// `Cmd_getexp` state 1 for a fainted opponent whose exp was not given yet: the exp of
    /// each participant still standing, as a [`Step::GainExp`].
    fn exp_for_fainted_foe(&mut self, env: &mut Env) -> Option<Step> {
        let o = *self.battler(Side::Opponent);
        if o.mon.hp != 0 || self.exp_given & (1 << o.mon.party) != 0 {
            return None;
        }
        self.exp_given |= 1 << o.mon.party;
        let sent = self.sent_in;
        let participants = self.parties[Side::Player as usize]
            .iter()
            .enumerate()
            .filter(|(i, p)| p.hp > 0 && sent & (1 << i) != 0)
            .count() as u8;
        let base = env.t.species_info(o.mon.species).exp_yield;
        let exp = calc::exp_yield(base, o.mon.level, self.is_trainer(), participants);
        // HandleFaintedMonActions case 2.
        self.sent_in = 1 << self.battler(Side::Player).mon.party;
        Some(Step::GainExp {
            party: 0,
            exp,
            sent,
            defeated: o.mon.species,
        })
    }

    /// `SwitchInClearSetData` / `FaintClearSetData` for the battler leaving `side`: the other
    /// battler's escape prevention and infatuation (their source in singles) end.
    /// `BattleScript_ActionSwitch` from `switchoutabilities` (Natural Cure clears the status,
    /// silently) to `returnatktoball`.
    /// A battler knocked out on its way out (Pursuit) is not recalled (`returnatktoball`
    /// skips a fainted battler).
    fn switch_out(&mut self, env: &mut Env, side: Side) {
        if self.battler(side).mon.hp == 0 {
            return;
        }
        if self.battler(side).ability == Ability::NaturalCure {
            self.battler_mut(side).mon.status = crate::mon::Status::None;
            self.write_back(side);
        }
        env.out.push(BattleEvent::Recall { side });
    }

    /// `Cmd_pickup` (`BattleScript_PayDayMoneyAndPickUpItems`, after a win): for each player
    /// mon whose ability is Pickup and that holds no item (any HP), `Random() % 10`, and on 0
    /// `Random() % 100` for the item. The engine's mons hold nothing and get no item: reported.
    fn pickup(&mut self, env: &mut Env) {
        for m in self.parties[Side::Player as usize].iter() {
            if m.ability(env.t) != Ability::Pickup || !env.rng.random().is_multiple_of(10) {
                continue;
            }
            env.rng.random();
            env.out
                .push(BattleEvent::Report(BattleReport::Degraded("pickup item")));
        }
    }

    pub(crate) fn clear_bonds(&mut self, side: Side) {
        let v = &mut self.battler_mut(side.other()).v;
        v.escape_prevented = false;
        v.infatuated = false;
    }

    /// `checkteamslost`, then the replacement of a fainted battler.
    fn replace(&mut self, env: &mut Env, side: Side) {
        let lost = self.parties[Side::Player as usize].all_fainted();
        let won = self.parties[Side::Opponent as usize].all_fainted();
        let outcome = match (lost, won) {
            (true, true) => Some(BattleOutcome::Drew),
            (true, false) => Some(BattleOutcome::Lost),
            (false, true) => Some(BattleOutcome::Won),
            (false, false) => None,
        };
        if let Some(o) = outcome {
            self.finish(env, o);
            return;
        }
        if self.battler(side).mon.hp != 0 {
            return;
        }
        match side {
            // Simplified: no "Use next Pokémon?" box nor party screen; the UI picks.
            Side::Player => self.need = Need::Switch,
            Side::Opponent => {
                // Simplified GetMostSuitableMonToSwitchInto: the next usable mon in party order.
                if let Some(i) = self.parties[side as usize].first_usable() {
                    self.switch_in(env, side, i);
                    self.actions = [None; 2];
                }
            }
        }
    }

    /// `Cmd_getexp` state 2 for party slot `party`: the message and EVs, then the steps that
    /// give its exp, then the next slot.
    fn gain_exp(&mut self, env: &mut Env, party: u8, exp: u32, sent: u8, defeated: SpeciesId) {
        let player = &self.parties[Side::Player as usize];
        if party as usize >= PARTY_SIZE || party as usize >= player.len() {
            return;
        }
        self.steps.push_front(Step::GainExp {
            party: party + 1,
            exp,
            sent,
            defeated,
        });
        let p = player.get(party as usize).copied().expect("party slot");
        if sent & (1 << party) == 0 || p.level >= 100 || p.hp == 0 {
            return;
        }
        let side = Side::Player;
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [Buff::Mon { side, party }, Buff::None, Buff::Number(exp)],
            ..msg(Str::PkmnGainedExp, side, Side::Opponent, side)
        }));
        if let Some(p) = self.parties[side as usize].get_mut(party as usize) {
            p.gain_evs(env.t, defeated);
        }
        self.steps.push_front(Step::AddExp { party, rest: exp });
    }

    /// `Cmd_getexp` states 3–5: one level's worth of exp, the level-up and its moves.
    fn exp_step(&mut self, env: &mut Env, party: u8, rest: u32) {
        let side = Side::Player;
        let Some(p) = self.parties[side as usize].get_mut(party as usize) else {
            return;
        };
        let (from, mut rest) = (p.exp, rest);
        let leveled = p.add_exp_step(env.t, &mut rest);
        let p = *p;
        env.out.push(BattleEvent::Exp {
            party,
            from,
            to: p.exp,
        });
        if !leveled {
            return;
        }
        self.leveled_up |= 1 << party;
        // Gen3 `AdjustFriendship(.., FRIENDSHIP_EVENT_GROW_LEVEL)` on the level-up (Task E).
        let hold = self.x.holds[side as usize][party as usize];
        if let Some(m) = self.parties[side as usize].get_mut(party as usize) {
            use crate::world::friendship::{FriendshipEvent, on_event};
            on_event(m, FriendshipEvent::GrowLevel, hold, env.t, env.rng);
        }
        // Update the battle mon after the level-up (when it is out and standing).
        let b = &mut self.battlers[side as usize];
        if b.mon.party == party && b.mon.hp > 0 {
            b.mon.level = p.level;
            b.mon.hp = p.hp;
            b.mon.max_hp = p.stats[0];
            b.stats = p.stats;
            if self.form_of(side, party).is_some() {
                // A battle form's stats come from the form's base stats.
                let f = self.battler_for(env.t, side, party);
                self.battlers[side as usize].stats = f.stats;
            }
        }
        env.out.push(BattleEvent::LevelUp {
            party,
            level: p.level,
        });
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [
                Buff::Mon { side, party },
                Buff::Number(p.level as u32),
                Buff::None,
            ],
            ..about(Str::PkmnGrewToLv, side)
        }));
        if rest > 0 {
            self.steps.push_front(Step::AddExp { party, rest });
        }
        self.steps.push_front(Step::Learn {
            party,
            level: p.level,
            next: 0,
        });
    }

    /// `handlelearnnewmove` over the learnset moves of `level`; a full moveset asks the player.
    fn learn(&mut self, env: &mut Env, party: u8, level: u8, next: u8) {
        let side = Side::Player;
        let Some(p) = self.parties[side as usize].get(party as usize) else {
            return;
        };
        let moves: Vec<MoveId> = p.moves_at_level(env.t, level).collect();
        for (i, &m) in moves.iter().enumerate().skip(next as usize) {
            let p = self.parties[side as usize]
                .get_mut(party as usize)
                .expect("party slot");
            match p.try_learn(env.t, m) {
                Learn::Learned(slot) => {
                    self.sync_move(env.t, party, slot);
                    self.learned_msg(env, party, m);
                }
                Learn::AlreadyKnown => {}
                Learn::Full => {
                    // BattleScript_AskToLearnMove.
                    for id in [
                        Str::TryToLearnMove1,
                        Str::TryToLearnMove2,
                        Str::TryToLearnMove3,
                    ] {
                        env.out.push(BattleEvent::Msg(self.learn_msg(id, party, m)));
                    }
                    self.steps.push_front(Step::Learn {
                        party,
                        level,
                        next: i as u8 + 1,
                    });
                    self.need = Need::LearnMove { party, mov: m };
                    return;
                }
            }
        }
    }

    /// The player's answer to `Need::LearnMove`.
    fn answer_learn(&mut self, env: &mut Env, party: u8, mov: MoveId, forget: Option<u8>) {
        let side = Side::Player;
        let p = self.parties[side as usize]
            .get_mut(party as usize)
            .expect("party slot");
        match forget.filter(|&s| s < 4) {
            Some(slot) => {
                // BattleScript_ForgotAndLearnedNewMove (123POOF / ANDELLIPSIS have no `Str`).
                let old = p.moves[slot as usize];
                p.replace_move(env.t, slot, mov);
                if let Some(old) = old {
                    env.out.push(BattleEvent::Msg(self.learn_msg(
                        Str::PkmnForgotMove,
                        party,
                        old,
                    )));
                }
                self.sync_move(env.t, party, slot);
                self.learned_msg(env, party, mov);
            }
            None => {
                env.out.push(BattleEvent::Msg(self.learn_msg(
                    Str::DidNotLearnMove,
                    party,
                    mov,
                )));
            }
        }
    }

    fn learn_msg(&self, id: Str, party: u8, m: MoveId) -> Msg {
        let side = Side::Player;
        Msg {
            buffs: [Buff::Mon { side, party }, Buff::Move(m), Buff::None],
            ..about(id, side)
        }
    }

    /// BattleScript_LearnedNewMove.
    fn learned_msg(&mut self, env: &mut Env, party: u8, m: MoveId) {
        env.out.push(BattleEvent::Msg(Msg {
            wait: LONG,
            ..self.learn_msg(Str::PkmnLearnedMove, party, m)
        }));
    }

    /// Copies a learned move into the battle mon when that party mon is out
    /// (`GiveMoveToBattleMon`, `SetBattleMonMoveSlot`).
    fn sync_move(&mut self, t: &BattleTables, party: u8, slot: u8) {
        let p = *self.parties[0].get(party as usize).expect("party slot");
        let b = &mut self.battlers[0];
        if b.mon.party == party {
            let s = slot as usize;
            b.mon.moves[s] = p.moves[s];
            b.mon.pp[s] = p.pp[s];
            b.mon.max_pp[s] = p.max_pp(t, slot);
        }
    }

    /// Ends the battle: the closing messages of `HandleEndTurn_*`, then `End`.
    pub(crate) fn finish(&mut self, env: &mut Env, outcome: BattleOutcome) {
        self.steps.clear();
        self.actions = [None; 2];
        let (p, o) = (Side::Player, Side::Opponent);
        let button = |m: Msg| {
            BattleEvent::Msg(Msg {
                wait: MsgWait::Button,
                ..m
            })
        };
        match outcome {
            BattleOutcome::Won if self.is_trainer() => {
                // BattleScript_LocalTrainerBattleWon: STRINGID_PLAYERDEFEATEDTRAINER1, then
                // `trainerslidein` and STRINGID_TRAINER1LOSETEXT: `TrainerDefeated` is the
                // slide-in, after which presentation prints the `BattleKind` defeat text and
                // waits for a button (no `Str` carries a `TextId`).
                env.out
                    .push(BattleEvent::Msg(msg(Str::PlayerDefeatedTrainer, p, o, o)));
                env.out.push(BattleEvent::TrainerDefeated);
                // BattleScript_LocalBattleWonReward: `getmoneyreward` (the caller adds the
                // money), STRINGID_PLAYERGOTMONEY, `waitmessage B_WAIT_TIME_LONG`.
                match self.prize {
                    Some(money) => env.out.push(BattleEvent::Msg(Msg {
                        buffs: [Buff::Number(money), Buff::None, Buff::None],
                        wait: LONG,
                        ..about(Str::PlayerGotMoney, p)
                    })),
                    None => env
                        .out
                        .push(BattleEvent::Report(BattleReport::Degraded("trainer money"))),
                }
            }
            BattleOutcome::Lost | BattleOutcome::Drew => {
                // HandleEndTurn_BattleLost (also for B_OUTCOME_DREW: `IsPlayerDefeated`) →
                // BattleScript_LocalBattleLostPrintWhiteOut.
                env.out.push(BattleEvent::Msg(Msg {
                    wait: LONG,
                    ..about(Str::PlayerWhiteout, p)
                }));
                env.out.push(button(about(Str::PlayerWhiteout2, p)));
            }
            BattleOutcome::Ran => env.out.push(button(about(Str::GotAwaySafely, p))),
            BattleOutcome::MonFled => {
                let party = self.battler(o).mon.party;
                // HandleEndTurn_MonFled (battle_main.c): PREPARE_MON_NICK_BUFFER is
                // `B_BUFF_MON_NICK`, the nickname without the wild tag
                // (`ExpandBattleTextBuffPlaceholders`); STRINGID_WILDPKMNFLED carries the tag
                // itself after {B_BUFF1}.
                env.out.push(button(Msg {
                    buffs: [Buff::MonNick { side: o, party }, Buff::None, Buff::None],
                    ..about(Str::WildPkmnFled, o)
                }));
            }
            // HandleEndTurn_FinishBattle: no closing message.
            BattleOutcome::Won
            | BattleOutcome::Caught
            | BattleOutcome::PlayerTeleported
            | BattleOutcome::MonTeleported => {}
        }
        if outcome == BattleOutcome::Won {
            self.pickup(env);
        }
        env.out.push(BattleEvent::End(outcome));
        self.need = Need::Ended(outcome);
    }
}
