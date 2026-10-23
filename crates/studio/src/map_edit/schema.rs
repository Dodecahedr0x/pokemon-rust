//! The files the editor writes under a games dir (`games/<game>/`):
//!
//! ```text
//! maps/<Name>.ron     GameMap: the extract's `MapFile` with the layout by name and the warp and
//!                     connection targets by map name only (no Gen3 group.num)
//! layouts/<Name>.ron  extract::schema::Layout: size, tilesets by extract name, raw blocks
//! ```
//!
//! Only names, raw block values (metatile index | collision << 10 | elevation << 12) and event
//! data: never tiles, palettes or metatile graphics (`hygiene`). Objects, coord events, bg
//! events, the header and the map scripts are the extract's own types, so the content build
//! (SIL1 Task A) reads them with the extract schema.

use anyhow::{Context, Result};
use extract::schema::{Bg, ConnectionDir, Coord, Header, MapScripts, Object};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// `maps/<Name>.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameMap {
    pub name: String,
    /// `layouts/<layout>.ron` in the same games dir (maps may share a layout).
    pub layout: String,
    pub header: Header,
    pub objects: Vec<Object>,
    pub warps: Vec<GameWarp>,
    pub coords: Vec<Coord>,
    pub bgs: Vec<Bg>,
    pub connections: Vec<GameConnection>,
    pub scripts: MapScripts,
}

/// A warp: the extract's `Warp` with the destination by name only.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameWarp {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    /// A map of the same games dir, or an extracted map.
    pub dest_map: String,
    /// Index in the destination's warps.
    pub dest_warp: u8,
}

/// A connection: the extract's `Connection` with the neighbor by name only.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameConnection {
    pub dir: ConnectionDir,
    pub offset: i32,
    pub map: String,
}

/// The header of a new map: no music, weather or map type yet, running allowed.
pub fn default_header() -> Header {
    Header {
        music: 0,
        region_map_section: 0,
        cave: 0,
        weather: 0,
        map_type: 0,
        allow_cycling: true,
        allow_escaping: false,
        allow_running: true,
        show_map_name: true,
        battle_type: 0,
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
