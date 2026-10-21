//! Map headers, layouts, events, connections and tileset structs (layouts from `include/global.fieldmap.h`).
//!
//! `gMapGroups` is found by anchoring on the first 64 bytes of Littleroot's `map.bin`, then following
//! pointers back: map.bin -> `MapLayout` -> `MapHeader` -> group-0 table -> `gMapGroups`.

use crate::Rom;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct MapId {
    pub group: u8,
    pub num: u8,
}

pub const LITTLEROOT: MapId = MapId { group: 0, num: 9 };
pub const OLDALE: MapId = MapId { group: 0, num: 10 };
pub const ROUTE101: MapId = MapId { group: 0, num: 16 };

/// Maps per group, from `data/maps/map_groups.json` (`group_order`).
const GROUP_SIZES: [u8; 34] = [
    57, 5, 5, 6, 7, 8, 9, 7, 7, 14, 8, 17, 10, 23, 13, 15, 15, 2, 2, 2, 3, 1, 1, 1, 108, 61, 89, 2,
    1, 13, 1, 1, 3, 1,
];

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

const MAX_METATILES: u32 = 512;
const METATILE_SIZE: u32 = 16;
/// Largest map dimension accepted before reading block data (Emerald's biggest is well under this).
const MAX_DIM: u32 = 0x400;

/// `gMapGroups`: `offset` is the table itself, `groups[g][n]` the file offset of each `MapHeader`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapTable {
    pub offset: u32,
    pub groups: Vec<Vec<u32>>,
    /// Sorted metatiles/attributes pointers of every tileset the table uses, filled by the first
    /// `read_tileset`. Not serialized and ignored by `PartialEq`.
    #[serde(skip)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Warp {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub warp_id: u8,
    pub dest: MapId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CoordEvent {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub trigger_var: u16,
    pub index: u16,
    pub script: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BgEventKind {
    Script(Option<u32>),
    HiddenItem { item: u16, hidden_item_id: u16 },
    SecretBase(u32),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BgEvent {
    pub x: u16,
    pub y: u16,
    pub elevation: u8,
    pub kind_raw: u8,
    pub kind: BgEventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapEvents {
    pub objects: Vec<ObjectEvent>,
    pub warps: Vec<Warp>,
    pub coords: Vec<CoordEvent>,
    pub bgs: Vec<BgEvent>,
}

/// Decomp `CONNECTION_*` order (1 = south … 6 = emerge).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Connection {
    pub direction: Direction,
    pub offset: i32,
    pub map: MapId,
}

const BG_EVENT_HIDDEN_ITEM: u8 = 7;
const BG_EVENT_SECRET_BASE: u8 = 8;

fn req_ptr(rom: &Rom, off: u32, what: &str) -> Result<u32> {
    rom.ptr(off)?
        .with_context(|| format!("null {what} pointer at {off:#x}"))
}

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
    let mut groups = Vec::with_capacity(GROUP_SIZES.len());
    for (g, &size) in GROUP_SIZES.iter().enumerate() {
        let list = req_ptr(rom, table + 4 * g as u32, "map group")?;
        let mut headers = Vec::with_capacity(size as usize);
        for n in 0..size as u32 {
            headers.push(req_ptr(rom, list + 4 * n, "map header")?);
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
    let layout = read_layout(rom, req_ptr(rom, off, "layout")?)?;
    let events = read_events(rom, req_ptr(rom, off + 4, "events")?)?;
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

fn u16s(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

pub fn read_layout(rom: &Rom, off: u32) -> Result<MapLayout> {
    let (width, height) = (rom.u32(off)?, rom.u32(off + 4)?);
    ensure!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "layout at {off:#x} has bad size {width}x{height}"
    );
    let border = u16s(rom.slice(req_ptr(rom, off + 8, "border")?, 2 * 4)?);
    let blocks_off = req_ptr(rom, off + 0x0C, "block data")?;
    let blocks = u16s(rom.slice(blocks_off, 2 * (width * height) as usize)?);
    Ok(MapLayout {
        offset: off,
        width: width as u16,
        height: height as u16,
        border,
        blocks,
        primary_tileset: req_ptr(rom, off + 0x10, "primary tileset")?,
        secondary_tileset: req_ptr(rom, off + 0x14, "secondary tileset")?,
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
    let base = req_ptr(rom, ptr_off, what)?;
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
            let layout = req_ptr(rom, header, "layout")?;
            for slot in [0x10, 0x14] {
                let other = read_tileset_struct(rom, req_ptr(rom, layout + slot, "tileset")?)?;
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
    let count = ((next - ts.metatiles) / METATILE_SIZE).min(MAX_METATILES);
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
        tiles: req_ptr(rom, off + 4, "tileset tiles")?,
        palettes: req_ptr(rom, off + 8, "tileset palettes")?,
        metatiles: req_ptr(rom, off + 0x0C, "tileset metatiles")?,
        metatile_attributes: req_ptr(rom, off + 0x10, "tileset metatile attributes")?,
        metatile_count: 0,
    })
}

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
    fn group_sizes_sum_to_518() {
        assert_eq!(GROUP_SIZES.iter().map(|&s| s as u32).sum::<u32>(), 518);
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
