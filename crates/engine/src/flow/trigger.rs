//! Step, push and interaction triggers as pure functions over map data (Gen3
//! `field_control_avatar.c`, `script.c`).
//!
//! Positions are map cells. `behavior` arguments are the classified behavior of the cell named by
//! the same call (the caller reads it from the live field, which scripts may have edited);
//! [`behavior_at`] gives it from static data.

use super::Vars;
use crate::data::{
    Behavior, BgKind, Dir, GameData, METATILES_IN_PRIMARY, MapData, PLAYER, ScriptId, VarScript,
    attr_behavior, block_elevation, block_metatile,
};
use crate::grid::block_at;

/// What the player talks to with A.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// An object, by local id.
    Object(u8),
    /// A sign or other bg event, by index into `MapData::bgs`.
    Bg(usize),
}

/// Raw behavior byte of an unknown metatile (Gen3 `MB_INVALID`, `GetMetatileAttributesById`).
const MB_INVALID: u8 = 0xFF;

/// Behavior of the cell from static data: its block (across connections and the border, see
/// [`block_at`]) looked up in `map`'s tilesets, as Gen3 `MapGridGetMetatileBehaviorAt`.
pub fn behavior_at(game: &impl GameData, map: &MapData, x: i16, y: i16) -> Behavior {
    let mt = block_metatile(block_at(game, map, x, y));
    let attr = if mt < METATILES_IN_PRIMARY {
        map.primary.attributes.get(mt as usize)
    } else {
        map.secondary
            .attributes
            .get((mt - METATILES_IN_PRIMARY) as usize)
    };
    game.behavior(attr.map_or(MB_INVALID, |&a| attr_behavior(a)))
}

/// Elevation used for events on the cell in front of the player (Gen3
/// `GetInFrontOfPlayerPosition`): the player's elevation, or 0 if the player's own cell has
/// elevation 0 in the map grid.
pub fn facing_elevation(
    game: &impl GameData,
    map: &MapData,
    px: i16,
    py: i16,
    player_elevation: u8,
) -> u8 {
    if block_elevation(block_at(game, map, px, py)) != 0 {
        player_elevation
    } else {
        0
    }
}

/// Object elevation rule (Gen3 `ObjectEventDoesElevationMatch`): 0 on either side matches all.
pub const fn elevation_matches(a: u8, b: u8) -> bool {
    a == 0 || b == 0 || a == b
}

/// Event elevation rule (Gen3 `Get{Warp,Coord,Background}EventAtPosition`): an event at
/// elevation 0 matches any position; otherwise elevations must be equal.
const fn event_matches(event: u8, position: u8) -> bool {
    event == 0 || event == position
}

fn warp_at(map: &MapData, x: i16, y: i16, elevation: u8) -> Option<usize> {
    map.warps
        .iter()
        .position(|w| w.x == x && w.y == y && event_matches(w.elevation, elevation))
}

/// Coord event script for the player finishing a step on (x, y) (Gen3
/// `GetCoordEventScriptAtPosition`): the first event there whose elevation matches and whose
/// var equals its value. As Gen3 the value is compared truncated to `u8`
/// (`VarGet(trigger) == (u8)index`). Var 0 events (Gen3 weather / run-immediately triggers)
/// never match.
pub fn coord_trigger(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    vars: &Vars,
) -> Option<ScriptId> {
    map.coords
        .iter()
        .filter(|c| c.x == x && c.y == y && event_matches(c.elevation, elevation))
        .filter(|c| c.var.0 != 0)
        .find(|c| vars.var_or_literal(c.var.0) == c.value as u8 as u16)
        .map(|c| c.script)
}

/// Warp taken by finishing a step on (x, y) (Gen3 `TryStartWarpEventScript`): the cell is a
/// `Door` or `Warp` and holds a warp event matching `elevation`. Returns the index into
/// `map.warps`.
pub fn warp_on_enter(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    behavior: Behavior,
) -> Option<usize> {
    if !matches!(behavior, Behavior::Door | Behavior::Warp) {
        return None;
    }
    warp_at(map, x, y, elevation)
}

/// Warp taken by pushing `dir` while standing on (x, y) (Gen3 `TryArrowWarp`): the cell is an
/// `ArrowWarp(dir)` with a warp event. Gen3 only checks this when the held d-pad direction equals
/// the facing direction; pass that direction.
pub fn warp_on_push(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    behavior: Behavior,
    dir: Dir,
) -> Option<usize> {
    if behavior != Behavior::ArrowWarp(dir) {
        return None;
    }
    warp_at(map, x, y, elevation)
}

/// Warp taken by pushing `dir` into the door on the cell in front, (x, y) (Gen3 `TryDoorWarp`):
/// only northwards, into a `Door` with a warp event. `elevation` is [`facing_elevation`].
pub fn door_warp(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    behavior: Behavior,
    dir: Dir,
) -> Option<usize> {
    if dir != Dir::Up || behavior != Behavior::Door {
        return None;
    }
    warp_at(map, x, y, elevation)
}

/// What pressing A while facing `dir` reaches (Gen3 `GetInteractionScript`, objects then bg
/// events). (x, y) is the cell in front of the player, `behavior` its behavior and `elevation`
/// [`facing_elevation`].
///
/// `object_at(x, y, elevation)` returns the local id of a live object on that cell whose elevation
/// [`elevation_matches`]; the player (`data::PLAYER`) is never a target. Across a `Counter` the
/// object one cell further is reached. Bg events are only looked up on the front cell; a
/// facing-restricted sign seen from the wrong side, and hidden items, give `None`.
pub fn interact_target(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    dir: Dir,
    behavior: Behavior,
    mut object_at: impl FnMut(i16, i16, u8) -> Option<u8>,
) -> Option<Target> {
    let mut obj = |x, y| object_at(x, y, elevation).filter(|&id| id != PLAYER);
    if let Some(id) = obj(x, y) {
        return Some(Target::Object(id));
    }
    if behavior == Behavior::Counter {
        let (dx, dy) = dir.delta();
        if let Some(id) = obj(x + dx, y + dy) {
            return Some(Target::Object(id));
        }
    }
    let (i, bg) = map
        .bgs
        .iter()
        .enumerate()
        .find(|(_, b)| b.x == x && b.y == y && event_matches(b.elevation, elevation))?;
    match bg.kind {
        BgKind::Script { facing, .. } if facing.is_none_or(|f| f == dir) => Some(Target::Bg(i)),
        _ => None,
    }
}

/// Gen3 `MapHeaderCheckScriptTable`: the first entry whose var equals its value, both read with
/// `VarGet` (so a value naming a var compares against that var).
fn check_table(table: &[VarScript], vars: &Vars) -> Option<ScriptId> {
    table
        .iter()
        .find(|s| vars.var_or_literal(s.var.0) == vars.var_or_literal(s.value))
        .map(|s| s.script)
}

/// `MAP_SCRIPT_ON_FRAME_TABLE` script to start now, if any. Gen3 checks it every frame the
/// player has control, before step triggers (`TryRunOnFrameMapScript`).
pub fn map_script_on_frame(map: &MapData, vars: &Vars) -> Option<ScriptId> {
    check_table(map.scripts.on_frame, vars)
}

/// `MAP_SCRIPT_ON_WARP_INTO_MAP_TABLE` script to run after a map load, if any.
pub fn on_warp_into(map: &MapData, vars: &Vars) -> Option<ScriptId> {
    check_table(map.scripts.on_warp_into, vars)
}

#[cfg(test)]
mod tests {
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
    fn facing_elevation_is_zero_when_standing_on_elevation_zero() {
        let m = town();
        assert_eq!(facing_elevation(&Game, m, 0, 0, 3), 0);
        assert_eq!(facing_elevation(&Game, m, 3, 2, 3), 3);
        assert_eq!(facing_elevation(&Game, m, 4, 4, 4), 4);
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
}
