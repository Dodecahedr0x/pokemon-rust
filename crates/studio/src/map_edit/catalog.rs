//! What an edited map may reference: tilesets (by extract name), maps (the game's and the games
//! dir's), object graphics and scripts.
//!
//! Tilesets are the content build's own `'static` ones, paired with their extract names
//! through the extract's map files (every compiled tileset is some map's), so a render is the
//! game's. On the stub world they are `StubPrimary` and `StubSecondary` (`StubTown`'s).

use super::games::GamesDir;
use anyhow::{Context, Result};
use content::Emerald;
use engine::data::{GameData, METATILES_IN_PRIMARY, MapId, Tileset};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A tileset the editor can paint with.
#[derive(Clone, Copy)]
pub struct TilesetRef {
    pub tileset: &'static Tileset,
    /// Whether it is used as a secondary tileset (metatiles from 512).
    pub secondary: bool,
}

impl TilesetRef {
    pub fn metatiles(&self) -> usize {
        self.tileset.metatiles.len()
    }
}

impl std::fmt::Debug for TilesetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "TilesetRef({} metatiles, secondary: {})",
            self.metatiles(),
            self.secondary
        )
    }
}

/// A map a warp or a connection may target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapRef {
    /// The game's id for an extracted map; `None` for a games dir map.
    pub id: Option<MapId>,
    pub warps: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub tilesets: BTreeMap<String, TilesetRef>,
    pub maps: BTreeMap<String, MapRef>,
    /// `OBJ_EVENT_GFX_*` names; empty when unknown (graphics are then not checked).
    pub gfx: BTreeSet<String>,
    /// Script names the build knows (`content::data::SCRIPT_NAMES`); others are warnings.
    pub scripts: BTreeSet<String>,
}

/// The extracted dir the content build read: `PKR_EXTRACTED_DIR`, else `content/extracted`.
pub fn extracted_dir() -> PathBuf {
    std::env::var_os("PKR_EXTRACTED_DIR").map_or_else(
        || {
            crate::paths::StudioPaths::detect()
                .workspace
                .join("content/extracted")
        },
        PathBuf::from,
    )
}

impl Catalog {
    /// The build's world: its tilesets, maps, object graphics and scripts.
    pub fn from_content() -> Result<Catalog> {
        let game = Emerald::for_shells();
        let mut c = Catalog::default();
        for (id, name) in crate::map_view::map_list() {
            let m = game.map(id);
            c.maps.insert(
                name.to_string(),
                MapRef {
                    id: Some(id),
                    warps: m.warps.len(),
                },
            );
        }
        c.scripts = content::data::SCRIPT_NAMES
            .iter()
            .map(|s| s.to_string())
            .collect();
        if content::STUB {
            let id = content::data::MAP_IDS[0];
            let m = game.map(id);
            c.add_tileset("StubPrimary", m.primary, false);
            c.add_tileset("StubSecondary", m.secondary, true);
            return Ok(c);
        }
        let dir = extracted_dir();
        for (id, name) in crate::map_view::map_list() {
            let path = dir.join("maps").join(format!("{name}.ron"));
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let file: extract::schema::MapFile =
                ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
            let m = game.map(id);
            c.add_tileset(&file.layout.primary, m.primary, false);
            c.add_tileset(&file.layout.secondary, m.secondary, true);
        }
        c.gfx = read_gfx(&dir).unwrap_or_default();
        Ok(c)
    }

    /// [`Catalog::from_content`] plus the maps of a games dir.
    pub fn for_games(games: &GamesDir) -> Result<Catalog> {
        let mut c = Catalog::from_content()?;
        c.add_games(games)?;
        Ok(c)
    }

    /// Adds (or replaces) the maps of a games dir.
    pub fn add_games(&mut self, games: &GamesDir) -> Result<()> {
        for name in games.map_names()? {
            let map = games.load_map(&name)?;
            self.maps.insert(
                name,
                MapRef {
                    id: None,
                    warps: map.warps.len(),
                },
            );
        }
        Ok(())
    }

    pub fn add_tileset(&mut self, name: &str, tileset: &'static Tileset, secondary: bool) {
        self.tilesets
            .insert(name.to_string(), TilesetRef { tileset, secondary });
    }

    /// Primary tileset names, then secondary ones, each sorted.
    pub fn tileset_names(&self, secondary: bool) -> Vec<String> {
        (self.tilesets.iter())
            .filter(|(_, t)| t.secondary == secondary)
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// The metatile indices `primary` + `secondary` define: `0..p` and `512..512 + s`.
    pub fn metatile_ranges(
        &self,
        primary: &str,
        secondary: &str,
    ) -> Option<(std::ops::Range<u16>, std::ops::Range<u16>)> {
        let p = self.tilesets.get(primary)?.metatiles() as u16;
        let s = self.tilesets.get(secondary)?.metatiles() as u16;
        Some((
            0..p.min(METATILES_IN_PRIMARY),
            METATILES_IN_PRIMARY..METATILES_IN_PRIMARY + s,
        ))
    }
}

/// `OBJ_EVENT_GFX_<name>` for every sprite of the extract's manifest, plus its skipped (var)
/// graphics.
fn read_gfx(dir: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(dir.join("manifest.ron"))?;
    let m: extract::schema::Manifest = ron::from_str(&text)?;
    Ok((m.sprites.iter())
        .map(|s| format!("OBJ_EVENT_GFX_{s}"))
        .chain(m.skipped_graphics.iter().cloned())
        .collect())
}
