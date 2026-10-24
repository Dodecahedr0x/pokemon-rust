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
    Double, MULTI_PARTY_SIZE, Msg, MsgWait, Need, Side, Str,
};
use crate::data::{FlagId, FormChange, GameData, ItemId, MoveId, SpeciesId};
use crate::hash::Fnv64;
use crate::mon::{
    Ability, BattleTables, HoldEffect, Learn, PARTY_SIZE, Party, Pokemon, StatusKind, Type,
};
use crate::rng::Rng;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::effects::{BattlerFx, FieldFx};

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
    /// Wave C's move-effect state (hashed only when set, see [`BattlerFx`]).
    pub fx: BattlerFx,
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
    /// `RESOURCE_FLAG_FLASH_FIRE`: its Fire moves are boosted (cleared by a switch).
    pub flash_fire: bool,
}

impl BattlerExt {
    fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.item.map_or(0xFFFF, |i| i.0));
        h.write(&self.hold.map_or([0xFF, 0], HoldEffect::code));
        h.write(&[self.first_turn]);
        if self.flash_fire {
            h.write(&[0xFF]);
        }
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
            fx: BattlerFx::default(),
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
            hold: self.x.hold.and_then(|h| h.for_species(self.mon.species)),
        }
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
        if self.fx.is_set() {
            h.write(&[0xAB]);
            self.fx.hash(h);
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
    /// `B_WEATHER_HAIL_TEMPORARY` (Hail, Wave C).
    Hail,
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
    /// `gBattleStruct->moneyMultiplier` 2: a player battler entered holding an Amulet Coin.
    pub double_prize: bool,
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
        if self.double_prize {
            h.write(&[0xDB]);
        }
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

/// The move slot of a move action.
pub(crate) fn action_slot(a: Action) -> Option<u8> {
    match a {
        Action::Move(s) => Some(s),
        _ => None,
    }
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
        /// With an Exp. Share or a Lucky Egg in the party: `gExpShareExp` (0 without an Exp.
        /// Share), and `exp` is the participants' share before the per-mon bonuses. `None`:
        /// `exp` already has the trainer bonus.
        share: Option<u32>,
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
    /// `HandleWishPerishSongOnTurnEnd` from a state (queued only when something is pending),
    /// after `HandleFaintedMonActions`.
    WishPerish(u8),
}

impl Step {
    fn hash(&self, h: &mut Fnv64) {
        match *self {
            Step::FirstTurn => h.write(&[0]),
            Step::NewTurn => h.write(&[1]),
            Step::Act(s) => h.write(&[2, s.code()]),
            Step::Fainted => h.write(&[3]),
            Step::Replace(s) => h.write(&[4, s.code()]),
            Step::GainExp {
                party,
                exp,
                sent,
                defeated,
                share,
            } => {
                h.write(&[5, party, sent]);
                h.write_u32(exp);
                h.write_u16(defeated.0);
                if let Some(share) = share {
                    h.write(&[0xE5]);
                    h.write_u32(share);
                }
            }
            Step::AddExp { party, rest } => {
                h.write(&[6, party]);
                h.write_u32(rest);
            }
            Step::Learn { party, level, next } => h.write(&[7, party, level, next]),
            Step::EndTurn => h.write(&[8]),
            Step::SwitchOut { side, party } => h.write(&[9, side.code(), party]),
            Step::Megas => h.write(&[10]),
            Step::WishPerish(i) => h.write(&[11, i]),
        }
    }
}

/// A running battle (singles or doubles). See the module docs for the flow.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Battle {
    pub(crate) setup: BattleSetup,
    /// `[Player, Opponent]` (index = [`Side::s`]), kept in sync with the battlers.
    pub(crate) parties: [Party; 2],
    /// By battler ([`Side::i`]): 2 in singles, 4 in doubles. A double battler without a mon
    /// to send (`gAbsentBattlerFlags`) keeps its last mon at 0 HP, `fainted`.
    pub(crate) battlers: Vec<Battler>,
    pub(crate) need: Need,
    pub(crate) steps: VecDeque<Step>,
    /// This turn's actions by battler, `None` once done or cancelled.
    pub(crate) actions: [Option<Action>; 4],
    /// `gBattleStruct->moveTarget`: each battler's chosen target (doubles; singles: unused).
    pub(crate) targets: [Side; 4],
    /// The player battler whose action ([`Need::Action`]) or replacement ([`Need::Switch`]) is
    /// asked (`Player` in singles).
    pub(crate) choosing: Side,
    /// `gAbsentBattlerFlags`: a bit per battler with no mon on the field (doubles).
    pub(crate) absent: u8,
    /// `gBattleStruct->runTries`.
    pub(crate) run_tries: u8,
    /// `gBattleResults.battleTurnCounter`.
    pub(crate) turn: u8,
    /// `gSentPokesToOpponent[flank]`: player party slots that faced the opponent battler on
    /// that flank (`[1]`: doubles only).
    pub(crate) sent_in: [u8; 2],
    /// `gBattleStruct->givenExpMons`: opponent party slots whose exp was given.
    pub(crate) exp_given: u8,
    /// `gRandomTurnNumber` (Quick Claw; drawn as Gen3, unused without items).
    pub(crate) random_turn: u16,
    /// The trainer's Gen3 `AI_SCRIPT_*` bits (0 in wild battles), for `ai`.
    pub(crate) trainer_ai: u32,
    /// `gBattlerByTurnOrder` of this turn (end-of-turn effects run in this order): the first
    /// [`Battle::count`] entries.
    pub(crate) order: [Side; 4],
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
    /// Wave C's side and field move-effect state (hashed only when set, see [`FieldFx`]).
    pub(crate) fx: FieldFx,
}

/// Gen3 `GetTrainerMoneyToGive` (`moneyMultiplier` 1: no Amulet Coin without held items):
/// `4 × the last party mon's level × gTrainerMoneyTable[class]`, doubled for a double battle.
pub fn trainer_money(last_level: u8, table: u8, double: bool) -> u32 {
    4 * last_level as u32 * table as u32 * if double { 2 } else { 1 }
}

/// The party slots battler `side` sends out, of a party of `len`: all of them, or in a multi
/// battle (the player's side) and a two-trainer battle (the opponent's) the left battler's
/// trainer `0..3` and the right one's `3..` (`MULTI_PARTY_SIZE`).
pub(crate) fn party_range(double: Option<&Double>, side: Side, len: u8) -> core::ops::Range<u8> {
    let halves = match double {
        Some(d) if side.is_player() => d.partner.is_some(),
        Some(d) => d.second.is_some(),
        None => false,
    };
    let half = MULTI_PARTY_SIZE.min(len);
    match (halves, side.i() < 2) {
        (false, _) => 0..len,
        (true, true) => 0..half,
        (true, false) => half..len,
    }
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
        slots: [0; 4],
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
        // The leads (`gBattlerPartyIndexes`): the first mons with HP of each battler's party
        // range; in doubles the right battler takes the next one (or, with none, is absent
        // with the left one's slot at 0 HP).
        let double = setup.double;
        let n = if double.is_some() { 4 } else { 2 };
        let mut taken = [0u8; 2];
        let mut absent = 0u8;
        let mut battlers: Vec<Battler> = Vec::with_capacity(n);
        for i in 0..n {
            let side = Side::at(i);
            let p = &parties[side.s()];
            let range = party_range(double.as_ref(), side, p.len() as u8);
            let free = |s: u8| taken[side.s()] & (1 << s) == 0;
            let usable = range
                .clone()
                .find(|&s| free(s) && p.get(s as usize).is_some_and(|m| m.hp > 0));
            let slot = match usable {
                Some(s) => s,
                // A left battler always has a mon (`a battle needs a mon`).
                None if i < 2 => p.first_usable().unwrap_or(0),
                // An absent battler (`gAbsentBattlerFlags`): a copy of its partner's mon,
                // never written back.
                None => {
                    absent |= 1 << i;
                    battlers[i - 2].mon.party
                }
            };
            taken[side.s()] |= 1 << slot;
            let mut b = Battler::new(t, p.get(slot as usize).expect("a battle needs a mon"), slot);
            if absent & (1 << i) != 0 {
                b.mon.hp = 0;
                b.fainted = true;
            }
            battlers.push(b);
        }
        let trainer_ai = match setup.kind {
            BattleKind::Trainer { trainer, .. } => {
                // BattleAI_SetupAIData: two trainers' flags are or'ed.
                let b = double
                    .and_then(|d| d.second)
                    .and_then(|(t, _)| game.trainer(t))
                    .map_or(0, |t| t.ai);
                game.trainer(trainer).map_or(0, |t| t.ai) | b
            }
            BattleKind::Wild { .. } => 0,
        };
        let prize = match setup.kind {
            BattleKind::Trainer { trainer, .. } => match double.and_then(|d| d.second) {
                // Cmd_getmoneyreward: `BATTLE_TYPE_TWO_OPPONENTS` adds trainer B's reward,
                // neither doubled (GetTrainerMoneyToGive), each from its party's last mon.
                Some((b, _)) => {
                    let last = |id| {
                        game.trainer(id)
                            .and_then(|t| t.party.last())
                            .map_or(0, |m| m.level)
                    };
                    let money = |id| {
                        game.trainer_money(id)
                            .map(|m| trainer_money(last(id), m, false))
                    };
                    money(trainer).map(|a| a + money(b).unwrap_or(0))
                }
                None => {
                    let last = setup.opponents.iter().last().map_or(0, |m| m.level);
                    let double =
                        double.is_some() || game.trainer(trainer).is_some_and(|t| t.double);
                    game.trainer_money(trainer)
                        .map(|m| trainer_money(last, m, double))
                }
            },
            BattleKind::Wild { .. } => None,
        };
        // SetUpBattleVarsAndBirchZigzagoon → BattleAI_HandleItemUseBeforeAISetup(0xF) →
        // BattleAI_SetupAIData: four `simulatedRNG` rolls, in every battle (battle_controllers.c,
        // battle_ai_script_commands.c). CB2_InitBattleInternal runs it before
        // CreateNPCTrainerParty / SetWildMonHeldItem, and BattleStartClearSetData's
        // gRandomTurnNumber comes later (CB2_HandleStartBattle).
        setup_ai_draws(rng);
        if double.is_some() {
            // BattleAI_SetupAIData in doubles: `gBattlerTarget = (Random() & BIT_FLANK) + ..`.
            rng.random();
        }
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
        // gSentPokesToOpponent: the player's left lead faces both opponent flanks at first
        // (OpponentSwitchInResetSentPokesToOpponentValue in the intro: the player's battlers).
        let mut sent = 1 << battlers[0].mon.party;
        if n == 4 && absent & 1 << 2 == 0 {
            sent |= 1 << battlers[2].mon.party;
        }
        let mut b = Battle {
            sent_in: [sent, if n == 4 { sent } else { 0 }],
            setup,
            parties,
            battlers,
            need: Need::Busy,
            steps: VecDeque::new(),
            actions: [None; 4],
            targets: [
                Side::Opponent,
                Side::Player,
                Side::OpponentRight,
                Side::PlayerRight,
            ],
            choosing: Side::Player,
            absent,
            run_tries: 0,
            turn: 0,
            exp_given: 0,
            random_turn,
            trainer_ai,
            order: [
                Side::Player,
                Side::Opponent,
                Side::PlayerRight,
                Side::OpponentRight,
            ],
            sync: None,
            caught_without_room: None,
            no_effect: false,
            multi_last: None,
            prize,
            leveled_up: 0,
            x,
            fx: FieldFx::default(),
        };
        b.find_forms(game, Side::Player, |_| false);
        b.find_forms(game, Side::Opponent, |_| false);
        for side in b.sides() {
            let party = b.battler(side).mon.party;
            let mut new = b.battler_for(t, side, party);
            if b.is_absent(side) {
                new.mon.hp = 0;
                new.fainted = true;
            }
            b.battlers[side.i()] = new;
        }
        b
    }

    /// The battlers' positions in battler order (2 in singles, 4 in doubles).
    pub fn sides(&self) -> impl Iterator<Item = Side> + use<> {
        (0..self.battlers.len()).map(Side::at)
    }

    /// `gBattlerByTurnOrder` of this turn (`gBattlersCount` entries).
    pub(crate) fn turn_sides(&self) -> Vec<Side> {
        self.order[..self.count()].to_vec()
    }

    /// Whether `side` acts last this turn (`gCurrentTurnActionNumber == gBattlersCount - 1`).
    pub(crate) fn acts_last(&self, side: Side) -> bool {
        self.order[self.count() - 1] == side
    }

    /// `gBattlersCount`.
    pub fn count(&self) -> usize {
        self.battlers.len()
    }

    /// `BATTLE_TYPE_DOUBLE`.
    pub fn is_double(&self) -> bool {
        self.battlers.len() == 4
    }

    /// `BATTLE_TYPE_INGAME_PARTNER`: `PlayerRight` is the partner's (its AI chooses).
    pub fn has_partner(&self) -> bool {
        self.setup.double.is_some_and(|d| d.partner.is_some())
    }

    /// `gAbsentBattlerFlags`: no mon on the field at `side`.
    pub fn is_absent(&self, side: Side) -> bool {
        self.absent & (1 << side.i()) != 0
    }

    /// Whether `side` is a battler of this battle with a mon on the field (any HP).
    pub fn is_present(&self, side: Side) -> bool {
        side.i() < self.battlers.len() && !self.is_absent(side)
    }

    /// The player battler whose action or replacement is asked ([`Need::Action`],
    /// [`Need::Switch`]): `Player` in singles.
    pub fn choosing(&self) -> Side {
        self.choosing
    }

    /// The party slots `side` may send out (`MULTI_PARTY_SIZE` halves in multi and two-trainer
    /// battles).
    pub fn party_range(&self, side: Side) -> core::ops::Range<u8> {
        party_range(
            self.setup.double.as_ref(),
            side,
            self.parties[side.s()].len() as u8,
        )
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
        let p = *self.parties[side.s()]
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
        b.x.hold = self.x.holds[side.s()][i as usize];
        // Knock Off: the item stays gone for the battle (`gWishFutureKnock.knockedOffMons`).
        if self.fx.knocked_off[side.s()] & (1 << i) != 0 {
            b.x.item = None;
            b.x.hold = None;
        }
        if b.ability == Ability::SpeedBoost || super::effects::reads_first_turn(t, &b) {
            b.x.first_turn = 2;
        }
        b
    }

    /// The battle form of party slot `i` of `side`, once it changed.
    pub(crate) fn form_of(&self, side: Side, i: u8) -> Option<SpeciesId> {
        if self.x.formed[side.s()] & (1 << i) == 0 {
            return None;
        }
        self.x.forms[side.s()][i as usize].map(|f| f.form)
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
        if self.is_double() {
            self.double_intro(&mut env);
        } else if self.is_trainer() {
            env.out.push(BattleEvent::TrainerAppear);
            env.out
                .push(BattleEvent::Msg(msg(Str::IntroTrainer, p, o, o)));
            self.send_out_events(&mut env, o, true);
        } else {
            env.out.push(BattleEvent::WildAppear);
            env.out.push(BattleEvent::Msg(msg(Str::Intro1, p, o, o)));
        }
        if !self.is_double() {
            self.send_out_events(&mut env, p, true);
        }
        self.steps.push_back(Step::FirstTurn);
        self.run(&mut env);
        self.stamp(&mut env);
    }

    /// The intro of a double battle (`PrintTrainerWantsToBattle`, `STRINGID_INTROSENDOUT` of
    /// both sides). One trainer: `sText_Trainer1SentOutTwoPkmn`. Two trainers: the intro
    /// names trainer A only and each trainer's mon gets its own `sText_Trainer1SentOutPkmn`
    /// (Gen3's `sText_TwoTrainersWantToBattle` / `sText_TwoTrainersSentPkmn` are not
    /// extracted: degraded). The player: `sText_GoTwoPkmn` (also with a partner, where Gen3
    /// prints `sText_InGamePartnerSentOutZGoN`: degraded).
    fn double_intro(&mut self, env: &mut Env) {
        let (p, o) = (Side::Player, Side::Opponent);
        let present = |b: &Battle, s: Side| b.is_present(s);
        env.out.push(BattleEvent::TrainerAppear);
        env.out
            .push(BattleEvent::Msg(msg(Str::IntroTrainer, p, o, o)));
        let two = self.setup.double.is_some_and(|d| d.second.is_some());
        let partner = self.has_partner();
        if two || partner {
            env.out.push(BattleEvent::Report(BattleReport::Degraded(
                "double battle intro text",
            )));
        }
        if two {
            self.send_out_events(env, o, true);
            if present(self, Side::OpponentRight) {
                self.send_out_events(env, Side::OpponentRight, false);
            }
        } else {
            let buffs = [Buff::None; 3];
            env.out.push(BattleEvent::Msg(Msg {
                buffs,
                ..msg(Str::TrainerSentOutTwo, p, o, o)
            }));
            env.out.push(BattleEvent::TrainerSlideOut);
            for s in [o, Side::OpponentRight] {
                if present(self, s) {
                    let party = self.battler(s).mon.party;
                    env.out.push(BattleEvent::SendOut { side: s, party });
                }
            }
        }
        if present(self, Side::PlayerRight) {
            env.out.push(BattleEvent::Msg(msg(Str::GoTwoPkmn, p, o, p)));
            for s in [p, Side::PlayerRight] {
                let party = self.battler(s).mon.party;
                env.out.push(BattleEvent::SendOut { side: s, party });
            }
        } else {
            self.send_out_events(env, p, true);
        }
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
        let me = self.choosing;
        match (self.need, c) {
            (Need::Action, Choice::Cancel) => {
                // B_ACTION_CANCEL_PARTNER: `PlayerRight`'s menu goes back to the left
                // battler's choice (a locked left battler keeps its action).
                if me == Side::PlayerRight && self.player_chooses(Side::Player) {
                    self.actions[0] = None;
                    self.x.mega_pending[0] = false;
                    self.choosing = Side::Player;
                }
                return;
            }
            (Need::Action, c) => {
                let Some(a) = self.player_action(env, c) else {
                    return;
                };
                self.actions[me.i()] = Some(a);
                self.targets[me.i()] = match c {
                    Choice::MoveAt { target, .. } if self.is_double() => target,
                    _ => self.default_target(env.t, me, action_slot(a)),
                };
                if let Some(next) = self.next_chooser() {
                    // HandleTurnActionSelectionState: the right battler is asked once the left
                    // one confirmed.
                    self.choosing = next;
                    return;
                }
                self.choosing = Side::Player;
                self.need = Need::Busy;
                self.queue_turn(env);
            }
            (Need::Switch, Choice::Switch(i)) if self.can_switch_to(me, i) => {
                self.need = Need::Busy;
                if self.battler(me).fx.baton {
                    // BattleScript_EffectBatonPass after `openpartyscreen`: the turn goes on.
                    self.switch_out(env, me);
                    self.switch_in(env, me, i);
                } else {
                    self.switch_in(env, me, i);
                    // BattleScript_FaintedMonSendOutNew ends with `cancelallactions`, except
                    // in doubles (`jumpifbattletype BATTLE_TYPE_DOUBLE`).
                    if !self.is_double() {
                        self.actions = [None; 4];
                    }
                }
                self.choosing = Side::Player;
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
        let slot = |s: Side| self.battlers.get(s.i()).map_or(0, |b| b.mon.party);
        let slots = Side::ALL.map(slot);
        for e in &mut env.out[env.stamped..] {
            if let BattleEvent::Msg(m) = e {
                m.slots = slots;
            }
        }
        env.stamped = env.out.len();
    }

    /// The battle mon on `side` (in singles a right battler is its left one).
    pub fn mon(&self, side: Side) -> &BattleMon {
        &self.battlers[side.i() % self.battlers.len()].mon
    }

    /// Whether the player (not a partner's AI) chooses `side`'s action this turn: a player
    /// battler on the field, standing.
    pub(crate) fn player_chooses(&self, side: Side) -> bool {
        side.is_player()
            && self.is_present(side)
            && self.battler(side).mon.hp > 0
            && !(side == Side::PlayerRight && self.has_partner())
    }

    /// The next player battler whose action is not chosen yet (`None`: the turn can start).
    fn next_chooser(&self) -> Option<Side> {
        [Side::Player, Side::PlayerRight]
            .into_iter()
            .find(|&s| self.player_chooses(s) && self.actions[s.i()].is_none())
    }

    /// The target of `side`'s action when none was chosen (`Choice::Move` in doubles, the
    /// AI's spread moves): Gen3 `GetMoveTarget(move, NO_TARGET_OVERRIDE)` without its roll,
    /// the facing foe (or its partner when absent), or the user for its own moves. Singles:
    /// the foe.
    pub fn default_target(&self, t: &BattleTables, side: Side, slot: Option<u8>) -> Side {
        let mov = slot.and_then(|s| {
            self.battler(side)
                .mon
                .moves
                .get(s as usize)
                .copied()
                .flatten()
        });
        let user = mov.is_some_and(|m| {
            matches!(
                t.move_info(m).target,
                crate::mon::MoveTarget::User | crate::mon::MoveTarget::UserOrSelected
            )
        });
        if user {
            return side;
        }
        let foe = side.other();
        if self.is_double() && (self.is_absent(foe) || self.battler(foe).mon.hp == 0) {
            foe.partner()
        } else {
            foe
        }
    }

    /// The target cursor of `side`'s move in `slot` (`HandleInputChooseMove`, doubles): `None`
    /// when there is none (singles, a move that does not select its target, or one other
    /// battler left standing), else the battlers in cursor order (`sTargetIdentities`:
    /// player left, player right, opponent right, opponent left), the first one the cursor
    /// starts on (the opposite battler, else the next).
    pub fn target_choices(&self, t: &BattleTables, side: Side, slot: u8) -> Option<Vec<Side>> {
        use crate::mon::MoveTarget as T;
        let mov = self
            .battler(side)
            .mon
            .moves
            .get(slot as usize)
            .copied()
            .flatten()?;
        if !self.is_double() || !matches!(t.move_info(mov).target, T::Selected | T::UserOrSelected)
        {
            return None;
        }
        let standing = |s: Side| s != side && self.is_present(s) && self.battler(s).mon.hp > 0;
        if self.sides().filter(|&s| standing(s)).count() <= 1 {
            return None;
        }
        let order = [
            Side::Player,
            Side::PlayerRight,
            Side::OpponentRight,
            Side::Opponent,
        ];
        let mut v: Vec<Side> = order.into_iter().filter(|&s| standing(s)).collect();
        let start = if side.is_player() {
            Side::Opponent
        } else {
            Side::Player
        };
        if let Some(k) = v.iter().position(|&s| s == start) {
            v.rotate_left(k);
        }
        Some(v)
    }

    /// Whether move slot `slot` of `side` can be chosen: a move with PP that Disable, Torment,
    /// Taunt or Imprison does not block (Gen3 `CheckMoveLimitations(MOVE_LIMITATIONS_ALL)`).
    /// Choosing another one is refused with a message and the menu reopens.
    pub fn move_selectable(&self, t: &BattleTables, side: Side, slot: u8) -> bool {
        slot < 4 && !super::effects::slot_limited(self, t, side, slot as usize)
    }

    /// The party of `side`. Player: written back to the World at the end.
    pub fn party(&self, side: Side) -> &Party {
        &self.parties[side.s()]
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
        let n = self.count();
        for a in &self.actions[..n] {
            h.write(&a.map_or([0xFF, 0], Action::code));
        }
        h.write(&[self.run_tries, self.turn, self.sent_in[0], self.exp_given]);
        h.write_u16(self.random_turn);
        h.write_u32(self.trainer_ai);
        h.write(&[
            self.order[0].code(),
            self.sync.map_or(0xFF, |k| k as u8),
            self.caught_without_room.is_some() as u8,
        ]);
        if self.is_double() {
            // Doubles only, so singles hash as before.
            h.write(&[0xDB, self.sent_in[1], self.choosing.code(), self.absent]);
            for s in &self.order[1..] {
                h.write(&[s.code()]);
            }
            for s in self.targets {
                h.write(&[s.code()]);
            }
            if let Some(d) = self.setup.double {
                h.write_u16(d.second.map_or(0xFFFF, |(t, _)| t.0));
                h.write_u16(d.second.map_or(0xFFFF, |(_, x)| x.0));
                h.write_u16(d.partner.map_or(0xFFFF, |t| t.0));
            }
        }
        if let Some(p) = &self.caught_without_room {
            p.hash(h);
        }
        h.write(&[self.leveled_up]);
        if self.x != BattleExt::default() {
            h.write(&[0xA9]);
            self.x.hash(h);
        }
        if self.fx.is_set() {
            h.write(&[0xAC]);
            self.fx.hash(h);
        }
    }

    /// The prize money of a trainer battle (Gen3 `getmoneyreward`), paid on a win; `None` in a
    /// wild battle or without a money table entry.
    /// `ABILITY_ON_FIELD2` for Plus / Minus: `side` has one and a battler with HP (either side)
    /// has the other.
    pub(crate) fn plus_minus(&self, side: Side) -> bool {
        let partner = match self.battler(side).ability {
            Ability::Plus => Ability::Minus,
            Ability::Minus => Ability::Plus,
            _ => return false,
        };
        self.battlers
            .iter()
            .any(|b| b.ability == partner && b.mon.hp > 0)
    }

    /// `gPaydayMoney` x `moneyMultiplier` (`Cmd_givepaydaymoney`), paid on a win.
    pub fn pay_day(&self) -> Option<u32> {
        let m = self.fx.pay_day as u32;
        (m != 0).then(|| if self.x.double_prize { m * 2 } else { m })
    }

    pub fn prize(&self) -> Option<u32> {
        self.prize
            .map(|m| if self.x.double_prize { m * 2 } else { m })
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
        &self.battlers[side.i()]
    }

    pub(crate) fn battler_mut(&mut self, side: Side) -> &mut Battler {
        &mut self.battlers[side.i()]
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
        if self.is_absent(side) {
            return;
        }
        let b = self.battlers[side.i()];
        if let Some(p) = self.parties[side.s()].get_mut(b.mon.party as usize) {
            p.hp = b.mon.hp;
            p.status = b.mon.status;
            // MOVE_IS_PERMANENT: not a transformed battler's PP, nor a Mimic'd slot's.
            for i in 0..4 {
                if !b.fx.transformed && b.fx.mimicked & (1 << i) == 0 {
                    p.pp[i] = b.mon.pp[i];
                }
            }
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
            for side in self.sides() {
                self.write_back(side);
            }
        }
    }

    fn step(&mut self, env: &mut Env, step: Step) {
        match step {
            Step::FirstTurn => {
                // TryDoEventsBeforeFirstTurn: the battlers sorted by speed (a tie draws a roll),
                // then `ABILITYEFFECT_ON_SWITCH_IN` for each in that order (Primal Reversion
                // first, Gen6; the weather abilities), then Intimidate and Trace.
                let order = self.strike_order(env, true);
                let order: Vec<Side> = order.into_iter().filter(|&s| !self.is_absent(s)).collect();
                for &side in &order {
                    self.primal_reversion(env, side);
                    super::effects::on_switch_in(self, env, side);
                }
                super::effects::first_turn(self, env);
                for &side in &order {
                    super::effects::items_on_switch_in(self, env, side);
                }
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
                share,
            } => self.gain_exp(env, party, (exp, share), sent, defeated),
            Step::AddExp { party, rest } => self.exp_step(env, party, rest),
            Step::Learn { party, level, next } => self.learn(env, party, level, next),
            Step::SwitchOut { side, party } => {
                self.switch_out(env, side);
                self.switch_in(env, side, party);
            }
            Step::Megas => self.mega_evolutions(env),
            Step::WishPerish(state) => {
                // Each future attack or perish count, then its faints (`BattleTurnPassed`).
                if let Some(next) = super::effects::wish_perish(self, env, state) {
                    self.steps.push_front(Step::WishPerish(next));
                    self.steps.push_front(Step::Fainted);
                }
            }
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
                if super::effects::wish_perish_pending(self) {
                    self.steps.push_front(Step::WishPerish(0));
                }
                self.steps.push_front(Step::Fainted);
            }
        }
    }

    /// `gRandomTurnNumber = Random()`, then `HandleTurnActionSelectionState`: the AI battlers
    /// choose at once (their `Random()`s come before the player's turn runs), in battler order
    /// (doubles: the right opponent once the left one confirmed; with a partner, Gen3 asks
    /// battlers 1 to 3 at once and runs their action choices before their move choices: here
    /// each battler's action and move in battler order, a documented deviation).
    /// A battler locked into a move (`STATUS2_MULTIPLETURNS`, Bide) is not asked: neither the
    /// AI nor the player chooses, and the turn starts at once when the player's are locked.
    fn new_turn(&mut self, env: &mut Env) {
        // TurnValuesCleanUp(FALSE) right before HandleTurnActionSelectionState: at the end of
        // TryDoEventsBeforeFirstTurn, and in BattleTurnPassed after HandleFaintedMonActions
        // (so a replacement sent in at the end of a turn counts that turn).
        for b in &mut self.battlers {
            b.x.first_turn = b.x.first_turn.saturating_sub(1);
        }
        self.random_turn = env.rng.random();
        self.actions = [None; 4];
        for side in self.sides() {
            let ai = !side.is_player() || (side == Side::PlayerRight && self.has_partner());
            if !ai || !self.is_present(side) || self.battler(side).mon.hp == 0 {
                continue;
            }
            let action = self.ai_action(env, side);
            self.actions[side.i()] = Some(action);
        }
        for side in [Side::Player, Side::PlayerRight] {
            if self.player_chooses(side)
                && let Some(a) = super::effects::locked_action(self, env.t, side)
            {
                self.actions[side.i()] = Some(a);
                self.targets[side.i()] = self.locked_target(env.t, side, a);
            }
        }
        match self.next_chooser() {
            None => self.queue_turn(env),
            Some(s) => {
                self.choosing = s;
                self.need = Need::Action;
            }
        }
    }

    /// The target a locked battler keeps (`gBattleStruct->moveTarget` from its last choice):
    /// its chosen target when it is still on the field, else the default.
    fn locked_target(&self, t: &BattleTables, side: Side, a: Action) -> Side {
        let kept = self.targets[side.i()];
        if self.is_double() && self.is_present(kept) && self.battler(kept).mon.hp > 0 {
            kept
        } else {
            self.default_target(t, side, action_slot(a))
        }
    }

    /// An AI battler's action (`OpponentHandleChooseAction` / `PlayerPartnerHandleChooseAction`
    /// then `..ChooseMove`): a locked move, the switch AI (`AI_TrySwitchOrUseItem`), Struggle,
    /// the encored move, or the move AI with its target.
    fn ai_action(&mut self, env: &mut Env, side: Side) -> Action {
        if let Some(a) = super::effects::locked_action(self, env.t, side) {
            self.targets[side.i()] = self.locked_target(env.t, side, a);
            return a;
        }
        if let Some(i) =
            super::ai::switch(self, env.t, env.rng, side).filter(|&i| self.can_switch_to(side, i))
        {
            // OpponentHandleChooseAction → AI_TrySwitchOrUseItem, before the move choice
            // (and before the all-moves-unusable check of B_ACTION_USE_MOVE).
            return Action::Switch(i);
        }
        let mega = self.is_trainer() && !side.is_player();
        if super::effects::all_moves_unusable(self, env.t, side) {
            if mega && self.can_mega(side) {
                self.x.mega_pending[side.s()] = true;
            }
            self.targets[side.i()] = self.default_target(env.t, side, None);
            return Action::Struggle;
        }
        if let Some(slot) = super::effects::encored_slot(self, side) {
            // HandleTurnActionSelectionState: an encored battler's move is not asked.
            self.targets[side.i()] = self.default_target(env.t, side, Some(slot));
            return Action::Move(slot);
        }
        let o = *self.battler(side);
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
        // TrySetCantSelectMoveBattleScript refuses a disabled, tormented or taunted move:
        // the action is chosen again, and Gen3's OpponentHandleChooseMove runs
        // BattleAI_SetupAIData + BattleAI_ChooseMoveOrAction again (new rolls), as here.
        let mut choice = super::ai::choose(self, env.t, env.rng, side);
        for _ in 0..64 {
            match choice {
                Choice::Move(s) | Choice::MoveAt { slot: s, .. }
                    if super::effects::selection_block(self, env.t, side, s).is_some() =>
                {
                    choice = super::ai::choose(self, env.t, env.rng, side)
                }
                _ => break,
            }
        }
        let (slot, target) = match choice {
            Choice::Move(s) => (s, None),
            Choice::MoveAt { slot, target } => (slot, Some(target)),
            Choice::Switch(i) if self.can_switch_to(side, i) => return Action::Switch(i),
            Choice::Run => return Action::Run,
            _ => (4, None),
        };
        if (slot as usize) < 4 && o.mon.pp[slot as usize] > 0 {
            // A trainer Mega Evolves at the first opportunity (with a move).
            if mega && self.can_mega(side) {
                self.x.mega_pending[side.s()] = true;
            }
            self.targets[side.i()] = match target {
                Some(t) => self.ai_target(env.t, side, slot, t),
                None => self.default_target(env.t, side, Some(slot)),
            };
            Action::Move(slot)
        } else {
            self.targets[side.i()] = self.default_target(env.t, side, None);
            Action::Struggle
        }
    }

    /// `OpponentHandleChooseMove` / `PlayerPartnerHandleChooseMove` in doubles: the AI's
    /// target, except the user for `MOVE_TARGET_USER(_OR_SELECTED)` moves and the left foe
    /// (its partner when absent) for `MOVE_TARGET_BOTH`.
    fn ai_target(&self, t: &BattleTables, side: Side, slot: u8, target: Side) -> Side {
        use crate::mon::MoveTarget as T;
        let Some(mov) = self.battler(side).mon.moves[slot as usize] else {
            return target;
        };
        match t.move_info(mov).target {
            T::User | T::UserOrSelected => side,
            T::BothFoes => {
                let left = if side.is_player() {
                    Side::Opponent
                } else {
                    Side::Player
                };
                if self.is_absent(left) {
                    left.partner()
                } else {
                    left
                }
            }
            _ => target,
        }
    }

    /// Every action is chosen: the turn order (`SetActionsAndBattlersTurnOrder`).
    /// A pending Mega Evolution happens after the switches, items and running and before the
    /// first move action (expansion `TryDoGimmicksBeforeMoves`); when every action is a move,
    /// the turn is then ordered with the new speeds.
    fn queue_turn(&mut self, env: &mut Env) {
        let moves = |a: Option<Action>| matches!(a, Some(Action::Move(_) | Action::Struggle));
        let n = self.count();
        let pending = self.x.mega_pending != [false; 2];
        let present: Vec<Side> = self.sides().filter(|&s| !self.is_absent(s)).collect();
        if pending && present.iter().all(|&s| moves(self.actions[s.i()])) {
            self.mega_evolutions(env);
        }
        let order = self.turn_order(env);
        self.order[..n].copy_from_slice(&order);
        // CheckFocusPunch_ClearVarsBeforeTurnStarts.
        super::effects::before_turn(self, env);
        let mut megas = !pending || moves(self.actions[order[0].i()]);
        for (k, &side) in order.iter().enumerate() {
            if !megas && k > 0 {
                self.steps.push_back(Step::Megas);
                megas = true;
            }
            self.steps.push_back(Step::Act(side));
            self.steps.push_back(Step::Fainted);
        }
        self.steps.push_back(Step::EndTurn);
    }

    /// The choice of the player battler being asked ([`Battle::choosing`]) at
    /// `Need::Action`, or `None` when it does not start the turn (selection-time messages as
    /// Gen3's selection scripts).
    fn player_action(&mut self, env: &mut Env, c: Choice) -> Option<Action> {
        let p = self.choosing;
        let b = self.battler(p);
        match c {
            Choice::Mega(slot) => {
                let a = self.player_action(env, Choice::Move(slot))?;
                if matches!(a, Action::Move(_) | Action::Struggle) && self.can_mega(p) {
                    self.x.mega_pending[p.s()] = true;
                }
                Some(a)
            }
            Choice::MoveAt { slot, target } => {
                if self.is_double() && !self.sides().any(|s| s == target) {
                    return None;
                }
                self.player_action(env, Choice::Move(slot))
            }
            Choice::Move(slot) => {
                let slot = slot as usize;
                if slot >= 4 || b.mon.moves[slot].is_none() {
                    return None;
                }
                if super::effects::all_moves_unusable(self, env.t, p) {
                    // BattleScript_NoMovesLeft (STRINGID_PKMNHASNOMOVESLEFT), then Struggle.
                    env.out.push(BattleEvent::Msg(about(Str::Struggle, p)));
                    return Some(Action::Struggle);
                }
                if let Some(s) = super::effects::encored_slot(self, p) {
                    return Some(Action::Move(s));
                }
                if let Some(m) = super::effects::selection_block(self, env.t, p, slot as u8) {
                    // BattleScript_SelectingDisabledMove / TormentedMove / NotAllowedMoveTaunt.
                    env.out.push(BattleEvent::Msg(m));
                    return None;
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
            Choice::Switch(_) | Choice::LearnMove { .. } | Choice::Cancel => None,
        }
    }

    /// Gen3 `SetActionsAndBattlersTurnOrder` (not a link battle): a running player battler
    /// first (the right one when both run), else the items and switches in battler order,
    /// then the other actions, sorted pairwise by [`Battle::strikes_first`] (a bubble sort
    /// that never moves an item or a switch). Absent battlers keep their place.
    fn turn_order(&mut self, env: &mut Env) -> Vec<Side> {
        let n = self.count();
        let sides: Vec<Side> = self.sides().collect();
        let runner = [Side::Player, Side::PlayerRight]
            .into_iter()
            .filter(|s| s.i() < n && self.actions[s.i()] == Some(Action::Run))
            .last();
        if let Some(r) = runner {
            let mut order = alloc::vec![r];
            order.extend(sides.iter().copied().filter(|&s| s != r));
            return order;
        }
        // Only the player's side (battlers 0 / 2) runs first. The opponent's B_ACTION_RUN (a
        // fleeing wild mon) is ordered by `GetWhoStrikesFirst` like a move of priority 0.
        let first_class = |a: Option<Action>| matches!(a, Some(Action::Switch(_) | Action::Ball));
        let mut order: Vec<Side> = sides
            .iter()
            .copied()
            .filter(|s| first_class(self.actions[s.i()]))
            .collect();
        order.extend(
            sides
                .iter()
                .copied()
                .filter(|s| !first_class(self.actions[s.i()])),
        );
        for i in 0..n.saturating_sub(1) {
            for j in i + 1..n {
                let (a, b) = (order[i], order[j]);
                if !first_class(self.actions[a.i()])
                    && !first_class(self.actions[b.i()])
                    && self.strikes_first(env, b, a, false)
                {
                    order.swap(i, j);
                }
            }
        }
        order
    }

    /// `TryDoEventsBeforeFirstTurn`'s order: every battler by battler index, bubble-sorted
    /// with `GetWhoStrikesFirst(.., ignoreChosenMoves)` (a tie draws a roll).
    pub(crate) fn strike_order(&mut self, env: &mut Env, ignore_moves: bool) -> Vec<Side> {
        let mut order: Vec<Side> = self.sides().collect();
        let n = order.len();
        for i in 0..n - 1 {
            for j in i + 1..n {
                if self.strikes_first(env, order[j], order[i], ignore_moves) {
                    order.swap(i, j);
                }
            }
        }
        order
    }

    /// Gen3 `GetWhoStrikesFirst(first, second, ignoreChosenMoves) != 0`: whether `second`
    /// moves before `first`. Priority, then speed with Swift Swim / Chlorophyll, stages and
    /// paralysis, and Quick Claw (`gRandomTurnNumber` below `0xFFFF * param / 100`:
    /// `UINT_MAX`); a speed tie draws `Random() & 1` (only on a tie). Prankster adds 1 to a
    /// status move's priority.
    pub(crate) fn strikes_first(
        &mut self,
        env: &mut Env,
        second: Side,
        first: Side,
        ignore_moves: bool,
    ) -> bool {
        let random_turn = self.random_turn as u32;
        let speed = |s: Side| match self.battler(s).x.hold {
            Some(HoldEffect::QuickClaw(p)) if random_turn < 0xFFFF * p as u32 / 100 => u32::MAX,
            _ => self.full_speed(s),
        };
        let (s1, s2) = (speed(first), speed(second));
        let priority = |s: Side| -> i8 {
            let b = self.battler(s);
            let mov = match self.actions[s.i()] {
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
        let (p1, p2) = (priority(first), priority(second));
        if (p1 != 0 || p2 != 0) && p1 != p2 {
            p1 < p2
        } else {
            (s1 == s2 && env.rng.random() & 1 != 0) || s1 < s2
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
        let Some(action) = self.actions[side.i()].take() else {
            return;
        };
        if self.is_absent(side) || self.battler(side).mon.hp == 0 {
            return;
        }
        match action {
            Action::Move(_) | Action::Struggle => self.use_move(env, side, action),
            Action::Switch(i) => {
                // BattleScript_ActionSwitch (no STRINGID_RETURNMON string in `Str`): the foe's
                // Pursuit strikes first (`jumpifnopursuitswitchdmg`), then its exp.
                if super::effects::pursuit_on_switch(self, env, side) {
                    self.steps.push_front(Step::SwitchOut { side, party: i });
                    if let Some(exp) = self.exp_for_fainted_foe(env, side) {
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
        if !side.is_player() {
            // A wild mon fleeing (B_OUTCOME_MON_FLED).
            self.finish(env, BattleOutcome::MonFled);
            return;
        }
        let (me, foe) = (self.battler(side), self.battler(side.other()));
        let fled = if me.x.hold == Some(HoldEffect::CanAlwaysRun) || me.ability == Ability::RunAway
        {
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

    /// Whether `side` can send out party slot `i`: one of its trainer's mons (its
    /// [`Battle::party_range`]) that can fight, is not out on its side, and that its partner
    /// is not already switching to (`gBattleStruct->monToSwitchIntoId`).
    pub fn can_switch_to(&self, side: Side, i: u8) -> bool {
        let out = self.sides().any(|s| {
            s.s() == side.s()
                && !self.is_absent(s)
                && (self.battler(s).mon.party == i
                    || (s != side && self.actions[s.i()] == Some(Action::Switch(i))))
        });
        let own = self.battler(side).mon.party == i;
        !own && !out
            && self.party_range(side).contains(&i)
            && self.parties[side.s()]
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
        let id = if side.is_player() {
            Str::GoPkmn
        } else {
            Str::PkmnSentOut
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
        self.switch_in_with(env, side, i, None);
    }

    /// [`Battle::switch_in`]; `dragged` (Roar, Whirlwind) replaces the send-out message
    /// (`BattleScript_SuccessForceOut`: `switchinanim`, then "was dragged out!").
    pub(crate) fn switch_in_with(
        &mut self,
        env: &mut Env,
        side: Side,
        i: u8,
        dragged: Option<Msg>,
    ) {
        self.stamp(env);
        self.write_back(side);
        let old = *self.battler(side);
        self.battlers[side.i()] = self.battler_for(env.t, side, i);
        self.absent &= !(1 << side.i());
        // A primal weather ends when its source leaves.
        super::effects::left_field(self, env, old.ability);
        if old.fx.baton {
            // Baton Pass: the passed state, and what it held on the foe (Wave D).
            super::effects::baton_pass_in(self, &old, side);
        } else {
            // SwitchInClearSetData: the foe's escape prevention and infatuation by this
            // battler.
            self.clear_bonds(side);
        }
        // `switchineffects`: a Truant mon sent out mid-battle starts with the counter set (the
        // end of the turn clears it before its next move).
        if self.battler(side).ability == Ability::Truant {
            self.battler_mut(side).v.truant = true;
        }
        if side.is_player() {
            // UpdateSentPokesToOpponentValue: every opponent flank.
            for flank in 0..self.count() / 2 {
                self.sent_in[flank] |= 1 << i;
            }
        } else {
            // OpponentSwitchInResetSentPokesToOpponentValue.
            self.reset_sent(side);
        }
        match dragged {
            Some(m) => {
                env.out.push(BattleEvent::SendOut { side, party: i });
                env.out.push(BattleEvent::Msg(m));
            }
            None => self.send_out_events(env, side, false),
        }
        super::effects::spikes(self, env, side);
        // Primal Reversion on entry (Gen6), then the switch-in abilities.
        self.primal_reversion(env, side);
        super::effects::on_switch_in(self, env, side);
        super::effects::switch_in(self, env, side);
        super::effects::items_on_switch_in(self, env, side);
    }

    /// `OpponentSwitchInResetSentPokesToOpponentValue(opponent)`: the player battlers on the
    /// field are the ones that faced the opponent on `opponent`'s flank.
    fn reset_sent(&mut self, opponent: Side) {
        let bits = [Side::Player, Side::PlayerRight]
            .into_iter()
            .filter(|&s| self.is_present(s))
            .fold(0, |b, s| b | 1 << self.battler(s).mon.party);
        self.sent_in[opponent.i() >> 1] = bits;
    }

    /// Gen3 `HandleFaintedMonActions`: the exp of each fainted opponent (once per party slot,
    /// battler order), then `BattleScript_HandleFaintedMon` for each fainted battler on the
    /// field, in battler order.
    fn fainted(&mut self, env: &mut Env) {
        let mut next: Vec<Step> = Vec::new();
        for side in self.sides() {
            if let Some(exp) = self.exp_for_fainted_foe(env, side) {
                next.push(exp);
            }
        }
        for side in self.sides() {
            if self.battler(side).mon.hp == 0 && !self.is_absent(side) {
                next.push(Step::Replace(side));
            }
        }
        // Case 6 (no faint to handle): ABILITYEFFECT_FORECAST.
        if next.is_empty() {
            super::effects::after_action(self, env);
        }
        for s in next.into_iter().rev() {
            self.steps.push_front(s);
        }
    }

    /// `Cmd_getexp` state 1 for the opponent battler `foe` when it fainted and its exp was not
    /// given yet: the exp of each participant still standing (those that faced its flank,
    /// `gSentPokesToOpponent`), as a [`Step::GainExp`]. Then (`HandleFaintedMonActions` case
    /// 2) its flank's participants are reset.
    fn exp_for_fainted_foe(&mut self, env: &mut Env, foe: Side) -> Option<Step> {
        if foe.is_player() || self.is_absent(foe) {
            return None;
        }
        let o = *self.battler(foe);
        if o.mon.hp != 0 || self.exp_given & (1 << o.mon.party) != 0 {
            return None;
        }
        self.exp_given |= 1 << o.mon.party;
        let sent = self.sent_in[foe.i() >> 1];
        let participants = self.parties[0]
            .iter()
            .enumerate()
            .filter(|(i, p)| p.hp > 0 && sent & (1 << i) != 0)
            .count() as u8;
        let base = env.t.species_info(o.mon.species).exp_yield;
        // viaExpShare: the standing party mons holding an Exp. Share.
        let holds = &self.x.holds[0];
        let party = &self.parties[0];
        let sharers = party
            .iter()
            .zip(holds)
            .filter(|(p, h)| p.hp > 0 && **h == Some(HoldEffect::ExpShare))
            .count() as u32;
        let lucky = holds.contains(&Some(HoldEffect::LuckyEgg));
        let (exp, share) = if sharers > 0 || lucky {
            // Cmd_getexp case 1: with an Exp. Share, half to the participants and half to the
            // holders (each at least 1); the Lucky Egg and the trainer bonus come per mon.
            let calculated = (base as u32 * o.mon.level as u32 / 7) as u16 as u32;
            let half = if sharers > 0 {
                calculated / 2
            } else {
                calculated
            };
            let per = half.checked_div(participants as u32).unwrap_or(0).max(1);
            let share = (calculated / 2)
                .checked_div(sharers)
                .map_or(0, |s| s.max(1));
            (per, Some(share))
        } else {
            let exp = calc::exp_yield(base, o.mon.level, self.is_trainer(), participants);
            (exp, None)
        };
        // HandleFaintedMonActions case 2.
        self.reset_sent(foe);
        Some(Step::GainExp {
            party: 0,
            exp,
            sent,
            defeated: o.mon.species,
            share,
        })
    }

    /// `SwitchInClearSetData` / `FaintClearSetData` for the battler leaving `side`: the other
    /// battler's escape prevention and infatuation (their source in singles) end.
    /// `BattleScript_ActionSwitch` from `switchoutabilities` (Natural Cure clears the status,
    /// silently) to `returnatktoball`.
    /// A battler knocked out on its way out (Pursuit) is not recalled (`returnatktoball`
    /// skips a fainted battler).
    pub(crate) fn switch_out(&mut self, env: &mut Env, side: Side) {
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
        for m in self.parties[0].iter() {
            if m.ability(env.t) != Ability::Pickup || !env.rng.random().is_multiple_of(10) {
                continue;
            }
            env.rng.random();
            env.out
                .push(BattleEvent::Report(BattleReport::Degraded("pickup item")));
        }
    }

    pub(crate) fn clear_bonds(&mut self, side: Side) {
        // In doubles the bonds' source is not kept: the facing battler's are cleared (Gen3
        // clears those whose source is `side`).
        if self.is_present(side.other()) {
            let v = &mut self.battler_mut(side.other()).v;
            v.escape_prevented = false;
            v.infatuated = false;
        }
        super::effects::clear_bonds(self, side);
    }

    /// `BattleScript_HandleFaintedMon` for `side`: `checkteamslost`, then `openpartyscreen`:
    /// the replacement (the player's choice, or the AI's), or, with no mon left to send, the
    /// battler becomes absent (doubles).
    fn replace(&mut self, env: &mut Env, side: Side) {
        let lost = self.parties[0].all_fainted();
        let won = self.parties[1].all_fainted();
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
        if self.battler(side).mon.hp != 0 || self.is_absent(side) {
            return;
        }
        let range = self.party_range(side);
        let Some(next) = range.clone().find(|&i| self.can_switch_to(side, i)) else {
            // HasNoMonsToSwitch: `gAbsentBattlerFlags |= gBitTable[battler]`.
            self.absent |= 1 << side.i();
            self.actions[side.i()] = None;
            return;
        };
        if self.player_chooses_switch(side) {
            // Simplified: no "Use next Pokémon?" box; the party screen is the UI's.
            self.ask_switch(side);
            return;
        }
        // Simplified GetMostSuitableMonToSwitchInto: the next usable mon in party order.
        self.switch_in(env, side, next);
        if self.is_double() {
            // The replacement does not act this turn (a deviation: Gen3 lets it use its
            // slot's move).
            self.actions[side.i()] = None;
        } else {
            self.actions = [None; 4];
        }
    }

    /// Asks the player for `side`'s next mon ([`Need::Switch`], [`Battle::choosing`]).
    pub(crate) fn ask_switch(&mut self, side: Side) {
        self.choosing = side;
        self.need = Need::Switch;
    }

    /// Whether the player picks `side`'s replacement (not an opponent nor the partner).
    pub(crate) fn player_chooses_switch(&self, side: Side) -> bool {
        side.is_player() && !(side == Side::PlayerRight && self.has_partner())
    }

    /// `Cmd_getexp` state 2 for party slot `party`: the message and EVs, then the steps that
    /// give its exp, then the next slot. A partner's mons (slots 3.. with a partner) get no
    /// exp here (Gen3 gives them some silently; they leave with the partner).
    fn gain_exp(
        &mut self,
        env: &mut Env,
        party: u8,
        (exp, share): (u32, Option<u32>),
        sent: u8,
        defeated: SpeciesId,
    ) {
        let player = &self.parties[0];
        let end = if self.has_partner() {
            MULTI_PARTY_SIZE
        } else {
            PARTY_SIZE as u8
        };
        if party as usize >= PARTY_SIZE || party as usize >= player.len() || party >= end {
            return;
        }
        self.steps.push_front(Step::GainExp {
            party: party + 1,
            exp,
            sent,
            defeated,
            share,
        });
        let p = player.get(party as usize).copied().expect("party slot");
        let hold = self.x.holds[0][party as usize];
        let sharer = hold == Some(HoldEffect::ExpShare);
        let took_part = sent & (1 << party) != 0;
        if (!took_part && !sharer) || p.level >= 100 || p.hp == 0 {
            return;
        }
        // Cmd_getexp case 2: the participant's exp plus the Exp. Share's, then the Lucky Egg,
        // then the trainer bonus (already in `exp` without either item in the party).
        let exp = match share {
            None => exp,
            Some(share) => {
                let mut e = if took_part { exp } else { 0 } + if sharer { share } else { 0 };
                if hold == Some(HoldEffect::LuckyEgg) {
                    e = e * 150 / 100;
                }
                if self.is_trainer() {
                    e = e * 150 / 100;
                }
                e
            }
        };
        let side = Side::Player;
        env.out.push(BattleEvent::Msg(Msg {
            buffs: [Buff::Mon { side, party }, Buff::None, Buff::Number(exp)],
            ..msg(Str::PkmnGainedExp, side, Side::Opponent, side)
        }));
        if let Some(p) = self.parties[0].get_mut(party as usize) {
            p.gain_evs_x(env.t, defeated, hold == Some(HoldEffect::MachoBrace));
        }
        self.steps.push_front(Step::AddExp { party, rest: exp });
    }

    /// The player battler that has party slot `party` out (standing), if any.
    fn player_battler_of(&self, party: u8) -> Option<Side> {
        [Side::Player, Side::PlayerRight].into_iter().find(|&s| {
            self.is_present(s) && self.battler(s).mon.party == party && self.battler(s).mon.hp > 0
        })
    }

    /// `Cmd_getexp` states 3–5: one level's worth of exp, the level-up and its moves.
    fn exp_step(&mut self, env: &mut Env, party: u8, rest: u32) {
        let side = Side::Player;
        let Some(p) = self.parties[0].get_mut(party as usize) else {
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
        let hold = self.x.holds[0][party as usize];
        if let Some(m) = self.parties[0].get_mut(party as usize) {
            use crate::world::friendship::{FriendshipEvent, on_event};
            on_event(m, FriendshipEvent::GrowLevel, hold, env.t, env.rng);
        }
        // Update the battle mon after the level-up (when it is out and standing).
        if let Some(s) = self.player_battler_of(party) {
            let b = &mut self.battlers[s.i()];
            b.mon.level = p.level;
            b.mon.hp = p.hp;
            b.mon.max_hp = p.stats[0];
            b.stats = p.stats;
            if self.form_of(s, party).is_some() {
                // A battle form's stats come from the form's base stats.
                let f = self.battler_for(env.t, s, party);
                self.battlers[s.i()].stats = f.stats;
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
        let Some(p) = self.parties[0].get(party as usize) else {
            return;
        };
        let moves: Vec<MoveId> = p.moves_at_level(env.t, level).collect();
        for (i, &m) in moves.iter().enumerate().skip(next as usize) {
            let p = self.parties[0].get_mut(party as usize).expect("party slot");
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
        let p = self.parties[0].get_mut(party as usize).expect("party slot");
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
        for s in [Side::Player, Side::PlayerRight] {
            if !self.is_present(s) {
                continue;
            }
            let b = &mut self.battlers[s.i()];
            if b.mon.party == party {
                let k = slot as usize;
                b.mon.moves[k] = p.moves[k];
                b.mon.pp[k] = p.pp[k];
                b.mon.max_pp[k] = p.max_pp(t, slot);
            }
        }
    }

    /// Ends the battle: the closing messages of `HandleEndTurn_*`, then `End`.
    pub(crate) fn finish(&mut self, env: &mut Env, outcome: BattleOutcome) {
        self.steps.clear();
        self.actions = [None; 4];
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
                if self.setup.double.is_some_and(|d| d.second.is_some()) {
                    // BattleScript_LocalTwoTrainersDefeated: STRINGID_TWOENEMIESDEFEATED, A's
                    // slide-in and lose text, then B's.
                    env.out
                        .push(BattleEvent::Msg(msg(Str::TwoEnemiesDefeated, p, o, o)));
                    env.out.push(BattleEvent::TrainerDefeated);
                    env.out.push(BattleEvent::TrainerDefeated2);
                } else {
                    env.out
                        .push(BattleEvent::Msg(msg(Str::PlayerDefeatedTrainer, p, o, o)));
                    env.out.push(BattleEvent::TrainerDefeated);
                }
                // BattleScript_LocalBattleWonReward: `getmoneyreward` (the caller adds the
                // money), STRINGID_PLAYERGOTMONEY, `waitmessage B_WAIT_TIME_LONG`.
                match self.prize() {
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
            // HandleEndTurn_RanFromBattle: FLEE_ITEM (a Smoke Ball, checked first) names the
            // item.
            BattleOutcome::Ran if self.battler(p).x.hold == Some(HoldEffect::CanAlwaysRun) => {
                let item = self.battler(p).x.item.map_or(Buff::None, Buff::Item);
                env.out.push(button(Msg {
                    buffs: [Buff::None, Buff::None, item],
                    ..about(Str::FledUsingIts, p)
                }))
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
            // BattleScript_PayDayMoneyAndPickUpItems: `givepaydaymoney` (the caller adds it),
            // then `pickup`.
            if let Some(money) = self.pay_day() {
                env.out.push(BattleEvent::Msg(Msg {
                    buffs: [Buff::Number(money), Buff::None, Buff::None],
                    wait: LONG,
                    ..about(Str::PlayerPickedUpMoney, p)
                }));
            }
            self.pickup(env);
        }
        env.out.push(BattleEvent::End(outcome));
        self.need = Need::Ended(outcome);
    }
}
