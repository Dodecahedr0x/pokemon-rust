//! Reads the games dir's maps and layouts (`maps/<Name>.ron`, `layouts/<Name>.ron`; the studio
//! map editor's format, `extract::games::maps`). SIL1 Wave 0 reads every map; Task A adds the
//! editor's limits (objects, sizes, warps), header names and the other files.

use super::schema::{GameMap, Layout};
use anyhow::{Context, Result, ensure};
use extract::export::read_ron;
use std::path::{Path, PathBuf};

/// One map with its layout.
#[derive(Clone, Debug)]
pub struct MapIn {
    pub map: GameMap,
    pub layout: Layout,
}

/// Every `maps/*.ron`, in file name order, each with its layout. A map's `name` is its file
/// stem; its layout is `layouts/<layout>.ron`.
pub fn load_maps(games: &Path) -> Result<Vec<MapIn>> {
    let dir = games.join("maps");
    let mut paths: Vec<PathBuf> = match dir.is_dir() {
        true => std::fs::read_dir(&dir)
            .with_context(|| dir.display().to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<_>>()?,
        false => Vec::new(),
    };
    paths.retain(|p| p.extension().is_some_and(|x| x == "ron"));
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let map: GameMap = read_ron(&p)?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        ensure!(
            map.name == stem,
            "{}: the map is named {}",
            p.display(),
            map.name
        );
        let lp = games.join(format!("layouts/{}.ron", map.layout));
        let layout: Layout =
            read_ron(&lp).with_context(|| format!("map {}: its layout", map.name))?;
        out.push(MapIn { map, layout });
    }
    Ok(out)
}
