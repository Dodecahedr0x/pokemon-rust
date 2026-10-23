//! Milestone 8 Task E: every behavior byte is classified, exactly or as a listed approximation.

#[allow(dead_code, unused_imports)]
#[path = "../build/classify/mod.rs"]
mod classify;

use classify::*;

#[test]
fn every_behavior_is_exact_or_an_approximation() {
    let mut approx = 0;
    for raw in 0..=255u8 {
        let name = extract::names::behavior_name(raw);
        let (variant, exact) = behavior_class(&name);
        assert_eq!(variant, behavior(&name), "{name}");
        if !exact {
            approx += 1;
        }
    }
    // Some named behaviors have Gen3 effects the engine lacks (ice, sand, bikes, Surf, …).
    assert!(approx > 0);
}

#[test]
fn behavior_exactness() {
    for (name, exact) in [
        ("MB_NORMAL", true),
        ("MB_TALL_GRASS", true),
        ("MB_JUMP_EAST", true),
        ("MB_ANIMATED_DOOR", true),
        ("MB_LADDER", true),
        ("MB_SOUTH_ARROW_WARP", true),
        ("MB_COUNTER", true),
        ("MB_0xF5", true),
        // Never used by Gen3, even when mapped to `Blocked`.
        ("MB_UNUSED_04", true),
        ("MB_UNUSED_6F", true),
        // Surfable water (Wave C); Waterfall, Dive and the currents' speed are missing.
        ("MB_POND_WATER", true),
        ("MB_OCEAN_WATER", true),
        ("MB_WATERFALL", false),
        ("MB_NO_SURFACING", false),
        ("MB_EASTWARD_CURRENT", false),
        ("MB_WATER_DOOR", false),
        // Directional walls (Task L).
        ("MB_IMPASSABLE_SOUTH", true),
        ("MB_IMPASSABLE_WEST_AND_EAST", true),
        // Walls only: breaking the door is missing.
        ("MB_SECRET_BASE_BREAKABLE_DOOR", false),
        // Forced movement (M9, forced-movement agent): exact on foot.
        ("MB_ICE", true),
        ("MB_WALK_EAST", true),
        ("MB_SLIDE_NORTH", true),
        // The Mach Bike climb and the per-step puzzles' Emerald hooks are missing.
        ("MB_MUDDY_SLOPE", false),
        ("MB_THIN_ICE", false),
        ("MB_CRACKED_FLOOR", false),
        // Encounter floors and the battle background's grounds (off-grass encounters); the
        // ash pickup is missing.
        ("MB_LONG_GRASS", true),
        ("MB_CAVE", true),
        ("MB_DEEP_SAND", true),
        ("MB_SAND", true),
        ("MB_MOUNTAIN_TOP", true),
        ("MB_ASHGRASS", false),
    ] {
        assert_eq!(behavior_class(name).1, exact, "{name}");
    }
}
