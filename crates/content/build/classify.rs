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

/// Non-animated warp cells entered by stepping on them (Gen3 `TryStartWarpEventScript`):
/// stairs (`MB_NON_ANIMATED_DOOR`), ladders, escalators, holes, puzzle warps.
pub const WARPS: &[&str] = &[
    "MB_NON_ANIMATED_DOOR",
    "MB_LADDER",
    "MB_UP_ESCALATOR",
    "MB_DOWN_ESCALATOR",
    "MB_WATER_DOOR",
    "MB_DEEP_SOUTH_WARP",
    "MB_LAVARIDGE_GYM_1F_WARP",
    "MB_LAVARIDGE_GYM_B1F_WARP",
    "MB_AQUA_HIDEOUT_WARP",
    "MB_MT_PYRE_HOLE",
    "MB_MOSSDEEP_GYM_WARP",
    "MB_BATTLE_PYRAMID_WARP",
    "MB_CRACKED_FLOOR_HOLE",
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
        .and_then(|n| n.strip_prefix("MB_WATER_").or(n.strip_prefix("MB_")))
        .and_then(compass)
    {
        return format!("ArrowWarp({d})");
    }
    match name {
        "MB_ANIMATED_DOOR" | "MB_PETALBURG_GYM_DOOR" => "Door",
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
