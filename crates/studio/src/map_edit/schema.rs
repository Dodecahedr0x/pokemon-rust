//! The files the editor writes under a games dir (`games/<game>/`):
//!
//! ```text
//! maps/<Name>.ron     GameMap: the extract's `MapFile` with the layout by name, objects with
//!                     an optional LOCALID_* name, the header by constant names, and warp and
//!                     connection targets by map name only (no Gen3 group.num)
//! layouts/<Name>.ron  extract::schema::Layout: size, tilesets by extract name, raw blocks
//! ```
//!
//! Only names, raw block values (metatile index | collision << 10 | elevation << 12) and event
//! data: never tiles, palettes or metatile graphics (`hygiene`). Coord events, bg events and
//! the map scripts are the extract's own types. The content build (SIL1 Task A) mirrors
//! [`GameMap`], [`GameObject`], [`GameHeader`], [`GameWarp`] and [`GameConnection`] (it cannot
//! depend on the studio) and resolves the names.

use super::names;
use anyhow::{Context, Result};
use extract::schema::{Bg, ConnectionDir, Coord, Header, MapScripts, Object};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The warp destination resolved at run time (Gen3 `MAP_DYNAMIC`, 0x7F7F); its `dest_warp` is
/// [`WARP_ID_DYNAMIC`].
pub const MAP_DYNAMIC: &str = "MAP_DYNAMIC";
/// Gen3 `WARP_ID_DYNAMIC`.
pub const WARP_ID_DYNAMIC: u8 = 127;
/// Gen3 `WARP_ID_SECRET_BASE`: the secret bases' exit, also a [`MAP_DYNAMIC`] warp (Gen3
/// `SetupWarp` takes the dynamic warp whatever the id).
pub const WARP_ID_SECRET_BASE: u8 = 126;

/// Whether `id` is a valid destination warp of a [`MAP_DYNAMIC`] warp.
pub const fn dynamic_warp_id(id: u8) -> bool {
    id == WARP_ID_DYNAMIC || id == WARP_ID_SECRET_BASE
}

/// `maps/<Name>.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameMap {
    pub name: String,
    /// `layouts/<layout>.ron` in the same games dir (maps may share a layout).
    pub layout: String,
    pub header: GameHeader,
    pub objects: Vec<GameObject>,
    pub warps: Vec<GameWarp>,
    pub coords: Vec<Coord>,
    pub bgs: Vec<Bg>,
    pub connections: Vec<GameConnection>,
    pub scripts: MapScripts,
}

/// The extract's `Header` with constant names instead of raw numbers (Silice adds its own
/// `MUS_*`, `WEATHER_*`…; the content build resolves them).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameHeader {
    /// `MUS_*`.
    pub music: String,
    /// `MAPSEC_*`.
    pub region_map_section: String,
    /// Raw Gen3 `cave` (flash requirement).
    pub cave: u8,
    /// `WEATHER_*`.
    pub weather: String,
    /// `MAP_TYPE_*`.
    pub map_type: String,
    pub allow_cycling: bool,
    pub allow_escaping: bool,
    pub allow_running: bool,
    pub show_map_name: bool,
    /// `MAP_BATTLE_SCENE_*`.
    pub battle_scene: String,
}

impl GameHeader {
    /// A new map's: no music change, no weather, map section none, running allowed.
    pub fn new_map() -> GameHeader {
        GameHeader {
            music: "MUS_NONE".into(),
            region_map_section: "MAPSEC_NONE".into(),
            cave: 0,
            weather: "WEATHER_NONE".into(),
            map_type: "MAP_TYPE_NONE".into(),
            allow_cycling: true,
            allow_escaping: false,
            allow_running: true,
            show_map_name: true,
            battle_scene: "MAP_BATTLE_SCENE_NORMAL".into(),
        }
    }

    /// An extracted header, numbers named (unnamed ones as `<PREFIX>0x<HEX>`).
    pub fn from_extract(h: &Header) -> GameHeader {
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
}

/// An object: the extract's `Object` plus an optional `LOCALID_*` name, from which the content
/// build emits the constant scripts use. `local_id` is stable: a new object gets the highest
/// id + 1 and a deletion leaves a gap (the engine and Gen3 look templates up by local id, and
/// checkpoints record it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameObject {
    pub local_id: u8,
    /// `LOCALID_*`.
    pub id: Option<String>,
    /// `OBJ_EVENT_GFX_*` name.
    pub gfx: String,
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    /// `MOVEMENT_TYPE_*` name.
    pub movement: String,
    pub range_x: u8,
    pub range_y: u8,
    /// `TRAINER_TYPE_*` name.
    pub trainer_type: String,
    pub trainer_sight_or_berry_tree: u16,
    pub script: Option<String>,
    /// Hide flag (`FLAG_*`).
    pub flag: Option<String>,
}

impl GameObject {
    pub fn from_extract(o: &Object) -> GameObject {
        GameObject {
            local_id: o.local_id,
            id: None,
            gfx: o.gfx.clone(),
            x: o.x,
            y: o.y,
            elevation: o.elevation,
            movement: o.movement.clone(),
            range_x: o.range_x,
            range_y: o.range_y,
            trainer_type: o.trainer_type.clone(),
            trainer_sight_or_berry_tree: o.trainer_sight_or_berry_tree,
            script: o.script.clone(),
            flag: o.flag.clone(),
        }
    }
}

/// A warp: the extract's `Warp` with the destination by name only.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameWarp {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    /// A map of the same games dir, or [`MAP_DYNAMIC`].
    pub dest_map: String,
    /// Index in the destination's warps ([`WARP_ID_DYNAMIC`] for [`MAP_DYNAMIC`]).
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
