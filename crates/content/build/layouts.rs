//! The layout variants of maps (Wave C, Gen3 `setmaplayoutindex`): for every map whose own
//! scripts switch it to another `LAYOUT_*` (in `on_transition`, before `InitMap`), that layout
//! from `layouts/<Layout>.ron`. `codegen::data` compiles each as a `MapData` with the map's
//! events and the layout's blocks (`GameData::map_layout`); `codegen::map_digests` hashes them
//! with their map.

use crate::input::World;
use anyhow::{Context, Result};
use extract::schema::{IrArg, Layout, LayoutFile};
use std::collections::BTreeSet;

/// One map drawn with another layout.
pub struct Variant {
    /// The map's name (its module and `M_<name>` static).
    pub map: String,
    /// `LAYOUT_*`.
    pub id: String,
    /// The `gMapLayouts` number `setmaplayoutindex` takes.
    pub number: u16,
    pub layout: Layout,
}

/// `LAYOUT_LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB_WITH_TABLE` and the file stem
/// `LittlerootTown_ProfessorBirchsLabWithTable` have the same key.
fn key(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Every map's layout variants, in map then layout order. Variants equal to the map's own
/// layout (Route 130's `LAYOUT_ROUTE130`) are left out: the map keeps its own. Empty without an
/// extracted dir or a script IR (the stub, Silice).
pub fn variants(w: &World) -> Result<Vec<Variant>> {
    let (Some(dir), Some(ir)) = (&w.dir, &w.ir) else {
        return Ok(Vec::new());
    };
    let mut wanted: BTreeSet<(String, String)> = BTreeSet::new();
    for f in &ir.files {
        if !w.maps.iter().any(|m| m.name == f.module) {
            continue;
        }
        for i in f.labels.iter().flat_map(|l| &l.insns) {
            if let ("setmaplayoutindex", [IrArg::Const { name, .. }]) = (i.op.as_str(), &i.args[..])
            {
                wanted.insert((f.module.clone(), name.clone()));
            }
        }
    }
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let layouts_dir = dir.join("layouts");
    let mut stems: Vec<String> = std::fs::read_dir(&layouts_dir)
        .with_context(|| format!("reading {}", layouts_dir.display()))?
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "ron").then(|| p.file_stem()?.to_str().map(String::from))?
        })
        .collect();
    stems.sort();
    let mut out = Vec::new();
    for (map, id) in wanted {
        let number = extract::names::LAYOUTS
            .value(&id)
            .with_context(|| format!("map {map}: unknown layout {id}"))?;
        let want = key(id.trim_start_matches("LAYOUT_"));
        let stem = stems
            .iter()
            .find(|s| key(s) == want)
            .with_context(|| format!("map {map}: no layouts/ file for {id}"))?;
        let path = layouts_dir.join(format!("{stem}.ron"));
        let file: LayoutFile = extract::export::read_ron(&path)?;
        anyhow::ensure!(file.id == id, "{}: id {} for {id}", path.display(), file.id);
        let own = w.maps.iter().find(|m| m.name == map).map(|m| &m.layout);
        if own == Some(&file.layout) {
            continue;
        }
        out.push(Variant {
            map,
            id,
            number: u16::try_from(number)?,
            layout: file.layout,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::key;

    #[test]
    fn layout_ids_and_file_stems_share_a_key() {
        assert_eq!(
            key("LITTLEROOT_TOWN_PROFESSOR_BIRCHS_LAB_WITH_TABLE"),
            key("LittlerootTown_ProfessorBirchsLabWithTable")
        );
        assert_eq!(key("SKY_PILLAR_1F_CLEAN"), key("SkyPillar_1F_Clean"));
        assert_ne!(key("ROUTE111"), key("Route111_NoMirageTower"));
    }
}
