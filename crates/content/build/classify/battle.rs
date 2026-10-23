//! Battle classification (owned by Milestone 8 Task G): growth rates, move targets, flags and
//! effects, abilities, and the battle strings (`STRS`).

/// `GROWTH_*` name -> `GrowthRate` expression.
pub fn growth(name: &str) -> Option<&'static str> {
    Some(match name {
        "GROWTH_MEDIUM_FAST" => "GrowthRate::MediumFast",
        "GROWTH_ERRATIC" => "GrowthRate::Erratic",
        "GROWTH_FLUCTUATING" => "GrowthRate::Fluctuating",
        "GROWTH_MEDIUM_SLOW" => "GrowthRate::MediumSlow",
        "GROWTH_FAST" => "GrowthRate::Fast",
        "GROWTH_SLOW" => "GrowthRate::Slow",
        _ => return None,
    })
}

/// `MOVE_TARGET_*` name -> `MoveTarget` expression.
pub fn move_target(name: &str) -> Option<&'static str> {
    Some(match name {
        "MOVE_TARGET_SELECTED" => "MoveTarget::Selected",
        "MOVE_TARGET_DEPENDS" => "MoveTarget::Depends",
        "MOVE_TARGET_USER_OR_SELECTED" => "MoveTarget::UserOrSelected",
        "MOVE_TARGET_RANDOM" => "MoveTarget::Random",
        "MOVE_TARGET_BOTH" => "MoveTarget::BothFoes",
        "MOVE_TARGET_USER" => "MoveTarget::User",
        "MOVE_TARGET_FOES_AND_ALLY" => "MoveTarget::FoesAndAlly",
        "MOVE_TARGET_OPPONENTS_FIELD" => "MoveTarget::OpponentsField",
        _ => return None,
    })
}

/// Gen3 move flag (`FLAG_*`, `include/pokemon.h`) -> its bit.
pub fn move_flag(name: &str) -> Option<u8> {
    Some(match name {
        "FLAG_MAKES_CONTACT" => 1 << 0,
        "FLAG_PROTECT_AFFECTED" => 1 << 1,
        "FLAG_MAGIC_COAT_AFFECTED" => 1 << 2,
        "FLAG_SNATCH_AFFECTED" => 1 << 3,
        "FLAG_MIRROR_MOVE_AFFECTED" => 1 << 4,
        "FLAG_KINGS_ROCK_AFFECTED" => 1 << 5,
        _ => return None,
    })
}

/// `MoveFlags::SOUND` (1 << 6, not a Gen3 move flag): the moves of `sSoundMovesTable`
/// (battle_util.c), which Soundproof blocks.
pub const SOUND_MOVES: [&str; 10] = [
    "MOVE_GROWL",
    "MOVE_ROAR",
    "MOVE_SING",
    "MOVE_SUPERSONIC",
    "MOVE_SCREECH",
    "MOVE_SNORE",
    "MOVE_UPROAR",
    "MOVE_METAL_SOUND",
    "MOVE_GRASS_WHISTLE",
    "MOVE_HYPER_VOICE",
];
pub const SOUND_FLAG: u8 = 1 << 6;

/// The `MoveFlags` bits of move `id` (`MOVE_*`) that are not Gen3 move flags (OR them into the
/// `move_flag` bits): [`SOUND_FLAG`] for the moves of [`SOUND_MOVES`].
pub fn extra_move_flags(id: &str) -> u8 {
    if SOUND_MOVES.contains(&id) {
        SOUND_FLAG
    } else {
        0
    }
}

/// Whether trainer mons get their `heldItem` (`TrainerMon::item`) in Emerald (Silice turns it on:
/// [`Rules`]). **Off for Emerald**: the
/// recorded route (parity, checkpoints) was played without trainer items, and a trainer that
/// holds a berry or a Quick Claw changes its battle. Turning it on is a later, deliberate
/// Emerald route change (re-baseline the parity route and the checkpoint tree).
pub const TRAINER_HELD_ITEMS: bool = false;

/// `ITEM_*` name -> `HoldEffect` expression (`src/data/items.h` `holdEffect` /
/// `holdEffectParam`, `sHoldEffectToType`), for the hold effects the engine models; `None`
/// for any other item.
pub fn hold_effect(name: &str) -> Option<&'static str> {
    Some(match name {
        "ITEM_ORAN_BERRY" => "HoldEffect::RestoreHp(10)",
        "ITEM_SITRUS_BERRY" => "HoldEffect::RestoreHp(30)",
        "ITEM_BERRY_JUICE" => "HoldEffect::RestoreHp(20)",
        "ITEM_CHERI_BERRY" => "HoldEffect::CurePar",
        "ITEM_CHESTO_BERRY" => "HoldEffect::CureSlp",
        "ITEM_PECHA_BERRY" => "HoldEffect::CurePsn",
        "ITEM_RAWST_BERRY" => "HoldEffect::CureBrn",
        "ITEM_ASPEAR_BERRY" => "HoldEffect::CureFrz",
        "ITEM_PERSIM_BERRY" => "HoldEffect::CureConfusion",
        "ITEM_LUM_BERRY" => "HoldEffect::CureStatus",
        "ITEM_LEFTOVERS" => "HoldEffect::Leftovers",
        "ITEM_CHOICE_BAND" => "HoldEffect::ChoiceBand",
        "ITEM_QUICK_CLAW" => "HoldEffect::QuickClaw(20)",
        "ITEM_KINGS_ROCK" => "HoldEffect::KingsRock(10)",
        "ITEM_BRIGHT_POWDER" => "HoldEffect::EvasionUp(10)",
        "ITEM_LAX_INCENSE" => "HoldEffect::EvasionUp(5)",
        "ITEM_FOCUS_BAND" => "HoldEffect::FocusBand(10)",
        "ITEM_SILVER_POWDER" => "HoldEffect::TypePower(Type::Bug, 10)",
        "ITEM_METAL_COAT" => "HoldEffect::TypePower(Type::Steel, 10)",
        "ITEM_SOFT_SAND" => "HoldEffect::TypePower(Type::Ground, 10)",
        "ITEM_HARD_STONE" => "HoldEffect::TypePower(Type::Rock, 10)",
        "ITEM_MIRACLE_SEED" => "HoldEffect::TypePower(Type::Grass, 10)",
        "ITEM_BLACK_GLASSES" => "HoldEffect::TypePower(Type::Dark, 10)",
        "ITEM_BLACK_BELT" => "HoldEffect::TypePower(Type::Fighting, 10)",
        "ITEM_MAGNET" => "HoldEffect::TypePower(Type::Electric, 10)",
        "ITEM_MYSTIC_WATER" => "HoldEffect::TypePower(Type::Water, 10)",
        "ITEM_SEA_INCENSE" => "HoldEffect::TypePower(Type::Water, 5)",
        "ITEM_SHARP_BEAK" => "HoldEffect::TypePower(Type::Flying, 10)",
        "ITEM_POISON_BARB" => "HoldEffect::TypePower(Type::Poison, 10)",
        "ITEM_NEVER_MELT_ICE" => "HoldEffect::TypePower(Type::Ice, 10)",
        "ITEM_SPELL_TAG" => "HoldEffect::TypePower(Type::Ghost, 10)",
        "ITEM_TWISTED_SPOON" => "HoldEffect::TypePower(Type::Psychic, 10)",
        "ITEM_CHARCOAL" => "HoldEffect::TypePower(Type::Fire, 10)",
        "ITEM_DRAGON_FANG" => "HoldEffect::TypePower(Type::Dragon, 10)",
        "ITEM_SILK_SCARF" => "HoldEffect::TypePower(Type::Normal, 10)",
        _ => return None,
    })
}

/// `ABILITY_*` name -> `Ability` expression under Emerald's [`Rules`]; the abilities the
/// engine does not implement are `Ability::Other`.
pub fn ability(name: &str) -> &'static str {
    ability_with(name, Rules::EMERALD)
}

/// A flavour's battle switches. The Gen3 behaviour the recorded Emerald route was played
/// without stays off for Emerald ([`Rules::EMERALD`]); a game with no recorded route turns it
/// on ([`Rules::SILICE`], SIL1 Task A). The generator reads them from `input::AuthoredIn`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// Trainer mons get their held item ([`TRAINER_HELD_ITEMS`]).
    pub trainer_held_items: bool,
    /// The Gen3 abilities the Silice engine added act ([`GEN3_EXTRA_ABILITIES`]).
    pub gen3_extra_abilities: bool,
    /// Lightning Rod's Gen5 absorb, Shadow Tag's Gen4 exemption ([`SILICE_ABILITY_RULES`]).
    pub silice_abilities: bool,
}

impl Rules {
    /// Emerald and the stub: the constants.
    pub const EMERALD: Rules = Rules {
        trainer_held_items: TRAINER_HELD_ITEMS,
        gen3_extra_abilities: GEN3_EXTRA_ABILITIES,
        silice_abilities: SILICE_ABILITY_RULES,
    };
    /// Silice: trainer held items and every ability the engine models (its Mega forms need
    /// Drought, Sand Stream, Levitate…).
    pub const SILICE: Rules = Rules {
        trainer_held_items: true,
        gen3_extra_abilities: true,
        silice_abilities: true,
    };
}

impl Default for Rules {
    fn default() -> Rules {
        Rules::EMERALD
    }
}

/// [`ability`] under `r`.
pub fn ability_with(name: &str, r: Rules) -> &'static str {
    let (gen3_extra, silice) = (r.gen3_extra_abilities, r.silice_abilities);
    match name {
        "ABILITY_NONE" => "Ability::None",
        "ABILITY_OVERGROW" => "Ability::Overgrow",
        "ABILITY_BLAZE" => "Ability::Blaze",
        "ABILITY_TORRENT" => "Ability::Torrent",
        "ABILITY_SWARM" => "Ability::Swarm",
        "ABILITY_RUN_AWAY" => "Ability::RunAway",
        "ABILITY_SHIELD_DUST" => "Ability::ShieldDust",
        "ABILITY_KEEN_EYE" => "Ability::KeenEye",
        "ABILITY_GUTS" => "Ability::Guts",
        "ABILITY_EFFECT_SPORE" => "Ability::EffectSpore",
        "ABILITY_SYNCHRONIZE" => "Ability::Synchronize",
        "ABILITY_TRACE" => "Ability::Trace",
        "ABILITY_INTIMIDATE" => "Ability::Intimidate",
        "ABILITY_STATIC" => "Ability::Static",
        "ABILITY_POISON_POINT" => "Ability::PoisonPoint",
        // Task G.
        "ABILITY_CHLOROPHYLL" => "Ability::Chlorophyll",
        "ABILITY_COMPOUND_EYES" => "Ability::CompoundEyes",
        "ABILITY_CUTE_CHARM" => "Ability::CuteCharm",
        "ABILITY_EARLY_BIRD" => "Ability::EarlyBird",
        "ABILITY_HUGE_POWER" => "Ability::HugePower",
        "ABILITY_INNER_FOCUS" => "Ability::InnerFocus",
        "ABILITY_MAGNET_PULL" => "Ability::MagnetPull",
        "ABILITY_PICKUP" => "Ability::Pickup",
        "ABILITY_RAIN_DISH" => "Ability::RainDish",
        "ABILITY_ROCK_HEAD" => "Ability::RockHead",
        "ABILITY_SHED_SKIN" => "Ability::ShedSkin",
        "ABILITY_SOUNDPROOF" => "Ability::Soundproof",
        "ABILITY_STURDY" => "Ability::Sturdy",
        "ABILITY_SWIFT_SWIM" => "Ability::SwiftSwim",
        "ABILITY_THICK_FAT" => "Ability::ThickFat",
        "ABILITY_TRUANT" => "Ability::Truant",
        "ABILITY_OBLIVIOUS" => "Ability::Oblivious",
        "ABILITY_INSOMNIA" => "Ability::Insomnia",
        "ABILITY_VITAL_SPIRIT" => "Ability::VitalSpirit",
        "ABILITY_HUSTLE" => "Ability::Hustle",
        "ABILITY_MARVEL_SCALE" => "Ability::MarvelScale",
        "ABILITY_PURE_POWER" => "Ability::PurePower",
        "ABILITY_CLEAR_BODY" => "Ability::ClearBody",
        "ABILITY_LIQUID_OOZE" => "Ability::LiquidOoze",
        "ABILITY_NATURAL_CURE" => "Ability::NaturalCure",
        // Silice engine: Gen3 abilities that Emerald species have, gated.
        "ABILITY_DROUGHT" if gen3_extra => "Ability::Drought",
        "ABILITY_DRIZZLE" if gen3_extra => "Ability::Drizzle",
        "ABILITY_SAND_STREAM" if gen3_extra => "Ability::SandStream",
        "ABILITY_SPEED_BOOST" if gen3_extra => "Ability::SpeedBoost",
        "ABILITY_SHADOW_TAG" if silice => "Ability::ShadowTagGen4",
        "ABILITY_SHADOW_TAG" if gen3_extra => "Ability::ShadowTag",
        "ABILITY_LEVITATE" if gen3_extra => "Ability::Levitate",
        "ABILITY_SHELL_ARMOR" if gen3_extra => "Ability::ShellArmor",
        "ABILITY_BATTLE_ARMOR" if gen3_extra => "Ability::BattleArmor",
        "ABILITY_SAND_VEIL" if gen3_extra => "Ability::SandVeil",
        // Gen3's Lightning Rod only redirects in doubles: nothing in singles (`Other`).
        "ABILITY_LIGHTNING_ROD" if silice => "Ability::LightningRod",
        // Post-Gen3 (no Emerald species has them).
        "ABILITY_PRIMORDIAL_SEA" => "Ability::PrimordialSea",
        "ABILITY_DESOLATE_LAND" => "Ability::DesolateLand",
        "ABILITY_AERILATE" => "Ability::Aerilate",
        "ABILITY_PIXILATE" => "Ability::Pixilate",
        "ABILITY_REFRIGERATE" => "Ability::Refrigerate",
        "ABILITY_TOUGH_CLAWS" => "Ability::ToughClaws",
        "ABILITY_MAGIC_BOUNCE" => "Ability::MagicBounce",
        "ABILITY_PARENTAL_BOND" => "Ability::ParentalBond",
        "ABILITY_MOLD_BREAKER" => "Ability::MoldBreaker",
        "ABILITY_TECHNICIAN" => "Ability::Technician",
        "ABILITY_SKILL_LINK" => "Ability::SkillLink",
        "ABILITY_SAND_FORCE" => "Ability::SandForce",
        "ABILITY_SOLAR_POWER" => "Ability::SolarPower",
        "ABILITY_FILTER" => "Ability::Filter",
        "ABILITY_PRANKSTER" => "Ability::Prankster",
        "ABILITY_STEADFAST" => "Ability::Steadfast",
        _ => "Ability::Other",
    }
}

/// Whether the Gen3 abilities the Silice engine added (Drought, Drizzle, Sand Stream, Speed
/// Boost, Shadow Tag, Levitate, Shell / Battle Armor, Sand Veil) act for Emerald's
/// species, with their Gen3 behaviour. **Off**: Torkoal, Tyranitar, Ninjask, Wobbuffet, the
/// Levitate ghosts … would change the recorded route; turning it on is a later Emerald route
/// change (re-baseline parity and the checkpoint tree). Gen3's Lightning Rod stays `Other`
/// (a doubles redirect). Rain Dish is mapped as before: only these bring rain. Not modelled: Cloud Nine, Air Lock (weather suppression), Sand
/// Veil's overworld encounter halving.
pub const GEN3_EXTRA_ABILITIES: bool = false;

/// The Silice game's later-generation rules for Gen3 abilities: Lightning Rod's Gen5 absorb,
/// Shadow Tag's Gen4 exemption. **Off** for Emerald (not Gen3 behaviour); the Silice content
/// turns it on ([`Rules::SILICE`]).
pub const SILICE_ABILITY_RULES: bool = false;

/// The `Stat` of a `battle_move_effects.h` stat word.
fn effect_stat(s: &str) -> Option<&'static str> {
    Some(match s {
        "ATTACK" => "Stat::Atk",
        "DEFENSE" => "Stat::Def",
        "SPEED" => "Stat::Speed",
        "SPECIAL_ATTACK" => "Stat::SpAtk",
        "SPECIAL_DEFENSE" => "Stat::SpDef",
        "ACCURACY" => "Stat::Accuracy",
        "EVASION" => "Stat::Evasion",
        _ => return None,
    })
}

/// `EFFECT_*` name -> `MoveEffect` expression; `None` for an effect the engine does not
/// implement (the build writes `MoveEffect::Unsupported(<EFFECT_* number>)`).
pub fn move_effect(name: &str) -> Option<String> {
    let e = name.strip_prefix("EFFECT_")?;
    for (suffix, stages, v) in [
        ("_UP_2", 2, "StatUp"),
        ("_UP", 1, "StatUp"),
        ("_DOWN_2", 2, "StatDown"),
        ("_DOWN", 1, "StatDown"),
    ] {
        if let Some(stat) = e.strip_suffix(suffix).and_then(effect_stat) {
            return Some(format!(
                "MoveEffect::{v} {{ stat: {stat}, stages: {stages} }}"
            ));
        }
    }
    if let Some(stat) = e.strip_suffix("_DOWN_HIT").and_then(effect_stat) {
        return Some(format!("MoveEffect::StatDownHit({stat})"));
    }
    Some(
        match e {
            "HIT" => "MoveEffect::Hit",
            "POISON_HIT" => "MoveEffect::StatusHit(StatusKind::Poison)",
            "BURN_HIT" => "MoveEffect::StatusHit(StatusKind::Burn)",
            "PARALYZE_HIT" => "MoveEffect::StatusHit(StatusKind::Paralysis)",
            "FREEZE_HIT" => "MoveEffect::StatusHit(StatusKind::Freeze)",
            "POISON" => "MoveEffect::Status(StatusKind::Poison)",
            "PARALYZE" => "MoveEffect::Status(StatusKind::Paralysis)",
            "SLEEP" => "MoveEffect::Status(StatusKind::Sleep)",
            "TOXIC" => "MoveEffect::Status(StatusKind::Toxic)",
            "CONFUSE" => "MoveEffect::Confuse",
            "CONFUSE_HIT" => "MoveEffect::ConfuseHit",
            "FLINCH_HIT" => "MoveEffect::FlinchHit",
            "FLINCH_MINIMIZE_HIT" => "MoveEffect::FlinchMinimizeHit",
            "ABSORB" => "MoveEffect::Absorb",
            "LEECH_SEED" => "MoveEffect::LeechSeed",
            "BIDE" => "MoveEffect::Bide",
            "FOCUS_ENERGY" => "MoveEffect::FocusEnergy",
            "NATURE_POWER" => "MoveEffect::NaturePower",
            "QUICK_ATTACK" => "MoveEffect::QuickAttack",
            "RECOIL" => "MoveEffect::Recoil",
            "MULTI_HIT" => "MoveEffect::MultiHit",
            "SPLASH" => "MoveEffect::Splash",
            "DEFENSE_CURL" => "MoveEffect::DefenseCurl",
            "SONICBOOM" => "MoveEffect::SonicBoom",
            "VITAL_THROW" => "MoveEffect::VitalThrow",
            "CHARGE" => "MoveEffect::Charge",
            "ATTRACT" => "MoveEffect::Attract",
            "MEAN_LOOK" => "MoveEffect::MeanLook",
            "MUD_SPORT" => "MoveEffect::MudSport",
            "WATER_SPORT" => "MoveEffect::WaterSport",
            "TELEPORT" => "MoveEffect::Teleport",
            "UPROAR" => "MoveEffect::Uproar",
            "YAWN" => "MoveEffect::Yawn",
            "DOUBLE_HIT" => "MoveEffect::DoubleHit",
            "FORESIGHT" => "MoveEffect::Foresight",
            "FURY_CUTTER" => "MoveEffect::FuryCutter",
            "PURSUIT" => "MoveEffect::Pursuit",
            "LOW_KICK" => "MoveEffect::LowKick",
            _ => return None,
        }
        .to_string(),
    )
}

/// Where the text of a `battle::Str` comes from (`battle/strings.ron` keys).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StrSrc {
    /// The string as exported.
    Key(&'static str),
    /// `STRINGID_USEDMOVE` (`BufferStringBattle`): `sText_AttackerUsedX` with the move name
    /// (its `{B_BUFF2}`) read from `Msg::mov` (`{B_CURRENT_MOVE}`), then the ending
    /// `ChooseTypeOfMoveUsedString` appends: `sText_ExclamationMark5` for every move outside
    /// `sGrammarMoveUsedTable` (the build checks that the five endings are equal).
    UsedMove,
    /// "sharply rose!" / "harshly fell!": the adverb string alone when it is a complete phrase
    /// (BPEF, `extract::battle_data::stat_sharply_is_complete`), else adverb then verb (BPEE).
    Sharply {
        adverb: &'static str,
        verb: &'static str,
    },
    /// The string with `code` replaced by `{B_BUFF1}`: the core puts the battler's party slot
    /// in buffer 1 (`Buff::Mon`), so a replacement sent out before the message plays is not
    /// named instead.
    Buff1 {
        key: &'static str,
        code: &'static str,
    },
    /// `gTypeNames` (`battle/types.ron`), by `Type`.
    TypeNames,
    /// `gStatNamesTable` (`extract::battle_data::STAT_NAME_KEYS`), by `Stat`.
    StatNames,
    /// `gAbilityNames` (`battle/types.ron` `ability_names`), by `Ability` (through
    /// [`ability`]); `Ability::Other` stands for many abilities and has no name (empty).
    AbilityNames,
    /// No Gen3 string (the post-Gen3 messages of the Silice engine): empty for Emerald, left
    /// out of `ALL_STRS`.
    Missing,
}

/// Every `battle::Str` variant (as a `match` pattern) and its source. The build writes one
/// exhaustive `match` from this list, so a new engine variant fails the build until it is
/// added here.
pub const STRS: &[(&str, StrSrc)] = {
    use StrSrc::*;
    &[
        ("Intro1", Key("sText_WildPkmnAppeared")),
        ("IntroTrainer", Key("sText_Trainer1WantsToBattle")),
        ("GoPkmn", Key("sText_GoPkmn")),
        ("PkmnSentOut", Key("sText_Trainer1SentOutPkmn")),
        ("WhatWillPkmnDo", Key("gText_WhatWillPkmnDo")),
        ("BattleMenu", Key("gText_BattleMenu")),
        ("MoveInterfacePP", Key("gText_MoveInterfacePP")),
        ("MoveInterfaceType", Key("gText_MoveInterfaceType")),
        ("UsedMove", UsedMove),
        ("AttackMissed", Key("STRINGID_ATTACKMISSED")),
        ("CriticalHit", Key("STRINGID_CRITICALHIT")),
        ("SuperEffective", Key("STRINGID_SUPEREFFECTIVE")),
        ("NotVeryEffective", Key("STRINGID_NOTVERYEFFECTIVE")),
        ("ItDoesntAffect", Key("STRINGID_ITDOESNTAFFECT")),
        (
            "PkmnFainted",
            Buff1 {
                key: "STRINGID_TARGETFAINTED",
                code: "{B_DEF_NAME_WITH_PREFIX}",
            },
        ),
        ("GotAwaySafely", Key("STRINGID_GOTAWAYSAFELY")),
        ("CantEscape", Key("STRINGID_CANTESCAPE")),
        (
            "NoRunningFromTrainers",
            Key("STRINGID_NORUNNINGFROMTRAINERS"),
        ),
        ("DontLeaveBirch", Key("STRINGID_DONTLEAVEBIRCH")),
        // `sText_WildPkmnFled` carries the wild tag after `{B_BUFF1}` itself: Gen3
        // `HandleEndTurn_MonFled` fills buffer 1 with `PREPARE_MON_NICK_BUFFER` (the nickname
        // alone, `B_BUFF_MON_NICK`), not the `_WITH_PREFIX` form. The engine must likewise send
        // `Buff::MonNick`; `Buff::Mon` would print the tag twice.
        ("WildPkmnFled", Key("STRINGID_WILDPKMNFLED")),
        (
            "PlayerDefeatedTrainer",
            Key("STRINGID_PLAYERDEFEATEDTRAINER1"),
        ),
        ("PlayerWhiteout", Key("STRINGID_PLAYERWHITEOUT")),
        ("PlayerWhiteout2", Key("STRINGID_PLAYERWHITEOUT2")),
        ("PkmnGainedExp", Key("STRINGID_PKMNGAINEDEXP")),
        ("PkmnGrewToLv", Key("STRINGID_PKMNGREWTOLV")),
        ("PkmnLearnedMove", Key("STRINGID_PKMNLEARNEDMOVE")),
        ("TryToLearnMove1", Key("STRINGID_TRYTOLEARNMOVE1")),
        ("TryToLearnMove2", Key("STRINGID_TRYTOLEARNMOVE2")),
        ("TryToLearnMove3", Key("STRINGID_TRYTOLEARNMOVE3")),
        ("PkmnForgotMove", Key("STRINGID_PKMNFORGOTMOVE")),
        ("StopLearningMove", Key("STRINGID_STOPLEARNINGMOVE")),
        ("DidNotLearnMove", Key("STRINGID_DIDNOTLEARNMOVE")),
        // `gStatUpStringIds` / `gStatDownStringIds` pick ...ROSE or ...FELL, which BPEF words
        // differently: one `Str` per battler and direction.
        ("AttackerStatRose", Key("STRINGID_ATTACKERSSTATROSE")),
        ("AttackerStatFell", Key("STRINGID_ATTACKERSSTATFELL")),
        ("DefenderStatRose", Key("STRINGID_DEFENDERSSTATROSE")),
        ("DefenderStatFell", Key("STRINGID_DEFENDERSSTATFELL")),
        ("StatRose", Key("STRINGID_STATROSE")),
        (
            "StatSharplyRose",
            Sharply {
                adverb: "STRINGID_STATSHARPLY",
                verb: "STRINGID_STATROSE",
            },
        ),
        ("StatFell", Key("STRINGID_STATFELL")),
        (
            "StatHarshlyFell",
            Sharply {
                adverb: "STRINGID_STATHARSHLY",
                verb: "STRINGID_STATFELL",
            },
        ),
        ("StatWontIncrease", Key("STRINGID_STATSWONTINCREASE")),
        ("StatWontDecrease", Key("STRINGID_STATSWONTDECREASE")),
        ("ButItFailed", Key("STRINGID_BUTITFAILED")),
        ("PkmnWasPoisoned", Key("STRINGID_PKMNWASPOISONED")),
        ("PkmnWasBurned", Key("STRINGID_PKMNWASBURNED")),
        ("PkmnWasParalyzed", Key("STRINGID_PKMNWASPARALYZED")),
        ("PkmnFellAsleep", Key("STRINGID_PKMNFELLASLEEP")),
        ("PkmnBecameConfused", Key("STRINGID_PKMNWASCONFUSED")),
        ("PkmnFlinched", Key("STRINGID_PKMNFLINCHED")),
        ("PkmnIsConfused", Key("STRINGID_PKMNISCONFUSED")),
        ("PkmnHurtItself", Key("STRINGID_ITHURTCONFUSION")),
        ("PkmnIsParalyzed", Key("STRINGID_PKMNISPARALYZED")),
        ("PkmnFastAsleep", Key("STRINGID_PKMNFASTASLEEP")),
        ("PkmnWokeUp", Key("STRINGID_PKMNWOKEUP")),
        ("PkmnHurtByPoison", Key("STRINGID_PKMNHURTBYPOISON")),
        ("PkmnHurtByBurn", Key("STRINGID_PKMNHURTBYBURN")),
        ("PkmnSeeded", Key("STRINGID_PKMNSEEDED")),
        ("LeechSeedDrain", Key("STRINGID_PKMNSAPPEDBYLEECHSEED")),
        ("PkmnStoringEnergy", Key("STRINGID_PKMNSTORINGENERGY")),
        ("PkmnUnleashedEnergy", Key("STRINGID_PKMNUNLEASHEDENERGY")),
        ("GettingPumped", Key("STRINGID_PKMNGETTINGPUMPED")),
        ("AbsorbDrain", Key("STRINGID_PKMNENERGYDRAINED")),
        ("PkmnHitWithRecoil", Key("STRINGID_PKMNHITWITHRECOIL")),
        ("NoPPLeft", Key("STRINGID_NOPPLEFT")),
        ("Struggle", Key("STRINGID_PKMNHASNOMOVESLEFT")),
        ("Gotcha", Key("STRINGID_GOTCHAPKMNCAUGHTPLAYER")),
        ("WildPrefix", Key("sText_WildPkmnPrefix")),
        ("FoePrefix", Key("sText_FoePkmnPrefix")),
        ("BattleYesNoChoice", Key("gText_BattleYesNoChoice")),
        ("ConfirmStarterChoice", Key("gText_ConfirmStarterChoice")),
        ("BirchInTrouble", Key("gText_BirchInTrouble")),
        ("TypeName(t)", TypeNames),
        ("StatName(s)", StatNames),
        // Task E2: statuses, abilities, catching, the cheap move effects.
        ("PkmnBadlyPoisoned", Key("STRINGID_PKMNBADLYPOISONED")),
        ("PkmnWasFrozen", Key("STRINGID_PKMNWASFROZEN")),
        ("PkmnIsFrozen", Key("STRINGID_PKMNISFROZEN")),
        ("PkmnWasDefrosted2", Key("STRINGID_PKMNWASDEFROSTED2")),
        ("PkmnWasDefrosted", Key("STRINGID_PKMNWASDEFROSTED")),
        ("PkmnHealedConfusion", Key("STRINGID_PKMNHEALEDCONFUSION")),
        ("PkmnPoisonedBy", Key("STRINGID_PKMNPOISONEDBY")),
        ("PkmnBurnedBy", Key("STRINGID_PKMNBURNEDBY")),
        ("PkmnFrozenBy", Key("STRINGID_PKMNFROZENBY")),
        ("PkmnWasParalyzedBy", Key("STRINGID_PKMNWASPARALYZEDBY")),
        ("PkmnMadeSleep", Key("STRINGID_PKMNMADESLEEP")),
        ("PkmnAlreadyAsleep", Key("STRINGID_PKMNALREADYASLEEP")),
        ("PkmnAlreadyPoisoned", Key("STRINGID_PKMNALREADYPOISONED")),
        (
            "PkmnIsAlreadyParalyzed",
            Key("STRINGID_PKMNISALREADYPARALYZED"),
        ),
        ("PkmnAlreadyHasBurn", Key("STRINGID_PKMNALREADYHASBURN")),
        ("PkmnAlreadyConfused", Key("STRINGID_PKMNALREADYCONFUSED")),
        ("PkmnEvadedAttack", Key("STRINGID_PKMNEVADEDATTACK")),
        (
            "NaturePowerTurnedInto",
            Key("STRINGID_NATUREPOWERTURNEDINTO"),
        ),
        ("PkmnCutsAttackWith", Key("STRINGID_PKMNCUTSATTACKWITH")),
        ("PkmnTraced", Key("STRINGID_PKMNTRACED")),
        ("PkmnSXPreventsYLoss", Key("STRINGID_PKMNSXPREVENTSYLOSS")),
        ("PkmnSXHadNoEffectOnY", Key("STRINGID_PKMNSXHADNOEFFECTONY")),
        ("PkmnBrokeFree", Key("STRINGID_PKMNBROKEFREE")),
        ("ItAppearedCaught", Key("STRINGID_ITAPPEAREDCAUGHT")),
        ("AarghAlmostHadIt", Key("STRINGID_AARGHALMOSTHADIT")),
        ("ShootSoClose", Key("STRINGID_SHOOTSOCLOSE")),
        ("HitXTimes", Key("STRINGID_HITXTIMES")),
        ("ButNothingHappened", Key("STRINGID_BUTNOTHINGHAPPENED")),
        ("PkmnChargingPower", Key("STRINGID_PKMNCHARGINGPOWER")),
        ("AbilityName(a)", AbilityNames),
        // Task G.
        ("PkmnFellInLove", Key("STRINGID_PKMNFELLINLOVE")),
        ("PkmnInLove", Key("STRINGID_PKMNINLOVE")),
        (
            "PkmnImmobilizedByLove",
            Key("STRINGID_PKMNIMMOBILIZEDBYLOVE"),
        ),
        (
            "PkmnPreventsRomanceWith",
            Key("STRINGID_PKMNPREVENTSROMANCEWITH"),
        ),
        ("PkmnSXInfatuatedY", Key("STRINGID_PKMNSXINFATUATEDY")),
        ("TargetCantEscapeNow", Key("STRINGID_TARGETCANTESCAPENOW")),
        ("PreventsEscape", Key("STRINGID_PREVENTSESCAPE")),
        ("ElectricityWeakened", Key("STRINGID_ELECTRICITYWEAKENED")),
        ("FireWeakened", Key("STRINGID_FIREWEAKENED")),
        ("PkmnFledFromBattle", Key("STRINGID_PKMNFLEDFROMBATTLE")),
        (
            "PkmnSXMadeItIneffective",
            Key("STRINGID_PKMNSXMADEITINEFFECTIVE"),
        ),
        ("PkmnCausedUproar", Key("STRINGID_PKMNCAUSEDUPROAR")),
        ("PkmnMakingUproar", Key("STRINGID_PKMNMAKINGUPROAR")),
        ("PkmnCalmedDown", Key("STRINGID_PKMNCALMEDDOWN")),
        ("PkmnWokeUpInUproar", Key("STRINGID_PKMNWOKEUPINUPROAR")),
        (
            "PkmnCantSleepInUproar",
            Key("STRINGID_PKMNCANTSLEEPINUPROAR2"),
        ),
        ("UproarKeptPkmnAwake", Key("STRINGID_UPROARKEPTPKMNAWAKE")),
        ("PkmnStayedAwakeUsing", Key("STRINGID_PKMNSTAYEDAWAKEUSING")),
        ("PkmnWasMadeDrowsy", Key("STRINGID_PKMNWASMADEDROWSY")),
        ("PkmnIdentified", Key("STRINGID_PKMNIDENTIFIED")),
        ("PkmnLoafing", Key("STRINGID_PKMNLOAFING")),
        ("PkmnSXCuredYProblem", Key("STRINGID_PKMNSXCUREDYPROBLEM")),
        (
            "PkmnSXPreventsFlinching",
            Key("STRINGID_PKMNSXPREVENTSFLINCHING"),
        ),
        ("PkmnSXBlocksY", Key("STRINGID_PKMNSXBLOCKSY")),
        ("PkmnProtectedBy", Key("STRINGID_PKMNPROTECTEDBY")),
        (
            "PkmnPreventsStatLossWith",
            Key("STRINGID_PKMNPREVENTSSTATLOSSWITH"),
        ),
        ("PreventedFromWorking", Key("STRINGID_PREVENTEDFROMWORKING")),
        ("ItSuckedLiquidOoze", Key("STRINGID_ITSUCKEDLIQUIDOOZE")),
        ("PlayerGotMoney", Key("STRINGID_PLAYERGOTMONEY")),
        // Silice engine: held items, weather, abilities.
        (
            "ItemRestoredHealth",
            Key("STRINGID_PKMNSITEMRESTOREDHEALTH"),
        ),
        (
            "ItemRestoredHpALittle",
            Key("STRINGID_PKMNSITEMRESTOREDHPALITTLE"),
        ),
        (
            "ItemCuredParalysis",
            Key("STRINGID_PKMNSITEMCUREDPARALYSIS"),
        ),
        ("ItemCuredPoison", Key("STRINGID_PKMNSITEMCUREDPOISON")),
        ("ItemHealedBurn", Key("STRINGID_PKMNSITEMHEALEDBURN")),
        ("ItemDefrostedIt", Key("STRINGID_PKMNSITEMDEFROSTEDIT")),
        ("ItemWokeIt", Key("STRINGID_PKMNSITEMWOKEIT")),
        ("ItemSnappedOut", Key("STRINGID_PKMNSITEMSNAPPEDOUT")),
        ("ItemCuredProblem", Key("STRINGID_PKMNSITEMCUREDPROBLEM")),
        (
            "ItemNormalizedStatus",
            Key("STRINGID_PKMNSITEMNORMALIZEDSTATUS"),
        ),
        ("HungOnWithX", Key("STRINGID_PKMNHUNGONWITHX")),
        ("IntensifiedSun", Key("STRINGID_PKMNSXINTENSIFIEDSUN")),
        (
            "WhippedUpSandstorm",
            Key("STRINGID_PKMNSXWHIPPEDUPSANDSTORM"),
        ),
        ("MadeItRain", Key("STRINGID_PKMNMADEITRAIN")),
        ("RainContinues", Key("STRINGID_RAINCONTINUES")),
        ("SunlightStrong", Key("STRINGID_SUNLIGHTSTRONG")),
        ("SandstormRages", Key("STRINGID_SANDSTORMRAGES")),
        (
            "BuffetedBySandstorm",
            Key("STRINGID_PKMNBUFFETEDBYSANDSTORM"),
        ),
        ("RaisedSpeed", Key("STRINGID_PKMNRAISEDSPEED")),
        ("MakesGroundMiss", Key("STRINGID_PKMNMAKESGROUNDMISS")),
        (
            "AbilityRestoredHp",
            Key("STRINGID_PKMNSXRESTOREDHPALITTLE2"),
        ),
        // Post-Gen3: no Gen3 string.
        ("MegaReacting", Missing),
        ("MegaEvolved", Missing),
        ("PrimalReversion", Missing),
        ("HeavyRainBegan", Missing),
        ("HarshSunBegan", Missing),
        ("HeavyRainLifted", Missing),
        ("HarshSunFaded", Missing),
        ("FizzledInRain", Missing),
        ("EvaporatedInSun", Missing),
        ("BreaksTheMold", Missing),
        ("BouncedBack", Missing),
        ("MegaArmed", Missing),
        ("SolarPowerHurt", Missing),
    ]
};
