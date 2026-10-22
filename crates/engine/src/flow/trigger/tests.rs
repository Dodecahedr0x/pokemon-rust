use super::super::test_data::*;
use super::*;
use crate::data::VarId;

fn vars(set: &[(u16, u16)]) -> Vars {
    let mut v = Vars::new();
    for &(id, x) in set {
        v.set_var(VarId(id), x);
    }
    v
}

#[test]
fn behavior_from_primary_secondary_and_unknown_metatiles() {
    let m = town();
    assert_eq!(behavior_at(&Game, m, 0, 0), Behavior::Normal);
    assert_eq!(
        behavior_at(&Game, m, 1, 0),
        Behavior::ArrowWarp(Dir::Up),
        "layer bits ignored"
    );
    assert_eq!(behavior_at(&Game, m, 2, 1), Behavior::Door);
    assert_eq!(
        behavior_at(&Game, m, 4, 4),
        Behavior::Warp,
        "elevation bits ignored"
    );
    assert_eq!(
        behavior_at(&Game, m, 5, 5),
        Behavior::Door,
        "metatile 512 is secondary[0]"
    );
    assert_eq!(behavior_at(&Game, m, 1, 3), Behavior::Counter);
}

#[test]
fn block_behavior_classifies_live_blocks() {
    let m = town();
    assert_eq!(block_behavior(&Game, m, DOOR | E3), Behavior::Door);
    assert_eq!(block_behavior(&Game, m, 512), Behavior::Door);
    assert_eq!(block_behavior(&Game, m, COUNTER), Behavior::Counter);
    assert_eq!(
        block_behavior(&Game, m, 0x3FF),
        Behavior::Normal,
        "unknown is MB_INVALID"
    );
}

#[test]
fn facing_elevation_is_zero_when_standing_on_elevation_zero() {
    let m = town();
    let b = |x, y| block_at(&Game, m, x, y);
    assert_eq!(facing_elevation(b(0, 0), 3), 0);
    assert_eq!(facing_elevation(b(3, 2), 3), 3);
    assert_eq!(facing_elevation(b(4, 4), 4), 4);
}

#[test]
fn elevation_rules() {
    assert!(elevation_matches(0, 5) && elevation_matches(5, 0) && elevation_matches(3, 3));
    assert!(!elevation_matches(3, 4));
    assert!(event_matches(0, 7) && event_matches(4, 4));
    assert!(
        !event_matches(4, 0),
        "an elevated event needs that elevation"
    );
}

#[test]
fn coord_trigger_matches_var_value() {
    let m = town();
    assert_eq!(coord_trigger(m, 3, 3, 0, &vars(&[])), Some(ScriptId(10)));
    assert_eq!(
        coord_trigger(m, 3, 3, 0, &vars(&[(0x4001, 1)])),
        Some(ScriptId(11)),
        "first matching entry on the cell"
    );
    assert_eq!(coord_trigger(m, 3, 3, 0, &vars(&[(0x4001, 2)])), None);
    assert_eq!(coord_trigger(m, 3, 4, 0, &vars(&[])), None);
}

#[test]
fn coord_trigger_elevation_rule() {
    let m = town();
    assert_eq!(
        coord_trigger(m, 3, 3, 7, &vars(&[])),
        Some(ScriptId(10)),
        "elevation 0 event matches any"
    );
    assert_eq!(coord_trigger(m, 2, 2, 4, &vars(&[])), Some(ScriptId(12)));
    assert_eq!(coord_trigger(m, 2, 2, 3, &vars(&[])), None);
    assert_eq!(coord_trigger(m, 2, 2, 0, &vars(&[])), None);
}

#[test]
fn coord_trigger_value_is_compared_as_u8_and_var_zero_never_matches() {
    let m = town();
    assert_eq!(
        coord_trigger(m, 0, 0, 0, &vars(&[(0x4003, 1)])),
        Some(ScriptId(13))
    );
    assert_eq!(coord_trigger(m, 0, 0, 0, &vars(&[(0x4003, 0x101)])), None);
    assert_eq!(coord_trigger(m, 5, 0, 0, &vars(&[])), None);
}

#[test]
fn warp_on_enter_needs_door_or_warp_behavior_and_a_warp() {
    let m = town();
    assert_eq!(warp_on_enter(m, 2, 1, 0, Behavior::Door), Some(0));
    assert_eq!(warp_on_enter(m, 2, 1, 0, Behavior::Warp), Some(0));
    assert_eq!(warp_on_enter(m, 2, 1, 0, Behavior::Normal), None);
    assert_eq!(
        warp_on_enter(m, 1, 0, 0, Behavior::ArrowWarp(Dir::Up)),
        None,
        "arrow warps need a push"
    );
    assert_eq!(
        warp_on_enter(m, 0, 0, 0, Behavior::Warp),
        None,
        "no warp event"
    );
}

#[test]
fn warp_on_enter_elevation_rule() {
    let m = town();
    assert_eq!(warp_on_enter(m, 4, 4, 3, Behavior::Warp), Some(1));
    assert_eq!(warp_on_enter(m, 4, 4, 4, Behavior::Warp), None);
    assert_eq!(warp_on_enter(m, 2, 1, 9, Behavior::Door), Some(0));
}

#[test]
fn warp_on_push_only_in_the_arrow_direction() {
    let m = town();
    let b = Behavior::ArrowWarp(Dir::Up);
    assert_eq!(warp_on_push(m, 1, 0, 0, b, Dir::Up), Some(2));
    for d in [Dir::Down, Dir::Left, Dir::Right] {
        assert_eq!(warp_on_push(m, 1, 0, 0, b, d), None);
    }
    assert_eq!(warp_on_push(m, 1, 0, 0, Behavior::Warp, Dir::Up), None);
    assert_eq!(warp_on_push(m, 0, 0, 0, b, Dir::Up), None, "no warp event");
}

#[test]
fn door_warp_only_northwards_into_a_door() {
    let m = town();
    assert_eq!(door_warp(m, 2, 1, 0, Behavior::Door, Dir::Up), Some(0));
    assert_eq!(door_warp(m, 2, 1, 0, Behavior::Door, Dir::Left), None);
    assert_eq!(door_warp(m, 2, 1, 0, Behavior::Warp, Dir::Up), None);
}

fn no_objects(_: i16, _: i16, _: u8) -> Option<u8> {
    None
}

#[test]
fn interact_object_in_front() {
    let m = town();
    let t = interact_target(m, 2, 2, 0, Dir::Up, Behavior::Normal, |x, y, _| {
        ((x, y) == (2, 2)).then_some(4)
    });
    assert_eq!(t, Some(Target::Object(4)));
}

#[test]
fn interact_passes_elevation_to_object_lookup() {
    let m = town();
    let mut seen = None;
    interact_target(m, 2, 2, 3, Dir::Up, Behavior::Normal, |_, _, e| {
        seen = Some(e);
        None
    });
    assert_eq!(seen, Some(3));
}

#[test]
fn interact_never_targets_the_player() {
    let m = town();
    let t = interact_target(m, 2, 2, 0, Dir::Up, Behavior::Normal, |_, _, _| {
        Some(PLAYER)
    });
    assert_eq!(t, None);
}

#[test]
fn interact_reaches_across_a_counter() {
    let m = town();
    // Player at (1, 4) facing up at the counter (1, 3); the clerk stands at (1, 2).
    let clerk = |x, y, _| ((x, y) == (1, 2)).then_some(7);
    assert_eq!(
        interact_target(m, 1, 3, 0, Dir::Up, Behavior::Counter, clerk),
        Some(Target::Object(7))
    );
    assert_eq!(
        interact_target(m, 1, 3, 0, Dir::Up, Behavior::Normal, clerk),
        None,
        "not across a plain cell"
    );
}

#[test]
fn interact_object_beats_bg_event() {
    let m = town();
    let t = interact_target(m, 5, 2, 0, Dir::Right, Behavior::Normal, |_, _, _| Some(1));
    assert_eq!(t, Some(Target::Object(1)));
}

#[test]
fn interact_bg_facing_rules() {
    let m = town();
    let t = |x, y, d| interact_target(m, x, y, 0, d, Behavior::Normal, no_objects);
    assert_eq!(t(0, 2, Dir::Up), Some(Target::Bg(0)));
    for d in [Dir::Down, Dir::Left, Dir::Right] {
        assert_eq!(t(0, 2, d), None, "sign readable only facing up");
    }
    for d in Dir::ALL {
        assert_eq!(t(5, 2, d), Some(Target::Bg(1)), "any facing");
    }
    assert_eq!(t(5, 3, Dir::Up), None, "hidden items are ignored");
    assert_eq!(t(3, 3, Dir::Up), None);
}

#[test]
fn interact_bg_elevation_rule() {
    let m = town();
    let t = |e| interact_target(m, 4, 0, e, Dir::Up, Behavior::Normal, no_objects);
    assert_eq!(t(2), Some(Target::Bg(3)), "first matching bg event");
    assert_eq!(
        t(0),
        Some(Target::Bg(4)),
        "elevated sign skipped at elevation 0"
    );
    assert_eq!(t(5), Some(Target::Bg(4)));
}

#[test]
fn on_frame_first_entry_whose_var_equals_value() {
    let m = town();
    assert_eq!(map_script_on_frame(m, &vars(&[(0x4006, 9)])), None);
    assert_eq!(
        map_script_on_frame(m, &vars(&[(0x4005, 1), (0x4006, 9)])),
        Some(ScriptId(30))
    );
    // Value 0x4007 names a var: compares against it, as Gen3 VarGet.
    assert_eq!(
        map_script_on_frame(m, &vars(&[(0x4006, 9), (0x4007, 9)])),
        Some(ScriptId(32))
    );
    assert_eq!(
        map_script_on_frame(m, &vars(&[])),
        Some(ScriptId(32)),
        "two unset vars are equal"
    );
}

#[test]
fn on_warp_into_first_entry_whose_var_equals_value() {
    let m = town();
    assert_eq!(on_warp_into(m, &vars(&[])), Some(ScriptId(41)));
    assert_eq!(on_warp_into(m, &vars(&[(0x4005, 2)])), Some(ScriptId(40)));
    assert_eq!(on_warp_into(m, &vars(&[(0x4006, 1)])), None);
    assert_eq!(on_warp_into(Game.map(HOUSE), &vars(&[])), None);
}
