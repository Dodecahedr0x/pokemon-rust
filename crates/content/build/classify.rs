//! Gen3 constant names -> engine values, as Rust source snippets. Shared by the build script and
//! `tests/build_classify.rs`.

use extract::names;
use std::collections::BTreeSet;

/// Behaviors that are never walkable for now: water that needs Surf, and the directional
/// `MB_IMPASSABLE_*` (the engine has no directional blocking yet).
pub const BLOCKED: &[&str] = &[
    "MB_POND_WATER",
    "MB_INTERIOR_DEEP_WATER",
    "MB_DEEP_WATER",
    "MB_WATERFALL",
    "MB_SOOTOPOLIS_DEEP_WATER",
    "MB_OCEAN_WATER",
    "MB_NO_SURFACING",
    "MB_SEAWEED",
    "MB_SEAWEED_NO_SURFACING",
    "MB_EASTWARD_CURRENT",
    "MB_WESTWARD_CURRENT",
    "MB_NORTHWARD_CURRENT",
    "MB_SOUTHWARD_CURRENT",
    "MB_UNUSED_6F",
    // Surfable warps (Gen3 `TILE_FLAG_SURFABLE`): unreachable without Surf.
    "MB_WATER_DOOR",
    "MB_WATER_SOUTH_ARROW_WARP",
    "MB_IMPASSABLE_EAST",
    "MB_IMPASSABLE_WEST",
    "MB_IMPASSABLE_NORTH",
    "MB_IMPASSABLE_SOUTH",
    "MB_IMPASSABLE_NORTHEAST",
    "MB_IMPASSABLE_NORTHWEST",
    "MB_IMPASSABLE_SOUTHEAST",
    "MB_IMPASSABLE_SOUTHWEST",
    "MB_IMPASSABLE_SOUTH_AND_NORTH",
    "MB_IMPASSABLE_WEST_AND_EAST",
    "MB_SECRET_BASE_IMPASSABLE",
];

/// Warp cells entered by stepping on them, as Gen3 `IsWarpMetatileBehavior`
/// (`field_control_avatar.c`) minus `MetatileBehavior_IsWarpDoor` (`MB_ANIMATED_DOOR`, our
/// `Door`) and `MB_WATER_DOOR` (surfable, `Blocked`): ladders, escalators, non-animated doors
/// (stairs), `MB_DEEP_SOUTH_WARP` (both in `IsNonAnimDoor`), the gym and hideout warps, Mt.
/// Pyre holes, and `MB_BRIDGE_OVER_OCEAN`, which Gen3 reuses as the Union Room exit
/// (`MetatileBehavior_IsUnionRoomWarp`; there is no `MB_UNION_ROOM_WARP`).
pub const WARPS: &[&str] = &[
    "MB_NON_ANIMATED_DOOR",
    "MB_DEEP_SOUTH_WARP",
    "MB_LADDER",
    "MB_UP_ESCALATOR",
    "MB_DOWN_ESCALATOR",
    "MB_LAVARIDGE_GYM_B1F_WARP",
    "MB_LAVARIDGE_GYM_1F_WARP",
    "MB_AQUA_HIDEOUT_WARP",
    "MB_MT_PYRE_HOLE",
    "MB_MOSSDEEP_GYM_WARP",
    "MB_BRIDGE_OVER_OCEAN",
];

fn compass(s: &str) -> Option<&'static str> {
    Some(match s {
        "NORTH" | "UP" => "Dir::Up",
        "SOUTH" | "DOWN" => "Dir::Down",
        "WEST" | "LEFT" => "Dir::Left",
        "EAST" | "RIGHT" => "Dir::Right",
        _ => return None,
    })
}

/// `engine::data::Behavior` variant (without the `Behavior::` prefix) for an `MB_*` name.
pub fn behavior(name: &str) -> String {
    if BLOCKED.contains(&name) {
        return "Blocked".into();
    }
    if WARPS.contains(&name) {
        return "Warp".into();
    }
    if let Some(d) = name.strip_prefix("MB_JUMP_").and_then(compass) {
        return format!("Ledge({d})");
    }
    if let Some(d) = name
        .strip_suffix("_ARROW_WARP")
        .and_then(|n| n.strip_prefix("MB_"))
        .and_then(compass)
    {
        return format!("ArrowWarp({d})");
    }
    match name {
        // Gen3 `MetatileBehavior_IsWarpDoor`: the only door `TryDoorWarp` enters.
        // (`MB_PETALBURG_GYM_DOOR` is only in `IsDoor`, for the door animation of the gym's
        // scripted room changes; it starts no warp, so it stays `Normal`.)
        "MB_ANIMATED_DOOR" => "Door",
        "MB_COUNTER" => "Counter",
        "MB_TALL_GRASS" => "TallGrass",
        _ => "Normal",
    }
    .into()
}

/// `Behavior` variant of every raw behavior byte, through `names::behavior_name`.
pub fn behavior_table() -> Vec<String> {
    (0..=255u8)
        .map(|raw| behavior(&names::behavior_name(raw)))
        .collect()
}

/// `MovementType` expression for a `MOVEMENT_TYPE_*` name, and whether the mapping is exact
/// (false: closest equivalent).
pub fn movement(name: &str) -> (String, bool) {
    let t = name.strip_prefix("MOVEMENT_TYPE_").unwrap_or(name);
    let exact = |s: &str| (format!("MovementType::{s}"), true);
    let approx = |s: &str| (format!("MovementType::{s}"), false);
    if let Some(d) = t.strip_prefix("FACE_").and_then(compass) {
        return exact(&format!("Face({d})"));
    }
    if let Some(d) = t.strip_prefix("WALK_IN_PLACE_").and_then(compass) {
        return exact(&format!("WalkInPlace({d})"));
    }
    for p in ["JOG_IN_PLACE_", "RUN_IN_PLACE_", "WALK_SLOWLY_IN_PLACE_"] {
        if let Some(d) = t.strip_prefix(p).and_then(compass) {
            return approx(&format!("WalkInPlace({d})"));
        }
    }
    match t {
        "LOOK_AROUND" => exact("LookAround"),
        "WANDER_AROUND" => exact("Wander"),
        "WANDER_UP_AND_DOWN" | "WANDER_DOWN_AND_UP" => exact("WanderVertical"),
        "WANDER_LEFT_AND_RIGHT" | "WANDER_RIGHT_AND_LEFT" => exact("WanderHorizontal"),
        "INVISIBLE" => exact("Invisible"),
        "WALK_UP_AND_DOWN" | "WALK_DOWN_AND_UP" => approx("WanderVertical"),
        "WALK_LEFT_AND_RIGHT" | "WALK_RIGHT_AND_LEFT" => approx("WanderHorizontal"),
        _ if t.starts_with("WALK_SEQUENCE_") => approx("Wander"),
        // Two- and three-direction facings, rotations: turn in place.
        _ if t.starts_with("FACE_") || t.starts_with("ROTATE_") => approx("LookAround"),
        // NONE, PLAYER, BERRY_TREE_GROWTH, COPY_PLAYER*, disguises, BURIED, unnamed.
        _ => approx("Face(Dir::Down)"),
    }
}

/// `Gfx` expression for an `OBJ_EVENT_GFX_*` name; `OBJ_EVENT_GFX_VAR_<n>` reads
/// `VAR_OBJ_GFX_ID_<n>` as Gen3 does.
pub fn gfx(name: &str) -> Option<String> {
    if let Some(n) = name.strip_prefix("OBJ_EVENT_GFX_VAR_") {
        let n = u32::from_str_radix(n, 16).ok()?;
        let base = names::VARS.value("VAR_OBJ_GFX_ID_0")?;
        return Some(format!("Gfx::Var(VarId({:#x}))", base + n));
    }
    let id = names::OBJ_EVENT_GFX.value(name)?;
    Some(format!("Gfx::Fixed(GfxId({id:#x}))"))
}

/// Prefixes of constants whose unnamed numbers are written `<PREFIX>0x<HEX>`.
pub const HEX_PREFIXES: &[&str] = &[
    "FLAG_",
    "VAR_",
    "SPECIAL_",
    "ITEM_",
    "OBJ_EVENT_GFX_",
    "STD_",
];

/// Every `<PREFIX>0x<HEX>` identifier (`FLAG_0x1AB`) in `src`.
pub fn hex_names(src: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = BTreeSet::new();
    for p in HEX_PREFIXES {
        let pat = format!("{p}0x");
        let mut from = 0;
        while let Some(i) = src[from..].find(&pat).map(|i| i + from) {
            from = i + pat.len();
            if i > 0 && ident(b[i - 1]) {
                continue;
            }
            let end = from + b[from..].iter().take_while(|c| ident(**c)).count();
            if end > from && b[from..end].iter().all(u8::is_ascii_hexdigit) {
                out.insert(src[i..end].to_string());
            }
        }
    }
    out
}

/// Every whole identifier of `src` that starts with `prefix` (`TRAINER_MAY_ROUTE_103_MUDKIP`).
pub fn idents_with_prefix(src: &str, prefix: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = BTreeSet::new();
    let mut from = 0;
    while let Some(i) = src[from..].find(prefix).map(|i| i + from) {
        let end = i + b[i..].iter().take_while(|c| ident(**c)).count();
        from = end.max(i + 1);
        if i > 0 && ident(b[i - 1]) {
            continue;
        }
        out.insert(src[i..end].to_string());
    }
    out
}

/// `src` without its `//` line comments (generated scripts have no string literal holding
/// `//`), so names quoted in comments are not taken for references.
pub fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| l.split_once("//").map_or(l, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- battle (Milestone 4) ----

/// `TYPE_*` name -> `engine::mon::Type` expression.
pub fn type_variant(name: &str) -> Option<&'static str> {
    Some(match name {
        "TYPE_NORMAL" => "Type::Normal",
        "TYPE_FIGHTING" => "Type::Fighting",
        "TYPE_FLYING" => "Type::Flying",
        "TYPE_POISON" => "Type::Poison",
        "TYPE_GROUND" => "Type::Ground",
        "TYPE_ROCK" => "Type::Rock",
        "TYPE_BUG" => "Type::Bug",
        "TYPE_GHOST" => "Type::Ghost",
        "TYPE_STEEL" => "Type::Steel",
        "TYPE_MYSTERY" => "Type::Mystery",
        "TYPE_FIRE" => "Type::Fire",
        "TYPE_WATER" => "Type::Water",
        "TYPE_GRASS" => "Type::Grass",
        "TYPE_ELECTRIC" => "Type::Electric",
        "TYPE_PSYCHIC" => "Type::Psychic",
        "TYPE_ICE" => "Type::Ice",
        "TYPE_DRAGON" => "Type::Dragon",
        "TYPE_DARK" => "Type::Dark",
        _ => return None,
    })
}

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

/// `ABILITY_*` name -> `Ability` expression; the abilities the engine does not implement are
/// `Ability::Other`.
pub fn ability(name: &str) -> &'static str {
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
        _ => "Ability::Other",
    }
}

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
    /// `gAbilityNames`, by `Ability`. Not exported by `extract` yet: every ability name is
    /// the empty string until it is (follow-up), so `{B_*_ABILITY}` expands to nothing.
    AbilityNames,
}

/// Every `battle::Str` variant (as a `match` pattern) and its source. The build writes one
/// exhaustive `match` from this list, so a new engine variant fails the build until it is
/// added here.
pub const STRS: &[(&str, StrSrc)] = {
    use StrSrc::*;
    &[
        ("Intro1", Key("sText_WildPkmnAppeared")),
        ("IntroTrainer", Key("sText_Trainer1WantsToBattle")),
        ("IntroMsg", Key("sText_WildPkmnAppeared")),
        ("Intro2", Key("sText_GoPkmn")),
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
        ("AbilityName(_)", AbilityNames),
    ]
};
