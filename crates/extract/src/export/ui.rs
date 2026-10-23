//! Writes the Milestone 9 UI graphics: `ui/<Name>.{png,ron}`, `ui/frames/<n>.{png,ron}`,
//! `icons/items/`, `icons/mons/` (Task X1 owns the file, with `ui_gfx.rs`).
//!
//! Wave 0: writes nothing.

use super::Out;
use crate::Rom;
use crate::schema::Hex;
use anyhow::Result;
use std::collections::BTreeMap;

/// Exports the UI pictures, window frames and icons, and records their table offsets in
/// `provenance` (Task X1). Wave 0: nothing.
pub fn export_ui(rom: &Rom, out: &mut Out, provenance: &mut BTreeMap<String, Hex>) -> Result<()> {
    let _ = (rom, out, provenance);
    Ok(())
}
