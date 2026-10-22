//! Map headers, layouts, events, connections and tileset structs (layouts from `include/global.fieldmap.h`).
//!
//! `gMapGroups` is found by anchoring on the first 64 bytes of Littleroot's `map.bin`, then following
//! pointers back: map.bin -> `MapLayout` -> `MapHeader` -> group-0 table -> `gMapGroups`.

use crate::{Rom, names};
use anyhow::{Context, Result, bail, ensure};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct MapId {
    pub group: u8,
    pub num: u8,
}

pub const LITTLEROOT: MapId = MapId { group: 0, num: 9 };
pub const OLDALE: MapId = MapId { group: 0, num: 10 };
pub const ROUTE101: MapId = MapId { group: 0, num: 16 };

/// First 64 bytes of `data/layouts/LittlerootTown/map.bin` (unchanged in every Western ROM).
const LITTLEROOT_ANCHOR: [u8; 64] = [
    0xd4, 0x05, 0xd5, 0x05, 0xd4, 0x05, 0xd5, 0x05, 0xd4, 0x05, 0xd5, 0x05, 0xd4, 0x05, 0xd5, 0x05,
    0xd4, 0x05, 0xd5, 0x35, 0x01, 0x30, 0x01, 0x30, 0xd4, 0x05, 0xd5, 0x05, 0xd4, 0x05, 0xd5, 0x05,
    0xd4, 0x05, 0xd5, 0x05, 0xd4, 0x05, 0xd5, 0x05, 0xdc, 0x05, 0xdd, 0x05, 0xe4, 0x05, 0xe5, 0x05,
    0xe4, 0x05, 0xe5, 0x05, 0xe4, 0x05, 0xe5, 0x05, 0xe4, 0x05, 0xe7, 0x05, 0x01, 0x30, 0x01, 0x30,
];
const LITTLEROOT_LAYOUT_ID: u16 = 10;
/// Littleroot's layout is 20x20 blocks (`data/layouts/layouts.json`).
const LITTLEROOT_WIDTH: u32 = 20;
const LITTLEROOT_HEIGHT: u32 = 20;

/// Tiles per tileset in VRAM (`NUM_TILES_IN_PRIMARY`): the base index of secondary tiles.
pub const TILES_PER_TILESET: usize = 512;
/// Metatiles per tileset (`NUM_METATILES_IN_PRIMARY`): the base id of secondary metatiles.
pub const METATILES_PER_TILESET: usize = 512;
const METATILE_SIZE: u32 = 16;
/// Largest map dimension accepted before reading block data (Emerald's biggest is well under this).
const MAX_DIM: u32 = 0x400;

/// `gMapGroups`: `offset` is the table itself, `groups[g][n]` the file offset of each `MapHeader`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapTable {
    pub offset: u32,
    pub groups: Vec<Vec<u32>>,
    /// Sorted metatiles/attributes pointers of every tileset the table uses, filled by the first
    /// `read_tileset`. Not serialized and ignored by `PartialEq`.
    tileset_bounds: BoundsCache,
}

#[derive(Clone, Default)]
struct BoundsCache(OnceLock<Vec<u32>>);

impl PartialEq for BoundsCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for BoundsCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() {
            "cached"
        } else {
            "empty"
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapHeader {
    pub offset: u32,
    pub layout: MapLayout,
    pub events: MapEvents,
    pub map_scripts: Option<u32>,
    pub connections: Vec<Connection>,
    pub music: u16,
    pub layout_id: u16,
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

#[derive(Clone, Debug, PartialEq)]
pub struct MapLayout {
    pub offset: u32,
    pub width: u16,
    pub height: u16,
    /// 2x2 border blocks, row-major.
    pub border: Vec<u16>,
    /// `width * height` raw blocks (metatile id | collision << 10 | elevation << 12).
    pub blocks: Vec<u16>,
    /// File offset of the primary `Tileset` struct.
    pub primary_tileset: u32,
    /// File offset of the secondary `Tileset` struct.
    pub secondary_tileset: u32,
}

/// A `Tileset` struct. Pointers are file offsets; `metatiles` holds `metatile_count * 8` u16 and
/// `metatile_attributes` holds `metatile_count` u16.
#[derive(Clone, Debug, PartialEq)]
pub struct TilesetRef {
    pub offset: u32,
    pub compressed: bool,
    pub secondary: bool,
    pub tiles: u32,
    pub palettes: u32,
    pub metatiles: u32,
    pub metatile_attributes: u32,
    pub metatile_count: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectEvent {
    pub local_id: u8,
    pub graphics_id: u8,
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub movement_type: u8,
    pub movement_range_x: u8,
    pub movement_range_y: u8,
    pub trainer_type: u16,
    pub trainer_range_berry_tree_id: u16,
    pub script: Option<u32>,
    pub flag_id: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warp {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub warp_id: u8,
    pub dest: MapId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoordEvent {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub trigger_var: u16,
    pub index: u16,
    pub script: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BgEventKind {
    Script(Option<u32>),
    HiddenItem { item: u16, hidden_item_id: u16 },
    SecretBase(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BgEvent {
    pub x: u16,
    pub y: u16,
    pub elevation: u8,
    pub kind_raw: u8,
    pub kind: BgEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapEvents {
    pub objects: Vec<ObjectEvent>,
    pub warps: Vec<Warp>,
    pub coords: Vec<CoordEvent>,
    pub bgs: Vec<BgEvent>,
}

/// Decomp `CONNECTION_*` order (1 = south … 6 = emerge).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Direction {
    South,
    North,
    West,
    East,
    Dive,
    Emerge,
}

impl Direction {
    fn from_raw(raw: u8) -> Option<Direction> {
        Some(match raw {
            1 => Direction::South,
            2 => Direction::North,
            3 => Direction::West,
            4 => Direction::East,
            5 => Direction::Dive,
            6 => Direction::Emerge,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub direction: Direction,
    pub offset: i32,
    pub map: MapId,
}

const BG_EVENT_HIDDEN_ITEM: u8 = 7;
const BG_EVENT_SECRET_BASE: u8 = 8;

/// Locates `gMapGroups` from the Littleroot map.bin anchor.
pub fn locate_map_table(rom: &Rom) -> Result<MapTable> {
    let hits = rom.find_all(&LITTLEROOT_ANCHOR);
    let [map_bin] = hits[..] else {
        bail!(
            "Littleroot map.bin anchor found {} times: {hits:x?}",
            hits.len()
        );
    };
    // MapLayout.map is at +0x0C; check width/height to reject stray pointers.
    let layouts: Vec<u32> = rom
        .pointers_to(map_bin)
        .into_iter()
        .filter_map(|p| p.checked_sub(0x0C))
        .filter(|&l| {
            rom.u32(l).ok() == Some(LITTLEROOT_WIDTH)
                && rom.u32(l + 4).ok() == Some(LITTLEROOT_HEIGHT)
        })
        .collect();
    let [layout] = layouts[..] else {
        bail!("Littleroot layout candidates for map.bin {map_bin:#x}: {layouts:x?}");
    };
    let headers: Vec<u32> = rom
        .pointers_to(layout)
        .into_iter()
        .filter(|&h| {
            rom.u16(h + 0x12).ok() == Some(LITTLEROOT_LAYOUT_ID)
                && matches!(rom.ptr(h + 4), Ok(Some(_)))
        })
        .collect();
    let [header] = headers[..] else {
        bail!("Littleroot header candidates for layout {layout:#x}: {headers:x?}");
    };
    let group0: Vec<u32> = rom
        .pointers_to(header)
        .into_iter()
        .filter_map(|p| p.checked_sub(4 * LITTLEROOT.num as u32))
        .collect();
    let mut found = Vec::new();
    for &g0 in &group0 {
        for table in rom.pointers_to(g0) {
            if let Ok(groups) = read_groups(rom, table) {
                found.push(MapTable {
                    offset: table,
                    groups,
                    tileset_bounds: BoundsCache::default(),
                });
            }
        }
    }
    match found.len() {
        1 => Ok(found.pop().unwrap()),
        n => bail!("{n} gMapGroups candidates for Littleroot header {header:#x}"),
    }
}

fn read_groups(rom: &Rom, table: u32) -> Result<Vec<Vec<u32>>> {
    let sizes = names::group_sizes();
    let mut groups = Vec::with_capacity(sizes.len());
    for (g, &size) in sizes.iter().enumerate() {
        let list = rom.req_ptr(table + 4 * g as u32, "map group")?;
        let mut headers = Vec::with_capacity(size as usize);
        for n in 0..size as u32 {
            headers.push(rom.req_ptr(list + 4 * n, "map header")?);
        }
        groups.push(headers);
    }
    Ok(groups)
}

impl MapTable {
    pub fn header_offset(&self, id: MapId) -> Result<u32> {
        self.groups
            .get(id.group as usize)
            .and_then(|g| g.get(id.num as usize))
            .copied()
            .with_context(|| format!("map {}.{} is not in the map table", id.group, id.num))
    }

    pub fn header(&self, rom: &Rom, id: MapId) -> Result<MapHeader> {
        let off = self.header_offset(id)?;
        read_header(rom, off)
            .with_context(|| format!("map {}.{} header at {off:#x}", id.group, id.num))
    }
}

pub fn read_header(rom: &Rom, off: u32) -> Result<MapHeader> {
    let layout = read_layout(rom, rom.req_ptr(off, "layout")?)?;
    let events = read_events(rom, rom.req_ptr(off + 4, "events")?)?;
    let connections = match rom.ptr(off + 0x0C)? {
        Some(c) => read_connections(rom, c)?,
        None => Vec::new(),
    };
    let flags = rom.u8(off + 0x1A)?;
    Ok(MapHeader {
        offset: off,
        layout,
        events,
        map_scripts: rom.ptr(off + 8)?,
        connections,
        music: rom.u16(off + 0x10)?,
        layout_id: rom.u16(off + 0x12)?,
        region_map_section: rom.u8(off + 0x14)?,
        cave: rom.u8(off + 0x15)?,
        weather: rom.u8(off + 0x16)?,
        map_type: rom.u8(off + 0x17)?,
        allow_cycling: flags & 1 != 0,
        allow_escaping: flags & 2 != 0,
        allow_running: flags & 4 != 0,
        show_map_name: flags >> 3 != 0,
        battle_type: rom.u8(off + 0x1B)?,
    })
}

pub fn read_layout(rom: &Rom, off: u32) -> Result<MapLayout> {
    let (width, height) = (rom.u32(off)?, rom.u32(off + 4)?);
    ensure!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "layout at {off:#x} has bad size {width}x{height}"
    );
    let border = rom.u16s(rom.req_ptr(off + 8, "border")?, 4)?;
    let blocks_off = rom.req_ptr(off + 0x0C, "block data")?;
    let blocks = rom.u16s(blocks_off, (width * height) as usize)?;
    Ok(MapLayout {
        offset: off,
        width: width as u16,
        height: height as u16,
        border,
        blocks,
        primary_tileset: rom.req_ptr(off + 0x10, "primary tileset")?,
        secondary_tileset: rom.req_ptr(off + 0x14, "secondary tileset")?,
    })
}

/// Reads `count` records of `size` bytes from the pointer at `ptr_off` (null is fine when `count` is 0).
fn records<T>(
    rom: &Rom,
    ptr_off: u32,
    count: u8,
    size: u32,
    what: &str,
    read: impl Fn(u32) -> Result<T>,
) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let base = rom.req_ptr(ptr_off, what)?;
    (0..count as u32).map(|i| read(base + i * size)).collect()
}

pub fn read_events(rom: &Rom, off: u32) -> Result<MapEvents> {
    let objects = records(rom, off + 4, rom.u8(off)?, 0x18, "object events", |o| {
        let ranges = rom.u8(o + 0x0A)?;
        Ok(ObjectEvent {
            local_id: rom.u8(o)?,
            graphics_id: rom.u8(o + 1)?,
            x: rom.i16(o + 4)?,
            y: rom.i16(o + 6)?,
            elevation: rom.u8(o + 8)?,
            movement_type: rom.u8(o + 9)?,
            movement_range_x: ranges & 0x0F,
            movement_range_y: ranges >> 4,
            trainer_type: rom.u16(o + 0x0C)?,
            trainer_range_berry_tree_id: rom.u16(o + 0x0E)?,
            script: rom.ptr(o + 0x10)?,
            flag_id: rom.u16(o + 0x14)?,
        })
    })?;
    let warps = records(rom, off + 8, rom.u8(off + 1)?, 8, "warps", |w| {
        Ok(Warp {
            x: rom.i16(w)?,
            y: rom.i16(w + 2)?,
            elevation: rom.u8(w + 4)?,
            warp_id: rom.u8(w + 5)?,
            dest: MapId {
                num: rom.u8(w + 6)?,
                group: rom.u8(w + 7)?,
            },
        })
    })?;
    let coords = records(
        rom,
        off + 0x0C,
        rom.u8(off + 2)?,
        0x10,
        "coord events",
        |c| {
            Ok(CoordEvent {
                x: rom.i16(c)?,
                y: rom.i16(c + 2)?,
                elevation: rom.u8(c + 4)?,
                trigger_var: rom.u16(c + 6)?,
                index: rom.u16(c + 8)?,
                script: rom.ptr(c + 0x0C)?,
            })
        },
    )?;
    let bgs = records(rom, off + 0x10, rom.u8(off + 3)?, 0x0C, "bg events", |b| {
        let kind_raw = rom.u8(b + 5)?;
        let kind = match kind_raw {
            BG_EVENT_HIDDEN_ITEM => BgEventKind::HiddenItem {
                item: rom.u16(b + 8)?,
                hidden_item_id: rom.u16(b + 0x0A)?,
            },
            BG_EVENT_SECRET_BASE => BgEventKind::SecretBase(rom.u32(b + 8)?),
            _ => BgEventKind::Script(rom.ptr(b + 8)?),
        };
        Ok(BgEvent {
            x: rom.u16(b)?,
            y: rom.u16(b + 2)?,
            elevation: rom.u8(b + 4)?,
            kind_raw,
            kind,
        })
    })?;
    Ok(MapEvents {
        objects,
        warps,
        coords,
        bgs,
    })
}

pub fn read_connections(rom: &Rom, off: u32) -> Result<Vec<Connection>> {
    let count = rom.i32(off)?;
    ensure!(
        (0..=16).contains(&count),
        "connections at {off:#x} have bad count {count}"
    );
    records(rom, off + 4, count as u8, 0x0C, "connection list", |c| {
        let raw = rom.u8(c)?;
        Ok(Connection {
            direction: Direction::from_raw(raw)
                .with_context(|| format!("bad connection direction {raw} at {c:#x}"))?,
            offset: rom.i32(c + 4)?,
            map: MapId {
                group: rom.u8(c + 8)?,
                num: rom.u8(c + 9)?,
            },
        })
    })
}

impl MapTable {
    /// Sorted, deduplicated metatiles and attributes pointers of every tileset the table uses.
    fn tileset_bounds(&self, rom: &Rom) -> Result<&[u32]> {
        if let Some(b) = self.tileset_bounds.0.get() {
            return Ok(b);
        }
        let mut bounds = Vec::new();
        for &header in self.groups.iter().flatten() {
            let layout = rom.req_ptr(header, "layout")?;
            for slot in [0x10, 0x14] {
                let other = read_tileset_struct(rom, rom.req_ptr(layout + slot, "tileset")?)?;
                bounds.push(other.metatiles);
                bounds.push(other.metatile_attributes);
            }
        }
        bounds.sort_unstable();
        bounds.dedup();
        Ok(self.tileset_bounds.0.get_or_init(|| bounds))
    }
}

/// Reads the `Tileset` at `off`.
///
/// `metatile_count` is the gap between its `metatiles` pointer and the next metatiles/attributes
/// pointer of any tileset used by `all`, capped at 512: the decomp (`src/data/tilesets/metatiles.h`)
/// emits each tileset's metatiles immediately followed by its attributes, so the gap is exactly the
/// metatile data. Primary tilesets can hold fewer than 512 (`building` has 8, `secret_base` 2).
///
/// Indexing: a block's metatile id below 512 selects primary metatile `id`; ids from 512 select
/// secondary metatile `id - 512`. Likewise, tile numbers inside metatile entries from 512 index the
/// secondary tileset's tiles. The secondary always starts at 512, whatever the primary's
/// `metatile_count` (or tile count) is.
pub fn read_tileset(rom: &Rom, off: u32, all: &MapTable) -> Result<TilesetRef> {
    let ts = read_tileset_struct(rom, off)?;
    let bounds = all.tileset_bounds(rom)?;
    let next = bounds[bounds.partition_point(|&b| b <= ts.metatiles)..]
        .first()
        .copied()
        .with_context(|| {
            format!(
                "tileset at {off:#x}: no metatiles/attributes pointer follows metatiles {:#x}",
                ts.metatiles
            )
        })?;
    let count = ((next - ts.metatiles) / METATILE_SIZE).min(METATILES_PER_TILESET as u32);
    ensure!(count > 0, "tileset at {off:#x} has no metatiles");
    rom.slice(ts.metatiles, (count * METATILE_SIZE) as usize)?;
    rom.slice(ts.metatile_attributes, (count * 2) as usize)?;
    Ok(TilesetRef {
        metatile_count: count as u16,
        ..ts
    })
}

fn read_tileset_struct(rom: &Rom, off: u32) -> Result<TilesetRef> {
    let flag = |o: u32| -> Result<bool> {
        match rom.u8(o)? {
            0 => Ok(false),
            1 => Ok(true),
            v => bail!("tileset at {off:#x}: byte {v} at {o:#x} is not a bool"),
        }
    };
    Ok(TilesetRef {
        offset: off,
        compressed: flag(off)?,
        secondary: flag(off + 1)?,
        tiles: rom.req_ptr(off + 4, "tileset tiles")?,
        palettes: rom.req_ptr(off + 8, "tileset palettes")?,
        metatiles: rom.req_ptr(off + 0x0C, "tileset metatiles")?,
        metatile_attributes: rom.req_ptr(off + 0x10, "tileset metatile attributes")?,
        metatile_count: 0,
    })
}

/// The `Tileset` header structs, which sit in the ROM in `src/data/tilesets/headers.h` order
/// ([`names::TILESETS`]) starting with `gTileset_General`, at the byte offsets
/// [`names::TILESET_HEADER_OFFSETS`] (0x18-byte structs, plus two 4-byte `Tileset` pointers
/// defined among them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TilesetHeaders {
    /// File offset of `gTileset_General`.
    pub offset: u32,
}

impl TilesetHeaders {
    /// Littleroot's primary tileset is `gTileset_General`, the first header. Every header must
    /// read as a `Tileset`.
    pub fn locate(rom: &Rom, table: &MapTable) -> Result<TilesetHeaders> {
        let general = table.header(rom, LITTLEROOT)?.layout.primary_tileset;
        for ((i, name), rel) in names::TILESETS.iter().zip(names::TILESET_HEADER_OFFSETS) {
            let off = general + rel;
            read_tileset_struct(rom, off)
                .with_context(|| format!("tileset header {i} (gTileset_{name}) at {off:#x}"))?;
        }
        Ok(TilesetHeaders { offset: general })
    }

    /// The decomp name (`General`, `Petalburg`) of the `Tileset` struct at file offset `off`.
    pub fn name(&self, off: u32) -> Result<&'static str> {
        off.checked_sub(self.offset)
            .and_then(|rel| names::TILESET_HEADER_OFFSETS.iter().position(|&r| r == rel))
            .and_then(|i| names::TILESETS.name(i as u32))
            .with_context(|| {
                format!(
                    "tileset at {off:#x} is not a header of the array at {:#x}",
                    self.offset
                )
            })
    }

    /// File offset of the `Tileset` struct named `name`.
    pub fn offset_of(&self, name: &str) -> Option<u32> {
        let i = names::TILESETS.value(name)? as usize;
        Some(self.offset + names::TILESET_HEADER_OFFSETS.get(i)?)
    }
}

/// `gMapLayouts`: `layouts[i]` is the file offset of the `MapLayout` of layout id `i + 1`
/// (`LAYOUT_*`, [`LAYOUT_NAMES`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTable {
    pub offset: u32,
    pub layouts: Vec<u32>,
}

/// Finds `gMapLayouts` from Littleroot's layout: the referenced pointer table holding it at index
/// `LITTLEROOT_LAYOUT_ID - 1` whose [`LAYOUT_NAMES`]`.len()` entries all look like layouts (sane
/// size, border, blocks and primary tileset). `LAYOUT_UNUSED_OUTDOOR_AREA` has no secondary
/// tileset, so [`read_layout`] (and [`LayoutTable::layout`]) reject it; no map uses it.
pub fn locate_layout_table(rom: &Rom, table: &MapTable) -> Result<LayoutTable> {
    let littleroot = table.header(rom, LITTLEROOT)?;
    ensure!(
        littleroot.layout_id == LITTLEROOT_LAYOUT_ID,
        "Littleroot's layout id is {}",
        littleroot.layout_id
    );
    let index = u32::from(LITTLEROOT_LAYOUT_ID) - 1;
    let mut found = Vec::new();
    for p in rom.pointers_to(littleroot.layout.offset) {
        let Some(start) = p.checked_sub(4 * index) else {
            continue;
        };
        if rom.pointers_to(start).is_empty() {
            continue;
        }
        let layouts: Option<Vec<u32>> = (0..LAYOUT_NAMES.len() as u32)
            .map(|i| {
                let l = rom.ptr(start + 4 * i).ok()??;
                layout_shape_ok(rom, l).then_some(l)
            })
            .collect();
        if let Some(layouts) = layouts {
            found.push(LayoutTable {
                offset: start,
                layouts,
            });
        }
    }
    match found.len() {
        1 => Ok(found.pop().unwrap()),
        n => bail!(
            "{n} gMapLayouts candidates for Littleroot layout {:#x}",
            littleroot.layout.offset
        ),
    }
}

/// A `MapLayout` with a sane size and non-null border, blocks and primary tileset.
fn layout_shape_ok(rom: &Rom, off: u32) -> bool {
    let dim_ok = |o: u32| rom.u32(o).is_ok_and(|d| (1..=MAX_DIM).contains(&d));
    dim_ok(off)
        && dim_ok(off + 4)
        && [8, 0x0C, 0x10]
            .iter()
            .all(|&f| matches!(rom.ptr(off + f), Ok(Some(_))))
}

impl LayoutTable {
    /// The layout of id `id` (`LAYOUT_*`; 0 is no layout).
    pub fn layout(&self, rom: &Rom, id: u16) -> Result<MapLayout> {
        let off = usize::from(id)
            .checked_sub(1)
            .and_then(|i| self.layouts.get(i))
            .with_context(|| format!("layout id {id} is not in gMapLayouts"))?;
        read_layout(rom, *off).with_context(|| format!("layout id {id}"))
    }

    /// The decomp name (`LittlerootTown`) of layout id `id`.
    pub fn name(id: u16) -> Result<&'static str> {
        usize::from(id)
            .checked_sub(1)
            .and_then(|i| LAYOUT_NAMES.get(i))
            .copied()
            .with_context(|| format!("layout id {id} has no name"))
    }
}

/// Decomp layout names (`data/layouts/layouts.json` `name` without `_Layout`) in `LAYOUT_*`
/// order: entry `i` is `gMapLayouts[i]`, layout id `i + 1` (`tests/maps.rs` checks the decomp).
pub const LAYOUT_NAMES: [&str; 441] = [
    "PetalburgCity",
    "SlateportCity",
    "MauvilleCity",
    "RustboroCity",
    "FortreeCity",
    "LilycoveCity",
    "MossdeepCity",
    "SootopolisCity",
    "EverGrandeCity",
    "LittlerootTown",
    "OldaleTown",
    "DewfordTown",
    "LavaridgeTown",
    "FallarborTown",
    "VerdanturfTown",
    "PacifidlogTown",
    "Route101",
    "Route102",
    "Route103",
    "Route104",
    "Route105",
    "Route106",
    "Route107",
    "Route108",
    "Route109",
    "Route110",
    "Route111",
    "Route112",
    "Route113",
    "Route114",
    "Route115",
    "Route116",
    "Route117",
    "Route118",
    "Route119",
    "Route120",
    "Route121",
    "Route122",
    "Route123",
    "Route124",
    "Route125",
    "Route126",
    "Route127",
    "Route128",
    "Route129",
    "Route130_MirageIsland",
    "Route131",
    "Route132",
    "Route133",
    "Route134",
    "Underwater_Route126",
    "Underwater_Route127",
    "Underwater_Route128",
    "LittlerootTown_BrendansHouse_1F",
    "LittlerootTown_BrendansHouse_2F",
    "LittlerootTown_MaysHouse_1F",
    "LittlerootTown_MaysHouse_2F",
    "LittlerootTown_ProfessorBirchsLab",
    "House1",
    "House2",
    "PokemonCenter_1F",
    "PokemonCenter_2F",
    "Mart",
    "House3",
    "DewfordTown_Gym",
    "DewfordTown_Hall",
    "House4",
    "LavaridgeTown_HerbShop",
    "LavaridgeTown_Gym_1F",
    "LavaridgeTown_Gym_B1F",
    "LavaridgeTown_PokemonCenter_1F",
    "FallarborTown_LeftoverRSContestLobby",
    "FallarborTown_LeftoverRSContestHall",
    "LilycoveCity_House2",
    "UnusedContestRoom1",
    "VerdanturfTown_WandasHouse",
    "PacifidlogTown_House1",
    "PacifidlogTown_House2",
    "PetalburgCity_Gym",
    "HouseWithBed",
    "SlateportCity_SternsShipyard_1F",
    "SlateportCity_SternsShipyard_2F",
    "UnusedContestRoom2",
    "UnusedContestRoom3",
    "SlateportCity_PokemonFanClub",
    "SlateportCity_OceanicMuseum_1F",
    "SlateportCity_OceanicMuseum_2F",
    "Harbor",
    "MauvilleCity_Gym",
    "MauvilleCity_BikeShop",
    "MauvilleCity_GameCorner",
    "RustboroCity_DevonCorp_1F",
    "RustboroCity_DevonCorp_2F",
    "RustboroCity_Gym",
    "RustboroCity_PokemonSchool",
    "RustboroCity_House",
    "RustboroCity_House1",
    "RustboroCity_CuttersHouse",
    "FortreeCity_House1",
    "FortreeCity_Gym",
    "FortreeCity_House2",
    "Route104_MrBrineysHouse",
    "LilycoveCity_LilycoveMuseum_1F",
    "LilycoveCity_LilycoveMuseum_2F",
    "LilycoveCity_ContestLobby",
    "LilycoveCity_ContestHall",
    "LilycoveCity_PokemonTrainerFanClub",
    "MossdeepCity_Gym",
    "SootopolisCity_Gym_1F",
    "SootopolisCity_Gym_B1F",
    "EverGrandeCity_SidneysRoom",
    "EverGrandeCity_PhoebesRoom",
    "EverGrandeCity_GlaciasRoom",
    "EverGrandeCity_DrakesRoom",
    "EverGrandeCity_ChampionsRoom",
    "EverGrandeCity_ShortHall",
    "Route104_PrettyPetalFlowerShop",
    "CableCarStation",
    "Route114_FossilManiacsHouse",
    "Route114_FossilManiacsTunnel",
    "Route114_LanettesHouse",
    "Route116_TunnelersRestHouse",
    "Route117_PokemonDayCare",
    "Route121_SafariZoneEntrance",
    "MeteorFalls_1F_1R",
    "MeteorFalls_1F_2R",
    "MeteorFalls_B1F_1R",
    "MeteorFalls_B1F_2R",
    "RusturfTunnel",
    "Underwater_SootopolisCity",
    "DesertRuins",
    "GraniteCave_1F",
    "GraniteCave_B1F",
    "GraniteCave_B2F",
    "PetalburgWoods",
    "MtChimney",
    "MtPyre_1F",
    "MtPyre_2F",
    "MtPyre_3F",
    "MtPyre_4F",
    "MtPyre_5F",
    "MtPyre_6F",
    "AquaHideout_1F",
    "AquaHideout_B1F",
    "AquaHideout_B2F",
    "Underwater_SeafloorCavern",
    "SeafloorCavern_Entrance",
    "SeafloorCavern_Room1",
    "SeafloorCavern_Room2",
    "SeafloorCavern_Room3",
    "SeafloorCavern_Room4",
    "SeafloorCavern_Room5",
    "SeafloorCavern_Room6",
    "SeafloorCavern_Room7",
    "SeafloorCavern_Room8",
    "SeafloorCavern_Room9",
    "CaveOfOrigin_Entrance",
    "CaveOfOrigin_1F",
    "CaveOfOrigin_UnusedRubySapphireMap1",
    "CaveOfOrigin_UnusedRubySapphireMap2",
    "CaveOfOrigin_UnusedRubySapphireMap3",
    "CaveOfOrigin_B1F",
    "VictoryRoad_1F",
    "ShoalCave_LowTideEntranceRoom",
    "ShoalCave_LowTideInnerRoom",
    "ShoalCave_LowTideStairsRoom",
    "ShoalCave_LowTideLowerRoom",
    "ShoalCave_HighTideEntranceRoom",
    "ShoalCave_HighTideInnerRoom",
    "UnusedCave1",
    "UnusedCave2",
    "UnusedCave3",
    "UnusedCave4",
    "UnusedCave5",
    "UnusedCave6",
    "UnusedCave7",
    "UnusedCave8",
    "UnusedCave9",
    "UnusedCave10",
    "UnusedCave11",
    "UnusedCave12",
    "UnusedCave13",
    "UnusedCave14",
    "NewMauville_Entrance",
    "NewMauville_Inside",
    "AbandonedShip_Deck",
    "AbandonedShip_Corridors_1F",
    "AbandonedShip_Rooms_1F",
    "AbandonedShip_Corridors_B1F",
    "AbandonedShip_Rooms_B1F",
    "AbandonedShip_Rooms2_B1F",
    "AbandonedShip_Underwater1",
    "AbandonedShip_Room_B1F",
    "AbandonedShip_Rooms2_1F",
    "AbandonedShip_CaptainsOffice",
    "AbandonedShip_Underwater2",
    "SecretBase_RedCave1",
    "SecretBase_BrownCave1",
    "SecretBase_BlueCave1",
    "SecretBase_YellowCave1",
    "SecretBase_Tree1",
    "SecretBase_Shrub1",
    "SecretBase_RedCave2",
    "SecretBase_BrownCave2",
    "SecretBase_BlueCave2",
    "SecretBase_YellowCave2",
    "SecretBase_Tree2",
    "SecretBase_Shrub2",
    "SecretBase_RedCave3",
    "SecretBase_BrownCave3",
    "SecretBase_BlueCave3",
    "SecretBase_YellowCave3",
    "SecretBase_Tree3",
    "SecretBase_Shrub3",
    "SecretBase_RedCave4",
    "SecretBase_BrownCave4",
    "SecretBase_BlueCave4",
    "SecretBase_YellowCave4",
    "SecretBase_Tree4",
    "SecretBase_Shrub4",
    "BattleColosseum_2P",
    "TradeCenter",
    "RecordCorner",
    "BattleColosseum_4P",
    "ContestHall",
    "UnusedContestHall1",
    "UnusedContestHall2",
    "UnusedContestHall3",
    "UnusedContestHall4",
    "UnusedContestHall5",
    "UnusedContestHall6",
    "ContestHallBeauty",
    "ContestHallTough",
    "ContestHallCool",
    "ContestHallSmart",
    "ContestHallCute",
    "InsideOfTruck",
    "SafariZone_Northwest",
    "SafariZone_North",
    "SafariZone_Southwest",
    "SafariZone_South",
    "UnusedOutdoorArea",
    "Route109_SeashoreHouse",
    "Route110_TrickHouseEntrance",
    "Route110_TrickHouseEnd",
    "Route110_TrickHouseCorridor",
    "Route110_TrickHousePuzzle1",
    "Route110_TrickHousePuzzle2",
    "Route110_TrickHousePuzzle3",
    "Route110_TrickHousePuzzle4",
    "Route110_TrickHousePuzzle5",
    "Route110_TrickHousePuzzle6",
    "Route110_TrickHousePuzzle7",
    "Route110_TrickHousePuzzle8",
    "FortreeCity_DecorationShop",
    "Route110_SeasideCyclingRoadEntrance",
    "LilycoveCity_DepartmentStore_1F",
    "LilycoveCity_DepartmentStore_2F",
    "LilycoveCity_DepartmentStore_3F",
    "LilycoveCity_DepartmentStore_4F",
    "LilycoveCity_DepartmentStore_5F",
    "LilycoveCity_DepartmentStoreRooftop",
    "Route130",
    "BattleFrontier_BattleTowerLobby",
    "BattleFrontier_OutsideWest",
    "BattleElevator",
    "BattleFrontier_BattleTowerCorridor",
    "BattleFrontier_BattleTowerBattleRoom",
    "RustboroCity_DevonCorp_3F",
    "EverGrandeCity_PokemonLeague_1F",
    "Route119_WeatherInstitute_1F",
    "Route119_WeatherInstitute_2F",
    "LilycoveCity_DepartmentStoreElevator",
    "Underwater_Route124",
    "MossdeepCity_SpaceCenter_1F",
    "MossdeepCity_SpaceCenter_2F",
    "SSTidalCorridor",
    "SSTidalLowerDeck",
    "SSTidalRooms",
    "IslandCave",
    "AncientTomb",
    "Underwater_Route134",
    "Underwater_SealedChamber",
    "SealedChamber_OuterRoom",
    "VictoryRoad_B1F",
    "VictoryRoad_B2F",
    "Route104_Prototype",
    "GraniteCave_StevensRoom",
    "AbandonedShip_HiddenFloorCorridors",
    "SouthernIsland_Exterior",
    "SouthernIsland_Interior",
    "JaggedPass",
    "FieryPath",
    "RustboroCity_Flat2_1F",
    "RustboroCity_Flat2_2F",
    "RustboroCity_Flat2_3F",
    "SootopolisCity_LotadAndSeedotHouse",
    "EverGrandeCity_HallOfFame",
    "LilycoveCity_CoveLilyMotel_1F",
    "LilycoveCity_CoveLilyMotel_2F",
    "Route124_DivingTreasureHuntersHouse",
    "MtPyre_Exterior",
    "MtPyre_Summit",
    "SealedChamber_InnerRoom",
    "MossdeepCity_GameCorner_1F",
    "MossdeepCity_GameCorner_B1F",
    "SootopolisCity_House1",
    "SootopolisCity_House2",
    "SootopolisCity_House3",
    "AbandonedShip_HiddenFloorRooms",
    "ScorchedSlab",
    "CaveOfOrigin_Unused_B4F_Lava",
    "RustboroCity_Flat1_1F",
    "RustboroCity_Flat1_2F",
    "EverGrandeCity_Hall4",
    "AquaHideout_UnusedRubyMap1",
    "AquaHideout_UnusedRubyMap2",
    "AquaHideout_UnusedRubyMap3",
    "Route131_SkyPillar",
    "SkyPillar_Entrance",
    "SkyPillar_Outside",
    "SkyPillar_1F",
    "SkyPillar_2F",
    "SkyPillar_3F",
    "SkyPillar_4F",
    "SeafloorCavern_Room9_Lava",
    "MossdeepCity_StevensHouse",
    "ShoalCave_LowTideIceRoom",
    "SafariZone_RestHouse",
    "SkyPillar_5F",
    "SkyPillar_Top",
    "BattleFrontier_BattleDomeLobby",
    "BattleFrontier_BattleDomeCorridor",
    "BattleFrontier_BattleDomePreBattleRoom",
    "BattleFrontier_BattleDomeBattleRoom",
    "MagmaHideout_1F",
    "MagmaHideout_2F_1R",
    "MagmaHideout_2F_2R",
    "MagmaHideout_3F_1R",
    "MagmaHideout_3F_2R",
    "MagmaHideout_4F",
    "BattleFrontier_BattlePalaceLobby",
    "BattleFrontier_BattlePalaceCorridor",
    "BattleFrontier_BattlePalaceBattleRoom",
    "BattleFrontier_OutsideEast",
    "BattleFrontier_BattleFactoryLobby",
    "BattleFrontier_BattleFactoryPreBattleRoom",
    "BattleFrontier_BattleFactoryBattleRoom",
    "BattleFrontier_BattlePikeLobby",
    "BattleFrontier_BattlePikeCorridor",
    "BattleFrontier_BattlePikeThreePathRoom",
    "BattleFrontier_BattlePikeRoomNormal",
    "BattleFrontier_BattlePikeRoomFinal",
    "BattleFrontier_BattleArenaLobby",
    "BattleFrontier_BattleArenaCorridor",
    "BattleFrontier_BattleArenaBattleRoom",
    "SootopolisCity_LegendsBattle",
    "BattleFrontier_BattlePikeRoomWildMons",
    "BattleFrontier_BattlePikeRoomUnused",
    "BattleFrontier_BattlePyramidLobby",
    "BattleFrontier_BattlePyramidFloor",
    "BattlePyramidSquare01",
    "BattlePyramidSquare02",
    "BattlePyramidSquare03",
    "BattlePyramidSquare04",
    "BattlePyramidSquare05",
    "BattlePyramidSquare06",
    "BattlePyramidSquare07",
    "BattlePyramidSquare08",
    "BattlePyramidSquare09",
    "BattlePyramidSquare10",
    "BattlePyramidSquare11",
    "BattlePyramidSquare12",
    "BattlePyramidSquare13",
    "BattlePyramidSquare14",
    "BattlePyramidSquare15",
    "BattlePyramidSquare16",
    "BattleFrontier_BattlePyramidTop",
    "MagmaHideout_3F_3R",
    "MagmaHideout_2F_3R",
    "MirageTower_1F",
    "MirageTower_2F",
    "MirageTower_3F",
    "BattleTentLobby",
    "BattleTentCorridor",
    "BattleTentBattleRoom",
    "VerdanturfTown_BattleTentBattleRoom",
    "MirageTower_4F",
    "DesertUnderpass",
    "BattleFrontier_BattleTowerMultiPartnerRoom",
    "BattleFrontier_BattleTowerMultiCorridor",
    "Route111_NoMirageTower",
    "UnionRoom",
    "SafariZone_Northeast",
    "SafariZone_Southeast",
    "BattleFrontier_RankingHall",
    "BattleFrontier_Lounge1",
    "BattleFrontier_ExchangeServiceCorner",
    "BattleFrontier_ReceptionGate",
    "ArtisanCave_B1F",
    "ArtisanCave_1F",
    "FarawayIsland_Entrance",
    "FarawayIsland_Interior",
    "BirthIsland_Exterior",
    "IslandHarbor",
    "Underwater_MarineCave",
    "MarineCave_Entrance",
    "TerraCave_Entrance",
    "TerraCave_End",
    "Underwater_Route105",
    "Underwater_Route125",
    "Underwater_Route129",
    "MarineCave_End",
    "TrainerHill_Entrance",
    "TrainerHill_1F",
    "TrainerHill_2F",
    "TrainerHill_3F",
    "TrainerHill_4F",
    "TrainerHill_Roof",
    "AlteringCave",
    "NavelRock_Exterior",
    "NavelRock_Entrance",
    "NavelRock_Top",
    "NavelRock_Bottom",
    "NavelRock_LadderRoom1",
    "NavelRock_LadderRoom2",
    "NavelRock_B1F",
    "NavelRock_Fork",
    "BattleFrontier_Lounge2",
    "BattleFrontier_ScottsHouse",
    "MeteorFalls_StevensCave",
    "LittlerootTown_ProfessorBirchsLabWithTable",
    "SkyPillar_1F_Clean",
    "SkyPillar_2F_Clean",
    "SkyPillar_3F_Clean",
    "SkyPillar_4F_Clean",
    "SkyPillar_5F_Clean",
    "SkyPillar_Top_Clean",
    "SootopolisCity_MysteryEventsHouse_1F",
    "SootopolisCity_MysteryEventsHouse_B1F",
    "SootopolisCity_MysteryEventsHouse_1F_StairsUnblocked",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROM_BASE;

    /// A minimal BPEE-coded ROM image with `data` placed at 0x200.
    fn rom_with(data: &[u8]) -> Rom {
        let mut b = vec![0u8; 0x200];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b.extend_from_slice(data);
        Rom::from_bytes(b).unwrap()
    }

    fn put(buf: &mut [u8], at: usize, v: &[u8]) {
        buf[at..at + v.len()].copy_from_slice(v);
    }

    #[test]
    fn events_parse_all_record_kinds() {
        let mut d = vec![0u8; 0x100];
        let p = |o: u32| (ROM_BASE + 0x200 + o).to_le_bytes();
        // MapEvents at 0: 1 object, 1 warp, 1 coord, 2 bgs.
        put(&mut d, 0, &[1, 1, 1, 2]);
        put(&mut d, 4, &p(0x20));
        put(&mut d, 8, &p(0x40));
        put(&mut d, 12, &p(0x50));
        put(&mut d, 16, &p(0x60));
        // Object: local 3, gfx 7, x -1, y 5, elev 3, move 2, ranges x=1 y=2.
        put(&mut d, 0x20, &[3, 7, 0, 0, 0xFF, 0xFF, 5, 0, 3, 2, 0x21, 0]);
        put(&mut d, 0x30, &p(0x90));
        put(&mut d, 0x34, &0x1234u16.to_le_bytes());
        // Warp: x 4, y 6, elev 0, id 1, num 9, group 0.
        put(&mut d, 0x40, &[4, 0, 6, 0, 0, 1, 9, 0]);
        // Coord: trigger 0x4050, index 1, null script.
        put(&mut d, 0x50, &[1, 0, 2, 0, 3, 0, 0x50, 0x40, 1, 0]);
        // Bg 0: hidden item 0x10 / id 0x20. Bg 1: script.
        put(&mut d, 0x60, &[8, 0, 9, 0, 0, 7, 0, 0, 0x10, 0, 0x20, 0]);
        put(&mut d, 0x6C, &[1, 0, 1, 0, 0, 0, 0, 0]);
        put(&mut d, 0x74, &p(0x90));
        let rom = rom_with(&d);
        let ev = read_events(&rom, 0x200).unwrap();
        let o = &ev.objects[0];
        assert_eq!((o.local_id, o.graphics_id, o.x, o.y), (3, 7, -1, 5));
        assert_eq!((o.movement_range_x, o.movement_range_y), (1, 2));
        assert_eq!((o.script, o.flag_id), (Some(0x290), 0x1234));
        assert_eq!(ev.warps[0].dest, MapId { group: 0, num: 9 });
        assert_eq!(
            (ev.coords[0].trigger_var, ev.coords[0].script),
            (0x4050, None)
        );
        assert_eq!(
            ev.bgs[0].kind,
            BgEventKind::HiddenItem {
                item: 0x10,
                hidden_item_id: 0x20
            }
        );
        assert_eq!(ev.bgs[1].kind, BgEventKind::Script(Some(0x290)));
    }

    #[test]
    fn connections_reject_bad_direction() {
        let mut d = vec![0u8; 0x20];
        put(&mut d, 0, &1i32.to_le_bytes());
        put(&mut d, 4, &(ROM_BASE + 0x208).to_le_bytes());
        put(&mut d, 8, &[2, 0, 0, 0, 0xFB, 0xFF, 0xFF, 0xFF, 0, 16]);
        let rom = rom_with(&d);
        let c = read_connections(&rom, 0x200).unwrap();
        assert_eq!(
            c,
            vec![Connection {
                direction: Direction::North,
                offset: -5,
                map: ROUTE101
            }]
        );
        let mut d2 = d.clone();
        d2[8] = 9;
        assert!(read_connections(&rom_with(&d2), 0x200).is_err());
    }

    #[test]
    fn bad_layout_pointer_is_an_error_with_offset() {
        let mut d = vec![0u8; 0x20];
        put(&mut d, 0, &0xDEAD_BEEFu32.to_le_bytes());
        let err = read_header(&rom_with(&d), 0x200).unwrap_err();
        assert!(format!("{err:#}").contains("0x200"), "{err:#}");
    }
}
