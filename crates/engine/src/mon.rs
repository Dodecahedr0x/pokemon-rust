//! Gen3 Pokémon model. The numbers (species, moves, learnsets) are data that content fills in;
//! the formulas are Gen3's (`pokemon.c`).
//!
//! Formulas: `CreateMon`, `CalculateMonStats`, `MonGainEVs`, `gExperienceTables`, …

use crate::data::{MoveId, SpeciesId};
use crate::hash::Fnv64;
use crate::rng::Rng;
use crate::video::Tile;

pub const PARTY_SIZE: usize = 6;
/// `POKEMON_NAME_LENGTH` + EOS.
pub const NAME_LEN: usize = 11;

/// Gen3 `TYPE_*` order.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Type {
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    Mystery,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
}

impl Type {
    /// Every type, in `TYPE_*` order (`Type::ALL[t as usize] == t`).
    pub const ALL: [Type; 18] = [
        Type::Normal,
        Type::Fighting,
        Type::Flying,
        Type::Poison,
        Type::Ground,
        Type::Rock,
        Type::Bug,
        Type::Ghost,
        Type::Steel,
        Type::Mystery,
        Type::Fire,
        Type::Water,
        Type::Grass,
        Type::Electric,
        Type::Psychic,
        Type::Ice,
        Type::Dragon,
        Type::Dark,
    ];

    /// Gen3 `IS_TYPE_PHYSICAL`: the types before `Mystery` use Atk/Def.
    pub const fn is_physical(self) -> bool {
        (self as u8) < Type::Mystery as u8
    }

    /// The type of a Gen3 `TYPE_*` number.
    pub const fn from_u8(v: u8) -> Option<Type> {
        if (v as usize) < Self::ALL.len() {
            Some(Self::ALL[v as usize])
        } else {
            None
        }
    }
}

/// Gen3 `STAT_*` order.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Stat {
    Hp,
    Atk,
    Def,
    Speed,
    SpAtk,
    SpDef,
    Accuracy,
    Evasion,
}

impl Stat {
    /// Every stat, in `STAT_*` order (`Stat::ALL[s as usize] == s`).
    pub const ALL: [Stat; 8] = [
        Stat::Hp,
        Stat::Atk,
        Stat::Def,
        Stat::Speed,
        Stat::SpAtk,
        Stat::SpDef,
        Stat::Accuracy,
        Stat::Evasion,
    ];
}

/// Gen3 nature, 0..25.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Nature(pub u8);

impl Nature {
    /// `gNatureStatTable[n][stat - 1]`: +1 for the boosted stat (`Atk + n / 5`), -1 for the
    /// lowered one (`Atk + n % 5`), 0 otherwise and for neutral natures. HP, accuracy and evasion
    /// are never affected.
    pub const fn effect(self, stat: Stat) -> i8 {
        let s = stat as u8;
        if s == 0 || s > 5 {
            return 0;
        }
        let (up, down) = (1 + self.0 / 5, 1 + self.0 % 5);
        if up == down {
            0
        } else if s == up {
            1
        } else if s == down {
            -1
        } else {
            0
        }
    }
}

/// Gen3 `GROWTH_*` order.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrowthRate {
    MediumFast,
    Erratic,
    Fluctuating,
    MediumSlow,
    Fast,
    Slow,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gender {
    Male,
    Female,
    Genderless,
}

/// A non-volatile status to inflict (`MoveEffect::Status` / `StatusHit`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusKind {
    Sleep,
    Poison,
    Burn,
    Freeze,
    Paralysis,
    Toxic,
}

/// Gen3 `STATUS1_*`: `Sleep(turns left)`, `Toxic(counter)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    None,
    Sleep(u8),
    Poison,
    Burn,
    Freeze,
    Paralysis,
    Toxic(u8),
}

impl Status {
    /// `(tag, counter)`, for hashes.
    pub const fn code(self) -> (u8, u8) {
        match self {
            Status::None => (0, 0),
            Status::Sleep(n) => (1, n),
            Status::Poison => (2, 0),
            Status::Burn => (3, 0),
            Status::Freeze => (4, 0),
            Status::Paralysis => (5, 0),
            Status::Toxic(n) => (6, n),
        }
    }
}

/// Abilities the engine implements; content maps `ABILITY_*` names, the rest are `Other`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ability {
    None,
    Overgrow,
    Blaze,
    Torrent,
    Swarm,
    RunAway,
    ShieldDust,
    KeenEye,
    Guts,
    EffectSpore,
    Synchronize,
    Trace,
    Intimidate,
    Static,
    PoisonPoint,
    Other,
}

/// Gen3 move effect as a mechanic. Content maps `EFFECT_*` (`battle_move_effects.h`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveEffect {
    /// `EFFECT_HIT`.
    Hit,
    /// `EFFECT_<STAT>_UP` / `_UP_2`.
    StatUp {
        stat: Stat,
        stages: u8,
    },
    /// `EFFECT_<STAT>_DOWN` / `_DOWN_2`.
    StatDown {
        stat: Stat,
        stages: u8,
    },
    /// `EFFECT_<STAT>_DOWN_HIT` (secondary, `effect_chance`).
    StatDownHit(Stat),
    /// `EFFECT_POISON_HIT`, `BURN_HIT`, `PARALYZE_HIT`, `FREEZE_HIT`.
    StatusHit(StatusKind),
    /// `EFFECT_POISON`, `PARALYZE`, `SLEEP`, `TOXIC`.
    Status(StatusKind),
    Confuse,
    ConfuseHit,
    FlinchHit,
    FlinchMinimizeHit,
    Absorb,
    LeechSeed,
    Bide,
    FocusEnergy,
    NaturePower,
    QuickAttack,
    /// `EFFECT_RECOIL` (Struggle, Take Down).
    Recoil,
    /// `EFFECT_MULTI_HIT` (Double Slap, Arm Thrust): 2–5 hits.
    MultiHit,
    /// `EFFECT_SPLASH`.
    Splash,
    /// `EFFECT_DEFENSE_CURL`: Defense +1 (the Rollout bit is not modelled).
    DefenseCurl,
    /// `EFFECT_SONICBOOM`: 20 HP.
    SonicBoom,
    /// `EFFECT_VITAL_THROW`: a hit that never misses (its -1 priority is the move's).
    VitalThrow,
    /// `EFFECT_CHARGE`: doubles the user's Electric damage until the end of the next turn.
    Charge,
    /// Raw `EFFECT_*` number: logged; acts as `Hit` if power > 0, else "But it failed!".
    Unsupported(u8),
}

/// Gen3 `MOVE_TARGET_*` (a bit each in Gen3; one variant here).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveTarget {
    Selected,
    Depends,
    UserOrSelected,
    Random,
    BothFoes,
    User,
    FoesAndAlly,
    OpponentsField,
}

/// Gen3 move flags, same bit values.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MoveFlags(pub u8);

impl MoveFlags {
    pub const MAKES_CONTACT: u8 = 1 << 0;
    pub const PROTECT_AFFECTED: u8 = 1 << 1;
    pub const MAGIC_COAT_AFFECTED: u8 = 1 << 2;
    pub const SNATCH_AFFECTED: u8 = 1 << 3;
    pub const MIRROR_MOVE_AFFECTED: u8 = 1 << 4;
    pub const KINGS_ROCK_AFFECTED: u8 = 1 << 5;

    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LevelUpMove {
    pub level: u8,
    pub mov: MoveId,
}

/// Battle pics of a species.
#[derive(Clone, Copy, Debug)]
pub struct MonGfx {
    /// 64x64, frame 0 of `anim_front`, OBJ 1D order (64 tiles).
    pub front: &'static [Tile],
    pub back: &'static [Tile],
    pub palette: &'static [u16; 16],
    pub shiny: &'static [u16; 16],
    /// `gMonFrontPicCoords` / `gMonBackPicCoords` `y_offset`.
    pub front_y: u8,
    pub back_y: u8,
    /// `gEnemyMonElevation`.
    pub elevation: u8,
}

/// One `gSpeciesInfo` entry plus name, learnset and pics.
#[derive(Clone, Copy, Debug)]
pub struct SpeciesInfo {
    /// FR, Gen3-encoded, 0xFF-terminated.
    pub name: &'static [u8],
    /// `Stat` order (HP, Atk, Def, Speed, SpAtk, SpDef).
    pub base: [u8; 6],
    pub types: [Type; 2],
    pub catch_rate: u8,
    pub exp_yield: u8,
    pub ev_yield: [u8; 6],
    /// 0 male only, 254 female only, 255 genderless.
    pub gender_ratio: u8,
    pub friendship: u8,
    pub growth: GrowthRate,
    pub abilities: [Ability; 2],
    pub learnset: &'static [LevelUpMove],
    /// `None` for species the build left out.
    pub gfx: Option<&'static MonGfx>,
}

/// One `gBattleMoves` entry plus its name.
#[derive(Clone, Copy, Debug)]
pub struct MoveInfo {
    /// FR, Gen3-encoded, 0xFF-terminated.
    pub name: &'static [u8],
    pub effect: MoveEffect,
    pub power: u8,
    pub ty: Type,
    pub accuracy: u8,
    pub pp: u8,
    pub effect_chance: u8,
    pub target: MoveTarget,
    pub priority: i8,
    pub flags: MoveFlags,
}

/// One `gTypeEffectiveness` entry; `multiplier` is 0, 5 or 20 (tenths).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeMatchup {
    pub attack: Type,
    pub defend: Type,
    pub multiplier: u8,
}

/// A trainer front pic (or back pic frames, 64 tiles each).
#[derive(Clone, Copy, Debug)]
pub struct TrainerPic {
    pub tiles: &'static [Tile],
    pub palette: &'static [u16; 16],
    /// Gen3 `MonCoords.size` of `gTrainerFrontPicCoords` / `gTrainerBackPicCoords` (1..=8): the
    /// pic's centre y is derived from it (`(8 - size) * 4 + 40` front, `+ 80` back; see
    /// `battle::gfx::TrainerSprite`). 8 for every Emerald pic and for empty pics.
    pub y: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrainerMon {
    pub species: SpeciesId,
    pub level: u8,
    /// 0..=255, scaled to `iv * 31 / 255`.
    pub iv: u8,
    /// Custom moveset, `None` for the level-up default.
    pub moves: Option<[Option<MoveId>; 4]>,
}

/// One `gTrainers` entry.
#[derive(Clone, Copy, Debug)]
pub struct TrainerData {
    /// Gen3-encoded, 0xFF-terminated.
    pub class_name: &'static [u8],
    pub name: &'static [u8],
    pub pic: &'static TrainerPic,
    /// `gTrainers[].encounterMusic_gender & F_TRAINER_FEMALE`.
    pub female: bool,
    /// Gen3 `AI_SCRIPT_*` bits.
    pub ai: u32,
    pub double: bool,
    pub party: &'static [TrainerMon],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WildMon {
    pub species: SpeciesId,
    pub min_level: u8,
    pub max_level: u8,
}

/// `gWildMonHeaders[].landMonsInfo`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LandEncounters {
    pub rate: u8,
    pub mons: [WildMon; 12],
}

/// All battle data tables.
#[derive(Clone, Copy, Debug)]
pub struct BattleTables {
    /// Indexed by `SpeciesId`.
    pub species: &'static [SpeciesInfo],
    /// Indexed by `MoveId`.
    pub moves: &'static [MoveInfo],
    /// `gTypeEffectiveness` order (it affects rounding).
    pub type_chart: &'static [TypeMatchup],
    /// Entries from here on are ignored under Foresight.
    pub foresight_split: usize,
    pub struggle: MoveId,
    /// `sNaturePowerMoves`, by `battle::Environment`.
    pub nature_power: [MoveId; 10],
}

impl BattleTables {
    /// The species `id`. Panics if the tables have no such species (ids come from content).
    pub fn species_info(&self, id: SpeciesId) -> &'static SpeciesInfo {
        &self.species[id.0 as usize]
    }

    /// The move `id`. Panics if the tables have no such move.
    pub fn move_info(&self, id: MoveId) -> &'static MoveInfo {
        &self.moves[id.0 as usize]
    }
}

/// No species, no moves: the default of `GameData::battle`.
pub static EMPTY_TABLES: BattleTables = BattleTables {
    species: &[],
    moves: &[],
    type_chart: &[],
    foresight_split: 0,
    struggle: MoveId(0),
    nature_power: [MoveId(0); 10],
};

/// A party or box Pokémon (Gen3 `struct Pokemon`, decrypted).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pokemon {
    pub species: SpeciesId,
    pub personality: u32,
    pub ot_id: u32,
    pub nickname: [u8; NAME_LEN],
    pub level: u8,
    pub exp: u32,
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
    pub moves: [Option<MoveId>; 4],
    pub pp: [u8; 4],
    pub pp_bonuses: u8,
    pub friendship: u8,
    pub ability_slot: u8,
    pub status: Status,
    pub hp: u16,
    /// `CalculateMonStats` cache; `stats[0]` = max HP.
    pub stats: [u16; 6],
}

/// `USE_RANDOM_IVS` / `fixedIV`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ivs {
    Random,
    Fixed(u8),
}

/// Result of `Pokemon::try_learn` (`MonTryLearningNewMove`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Learn {
    /// Learned into this slot.
    Learned(u8),
    AlreadyKnown,
    /// Four moves known: the player must choose one to forget.
    Full,
}

/// Gen3 `MAX_LEVEL`.
pub const MAX_LEVEL: u8 = 100;
/// Gen3 `USE_RANDOM_IVS`: `Ivs::Fixed` values from here on draw random IVs, as `CreateBoxMon`.
const USE_RANDOM_IVS: u8 = 32;
/// Gen3 `MAX_TOTAL_EVS` / `MAX_PER_STAT_EVS`.
const MAX_TOTAL_EVS: u16 = 510;
const MAX_PER_STAT_EVS: u8 = 255;
/// Gen3 `SHINY_ODDS`.
const SHINY_ODDS: u32 = 8;

/// Gen3 `IsShinyOtIdPersonality`: `HIHALF ^ LOHALF` of both words below `SHINY_ODDS`.
pub const fn is_shiny_ot_personality(ot_id: u32, personality: u32) -> bool {
    ((ot_id >> 16) ^ (ot_id & 0xFFFF) ^ (personality >> 16) ^ (personality & 0xFFFF)) < SHINY_ODDS
}

/// Gen3 `CalculatePPWithBonus`: the max PP of `m` in `slot` with the slot's PP Ups (2 bits
/// each in `pp_bonuses`).
pub fn pp_with_bonus(t: &BattleTables, m: MoveId, pp_bonuses: u8, slot: u8) -> u8 {
    let base = t.move_info(m).pp as u32;
    let ups = (pp_bonuses >> (2 * slot)) & 3;
    (base + base * 20 * ups as u32 / 100) as u8
}

impl Pokemon {
    /// Gen3 `CreateMon` + `CreateBoxMon` + `GiveMonInitialMoveset` (`pokemon.c`), with a fixed
    /// personality and OT id (callers draw them where Gen3 does). `Ivs::Random` draws two
    /// `random()`s: HP/Atk/Def, then Spe/SpA/SpD, 5 bits each. The nickname is the species
    /// name, exp the level's minimum, friendship the species', `ability_slot` the personality's
    /// low bit when the species has a second ability. The current HP is the max HP.
    ///
    /// Callers draw the personality where Gen3 does: `GiveMon` (`ScriptGiveMon`) and the
    /// starter (`CreateMon` with a random personality) draw `random32()` for it first, then call
    /// `Pokemon::new(…, Ivs::Random, personality, player_id, rng)`; wild mons go through
    /// `wild::create_wild`, trainer mons through `trainer::create_trainer_party`.
    pub fn new(
        t: &BattleTables,
        species: SpeciesId,
        level: u8,
        ivs: Ivs,
        personality: u32,
        ot_id: u32,
        rng: &mut Rng,
    ) -> Pokemon {
        debug_assert!((1..=MAX_LEVEL).contains(&level), "level {level}");
        let info = t.species_info(species);
        let ivs = match ivs {
            Ivs::Fixed(v) if v < USE_RANDOM_IVS => [v; 6],
            _ => {
                let (a, b) = (rng.random(), rng.random());
                let f = |v: u16, i: u16| ((v >> (5 * i)) & 31) as u8;
                [f(a, 0), f(a, 1), f(a, 2), f(b, 0), f(b, 1), f(b, 2)]
            }
        };
        // GetSpeciesName: up to POKEMON_NAME_LENGTH characters, then EOS.
        let mut nickname = [0xFF; NAME_LEN];
        let name = info
            .name
            .iter()
            .take(NAME_LEN - 1)
            .take_while(|&&c| c != 0xFF);
        for (d, &c) in nickname.iter_mut().zip(name) {
            *d = c;
        }
        let ability_slot = if info.abilities[1] != Ability::None {
            (personality & 1) as u8
        } else {
            0
        };
        let mut p = Pokemon {
            species,
            personality,
            ot_id,
            nickname,
            level,
            exp: exp_for_level(info.growth, level),
            ivs,
            evs: [0; 6],
            moves: [None; 4],
            pp: [0; 4],
            pp_bonuses: 0,
            friendship: info.friendship,
            ability_slot,
            status: Status::None,
            hp: 0,
            stats: [0; 6],
        };
        p.give_initial_moveset(t);
        p.recalc_stats(t);
        p
    }

    /// Gen3 `GiveBoxMonInitialMoveset`: the learnset up to the level, skipping known moves and
    /// shifting the first move out when all four slots are taken.
    fn give_initial_moveset(&mut self, t: &BattleTables) {
        for lm in t.species_info(self.species).learnset {
            if lm.level > self.level {
                break;
            }
            if self.give_move(t, lm.mov) == Learn::Full {
                self.delete_first_move_and_give(t, lm.mov);
            }
        }
    }

    /// Gen3 `GiveMoveToBoxMon`: the first empty slot, unless the move is met first.
    fn give_move(&mut self, t: &BattleTables, m: MoveId) -> Learn {
        for i in 0..4 {
            match self.moves[i] {
                None => {
                    self.moves[i] = Some(m);
                    self.pp[i] = t.move_info(m).pp;
                    return Learn::Learned(i as u8);
                }
                Some(known) if known == m => return Learn::AlreadyKnown,
                Some(_) => {}
            }
        }
        Learn::Full
    }

    /// Gen3 `DeleteFirstMoveAndGiveMoveToBoxMon` (PP Ups shift along with the moves).
    fn delete_first_move_and_give(&mut self, t: &BattleTables, m: MoveId) {
        self.moves.copy_within(1.., 0);
        self.pp.copy_within(1.., 0);
        self.moves[3] = Some(m);
        self.pp[3] = t.move_info(m).pp;
        self.pp_bonuses >>= 2;
    }

    /// `personality % 25` (`GetNature`).
    pub fn nature(&self) -> Nature {
        Nature((self.personality % 25) as u8)
    }

    /// Gen3 `GetGenderFromSpeciesAndPersonality`: ratios 0 / 254 / 255 are fixed, otherwise
    /// female when `ratio > personality & 0xFF`.
    pub fn gender(&self, t: &BattleTables) -> Gender {
        match t.species_info(self.species).gender_ratio {
            0 => Gender::Male,
            254 => Gender::Female,
            255 => Gender::Genderless,
            r if r as u32 > self.personality & 0xFF => Gender::Female,
            _ => Gender::Male,
        }
    }

    /// Gen3 `GetAbilityBySpecies` with `ability_slot`.
    pub fn ability(&self, t: &BattleTables) -> Ability {
        t.species_info(self.species).abilities[(self.ability_slot != 0) as usize]
    }

    /// Gen3 `IsShinyOtIdPersonality`.
    pub fn is_shiny(&self) -> bool {
        is_shiny_ot_personality(self.ot_id, self.personality)
    }

    /// The max PP of the move in `slot` (0 for an empty slot), with PP Ups.
    pub fn max_pp(&self, t: &BattleTables, slot: u8) -> u8 {
        self.moves[slot as usize].map_or(0, |m| pp_with_bonus(t, m, self.pp_bonuses, slot))
    }

    /// Gen3 `CalculateMonStats`: the level from exp (`GetLevelFromMonExp`), `CALC_STAT` with
    /// `ModifyStatByNature` (x110/100, x90/100, in a u16 as Gen3). The current HP moves with
    /// the max HP; 0 stays 0 except for a new mon (both 0), which starts at full HP.
    /// Differences: the Pomeg underflow is clamped to 1 HP (Gen3 `BUGFIX`). Shedinja's rule
    /// (max HP 1) is deferred: the engine holds no species numbers, so if it is ever needed it
    /// becomes a data flag (e.g. `SpeciesInfo::max_hp_one`) that content sets.
    pub fn recalc_stats(&mut self, t: &BattleTables) {
        let info = t.species_info(self.species);
        let old_max = self.stats[0] as i32;
        let cur = self.hp as i32;
        self.level = level_for_exp_from(info.growth, self.exp, self.level);
        let level = self.level as i32;
        let raw = |i: usize| {
            (2 * info.base[i] as i32 + self.ivs[i] as i32 + self.evs[i] as i32 / 4) * level / 100
        };
        let new_max = raw(0) + level + 10;
        self.stats[0] = new_max as u16;
        let nature = self.nature();
        for (i, &stat) in Stat::ALL[1..6].iter().enumerate().map(|(i, s)| (i + 1, s)) {
            let n = (raw(i) + 5) as u16;
            self.stats[i] = match nature.effect(stat) {
                1 => n.wrapping_mul(110) / 100,
                -1 => n.wrapping_mul(90) / 100,
                _ => n,
            };
        }
        if cur == 0 && old_max == 0 {
            self.hp = new_max as u16;
        } else if cur != 0 {
            self.hp = (cur + new_max - old_max).max(1) as u16;
        }
    }

    /// Restores HP, PP (with `pp_bonuses`) and status (Gen3 `HealPlayerParty`, one mon).
    pub fn heal(&mut self, t: &BattleTables) {
        self.hp = self.stats[0];
        for slot in 0..4u8 {
            self.pp[slot as usize] = self.max_pp(t, slot);
        }
        self.status = Status::None;
    }

    /// Gen3 `MonGainEVs`, without Pokérus or held items: each stat in order gains the defeated
    /// species' yield, capped by the 510 total (the loop stops once it is reached) and 255 per
    /// stat.
    pub fn gain_evs(&mut self, t: &BattleTables, defeated: SpeciesId) {
        let yields = t.species_info(defeated).ev_yield;
        let mut total: u16 = self.evs.iter().map(|&e| e as u16).sum();
        for (ev, &gain) in self.evs.iter_mut().zip(&yields) {
            if total >= MAX_TOTAL_EVS {
                break;
            }
            let gain = (gain as u16)
                .min(MAX_TOTAL_EVS - total)
                .min((MAX_PER_STAT_EVS - *ev) as u16);
            *ev += gain as u8;
            total += gain;
        }
    }

    /// Adds exp up to the next level boundary; returns true if a level was gained (stats are
    /// recalculated). Callers loop, so each level-up gets its own events. Gen3 `Cmd_getexp`
    /// level loop (`Task_GiveExpWithExpBar`) + `CalculateMonStats`. At `MAX_LEVEL` nothing is
    /// gained and `exp` is dropped (Gen3 skips such mons).
    pub fn add_exp_step(&mut self, t: &BattleTables, exp: &mut u32) -> bool {
        if self.level >= MAX_LEVEL {
            *exp = 0;
            return false;
        }
        let growth = t.species_info(self.species).growth;
        let next = exp_for_level(growth, self.level + 1);
        if self.exp.saturating_add(*exp) >= next {
            *exp -= next.saturating_sub(self.exp);
            self.exp = next;
            self.recalc_stats(t);
            true
        } else {
            self.exp += *exp;
            *exp = 0;
            false
        }
    }

    /// The learnset moves of this species at exactly `level`, in learnset order (Gen3
    /// `MonTryLearningNewMove` walks the same entries).
    pub fn moves_at_level(&self, t: &BattleTables, level: u8) -> impl Iterator<Item = MoveId> {
        t.species_info(self.species)
            .learnset
            .iter()
            .filter(move |m| m.level == level)
            .map(|m| m.mov)
    }

    /// Gen3 `MonTryLearningNewMove` for one move (`GiveMoveToMon`): the first empty slot with
    /// full base PP, `AlreadyKnown` if the move is met before an empty slot, else `Full`.
    pub fn try_learn(&mut self, t: &BattleTables, m: MoveId) -> Learn {
        self.give_move(t, m)
    }

    /// Replaces the move in `slot` with full base PP, clearing the slot's PP Ups (Gen3
    /// `RemoveMonPPBonus` + `SetMonMoveSlot`, as the learn-move flow of `Cmd_yesnoboxlearnmove`).
    pub fn replace_move(&mut self, t: &BattleTables, slot: u8, m: MoveId) {
        self.pp_bonuses &= !(3 << (2 * slot));
        self.moves[slot as usize] = Some(m);
        self.pp[slot as usize] = t.move_info(m).pp;
    }

    /// Feeds every field into `h`.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.species.0);
        h.write_u32(self.personality);
        h.write_u32(self.ot_id);
        h.write(&self.nickname);
        h.write(&[self.level]);
        h.write_u32(self.exp);
        h.write(&self.ivs);
        h.write(&self.evs);
        for m in self.moves {
            h.write_u16(m.map_or(0xFFFF, |m| m.0));
        }
        h.write(&self.pp);
        let (tag, n) = self.status.code();
        h.write(&[self.pp_bonuses, self.friendship, self.ability_slot, tag, n]);
        h.write_u16(self.hp);
        for s in self.stats {
            h.write_u16(s);
        }
    }
}

/// Total exp needed to reach `level` (Gen3 `gExperienceTables`, the `experience_tables.h`
/// macros in C `int` arithmetic, which fits every level up to `MAX_LEVEL`). Levels 0 and 1 are
/// 0 and 1 in every table (the MediumSlow formula would be negative at 1).
pub fn exp_for_level(g: GrowthRate, level: u8) -> u32 {
    debug_assert!(level <= MAX_LEVEL, "level {level}");
    let n = level as i32;
    if n <= 1 {
        return n as u32;
    }
    let cube = n * n * n;
    let v = match g {
        GrowthRate::MediumFast => cube,
        GrowthRate::Erratic => match n {
            2..=50 => (100 - n) * cube / 50,
            51..=68 => (150 - n) * cube / 100,
            69..=98 => ((1911 - 10 * n) / 3) * cube / 500,
            _ => (160 - n) * cube / 100,
        },
        GrowthRate::Fluctuating => match n {
            2..=15 => ((n + 1) / 3 + 24) * cube / 50,
            16..=36 => (n + 14) * cube / 50,
            _ => (n / 2 + 32) * cube / 50,
        },
        GrowthRate::MediumSlow => 6 * cube / 5 - 15 * n * n + 100 * n - 140,
        GrowthRate::Fast => 4 * cube / 5,
        GrowthRate::Slow => 5 * cube / 4,
    };
    v as u32
}

/// Gen3 `GetLevelFromMonExp`: the highest level up to `MAX_LEVEL` whose exp is reached.
pub fn level_for_exp(g: GrowthRate, exp: u32) -> u8 {
    level_for_exp_from(g, exp, 1)
}

/// `level_for_exp`, scanning up from `start` when `start` is already reached (the tables
/// increase from level 1, so the result is the same); otherwise from level 1.
fn level_for_exp_from(g: GrowthRate, exp: u32, start: u8) -> u8 {
    let mut level = start.clamp(1, MAX_LEVEL);
    if exp_for_level(g, level) > exp {
        level = 1;
    }
    while level <= MAX_LEVEL && exp_for_level(g, level) <= exp {
        level += 1;
    }
    level - 1
}

/// Up to six Pokémon, compact (no holes): `get(i)` is `Some` exactly for `i < len()`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Party {
    mons: [Option<Pokemon>; PARTY_SIZE],
}

impl Party {
    pub fn len(&self) -> usize {
        self.mons.iter().take_while(|m| m.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.mons[0].is_none()
    }

    pub fn get(&self, i: usize) -> Option<&Pokemon> {
        self.mons.get(i)?.as_ref()
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut Pokemon> {
        self.mons.get_mut(i)?.as_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pokemon> {
        self.mons.iter().map_while(Option::as_ref)
    }

    /// Appends `p`; returns its slot, or gives it back when the party is full.
    pub fn push(&mut self, p: Pokemon) -> Result<u8, Pokemon> {
        let i = self.len();
        match self.mons.get_mut(i) {
            Some(slot) => {
                *slot = Some(p);
                Ok(i as u8)
            }
            None => Err(p),
        }
    }

    /// The first Pokémon that can fight (HP > 0).
    pub fn first_usable(&self) -> Option<u8> {
        self.iter().position(|p| p.hp > 0).map(|i| i as u8)
    }

    /// Whether no Pokémon can fight (true for an empty party).
    pub fn all_fainted(&self) -> bool {
        self.first_usable().is_none()
    }

    /// Heals every member (Gen3 `HealPlayerParty`).
    pub fn heal(&mut self, t: &BattleTables) {
        for p in self.mons.iter_mut().flatten() {
            p.heal(t);
        }
    }

    /// Feeds the length and every member into `h`.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.len() as u8]);
        for p in self.iter() {
            p.hash(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(hp: u16) -> Pokemon {
        Pokemon {
            species: SpeciesId(1),
            personality: 0,
            ot_id: 0,
            nickname: [0xFF; NAME_LEN],
            level: 5,
            exp: 0,
            ivs: [0; 6],
            evs: [0; 6],
            moves: [None; 4],
            pp: [0; 4],
            pp_bonuses: 0,
            friendship: 0,
            ability_slot: 0,
            status: Status::None,
            hp,
            stats: [hp; 6],
        }
    }

    #[test]
    fn type_and_stat_orders_match_gen3() {
        for (i, t) in Type::ALL.iter().enumerate() {
            assert_eq!(*t as usize, i);
            assert_eq!(Type::from_u8(i as u8), Some(*t));
        }
        assert_eq!(Type::from_u8(18), None);
        assert_eq!(Type::Dark as u8, 17);
        assert!(Type::Steel.is_physical());
        assert!(!Type::Mystery.is_physical());
        assert!(!Type::Fire.is_physical());
        for (i, s) in Stat::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i);
        }
    }

    #[test]
    fn nature_effects_follow_gnaturestattable() {
        // NATURE_HARDY, LONELY (+Atk -Def), ADAMANT (+Atk -SpA), BOLD (+Def -Atk), QUIRKY (24).
        assert_eq!(Nature(0).effect(Stat::Atk), 0);
        assert_eq!(Nature(1).effect(Stat::Atk), 1);
        assert_eq!(Nature(1).effect(Stat::Def), -1);
        assert_eq!(Nature(3).effect(Stat::SpAtk), -1);
        assert_eq!(Nature(3).effect(Stat::Speed), 0);
        assert_eq!(Nature(5).effect(Stat::Def), 1);
        assert_eq!(Nature(5).effect(Stat::Atk), -1);
        for s in Stat::ALL {
            assert_eq!(Nature(24).effect(s), 0);
            assert_eq!(Nature(1).effect(Stat::Hp), 0);
        }
        for n in 0..25 {
            let sum: i8 = Stat::ALL.iter().map(|&s| Nature(n).effect(s)).sum();
            assert_eq!(sum, 0, "nature {n}");
        }
    }

    #[test]
    fn party_stays_compact() {
        let mut p = Party::default();
        assert!(p.is_empty());
        assert!(p.all_fainted());
        assert_eq!(p.first_usable(), None);
        assert_eq!(p.push(mon(0)), Ok(0));
        assert_eq!(p.push(mon(7)), Ok(1));
        assert_eq!(p.len(), 2);
        assert_eq!(p.first_usable(), Some(1));
        assert!(!p.all_fainted());
        for _ in 2..PARTY_SIZE {
            assert!(p.push(mon(1)).is_ok());
        }
        assert_eq!(p.push(mon(9)), Err(mon(9)));
        assert_eq!(p.get(1).map(|m| m.hp), Some(7));
        assert_eq!(p.get(6), None);
        p.get_mut(0).unwrap().hp = 3;
        assert_eq!(p.first_usable(), Some(0));
        assert_eq!(p.iter().count(), PARTY_SIZE);
    }

    #[test]
    fn hashes_see_every_change() {
        let base = |p: &Party| {
            let mut h = Fnv64::new();
            p.hash(&mut h);
            h.finish()
        };
        let mut p = Party::default();
        let empty = base(&p);
        p.push(mon(5)).unwrap();
        let one = base(&p);
        assert_ne!(empty, one);
        p.get_mut(0).unwrap().status = Status::Sleep(2);
        let slept = base(&p);
        assert_ne!(one, slept);
        p.get_mut(0).unwrap().status = Status::Sleep(1);
        assert_ne!(slept, base(&p));
        p.get_mut(0).unwrap().moves[0] = Some(MoveId(0));
        let with_move = base(&p);
        p.get_mut(0).unwrap().moves[0] = None;
        assert_ne!(with_move, base(&p));
    }

    #[test]
    fn moves_at_level_filters_the_learnset() {
        static LEARNSET: [LevelUpMove; 3] = [
            LevelUpMove {
                level: 1,
                mov: MoveId(1),
            },
            LevelUpMove {
                level: 6,
                mov: MoveId(2),
            },
            LevelUpMove {
                level: 6,
                mov: MoveId(3),
            },
        ];
        static SPECIES: [SpeciesInfo; 2] = [
            SpeciesInfo {
                name: &[0xFF],
                base: [0; 6],
                types: [Type::Normal; 2],
                catch_rate: 0,
                exp_yield: 0,
                ev_yield: [0; 6],
                gender_ratio: 0,
                friendship: 0,
                growth: GrowthRate::MediumFast,
                abilities: [Ability::None; 2],
                learnset: &[],
                gfx: None,
            },
            SpeciesInfo {
                name: &[0xFF],
                base: [0; 6],
                types: [Type::Water; 2],
                catch_rate: 0,
                exp_yield: 0,
                ev_yield: [0; 6],
                gender_ratio: 0,
                friendship: 0,
                growth: GrowthRate::MediumSlow,
                abilities: [Ability::Torrent, Ability::None],
                learnset: &LEARNSET,
                gfx: None,
            },
        ];
        let t = BattleTables {
            species: &SPECIES,
            ..EMPTY_TABLES
        };
        let m = mon(1);
        let at6: alloc::vec::Vec<_> = m.moves_at_level(&t, 6).collect();
        assert_eq!(at6, [MoveId(2), MoveId(3)]);
        assert_eq!(m.moves_at_level(&t, 2).count(), 0);
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::battle::test_data::*;

    fn new(species: SpeciesId, level: u8, ivs: Ivs, personality: u32) -> Pokemon {
        Pokemon::new(
            &TABLES,
            species,
            level,
            ivs,
            personality,
            0,
            &mut Rng::new(1),
        )
    }

    #[test]
    fn mudkip_l5_adamant_stats() {
        // CalculateMonStats: HP ((2*50+10)*5)/100 + 5 + 10 = 20; Atk (150*5/100 + 5) = 12,
        // *110/100 = 13; Def 110*5/100 + 5 = 10; Spe 90*5/100 + 5 = 9; SpA 10 *90/100 = 9;
        // SpD 10. Personality 3: NATURE_ADAMANT (+Atk -SpA).
        let m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
        assert_eq!(m.nature(), Nature(3));
        assert_eq!(m.stats, [20, 13, 10, 9, 9, 10]);
        assert_eq!(m.hp, 20);
        assert_eq!(m.level, 5);
        assert_eq!(m.ivs, [10; 6]);
        assert_eq!(m.evs, [0; 6]);
        // gExperienceTables[GROWTH_MEDIUM_SLOW][5] = 6*125/5 - 15*25 + 500 - 140 = 135.
        assert_eq!(m.exp, 135);
        assert_eq!(m.friendship, 70);
        assert_eq!(&m.nickname[..7], MUDKIP_NAME);
        assert_eq!(m.nickname[7..], [0xFF; 4]);
        assert_eq!(m.moves, [Some(TACKLE), Some(GROWL), None, None]);
        assert_eq!(m.pp, [35, 40, 0, 0]);
        assert_eq!(m.status, Status::None);
        assert_eq!((m.personality, m.ot_id, m.pp_bonuses), (3, 0, 0));
    }

    #[test]
    fn zigzagoon_l2_hardy_stats() {
        // HP (76*2)/100 + 2 + 10 = 13; Def (82*2)/100 + 5 = 6.
        let m = new(ZIGZAGOON, 2, Ivs::Fixed(0), 0);
        assert_eq!(m.stats[Stat::Hp as usize], 13);
        assert_eq!(m.stats[Stat::Def as usize], 6);
        assert_eq!(m.exp, 8);
        assert_eq!(m.moves, [Some(TACKLE), None, None, None]);
    }

    #[test]
    fn random_ivs_use_two_rolls_of_three_five_bit_fields() {
        // CreateBoxMon: Random() -> HP | Atk << 5 | Def << 10, then Spe | SpA << 5 | SpD << 10.
        let mut r = Rng::new(99);
        let (a, b) = (r.random(), r.random());
        let m = Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Random, 0, 0, &mut Rng::new(99));
        let f = |v: u16, i: u16| ((v >> (5 * i)) & 31) as u8;
        assert_eq!(
            m.ivs,
            [f(a, 0), f(a, 1), f(a, 2), f(b, 0), f(b, 1), f(b, 2)]
        );
        // Fixed IVs draw nothing.
        let mut r = Rng::new(99);
        Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Fixed(31), 0, 0, &mut r);
        assert_eq!(r, Rng::new(99));
        // `fixedIV >= USE_RANDOM_IVS` (32) draws random IVs: the same two rolls.
        let mut r = Rng::new(99);
        let f32 = Pokemon::new(&TABLES, MUDKIP, 5, Ivs::Fixed(32), 0, 0, &mut r);
        assert_eq!(f32.ivs, m.ivs);
        let mut two = Rng::new(99);
        two.random();
        two.random();
        assert_eq!(r, two);
    }

    #[test]
    fn nickname_is_the_species_name_truncated_to_10() {
        // GetSpeciesName copies at most POKEMON_NAME_LENGTH (10) characters, then EOS.
        let m = new(GROWLER, 5, Ivs::Fixed(0), 0);
        assert_eq!(m.nickname[..10], LONG_NAME[..10]);
        assert_eq!(m.nickname[10], 0xFF);
    }

    #[test]
    fn recalc_after_exp_changes_finds_the_level_either_way() {
        // Scanning starts at the current level when reached; a lower exp rescans from 1.
        let mut m = new(MUDKIP, 10, Ivs::Fixed(0), 0);
        m.exp = exp_for_level(GrowthRate::MediumSlow, 30);
        m.recalc_stats(&TABLES);
        assert_eq!(m.level, 30);
        m.exp = exp_for_level(GrowthRate::MediumSlow, 7) + 1;
        m.recalc_stats(&TABLES);
        assert_eq!(m.level, 7);
    }

    #[test]
    fn initial_moveset_shifts_out_the_first_move_when_full() {
        // GiveBoxMonInitialMoveset: L1 Tackle, L1 Growl, L2 Mud-Slap, L3 Water Gun fill the four
        // slots; L4 Bide shifts Tackle out; L5 Tail Whip shifts Growl out; L5 Water Gun is
        // already known (skipped); L9 Growl is above the level.
        let m = new(SPARK, 5, Ivs::Fixed(0), 0);
        assert_eq!(
            m.moves,
            [Some(MUD_SLAP), Some(WATER_GUN), Some(BIDE), Some(TAIL_WHIP)]
        );
        assert_eq!(m.pp, [10, 25, 10, 30]);
        let m = new(SPARK, 4, Ivs::Fixed(0), 0);
        assert_eq!(
            m.moves,
            [Some(GROWL), Some(MUD_SLAP), Some(WATER_GUN), Some(BIDE)]
        );
    }

    #[test]
    fn ability_slot_follows_personality_bit_when_the_species_has_two() {
        let a = new(SPARK, 5, Ivs::Fixed(0), 0);
        assert_eq!((a.ability_slot, a.ability(&TABLES)), (0, Ability::Static));
        let b = new(SPARK, 5, Ivs::Fixed(0), 1);
        assert_eq!(
            (b.ability_slot, b.ability(&TABLES)),
            (1, Ability::Synchronize)
        );
        // No second ability: slot stays 0 (CreateBoxMon only sets it when abilities[1] != 0).
        let c = new(MUDKIP, 5, Ivs::Fixed(0), 1);
        assert_eq!((c.ability_slot, c.ability(&TABLES)), (0, Ability::Torrent));
    }

    #[test]
    fn natures_are_personality_mod_25() {
        let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
        for (p, n) in [(0, 0), (24, 24), (25, 0), (3, 3), (u32::MAX, 20)] {
            m.personality = p;
            assert_eq!(m.nature(), Nature(n), "personality {p:#x}");
        }
    }

    #[test]
    fn gender_thresholds() {
        // GetGenderFromSpeciesAndPersonality: ratio > (p & 0xFF) is female.
        let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
        for (p, g) in [
            (30, Gender::Female),
            (31, Gender::Male),
            (0x100, Gender::Female),
            (0xFF, Gender::Male),
        ] {
            m.personality = p;
            assert_eq!(m.gender(&TABLES), g, "personality {p:#x}");
        }
        let mut z = new(ZIGZAGOON, 5, Ivs::Fixed(0), 126);
        assert_eq!(z.gender(&TABLES), Gender::Female);
        z.personality = 127;
        assert_eq!(z.gender(&TABLES), Gender::Male);
        // MON_MALE (0), MON_FEMALE (254), MON_GENDERLESS (255) ignore the personality.
        assert_eq!(
            new(SPARK, 5, Ivs::Fixed(0), 0).gender(&TABLES),
            Gender::Male
        );
        assert_eq!(
            new(HAWK, 5, Ivs::Fixed(0), 0xFF).gender(&TABLES),
            Gender::Female
        );
        assert_eq!(
            new(BLOB, 5, Ivs::Fixed(0), 0).gender(&TABLES),
            Gender::Genderless
        );
    }

    #[test]
    fn shiny_when_the_xor_of_halves_is_below_8() {
        let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
        // HIHALF(ot) ^ LOHALF(ot) ^ HIHALF(p) ^ LOHALF(p): 0x1234 ^ 0x5678 ^ 0x1234 ^ 0x567F = 7.
        m.ot_id = 0x1234_5678;
        m.personality = 0x1234_567F;
        assert!(m.is_shiny());
        m.personality = 0x1234_5670; // 8
        assert!(!m.is_shiny());
        m.personality = 0x1234_5678; // 0
        assert!(m.is_shiny());
    }

    #[test]
    fn level_up_keeps_the_hp_offset_and_learns_mud_slap() {
        // Mudkip L5 (135 exp) gains 50: 185 >= 179 (L6), so exp = 179, 6 left, then 185 < 236.
        // L6 stats: HP 110*6/100 + 6 + 10 = 22, Atk (150*6/100 + 5) * 110/100 = 15,
        // Def 110*6/100 + 5 = 11, Spe 90*6/100 + 5 = 10, SpA 11 * 90/100 = 9, SpD 11.
        let mut m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
        m.hp = 15;
        let mut exp = 50;
        assert!(m.add_exp_step(&TABLES, &mut exp));
        assert_eq!((m.level, m.exp, exp), (6, 179, 6));
        assert_eq!(m.stats, [22, 15, 11, 10, 9, 11]);
        assert_eq!(m.hp, 17);
        let learned: alloc::vec::Vec<_> = m.moves_at_level(&TABLES, m.level).collect();
        assert_eq!(learned, [MUD_SLAP]);
        assert_eq!(m.try_learn(&TABLES, MUD_SLAP), Learn::Learned(2));
        assert_eq!(m.pp[2], 10);
        assert!(!m.add_exp_step(&TABLES, &mut exp));
        assert_eq!((m.level, m.exp, exp), (6, 185, 0));
        // A fainted mon stays at 0 HP through level-ups (CalculateMonStats).
        m.hp = 0;
        let mut exp = 1000;
        while m.add_exp_step(&TABLES, &mut exp) {}
        assert_eq!(m.hp, 0);
        assert!(m.level > 6);
    }

    #[test]
    fn exp_gain_stops_at_max_level() {
        let mut m = new(ZIGZAGOON, 100, Ivs::Fixed(0), 0);
        assert_eq!(m.exp, 1_000_000);
        let mut exp = 500;
        assert!(!m.add_exp_step(&TABLES, &mut exp));
        assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 0));
        // 99 -> 100: 970299 + 1000000 >= 1000000; 29701 used, the rest is dropped next step.
        let mut m = new(ZIGZAGOON, 99, Ivs::Fixed(0), 0);
        let mut exp = 1_000_000;
        assert!(m.add_exp_step(&TABLES, &mut exp));
        assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 1_000_000 - 29_701));
        assert!(!m.add_exp_step(&TABLES, &mut exp));
        assert_eq!((m.level, m.exp, exp), (100, 1_000_000, 0));
    }

    #[test]
    fn try_learn_and_replace() {
        let mut m = new(SPARK, 5, Ivs::Fixed(0), 0);
        assert_eq!(m.try_learn(&TABLES, BIDE), Learn::AlreadyKnown);
        assert_eq!(m.try_learn(&TABLES, TACKLE), Learn::Full);
        m.pp_bonuses = 0xFF;
        m.pp[1] = 3;
        m.replace_move(&TABLES, 1, TACKLE);
        assert_eq!(m.moves[1], Some(TACKLE));
        assert_eq!(m.pp[1], 35);
        // RemoveMonPPBonus clears the slot's two bits.
        assert_eq!(m.pp_bonuses, 0b1111_0011);
        let mut z = new(ZIGZAGOON, 2, Ivs::Fixed(0), 0);
        assert_eq!(z.try_learn(&TABLES, TACKLE), Learn::AlreadyKnown);
        assert_eq!(z.try_learn(&TABLES, GROWL), Learn::Learned(1));
        assert_eq!(z.pp[1], 40);
    }

    #[test]
    fn heal_restores_hp_pp_with_bonuses_and_status() {
        let mut m = new(MUDKIP, 5, Ivs::Fixed(10), 3);
        m.hp = 1;
        m.pp = [0, 0, 0, 0];
        m.status = Status::Poison;
        // CalculatePPWithBonus: slot 0 has 3 PP Ups: 35 + 35*20*3/100 = 56; slot 1 has 1:
        // 40 + 40*20/100 = 48.
        m.pp_bonuses = 0b0000_0111;
        m.heal(&TABLES);
        assert_eq!(m.hp, 20);
        assert_eq!(m.pp, [56, 48, 0, 0]);
        assert_eq!(m.status, Status::None);
    }

    #[test]
    fn ev_gain_respects_both_caps() {
        // MonGainEVs, BLOB yields 3 HP, 2 Atk.
        let mut m = new(MUDKIP, 5, Ivs::Fixed(0), 0);
        m.gain_evs(&TABLES, BLOB);
        assert_eq!(m.evs, [3, 2, 0, 0, 0, 0]);
        // Per-stat cap 255.
        m.evs = [254, 0, 0, 0, 0, 0];
        m.gain_evs(&TABLES, BLOB);
        assert_eq!(m.evs, [255, 2, 0, 0, 0, 0]);
        // Total cap 510: 509 + min(3, 1) on HP, then the loop stops.
        m.evs = [100, 255, 154, 0, 0, 0];
        m.gain_evs(&TABLES, BLOB);
        assert_eq!(m.evs, [101, 255, 154, 0, 0, 0]);
        m.evs = [255, 255, 0, 0, 0, 0];
        m.gain_evs(&TABLES, BLOB);
        assert_eq!(m.evs, [255, 255, 0, 0, 0, 0]);
        // EVs only count once the stats are recalculated:
        // HP ((100 + 0 + 252/4) * 5)/100 + 15 = 23.
        m.evs = [252, 0, 0, 0, 0, 0];
        let hp = m.stats[0];
        m.recalc_stats(&TABLES);
        assert_eq!((hp, m.stats[0]), (20, 23));
    }

    #[test]
    fn exp_tables_hand_values() {
        use GrowthRate::*;
        // EXP_MEDIUM_SLOW(n) = 6n^3/5 - 15n^2 + 100n - 140.
        assert_eq!(
            [5, 6, 7, 8].map(|l| exp_for_level(MediumSlow, l)),
            [135, 179, 236, 314]
        );
        assert_eq!(
            [2, 3, 5].map(|l| exp_for_level(MediumFast, l)),
            [8, 27, 125]
        );
        // Levels 0 and 1 are 0 and 1 in every table (MediumSlow L1 would be -54).
        for g in [MediumFast, Erratic, Fluctuating, MediumSlow, Fast, Slow] {
            assert_eq!((exp_for_level(g, 0), exp_for_level(g, 1)), (0, 1));
        }
        // EXP_ERRATIC(60) = (150 - 60) * 216000 / 100 = 194400;
        // EXP_ERRATIC(80) = ((1911 - 800) / 3) * 512000 / 500 = 370 * 1024 = 378880.
        assert_eq!(exp_for_level(Erratic, 60), 194_400);
        assert_eq!(exp_for_level(Erratic, 80), 378_880);
        assert_eq!(exp_for_level(Erratic, 100), 600_000);
        // EXP_FLUCTUATING(20) = (20 + 14) * 8000 / 50 = 5440; (10) = (11/3 + 24) * 1000 / 50 = 540.
        assert_eq!(exp_for_level(Fluctuating, 20), 5440);
        assert_eq!(exp_for_level(Fluctuating, 10), 540);
        assert_eq!(exp_for_level(Fluctuating, 100), 1_640_000);
        // EXP_FAST(10) = 4000/5 = 800, EXP_SLOW(10) = 5000/4 = 1250.
        assert_eq!(exp_for_level(Fast, 10), 800);
        assert_eq!(exp_for_level(Slow, 10), 1250);
        assert_eq!(exp_for_level(MediumSlow, 100), 1_059_860);
    }

    #[test]
    fn level_for_exp_inverts_the_table() {
        use GrowthRate::*;
        assert_eq!(level_for_exp(MediumSlow, 134), 4);
        assert_eq!(level_for_exp(MediumSlow, 135), 5);
        assert_eq!(level_for_exp(MediumSlow, 178), 5);
        assert_eq!(level_for_exp(MediumFast, u32::MAX), 100);
        assert_eq!(level_for_exp(MediumFast, 1), 1);
    }

    /// Every level of every rate against `gExperienceTables`, evaluated from the decomp's
    /// `experience_tables.h` by a small C-expression evaluator (skipped without `PKR_DECOMP`).
    #[test]
    fn exp_tables_match_the_decomp() {
        let Ok(dir) = std::env::var("PKR_DECOMP") else {
            return;
        };
        let src = std::fs::read_to_string(
            std::path::Path::new(&dir).join("src/data/pokemon/experience_tables.h"),
        )
        .unwrap();
        let tables = decomp_exp::tables(&src);
        let rates = [
            GrowthRate::MediumFast,
            GrowthRate::Erratic,
            GrowthRate::Fluctuating,
            GrowthRate::MediumSlow,
            GrowthRate::Fast,
            GrowthRate::Slow,
        ];
        assert!(tables.len() >= 6, "{} rows", tables.len());
        for (g, row) in rates.iter().zip(&tables) {
            assert_eq!(row.len(), 101, "{g:?}");
            for (level, &want) in row.iter().enumerate() {
                assert_eq!(
                    exp_for_level(*g, level as u8) as i64,
                    want,
                    "{g:?} L{level}"
                );
            }
        }
    }

    /// Test-side oracle: macros (`#define NAME(n) body`, with `\` continuations) and the
    /// `gExperienceTables` rows, evaluated with C integer semantics.
    mod decomp_exp {
        use std::collections::BTreeMap;
        use std::string::{String, ToString};
        use std::vec::Vec;

        type Macros = BTreeMap<String, (String, String)>;

        pub fn tables(src: &str) -> Vec<Vec<i64>> {
            let joined = src.replace("\\\n", " ");
            let mut macros = Macros::new();
            for line in joined.lines() {
                let Some(rest) = line.trim().strip_prefix("#define ") else {
                    continue;
                };
                let open = rest.find('(').unwrap();
                let close = rest.find(')').unwrap();
                let body = rest[close + 1..].split("//").next().unwrap().trim();
                macros.insert(
                    rest[..open].into(),
                    (rest[open + 1..close].trim().into(), body.into()),
                );
            }
            let start = joined.find("gExperienceTables").unwrap();
            let after = &joined[start..];
            let body = &after[after.find('{').unwrap() + 1..];
            let mut rows = Vec::new();
            for row in body.split('}').take_while(|r| r.contains('{')) {
                let row = &row[row.find('{').unwrap() + 1..];
                let mut vals = Vec::new();
                for line in row.lines() {
                    let e = line.split("//").next().unwrap().trim();
                    let e = e.trim_end_matches(',').trim();
                    if !e.is_empty() {
                        vals.push(eval(e, &macros, &BTreeMap::new()));
                    }
                }
                rows.push(vals);
            }
            rows
        }

        fn eval(e: &str, m: &Macros, vars: &BTreeMap<String, i64>) -> i64 {
            let toks = lex(e);
            let mut p = P {
                t: &toks,
                i: 0,
                m,
                vars,
            };
            let v = p.ternary();
            assert_eq!(p.i, toks.len(), "trailing tokens in {e}");
            v
        }

        fn lex(e: &str) -> Vec<String> {
            let mut out = Vec::new();
            let b = e.as_bytes();
            let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
            let mut i = 0;
            while i < b.len() {
                if b[i].is_ascii_whitespace() {
                    i += 1;
                } else if word(b[i]) {
                    let s = i;
                    while i < b.len() && word(b[i]) {
                        i += 1;
                    }
                    out.push(e[s..i].into());
                } else if e[i..].starts_with("<=") {
                    out.push("<=".into());
                    i += 2;
                } else {
                    out.push(e[i..i + 1].into());
                    i += 1;
                }
            }
            out
        }

        struct P<'a> {
            t: &'a [String],
            i: usize,
            m: &'a Macros,
            vars: &'a BTreeMap<String, i64>,
        }

        impl P<'_> {
            fn peek(&self) -> &str {
                self.t.get(self.i).map_or("", |s| s.as_str())
            }

            fn eat(&mut self, s: &str) -> bool {
                let hit = self.peek() == s;
                self.i += hit as usize;
                hit
            }

            fn ternary(&mut self) -> i64 {
                let c = self.cmp();
                if self.eat("?") {
                    let a = self.ternary();
                    assert!(self.eat(":"));
                    let b = self.ternary();
                    if c != 0 { a } else { b }
                } else {
                    c
                }
            }

            fn cmp(&mut self) -> i64 {
                let a = self.add();
                if self.eat("<=") {
                    (a <= self.add()) as i64
                } else {
                    a
                }
            }

            fn add(&mut self) -> i64 {
                let mut v = self.mul();
                loop {
                    if self.eat("+") {
                        v += self.mul();
                    } else if self.eat("-") {
                        v -= self.mul();
                    } else {
                        return v;
                    }
                }
            }

            fn mul(&mut self) -> i64 {
                let mut v = self.atom();
                loop {
                    if self.eat("*") {
                        v *= self.atom();
                    } else if self.eat("/") {
                        v /= self.atom(); // C truncation
                    } else {
                        return v;
                    }
                }
            }

            fn atom(&mut self) -> i64 {
                let tok = self.peek().to_string();
                self.i += 1;
                if tok == "(" {
                    let v = self.ternary();
                    assert!(self.eat(")"));
                    return v;
                }
                if let Ok(v) = tok.parse() {
                    return v;
                }
                if let Some(v) = self.vars.get(&tok) {
                    return *v;
                }
                let (param, body) = self.m.get(&tok).unwrap_or_else(|| panic!("unknown {tok}"));
                assert!(self.eat("("));
                let arg = self.ternary();
                assert!(self.eat(")"));
                let mut vars = BTreeMap::new();
                vars.insert(param.clone(), arg);
                eval(body, self.m, &vars)
            }
        }
    }
}
