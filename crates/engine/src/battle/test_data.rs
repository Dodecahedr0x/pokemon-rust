//! Synthetic battle tables shared by the engine's battle unit tests: a few species and moves,
//! a type chart subset in Gen3 order, and a `GameData` that serves them. Other tasks may extend
//! it (append: ids are indices, existing tests pin them).
//!
//! Battle test mons are built with explicit stats ([`mon`]) so tests do not depend on the stat
//! formulas. The battle species use the MediumFast growth rate (`exp = level³`) to keep exp
//! goldens easy; the Pokémon model species ([`MUDKIP`] on) mirror a few Gen3 species for the
//! stat, growth, gender and ability goldens of `mon`, `wild` and `trainer`.

use crate::battle::Str;
use crate::data::Dir;
use crate::data::{
    Behavior, Font, GameData, GfxId, MapData, MapId, MoveId, NewGame, ScriptId, SpeciesId,
    SpriteData, TextId, UiText, WindowGfx,
};
use crate::mon::{
    Ability, BattleTables, GrowthRate, LevelUpMove, MoveEffect, MoveFlags, MoveInfo, MoveTarget,
    NAME_LEN, Party, Pokemon, SemiKind, SpeciesInfo, Stat, Status, StatusKind, TrapKind,
    TwoTurnKind, Type, TypeMatchup, WeatherKind,
};
use crate::script::{Ctx, Script};

// ---- moves (MoveId = index) ----
pub const TACKLE: MoveId = MoveId(1);
pub const GROWL: MoveId = MoveId(2);
pub const EMBER: MoveId = MoveId(3);
pub const STRUGGLE: MoveId = MoveId(4);
pub const QUICK_ATTACK: MoveId = MoveId(5);
pub const HARDEN: MoveId = MoveId(6);
pub const WATER_GUN: MoveId = MoveId(7);
pub const SWORDS_DANCE: MoveId = MoveId(8);
pub const TAKE_DOWN: MoveId = MoveId(9);
/// A status move whose effect the engine does not implement (`Unsupported`).
pub const SPLASH: MoveId = MoveId(10);
/// A 250-power Normal hit, for quick knock-outs.
pub const MEGA_HIT: MoveId = MoveId(11);
/// Defense -2.
pub const SCREECH: MoveId = MoveId(12);
/// A Normal move that never misses in practice (accuracy 100) with priority -1.
pub const SLOW_HIT: MoveId = MoveId(13);
/// An unsupported damaging effect with accuracy 0 (an always-hit move, as Gen3 Swift).
pub const SWIFT: MoveId = MoveId(14);
// Task E2 (Gen3 data of the namesakes).
pub const ABSORB: MoveId = MoveId(15);
/// Accuracy -1 on hit, chance 100.
pub const MUD_SLAP: MoveId = MoveId(16);
pub const SAND_ATTACK: MoveId = MoveId(17);
/// Poison 30%.
pub const POISON_STING: MoveId = MoveId(18);
pub const STUN_SPORE: MoveId = MoveId(19);
pub const SUPERSONIC: MoveId = MoveId(20);
/// Confusion 10%.
pub const CONFUSION: MoveId = MoveId(21);
/// Flinch 30%, contact.
pub const BITE: MoveId = MoveId(22);
pub const FOCUS_ENERGY: MoveId = MoveId(23);
pub const LEECH_SEED: MoveId = MoveId(24);
pub const BIDE: MoveId = MoveId(25);
/// Calls `TABLES.nature_power` (Swift everywhere).
pub const NATURE_POWER: MoveId = MoveId(26);
pub const SLEEP_POWDER: MoveId = MoveId(27);
pub const POISON_POWDER: MoveId = MoveId(28);
pub const DOUBLE_SLAP: MoveId = MoveId(29);
pub const SONIC_BOOM: MoveId = MoveId(30);
pub const CHARGE: MoveId = MoveId(31);
pub const THUNDER_SHOCK: MoveId = MoveId(32);
pub const DEFENSE_CURL: MoveId = MoveId(33);
pub const VITAL_THROW: MoveId = MoveId(34);
pub const TOXIC: MoveId = MoveId(35);
/// Freeze 100% (chance roll still drawn), Ice.
pub const FREEZE_HIT: MoveId = MoveId(36);
/// A Splash that the engine implements (`MoveEffect::Splash`).
pub const REAL_SPLASH: MoveId = MoveId(37);
/// Flinch 100%, contact (Bite with a sure flinch).
pub const FLINCH_ALL: MoveId = MoveId(38);
/// Confusion 100%.
pub const CONFUSE_ALL: MoveId = MoveId(39);
/// Poison 100%.
pub const POISON_ALL: MoveId = MoveId(40);
// The Pokémon model, wild and trainer tests (formerly `mon::test_tables`).
/// Defense -1, PP 30.
pub const TAIL_WHIP: MoveId = MoveId(41);
// Task G (Gen3 data of the namesakes).
pub const ATTRACT: MoveId = MoveId(42);
/// Mean Look's effect.
pub const BLOCK: MoveId = MoveId(43);
pub const MUD_SPORT: MoveId = MoveId(44);
pub const WATER_SPORT: MoveId = MoveId(45);
pub const TELEPORT: MoveId = MoveId(46);
/// Sound.
pub const UPROAR: MoveId = MoveId(47);
pub const YAWN: MoveId = MoveId(48);
pub const DOUBLE_KICK: MoveId = MoveId(49);
pub const FORESIGHT: MoveId = MoveId(50);
pub const FURY_CUTTER: MoveId = MoveId(51);
pub const PURSUIT: MoveId = MoveId(52);
/// Growl with the sound flag (Soundproof blocks it).
pub const SOUND_GROWL: MoveId = MoveId(53);
/// Speed -1 secondary with a 100% chance (Icy Wind).
pub const ICY_WIND: MoveId = MoveId(54);
/// Low Kick: power from the target's weight.
pub const LOW_KICK: MoveId = MoveId(55);
// Wave C (Gen3 data of the namesakes, PP 10).
/// A plain 40-power Normal hit that Protect blocks and Mirror Move copies.
pub const POUND: MoveId = MoveId(56);
pub const ROLLOUT: MoveId = MoveId(57);
pub const THRASH: MoveId = MoveId(58);
pub const HYPER_BEAM: MoveId = MoveId(59);
pub const SOLAR_BEAM: MoveId = MoveId(60);
pub const FLY: MoveId = MoveId(61);
pub const SUPER_FANG: MoveId = MoveId(62);
pub const DRAGON_RAGE: MoveId = MoveId(63);
pub const SEISMIC_TOSS: MoveId = MoveId(64);
pub const COUNTER: MoveId = MoveId(65);
pub const CURSE: MoveId = MoveId(66);
pub const SWAGGER: MoveId = MoveId(67);
pub const REST: MoveId = MoveId(68);
pub const RECOVER: MoveId = MoveId(69);
pub const HAZE: MoveId = MoveId(70);
pub const BELLY_DRUM: MoveId = MoveId(71);
pub const PAIN_SPLIT: MoveId = MoveId(72);
pub const DISABLE: MoveId = MoveId(73);
pub const ENCORE: MoveId = MoveId(74);
pub const TAUNT: MoveId = MoveId(75);
pub const TORMENT: MoveId = MoveId(76);
pub const PROTECT: MoveId = MoveId(77);
pub const REFLECT: MoveId = MoveId(78);
pub const SAFEGUARD: MoveId = MoveId(79);
pub const SPIKES: MoveId = MoveId(80);
pub const RAIN_DANCE: MoveId = MoveId(81);
pub const HAIL: MoveId = MoveId(82);
pub const MIST: MoveId = MoveId(83);
pub const METRONOME: MoveId = MoveId(84);
pub const SLEEP_TALK: MoveId = MoveId(85);
pub const MIRROR_MOVE: MoveId = MoveId(86);
pub const HELPING_HAND: MoveId = MoveId(87);
pub const FOLLOW_ME: MoveId = MoveId(88);
pub const PERISH_SONG: MoveId = MoveId(89);
pub const FUTURE_SIGHT: MoveId = MoveId(90);
pub const WRAP: MoveId = MoveId(91);
pub const DESTINY_BOND: MoveId = MoveId(92);
pub const INGRAIN: MoveId = MoveId(93);
pub const WISH: MoveId = MoveId(94);
pub const FACADE: MoveId = MoveId(95);
pub const KNOCK_OFF: MoveId = MoveId(96);
pub const ROAR: MoveId = MoveId(97);
pub const OHKO: MoveId = MoveId(98);
pub const RAGE: MoveId = MoveId(99);
pub const ROLE_PLAY: MoveId = MoveId(100);
pub const SKILL_SWAP: MoveId = MoveId(101);
pub const GRUDGE: MoveId = MoveId(102);
pub const IMPRISON: MoveId = MoveId(103);
pub const MAGIC_COAT: MoveId = MoveId(104);
pub const TRICK: MoveId = MoveId(105);
// Wave D (Gen3 data of the namesakes, PP 10).
pub const SUBSTITUTE: MoveId = MoveId(106);
pub const BATON_PASS: MoveId = MoveId(107);
pub const TRANSFORM: MoveId = MoveId(108);
pub const MIMIC: MoveId = MoveId(109);
pub const SKETCH: MoveId = MoveId(110);
pub const RECYCLE: MoveId = MoveId(111);
pub const CONVERSION_2: MoveId = MoveId(112);
pub const SNATCH: MoveId = MoveId(113);
pub const BEAT_UP: MoveId = MoveId(114);
pub const PAY_DAY: MoveId = MoveId(115);
pub const THIEF: MoveId = MoveId(116);
/// Swords Dance with its Gen3 `FLAG_SNATCH_AFFECTED`.
pub const SNATCH_DANCE: MoveId = MoveId(117);
// Doubles (Task G: `MOVE_TARGET_BOTH`, `MOVE_TARGET_FOES_AND_ALLY`).
/// A 50-power Normal hit on both foes (Gen3 Surf's target).
pub const SPREAD_HIT: MoveId = MoveId(118);
/// Earthquake (Gen3 data, PP 10).
pub const QUAKE: MoveId = MoveId(119);

// ---- species (SpeciesId = index) ----
/// Water, Torrent (Mudkip-like).
pub const WATER: SpeciesId = SpeciesId(1);
/// Normal, catch rate 255, exp yield 60 (Zigzagoon-like).
pub const NORMAL: SpeciesId = SpeciesId(2);
/// Fire, Blaze (Torchic-like).
pub const FIRE: SpeciesId = SpeciesId(3);
/// Bug, Shield Dust (Wurmple-like).
pub const BUG: SpeciesId = SpeciesId(4);
/// Dark, Run Away, exp yield 55 (Poochyena-like).
pub const DARK: SpeciesId = SpeciesId(5);
/// Ghost (immune to Normal).
pub const GHOST: SpeciesId = SpeciesId(6);
// Task E2 (one ability each).
/// Grass, Effect Spore.
pub const SPORE: SpeciesId = SpeciesId(7);
/// Psychic, Synchronize.
pub const SYNC: SpeciesId = SpeciesId(8);
/// Dark, Intimidate.
pub const INTIM: SpeciesId = SpeciesId(9);
/// Psychic, Trace.
pub const TRACER: SpeciesId = SpeciesId(10);
/// Poison, Poison Point.
pub const POISON: SpeciesId = SpeciesId(11);
/// Flying, Keen Eye.
pub const BIRD: SpeciesId = SpeciesId(12);
/// Electric, Static.
pub const ELECTRIC: SpeciesId = SpeciesId(13);
/// Steel, no ability.
pub const STEEL: SpeciesId = SpeciesId(14);
/// Normal, Guts.
pub const GUTS: SpeciesId = SpeciesId(15);
// The Pokémon model, wild and trainer tests (formerly `mon::test_tables`): named, with Gen3
// numbers of a few species so the goldens can be checked against the decomp by hand.
/// Mudkip-like: base 50/70/50/40/50/50, MediumSlow, genderRatio 31, Torrent only.
pub const MUDKIP: SpeciesId = SpeciesId(16);
/// Zigzagoon-like: base 38/30/41/60/30/41, MediumFast, genderRatio 127.
pub const ZIGZAGOON: SpeciesId = SpeciesId(17);
/// Electric, male only, Static/Synchronize, a long early learnset.
pub const SPARK: SpeciesId = SpeciesId(18);
/// Genderless, Synchronize only, EV yield 3 HP / 2 Atk.
pub const BLOB: SpeciesId = SpeciesId(19);
/// Female only, Keen Eye.
pub const HAWK: SpeciesId = SpeciesId(20);
/// Intimidate only; its name (`LONG_NAME`) is 12 characters, over `POKEMON_NAME_LENGTH`.
pub const GROWLER: SpeciesId = SpeciesId(21);
// Task G (one ability each, `STATS`).
/// Normal, Oblivious.
pub const OBLIVIOUS: SpeciesId = SpeciesId(22);
/// Normal, Cute Charm.
pub const CHARMER: SpeciesId = SpeciesId(23);
/// Rock, Magnet Pull.
pub const MAGNET: SpeciesId = SpeciesId(24);
/// Normal, Soundproof.
pub const SOUNDPROOF: SpeciesId = SpeciesId(25);
/// Normal, Insomnia.
pub const INSOMNIAC: SpeciesId = SpeciesId(26);
/// Normal, Truant.
pub const TRUANT: SpeciesId = SpeciesId(27);
/// Bug, Shed Skin.
pub const SHEDDER: SpeciesId = SpeciesId(28);
/// Psychic, Inner Focus.
pub const FOCUSED: SpeciesId = SpeciesId(29);
/// Grass, Early Bird.
pub const EARLY: SpeciesId = SpeciesId(30);
/// Rock, Rock Head.
pub const ROCKHEAD: SpeciesId = SpeciesId(31);
/// Bug, Compound Eyes.
pub const EYES: SpeciesId = SpeciesId(32);
/// Water, Clear Body.
pub const CLEAR: SpeciesId = SpeciesId(33);
/// Water, Liquid Ooze.
pub const OOZE: SpeciesId = SpeciesId(34);
/// Normal, Natural Cure.
pub const CURER: SpeciesId = SpeciesId(35);
/// Normal, Pickup.
pub const PICKER: SpeciesId = SpeciesId(36);
/// Rock, no ability, weight 200 hectograms (Geodude's).
pub const HEAVY: SpeciesId = SpeciesId(37);
/// Gen3-encoded "MUDKIP", "ZIGZAGOON", "ABCDEFGHIJKL".
pub const MUDKIP_NAME: &[u8] = &[0xC7, 0xCF, 0xBE, 0xC5, 0xC3, 0xCA, 0xFF];
pub const ZIGZAGOON_NAME: &[u8] = &[0xD4, 0xC3, 0xC1, 0xD4, 0xBB, 0xC1, 0xC9, 0xC9, 0xC8, 0xFF];
pub const LONG_NAME: &[u8] = &[
    0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xFF,
];

const fn mv(
    effect: MoveEffect,
    power: u8,
    ty: Type,
    accuracy: u8,
    pp: u8,
    target: MoveTarget,
    priority: i8,
) -> MoveInfo {
    MoveInfo {
        name: &[0xFF],
        effect,
        power,
        ty,
        accuracy,
        pp,
        effect_chance: 0,
        target,
        priority,
        flags: MoveFlags(0),
    }
}

use MoveTarget::{Selected, User};

/// A Wave C move: PP 10, priority 0, `effect_chance` 0.
const fn wc(
    effect: MoveEffect,
    power: u8,
    ty: Type,
    accuracy: u8,
    target: MoveTarget,
    flags: u8,
) -> MoveInfo {
    MoveInfo {
        flags: MoveFlags(flags),
        ..mv(effect, power, ty, accuracy, 10, target, 0)
    }
}

/// A Task E2 move: `Selected`, priority 0, with its secondary chance and contact flag.
const fn e2(
    effect: MoveEffect,
    power: u8,
    ty: Type,
    accuracy: u8,
    pp: u8,
    effect_chance: u8,
    contact: bool,
) -> MoveInfo {
    MoveInfo {
        effect_chance,
        flags: MoveFlags(if contact { MoveFlags::MAKES_CONTACT } else { 0 }),
        ..mv(effect, power, ty, accuracy, pp, Selected, 0)
    }
}

pub static MOVES: [MoveInfo; 120] = [
    mv(MoveEffect::Hit, 0, Type::Normal, 0, 0, Selected, 0),
    MoveInfo {
        flags: MoveFlags(MoveFlags::MAKES_CONTACT),
        ..mv(MoveEffect::Hit, 35, Type::Normal, 95, 35, Selected, 0)
    },
    mv(
        MoveEffect::StatDown {
            stat: Stat::Atk,
            stages: 1,
        },
        0,
        Type::Normal,
        100,
        40,
        Selected,
        0,
    ),
    MoveInfo {
        effect_chance: 10,
        ..mv(
            MoveEffect::StatusHit(crate::mon::StatusKind::Burn),
            40,
            Type::Fire,
            100,
            25,
            Selected,
            0,
        )
    },
    mv(MoveEffect::Recoil, 50, Type::Normal, 100, 1, Selected, 0),
    mv(
        MoveEffect::QuickAttack,
        40,
        Type::Normal,
        100,
        30,
        Selected,
        1,
    ),
    mv(
        MoveEffect::StatUp {
            stat: Stat::Def,
            stages: 1,
        },
        0,
        Type::Normal,
        0,
        30,
        User,
        0,
    ),
    mv(MoveEffect::Hit, 40, Type::Water, 100, 25, Selected, 0),
    mv(
        MoveEffect::StatUp {
            stat: Stat::Atk,
            stages: 2,
        },
        0,
        Type::Normal,
        0,
        30,
        User,
        0,
    ),
    mv(MoveEffect::Recoil, 90, Type::Normal, 85, 20, Selected, 0),
    mv(MoveEffect::Unsupported(85), 0, Type::Normal, 0, 40, User, 0),
    mv(MoveEffect::Hit, 250, Type::Normal, 100, 5, Selected, 0),
    mv(
        MoveEffect::StatDown {
            stat: Stat::Def,
            stages: 2,
        },
        0,
        Type::Normal,
        85,
        40,
        Selected,
        0,
    ),
    mv(MoveEffect::Hit, 40, Type::Normal, 100, 10, Selected, -1),
    mv(
        MoveEffect::Unsupported(17),
        60,
        Type::Normal,
        0,
        20,
        Selected,
        0,
    ),
    // ---- Task E2 ----
    e2(MoveEffect::Absorb, 20, Type::Grass, 100, 20, 0, false),
    e2(
        MoveEffect::StatDownHit(Stat::Accuracy),
        20,
        Type::Ground,
        100,
        10,
        100,
        false,
    ),
    e2(
        MoveEffect::StatDown {
            stat: Stat::Accuracy,
            stages: 1,
        },
        0,
        Type::Ground,
        100,
        15,
        0,
        false,
    ),
    e2(
        MoveEffect::StatusHit(StatusKind::Poison),
        15,
        Type::Poison,
        100,
        35,
        30,
        false,
    ),
    e2(
        MoveEffect::Status(StatusKind::Paralysis),
        0,
        Type::Grass,
        75,
        30,
        0,
        false,
    ),
    e2(MoveEffect::Confuse, 0, Type::Normal, 55, 20, 0, false),
    e2(
        MoveEffect::ConfuseHit,
        50,
        Type::Psychic,
        100,
        25,
        10,
        false,
    ),
    e2(MoveEffect::FlinchHit, 60, Type::Dark, 100, 25, 30, true),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::FocusEnergy, 0, Type::Normal, 0, 30, 0, false)
    },
    e2(MoveEffect::LeechSeed, 0, Type::Grass, 90, 10, 0, false),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::Bide, 1, Type::Normal, 100, 10, 0, true)
    },
    e2(MoveEffect::NaturePower, 0, Type::Normal, 95, 20, 0, false),
    e2(
        MoveEffect::Status(StatusKind::Sleep),
        0,
        Type::Grass,
        75,
        15,
        0,
        false,
    ),
    e2(
        MoveEffect::Status(StatusKind::Poison),
        0,
        Type::Poison,
        75,
        35,
        0,
        false,
    ),
    e2(MoveEffect::MultiHit, 15, Type::Normal, 85, 10, 0, true),
    e2(MoveEffect::SonicBoom, 1, Type::Normal, 90, 20, 0, false),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::Charge, 0, Type::Electric, 0, 20, 0, false)
    },
    e2(MoveEffect::Hit, 40, Type::Electric, 100, 30, 0, false),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::DefenseCurl, 0, Type::Normal, 0, 40, 0, false)
    },
    MoveInfo {
        priority: -1,
        ..e2(MoveEffect::VitalThrow, 70, Type::Fighting, 0, 10, 0, true)
    },
    e2(
        MoveEffect::Status(StatusKind::Toxic),
        0,
        Type::Poison,
        85,
        10,
        0,
        false,
    ),
    e2(
        MoveEffect::StatusHit(StatusKind::Freeze),
        40,
        Type::Ice,
        100,
        10,
        100,
        false,
    ),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::Splash, 0, Type::Normal, 0, 40, 0, false)
    },
    e2(MoveEffect::FlinchHit, 60, Type::Dark, 100, 25, 100, true),
    e2(
        MoveEffect::ConfuseHit,
        50,
        Type::Psychic,
        100,
        25,
        100,
        false,
    ),
    e2(
        MoveEffect::StatusHit(StatusKind::Poison),
        15,
        Type::Poison,
        100,
        35,
        100,
        true,
    ),
    // ---- the Pokémon model tests ----
    mv(
        MoveEffect::StatDown {
            stat: Stat::Def,
            stages: 1,
        },
        0,
        Type::Normal,
        100,
        30,
        Selected,
        0,
    ),
    // ---- Task G ----
    e2(MoveEffect::Attract, 0, Type::Normal, 100, 15, 0, false),
    e2(MoveEffect::MeanLook, 0, Type::Normal, 100, 5, 0, false),
    MoveInfo {
        target: User,
        ..e2(MoveEffect::MudSport, 0, Type::Ground, 100, 15, 0, false)
    },
    MoveInfo {
        target: User,
        ..e2(MoveEffect::WaterSport, 0, Type::Water, 100, 15, 0, false)
    },
    MoveInfo {
        target: User,
        ..e2(MoveEffect::Teleport, 0, Type::Psychic, 0, 20, 0, false)
    },
    MoveInfo {
        target: MoveTarget::Random,
        flags: MoveFlags(MoveFlags::SOUND),
        ..e2(MoveEffect::Uproar, 50, Type::Normal, 100, 10, 100, false)
    },
    e2(MoveEffect::Yawn, 0, Type::Normal, 100, 10, 0, false),
    e2(MoveEffect::DoubleHit, 30, Type::Fighting, 100, 30, 0, true),
    e2(MoveEffect::Foresight, 0, Type::Normal, 100, 40, 0, false),
    e2(MoveEffect::FuryCutter, 10, Type::Bug, 95, 20, 0, true),
    e2(MoveEffect::Pursuit, 40, Type::Dark, 100, 20, 0, true),
    MoveInfo {
        flags: MoveFlags(MoveFlags::SOUND),
        ..e2(
            MoveEffect::StatDown {
                stat: Stat::Atk,
                stages: 1,
            },
            0,
            Type::Normal,
            100,
            40,
            0,
            false,
        )
    },
    e2(
        MoveEffect::StatDownHit(Stat::Speed),
        55,
        Type::Ice,
        95,
        15,
        100,
        false,
    ),
    e2(MoveEffect::LowKick, 1, Type::Fighting, 100, 20, 0, true),
    wc(
        MoveEffect::Hit,
        40,
        Type::Normal,
        100,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Rollout,
        30,
        Type::Rock,
        90,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    MoveInfo {
        effect_chance: 100,
        ..wc(
            MoveEffect::Rampage,
            90,
            Type::Normal,
            100,
            MoveTarget::Random,
            MoveFlags::MAKES_CONTACT
                | MoveFlags::PROTECT_AFFECTED
                | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    wc(
        MoveEffect::Recharge,
        150,
        Type::Normal,
        90,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::TwoTurn(TwoTurnKind::SolarBeam),
        120,
        Type::Grass,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::SemiInvulnerable(SemiKind::Fly),
        70,
        Type::Flying,
        95,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::SuperFang,
        1,
        Type::Normal,
        90,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::DragonRage,
        1,
        Type::Dragon,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::LevelDamage,
        1,
        Type::Fighting,
        100,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    MoveInfo {
        priority: -5,
        ..wc(
            MoveEffect::Counter,
            1,
            Type::Fighting,
            100,
            MoveTarget::Depends,
            MoveFlags::MAKES_CONTACT
                | MoveFlags::PROTECT_AFFECTED
                | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    wc(MoveEffect::Curse, 0, Type::Mystery, 0, User, 0),
    wc(
        MoveEffect::Swagger,
        0,
        Type::Normal,
        90,
        Selected,
        MoveFlags::PROTECT_AFFECTED
            | MoveFlags::MIRROR_MOVE_AFFECTED
            | MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    wc(
        MoveEffect::Rest,
        0,
        Type::Psychic,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::RestoreHp,
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(MoveEffect::Haze, 0, Type::Ice, 0, User, 0),
    wc(
        MoveEffect::BellyDrum,
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::PainSplit,
        0,
        Type::Normal,
        0,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Disable,
        0,
        Type::Normal,
        55,
        Selected,
        MoveFlags::PROTECT_AFFECTED
            | MoveFlags::MIRROR_MOVE_AFFECTED
            | MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    wc(
        MoveEffect::Encore,
        0,
        Type::Normal,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED
            | MoveFlags::MIRROR_MOVE_AFFECTED
            | MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    wc(
        MoveEffect::Taunt,
        0,
        Type::Dark,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Torment,
        0,
        Type::Dark,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED
            | MoveFlags::MIRROR_MOVE_AFFECTED
            | MoveFlags::MAGIC_COAT_AFFECTED,
    ),
    MoveInfo {
        priority: 3,
        ..wc(MoveEffect::Protect, 0, Type::Normal, 0, User, 0)
    },
    wc(
        MoveEffect::Reflect,
        0,
        Type::Psychic,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Safeguard,
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Spikes,
        0,
        Type::Ground,
        0,
        MoveTarget::OpponentsField,
        0,
    ),
    wc(
        MoveEffect::SetWeather(WeatherKind::Rain),
        0,
        Type::Water,
        0,
        User,
        0,
    ),
    wc(
        MoveEffect::SetWeather(WeatherKind::Hail),
        0,
        Type::Ice,
        0,
        User,
        0,
    ),
    wc(
        MoveEffect::Mist,
        0,
        Type::Ice,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Metronome,
        0,
        Type::Normal,
        0,
        MoveTarget::Depends,
        0,
    ),
    wc(
        MoveEffect::SleepTalk,
        0,
        Type::Normal,
        0,
        MoveTarget::Depends,
        0,
    ),
    wc(
        MoveEffect::MirrorMove,
        0,
        Type::Flying,
        0,
        MoveTarget::Depends,
        0,
    ),
    MoveInfo {
        priority: 5,
        ..wc(MoveEffect::HelpingHand, 0, Type::Normal, 0, User, 0)
    },
    MoveInfo {
        priority: 3,
        ..wc(MoveEffect::FollowMe, 0, Type::Normal, 0, User, 0)
    },
    wc(
        MoveEffect::PerishSong,
        0,
        Type::Normal,
        0,
        MoveTarget::FoesAndAlly,
        MoveFlags::SOUND,
    ),
    wc(
        MoveEffect::FutureSight(false),
        80,
        Type::Psychic,
        90,
        Selected,
        0,
    ),
    MoveInfo {
        effect_chance: 100,
        ..wc(
            MoveEffect::Trap(TrapKind::Wrap),
            15,
            Type::Normal,
            85,
            Selected,
            MoveFlags::MAKES_CONTACT
                | MoveFlags::PROTECT_AFFECTED
                | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    wc(MoveEffect::DestinyBond, 0, Type::Ghost, 0, User, 0),
    wc(
        MoveEffect::Ingrain,
        0,
        Type::Grass,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Wish,
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Facade,
        70,
        Type::Normal,
        100,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    MoveInfo {
        effect_chance: 100,
        ..wc(
            MoveEffect::KnockOff,
            20,
            Type::Dark,
            100,
            Selected,
            MoveFlags::MAKES_CONTACT
                | MoveFlags::PROTECT_AFFECTED
                | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    MoveInfo {
        priority: -6,
        ..wc(
            MoveEffect::Roar,
            0,
            Type::Normal,
            100,
            Selected,
            MoveFlags::SOUND | MoveFlags::MIRROR_MOVE_AFFECTED | MoveFlags::MAGIC_COAT_AFFECTED,
        )
    },
    wc(
        MoveEffect::Ohko,
        1,
        Type::Normal,
        30,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Rage,
        20,
        Type::Normal,
        100,
        Selected,
        MoveFlags::MAKES_CONTACT | MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(MoveEffect::RolePlay, 0, Type::Psychic, 0, Selected, 0),
    wc(
        MoveEffect::SkillSwap,
        0,
        Type::Psychic,
        0,
        Selected,
        MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(MoveEffect::Grudge, 0, Type::Ghost, 0, User, 0),
    wc(MoveEffect::Imprison, 0, Type::Psychic, 0, User, 0),
    MoveInfo {
        priority: 4,
        ..wc(MoveEffect::MagicCoat, 0, Type::Psychic, 0, User, 0)
    },
    wc(
        MoveEffect::Trick,
        0,
        Type::Psychic,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Substitute,
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(MoveEffect::BatonPass, 0, Type::Normal, 0, User, 0),
    wc(MoveEffect::Transform, 0, Type::Normal, 0, Selected, 0),
    wc(MoveEffect::Mimic, 0, Type::Normal, 100, Selected, 0),
    MoveInfo {
        pp: 1,
        ..wc(MoveEffect::Sketch, 0, Type::Normal, 0, Selected, 0)
    },
    wc(MoveEffect::Recycle, 0, Type::Normal, 100, User, 0),
    wc(MoveEffect::Conversion2, 0, Type::Normal, 100, Selected, 0),
    MoveInfo {
        priority: 4,
        ..wc(MoveEffect::Snatch, 0, Type::Dark, 0, User, 0)
    },
    wc(
        MoveEffect::BeatUp,
        10,
        Type::Dark,
        100,
        Selected,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    MoveInfo {
        effect_chance: 100,
        ..wc(
            MoveEffect::PayDay,
            40,
            Type::Normal,
            100,
            Selected,
            MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    MoveInfo {
        effect_chance: 100,
        ..wc(
            MoveEffect::Thief,
            40,
            Type::Dark,
            100,
            Selected,
            MoveFlags::MAKES_CONTACT
                | MoveFlags::PROTECT_AFFECTED
                | MoveFlags::MIRROR_MOVE_AFFECTED,
        )
    },
    wc(
        MoveEffect::StatUp {
            stat: Stat::Atk,
            stages: 2,
        },
        0,
        Type::Normal,
        0,
        User,
        MoveFlags::SNATCH_AFFECTED,
    ),
    wc(
        MoveEffect::Hit,
        50,
        Type::Normal,
        100,
        MoveTarget::BothFoes,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
    wc(
        MoveEffect::Earthquake,
        100,
        Type::Ground,
        100,
        MoveTarget::FoesAndAlly,
        MoveFlags::PROTECT_AFFECTED | MoveFlags::MIRROR_MOVE_AFFECTED,
    ),
];

const fn lv(level: u8, mov: MoveId) -> LevelUpMove {
    LevelUpMove { level, mov }
}

static WATER_LEARNSET: [LevelUpMove; 4] = [
    lv(1, TACKLE),
    lv(1, GROWL),
    lv(6, WATER_GUN),
    lv(10, HARDEN),
];
static NORMAL_LEARNSET: [LevelUpMove; 2] = [lv(1, TACKLE), lv(3, GROWL)];
/// Learns two moves at level 6 (a learn-move prompt per move when full).
static FIRE_LEARNSET: [LevelUpMove; 4] = [
    lv(1, TACKLE),
    lv(1, GROWL),
    lv(6, EMBER),
    lv(6, QUICK_ATTACK),
];

const fn species(
    base: [u8; 6],
    ty: Type,
    exp_yield: u8,
    ability: Ability,
    learnset: &'static [LevelUpMove],
) -> SpeciesInfo {
    SpeciesInfo {
        name: &[0xFF],
        base,
        types: [ty, ty],
        catch_rate: 255,
        exp_yield,
        ev_yield: [0, 0, 0, 1, 0, 0],
        gender_ratio: 127,
        friendship: 70,
        growth: GrowthRate::MediumFast,
        abilities: [ability, Ability::None],
        learnset,
        weight: 0,
        gfx: None,
    }
}

pub static SPECIES: [SpeciesInfo; 38] = [
    species([0; 6], Type::Normal, 0, Ability::None, &[]),
    species(
        [50, 70, 50, 40, 50, 50],
        Type::Water,
        65,
        Ability::Torrent,
        &WATER_LEARNSET,
    ),
    species(
        [38, 30, 41, 60, 30, 41],
        Type::Normal,
        60,
        Ability::Other,
        &NORMAL_LEARNSET,
    ),
    species(
        [45, 60, 40, 45, 70, 50],
        Type::Fire,
        65,
        Ability::Blaze,
        &FIRE_LEARNSET,
    ),
    species(
        [45, 45, 35, 20, 20, 30],
        Type::Bug,
        54,
        Ability::ShieldDust,
        &[],
    ),
    species(
        [35, 55, 35, 35, 30, 30],
        Type::Dark,
        55,
        Ability::RunAway,
        &[],
    ),
    species(
        [30, 35, 30, 80, 100, 35],
        Type::Ghost,
        95,
        Ability::None,
        &[],
    ),
    // ---- Task E2 ----
    species(STATS, Type::Grass, 50, Ability::EffectSpore, &[]),
    species(STATS, Type::Psychic, 50, Ability::Synchronize, &[]),
    species(STATS, Type::Dark, 50, Ability::Intimidate, &[]),
    species(STATS, Type::Psychic, 50, Ability::Trace, &[]),
    species(STATS, Type::Poison, 50, Ability::PoisonPoint, &[]),
    species(STATS, Type::Flying, 50, Ability::KeenEye, &[]),
    species(STATS, Type::Electric, 50, Ability::Static, &[]),
    species(STATS, Type::Steel, 50, Ability::None, &[]),
    species(STATS, Type::Normal, 50, Ability::Guts, &[]),
    // ---- the Pokémon model tests ----
    named(
        MUDKIP_NAME,
        [50, 70, 50, 40, 50, 50],
        Type::Water,
        [0, 1, 0, 0, 0, 0],
        31,
        GrowthRate::MediumSlow,
        [Ability::Torrent, Ability::None],
        &[
            lv(1, TACKLE),
            lv(1, GROWL),
            lv(6, MUD_SLAP),
            lv(10, WATER_GUN),
            lv(15, BIDE),
        ],
    ),
    named(
        ZIGZAGOON_NAME,
        [38, 30, 41, 60, 30, 41],
        Type::Normal,
        [0, 0, 0, 1, 0, 0],
        127,
        GrowthRate::MediumFast,
        [Ability::Other, Ability::Other],
        &[lv(1, TACKLE), lv(3, GROWL), lv(5, TAIL_WHIP)],
    ),
    named(
        &[0xCD, 0xFF],
        [40; 6],
        Type::Electric,
        [0; 6],
        0,
        GrowthRate::Fast,
        [Ability::Static, Ability::Synchronize],
        &[
            lv(1, TACKLE),
            lv(1, GROWL),
            lv(2, MUD_SLAP),
            lv(3, WATER_GUN),
            lv(4, BIDE),
            lv(5, TAIL_WHIP),
            lv(5, WATER_GUN),
            lv(9, GROWL),
        ],
    ),
    named(
        &[0xBC, 0xFF],
        [40; 6],
        Type::Normal,
        [3, 2, 0, 0, 0, 0],
        255,
        GrowthRate::Erratic,
        [Ability::Synchronize, Ability::None],
        &[lv(1, TACKLE)],
    ),
    named(
        &[0xC2, 0xFF],
        [40; 6],
        Type::Flying,
        [0; 6],
        254,
        GrowthRate::Fluctuating,
        [Ability::KeenEye, Ability::None],
        &[lv(1, TACKLE)],
    ),
    named(
        LONG_NAME,
        [40; 6],
        Type::Normal,
        [0; 6],
        127,
        GrowthRate::MediumFast,
        [Ability::Intimidate, Ability::None],
        &[lv(1, TACKLE)],
    ),
    // ---- Task G ----
    species(STATS, Type::Normal, 50, Ability::Oblivious, &[]),
    species(STATS, Type::Normal, 50, Ability::CuteCharm, &[]),
    species(STATS, Type::Rock, 50, Ability::MagnetPull, &[]),
    species(STATS, Type::Normal, 50, Ability::Soundproof, &[]),
    species(STATS, Type::Normal, 50, Ability::Insomnia, &[]),
    species(STATS, Type::Normal, 50, Ability::Truant, &[]),
    species(STATS, Type::Bug, 50, Ability::ShedSkin, &[]),
    species(STATS, Type::Psychic, 50, Ability::InnerFocus, &[]),
    species(STATS, Type::Grass, 50, Ability::EarlyBird, &[]),
    species(STATS, Type::Rock, 50, Ability::RockHead, &[]),
    species(STATS, Type::Bug, 50, Ability::CompoundEyes, &[]),
    species(STATS, Type::Water, 50, Ability::ClearBody, &[]),
    species(STATS, Type::Water, 50, Ability::LiquidOoze, &[]),
    species(STATS, Type::Normal, 50, Ability::NaturalCure, &[]),
    species(STATS, Type::Normal, 50, Ability::Pickup, &[]),
    SpeciesInfo {
        weight: 200,
        ..species(STATS, Type::Rock, 50, Ability::None, &[])
    },
];

/// A Pokémon-model species: named, exp yield 50, its own EV yield, gender ratio, growth rate
/// and abilities.
#[allow(clippy::too_many_arguments)]
const fn named(
    name: &'static [u8],
    base: [u8; 6],
    ty: Type,
    ev_yield: [u8; 6],
    gender_ratio: u8,
    growth: GrowthRate,
    abilities: [Ability; 2],
    learnset: &'static [LevelUpMove],
) -> SpeciesInfo {
    SpeciesInfo {
        name,
        ev_yield,
        gender_ratio,
        growth,
        abilities,
        ..species(base, ty, 50, Ability::None, learnset)
    }
}

/// Base stats of the E2 species (tests set explicit stats anyway).
const STATS: [u8; 6] = [50; 6];

const fn tm(attack: Type, defend: Type, multiplier: u8) -> TypeMatchup {
    TypeMatchup {
        attack,
        defend,
        multiplier,
    }
}

/// A `gTypeEffectiveness` subset, in Gen3 order; Foresight-ignored entries last.
pub static TYPE_CHART: [TypeMatchup; 14] = [
    tm(Type::Normal, Type::Rock, 5),
    tm(Type::Normal, Type::Steel, 5),
    tm(Type::Fire, Type::Fire, 5),
    tm(Type::Fire, Type::Water, 5),
    tm(Type::Fire, Type::Grass, 20),
    tm(Type::Fire, Type::Bug, 20),
    tm(Type::Water, Type::Fire, 20),
    tm(Type::Water, Type::Water, 5),
    tm(Type::Water, Type::Grass, 5),
    tm(Type::Grass, Type::Water, 20),
    tm(Type::Bug, Type::Fire, 5),
    tm(Type::Dark, Type::Ghost, 20),
    // TYPE_FORESIGHT
    tm(Type::Normal, Type::Ghost, 0),
    tm(Type::Fighting, Type::Ghost, 0),
];

pub static TABLES: BattleTables = BattleTables {
    species: &SPECIES,
    moves: &MOVES,
    type_chart: &TYPE_CHART,
    foresight_split: 12,
    struggle: STRUGGLE,
    nature_power: [SWIFT; 10],
};

/// A test mon with explicit stats (HP, Atk, Def, Speed, SpAtk, SpDef), full HP and PP, exp at
/// its level's minimum (MediumFast: `level³`), personality 0 (Hardy, female for ratio 127).
pub fn mon(species: SpeciesId, level: u8, stats: [u16; 6], moves: &[MoveId]) -> Pokemon {
    let mut m = [None; 4];
    let mut pp = [0; 4];
    for (i, &mv) in moves.iter().enumerate().take(4) {
        m[i] = Some(mv);
        pp[i] = TABLES.move_info(mv).pp;
    }
    let mut nickname = [0xFF; NAME_LEN];
    nickname[0] = 0xBB + species.0 as u8; // "A", "B", ...
    Pokemon {
        species,
        personality: 0,
        ot_id: 0,
        nickname,
        level,
        exp: level as u32 * level as u32 * level as u32,
        ivs: [0; 6],
        evs: [0; 6],
        moves: m,
        pp,
        pp_bonuses: 0,
        friendship: 70,
        ability_slot: 0,
        status: Status::None,
        hp: stats[0],
        stats,
        held: None,
    }
}

/// A party of the given mons.
pub fn party(mons: &[Pokemon]) -> Party {
    let mut p = Party::default();
    for &m in mons {
        p.push(m).unwrap();
    }
    p
}

/// Serves [`TABLES`]; everything else is unused by battle tests.
pub struct Game;

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        unimplemented!()
    }
    fn window(&self) -> &'static WindowGfx {
        unimplemented!()
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, _: UiText) -> &'static [u8] {
        &[0xFF]
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        alloc::boxed::Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
    /// `STRINGID_PKMNFAINTED` reduced to its name code (`{B_DEF_NAME_WITH_PREFIX}`); every
    /// other string (the wild/foe tags too) is empty.
    fn battle_text(&self, s: Str) -> &'static [u8] {
        match s {
            Str::PkmnFainted => &[0xFD, 0x10, 0xFF],
            _ => &[0xFF],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_indexed_by_id() {
        assert_eq!(TABLES.move_info(STRUGGLE).effect, MoveEffect::Recoil);
        assert_eq!(TABLES.move_info(QUICK_ATTACK).priority, 1);
        assert_eq!(TABLES.species_info(GHOST).types, [Type::Ghost; 2]);
        assert_eq!(
            TABLES.type_chart[TABLES.foresight_split],
            tm(Type::Normal, Type::Ghost, 0)
        );
        let m = mon(WATER, 5, [20, 13, 10, 9, 9, 10], &[TACKLE, GROWL]);
        assert_eq!(m.pp, [35, 40, 0, 0]);
        assert_eq!(m.exp, 125);
    }

    /// The entries no test uses yet sit at their ids (the tables are indexed by id).
    #[test]
    fn spare_entries_are_at_their_ids() {
        let effect = |m| TABLES.move_info(m).effect;
        assert_eq!(
            effect(POISON_STING),
            MoveEffect::StatusHit(StatusKind::Poison)
        );
        assert_eq!(effect(CONFUSION), MoveEffect::ConfuseHit);
        assert_eq!(effect(BITE), MoveEffect::FlinchHit);
        assert_eq!(effect(CHARGE), MoveEffect::Charge);
        assert_eq!(
            effect(FREEZE_HIT),
            MoveEffect::StatusHit(StatusKind::Freeze)
        );
        assert_eq!(effect(CONFUSE_ALL), MoveEffect::ConfuseHit);
        assert_eq!(TABLES.move_info(CONFUSE_ALL).effect_chance, 100);
        assert_eq!(TABLES.species_info(GUTS).abilities[0], Ability::Guts);
        assert_eq!(TABLES.species_info(GROWLER).name, LONG_NAME);
    }
}
