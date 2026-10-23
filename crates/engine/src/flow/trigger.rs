//! Step, push and interaction triggers as pure functions over map data (Gen3
//! `field_control_avatar.c`, `script.c`).
//!
//! Positions are map cells. `behavior` arguments are the classified behavior of the cell named by
//! the same call: the caller reads the block from the live field (`Field::block`, which includes
//! `setmetatile` edits) and classifies it with [`block_behavior`].

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

/// Classified behavior of `block` (raw, possibly a live `setmetatile` override), looked up in
/// `map`'s primary or secondary tileset attributes (Gen3 `MapGridGetMetatileBehaviorAt`, which
/// uses the current map's tilesets even for connection cells). Unknown metatiles give the
/// classification of raw 0xFF (`MB_INVALID`).
pub fn block_behavior(game: &impl GameData, map: &MapData, block: u16) -> Behavior {
    let mt = block_metatile(block);
    let attr = if mt < METATILES_IN_PRIMARY {
        map.primary.attributes.get(mt as usize)
    } else {
        map.secondary
            .attributes
            .get((mt - METATILES_IN_PRIMARY) as usize)
    };
    game.behavior(attr.map_or(MB_INVALID, |&a| attr_behavior(a)))
}

/// [`block_behavior`] of the static block at (x, y) ([`block_at`]: own blocks, connections,
/// border). Callers with a live field should pass its block to [`block_behavior`] instead.
pub fn behavior_at(game: &impl GameData, map: &MapData, x: i16, y: i16) -> Behavior {
    block_behavior(game, map, block_at(game, map, x, y))
}

/// Elevation used for events on the cell in front of the player (Gen3
/// `GetInFrontOfPlayerPosition`): the player's elevation, or 0 if the block under the player
/// (`player_block`, from the live field) has elevation 0.
pub const fn facing_elevation(player_block: u16, player_elevation: u8) -> u8 {
    if block_elevation(player_block) != 0 {
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

/// Warp taken by finishing a step on (x, y) (Gen3 `TryStartWarpEventScript`): the cell
/// [`Behavior::enters_warp`] (doors, non-animated doors, ladders, escalators, other warps) and
/// holds a warp event matching `elevation`. Returns the index into `map.warps`.
pub fn warp_on_enter(
    map: &MapData,
    x: i16,
    y: i16,
    elevation: u8,
    behavior: Behavior,
) -> Option<usize> {
    if !behavior.enters_warp() {
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
/// [`elevation_matches`]. It should skip the player; a returned `data::PLAYER` is ignored anyway. Across a `Counter` the
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
mod tests;
