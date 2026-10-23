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
        "ITEM_CLEANSE_TAG" => "HoldEffect::CleanseTag",
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
        // Wave C. `{SPECIES_*}` placeholders are the build's species ids
        // ([`hold_effect_species`]).
        "ITEM_WHITE_HERB" => "HoldEffect::RestoreStats",
        "ITEM_LIECHI_BERRY" => "HoldEffect::StatUp(Stat::Atk, 4)",
        "ITEM_GANLON_BERRY" => "HoldEffect::StatUp(Stat::Def, 4)",
        "ITEM_SALAC_BERRY" => "HoldEffect::StatUp(Stat::Speed, 4)",
        "ITEM_PETAYA_BERRY" => "HoldEffect::StatUp(Stat::SpAtk, 4)",
        "ITEM_APICOT_BERRY" => "HoldEffect::StatUp(Stat::SpDef, 4)",
        "ITEM_LANSAT_BERRY" => "HoldEffect::CriticalUp(4)",
        "ITEM_STARF_BERRY" => "HoldEffect::RandomStatUp(4)",
        // FLAVOR_SPICY, _DRY, _SWEET, _BITTER, _SOUR.
        "ITEM_FIGY_BERRY" => "HoldEffect::ConfuseBerry { flavor: 0, param: 8 }",
        "ITEM_WIKI_BERRY" => "HoldEffect::ConfuseBerry { flavor: 1, param: 8 }",
        "ITEM_MAGO_BERRY" => "HoldEffect::ConfuseBerry { flavor: 2, param: 8 }",
        "ITEM_AGUAV_BERRY" => "HoldEffect::ConfuseBerry { flavor: 3, param: 8 }",
        "ITEM_IAPAPA_BERRY" => "HoldEffect::ConfuseBerry { flavor: 4, param: 8 }",
        "ITEM_LEPPA_BERRY" => "HoldEffect::RestorePp(10)",
        "ITEM_MENTAL_HERB" => "HoldEffect::CureAttract",
        "ITEM_SHELL_BELL" => "HoldEffect::ShellBell(8)",
        "ITEM_SCOPE_LENS" => "HoldEffect::ScopeLens",
        "ITEM_LUCKY_PUNCH" => "HoldEffect::SpeciesCrit({SPECIES_CHANSEY})",
        "ITEM_STICK" => "HoldEffect::SpeciesCrit({SPECIES_FARFETCHD})",
        "ITEM_DEEP_SEA_TOOTH" => "HoldEffect::DeepSeaTooth({SPECIES_CLAMPERL})",
        "ITEM_DEEP_SEA_SCALE" => "HoldEffect::DeepSeaScale({SPECIES_CLAMPERL})",
        "ITEM_LIGHT_BALL" => "HoldEffect::LightBall({SPECIES_PIKACHU})",
        "ITEM_METAL_POWDER" => "HoldEffect::MetalPowder({SPECIES_DITTO})",
        "ITEM_THICK_CLUB" => "HoldEffect::ThickClub({SPECIES_CUBONE}, {SPECIES_MAROWAK})",
        "ITEM_SOUL_DEW" => "HoldEffect::SoulDew({SPECIES_LATIAS}, {SPECIES_LATIOS})",
        "ITEM_MACHO_BRACE" => "HoldEffect::MachoBrace",
        "ITEM_SMOKE_BALL" => "HoldEffect::CanAlwaysRun",
        "ITEM_AMULET_COIN" => "HoldEffect::DoublePrize",
        "ITEM_EXP_SHARE" => "HoldEffect::ExpShare",
        "ITEM_LUCKY_EGG" => "HoldEffect::LuckyEgg",
        "ITEM_SOOTHE_BELL" => "HoldEffect::FriendshipUp",
        "ITEM_EVERSTONE" => "HoldEffect::PreventEvolve",
        "ITEM_DRAGON_SCALE" | "ITEM_UP_GRADE" => "HoldEffect::TradeItem",
        "ITEM_ENIGMA_BERRY" => "HoldEffect::NoSwap",
        _ if name.starts_with("ITEM_") && name.ends_with("_MAIL") => "HoldEffect::NoSwap",
        _ => return None,
    })
}

/// [`hold_effect`]'s expression with each `{SPECIES_*}` placeholder replaced by
/// `species(name)` (`None` for a species the build lacks: the item gets no effect).
pub fn hold_effect_species(expr: &str, species: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = expr;
    while let Some(i) = rest.find('{') {
        let Some(j) = rest[i..].find('}') else { break };
        let name = &rest[i + 1..i + j];
        if !name.starts_with("SPECIES_") {
            out.push_str(&rest[..i + j + 1]);
            rest = &rest[i + j + 1..];
            continue;
        }
        out.push_str(&rest[..i]);
        out.push_str(&species(name)?);
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    Some(out)
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
        // Field-only in Gen3 (the encounter rate; no battle effect).
        "ABILITY_STENCH" => "Ability::Stench",
        "ABILITY_ILLUMINATE" => "Ability::Illuminate",
        // Wave C: the remaining Gen3 abilities (battle and encounter effects).
        "ABILITY_WHITE_SMOKE" => "Ability::WhiteSmoke",
        "ABILITY_ARENA_TRAP" => "Ability::ArenaTrap",
        "ABILITY_PRESSURE" => "Ability::Pressure",
        "ABILITY_MAGMA_ARMOR" => "Ability::MagmaArmor",
        "ABILITY_FLAME_BODY" => "Ability::FlameBody",
        "ABILITY_PLUS" => "Ability::Plus",
        "ABILITY_MINUS" => "Ability::Minus",
        "ABILITY_WATER_VEIL" => "Ability::WaterVeil",
        "ABILITY_ROUGH_SKIN" => "Ability::RoughSkin",
        "ABILITY_STICKY_HOLD" => "Ability::StickyHold",
        "ABILITY_HYPER_CUTTER" => "Ability::HyperCutter",
        "ABILITY_OWN_TEMPO" => "Ability::OwnTempo",
        "ABILITY_COLOR_CHANGE" => "Ability::ColorChange",
        "ABILITY_IMMUNITY" => "Ability::Immunity",
        "ABILITY_FORECAST" => "Ability::Forecast",
        "ABILITY_FLASH_FIRE" => "Ability::FlashFire",
        "ABILITY_VOLT_ABSORB" => "Ability::VoltAbsorb",
        "ABILITY_WATER_ABSORB" => "Ability::WaterAbsorb",
        "ABILITY_LIMBER" => "Ability::Limber",
        "ABILITY_SUCTION_CUPS" => "Ability::SuctionCups",
        "ABILITY_AIR_LOCK" => "Ability::AirLock",
        "ABILITY_CLOUD_NINE" => "Ability::CloudNine",
        "ABILITY_CACOPHONY" => "Ability::Cacophony",
        "ABILITY_DAMP" => "Ability::Damp",
        "ABILITY_SERENE_GRACE" => "Ability::SereneGrace",
        "ABILITY_WONDER_GUARD" => "Ability::WonderGuard",
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
        "ABILITY_SAND_VEIL" if silice => "Ability::SandVeil",
        "ABILITY_SAND_VEIL" if gen3_extra => "Ability::SandVeilGen3",
        // Gen3's Lightning Rod only redirects in doubles: nothing in singles.
        "ABILITY_LIGHTNING_ROD" if silice => "Ability::LightningRod",
        "ABILITY_LIGHTNING_ROD" => "Ability::LightningRodGen3",
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
/// species, with their Gen3 behaviour. **On** since Wave C (an Emerald route change: Torkoal,
/// Tyranitar, Ninjask, Wobbuffet, the Levitate ghosts …; the checkpoint tree needs a
/// re-baseline). Not modelled: Sand Veil's overworld encounter halving (no field weather).
pub const GEN3_EXTRA_ABILITIES: bool = true;

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
            // Wave C.
            "ALWAYS_HIT" => "MoveEffect::AlwaysHit",
            "HIGH_CRITICAL" => "MoveEffect::HighCritical(None)",
            "BLAZE_KICK" => "MoveEffect::HighCritical(Some(StatusKind::Burn))",
            "POISON_TAIL" => "MoveEffect::HighCritical(Some(StatusKind::Poison))",
            "TRI_ATTACK" => "MoveEffect::TriAttack",
            "THAW_HIT" => "MoveEffect::ThawHit",
            "POISON_FANG" => "MoveEffect::PoisonFang",
            "GUST" => "MoveEffect::Gust",
            "TWISTER" => "MoveEffect::Twister",
            "SKY_UPPERCUT" => "MoveEffect::SkyUppercut",
            "THUNDER" => "MoveEffect::Thunder",
            "EARTHQUAKE" => "MoveEffect::Earthquake",
            "ATTACK_UP_HIT" => "MoveEffect::StatUpHit(Stat::Atk)",
            "DEFENSE_UP_HIT" => "MoveEffect::StatUpHit(Stat::Def)",
            "ALL_STATS_UP_HIT" => "MoveEffect::AllStatsUpHit",
            "FAKE_OUT" => "MoveEffect::FakeOut",
            "FALSE_SWIPE" => "MoveEffect::FalseSwipe",
            "PAY_DAY" => "MoveEffect::PayDay",
            "RAPID_SPIN" => "MoveEffect::RapidSpin",
            "SECRET_POWER" => "MoveEffect::SecretPower",
            "SUPERPOWER" => "MoveEffect::Superpower",
            "OVERHEAT" => "MoveEffect::Overheat",
            "KNOCK_OFF" => "MoveEffect::KnockOff",
            "THIEF" => "MoveEffect::Thief",
            "SMELLINGSALT" => "MoveEffect::SmellingSalt",
            "FACADE" => "MoveEffect::Facade",
            "REVENGE" => "MoveEffect::Revenge",
            "BRICK_BREAK" => "MoveEffect::BrickBreak",
            "RECOIL_IF_MISS" => "MoveEffect::RecoilIfMiss",
            "DOUBLE_EDGE" => "MoveEffect::DoubleEdge",
            "RAMPAGE" => "MoveEffect::Rampage",
            "RECHARGE" => "MoveEffect::Recharge",
            // Refined per move by `move_effect_for`.
            "TRAP" => "MoveEffect::Trap(TrapKind::Wrap)",
            "TWINEEDLE" => "MoveEffect::Twineedle",
            "TRIPLE_KICK" => "MoveEffect::TripleKick",
            "DREAM_EATER" => "MoveEffect::DreamEater",
            "EXPLOSION" => "MoveEffect::Explosion",
            "OHKO" => "MoveEffect::Ohko",
            "SUPER_FANG" => "MoveEffect::SuperFang",
            "DRAGON_RAGE" => "MoveEffect::DragonRage",
            "LEVEL_DAMAGE" => "MoveEffect::LevelDamage",
            "PSYWAVE" => "MoveEffect::Psywave",
            "COUNTER" => "MoveEffect::Counter",
            "MIRROR_COAT" => "MoveEffect::MirrorCoat",
            "ENDEAVOR" => "MoveEffect::Endeavor",
            "FLAIL" => "MoveEffect::Flail",
            "ERUPTION" => "MoveEffect::Eruption",
            "RETURN" => "MoveEffect::Return",
            "FRUSTRATION" => "MoveEffect::Frustration",
            "PRESENT" => "MoveEffect::Present",
            "MAGNITUDE" => "MoveEffect::Magnitude",
            "HIDDEN_POWER" => "MoveEffect::HiddenPower",
            "WEATHER_BALL" => "MoveEffect::WeatherBall",
            "ROLLOUT" => "MoveEffect::Rollout",
            "RAGE" => "MoveEffect::Rage",
            "SNORE" => "MoveEffect::Snore",
            "FOCUS_PUNCH" => "MoveEffect::FocusPunch",
            "RAZOR_WIND" => "MoveEffect::TwoTurn(TwoTurnKind::RazorWind)",
            "SKY_ATTACK" => "MoveEffect::TwoTurn(TwoTurnKind::SkyAttack)",
            "SKULL_BASH" => "MoveEffect::TwoTurn(TwoTurnKind::SkullBash)",
            "SOLAR_BEAM" => "MoveEffect::TwoTurn(TwoTurnKind::SolarBeam)",
            // Refined per move by `move_effect_for`.
            "SEMI_INVULNERABLE" => "MoveEffect::SemiInvulnerable(SemiKind::Dig)",
            "STOCKPILE" => "MoveEffect::Stockpile",
            "SPIT_UP" => "MoveEffect::SpitUp",
            "SWALLOW" => "MoveEffect::Swallow",
            "FUTURE_SIGHT" => "MoveEffect::FutureSight(false)",
            "WILL_O_WISP" => "MoveEffect::Status(StatusKind::Burn)",
            "SWAGGER" => "MoveEffect::Swagger",
            "FLATTER" => "MoveEffect::Flatter",
            "TEETER_DANCE" => "MoveEffect::TeeterDance",
            "NIGHTMARE" => "MoveEffect::Nightmare",
            "CURSE" => "MoveEffect::Curse",
            "REST" => "MoveEffect::Rest",
            "RESTORE_HP" | "SOFTBOILED" => "MoveEffect::RestoreHp",
            "MORNING_SUN" | "SYNTHESIS" | "MOONLIGHT" => "MoveEffect::WeatherHeal",
            "HEAL_BELL" => "MoveEffect::HealBell(false)",
            "REFRESH" => "MoveEffect::Refresh",
            "PAIN_SPLIT" => "MoveEffect::PainSplit",
            "BELLY_DRUM" => "MoveEffect::BellyDrum",
            "PSYCH_UP" => "MoveEffect::PsychUp",
            "HAZE" => "MoveEffect::Haze",
            "COSMIC_POWER" => "MoveEffect::StatsUp(Stat::Def, Stat::SpDef)",
            "BULK_UP" => "MoveEffect::StatsUp(Stat::Atk, Stat::Def)",
            "CALM_MIND" => "MoveEffect::StatsUp(Stat::SpAtk, Stat::SpDef)",
            "DRAGON_DANCE" => "MoveEffect::StatsUp(Stat::Atk, Stat::Speed)",
            "TICKLE" => "MoveEffect::Tickle",
            "MEMENTO" => "MoveEffect::Memento",
            "MINIMIZE" => "MoveEffect::Minimize",
            "CONVERSION" => "MoveEffect::Conversion",
            "CAMOUFLAGE" => "MoveEffect::Camouflage",
            "METRONOME" => "MoveEffect::Metronome",
            "MIRROR_MOVE" => "MoveEffect::MirrorMove",
            "SLEEP_TALK" => "MoveEffect::SleepTalk",
            "ASSIST" => "MoveEffect::Assist",
            "DISABLE" => "MoveEffect::Disable",
            "ENCORE" => "MoveEffect::Encore",
            "TORMENT" => "MoveEffect::Torment",
            "TAUNT" => "MoveEffect::Taunt",
            "SPITE" => "MoveEffect::Spite",
            "DESTINY_BOND" => "MoveEffect::DestinyBond",
            "LOCK_ON" => "MoveEffect::LockOn",
            "PERISH_SONG" => "MoveEffect::PerishSong",
            "ROAR" => "MoveEffect::Roar",
            "PROTECT" => "MoveEffect::Protect",
            "ENDURE" => "MoveEffect::Endure",
            "SPIKES" => "MoveEffect::Spikes",
            "LIGHT_SCREEN" => "MoveEffect::LightScreen",
            "REFLECT" => "MoveEffect::Reflect",
            "MIST" => "MoveEffect::Mist",
            "SAFEGUARD" => "MoveEffect::Safeguard",
            "RAIN_DANCE" => "MoveEffect::SetWeather(WeatherKind::Rain)",
            "SUNNY_DAY" => "MoveEffect::SetWeather(WeatherKind::Sun)",
            "SANDSTORM" => "MoveEffect::SetWeather(WeatherKind::Sandstorm)",
            "HAIL" => "MoveEffect::SetWeather(WeatherKind::Hail)",
            "INGRAIN" => "MoveEffect::Ingrain",
            "WISH" => "MoveEffect::Wish",
            "HELPING_HAND" => "MoveEffect::HelpingHand",
            "FOLLOW_ME" => "MoveEffect::FollowMe",
            "ROLE_PLAY" => "MoveEffect::RolePlay",
            "SKILL_SWAP" => "MoveEffect::SkillSwap",
            "GRUDGE" => "MoveEffect::Grudge",
            "IMPRISON" => "MoveEffect::Imprison",
            "MAGIC_COAT" => "MoveEffect::MagicCoat",
            "TRICK" => "MoveEffect::Trick",
            "SUBSTITUTE" => "MoveEffect::Substitute",
            "BATON_PASS" => "MoveEffect::BatonPass",
            "TRANSFORM" => "MoveEffect::Transform",
            "SKETCH" => "MoveEffect::Sketch",
            "MIMIC" => "MoveEffect::Mimic",
            "RECYCLE" => "MoveEffect::Recycle",
            "CONVERSION_2" => "MoveEffect::Conversion2",
            "SNATCH" => "MoveEffect::Snatch",
            "BEAT_UP" => "MoveEffect::BeatUp",
            _ => return None,
        }
        .to_string(),
    )
}

/// [`move_effect`] for move `id` (`MOVE_*`): the effects whose message or behaviour depends on
/// the move (`gTrappingMoves`' `gWrappedStringIds`, the semi-invulnerable kind, Aromatherapy,
/// Doom Desire).
pub fn move_effect_for(name: &str, id: &str) -> Option<String> {
    let refined = match (name, id) {
        ("EFFECT_TRAP", "MOVE_BIND") => "MoveEffect::Trap(TrapKind::Bind)",
        ("EFFECT_TRAP", "MOVE_FIRE_SPIN") => "MoveEffect::Trap(TrapKind::FireSpin)",
        ("EFFECT_TRAP", "MOVE_CLAMP") => "MoveEffect::Trap(TrapKind::Clamp)",
        ("EFFECT_TRAP", "MOVE_WHIRLPOOL") => "MoveEffect::Trap(TrapKind::Whirlpool)",
        ("EFFECT_TRAP", "MOVE_SAND_TOMB") => "MoveEffect::Trap(TrapKind::SandTomb)",
        ("EFFECT_SEMI_INVULNERABLE", "MOVE_FLY") => "MoveEffect::SemiInvulnerable(SemiKind::Fly)",
        ("EFFECT_SEMI_INVULNERABLE", "MOVE_BOUNCE") => {
            "MoveEffect::SemiInvulnerable(SemiKind::Bounce)"
        }
        ("EFFECT_SEMI_INVULNERABLE", "MOVE_DIVE") => "MoveEffect::SemiInvulnerable(SemiKind::Dive)",
        ("EFFECT_HEAL_BELL", "MOVE_AROMATHERAPY") => "MoveEffect::HealBell(true)",
        ("EFFECT_FUTURE_SIGHT", "MOVE_DOOM_DESIRE") => "MoveEffect::FutureSight(true)",
        _ => return move_effect(name),
    };
    Some(refined.to_string())
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
        // Wave C: move effects.
        ("PkmnRegainedHealth", Key("STRINGID_PKMNREGAINEDHEALTH")),
        ("PkmnHpFull", Key("STRINGID_PKMNHPFULL")),
        ("StatChangesGone", Key("STRINGID_STATCHANGESGONE")),
        ("PkmnWasntAffected", Key("STRINGID_PKMNWASNTAFFECTED")),
        ("PkmnDreamEaten", Key("STRINGID_PKMNDREAMEATEN")),
        ("CoinsScattered", Key("STRINGID_COINSSCATTERED")),
        ("PkmnCrashed", Key("STRINGID_PKMNCRASHED")),
        ("PkmnMustRecharge", Key("STRINGID_PKMNMUSTRECHARGE")),
        ("PkmnRageBuilding", Key("STRINGID_PKMNRAGEBUILDING")),
        (
            "PkmnFellIntoNightmare",
            Key("STRINGID_PKMNFELLINTONIGHTMARE"),
        ),
        (
            "PkmnLockedInNightmare",
            Key("STRINGID_PKMNLOCKEDINNIGHTMARE"),
        ),
        ("PkmnLaidCurse", Key("STRINGID_PKMNLAIDCURSE")),
        ("PkmnAfflictedByCurse", Key("STRINGID_PKMNAFFLICTEDBYCURSE")),
        ("PkmnProtectedItself2", Key("STRINGID_PKMNPROTECTEDITSELF2")),
        ("PkmnBracedItself", Key("STRINGID_PKMNBRACEDITSELF")),
        ("PkmnProtectedItself", Key("STRINGID_PKMNPROTECTEDITSELF")),
        ("PkmnEnduredHit", Key("STRINGID_PKMNENDUREDHIT")),
        ("SpikesScattered", Key("STRINGID_SPIKESSCATTERED")),
        ("PkmnHurtBySpikes", Key("STRINGID_PKMNHURTBYSPIKES")),
        ("FaintInThree", Key("STRINGID_FAINTINTHREE")),
        ("PkmnPerishCountFell", Key("STRINGID_PKMNPERISHCOUNTFELL")),
        ("StartedToRain", Key("STRINGID_STARTEDTORAIN")),
        ("SunlightGotBright", Key("STRINGID_SUNLIGHTGOTBRIGHT")),
        ("SandstormBrewed", Key("STRINGID_SANDSTORMBREWED")),
        ("StartedHail", Key("STRINGID_STARTEDHAIL")),
        ("RainStopped", Key("STRINGID_RAINSTOPPED")),
        ("SunlightFaded", Key("STRINGID_SUNLIGHTFADED")),
        ("SandstormSubsided", Key("STRINGID_SANDSTORMSUBSIDED")),
        ("HailContinues", Key("STRINGID_HAILCONTINUES")),
        ("HailStopped", Key("STRINGID_HAILSTOPPED")),
        ("PkmnPeltedByHail", Key("STRINGID_PKMNPELTEDBYHAIL")),
        ("PkmnRaisedDef", Key("STRINGID_PKMNRAISEDDEF")),
        ("PkmnRaisedSpDef", Key("STRINGID_PKMNRAISEDSPDEF")),
        ("PkmnCoveredByVeil", Key("STRINGID_PKMNCOVEREDBYVEIL")),
        ("PkmnSXWoreOff", Key("STRINGID_PKMNSXWOREOFF")),
        ("PkmnSafeguardExpired", Key("STRINGID_PKMNSAFEGUARDEXPIRED")),
        ("PkmnUsedSafeguard", Key("STRINGID_PKMNUSEDSAFEGUARD")),
        ("PkmnShroudedInMist", Key("STRINGID_PKMNSHROUDEDINMIST")),
        ("PkmnProtectedByMist", Key("STRINGID_PKMNPROTECTEDBYMIST")),
        ("PkmnWishCameTrue", Key("STRINGID_PKMNWISHCAMETRUE")),
        ("PkmnPlantedRoots", Key("STRINGID_PKMNPLANTEDROOTS")),
        (
            "PkmnAbsorbedNutrients",
            Key("STRINGID_PKMNABSORBEDNUTRIENTS"),
        ),
        ("PkmnAnchoredItself", Key("STRINGID_PKMNANCHOREDITSELF")),
        ("PkmnCenterAttention", Key("STRINGID_PKMNCENTERATTENTION")),
        ("PkmnSqueezedByBind", Key("STRINGID_PKMNSQUEEZEDBYBIND")),
        ("PkmnWrappedBy", Key("STRINGID_PKMNWRAPPEDBY")),
        ("PkmnTrappedInVortex", Key("STRINGID_PKMNTRAPPEDINVORTEX")),
        ("PkmnClamped", Key("STRINGID_PKMNCLAMPED")),
        (
            "PkmnTrappedBySandTomb",
            Key("STRINGID_PKMNTRAPPEDBYSANDTOMB"),
        ),
        ("PkmnHurtBy", Key("STRINGID_PKMNHURTBY")),
        ("PkmnFreedFrom", Key("STRINGID_PKMNFREEDFROM")),
        ("PkmnGotFree", Key("STRINGID_PKMNGOTFREE")),
        ("PkmnShedLeechSeed", Key("STRINGID_PKMNSHEDLEECHSEED")),
        ("PkmnBlewAwaySpikes", Key("STRINGID_PKMNBLEWAWAYSPIKES")),
        ("PkmnFatigueConfusion", Key("STRINGID_PKMNFATIGUECONFUSION")),
        ("PkmnTookAim", Key("STRINGID_PKMNTOOKAIM")),
        ("PkmnWasDraggedOut", Key("STRINGID_PKMNWASDRAGGEDOUT")),
        ("PkmnLostFocus", Key("STRINGID_PKMNLOSTFOCUS")),
        ("PkmnTighteningFocus", Key("STRINGID_PKMNTIGHTENINGFOCUS")),
        ("PkmnStockpiled", Key("STRINGID_PKMNSTOCKPILED")),
        ("PkmnCantStockpile", Key("STRINGID_PKMNCANTSTOCKPILE")),
        ("FailedToSpitUp", Key("STRINGID_FAILEDTOSPITUP")),
        ("FailedToSwallow", Key("STRINGID_FAILEDTOSWALLOW")),
        ("PkmnForesawAttack", Key("STRINGID_PKMNFORESAWATTACK")),
        ("PkmnChoseXAsDestiny", Key("STRINGID_PKMNCHOSEXASDESTINY")),
        ("PkmnTookAttack", Key("STRINGID_PKMNTOOKATTACK")),
        ("PkmnCutHpMaxedAttack", Key("STRINGID_PKMNCUTHPMAXEDATTACK")),
        (
            "PkmnCopiedStatChanges",
            Key("STRINGID_PKMNCOPIEDSTATCHANGES"),
        ),
        ("PkmnChangedType", Key("STRINGID_PKMNCHANGEDTYPE")),
        ("PkmnMoveWasDisabled", Key("STRINGID_PKMNMOVEWASDISABLED")),
        ("PkmnMoveIsDisabled", Key("STRINGID_PKMNMOVEISDISABLED")),
        (
            "PkmnMoveDisabledNoMore",
            Key("STRINGID_PKMNMOVEDISABLEDNOMORE"),
        ),
        ("PkmnGotEncore", Key("STRINGID_PKMNGOTENCORE")),
        ("PkmnEncoreEnded", Key("STRINGID_PKMNENCOREENDED")),
        (
            "PkmnSubjectedToTorment",
            Key("STRINGID_PKMNSUBJECTEDTOTORMENT"),
        ),
        (
            "PkmnCantUseMoveTorment",
            Key("STRINGID_PKMNCANTUSEMOVETORMENT"),
        ),
        ("PkmnFellForTaunt", Key("STRINGID_PKMNFELLFORTAUNT")),
        ("PkmnCantUseMoveTaunt", Key("STRINGID_PKMNCANTUSEMOVETAUNT")),
        ("PkmnReducedPP", Key("STRINGID_PKMNREDUCEDPP")),
        ("PkmnTryingToTakeFoe", Key("STRINGID_PKMNTRYINGTOTAKEFOE")),
        ("PkmnTookFoe", Key("STRINGID_PKMNTOOKFOE")),
        ("PkmnStatusNormal", Key("STRINGID_PKMNSTATUSNORMAL")),
        ("BellChimed", Key("STRINGID_BELLCHIMED")),
        ("SoothingAroma", Key("STRINGID_SOOTHINGAROMA")),
        ("SharedPain", Key("STRINGID_SHAREDPAIN")),
        ("PkmnStoleItem", Key("STRINGID_PKMNSTOLEITEM")),
        ("PkmnKnockedOff", Key("STRINGID_PKMNKNOCKEDOFF")),
        ("TheWallShattered", Key("STRINGID_THEWALLSHATTERED")),
        ("MagnitudeStrength", Key("STRINGID_MAGNITUDESTRENGTH")),
        ("PkmnWhippedWhirlwind", Key("STRINGID_PKMNWHIPPEDWHIRLWIND")),
        ("PkmnTookSunlight", Key("STRINGID_PKMNTOOKSUNLIGHT")),
        ("PkmnLoweredHead", Key("STRINGID_PKMNLOWEREDHEAD")),
        ("PkmnIsGlowing", Key("STRINGID_PKMNISGLOWING")),
        ("PkmnFlewHigh", Key("STRINGID_PKMNFLEWHIGH")),
        ("PkmnDugHole", Key("STRINGID_PKMNDUGHOLE")),
        ("PkmnHidUnderwater", Key("STRINGID_PKMNHIDUNDERWATER")),
        ("PkmnSprangUp", Key("STRINGID_PKMNSPRANGUP")),
        ("ButNoEffect", Key("STRINGID_BUTNOEFFECT")),
        ("OneHitKo", Key("STRINGID_ONEHITKO")),
        ("PkmnUnaffected", Key("STRINGID_PKMNUNAFFECTED")),
        ("MirrorMoveFailed", Key("STRINGID_MIRRORMOVEFAILED")),
        ("PkmnSleptHealthy", Key("STRINGID_PKMNSLEPTHEALTHY")),
        ("PkmnWentToSleep", Key("STRINGID_PKMNWENTTOSLEEP")),
        ("PkmnWasDefrostedBy", Key("STRINGID_PKMNWASDEFROSTEDBY")),
        ("PkmnAlreadyAsleep2", Key("STRINGID_PKMNALREADYASLEEP2")),
        ("PkmnSXBlocksY2", Key("STRINGID_PKMNSXBLOCKSY2")),
        ("StatsWontIncrease2", Key("STRINGID_STATSWONTINCREASE2")),
        ("StatsWontDecrease2", Key("STRINGID_STATSWONTDECREASE2")),
        ("PkmnHealedParalysis", Key("STRINGID_PKMNHEALEDPARALYSIS")),
        ("PkmnCopiedFoe", Key("STRINGID_PKMNCOPIEDFOE")),
        ("PkmnSwappedAbilities", Key("STRINGID_PKMNSWAPPEDABILITIES")),
        ("PkmnWantsGrudge", Key("STRINGID_PKMNWANTSGRUDGE")),
        ("PkmnLostPPGrudge", Key("STRINGID_PKMNLOSTPPGRUDGE")),
        (
            "PkmnSealedOpponentMove",
            Key("STRINGID_PKMNSEALEDOPPONENTMOVE"),
        ),
        (
            "PkmnCantUseMoveSealed",
            Key("STRINGID_PKMNCANTUSEMOVESEALED"),
        ),
        ("PkmnShroudedItself", Key("STRINGID_PKMNSHROUDEDITSELF")),
        ("PkmnMoveBounced", Key("STRINGID_PKMNMOVEBOUNCED")),
        ("PkmnSwitchedItems", Key("STRINGID_PKMNSWITCHEDITEMS")),
        ("PkmnObtainedX", Key("STRINGID_PKMNOBTAINEDX")),
        ("PkmnObtainedX2", Key("STRINGID_PKMNOBTAINEDX2")),
        (
            "PkmnObtainedXYObtainedZ",
            Key("STRINGID_PKMNOBTAINEDXYOBTAINEDZ"),
        ),
        (
            "PkmnAnchorsItselfWith",
            Key("STRINGID_PKMNANCHORSITSELFWITH"),
        ),
        ("PkmnMadeSubstitute", Key("STRINGID_PKMNMADESUBSTITUTE")),
        ("PkmnHasSubstitute", Key("STRINGID_PKMNHASSUBSTITUTE")),
        ("SubstituteDamaged", Key("STRINGID_SUBSTITUTEDAMAGED")),
        ("PkmnSubstituteFaded", Key("STRINGID_PKMNSUBSTITUTEFADED")),
        ("TooWeakForSubstitute", Key("STRINGID_TOOWEAKFORSUBSTITUTE")),
        ("PkmnTransformedInto", Key("STRINGID_PKMNTRANSFORMEDINTO")),
        ("PkmnLearnedMove2", Key("STRINGID_PKMNLEARNEDMOVE2")),
        ("PkmnSketchedMove", Key("STRINGID_PKMNSKETCHEDMOVE")),
        ("XFoundOneY", Key("STRINGID_XFOUNDONEY")),
        ("PkmnWaitsForTarget", Key("STRINGID_PKMNWAITSFORTARGET")),
        ("PkmnSnatchedMove", Key("STRINGID_PKMNSNATCHEDMOVE")),
        ("PkmnAttack", Key("STRINGID_PKMNATTACK")),
        ("PlayerPickedUpMoney", Key("STRINGID_PLAYERPICKEDUPMONEY")),
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
        // Wave C: abilities and held items.
        (
            "PkmnPreventsParalysisWith",
            Key("STRINGID_PKMNPREVENTSPARALYSISWITH"),
        ),
        (
            "PkmnPreventsPoisoningWith",
            Key("STRINGID_PKMNPREVENTSPOISONINGWITH"),
        ),
        (
            "PkmnPreventsConfusionWith",
            Key("STRINGID_PKMNPREVENTSCONFUSIONWITH"),
        ),
        ("PkmnSXPreventsBurns", Key("STRINGID_PKMNSXPREVENTSBURNS")),
        ("PkmnSXPreventsYSZ", Key("STRINGID_PKMNSXPREVENTSYSZ")),
        ("PkmnRestoredHpUsing", Key("STRINGID_PKMNRESTOREDHPUSING")),
        ("PkmnSXMadeYUseless", Key("STRINGID_PKMNSXMADEYUSELESS")),
        (
            "PkmnRaisedFirePowerWith",
            Key("STRINGID_PKMNRAISEDFIREPOWERWITH"),
        ),
        (
            "PkmnSXMadeYIneffective",
            Key("STRINGID_PKMNSXMADEYINEFFECTIVE"),
        ),
        ("PkmnChangedTypeWith", Key("STRINGID_PKMNCHANGEDTYPEWITH")),
        ("PkmnHurtsWith", Key("STRINGID_PKMNHURTSWITH")),
        (
            "PkmnSXCuredItsYProblem",
            Key("STRINGID_PKMNSXCUREDITSYPROBLEM"),
        ),
        ("PkmnTransformed", Key("STRINGID_PKMNTRANSFORMED")),
        ("PkmnPreventsUsage", Key("STRINGID_PKMNPREVENTSUSAGE")),
        ("AvoidedDamage", Key("STRINGID_AVOIDEDDAMAGE")),
        (
            "ItemRestoredStatus",
            Key("STRINGID_PKMNSITEMRESTOREDSTATUS"),
        ),
        ("ItemRestoredPp", Key("STRINGID_PKMNSITEMRESTOREDPP")),
        ("ForXCommaYZ", Key("STRINGID_FORXCOMMAYZ")),
        ("UsedXToGetPumped", Key("STRINGID_PKMNUSEDXTOGETPUMPED")),
        ("UsingItemStatRose", Key("STRINGID_USINGITEMSTATOFPKMNROSE")),
        ("FledUsingIts", Key("STRINGID_PKMNFLEDUSINGITS")),
        (
            "HmMovesCantBeForgotten",
            Key("STRINGID_HMMOVESCANTBEFORGOTTEN"),
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
