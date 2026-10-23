//! `games/<game>/game.ron` (SIL1 Task E): the new game and the system ids of an authored game,
//! which the content build loads (`crates/content/build/silice/game.rs`) into the game root's
//! `GameData::{new_game, special_ids, heal_location}` and its new-game script.
//!
//! Names only: maps by file name, flags, vars and species by constant name. A Silice name is
//! resolved through the ids files; a Gen3 flag or var must be one the shared code reserves
//! ([`super::SHARED_GEN3`]), so no Silice id can take its number.
//!
//! - `start`: the new game's map, cell and facing.
//! - `running_flag`: running is allowed while it is set (`SpecialIds::flag_running`).
//! - `cleared_on_map_change`: the Gen3 system flags cleared on every map change
//!   (`SpecialIds::flags_cleared_on_map_change`, Gen3 `ClearTempFieldEventData`).
//! - `heal_locations`: **append-only**; a heal location's number is its index plus one (Gen3
//!   numbers them from 1, 0 is `HEAL_LOCATION_NONE`). Each name becomes a `u8` constant.
//! - `respawn`: the heal location of a new game (`SpecialIds::respawn_default`).
//! - `starters` and `starter_level`: what `choose_starter` offers and gives.
//! - `new_game_flags` / `new_game_vars`: what the new-game script sets before the first map
//!   loads, for example the hide flags of objects that a scene shows later.

use serde::{Deserialize, Serialize};

/// `game.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameFile {
    pub start: GameStart,
    pub running_flag: String,
    pub cleared_on_map_change: Vec<String>,
    pub heal_locations: Vec<HealLocation>,
    /// A heal location's name.
    pub respawn: String,
    /// Exactly three species.
    pub starters: Vec<String>,
    pub starter_level: u8,
    #[serde(default)]
    pub new_game_flags: Vec<String>,
    #[serde(default)]
    pub new_game_vars: Vec<(String, u16)>,
}

/// Where a new game starts.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameStart {
    /// A map file name.
    pub map: String,
    pub x: i16,
    pub y: i16,
    pub facing: Facing,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Down,
    Up,
    Left,
    Right,
}

/// A heal location: where a whiteout (or, later, Fly) puts the player.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HealLocation {
    /// `HEAL_LOCATION_SIL_*`.
    pub name: String,
    /// A map file name.
    pub map: String,
    pub x: i16,
    pub y: i16,
}

impl GameFile {
    /// The number of heal location `name` (its index plus one).
    pub fn heal_location(&self, name: &str) -> Option<u8> {
        let i = self.heal_locations.iter().position(|h| h.name == name)?;
        u8::try_from(i + 1).ok()
    }
}
