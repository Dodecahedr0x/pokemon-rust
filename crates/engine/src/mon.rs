//! Gen3 Pokémon model. The numbers (species, moves, learnsets) are data that content fills in;
//! the formulas are Gen3's (`pokemon.c`).
//!
//! Wave 0 contract: types are complete. `Pokemon` formula bodies are Task C (`todo!`).

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

impl Pokemon {
    /// Gen3 `CreateMon` + `GiveMonInitialMoveset` (`pokemon.c`). Task C.
    #[allow(unused_variables)]
    pub fn new(
        t: &BattleTables,
        species: SpeciesId,
        level: u8,
        ivs: Ivs,
        personality: u32,
        ot_id: u32,
        rng: &mut Rng,
    ) -> Pokemon {
        todo!("Task C: CreateMon + GiveMonInitialMoveset")
    }

    /// `personality % 25` (`GetNature`). Task C.
    pub fn nature(&self) -> Nature {
        todo!("Task C: GetNature")
    }

    /// Gen3 `GetGenderFromSpeciesAndPersonality`. Task C.
    #[allow(unused_variables)]
    pub fn gender(&self, t: &BattleTables) -> Gender {
        todo!("Task C: GetGenderFromSpeciesAndPersonality")
    }

    /// Gen3 `GetAbilityBySpecies` with `ability_slot`. Task C.
    #[allow(unused_variables)]
    pub fn ability(&self, t: &BattleTables) -> Ability {
        todo!("Task C: GetAbilityBySpecies")
    }

    /// Gen3 `IsShinyOtIdPersonality`. Task C.
    pub fn is_shiny(&self) -> bool {
        todo!("Task C: IsShinyOtIdPersonality")
    }

    /// Gen3 `CalculateMonStats` (`CALC_STAT`, `ModifyStatByNature`). Task C.
    #[allow(unused_variables)]
    pub fn recalc_stats(&mut self, t: &BattleTables) {
        todo!("Task C: CalculateMonStats")
    }

    /// Restores HP, PP (with `pp_bonuses`) and status (Gen3 `HealPlayerParty`). Task C.
    #[allow(unused_variables)]
    pub fn heal(&mut self, t: &BattleTables) {
        todo!("Task C: HealPlayerParty for one mon")
    }

    /// Gen3 `MonGainEVs`, no items. Task C.
    #[allow(unused_variables)]
    pub fn gain_evs(&mut self, t: &BattleTables, defeated: SpeciesId) {
        todo!("Task C: MonGainEVs")
    }

    /// Adds exp up to the next level boundary; returns true if a level was gained (stats are
    /// recalculated). Callers loop, so each level-up gets its own events. Gen3 `Cmd_getexp`
    /// level loop + `CalculateMonStats`. Task C.
    #[allow(unused_variables)]
    pub fn add_exp_step(&mut self, t: &BattleTables, exp: &mut u32) -> bool {
        todo!("Task C: Cmd_getexp level step")
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

    /// Gen3 `MonTryLearningNewMove` for one move. Task C.
    #[allow(unused_variables)]
    pub fn try_learn(&mut self, t: &BattleTables, m: MoveId) -> Learn {
        todo!("Task C: MonTryLearningNewMove")
    }

    /// Replaces the move in `slot` and resets its PP (Gen3 `SetMonMoveSlot`). Task C.
    #[allow(unused_variables)]
    pub fn replace_move(&mut self, t: &BattleTables, slot: u8, m: MoveId) {
        todo!("Task C: SetMonMoveSlot")
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

/// Total exp needed to reach `level` (Gen3 `gExperienceTables`, `experience_tables.h` macros).
/// Task C.
#[allow(unused_variables)]
pub fn exp_for_level(g: GrowthRate, level: u8) -> u32 {
    todo!("Task C: gExperienceTables formulas")
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
