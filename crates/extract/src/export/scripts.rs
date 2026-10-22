//! The script export (Milestone 3 Task B; owned by Milestone 8 Task B): decodes the scripts
//! reachable from the exported maps' roots and writes `text/<lang>.ron`, the generated
//! `scripts/<Map>.rs` and `scripts/legacy_index.ron`. Task B adds the IR (`scripts/<Module>.ron`
//! and `scripts/index.ron`, [`crate::schema::ScriptFile`]) and keeps the `.rs` output alongside
//! until Task D2 lowers the IR in the content build.

use super::{ExportedMap, Out};
use crate::{GameCode, Rom, codegen};
use anyhow::Result;
use std::collections::BTreeMap;

/// Everything the script export needs.
pub struct ScriptInput<'a> {
    /// The exported maps in export order.
    pub maps: &'a [ExportedMap],
    /// Root offset -> canonical name ([`crate::roots::canonical`] over all maps in export
    /// order).
    pub canonical: &'a BTreeMap<u32, String>,
}

/// Decodes the scripts reachable from `input`'s roots ([`codegen`]) and writes
/// `text/<lang>.ron` (keys `<Script>_T<n>`), `scripts/<Map>.rs` and
/// `scripts/legacy_index.ron`.
pub fn export_scripts(rom: &Rom, input: &ScriptInput, o: &mut Out) -> Result<()> {
    let maps: Vec<codegen::MapRoots> = input
        .maps
        .iter()
        .map(|m| codegen::MapRoots {
            map: m.name.to_string(),
            roots: m.roots.clone(),
        })
        .collect();
    let code = match rom.code() {
        GameCode::Bpee => "BPEE",
        GameCode::Bpef => "BPEF",
    };
    let g = codegen::generate(rom, &maps, &codegen::header(code, &rom.sha1_hex()))?;
    o.ron(&format!("text/{}.ron", rom.lang().code()), &g.texts)?;
    for (map, src) in &g.files {
        o.bytes(&format!("scripts/{map}.rs"), src.as_bytes())?;
    }
    o.ron("scripts/legacy_index.ron", &g.index)?;
    println!("{}", g.stats.summary());
    Ok(())
}
