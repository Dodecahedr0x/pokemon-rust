//! On-disk format (v2) of `content/extracted/`: written by `export`, read by `content/build.rs`
//! and later the studio. Everything is referenced by stable decomp-style **names**; ROM offsets
//! only appear in `manifest.ron` as provenance.
//!
//! ```text
//! content/extracted/
//!   manifest.ron                 Manifest
//!   maps/<Map>.ron               MapFile        (<Map> = decomp map dir name, e.g. LittlerootTown)
//!   tilesets/<Tileset>.ron       TilesetFile    (<Tileset> = decomp name, e.g. General, Petalburg)
//!   tilesets/<Tileset>.png       tiles, indexed 4bpp, 16 tiles per row, palette = the tileset's
//!                                first used palette (0 primary, 6 secondary)
//!   sprites/<Gfx>.ron            SpriteFile     (<Gfx> = OBJ_EVENT_GFX_* name without prefix)
//!   sprites/<Gfx>.png            frames side by side, own palette
//!   font/latin_normal.{png,ron}  FontFile
//!   window/message_box.{png,ron} WindowFile
//!   text/<lang>.ron              TextFile (raw RON strings in decomp `.string` syntax)
//!   scripts/<Map>.rs             generated async Rust (M3 Task B)
//!   scripts/index.ron            ScriptIndex
//!   previews/<Map>.png
//! ```
//!
//! **Naming rules** (shared by every writer and reader):
//! - Map: decomp map directory name from `names::MAPS` (`LittlerootTown`, `Route101`).
//! - Tileset: decomp `gTileset_<Name>` without prefix (`General`, `Petalburg`, `Building`).
//! - Script roots, in this order: `<Map>_Obj<local_id>`, `<Map>_Coord<i>`, `<Map>_Bg<i>`,
//!   `<Map>_OnLoad`, `<Map>_OnTransition`, `<Map>_OnResume`, `<Map>_OnReturnToField`,
//!   `<Map>_OnDiveWarp`, `<Map>_OnFrame<i>`, `<Map>_OnWarpInto<i>` (`i` = index in that list).
//!   When several roots share one ROM script, every reference uses the **canonical** name: the
//!   first in that order, maps taken in `manifest.maps` order.
//! - Labels reached by `goto`/`call` inside a script: `<Root>_L<n>`, `n` in order first reached.
//! - Texts: `<Script>_T<n>` where `<Script>` is the root or label first reaching it and `n` counts
//!   texts within that script in order first reached.
//! - Movements: `<Script>_M<n>`, same scheme.
//! - Flags, vars, gfx, movement types, items, specials: decomp constant names (`FLAG_HIDE_…`,
//!   `VAR_…`) from `names`. In `.ron` files a number with no name is written `FLAG_0x123` /
//!   `VAR_0x4123`; generated scripts write it `FlagId(0x123)` / `VarId(0x4123)` (other kinds as
//!   plain number literals), since the prelude only defines the named constants.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u32 = 2;

/// A ROM file offset, written `"0x3e45f0"`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Hex(pub u32);

impl Serialize for Hex {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{:#x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Hex, D::Error> {
        let s = String::deserialize(d)?;
        s.strip_prefix("0x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .map(Hex)
            .ok_or_else(|| serde::de::Error::custom(format!("expected 0x-prefixed hex, got {s:?}")))
    }
}

/// One row of u16 values written on a single line as a RON tuple.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row(pub Vec<u16>);

impl Serialize for Row {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut t = s.serialize_tuple(self.0.len())?;
        for v in &self.0 {
            t.serialize_element(v)?;
        }
        t.end()
    }
}

impl<'de> Deserialize<'de> for Row {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Row, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Row;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a tuple of u16")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Row, A::Error> {
                let mut row = Vec::new();
                while let Some(v) = seq.next_element()? {
                    row.push(v);
                }
                Ok(Row(row))
            }
        }
        d.deserialize_tuple(usize::MAX, V)
    }
}

pub fn to_rows(values: &[u16], width: usize) -> Vec<Row> {
    values
        .chunks(width.max(1))
        .map(|r| Row(r.to_vec()))
        .collect()
}

pub fn from_rows(rows: &[Row]) -> Vec<u16> {
    rows.iter().flat_map(|r| r.0.iter().copied()).collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    pub format: u32,
    /// "BPEF" or "BPEE".
    pub game_code: String,
    pub sha1: String,
    /// "fr" or "en": the text file is `text/<language>.ron`.
    pub language: String,
    /// Exported maps, in export order (this order decides canonical script names).
    pub maps: Vec<String>,
    pub tilesets: Vec<String>,
    pub sprites: Vec<String>,
    /// Where things came from, e.g. `"map/LittlerootTown/header" -> 0x4874a0`.
    pub provenance: BTreeMap<String, Hex>,
    /// Object graphics ids referenced but not exported (e.g. VAR graphics).
    pub skipped_graphics: BTreeSet<String>,
    /// Script opcodes the decoder did not understand, per script name.
    pub unknown_script_ops: BTreeMap<String, BTreeMap<Hex, u8>>,
    /// sha1 of every other written file, by path relative to the output root.
    pub files: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapFile {
    pub name: String,
    pub group: u8,
    pub num: u8,
    pub layout: Layout,
    pub header: Header,
    pub objects: Vec<Object>,
    pub warps: Vec<Warp>,
    pub coords: Vec<Coord>,
    pub bgs: Vec<Bg>,
    pub connections: Vec<Connection>,
    pub scripts: MapScripts,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub primary: String,
    pub secondary: String,
    /// 2 rows of 2 raw blocks.
    pub border: Vec<Row>,
    /// `height` rows of `width` raw blocks (metatile | collision << 10 | elevation << 12).
    pub blocks: Vec<Row>,
}

/// Scalar `MapHeader` fields, raw Gen3 values.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Header {
    pub music: u16,
    pub region_map_section: u8,
    pub cave: u8,
    pub weather: u8,
    pub map_type: u8,
    pub allow_cycling: bool,
    pub allow_escaping: bool,
    pub allow_running: bool,
    pub show_map_name: bool,
    pub battle_type: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Object {
    pub local_id: u8,
    /// `OBJ_EVENT_GFX_*` name; `OBJ_EVENT_GFX_VAR_<n>` for var graphics.
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
    /// `FLAG_*` name; `None` when the object has no hide flag (flag 0).
    pub flag: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Warp {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    /// Destination map name; `None` for `MAP_DYNAMIC` (0x7F7F) and other unexported targets.
    pub dest_map: Option<String>,
    /// Destination map as `"<group>.<num>"`, always present.
    pub dest_map_id: String,
    pub dest_warp: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    /// `VAR_*` name; `None` for weather triggers (var 0).
    pub var: Option<String>,
    pub value: u16,
    pub script: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BgKind {
    /// Raw Gen3 kind 0..=4 (`BG_EVENT_PLAYER_FACING_ANY/NORTH/SOUTH/EAST/WEST`).
    Script {
        facing: u8,
        script: Option<String>,
    },
    HiddenItem {
        item: String,
        flag: String,
    },
    SecretBase {
        id: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Bg {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub kind: BgKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionDir {
    South,
    North,
    West,
    East,
    Dive,
    Emerge,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Connection {
    pub dir: ConnectionDir,
    pub offset: i32,
    /// Neighbor map name; `None` when that map is not exported.
    pub map: Option<String>,
    pub map_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MapScripts {
    pub on_load: Option<String>,
    pub on_transition: Option<String>,
    pub on_resume: Option<String>,
    pub on_return_to_field: Option<String>,
    pub on_dive_warp: Option<String>,
    pub on_frame: Vec<VarScript>,
    pub on_warp_into: Vec<VarScript>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VarScript {
    pub var: String,
    pub value: u16,
    pub script: String,
}

/// `tilesets/<Name>.ron`; tiles are in `tilesets/<Name>.png`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TilesetFile {
    pub name: String,
    pub secondary: bool,
    pub tile_count: u16,
    /// All 16 palettes as stored in the ROM.
    pub palettes: Vec<[u16; 16]>,
    /// 8 raw tile entries per metatile, one metatile per line.
    pub metatiles: Vec<[u16; 8]>,
    pub attributes: Vec<u16>,
}

/// `sprites/<Name>.ron`; frames are cells of `cell_width`x`cell_height` side by side in
/// `sprites/<Name>.png`, each frame drawn top-left in its cell.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpriteFile {
    pub name: String,
    pub gfx_id: u16,
    pub width: u16,
    pub height: u16,
    pub cell_width: u16,
    pub cell_height: u16,
    /// (width, height) of each frame; usually all equal to (width, height).
    pub frame_sizes: Vec<(u16, u16)>,
    pub palette_tag: u16,
    pub palette: [u16; 16],
}

/// `font/latin_normal.ron`; glyphs are in `font/latin_normal.png` (2bpp, 16x16 cells).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FontFile {
    pub glyphs: u16,
    pub glyphs_per_row: u16,
    pub widths: Vec<u8>,
}

/// `window/message_box.ron`; tiles are in `window/message_box.png` (4bpp, one row).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WindowFile {
    pub tile_count: u16,
    /// BG palette for the dialogue window: frame and text colors.
    pub palette: [u16; 16],
    /// Palette indices the text printer uses.
    pub text_fg: u8,
    pub text_shadow: u8,
    pub text_bg: u8,
}

/// `text/<lang>.ron`: text name -> string in decomp `.string` syntax.
pub type TextFile = BTreeMap<String, String>;

/// `scripts/index.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ScriptIndex {
    /// Every generated script or label fn, by name.
    pub scripts: BTreeMap<String, ScriptEntry>,
    /// Every movement table, by name.
    pub movements: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScriptEntry {
    /// `scripts/<file>.rs` defining it.
    pub file: String,
    /// Whether a map event or map script refers to it directly.
    pub root: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_as_a_string() {
        let s = ron::to_string(&Hex(0x3e45f0)).unwrap();
        assert_eq!(s, "\"0x3e45f0\"");
        assert_eq!(ron::from_str::<Hex>(&s).unwrap(), Hex(0x3e45f0));
        assert!(ron::from_str::<Hex>("\"3e45f0\"").is_err());
    }

    #[test]
    fn rows_round_trip() {
        let rows = to_rows(&[1, 2, 3, 4, 5, 6], 3);
        assert_eq!(rows.len(), 2);
        let s = ron::to_string(&rows).unwrap();
        assert_eq!(s, "[(1,2,3),(4,5,6)]");
        let back: Vec<Row> = ron::from_str(&s).unwrap();
        assert_eq!(from_rows(&back), vec![1, 2, 3, 4, 5, 6]);
    }
}
