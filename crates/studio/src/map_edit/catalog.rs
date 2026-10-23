//! What an edited map may reference: tilesets (by extract name), maps, object graphics and
//! scripts.
//!
//! Tilesets are the content build's own `'static` ones, paired with their extract names
//! through the extract's map files (every compiled tileset is some map's), so a render is the
//! game's. On the stub world they are `StubPrimary` and `StubSecondary` (`StubTown`'s).
//!
//! Maps: [`Catalog::from_content`] has the build's (for imported Emerald maps);
//! [`Catalog::for_games`] has only the games dir's own: a Silice map warps and connects to
//! Silice maps (and `MAP_DYNAMIC`), never to Emerald's.

use super::games::GamesDir;
use super::schema::GameConnection;
use anyhow::{Context, Result};
use content::Emerald;
use engine::data::{
    Dir, GameData, METATILES_IN_PRIMARY, MapId, Tileset, block_collision, block_metatile,
};
use extract::schema::ConnectionDir;
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

/// A map a warp or a connection may target, with what the cross-map checks need.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MapRef {
    /// The game's id for a build map; `None` for a games dir map.
    pub id: Option<MapId>,
    /// Width and height (`None`: its layout could not be read).
    pub size: Option<(u16, u16)>,
    /// Each warp's destination map name.
    pub warps: Vec<String>,
    pub connections: Vec<GameConnection>,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub tilesets: BTreeMap<String, TilesetRef>,
    pub maps: BTreeMap<String, MapRef>,
    /// `OBJ_EVENT_GFX_*` names; empty when unknown (graphics are then not checked).
    pub gfx: BTreeSet<String>,
    /// Script names the build knows (`content::data::SCRIPT_NAMES`); others are warnings.
    pub scripts: BTreeSet<String>,
    /// The games dir whose maps these are (renders draw connected games dir maps from it).
    pub games: Option<GamesDir>,
    /// How the extract's map cells use each metatile, keyed by (owning tileset name, metatile
    /// id as in a block): `[walkable, solid]` cell counts (`behavior::usual_collision`). Empty
    /// on the stub world.
    pub collision_uses: BTreeMap<(String, u16), [u32; 2]>,
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

fn connection_dir(d: Dir) -> ConnectionDir {
    match d {
        Dir::Down => ConnectionDir::South,
        Dir::Up => ConnectionDir::North,
        Dir::Left => ConnectionDir::West,
        Dir::Right => ConnectionDir::East,
    }
}

impl Catalog {
    /// The build's world: its tilesets, maps, object graphics and scripts.
    pub fn from_content() -> Result<Catalog> {
        let game = Emerald::for_shells();
        let mut c = Catalog::default();
        for (id, name) in crate::map_view::map_list() {
            let m = game.map(id);
            let name_of = |id| crate::map_view::map_name(&game, id).to_string();
            c.maps.insert(
                name.to_string(),
                MapRef {
                    id: Some(id),
                    size: Some((m.width, m.height)),
                    warps: m.warps.iter().map(|w| name_of(w.dest_map)).collect(),
                    connections: (m.connections.iter())
                        .map(|cn| GameConnection {
                            dir: connection_dir(cn.dir),
                            offset: cn.offset as i32,
                            map: name_of(cn.map),
                        })
                        .collect(),
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
        let mut seen = BTreeSet::new();
        for (id, name) in crate::map_view::map_list() {
            let path = dir.join("maps").join(format!("{name}.ron"));
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let file: extract::schema::MapFile =
                ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
            let m = game.map(id);
            c.add_tileset(&file.layout.primary, m.primary, false);
            c.add_tileset(&file.layout.secondary, m.secondary, true);
            c.count_uses(&file.layout, &mut seen);
        }
        c.gfx = read_gfx(&dir).unwrap_or_default();
        Ok(c)
    }

    /// The build's tilesets, graphics and scripts with only the maps of a games dir.
    pub fn for_games(games: &GamesDir) -> Result<Catalog> {
        let mut c = Catalog::from_content()?;
        c.maps.clear();
        c.add_games(games)?;
        Ok(c)
    }

    /// Adds (or replaces) the maps of a games dir. A map file that does not parse is left out
    /// (validating it reports why); a missing layout leaves its size unknown.
    pub fn add_games(&mut self, games: &GamesDir) -> Result<()> {
        for name in games.map_names()? {
            let Ok(map) = games.load_map(&name) else {
                continue;
            };
            let size = games
                .load_layout(&map.layout)
                .ok()
                .map(|l| (l.width, l.height));
            self.maps.insert(
                name,
                MapRef {
                    id: None,
                    size,
                    warps: map.warps.iter().map(|w| w.dest_map.clone()).collect(),
                    connections: map.connections.clone(),
                },
            );
        }
        self.games = Some(games.clone());
        Ok(())
    }

    /// Adds a layout's cells to [`Catalog::collision_uses`], once per layout: extracted map
    /// files inline their layout, so maps sharing one are deduplicated by its contents
    /// (`seen`).
    pub fn count_uses(
        &mut self,
        l: &extract::schema::Layout,
        seen: &mut BTreeSet<(String, String, u16, Vec<u16>)>,
    ) {
        let blocks = extract::schema::from_rows(&l.blocks);
        if !seen.insert((
            l.primary.clone(),
            l.secondary.clone(),
            l.width,
            blocks.clone(),
        )) {
            return;
        }
        for b in blocks {
            let mt = block_metatile(b);
            let ts = if mt < METATILES_IN_PRIMARY {
                &l.primary
            } else {
                &l.secondary
            };
            let e = self.collision_uses.entry((ts.clone(), mt)).or_default();
            e[(block_collision(b) != 0) as usize] += 1;
        }
    }

    pub fn add_tileset(&mut self, name: &str, tileset: &'static Tileset, secondary: bool) {
        self.tilesets
            .insert(name.to_string(), TilesetRef { tileset, secondary });
    }

    /// Tileset names of one kind, sorted.
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
