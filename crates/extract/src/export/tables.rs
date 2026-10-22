//! The C-side tables (owned by Milestone 8 Task C): `tables/items.ron`,
//! `tables/multichoice.ron`, `tables/std_strings.ron` and `tables/map_sections.ron`
//! ([`crate::schema::ItemsFile`] and friends), read by [`crate::tables`].

use super::Out;
use crate::Rom;
use crate::schema::Hex;
use anyhow::Result;
use std::collections::BTreeMap;

/// Writes the item, multichoice, std-string and map-section tables, and records where they came
/// from in `provenance`. Wave 0: writes nothing (Task C).
pub fn export_tables(
    rom: &Rom,
    out: &mut Out,
    provenance: &mut BTreeMap<String, Hex>,
) -> Result<()> {
    let _ = (rom, out, provenance);
    Ok(())
}
