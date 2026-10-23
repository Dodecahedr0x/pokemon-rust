//! Unit tests of the build script's name classification (`build/classify.rs`).

#[allow(dead_code)]
#[path = "../build/classify/mod.rs"]
mod classify;

use classify::*;

#[test]
fn behaviors_by_name() {
    assert_eq!(behavior("MB_NORMAL"), "Normal");
    assert_eq!(behavior("MB_TALL_GRASS"), "TallGrass");
    assert_eq!(behavior("MB_JUMP_EAST"), "Ledge(Dir::Right)");
    assert_eq!(behavior("MB_JUMP_WEST"), "Ledge(Dir::Left)");
    assert_eq!(behavior("MB_JUMP_NORTH"), "Ledge(Dir::Up)");
    assert_eq!(behavior("MB_JUMP_SOUTH"), "Ledge(Dir::Down)");
    assert_eq!(behavior("MB_JUMP_NORTHEAST"), "Normal");
    assert_eq!(behavior("MB_ANIMATED_DOOR"), "Door");
    assert_eq!(behavior("MB_PETALBURG_GYM_DOOR"), "Normal");
    assert_eq!(behavior("MB_BRIDGE_OVER_OCEAN"), "Warp");
    assert_eq!(behavior("MB_DEEP_SOUTH_WARP"), "Warp");
    assert_eq!(behavior("MB_CRACKED_FLOOR_HOLE"), "Normal");
    assert_eq!(behavior("MB_BATTLE_PYRAMID_WARP"), "Normal");
    assert_eq!(behavior("MB_WATER_DOOR"), "Blocked");
    assert_eq!(behavior("MB_WATER_SOUTH_ARROW_WARP"), "Blocked");
    assert_eq!(behavior("MB_NON_ANIMATED_DOOR"), "Warp");
    assert_eq!(behavior("MB_LADDER"), "Warp");
    assert_eq!(behavior("MB_SOUTH_ARROW_WARP"), "ArrowWarp(Dir::Down)");
    assert_eq!(behavior("MB_EAST_ARROW_WARP"), "ArrowWarp(Dir::Right)");
    assert_eq!(behavior("MB_COUNTER"), "Counter");
    assert_eq!(behavior("MB_POND_WATER"), "Blocked");
    assert_eq!(behavior("MB_DEEP_WATER"), "Blocked");
    assert_eq!(behavior("MB_OCEAN_WATER"), "Blocked");
    assert_eq!(behavior("MB_SHALLOW_WATER"), "Normal");
    assert_eq!(
        behavior("MB_IMPASSABLE_SOUTH"),
        "Impassable(Sides::of(&[Dir::Down]))"
    );
    assert_eq!(
        behavior("MB_IMPASSABLE_NORTHEAST"),
        "Impassable(Sides::of(&[Dir::Up, Dir::Right]))"
    );
    assert_eq!(
        behavior("MB_SECRET_BASE_BREAKABLE_DOOR"),
        "Impassable(Sides::of(&[Dir::Left, Dir::Right]))"
    );
    assert_eq!(behavior("MB_SECRET_BASE_IMPASSABLE"), "Blocked");
    assert_eq!(behavior("MB_0xF5"), "Normal");
}

#[test]
fn every_named_behavior_is_known_to_the_decomp_table() {
    for name in classify::BLOCKED.iter().chain(classify::WARPS) {
        assert!(
            extract::names::METATILE_BEHAVIORS.value(name).is_some(),
            "{name}"
        );
    }
}

#[test]
fn behavior_table_uses_the_names_table() {
    let t = behavior_table();
    let jump_east = extract::names::METATILE_BEHAVIORS
        .value("MB_JUMP_EAST")
        .unwrap() as usize;
    assert_eq!(t[jump_east], "Ledge(Dir::Right)");
    let door = extract::names::METATILE_BEHAVIORS
        .value("MB_ANIMATED_DOOR")
        .unwrap() as usize;
    assert_eq!(t[door], "Door");
    assert_eq!(t[0xFF], "Normal");
}

fn movement(name: &str) -> (&'static str, bool) {
    let (s, exact) = classify::movement(name);
    (Box::leak(s.into_boxed_str()), exact)
}

#[test]
fn movement_types() {
    assert_eq!(
        movement("MOVEMENT_TYPE_FACE_UP"),
        ("MovementType::Face(Dir::Up)", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_LOOK_AROUND"),
        ("MovementType::LookAround", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WANDER_AROUND"),
        ("MovementType::Wander", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WANDER_DOWN_AND_UP"),
        ("MovementType::WanderVertical", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WANDER_RIGHT_AND_LEFT"),
        ("MovementType::WanderHorizontal", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WALK_IN_PLACE_LEFT"),
        ("MovementType::WalkInPlace(Dir::Left)", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_JOG_IN_PLACE_RIGHT"),
        ("MovementType::JogInPlace(Dir::Right)", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_INVISIBLE"),
        ("MovementType::Invisible", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_FACE_DOWN_AND_RIGHT"),
        (
            "MovementType::FaceTwo(Dirs::two(Dir::Down, Dir::Right))",
            true
        )
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WALK_UP_AND_DOWN"),
        ("MovementType::WalkBackAndForth(Dir::Up)", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_WALK_LEFT_AND_RIGHT"),
        ("MovementType::WalkBackAndForth(Dir::Left)", true)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_BERRY_TREE_GROWTH"),
        ("MovementType::Face(Dir::Down)", false)
    );
    assert_eq!(
        movement("MOVEMENT_TYPE_NONE"),
        ("MovementType::Face(Dir::Down)", false)
    );
}

#[test]
fn every_movement_type_maps() {
    for (_, name) in extract::names::MOVEMENT_TYPES.iter() {
        assert!(
            classify::movement(name).0.starts_with("MovementType::"),
            "{name}"
        );
    }
}

#[test]
fn gfx_names() {
    let base = extract::names::VARS.value("VAR_OBJ_GFX_ID_0").unwrap();
    assert_eq!(
        gfx("OBJ_EVENT_GFX_VAR_0"),
        Some(format!("Gfx::Var(VarId({base:#x}))"))
    );
    assert_eq!(
        gfx("OBJ_EVENT_GFX_VAR_B"),
        Some(format!("Gfx::Var(VarId({:#x}))", base + 11))
    );
    assert_eq!(
        gfx("OBJ_EVENT_GFX_BRENDAN_NORMAL"),
        Some("Gfx::Fixed(GfxId(0x0))".into())
    );
    assert_eq!(gfx("OBJ_EVENT_GFX_NOPE"), None);
}

#[test]
fn hex_fallback_names_are_found() {
    let src = "setflag(FLAG_0x1AB); VAR_0x4001 x MAP_ROUTE101 FLAG_0x; aFLAG_0x12";
    let found: Vec<_> = hex_names(src).into_iter().collect();
    assert_eq!(found, ["FLAG_0x1AB", "VAR_0x4001"]);
}

#[test]
fn idents_with_a_prefix() {
    let src = "trainer_battle(TRAINER_MAY_ROUTE_103_MUDKIP, x); XTRAINER_NO TRAINER_RICK)";
    let found: Vec<_> = idents_with_prefix(src, "TRAINER_").into_iter().collect();
    assert_eq!(found, ["TRAINER_MAY_ROUTE_103_MUDKIP", "TRAINER_RICK"]);
    let src = "f(TRAINER_A); // TRAINER_B\n// TRAINER_C\ng(TRAINER_D)";
    let found: Vec<_> = idents_with_prefix(&without_line_comments(src), "TRAINER_")
        .into_iter()
        .collect();
    assert_eq!(found, ["TRAINER_A", "TRAINER_D"]);
}

#[test]
fn move_effects_by_name() {
    let e = |n| move_effect(n);
    assert_eq!(e("EFFECT_HIT").as_deref(), Some("MoveEffect::Hit"));
    assert_eq!(
        e("EFFECT_ATTACK_DOWN").as_deref(),
        Some("MoveEffect::StatDown { stat: Stat::Atk, stages: 1 }")
    );
    assert_eq!(
        e("EFFECT_SPECIAL_DEFENSE_UP_2").as_deref(),
        Some("MoveEffect::StatUp { stat: Stat::SpDef, stages: 2 }")
    );
    assert_eq!(
        e("EFFECT_ACCURACY_DOWN_HIT").as_deref(),
        Some("MoveEffect::StatDownHit(Stat::Accuracy)")
    );
    assert_eq!(
        e("EFFECT_TOXIC").as_deref(),
        Some("MoveEffect::Status(StatusKind::Toxic)")
    );
    assert_eq!(e("EFFECT_RECOIL").as_deref(), Some("MoveEffect::Recoil"));
    assert_eq!(
        e("EFFECT_MULTI_HIT").as_deref(),
        Some("MoveEffect::MultiHit")
    );
    assert_eq!(e("EFFECT_ATTRACT").as_deref(), Some("MoveEffect::Attract"));
    assert_eq!(e("EFFECT_LOW_KICK").as_deref(), Some("MoveEffect::LowKick"));
    assert_eq!(e("EFFECT_ROLLOUT"), None);
    assert_eq!(e("HIT"), None);
    // The mapped names are real `EFFECT_*`s.
    let mapped = extract::names::MOVE_EFFECTS
        .iter()
        .filter(|(_, n)| move_effect(n).is_some())
        .count();
    assert!(mapped >= 30, "{mapped}");
}

#[test]
fn battle_enums_by_name() {
    assert_eq!(type_variant("TYPE_DARK"), Some("Type::Dark"));
    // Silice content's Fairy (Emerald has no `TYPE_FAIRY`).
    assert_eq!(type_variant("TYPE_FAIRY"), Some("Type::Fairy"));
    for (_, n) in extract::names::GROWTH_RATES.iter() {
        assert!(growth(n).is_some(), "{n}");
    }
    for (_, n) in extract::names::MOVE_TARGETS.iter() {
        assert!(move_target(n).is_some(), "{n}");
    }
    for (v, n) in extract::names::MOVE_FLAGS.iter() {
        assert_eq!(move_flag(n).map(u32::from), Some(v), "{n}");
        assert_ne!(u32::from(SOUND_FLAG), v, "{n}");
    }
    assert_eq!(ability("ABILITY_TORRENT"), "Ability::Torrent");
    assert_eq!(ability("ABILITY_PICKUP"), "Ability::Pickup");
    assert_eq!(ability("ABILITY_CLEAR_BODY"), "Ability::ClearBody");
    assert_eq!(ability("ABILITY_WHITE_SMOKE"), "Ability::Other");
    // The Gen3 abilities the Silice engine adds stay off for Emerald's species; the post-Gen3
    // ones (no Emerald species) are mapped.
    assert_eq!(ability("ABILITY_DRIZZLE"), "Ability::Other");
    assert_eq!(ability("ABILITY_LEVITATE"), "Ability::Other");
    assert_eq!(ability("ABILITY_PIXILATE"), "Ability::Pixilate");
    assert_eq!(ability("ABILITY_PRIMORDIAL_SEA"), "Ability::PrimordialSea");
}

#[test]
fn extra_flags_mark_the_sound_moves() {
    assert_eq!(extra_move_flags("MOVE_GROWL"), SOUND_FLAG);
    assert_eq!(extra_move_flags("MOVE_HYPER_VOICE"), SOUND_FLAG);
    assert_eq!(extra_move_flags("MOVE_TACKLE"), 0);
    assert_eq!(extra_move_flags("GROWL"), 0);
}

/// `SOUND_MOVES` is `sSoundMovesTable` (battle_util.c), in order (skipped without
/// `PKR_DECOMP`).
#[test]
fn sound_moves_match_the_decomp() {
    let Ok(dir) = std::env::var("PKR_DECOMP") else {
        return;
    };
    let src =
        std::fs::read_to_string(std::path::Path::new(&dir).join("src/battle_util.c")).unwrap();
    let start = src.find("sSoundMovesTable[] =").expect("sSoundMovesTable");
    let body = &src[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find('}').unwrap()];
    let table: Vec<&str> = body
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty() && *n != "SOUND_MOVES_END")
        .collect();
    assert_eq!(table, SOUND_MOVES);
}

#[test]
fn every_str_source_names_an_exported_string() {
    let ids =
        |k: &str| !k.starts_with("STRINGID_") || extract::names::STRING_IDS.value(k).is_some();
    let mut patterns = std::collections::BTreeSet::new();
    for (pat, src) in STRS {
        assert!(patterns.insert(*pat), "{pat} listed twice");
        match *src {
            StrSrc::Key(k) => assert!(ids(k), "{pat}: {k}"),
            StrSrc::Sharply { adverb, verb } => assert!(ids(adverb) && ids(verb), "{pat}"),
            StrSrc::Buff1 { key, code } => {
                assert!(ids(key), "{pat}: {key}");
                assert!(code.starts_with("{B_"), "{pat}");
            }
            StrSrc::UsedMove
            | StrSrc::TypeNames
            | StrSrc::StatNames
            | StrSrc::AbilityNames
            | StrSrc::Missing => {}
        }
    }
}
