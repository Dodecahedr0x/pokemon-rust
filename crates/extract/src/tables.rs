//! Script-facing tables (Milestone 8 Task C): `gItems` (name, pocket, price, importance),
//! `sMultichoiceLists`, `gStdStrings` and the names of `gRegionMapEntries`.
//!
//! **Locating.** Every table is found on both ROMs from language-independent bytes; the US
//! decomp is the test oracle (`tests/tables.rs`):
//! - `gItems`: 44-byte `struct Item`s whose `itemId` is their index. The anchor is
//!   `ITEM_POTION` (id 13, price 300, `POCKET_ITEMS`) with `ITEM_POKE_BALL` (id 4, price 200,
//!   `POCKET_POKE_BALLS`) nine entries before it.
//! - `sMultichoiceLists`: the only long run of 8-byte `{ const struct MenuAction *list; u8
//!   count; }` entries (count ≥ 1, zero padding) whose lists are `count` `{ text, NULL }` pairs.
//! - `gStdStrings`: `src/data/script_menu.h` defines it right after `sMultichoiceLists`; it is
//!   checked to be a referenced run of string pointers.
//! - `gRegionMapEntries`: 8-byte `{ x, y, width, height, name }` entries; the anchor is the
//!   region-map geometry of the first three (Littleroot 4,11; Oldale 4,9; Dewford 2,14; each
//!   1×1).
//!
//! Every string is decoded with [`text::round_trip_at`], so it re-encodes to its ROM bytes.

use crate::text;
use crate::{ROM_BASE, Rom, names};
use anyhow::{Context, Result, bail, ensure};

/// `sizeof(struct Item)`.
pub const ITEM_SIZE: u32 = 44;
/// `sizeof(struct RegionMapLocation)`.
pub const MAPSEC_SIZE: u32 = 8;
const MULTICHOICE_SIZE: u32 = 8;
const MENU_ACTION_SIZE: u32 = 8;
const ITEM_NAME_LENGTH: u32 = 14;
/// Longest list a `struct MultichoiceListStruct` may hold here (the longest has 9).
const MAX_MULTICHOICE_COUNT: u8 = 16;
/// Shortest run of multichoice-shaped entries taken for `sMultichoiceLists`.
const MIN_MULTICHOICE_LISTS: u32 = 64;
/// `POCKET_ITEMS` .. `POCKET_KEY_ITEMS`.
const MAX_POCKET: u8 = 5;

/// `STDSTRING_BATTLE_PYRAMID + 1`.
// TODO(M8 Wave 0): from `names::STDSTRINGS`.
const STD_STRINGS_COUNT: u32 = 30;
/// `MAPSEC_NONE`: `gRegionMapEntries` has one entry per section before it.
// TODO(M8 Wave 0): from `names::MAPSECS`.
const MAP_SECTIONS_COUNT: u32 = 213;

/// File offsets of the tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    pub items: u32,
    pub multichoice: u32,
    /// Number of `sMultichoiceLists` entries (`MULTI_*`).
    pub multichoice_count: u32,
    pub std_strings: u32,
    pub map_sections: u32,
}

/// One `gItems` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub price: u16,
    /// `POCKET_*` (1-based).
    pub pocket: u8,
    pub importance: u8,
}

/// The decoded tables, indexed by their decomp ids (`ITEM_*`, `MULTI_*`, `STDSTRING_*`,
/// `MAPSEC_*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tables {
    pub items: Vec<Item>,
    pub multichoice: Vec<Vec<String>>,
    pub std_strings: Vec<String>,
    pub map_sections: Vec<String>,
}

fn is_rom_ptr(rom: &Rom, v: u32) -> bool {
    v >= ROM_BASE && ((v - ROM_BASE) as usize) < rom.bytes().len()
}

fn ensure_referenced(rom: &Rom, off: u32, what: &str) -> Result<()> {
    ensure!(
        !rom.pointers_to(off).is_empty(),
        "{what} at {off:#x}: nothing points to it"
    );
    Ok(())
}

/// `ITEMS_COUNT`.
pub fn item_count() -> u32 {
    names::ITEMS.entries.last().map_or(0, |&(v, _)| v + 1)
}

fn item_id(name: &str) -> u32 {
    names::ITEMS
        .value(name)
        .unwrap_or_else(|| panic!("{name} is not in names"))
}

pub fn locate(rom: &Rom) -> Result<Locations> {
    let items = locate_items(rom)?;
    let (multichoice, multichoice_count) = locate_multichoice(rom)?;
    let std_strings = multichoice + MULTICHOICE_SIZE * multichoice_count;
    ensure_referenced(rom, std_strings, "gStdStrings")?;
    for i in 0..STD_STRINGS_COUNT {
        rom.req_ptr(std_strings + 4 * i, "gStdStrings")?;
    }
    let map_sections = locate_map_sections(rom)?;
    Ok(Locations {
        items,
        multichoice,
        multichoice_count,
        std_strings,
        map_sections,
    })
}

fn locate_items(rom: &Rom) -> Result<u32> {
    let potion = item_id("ITEM_POTION");
    let ball = item_id("ITEM_POKE_BALL");
    // (id, price, pocket) of the two anchor entries.
    let entry_is = |at: u32, id: u32, price: u16, pocket: u8| {
        rom.u16(at + ITEM_NAME_LENGTH).ok() == Some(id as u16)
            && rom.u16(at + ITEM_NAME_LENGTH + 2).ok() == Some(price)
            && rom.u8(at + 26).ok() == Some(pocket)
    };
    let mut needle = (potion as u16).to_le_bytes().to_vec();
    needle.extend(300u16.to_le_bytes());
    let mut hits = Vec::new();
    for o in rom.find_all(&needle) {
        let Some(at) = o.checked_sub(ITEM_NAME_LENGTH) else {
            continue;
        };
        let Some(base) = at.checked_sub(ITEM_SIZE * potion) else {
            continue;
        };
        if at % 4 == 0
            && entry_is(at, potion, 300, 1)
            && entry_is(base + ITEM_SIZE * ball, ball, 200, 2)
            && (0..item_count()).all(|i| item_ok(rom, base, i))
        {
            hits.push(base);
        }
    }
    let base = match hits[..] {
        [b] => b,
        _ => bail!(
            "gItems: expected 1 table holding ITEM_POTION and ITEM_POKE_BALL, found {hits:#x?}"
        ),
    };
    ensure_referenced(rom, base, "gItems")?;
    Ok(base)
}

/// Entry `i` of a `gItems` candidate at `base` has a valid pocket and `itemId` `i`, or
/// `ITEM_NONE` (the unused `ITEM_034` … slots).
fn item_ok(rom: &Rom, base: u32, i: u32) -> bool {
    let at = base + ITEM_SIZE * i;
    rom.u16(at + ITEM_NAME_LENGTH)
        .is_ok_and(|id| id == i as u16 || id == 0)
        && rom.u8(at + 26).is_ok_and(|p| p <= MAX_POCKET)
}

/// A `struct MultichoiceListStruct` at `at`: a list of `count` `{ text, NULL }` menu actions.
fn multichoice_ok(rom: &Rom, at: u32) -> bool {
    let Ok(b) = rom.slice(at, MULTICHOICE_SIZE as usize) else {
        return false;
    };
    let list = u32::from_le_bytes(b[..4].try_into().unwrap());
    let count = b[4];
    if !is_rom_ptr(rom, list) || !(1..=MAX_MULTICHOICE_COUNT).contains(&count) || b[5..] != [0; 3] {
        return false;
    }
    let list = list - ROM_BASE;
    (0..count as u32).all(|k| {
        let a = list + MENU_ACTION_SIZE * k;
        rom.u32(a).is_ok_and(|t| is_rom_ptr(rom, t)) && rom.u32(a + 4).ok() == Some(0)
    })
}

fn locate_multichoice(rom: &Rom) -> Result<(u32, u32)> {
    let end = rom.bytes().len() as u32;
    let mut runs = Vec::new();
    let mut o = 0;
    while o + MULTICHOICE_SIZE <= end {
        if !multichoice_ok(rom, o) {
            o += 4;
            continue;
        }
        let start = o;
        while multichoice_ok(rom, o) {
            o += MULTICHOICE_SIZE;
        }
        let n = (o - start) / MULTICHOICE_SIZE;
        if n >= MIN_MULTICHOICE_LISTS && !rom.pointers_to(start).is_empty() {
            runs.push((start, n));
        }
    }
    match runs[..] {
        [r] => Ok(r),
        _ => bail!(
            "sMultichoiceLists: expected 1 referenced run of multichoice lists, found {runs:#x?}"
        ),
    }
}

fn locate_map_sections(rom: &Rom) -> Result<u32> {
    const GEOMETRY: [[u8; 4]; 3] = [[4, 11, 1, 1], [4, 9, 1, 1], [2, 14, 1, 1]];
    let hits: Vec<u32> = rom
        .find_all(&GEOMETRY[0])
        .into_iter()
        .filter(|&o| {
            o % 4 == 0
                && GEOMETRY.iter().enumerate().all(|(i, g)| {
                    let at = o + MAPSEC_SIZE * i as u32;
                    rom.slice(at, 4).ok() == Some(&g[..])
                        && rom.u32(at + 4).is_ok_and(|p| is_rom_ptr(rom, p))
                })
        })
        .collect();
    let base = match hits[..] {
        [b] => b,
        _ => bail!("gRegionMapEntries: expected 1 match, found {hits:#x?}"),
    };
    ensure_referenced(rom, base, "gRegionMapEntries")?;
    Ok(base)
}

/// Offsets of the strings of every `sMultichoiceLists` entry, in `MULTI_*` order.
pub fn multichoice_strings(rom: &Rom, loc: &Locations) -> Result<Vec<Vec<u32>>> {
    (0..loc.multichoice_count)
        .map(|i| {
            let at = loc.multichoice + MULTICHOICE_SIZE * i;
            let list = rom.req_ptr(at, "multichoice list")?;
            let count = rom.u8(at + 4)? as u32;
            (0..count)
                .map(|k| rom.req_ptr(list + MENU_ACTION_SIZE * k, "multichoice text"))
                .collect()
        })
        .collect()
}

pub fn extract(rom: &Rom) -> Result<Tables> {
    let loc = locate(rom)?;
    let items = (0..item_count())
        .map(|i| {
            let at = loc.items + ITEM_SIZE * i;
            let name = text::round_trip_at(rom, at).with_context(|| format!("item {i}"))?;
            let len = text::encode_lang(&name, rom.lang())?.len() as u32;
            ensure!(
                len <= ITEM_NAME_LENGTH,
                "item {i} at {at:#x}: name of {len} bytes overflows its {ITEM_NAME_LENGTH}"
            );
            Ok(Item {
                name,
                price: rom.u16(at + ITEM_NAME_LENGTH + 2)?,
                importance: rom.u8(at + 24)?,
                pocket: rom.u8(at + 26)?,
            })
        })
        .collect::<Result<_>>()?;
    let multichoice = multichoice_strings(rom, &loc)?
        .iter()
        .map(|list| list.iter().map(|&o| text::round_trip_at(rom, o)).collect())
        .collect::<Result<_>>()?;
    let std_strings = (0..STD_STRINGS_COUNT)
        .map(|i| text::round_trip_at(rom, rom.req_ptr(loc.std_strings + 4 * i, "gStdStrings")?))
        .collect::<Result<_>>()?;
    let map_sections = (0..MAP_SECTIONS_COUNT)
        .map(|i| {
            let p = rom.req_ptr(loc.map_sections + MAPSEC_SIZE * i + 4, "gRegionMapEntries")?;
            text::round_trip_at(rom, p)
        })
        .collect::<Result<_>>()?;
    Ok(Tables {
        items,
        multichoice,
        std_strings,
        map_sections,
    })
}
