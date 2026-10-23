//! The files the editor writes under a games dir (`games/<game>/`):
//!
//! ```text
//! maps/<Name>.ron     GameMap: the extract's `MapFile` with the layout by name, objects with
//!                     an optional LOCALID_* name, the header by constant names, and warp and
//!                     connection targets by map name only (no Gen3 group.num)
//! layouts/<Name>.ron  extract::schema::Layout: size, tilesets by extract name, raw blocks
//! ```
//!
//! The types are defined once, in `extract::games::maps` (SIL1 Wave 0), and shared with the
//! content build, which loads these files; this module re-exports them and adds the editor's
//! RON helpers and [`header_from_extract`] (which needs the editor's [`names`] tables).

use super::names;
use anyhow::{Context, Result};
pub use extract::games::maps::{
    GameConnection, GameHeader, GameMap, GameObject, GameWarp, MAP_DYNAMIC, WARP_ID_DYNAMIC,
    WARP_ID_SECRET_BASE, dynamic_warp_id,
};
use extract::schema::Header;
use serde::{Serialize, de::DeserializeOwned};

/// An extracted header, numbers named (unnamed ones as `<PREFIX>0x<HEX>`).
pub fn header_from_extract(h: &Header) -> GameHeader {
    GameHeader {
        music: names::MUSIC.name_or_hex(h.music as u32),
        region_map_section: names::MAP_SECTIONS.name_or_hex(h.region_map_section as u32),
        cave: h.cave,
        weather: names::WEATHER.name_or_hex(h.weather as u32),
        map_type: names::MAP_TYPES.name_or_hex(h.map_type as u32),
        allow_cycling: h.allow_cycling,
        allow_escaping: h.allow_escaping,
        allow_running: h.allow_running,
        show_map_name: h.show_map_name,
        battle_scene: names::BATTLE_SCENES.name_or_hex(h.battle_type as u32),
    }
}

/// The one RON style of every games dir file (the extract's: 4-space indent, `\n`, no struct
/// names, a trailing newline), so the same data always gives the same bytes.
pub fn to_ron<T: Serialize>(value: &T) -> Result<String> {
    extract::export::to_ron(value)
}

/// Parses a games dir file (strict: no RON extensions).
pub fn from_ron<T: DeserializeOwned>(text: &str) -> Result<T> {
    ron::from_str(text).context("RON")
}

/// Parses an edit script or a map spec: like [`from_ron`], with `implicit_some` so optional
/// fields take a bare value (`script: "Foo"` for `script: Some("Foo")`).
pub fn from_script_ron<T: DeserializeOwned>(text: &str) -> Result<T> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(text)
        .context("RON")
}
