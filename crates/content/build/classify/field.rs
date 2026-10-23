//! Field classification (owned by Milestone 8 Task E): behaviors, movement types, object
//! graphics, warp behaviors.

use extract::names;

/// Behaviors that are never walkable for now: water that needs Surf.
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
    "MB_SECRET_BASE_IMPASSABLE",
];

/// Sides a behavior walls off (Gen3 `MetatileBehavior_Is{North,South,West,East}Blocked`), as
/// `Dir::` names.
fn walls(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "MB_IMPASSABLE_NORTH" => &["Dir::Up"],
        "MB_IMPASSABLE_SOUTH" => &["Dir::Down"],
        "MB_IMPASSABLE_WEST" => &["Dir::Left"],
        "MB_IMPASSABLE_EAST" => &["Dir::Right"],
        "MB_IMPASSABLE_NORTHEAST" => &["Dir::Up", "Dir::Right"],
        "MB_IMPASSABLE_NORTHWEST" => &["Dir::Up", "Dir::Left"],
        "MB_IMPASSABLE_SOUTHEAST" => &["Dir::Down", "Dir::Right"],
        "MB_IMPASSABLE_SOUTHWEST" => &["Dir::Down", "Dir::Left"],
        "MB_IMPASSABLE_SOUTH_AND_NORTH" => &["Dir::Down", "Dir::Up"],
        "MB_IMPASSABLE_WEST_AND_EAST" | "MB_SECRET_BASE_BREAKABLE_DOOR" => {
            &["Dir::Left", "Dir::Right"]
        }
        _ => return None,
    })
}

/// Warp cells entered by stepping on them, as Gen3 `IsWarpMetatileBehavior`
/// (`field_control_avatar.c`) minus `MetatileBehavior_IsWarpDoor` (`MB_ANIMATED_DOOR`, our
/// `Door`) and `MB_WATER_DOOR` (surfable, `Blocked`): ladders, escalators, non-animated doors
/// (stairs), `MB_DEEP_SOUTH_WARP` (both in `IsNonAnimDoor`), the gym and hideout warps, Mt.
/// Pyre holes, and `MB_BRIDGE_OVER_OCEAN`, which Gen3 reuses as the Union Room exit
/// (`MetatileBehavior_IsUnionRoomWarp`; there is no `MB_UNION_ROOM_WARP`). [`behavior`] tells
/// them apart by their arrival (`NonAnimDoor`, `Ladder`, `Escalator`, else `Warp`).
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
        // The arrival per kind (Gen3 `GetAdjustedInitialDirection`, `SetUpWarpExitTask`).
        return match name {
            "MB_NON_ANIMATED_DOOR" => "NonAnimDoor(Dir::Down)",
            "MB_DEEP_SOUTH_WARP" => "NonAnimDoor(Dir::Up)",
            "MB_LADDER" => "Ladder",
            "MB_UP_ESCALATOR" | "MB_DOWN_ESCALATOR" => "Escalator(Dir::Right)",
            _ => "Warp",
        }
        .into();
    }
    if let Some(sides) = walls(name) {
        return format!("Impassable(Sides::of(&[{}]))", sides.join(", "));
    }
    if let Some(d) = name.strip_prefix("MB_JUMP_").and_then(compass) {
        return format!("Ledge({d})");
    }
    // Forced movement (Gen3 `sForcedMovementTestFuncs`).
    if let Some(d) = name.strip_prefix("MB_WALK_").and_then(compass) {
        return format!("ForcedWalk({d})");
    }
    if let Some(d) = name.strip_prefix("MB_SLIDE_").and_then(compass) {
        return format!("Slide({d})");
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
        // Gen3 `MetatileBehavior_Is{North,South}ArrowWarp` also take these.
        "MB_STAIRS_OUTSIDE_ABANDONED_SHIP" => "ArrowWarp(Dir::Up)",
        "MB_SHOAL_CAVE_ENTRANCE" => "ArrowWarp(Dir::Down)",
        "MB_COUNTER" => "Counter",
        // Both `ForcedMovement_Slip`.
        "MB_ICE" | "MB_TRICK_HOUSE_PUZZLE_8_FLOOR" => "Ice",
        "MB_MUDDY_SLOPE" => "MuddySlope",
        // Read by the map's per-step callback (`GameData::step_callback`) and the hole trigger.
        "MB_THIN_ICE" => "ThinIce",
        "MB_CRACKED_ICE" => "CrackedIce",
        "MB_CRACKED_FLOOR" => "CrackedFloor",
        "MB_CRACKED_FLOOR_HOLE" => "CrackedFloorHole",
        "MB_TALL_GRASS" => "TallGrass",
        _ => "Normal",
    }
    .into()
}

/// [`behavior`], and whether it is exact. Exact: plain ground (`MB_NORMAL`, unnamed bytes),
/// tall grass, ledges, doors, warps, arrow warps, counters, ice, walk and slide arrows, which
/// the engine implements as Gen3 does. Every other named behavior is an approximation: its
/// Gen3 effect (Surf, currents, sand, bikes, puddles, hot springs, secret bases, …) is missing,
/// so it is `Normal` or, for water, `Blocked`. The directional walls are exact, except the
/// secret base's breakable door (walls only; breaking it is missing). The muddy slope is
/// approximate (no Mach Bike climb, no metatile animation), and so are thin and cracked ice,
/// cracked floors and their holes: the engine implements them, but they act only on maps whose
/// per-step callback, hole warp and fall script content answers. The build lists the
/// approximations in `content::REPORT`.
pub fn behavior_class(name: &str) -> (String, bool) {
    let v = behavior(name);
    // Gen3 never uses the `MB_UNUSED_*` bytes: whatever we map them to is as good as exact.
    let exact = name.starts_with("MB_UNUSED")
        || match v.as_str() {
            "Blocked" => false,
            "Normal" => name == "MB_NORMAL" || name.starts_with("MB_0x"),
            _ if name == "MB_SECRET_BASE_BREAKABLE_DOOR" => false,
            "MuddySlope" | "ThinIce" | "CrackedIce" | "CrackedFloor" | "CrackedFloorHole" => false,
            _ => true,
        };
    (v, exact)
}

/// `Behavior` variant of every raw behavior byte, through `names::behavior_name`.
pub fn behavior_table() -> Vec<String> {
    (0..=255u8)
        .map(|raw| behavior(&names::behavior_name(raw)))
        .collect()
}

/// `MovementType` expression for a `MOVEMENT_TYPE_*` name, and whether the mapping is exact
/// (false: closest equivalent). Direction lists keep the name's order, which is Gen3's array
/// order (`movement_type_func_tables.h`, the initial facing first).
///
/// Approximated (they face down): `NONE`, `PLAYER`, `BERRY_TREE_GROWTH`, the `COPY_PLAYER*`
/// types, the tree and mountain disguises, `BURIED`, and unnamed bytes.
pub fn movement(name: &str) -> (String, bool) {
    let t = name.strip_prefix("MOVEMENT_TYPE_").unwrap_or(name);
    let exact = |s: &str| (format!("MovementType::{s}"), true);
    let approx = |s: &str| (format!("MovementType::{s}"), false);
    // `UP`, `DOWN_AND_UP`, `DOWN_UP_AND_LEFT`, `UP_RIGHT_LEFT_DOWN`: the directions in order.
    let dirs = |s: &str| -> Option<Vec<&'static str>> {
        s.split('_').filter(|w| *w != "AND").map(compass).collect()
    };
    for (p, v) in [
        ("WALK_IN_PLACE_", "WalkInPlace"),
        ("JOG_IN_PLACE_", "JogInPlace"),
        ("RUN_IN_PLACE_", "RunInPlace"),
        ("WALK_SLOWLY_IN_PLACE_", "WalkSlowlyInPlace"),
    ] {
        if let Some(d) = t.strip_prefix(p).and_then(compass) {
            return exact(&format!("{v}({d})"));
        }
    }
    if let Some(ds) = t.strip_prefix("WALK_SEQUENCE_").and_then(dirs)
        && let [a, b, c, d] = ds[..]
    {
        return exact(&format!("WalkSequence(Dirs::new([{a}, {b}, {c}, {d}]))"));
    }
    // `WALK_UP_AND_DOWN` …: back and forth, starting towards the first.
    if let Some(ds) = t.strip_prefix("WALK_").and_then(dirs)
        && let [a, _] = ds[..]
    {
        return exact(&format!("WalkBackAndForth({a})"));
    }
    if let Some(ds) = t.strip_prefix("FACE_").and_then(dirs) {
        match ds[..] {
            [d] => return exact(&format!("Face({d})")),
            [a, b] => return exact(&format!("FaceTwo(Dirs::two({a}, {b}))")),
            [_, _, _] => {
                let all = ["Dir::Down", "Dir::Up", "Dir::Left", "Dir::Right"];
                if let Some(m) = all.into_iter().find(|d| !ds.contains(d)) {
                    return exact(&format!("FaceAllBut({m})"));
                }
            }
            _ => {}
        }
    }
    match t {
        "LOOK_AROUND" => exact("LookAround"),
        "WANDER_AROUND" => exact("Wander"),
        "WANDER_UP_AND_DOWN" | "WANDER_DOWN_AND_UP" => exact("WanderVertical"),
        "WANDER_LEFT_AND_RIGHT" | "WANDER_RIGHT_AND_LEFT" => exact("WanderHorizontal"),
        "INVISIBLE" => exact("Invisible"),
        "ROTATE_CLOCKWISE" => exact("Rotate { clockwise: true }"),
        "ROTATE_COUNTERCLOCKWISE" => exact("Rotate { clockwise: false }"),
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
