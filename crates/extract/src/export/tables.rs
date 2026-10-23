//! The C-side tables (owned by Milestone 8 Task C): `tables/items.ron`,
//! `tables/multichoice.ron`, `tables/std_strings.ron`, `tables/map_sections.ron` and (Task L)
//! `tables/field_strings.ron` ([`crate::schema::ItemsFile`] and friends), read by
//! [`crate::tables`].

use super::Out;
use crate::schema::Hex;
use crate::{Rom, tables};
use anyhow::Result;
use std::collections::BTreeMap;

/// The files [`export_tables`] writes, relative to the extracted dir.
pub const FILES: [&str; 5] = [
    "tables/items.ron",
    "tables/multichoice.ron",
    "tables/std_strings.ron",
    "tables/map_sections.ron",
    "tables/field_strings.ron",
];

/// The TM/HM file [`export_tmhm`] writes (format minor 7).
pub const TMHM_FILE: &str = "tables/tmhm.ron";

/// The berry file [`export_berries`] writes (format minor 9).
pub const BERRIES_FILE: &str = "tables/berries.ron";

/// Writes the item, multichoice, std-string, map-section and field-string tables, and records the table
/// offsets in `provenance` (`tables/items` = `gItems`, …).
pub fn export_tables(
    rom: &Rom,
    out: &mut Out,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<()> {
    let loc = tables::locate(rom)?;
    let t = tables::extract_at(rom, &loc)?;
    out.ron(FILES[0], &t.items)?;
    out.ron(FILES[1], &t.multichoice)?;
    out.ron(FILES[2], &t.std_strings)?;
    out.ron(FILES[3], &t.map_sections)?;
    out.ron(FILES[4], &t.field_strings)?;
    for (key, off) in [
        ("tables/items", loc.items),
        ("tables/multichoice", loc.multichoice),
        ("tables/std_strings", loc.std_strings),
        ("tables/map_sections", loc.map_sections),
        ("tables/field_strings", loc.field_strings[0]),
    ] {
        provenance.insert(key.to_string(), Hex(off));
    }
    Ok(())
}

/// `tables/tmhm.ron` ([`crate::ui_text::extract_tmhm`]); offsets in `provenance` as
/// `tables/tmhm_moves` (`sTMHMMoves`) and `tables/tmhm_learnsets` (`gTMHMLearnsets`).
pub fn export_tmhm(rom: &Rom, out: &mut Out, provenance: &mut BTreeMap<String, Hex>) -> Result<()> {
    let (file, moves, learnsets) = crate::ui_text::extract_tmhm(rom)?;
    out.ron(TMHM_FILE, &file)?;
    provenance.insert("tables/tmhm_moves".to_string(), Hex(moves));
    provenance.insert("tables/tmhm_learnsets".to_string(), Hex(learnsets));
    Ok(())
}

/// `tables/berries.ron` ([`crate::berries::extract`]); the offset in `provenance` as
/// `tables/berries` (`gBerries`).
pub fn export_berries(
    rom: &Rom,
    out: &mut Out,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<()> {
    let (file, base) = crate::berries::extract(rom)?;
    out.ron(BERRIES_FILE, &file)?;
    provenance.insert("tables/berries".to_string(), Hex(base));
    Ok(())
}
