//! Gen3 Pokémon model. The numbers (species, moves, learnsets) are data that content fills in;
//! the formulas are Gen3's (`pokemon.c`).
//!
//! Formulas: `CreateMon`, `CalculateMonStats`, `MonGainEVs`, `gExperienceTables`, …

use crate::data::{EvoMethod, Evolution, ItemId, MoveId, SpeciesId};
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
    /// Not a Gen3 type: the Gen6 Fairy type, for content that uses it (Silice's Mega forms).
    /// Its matchups are [`FAIRY_MATCHUPS`], which only such content appends to its chart, so a
    /// Gen3 game never meets it. It sits after `Mystery`, so the Gen3 physical/special split by
    /// type ([`Type::is_physical`]) makes it special, as later fan Gen3 hacks do.
    Fairy,
}

impl Type {
    /// Every Gen3 type, in `TYPE_*` order (`Type::ALL[t as usize] == t`). [`Type::Fairy`] is
    /// not in it (see [`Type::EVERY`]).
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

    /// [`Type::ALL`] then [`Type::Fairy`] (`Type::EVERY[t as usize] == t`).
    pub const EVERY: [Type; 19] = {
        let mut all = [Type::Fairy; 19];
        let mut i = 0;
        while i < 18 {
            all[i] = Type::ALL[i];
            i += 1;
        }
        all
    };

    /// Gen3 `IS_TYPE_PHYSICAL`: the types before `Mystery` use Atk/Def.
    pub const fn is_physical(self) -> bool {
        (self as u8) < Type::Mystery as u8
    }

    /// The type of a Gen3 `TYPE_*` number (never [`Type::Fairy`]).
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
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
    // Task G (appended: the discriminants above are hashed and coded in `Str::AbilityName`).
    // Abilities without an effect in the engine's battles (no weather, no items after battle)
    // are named so Trace and the messages that name an ability print them.
    Chlorophyll,
    CompoundEyes,
    CuteCharm,
    EarlyBird,
    HugePower,
    InnerFocus,
    MagnetPull,
    Pickup,
    /// Max HP / 16 at the end of the turn in rain (`ABILITYEFFECT_ENDTURN`; only the gated
    /// abilities bring rain).
    RainDish,
    RockHead,
    ShedSkin,
    Soundproof,
    Sturdy,
    SwiftSwim,
    ThickFat,
    Truant,
    Oblivious,
    Insomnia,
    VitalSpirit,
    Hustle,
    MarvelScale,
    PurePower,
    ClearBody,
    LiquidOoze,
    NaturalCure,
    // Silice engine (appended). Gen3 abilities whose Emerald holders the content leaves
    // `Other` until a deliberate route change (`GEN3_EXTRA_ABILITIES` in the content build):
    /// Sun at switch-in (`ABILITYEFFECT_ON_SWITCH_IN`, permanent).
    Drought,
    /// Rain at switch-in.
    Drizzle,
    /// Sandstorm at switch-in.
    SandStream,
    /// Speed +1 at the end of each turn but its first.
    SpeedBoost,
    /// The foe cannot run nor switch (Gen3: even a Shadow Tag foe; see [`Self::ShadowTagGen4`]).
    ShadowTag,
    /// Ground moves miss.
    Levitate,
    /// No critical hits against it.
    ShellArmor,
    BattleArmor,
    /// Gen5: draws Electric moves, which it absorbs for Sp. Atk +1. Gen3's only redirects in
    /// doubles (nothing in singles): the content maps it here only under its Silice switch.
    LightningRod,
    // Post-Gen3 abilities (Gen7 behaviour adapted to Gen3, see the Silice plan).
    /// Heavy rain while it is out: Fire moves fail.
    PrimordialSea,
    /// Extremely harsh sun while it is out: Water moves fail.
    DesolateLand,
    /// Normal moves become Flying, x1.2.
    Aerilate,
    /// Normal moves become Fairy, x1.2.
    Pixilate,
    /// Normal moves become Ice, x1.2.
    Refrigerate,
    /// Contact moves x1.3.
    ToughClaws,
    /// Reflects `MAGIC_COAT_AFFECTED` moves.
    MagicBounce,
    /// A second hit at a quarter of the damage.
    ParentalBond,
    /// Ignores the target's breakable abilities.
    MoldBreaker,
    /// Moves of power 60 or less x1.5.
    Technician,
    /// Multi-hit moves hit 5 times.
    SkillLink,
    /// Rock / Ground / Steel moves x1.3 in a sandstorm; no sandstorm damage.
    SandForce,
    /// Sp. Atk x1.5 in the sun, losing 1/8 HP each turn.
    SolarPower,
    /// Super-effective damage x3/4.
    Filter,
    /// Status moves +1 priority (they fail against a Dark target).
    Prankster,
    /// Speed +1 when it flinches.
    Steadfast,
    // Gen3 (gated as the abilities above).
    /// No sandstorm damage; the foe's accuracy x80/100 in a sandstorm.
    SandVeil,
    // Silice rules for Gen3 abilities (the content's Silice switch).
    /// Shadow Tag with Gen4's exemption: a foe with Shadow Tag (Gen4) is not trapped.
    ShadowTagGen4,
    // Off-grass encounters (appended): the lead abilities of Gen3 `WildEncounterCheck`.
    /// A lead halves the encounter rate. No battle effect in Gen3.
    Stench,
    /// A lead doubles the encounter rate. No battle effect in Gen3.
    Illuminate,
    /// A lead halves the encounter rate. Its battle effect (Clear Body's) is not modelled, so
    /// the content leaves its holders `Other` (see `classify::battle::ability_with`).
    WhiteSmoke,
    /// A lead doubles the encounter rate. Its battle effect (trapping) is not modelled, so the
    /// content leaves its holders `Other`.
    ArenaTrap,
    /// A lead gets the wild table's top level half the time (as Hustle and Vital Spirit). Its
    /// battle effect (double PP use) is not modelled, so the content leaves its holders `Other`.
    Pressure,
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
    // Task G.
    /// `EFFECT_ATTRACT`.
    Attract,
    /// `EFFECT_MEAN_LOOK` (Mean Look, Block, Spider Web).
    MeanLook,
    /// `EFFECT_MUD_SPORT`: Electric power halved while the user is out.
    MudSport,
    /// `EFFECT_WATER_SPORT`: Fire power halved while the user is out.
    WaterSport,
    /// `EFFECT_TELEPORT`: flees a wild battle.
    Teleport,
    /// `EFFECT_UPROAR`: 2–5 locked turns; nobody sleeps meanwhile.
    Uproar,
    /// `EFFECT_YAWN`: the target falls asleep at the end of the next turn.
    Yawn,
    /// `EFFECT_DOUBLE_HIT` (Double Kick): two hits.
    DoubleHit,
    /// `EFFECT_FORESIGHT`.
    Foresight,
    /// `EFFECT_FURY_CUTTER`: power doubles with each consecutive hit (up to 5).
    FuryCutter,
    /// `EFFECT_PURSUIT`: a hit, doubled and first against a foe switching out.
    Pursuit,
    /// `EFFECT_LOW_KICK`: power from the target's weight.
    LowKick,
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
    /// Not a Gen3 flag: the move is in `sSoundMovesTable` (blocked by Soundproof).
    pub const SOUND: u8 = 1 << 6;

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
    /// Pokédex weight in hectograms (`GetPokedexHeightWeight`, Low Kick); 0 when the content
    /// does not carry it yet.
    pub weight: u16,
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

const fn mu(attack: Type, defend: Type, multiplier: u8) -> TypeMatchup {
    TypeMatchup {
        attack,
        defend,
        multiplier,
    }
}

/// The Gen6 Fairy matchups (attacking: x2 on Fighting / Dragon / Dark, x0.5 on Fire / Poison /
/// Steel; defending: x0.5 from Fighting / Bug / Dark, x0 from Dragon, x2 from Poison / Steel),
/// for content that has [`Type::Fairy`] to append to its chart before Foresight's entries. A
/// Gen3 chart never contains them.
pub const FAIRY_MATCHUPS: [TypeMatchup; 12] = [
    mu(Type::Fairy, Type::Fighting, 20),
    mu(Type::Fairy, Type::Dragon, 20),
    mu(Type::Fairy, Type::Dark, 20),
    mu(Type::Fairy, Type::Fire, 5),
    mu(Type::Fairy, Type::Poison, 5),
    mu(Type::Fairy, Type::Steel, 5),
    mu(Type::Fighting, Type::Fairy, 5),
    mu(Type::Bug, Type::Fairy, 5),
    mu(Type::Dark, Type::Fairy, 5),
    mu(Type::Dragon, Type::Fairy, 0),
    mu(Type::Poison, Type::Fairy, 20),
    mu(Type::Steel, Type::Fairy, 20),
];

/// The battle effect of a held item: Gen3 `HOLD_EFFECT_*` (`include/constants/hold_effects.h`)
/// with its `holdEffectParam`, for the effects the engine models (`ItemBattleEffects`,
/// `battle_util.c`). Content maps `gItems[]` to it (`GameData::hold_effect`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HoldEffect {
    /// `HOLD_EFFECT_RESTORE_HP` (Oran 10, Sitrus 30, Berry Juice 20): heals `param` HP once at
    /// or below half HP.
    RestoreHp(u8),
    /// `HOLD_EFFECT_CURE_PAR` (Cheri).
    CurePar,
    /// `HOLD_EFFECT_CURE_SLP` (Chesto).
    CureSlp,
    /// `HOLD_EFFECT_CURE_PSN` (Pecha).
    CurePsn,
    /// `HOLD_EFFECT_CURE_BRN` (Rawst).
    CureBrn,
    /// `HOLD_EFFECT_CURE_FRZ` (Aspear).
    CureFrz,
    /// `HOLD_EFFECT_CURE_CONFUSION` (Persim).
    CureConfusion,
    /// `HOLD_EFFECT_CURE_STATUS` (Lum): any status, then confusion.
    CureStatus,
    /// `HOLD_EFFECT_LEFTOVERS`: max HP / 16 at the end of each turn.
    Leftovers,
    /// The `sHoldEffectToType` items (Charcoal, Mystic Water …): moves of the type get
    /// `(100 + param)%` of the user's attacking stat.
    TypePower(Type, u8),
    /// `HOLD_EFFECT_CHOICE_BAND`: Attack x1.5 (the move lock is not modelled).
    ChoiceBand,
    /// `HOLD_EFFECT_QUICK_CLAW`: moves first when `gRandomTurnNumber < 0xFFFF * param / 100`.
    QuickClaw(u8),
    /// `HOLD_EFFECT_FLINCH` (King's Rock): `param`% flinch on a `KINGS_ROCK_AFFECTED` hit.
    KingsRock(u8),
    /// `HOLD_EFFECT_EVASION_UP` (BrightPowder 10, Lax Incense 5): the foe's accuracy x
    /// `(100 - param)%`.
    EvasionUp(u8),
    /// `HOLD_EFFECT_FOCUS_BAND`: `param`% to survive a hit at 1 HP.
    FocusBand(u8),
    /// `HOLD_EFFECT_REPEL` (Cleanse Tag): held by the lead, the land encounter rate x2/3
    /// (Gen3 `ApplyCleanseTagEncounterRateMod`, which tests `ITEM_CLEANSE_TAG`, the only item
    /// with this effect). No battle effect.
    CleanseTag,
}

impl HoldEffect {
    /// A stable hash code: tag and parameter.
    pub const fn code(self) -> [u8; 2] {
        use HoldEffect::*;
        match self {
            RestoreHp(p) => [1, p],
            CurePar => [2, 0],
            CureSlp => [3, 0],
            CurePsn => [4, 0],
            CureBrn => [5, 0],
            CureFrz => [6, 0],
            CureConfusion => [7, 0],
            CureStatus => [8, 0],
            Leftovers => [9, 0],
            TypePower(t, p) => [10 + t as u8, p],
            ChoiceBand => [40, 0],
            QuickClaw(p) => [41, p],
            KingsRock(p) => [42, p],
            EvasionUp(p) => [43, p],
            FocusBand(p) => [44, p],
            CleanseTag => [45, 0],
        }
    }
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
    /// `gTrainers[].party[].heldItem`. Emerald content leaves it `None` (its build flag
    /// `TRAINER_HELD_ITEMS`, see the Silice plan): content that sets it moves the battles of
    /// the trainers that hold items.
    pub item: Option<ItemId>,
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
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
    /// `MON_DATA_HELD_ITEM`. Not in the derived serde form: the checkpoint carries it in
    /// `WorldSnapshot::party_held` (schema 3), so a party without items encodes as schema 2.
    /// Hashed only when `Some`.
    #[serde(skip)]
    pub held: Option<ItemId>,
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

/// `GetSpeciesName` into a nickname: up to `POKEMON_NAME_LENGTH` characters, then EOS.
fn species_nickname(info: &SpeciesInfo) -> [u8; NAME_LEN] {
    let mut nickname = [0xFF; NAME_LEN];
    let name = info
        .name
        .iter()
        .take(NAME_LEN - 1)
        .take_while(|&&c| c != 0xFF);
    for (d, &c) in nickname.iter_mut().zip(name) {
        *d = c;
    }
    nickname
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
        let nickname = species_nickname(info);
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
            held: None,
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

    /// Gen3 `GetEvolutionTargetSpecies(mon, EVO_MODE_NORMAL, ..)` over `evos` (the species'
    /// `gEvolutionTable` row), level methods only: the last entry whose level is reached wins,
    /// as Gen3's loop. Friendship, item and trade methods never match here (M9). The content
    /// contract's `Level` may fold several Gen3 level methods (the stat and personality
    /// variants, such as Wurmple's and Tyrogue's): the second value is true when more than one
    /// level entry matched, a choice the contract cannot make as Gen3 does (the caller reports
    /// it). Whatever a `Level` entry stands for, it is taken at its level, as `EVO_LEVEL`.
    pub fn evolution_target(&self, evos: &[Evolution]) -> Option<(SpeciesId, bool)> {
        let mut found = None;
        let mut matches = 0;
        for e in evos {
            if e.method == EvoMethod::Level && e.param <= self.level as u16 {
                found = Some(e.target);
                matches += 1;
            }
        }
        found.map(|t| (t, matches > 1))
    }

    /// The species change of Gen3 `EvolutionScene`, without the scene: `EvolutionRenameMon`
    /// (a nickname equal to the old species name becomes the new one), then
    /// `CalculateMonStats` (the current HP moves with the max HP). The new species' moves at
    /// this level are not learned (the scene's `MonTryLearningNewMove`, M9).
    pub fn evolve(&mut self, t: &BattleTables, target: SpeciesId) {
        if self.nickname == species_nickname(t.species_info(self.species)) {
            self.nickname = species_nickname(t.species_info(target));
        }
        self.species = target;
        self.recalc_stats(t);
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
        // Only when set: a Gen3 party without items keeps its hashes.
        if let Some(item) = self.held {
            h.write(&[0xA1]);
            h.write_u16(item.0);
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
mod tests;

#[cfg(test)]
mod model_tests;
