//! Locating the Milestone 9 menu graphics in the ROM: mon icons (`gMonIconTable`,
//! `gMonIconPaletteIndices`, `gMonIconPalettes`), item icons (`gItemIconTable`) and the bag
//! sprite (`gBagMaleSpriteSheet`). `export/ui.rs` writes them.
//!
//! The tables are found by shape, not by a fixed offset: the mon-icon pointer table directly
//! before Bulbasaur's palette-index bytes, three sprite palettes tagged `POKE_ICON_BASE_PAL_TAG`,
//! a run of LZ item-icon pairs, and the two 0x3000-byte bag sheets tagged 100.

use crate::names;
use crate::rom::{ROM_BASE, Rom};

/// Species in `gMonIconTable`, `SPECIES_NONE` through `SPECIES_UNOWN_QMARK`.
pub const MON_ICON_COUNT: usize = 0x1B8;
/// `gItemIconTable[ITEMS_COUNT + 1]`.
pub const ITEM_ICON_COUNT: usize = 378;
/// One mon-icon frame is 32×32 4bpp; Emerald stores two.
pub const MON_ICON_BYTES: usize = 1024;
/// An item icon decompresses to 24×24 and is expanded into a 32×32 sheet.
pub const ITEM_ICON_RAW: usize = 0x120;
pub const ITEM_ICON_BYTES: usize = 512;
/// `TAG_BAG_GFX`.
const BAG_TAG: u16 = 100;
const BAG_SHEET: usize = 0x3000;
/// `POKE_ICON_BASE_PAL_TAG`.
const MON_PAL_TAG: u16 = 56000;

/// First bytes of `gMonIconPaletteIndices` (NONE, Bulbasaur, … Beedrill). Language-independent.
const PAL_INDEX_PREFIX: [u8; 16] = [0, 1, 1, 1, 0, 0, 0, 0, 2, 2, 1, 1, 0, 1, 2, 2];

/// Our bag pockets, in [`engine` order](POCKETS): Items, Poké Balls, TMs, Berries, Key Items.
/// Byte offset of that frame in the 64×64 sheet (`ANIMCMD_FRAME` tile × 32).
const BAG_FRAME_AT: [usize; 5] = [64 * 32, 192 * 32, 256 * 32, 320 * 32, 128 * 32];
const BAG_FRAME_BYTES: usize = 64 * 32;

pub struct MonIcons {
    pub table: u32,
    pub indices: u32,
    pub palettes_at: u32,
    /// `(tiles, palette index)` per species.
    pub icons: Vec<(Vec<u8>, u8)>,
    pub palettes: Vec<[u16; 16]>,
}

pub struct ItemIcons {
    pub table: u32,
    /// `(expanded 32×32 tiles, palette)` per item id.
    pub icons: Vec<(Vec<u8>, [u16; 16])>,
}

pub struct BagSprites {
    pub sheet: u32,
    pub palette_at: u32,
    pub palette: [u16; 16],
    /// One 64×64 frame per pocket, in `BAG_FRAME_AT` order.
    pub frames: Vec<Vec<u8>>,
}

fn fast_ptr(rom: &Rom, off: u32) -> Option<u32> {
    let b = rom.bytes();
    let i = off as usize;
    let w = u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?);
    if w < ROM_BASE {
        return None;
    }
    let p = w - ROM_BASE;
    ((p as usize) + 4 <= b.len()).then_some(p)
}

fn lz_size(rom: &Rom, off: u32) -> Option<usize> {
    let b = rom.bytes();
    let i = off as usize;
    if b.get(i).copied()? != 0x10 {
        return None;
    }
    let size = u32::from_le_bytes([b[i + 1], b[i + 2], b[i + 3], 0]) as usize;
    (size > 0 && size <= 0x8000).then_some(size)
}

fn read_pal(bytes: &[u8]) -> Option<[u16; 16]> {
    if bytes.len() < 32 {
        return None;
    }
    let mut p = [0u16; 16];
    for (i, slot) in p.iter_mut().enumerate() {
        let s = i * 2;
        *slot = u16::from_le_bytes([bytes[s], bytes[s + 1]]);
    }
    Some(p)
}

/// `gMonIconTable` sits immediately before `gMonIconPaletteIndices`. The index bytes start with
/// Bulbasaur's palette pattern; each pointer names a raw 1024-byte icon (two 32×32 frames).
/// Neighbours are not packed at +0x400: the linker leaves padding between the objects.
pub fn locate_mon_icons(rom: &Rom) -> Option<MonIcons> {
    let b = rom.bytes();
    let mut i = 0usize;
    while i + MON_ICON_COUNT <= b.len() {
        if b[i..i + PAL_INDEX_PREFIX.len()] != PAL_INDEX_PREFIX
            || !b[i..i + MON_ICON_COUNT].iter().all(|&v| v <= 2)
        {
            i += 1;
            continue;
        }
        let index_off = i as u32;
        let bytes = (MON_ICON_COUNT * 4) as u32;
        if index_off < bytes {
            i += 1;
            continue;
        }
        let table = index_off - bytes;
        if let Some(icons) = read_mon_table(rom, table, &b[i..i + MON_ICON_COUNT]) {
            let (palettes_at, palettes) = find_mon_palettes(rom)?;
            return Some(MonIcons {
                table,
                indices: index_off,
                palettes_at,
                icons,
                palettes,
            });
        }
        i += 1;
    }
    None
}

fn read_mon_table(rom: &Rom, table: u32, indices: &[u8]) -> Option<Vec<(Vec<u8>, u8)>> {
    let p0 = fast_ptr(rom, table)?;
    let p1 = fast_ptr(rom, table + 4)?;
    if p0 != p1 || p0 % 4 != 0 {
        return None;
    }
    let mut icons = Vec::with_capacity(MON_ICON_COUNT);
    for i in 0..MON_ICON_COUNT {
        let p = fast_ptr(rom, table + (i * 4) as u32)?;
        if p % 4 != 0 || (p as usize) + MON_ICON_BYTES > rom.bytes().len() {
            return None;
        }
        let tiles = rom.bytes()[p as usize..p as usize + MON_ICON_BYTES].to_vec();
        icons.push((tiles, indices[i]));
    }
    Some(icons)
}

fn find_mon_palettes(rom: &Rom) -> Option<(u32, Vec<[u16; 16]>)> {
    let len = rom.bytes().len() as u32;
    let mut off = 0u32;
    while off + 24 <= len {
        if let Some(pals) = mon_pal_at(rom, off) {
            return Some((off, pals));
        }
        off += 4;
    }
    None
}

fn mon_pal_at(rom: &Rom, off: u32) -> Option<Vec<[u16; 16]>> {
    let mut pals = Vec::with_capacity(3);
    let mut prev: Option<u32> = None;
    for i in 0..3u32 {
        let at = off + i * 8;
        let data = fast_ptr(rom, at)?;
        let tag = rom.u16(at + 4).ok()?;
        let pad = rom.u16(at + 6).ok()?;
        if tag != MON_PAL_TAG + i as u16 || pad != 0 {
            return None;
        }
        if let Some(p) = prev
            && data != p + 32
        {
            return None;
        }
        prev = Some(data);
        let bytes = rom.slice(data, 32).ok()?;
        pals.push(read_pal(bytes)?);
    }
    Some(pals)
}

fn item_pair(rom: &Rom, off: u32) -> bool {
    let Some(pic) = fast_ptr(rom, off) else {
        return false;
    };
    let Some(pal) = fast_ptr(rom, off + 4) else {
        return false;
    };
    pic % 4 == 0
        && pal % 4 == 0
        && lz_size(rom, pic) == Some(ITEM_ICON_RAW)
        && matches!(lz_size(rom, pal), Some(n) if (32..=64).contains(&n))
}

pub fn locate_item_icons(rom: &Rom) -> Option<ItemIcons> {
    let len = rom.bytes().len() as u32;
    let width = (ITEM_ICON_COUNT * 8) as u32;
    let mut off = 0u32;
    let mut table = None;
    while off + width <= len {
        if item_pair(rom, off) {
            let mut n = 1usize;
            while n < ITEM_ICON_COUNT && item_pair(rom, off + (n * 8) as u32) {
                n += 1;
            }
            if n == ITEM_ICON_COUNT {
                table = Some(off);
                break;
            }
        }
        off += 4;
    }
    let table = table?;
    let mut icons = Vec::with_capacity(ITEM_ICON_COUNT);
    for i in 0..ITEM_ICON_COUNT {
        let at = table + (i * 8) as u32;
        let pic = fast_ptr(rom, at)?;
        let pal = fast_ptr(rom, at + 4)?;
        let raw = rom.lz77_at(pic).ok()?;
        if raw.len() < ITEM_ICON_RAW {
            return None;
        }
        let pal_bytes = rom.lz77_at(pal).ok()?;
        icons.push((
            expand_item_icon(&raw[..ITEM_ICON_RAW]),
            read_pal(&pal_bytes)?,
        ));
    }
    Some(ItemIcons { table, icons })
}

/// `CopyItemIconPicTo4x4Buffer`: three rows of 96 bytes into a 32×32 (four rows of 128).
fn expand_item_icon(src: &[u8]) -> Vec<u8> {
    let mut dest = vec![0u8; ITEM_ICON_BYTES];
    for row in 0..3 {
        let s = row * 96;
        let d = row * 128;
        dest[d..d + 96].copy_from_slice(&src[s..s + 96]);
    }
    dest
}

fn sheet(rom: &Rom, off: u32) -> Option<(u32, u16, u16)> {
    let data = fast_ptr(rom, off)?;
    Some((data, rom.u16(off + 4).ok()?, rom.u16(off + 6).ok()?))
}

pub fn locate_bag_sprites(rom: &Rom) -> Option<BagSprites> {
    let len = rom.bytes().len() as u32;
    let mut hits = Vec::new();
    let mut off = 0u32;
    while off + 8 <= len {
        if let Some((data, size, tag)) = sheet(rom, off)
            && size as usize == BAG_SHEET
            && tag == BAG_TAG
        {
            hits.push((off, data));
        }
        off += 4;
    }
    // Male then female: the lower tile address is the male sheet (`gBagMaleTiles` is first).
    hits.sort_by_key(|(_, data)| *data);
    let (sheet_off, data) = *hits.first()?;
    let raw = rom.lz77_at(data).ok()?;
    if raw.len() < BAG_SHEET {
        return None;
    }
    let (palette_at, palette) = bag_palette(rom, sheet_off)?;
    let frames = BAG_FRAME_AT
        .iter()
        .map(|&at| raw[at..at + BAG_FRAME_BYTES].to_vec())
        .collect();
    Some(BagSprites {
        sheet: sheet_off,
        palette_at,
        palette,
        frames,
    })
}

fn bag_palette(rom: &Rom, sheet_off: u32) -> Option<(u32, [u16; 16])> {
    let start = sheet_off.saturating_sub(0x100);
    let end = (sheet_off + 0x100).min(rom.bytes().len() as u32);
    let mut off = start;
    while off + 8 <= end {
        if let Some(data) = fast_ptr(rom, off)
            && rom.u16(off + 4).ok() == Some(BAG_TAG)
            && rom.u16(off + 6).ok() == Some(0)
            && let Ok(raw) = rom.lz77_at(data)
            && let Some(pal) = read_pal(&raw)
        {
            return Some((off, pal));
        }
        off += 4;
    }
    None
}

/// Species name for `id`, when the decomp table has one.
pub fn species_name(id: u32) -> Option<&'static str> {
    names::SPECIES.name(id)
}

pub fn item_name(id: u32) -> Option<&'static str> {
    names::ITEMS.name(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_menu_icons_are_non_empty() {
        let rom = crate::fixtures::us();
        let mons = locate_mon_icons(&rom).expect("gMonIconTable");
        let bulb = &mons.icons[1].0;
        assert_eq!(bulb.len(), MON_ICON_BYTES);
        assert!(bulb.iter().any(|b| *b != 0), "bulbasaur icon is blank");
        assert_eq!(mons.icons[0].0, mons.icons[1].0, "NONE shares Bulbasaur");
        assert_eq!(mons.palettes.len(), 3);
        assert!(mons.palettes.iter().any(|p| p.iter().any(|c| *c != 0)));

        let items = locate_item_icons(&rom).expect("gItemIconTable");
        let ball = &items.icons[4].0;
        assert_eq!(ball.len(), ITEM_ICON_BYTES, "poke ball expands to 32x32");
        assert!(ball.iter().any(|b| *b != 0), "poke ball icon is blank");

        let bag = locate_bag_sprites(&rom).expect("bag sprite");
        assert_eq!(bag.frames.len(), 5);
        assert!(bag.frames.iter().all(|f| f.len() == BAG_FRAME_BYTES));
        assert!(
            bag.frames[0].iter().any(|b| *b != 0),
            "items pocket is blank"
        );
    }
}
