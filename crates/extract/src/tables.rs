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
//! - The field strings ([`FIELD_STRINGS`], Task L): C strings only code points to, found by
//!   the literal pools of the functions that copy them into `gStringVar1` (RAM addresses, the
//!   same on both ROMs). `GetPlayerBigGuyGirlString` and `GetRivalSonDaughterString` are
//!   adjacent: pools `{gSaveBlock2Ptr, gStringVar1, gText_BigGuy}` and `{gStringVar1,
//!   gText_BigGirl}`, then 0x38 bytes on `{gSaveBlock2Ptr, gStringVar1, gText_Daughter}` and
//!   `{gStringVar1, gText_Son}`. `GetMomOrDadStringForTVMessage` opens with `{gSaveBlock1Ptr,
//!   gSaveBlock2Ptr, gStringVar1, gText_Mom}`; its first later `{gStringVar1, s}` with
//!   `s != gText_Mom` is `gText_Dad`.
//!
//! Every string is decoded with [`text::round_trip_at`], so it re-encodes to its ROM bytes.
//!
//! **Ids** are the primary names of the `names` tables. TMs and HMs are named `ITEM_TMnn` /
//! `ITEM_HMnn` (not `ITEM_TM_FOCUS_PUNCH`): `names::ITEMS` has no `EQUIV_TM` aliases.

use crate::schema::{
    FieldStringsFile, Item, ItemPocket, ItemsFile, MapSectionsFile, MultichoiceFile, StdStringsFile,
};
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
/// Field offsets in `struct Item` (`include/item.h`).
const ITEM_ID: u32 = ITEM_NAME_LENGTH;
const ITEM_PRICE: u32 = 16;
const ITEM_IMPORTANCE: u32 = 24;
const ITEM_POCKET: u32 = 26;
/// `POCKET_ITEMS`, `POCKET_POKE_BALLS`.
const POCKET_ITEMS: u8 = 1;
const POCKET_POKE_BALLS: u8 = 2;
/// `gItems[ITEM_POTION].price` and `gItems[ITEM_POKE_BALL].price` (`src/data/items.h`).
const POTION_PRICE: u16 = 300;
const POKE_BALL_PRICE: u16 = 200;
/// `{ x, y, width, height }` of `MAPSEC_LITTLEROOT_TOWN`, `MAPSEC_OLDALE_TOWN` and
/// `MAPSEC_DEWFORD_TOWN`, the first three `gRegionMapEntries`.
const MAPSEC_ANCHOR: [[u8; 4]; 3] = [[4, 11, 1, 1], [4, 9, 1, 1], [2, 14, 1, 1]];
/// The Hoenn region map grid (`MAP_WIDTH` × `MAP_HEIGHT` in `region_map.c`) every section fits.
const REGION_MAP_WIDTH: u8 = 28;
const REGION_MAP_HEIGHT: u8 = 15;
/// Longest list a `struct MultichoiceListStruct` may hold here (the longest has 9).
const MAX_MULTICHOICE_COUNT: u8 = 16;
/// Shortest run of multichoice-shaped entries taken for `sMultichoiceLists`.
const MIN_MULTICHOICE_LISTS: u32 = 64;
/// `POCKET_ITEMS` .. `POCKET_KEY_ITEMS`.
const MAX_POCKET: u8 = 5;

/// `sMultichoiceLists` holds every `MULTI_*` but this one (the menu's cancel result).
const MULTI_B_PRESSED: &str = "MULTI_B_PRESSED";

/// File offsets of the tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    pub items: u32,
    pub multichoice: u32,
    /// Number of `sMultichoiceLists` entries (`MULTI_*`).
    pub multichoice_count: u32,
    pub std_strings: u32,
    pub map_sections: u32,
    /// [`FIELD_STRINGS`] in order: each string's offset.
    pub field_strings: Vec<u32>,
}

/// The decoded tables, as written to `tables/`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub items: ItemsFile,
    pub multichoice: MultichoiceFile,
    pub std_strings: StdStringsFile,
    pub map_sections: MapSectionsFile,
    pub field_strings: FieldStringsFile,
}

/// The C strings `tables/field_strings.ron` holds (decomp names, `src/strings.c`), in this
/// order: content numbers them by it.
pub const FIELD_STRINGS: [&str; 6] = [
    "gText_BigGuy",
    "gText_BigGirl",
    "gText_Son",
    "gText_Daughter",
    "gText_Mom",
    "gText_Dad",
];

/// `gSaveBlock1Ptr`, `gSaveBlock2Ptr` and `gStringVar1` (the same on BPEE and BPEF).
const SAVE_BLOCK1_PTR: u32 = 0x0300_5D8C;
const SAVE_BLOCK2_PTR: u32 = 0x0300_5D90;
const STRING_VAR1: u32 = 0x0202_1CC4;
/// `VAR_TEMP_3`, a literal of `GetMomOrDadStringForTVMessage`'s pools.
const VAR_TEMP_3: u32 = 0x4003;

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

fn dense_count(t: &names::Table) -> u32 {
    t.entries.last().map_or(0, |&(v, _)| v + 1)
}

/// `ITEMS_COUNT`.
pub fn item_count() -> u32 {
    dense_count(&names::ITEMS)
}

/// Number of `sMultichoiceLists` entries: every `MULTI_*` but `MULTI_B_PRESSED`.
pub fn multichoice_count() -> u32 {
    names::MULTICHOICE
        .iter()
        .filter(|&(_, n)| n != MULTI_B_PRESSED)
        .map(|(v, _)| v + 1)
        .max()
        .unwrap_or(0)
}

/// `STDSTRING_*` count.
pub fn std_string_count() -> u32 {
    dense_count(&names::STD_STRINGS)
}

/// `MAPSEC_NONE`: `gRegionMapEntries` has one entry per section before it.
pub fn map_section_count() -> u32 {
    names::MAP_SECTIONS
        .value("MAPSEC_NONE")
        .expect("MAPSEC_NONE in names")
}

fn item_id(name: &str) -> u32 {
    names::ITEMS
        .value(name)
        .unwrap_or_else(|| panic!("{name} is not in names"))
}

pub fn locate(rom: &Rom) -> Result<Locations> {
    let items = locate_items(rom)?;
    let (multichoice, multichoice_count) = locate_multichoice(rom)?;
    ensure!(
        multichoice_count == self::multichoice_count(),
        "sMultichoiceLists at {multichoice:#x}: {multichoice_count} lists, the decomp has {}",
        self::multichoice_count()
    );
    let std_strings = multichoice + MULTICHOICE_SIZE * multichoice_count;
    ensure_referenced(rom, std_strings, "gStdStrings")?;
    for i in 0..std_string_count() {
        rom.req_ptr(std_strings + 4 * i, "gStdStrings")?;
    }
    let end = std_strings + 4 * std_string_count();
    ensure!(
        !is_rom_ptr(rom, rom.u32(end)?),
        "gStdStrings at {std_strings:#x}: the word after its {} entries ({end:#x}) is a ROM \
         pointer, so the table is longer than the decomp's",
        std_string_count()
    );
    let map_sections = locate_map_sections(rom)?;
    let field_strings = locate_field_strings(rom)?;
    Ok(Locations {
        items,
        multichoice,
        multichoice_count,
        std_strings,
        map_sections,
        field_strings,
    })
}

/// The offsets of [`FIELD_STRINGS`], by the literal pools described in the module docs.
fn locate_field_strings(rom: &Rom) -> Result<Vec<u32>> {
    let word = |o: u32| rom.u32(o).ok();
    let words = |o: u32, w: &[u32]| (0..w.len()).all(|i| word(o + 4 * i as u32) == Some(w[i]));
    let string = |o: u32| rom.req_ptr(o, "field string");
    let aligned = (0..rom.bytes().len() as u32 / 4).map(|i| 4 * i);
    let big: Vec<u32> = aligned
        .clone()
        .filter(|&o| {
            words(o, &[SAVE_BLOCK2_PTR, STRING_VAR1])
                && words(o + 0x18, &[STRING_VAR1])
                && words(o + 0x38, &[SAVE_BLOCK2_PTR, STRING_VAR1])
                && words(o + 0x50, &[STRING_VAR1])
        })
        .collect();
    let [big] = big[..] else {
        bail!("GetPlayerBigGuyGirlString: expected 1 literal pool, found {big:#x?}");
    };
    let tv: Vec<u32> = aligned
        .filter(|&o| words(o, &[SAVE_BLOCK1_PTR, SAVE_BLOCK2_PTR, STRING_VAR1]))
        .collect();
    let [tv] = tv[..] else {
        bail!("GetMomOrDadStringForTVMessage: expected 1 literal pool, found {tv:#x?}");
    };
    // The function's pools (the same code on BPEE and BPEF): its first holds
    // `{gSaveBlock1Ptr, gSaveBlock2Ptr, gStringVar1, gText_Mom, VAR_TEMP_3}`; the `== 1` branch's
    // `{gStringVar1, gText_Mom, VAR_TEMP_3}` sits at +0x74 and the `== 2` branch's
    // `{gStringVar1, gText_Dad}` at +0x8c.
    let mom_ptr = word(tv + 12).context("GetMomOrDadStringForTVMessage: gText_Mom")?;
    ensure!(
        words(tv + 16, &[VAR_TEMP_3])
            && words(tv + 0x74, &[STRING_VAR1, mom_ptr, VAR_TEMP_3])
            && words(tv + 0x8c, &[STRING_VAR1]),
        "GetMomOrDadStringForTVMessage at {tv:#x}: unexpected literal pools"
    );
    let mom = string(tv + 12)?;
    let dad = tv + 0x8c;
    let dad_ptr = word(dad + 4).unwrap_or(0);
    ensure!(
        is_rom_ptr(rom, dad_ptr) && dad_ptr != mom_ptr,
        "GetMomOrDadStringForTVMessage at {tv:#x}: no gText_Dad at +0x90"
    );
    Ok(vec![
        string(big + 8)?,
        string(big + 0x1c)?,
        string(big + 0x54)?,
        string(big + 0x40)?,
        mom,
        string(dad + 4)?,
    ])
}

fn locate_items(rom: &Rom) -> Result<u32> {
    let potion = item_id("ITEM_POTION");
    let ball = item_id("ITEM_POKE_BALL");
    // (id, price, pocket) of the two anchor entries.
    let entry_is = |at: u32, id: u32, price: u16, pocket: u8| {
        rom.u16(at + ITEM_ID).ok() == Some(id as u16)
            && rom.u16(at + ITEM_PRICE).ok() == Some(price)
            && rom.u8(at + ITEM_POCKET).ok() == Some(pocket)
    };
    let mut needle = (potion as u16).to_le_bytes().to_vec();
    needle.extend(POTION_PRICE.to_le_bytes());
    let mut hits = Vec::new();
    for o in rom.find_all(&needle) {
        let Some(at) = o.checked_sub(ITEM_ID) else {
            continue;
        };
        let Some(base) = at.checked_sub(ITEM_SIZE * potion) else {
            continue;
        };
        if at % 4 == 0
            && entry_is(at, potion, POTION_PRICE, POCKET_ITEMS)
            && entry_is(
                base + ITEM_SIZE * ball,
                ball,
                POKE_BALL_PRICE,
                POCKET_POKE_BALLS,
            )
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
    rom.u16(at + ITEM_ID)
        .is_ok_and(|id| id == i as u16 || id == 0)
        && rom.u8(at + ITEM_POCKET).is_ok_and(|p| p <= MAX_POCKET)
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
    let hits: Vec<u32> = rom
        .find_all(&MAPSEC_ANCHOR[0])
        .into_iter()
        .filter(|&o| {
            o % 4 == 0
                && MAPSEC_ANCHOR.iter().enumerate().all(|(i, g)| {
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
    for i in 0..map_section_count() {
        let at = base + MAPSEC_SIZE * i;
        let [x, y, w, h] = map_section_geometry(rom, base, i)?;
        ensure!(
            w >= 1
                && h >= 1
                && x as u32 + w as u32 <= REGION_MAP_WIDTH as u32
                && y as u32 + h as u32 <= REGION_MAP_HEIGHT as u32,
            "gRegionMapEntries at {base:#x}: entry {i} ({at:#x}) is {w}x{h} at {x},{y}, outside \
             the {REGION_MAP_WIDTH}x{REGION_MAP_HEIGHT} map"
        );
        rom.req_ptr(at + 4, "gRegionMapEntries name")?;
    }
    Ok(base)
}

/// `{ x, y, width, height }` of `gRegionMapEntries[i]` (the table at `base`).
pub fn map_section_geometry(rom: &Rom, base: u32, i: u32) -> Result<[u8; 4]> {
    Ok(rom.slice(base + MAPSEC_SIZE * i, 4)?.try_into().unwrap())
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

/// `POCKET_*` (1-based, 0 = `POCKET_NONE`).
fn pocket(v: u8) -> Result<Option<ItemPocket>> {
    Ok(Some(match v {
        0 => return Ok(None),
        1 => ItemPocket::Items,
        2 => ItemPocket::PokeBalls,
        3 => ItemPocket::TmHm,
        4 => ItemPocket::Berries,
        5 => ItemPocket::KeyItems,
        _ => bail!("pocket {v} is not a POCKET_*"),
    }))
}

fn decomp_name(t: &names::Table, v: u32) -> Result<String> {
    t.name(v)
        .map(str::to_string)
        .with_context(|| format!("{}{v:#x} has no decomp name", t.prefix))
}

pub fn extract(rom: &Rom) -> Result<Tables> {
    extract_at(rom, &locate(rom)?)
}

/// [`extract`] from tables already located.
pub fn extract_at(rom: &Rom, loc: &Locations) -> Result<Tables> {
    let items = (0..item_count())
        .map(|i| {
            let at = loc.items + ITEM_SIZE * i;
            let id = decomp_name(&names::ITEMS, i)?;
            let name = text::round_trip_at(rom, at).with_context(|| id.clone())?;
            let len = text::encode_lang(&name, rom.lang())?.len() as u32;
            ensure!(
                len <= ITEM_NAME_LENGTH,
                "{id} at {at:#x}: name of {len} bytes overflows its {ITEM_NAME_LENGTH}"
            );
            Ok(Item {
                name,
                pocket: pocket(rom.u8(at + ITEM_POCKET)?).with_context(|| id.clone())?,
                price: rom.u16(at + ITEM_PRICE)?,
                importance: rom.u8(at + ITEM_IMPORTANCE)?,
                id,
            })
        })
        .collect::<Result<_>>()?;
    let lists = multichoice_strings(rom, loc)?
        .iter()
        .enumerate()
        .map(|(i, list)| {
            let id = decomp_name(&names::MULTICHOICE, i as u32)?;
            let strings = list.iter().enumerate().map(|(k, &o)| {
                text::round_trip_at(rom, o).with_context(|| format!("{id} string {k}"))
            });
            let strings = strings.collect::<Result<Vec<_>>>()?;
            Ok((id, strings))
        })
        .collect::<Result<_>>()?;
    let strings = (0..std_string_count())
        .map(|i| {
            let p = rom.req_ptr(loc.std_strings + 4 * i, "gStdStrings")?;
            let id = decomp_name(&names::STD_STRINGS, i)?;
            let s = text::round_trip_at(rom, p).with_context(|| id.clone())?;
            Ok((id, s))
        })
        .collect::<Result<_>>()?;
    let sections = (0..map_section_count())
        .map(|i| {
            let p = rom.req_ptr(loc.map_sections + MAPSEC_SIZE * i + 4, "gRegionMapEntries")?;
            let id = decomp_name(&names::MAP_SECTIONS, i)?;
            let s = text::round_trip_at(rom, p).with_context(|| id.clone())?;
            Ok((id, s))
        })
        .collect::<Result<_>>()?;
    let field = FIELD_STRINGS
        .iter()
        .zip(&loc.field_strings)
        .map(|(&id, &p)| {
            let s = text::round_trip_at(rom, p).with_context(|| id.to_string())?;
            Ok((id.to_string(), s))
        })
        .collect::<Result<_>>()?;
    Ok(Tables {
        items: ItemsFile { items },
        multichoice: MultichoiceFile { lists },
        std_strings: StdStringsFile { strings },
        map_sections: MapSectionsFile { sections },
        field_strings: FieldStringsFile { strings: field },
    })
}
