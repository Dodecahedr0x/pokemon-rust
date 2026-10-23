//! Pictures of an edited map and of a metatile palette, drawn by the map view's
//! [`render_map`] (the engine's `Field` on a native PPU) with the build's own tilesets.
//!
//! The engine reads maps as `&'static MapData`, so an edited map is served by [`Preview`], a
//! [`GameData`] that answers [`PREVIEW`] with a leaked `MapData` and delegates everything else
//! to the build. Leaks are bounded: one `MapData` per (size, tilesets, border, connections),
//! cached; the cells that differ from its blocks are drawn as `setmetatile` overrides.

use super::catalog::Catalog;
use super::model::Doc;
use crate::map_view::{MapImage, render_map};
use anyhow::{Context, Result, anyhow};
use content::Emerald;
use engine::data::{
    Behavior, Connection, Dir, Font, GameData, GfxId, MapData, MapId, MapScripts, NewGame,
    ScriptId, SpriteData, TextId, UiText, WindowGfx,
};
use engine::script::{Ctx, Script};
use extract::schema::ConnectionDir;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// The id [`Preview`] serves the edited map under (Gen3 has no map 0xFFFE).
pub const PREVIEW: MapId = MapId(0xFFFE);

/// The build's game with [`PREVIEW`] answered by `map`.
pub struct Preview {
    pub game: Emerald,
    pub map: &'static MapData,
}

impl Preview {
    pub fn new(map: &'static MapData) -> Preview {
        Preview {
            game: Emerald::for_shells(),
            map,
        }
    }
}

impl GameData for Preview {
    fn map(&self, id: MapId) -> &'static MapData {
        if id == PREVIEW {
            self.map
        } else {
            self.game.map(id)
        }
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        self.game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        self.game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        self.game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        self.game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        self.game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        self.game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        self.game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        self.game.script(id, ctx)
    }
    fn new_game(&self) -> NewGame {
        self.game.new_game()
    }
}

/// The engine's connections of `d` to the build's maps (connections to games dir maps, dive
/// and emerge are not drawn).
fn connections(d: &Doc, c: &Catalog) -> Vec<Connection> {
    (d.map.connections.iter())
        .filter_map(|cn| {
            let dir = match cn.dir {
                ConnectionDir::South => Dir::Down,
                ConnectionDir::North => Dir::Up,
                ConnectionDir::West => Dir::Left,
                ConnectionDir::East => Dir::Right,
                ConnectionDir::Dive | ConnectionDir::Emerge => return None,
            };
            let id = c.maps.get(&cn.map)?.id?;
            Some(Connection {
                dir,
                offset: cn.offset as i16,
                map: id,
            })
        })
        .collect()
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

/// `d` as the engine's `MapData` (no events: [`render_map`] draws none), leaked. Fails when a
/// tileset is not in the catalog.
pub fn leak_map_data(d: &Doc, c: &Catalog) -> Result<&'static MapData> {
    let ts = |n: &str| {
        c.tilesets
            .get(n)
            .map(|t| t.tileset)
            .ok_or_else(|| anyhow!("unknown tileset {n:?}"))
    };
    Ok(Box::leak(Box::new(MapData {
        name: "PREVIEW",
        width: d.width,
        height: d.height,
        blocks: leak(d.blocks.clone()),
        border: d.border,
        primary: ts(&d.primary)?,
        secondary: ts(&d.secondary)?,
        objects: &[],
        warps: &[],
        coords: &[],
        bgs: &[],
        connections: leak(connections(d, c)),
        scripts: MapScripts::default(),
        allow_running: true,
    })))
}

/// What a cached base `MapData` was built for (everything but the blocks).
#[derive(Clone, PartialEq, Eq, Hash)]
struct BaseKey {
    width: u16,
    height: u16,
    primary: String,
    secondary: String,
    border: [u16; 4],
    connections: Vec<(Dir, i16, MapId)>,
}

/// The whole map with `margin` cells of border (and connected build maps) around it.
pub fn render(d: &Doc, c: &Catalog, margin: u16) -> Result<MapImage> {
    static BASES: OnceLock<Mutex<HashMap<BaseKey, &'static MapData>>> = OnceLock::new();
    let key = BaseKey {
        width: d.width,
        height: d.height,
        primary: d.primary.clone(),
        secondary: d.secondary.clone(),
        border: d.border,
        connections: (connections(d, c).iter())
            .map(|cn| (cn.dir, cn.offset, cn.map))
            .collect(),
    };
    let base = {
        let mut bases = BASES.get_or_init(Default::default).lock().unwrap();
        match bases.get(&key) {
            // The catalog's tilesets are the build's statics, so a name always gives the same.
            Some(&m) => m,
            None => {
                let m = leak_map_data(d, c)?;
                bases.insert(key, m);
                m
            }
        }
    };
    let w = d.width as usize;
    let overrides: Vec<((i16, i16), u16)> = (d.blocks.iter().zip(base.blocks))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (&a, _))| (((i % w) as i16, (i / w) as i16), a))
        .collect();
    let mut img = render_map(&Preview::new(base), PREVIEW, margin, &overrides);
    img.map = PREVIEW;
    Ok(img)
}

/// Metatiles per palette row.
pub const PALETTE_COLS: u16 = 8;

/// A metatile palette: the primary tileset's metatiles then the secondary's, [`PALETTE_COLS`]
/// per row, each as a cell (the secondary starts on a new row). `metatiles[i]` is the metatile
/// of cell `i` (`None` for padding).
pub struct Palette {
    pub image: MapImage,
    pub cols: u16,
    pub metatiles: Vec<Option<u16>>,
}

impl Palette {
    /// The metatile of palette cell (x, y).
    pub fn at(&self, x: i32, y: i32) -> Option<u16> {
        if x < 0 || y < 0 || x >= self.cols as i32 {
            return None;
        }
        (self.metatiles.get((y * self.cols as i32 + x) as usize)).and_then(|m| *m)
    }

    /// The palette cell of `metatile`.
    pub fn cell_of(&self, metatile: u16) -> Option<(i32, i32)> {
        let i = self.metatiles.iter().position(|m| *m == Some(metatile))? as i32;
        Some((i % self.cols as i32, i / self.cols as i32))
    }
}

/// The palette of `primary` + `secondary`, drawn as a map of their metatiles.
pub fn palette(c: &Catalog, primary: &str, secondary: &str) -> Result<Palette> {
    let (p, s) = c
        .metatile_ranges(primary, secondary)
        .with_context(|| format!("unknown tilesets {primary:?} / {secondary:?}"))?;
    let cols = PALETTE_COLS as usize;
    let pad = |n: usize| n.div_ceil(cols) * cols;
    let mut metatiles: Vec<Option<u16>> = p.clone().map(Some).collect();
    metatiles.resize(pad(metatiles.len()), None);
    metatiles.extend(s.clone().map(Some));
    metatiles.resize(pad(metatiles.len()).max(cols), None);
    let rows = (metatiles.len() / cols) as u16;
    let d = Doc {
        width: PALETTE_COLS,
        height: rows,
        primary: primary.to_string(),
        secondary: secondary.to_string(),
        border: [0; 4],
        blocks: metatiles.iter().map(|m| m.unwrap_or(0)).collect(),
        ..palette_doc()
    };
    Ok(Palette {
        image: render(&d, c, 0)?,
        cols: PALETTE_COLS,
        metatiles,
    })
}

fn palette_doc() -> Doc {
    Doc::new(&super::model::NewMap {
        name: "Palette".into(),
        layout: None,
        width: 1,
        height: 1,
        primary: String::new(),
        secondary: String::new(),
        fill: 0,
        fill_collision: 0,
        fill_elevation: 0,
        border: None,
    })
    .expect("a 1x1 map")
}

/// RGBA8 of a picture.
pub fn rgba(img: &MapImage) -> Vec<u8> {
    let mut out = vec![0; img.width * img.height * 4];
    crate::frames::rgba_from_rgb555(&img.rgb555, &mut out);
    out
}

/// A picture as PNG bytes.
pub fn png(img: &MapImage) -> Vec<u8> {
    crate::map_view::encode_png(img.width, img.height, &rgba(img))
}

/// Writes a picture as a PNG file (creating its directory).
pub fn write_png(img: &MapImage, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, png(img)).with_context(|| format!("writing {}", path.display()))
}
