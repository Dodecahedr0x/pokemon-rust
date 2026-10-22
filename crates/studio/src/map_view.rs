//! The read-only map view core (M6 Task E): whole-map pictures drawn exactly as the game draws
//! them, per-cell data and event overlays.
//!
//! [`render_map`] does not reimplement metatile drawing: it drives the engine's
//! [`Field`] over a grid of 240x160 camera windows on a fresh native [`Ppu`] and stitches the
//! windows, so tileset uploads, layer routing, the NORMAL underlay, the border and connections
//! are the game's own (decision 9). `tests/map_pixels.rs` checks it against live sessions.

use engine::data::{
    BLOCK_METATILE_MASK, Behavior, BgKind, Dir, FlagId, GameData, Gfx, METATILES_IN_PRIMARY,
    MapData, MapId, MovementType, ScriptId, VarId, attr_behavior, block_collision, block_elevation,
    block_metatile,
};
use engine::field::Field;
use engine::video::{Frame, SCREEN_H, SCREEN_W};
use game_native::Ppu;

/// A whole-map picture: the map's cells plus `margin` cells of border/connections on each side.
/// Map cell (x, y) has its top-left pixel at `((x + margin) * 16, (y + margin) * 16)`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapImage {
    pub map: MapId,
    /// Cells drawn around the map on each side.
    pub margin: u16,
    /// Pixels: `(width + 2 * margin) * 16`.
    pub width: usize,
    /// Pixels: `(height + 2 * margin) * 16`.
    pub height: usize,
    /// Row-major RGB555, `width * height`.
    pub rgb555: Vec<u16>,
}

impl MapImage {
    /// Image pixel of the map pixel `(x, y)` (cell `(x / 16, y / 16)`).
    pub fn to_image(&self, x: i32, y: i32) -> (i32, i32) {
        let m = self.margin as i32 * 16;
        (x + m, y + m)
    }

    /// The 240x160 window whose top-left map pixel is `camera` (what the screen shows with
    /// that camera), or `None` if it does not fit inside the image.
    pub fn crop(&self, camera: (i32, i32)) -> Option<Vec<u16>> {
        let (x0, y0) = self.to_image(camera.0, camera.1);
        if x0 < 0
            || y0 < 0
            || x0 as usize + SCREEN_W > self.width
            || y0 as usize + SCREEN_H > self.height
        {
            return None;
        }
        let (x0, y0) = (x0 as usize, y0 as usize);
        let mut out = Vec::with_capacity(SCREEN_W * SCREEN_H);
        for y in y0..y0 + SCREEN_H {
            out.extend_from_slice(&self.rgb555[y * self.width + x0..][..SCREEN_W]);
        }
        Some(out)
    }
}

/// The map's BG layers as the game draws them: `Field::enter`, the `overrides` as
/// `Field::set_block` (recorded before the first draw, as on-load `setmetatile`s), then
/// `Field::set_camera` over a grid of 240x160 windows covering the image, each applied to one
/// fresh `Ppu` and rendered (no OBJs, brightness 0, BG0 off), stitched. Margin cells come from
/// connections and the border (`Field::block`). `overrides` are `setmetatile` blocks (a
/// snapshot's `field.overrides`); cells outside the map are ignored, as the game does.
///
/// A map the game does not have (0x0) gives a `2 * margin` square of its border.
pub fn render_map(
    game: &impl GameData,
    map: MapId,
    margin: u16,
    overrides: &[((i16, i16), u16)],
) -> MapImage {
    let m = game.map(map);
    let (cw, ch) = (
        m.width as usize + 2 * margin as usize,
        m.height as usize + 2 * margin as usize,
    );
    let (width, height) = (cw * 16, ch * 16);
    let mut img = MapImage {
        map,
        margin,
        width,
        height,
        rgb555: vec![0; width * height],
    };
    if width == 0 || height == 0 {
        return img;
    }
    let origin = -(margin as i32) * 16;
    let mut field = Field::new(map);
    let mut ppu = Ppu::new();
    let mut f = Frame::default();
    field.enter(game, map, (origin, origin), &mut f);
    for &((x, y), block) in overrides {
        field.set_block(game, x, y, block, &mut f);
    }
    ppu.apply(&f);
    let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
    for wy in (0..height).step_by(SCREEN_H) {
        for wx in (0..width).step_by(SCREEN_W) {
            let mut f = Frame::default();
            field.set_camera(game, (origin + wx as i32, origin + wy as i32), &mut f);
            ppu.apply(&f);
            ppu.render(&f, &mut fb);
            let (w, h) = (SCREEN_W.min(width - wx), SCREEN_H.min(height - wy));
            for row in 0..h {
                let dst = (wy + row) * width + wx;
                img.rgb555[dst..dst + w].copy_from_slice(&fb[row * SCREEN_W..][..w]);
            }
        }
    }
    img
}

/// One map cell for the hover panel and the collision/elevation/behavior overlays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellInfo {
    pub x: i16,
    pub y: i16,
    /// The raw block (metatile, collision and elevation bits).
    pub block: u16,
    pub metatile: u16,
    pub collision: u8,
    pub elevation: u8,
    /// `GameData::behavior(raw_behavior)`.
    pub behavior: Behavior,
    /// The metatile attribute's behavior byte (Gen3 `MB_*`); `0xFF` (`MB_INVALID`) for a
    /// metatile the tileset does not have.
    pub raw_behavior: u8,
}

/// Raw behavior byte of a metatile the tileset lacks (Gen3 `MB_INVALID`).
const MB_INVALID: u8 = 0xFF;

/// The behavior byte of `block` drawn with `map`'s tilesets (engine `block_behavior`'s lookup).
fn raw_behavior(map: &MapData, block: u16) -> u8 {
    let mt = block_metatile(block);
    let attr = if mt < METATILES_IN_PRIMARY {
        map.primary.attributes.get(mt as usize)
    } else {
        map.secondary
            .attributes
            .get((mt - METATILES_IN_PRIMARY) as usize)
    };
    attr.map_or(MB_INVALID, |&a| attr_behavior(a))
}

/// Every cell of the map's own blocks (no margin, no overrides), row-major, `width * height`.
pub fn cells(game: &impl GameData, map: MapId) -> Vec<CellInfo> {
    let m = game.map(map);
    let w = m.width as usize;
    m.blocks
        .iter()
        .enumerate()
        .map(|(i, &block)| {
            let raw = raw_behavior(m, block);
            CellInfo {
                x: (i % w) as i16,
                y: (i / w) as i16,
                block,
                metatile: block & BLOCK_METATILE_MASK,
                collision: block_collision(block),
                elevation: block_elevation(block),
                behavior: game.behavior(raw),
                raw_behavior: raw,
            }
        })
        .collect()
}

/// One event or connection drawn over the map. Coordinates are the map's cells.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    Object {
        local_id: u8,
        x: i16,
        y: i16,
        gfx: Gfx,
        movement: MovementType,
        range: (u8, u8),
        script: Option<&'static str>,
        hide_flag: Option<FlagId>,
    },
    Warp {
        /// Index in the map's warps (what other maps' `dest_warp` names).
        index: u8,
        x: i16,
        y: i16,
        dest: MapId,
        /// The destination's decomp name, `"MISSING"` when it is not exported.
        dest_name: &'static str,
        dest_warp: u8,
    },
    Coord {
        x: i16,
        y: i16,
        var: VarId,
        value: u16,
        script: &'static str,
    },
    Bg {
        x: i16,
        y: i16,
        kind: BgKind,
        /// The script of a `BgKind::Script`.
        script: Option<&'static str>,
    },
    Connection {
        dir: Dir,
        offset: i16,
        map: MapId,
        /// The connected map's decomp name, `"MISSING"` when it is not exported.
        name: &'static str,
    },
}

/// Script name (`content::data::SCRIPT_NAMES`), `"?"` for an id outside the table.
pub fn script_name(id: ScriptId) -> &'static str {
    content::data::SCRIPT_NAMES
        .get(id.0 as usize)
        .copied()
        .unwrap_or("?")
}

/// The decomp name of `map`, `"MISSING"` when the game does not have it.
pub fn map_name(game: &impl GameData, map: MapId) -> &'static str {
    if game.has_map(map) {
        game.map(map).name
    } else {
        "MISSING"
    }
}

/// Objects, warps, coord events, bg events and connections, in that order, each in `MapData`
/// order.
pub fn overlays(game: &impl GameData, map: MapId) -> Vec<Overlay> {
    let m = game.map(map);
    let objects = m.objects.iter().map(|o| Overlay::Object {
        local_id: o.local_id,
        x: o.x,
        y: o.y,
        gfx: o.gfx,
        movement: o.movement,
        range: o.range,
        script: o.script.map(script_name),
        hide_flag: o.hide_flag,
    });
    let warps = m.warps.iter().enumerate().map(|(i, w)| Overlay::Warp {
        index: i as u8,
        x: w.x,
        y: w.y,
        dest: w.dest_map,
        dest_name: map_name(game, w.dest_map),
        dest_warp: w.dest_warp,
    });
    let coords = m.coords.iter().map(|c| Overlay::Coord {
        x: c.x,
        y: c.y,
        var: c.var,
        value: c.value,
        script: script_name(c.script),
    });
    let bgs = m.bgs.iter().map(|b| Overlay::Bg {
        x: b.x,
        y: b.y,
        kind: b.kind,
        script: match b.kind {
            BgKind::Script { script, .. } => Some(script_name(script)),
            BgKind::HiddenItem { .. } => None,
        },
    });
    let connections = m.connections.iter().map(|c| Overlay::Connection {
        dir: c.dir,
        offset: c.offset,
        map: c.map,
        name: map_name(game, c.map),
    });
    objects
        .chain(warps)
        .chain(coords)
        .chain(bgs)
        .chain(connections)
        .collect()
}

/// Every exported map with its decomp name, in `content::data::MAP_IDS` order.
pub fn map_list() -> Vec<(MapId, &'static str)> {
    content::data::MAP_IDS
        .iter()
        .map(|&id| (id, content::data::map(id.0).map_or("MISSING", |m| m.name)))
        .collect()
}

// ---- The Map tab's model (M6 Task H): placement, hit-testing and routes, free of the UI. ----

/// Cells the Map tab draws around a map on each side (border and connections).
pub const VIEW_MARGIN: u16 = 8;

const CELL: i32 = 16;

/// The native pane's position, for the Map tab's "Show live map": its map and its camera (the
/// map pixel at the screen's top-left, as [`MapImage::crop`] takes it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Live {
    pub map: MapId,
    pub camera: (i32, i32),
}

impl Live {
    /// From the inspector's fields: `None` while the pane has no field camera.
    pub fn from_inspector(map: MapId, camera: Option<(i32, i32)>) -> Option<Live> {
        camera.map(|camera| Live { map, camera })
    }
}

/// The map on screen: the live map while following it, else the selected one.
pub fn shown_map(selected: MapId, follow: bool, live: Option<Live>) -> MapId {
    match live {
        Some(l) if follow => l.map,
        _ => selected,
    }
}

/// A rectangle in image pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// The image rectangle of map cell `(x, y)` with `margin` cells around the map.
    pub fn cell(x: i16, y: i16, margin: u16) -> Rect {
        Rect {
            x: (x as i32 + margin as i32) * CELL,
            y: (y as i32 + margin as i32) * CELL,
            w: CELL,
            h: CELL,
        }
    }

    /// The part inside a `w x h` image (an empty rectangle if none).
    pub fn clip(self, w: i32, h: i32) -> Rect {
        let (x0, y0) = (self.x.clamp(0, w), self.y.clamp(0, h));
        let (x1, y1) = ((self.x + self.w).clamp(0, w), (self.y + self.h).clamp(0, h));
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }

    pub fn center(self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// The live camera's 240x160 window in image pixels.
pub fn camera_rect(camera: (i32, i32), margin: u16) -> Rect {
    let m = margin as i32 * CELL;
    Rect {
        x: camera.0 + m,
        y: camera.1 + m,
        w: SCREEN_W as i32,
        h: SCREEN_H as i32,
    }
}

/// The cell the camera's top-left is in: the view re-centres on the camera only when this
/// changes.
pub fn camera_cell(camera: (i32, i32)) -> (i32, i32) {
    (camera.0.div_euclid(CELL), camera.1.div_euclid(CELL))
}

/// Where a connection is drawn: the neighbor's extent along the edge (shifted by `offset`
/// cells), across the margin on that side, clipped to the image. `map` and `dest` are sizes in
/// cells; a neighbor without cells (not exported) spans the whole edge.
pub fn connection_rect(
    dir: Dir,
    offset: i16,
    map: (u16, u16),
    dest: (u16, u16),
    margin: u16,
) -> Rect {
    let m = margin as i32 * CELL;
    let (w, h) = (map.0 as i32 * CELL, map.1 as i32 * CELL);
    let (iw, ih) = (w + 2 * m, h + 2 * m);
    let missing = dest.0 == 0 || dest.1 == 0;
    // (start, length) along the edge.
    let along = |len: i32, edge: i32| {
        if missing {
            (0, edge)
        } else {
            (m + offset as i32 * CELL, len)
        }
    };
    let r = match dir {
        Dir::Up | Dir::Down => {
            let (x, len) = along(dest.0 as i32 * CELL, iw);
            let y = if dir == Dir::Up { 0 } else { m + h };
            Rect { x, y, w: len, h: m }
        }
        Dir::Left | Dir::Right => {
            let (y, len) = along(dest.1 as i32 * CELL, ih);
            let x = if dir == Dir::Left { 0 } else { m + w };
            Rect { x, y, w: m, h: len }
        }
    };
    r.clip(iw, ih)
}

/// The map an overlay opens when clicked: a warp's or connection's destination, unless the
/// game does not have it (`"MISSING"`).
pub fn click_target(o: &Overlay) -> Option<MapId> {
    match *o {
        Overlay::Warp {
            dest, dest_name, ..
        } if dest_name != "MISSING" => Some(dest),
        Overlay::Connection { map, name, .. } if name != "MISSING" => Some(map),
        _ => None,
    }
}

/// One line about an overlay, for the hover panel.
pub fn describe(o: &Overlay) -> String {
    match o {
        Overlay::Object {
            local_id,
            gfx,
            movement,
            range,
            script,
            hide_flag,
            ..
        } => {
            let mut s = format!("object {local_id}: {gfx:?}, {movement:?}");
            if *range != (0, 0) {
                s += &format!(", range {}x{}", range.0, range.1);
            }
            if let Some(sc) = script {
                s += &format!(", script {sc}");
            }
            if let Some(f) = hide_flag {
                s += &format!(", hidden by flag {:#x}", f.0);
            }
            s
        }
        Overlay::Warp {
            index,
            dest_name,
            dest_warp,
            ..
        } => format!("warp {index} -> {dest_name} #{dest_warp}"),
        Overlay::Coord {
            var, value, script, ..
        } => format!("coord: var {:#x} == {value} -> {script}", var.0),
        Overlay::Bg { kind, script, .. } => match kind {
            BgKind::Script { facing, .. } => format!(
                "bg: {} (facing {})",
                script.unwrap_or("?"),
                facing.map_or("any".to_string(), |d| format!("{d:?}"))
            ),
            BgKind::HiddenItem { item, flag } => {
                format!("bg: hidden item {item} (flag {:#x})", flag.0)
            }
        },
        Overlay::Connection {
            dir, offset, name, ..
        } => format!("connection {dir:?} (offset {offset}) -> {name}"),
    }
}

/// The overlay's cell, or `None` for a connection (an edge, not a cell).
pub fn overlay_cell(o: &Overlay) -> Option<(i16, i16)> {
    match *o {
        Overlay::Object { x, y, .. }
        | Overlay::Warp { x, y, .. }
        | Overlay::Coord { x, y, .. }
        | Overlay::Bg { x, y, .. } => Some((x, y)),
        Overlay::Connection { .. } => None,
    }
}

/// The map cell under image pixel `(px, py)`, if it is one of the map's `cols x rows` cells.
pub fn cell_at(px: f64, py: f64, margin: u16, cols: usize, rows: usize) -> Option<(i16, i16)> {
    if px < 0.0 || py < 0.0 {
        return None;
    }
    let x = (px as i64) / CELL as i64 - margin as i64;
    let y = (py as i64) / CELL as i64 - margin as i64;
    ((0..cols as i64).contains(&x) && (0..rows as i64).contains(&y)).then_some((x as i16, y as i16))
}

/// An SVG path covering the cells `keep` selects (one 16x16 square each).
pub fn cells_path(cells: &[CellInfo], margin: u16, keep: impl Fn(&CellInfo) -> bool) -> String {
    let mut d = String::new();
    for c in cells.iter().filter(|c| keep(c)) {
        let r = Rect::cell(c.x, c.y, margin);
        d += &format!("M{} {}h16v16h-16z", r.x, r.y);
    }
    d
}

/// A short name for a behavior, `None` for `Normal` (not drawn).
pub fn behavior_label(b: Behavior) -> Option<String> {
    match b {
        Behavior::Normal => None,
        Behavior::Ledge(d) => Some(format!("ledge {d:?}")),
        Behavior::ArrowWarp(d) => Some(format!("arrow warp {d:?}")),
        other => Some(format!("{other:?}")),
    }
}

/// A distinct color for the `i`-th of `n` groups.
fn hue(i: usize, n: usize) -> String {
    format!("hsl({}, 85%, 55%)", i * 360 / n.max(1))
}

/// Which cell layers to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellLayers {
    pub collision: bool,
    pub elevation: bool,
    pub behavior: bool,
}

/// The cell layers as SVG paths `(fill, d)`: the collision cells; one path per elevation; one
/// per behavior other than `Normal`. Empty groups are left out.
pub fn layer_paths(cells: &[CellInfo], margin: u16, on: CellLayers) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    if on.collision {
        paths.push((
            "rgba(255,40,40,0.45)".to_string(),
            cells_path(cells, margin, |c| c.collision != 0),
        ));
    }
    if on.elevation {
        for lvl in 0..16u8 {
            paths.push((
                hue(lvl as usize, 16),
                cells_path(cells, margin, |c| c.elevation == lvl),
            ));
        }
    }
    if on.behavior {
        let labels: std::collections::BTreeSet<String> = cells
            .iter()
            .filter_map(|c| behavior_label(c.behavior))
            .collect();
        for (i, label) in labels.iter().enumerate() {
            let d = cells_path(cells, margin, |c| {
                behavior_label(c.behavior).as_ref() == Some(label)
            });
            paths.push((hue(i, labels.len()), d));
        }
    }
    paths.retain(|(_, d)| !d.is_empty());
    paths
}

/// `width x height` RGBA8 (`rgba` holds at least `width * height * 4` bytes) as a PNG with
/// fast compression.
pub fn encode_png(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width as u32, height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    // Writing to a Vec fails only on a size mismatch, which the slice below rules out.
    let mut w = enc.write_header().expect("PNG header");
    w.write_image_data(&rgba[..width * height * 4])
        .expect("PNG data");
    w.finish().expect("PNG end");
    out
}

/// An overlay with where it is drawn and, for an object, its sprite.
#[derive(Clone, PartialEq, Debug)]
pub struct Placed {
    pub overlay: Overlay,
    pub rect: Rect,
    /// An object's sprite as PNG (the UI serves it at `/maps/<map>/<index>.png`).
    pub sprite: Option<std::sync::Arc<Vec<u8>>>,
}

/// Everything the Map tab shows for one map.
#[derive(Debug)]
pub struct MapEntry {
    pub map: MapId,
    pub name: &'static str,
    /// Map size in cells.
    pub cols: usize,
    pub rows: usize,
    /// Image size in pixels (map plus [`VIEW_MARGIN`]).
    pub width: usize,
    pub height: usize,
    pub png: std::sync::Arc<Vec<u8>>,
    pub cells: Vec<CellInfo>,
    pub placed: Vec<Placed>,
}

impl MapEntry {
    /// Renders `map` with a [`VIEW_MARGIN`] margin (about 33 ms in a debug build) and places
    /// its overlays.
    pub fn build(game: &impl GameData, map: MapId) -> MapEntry {
        use crate::frames::rgba_from_rgb555;
        use crate::sprite::{initial_facing, object_sprite};
        use std::sync::Arc;
        let img = render_map(game, map, VIEW_MARGIN, &[]);
        let mut rgba = vec![0; img.width * img.height * 4];
        rgba_from_rgb555(&img.rgb555, &mut rgba);
        let m = game.map(map);
        let mg = VIEW_MARGIN as i32 * CELL;
        let placed = overlays(game, map)
            .into_iter()
            .map(|o| {
                let (rect, sprite) = match o {
                    Overlay::Object {
                        x,
                        y,
                        gfx,
                        movement,
                        ..
                    } => {
                        let s = object_sprite(game, gfx, initial_facing(movement));
                        let (tx, ty) = s.top_left(x, y);
                        let colors: Vec<u16> = s.pixels.iter().map(|p| p.unwrap_or(0)).collect();
                        let mut px = vec![0; colors.len() * 4];
                        rgba_from_rgb555(&colors, &mut px);
                        for (p, o) in s.pixels.iter().zip(px.chunks_exact_mut(4)) {
                            if p.is_none() {
                                o[3] = 0;
                            }
                        }
                        let rect = Rect {
                            x: tx + mg,
                            y: ty + mg,
                            w: s.width as i32,
                            h: s.height as i32,
                        };
                        (rect, Some(Arc::new(encode_png(s.width, s.height, &px))))
                    }
                    Overlay::Connection {
                        dir, offset, map, ..
                    } => {
                        let d = game.map(map);
                        let size = (m.width, m.height);
                        let rect =
                            connection_rect(dir, offset, size, (d.width, d.height), VIEW_MARGIN);
                        (rect, None)
                    }
                    ref o => {
                        let (x, y) = overlay_cell(o).unwrap_or_default();
                        (Rect::cell(x, y, VIEW_MARGIN), None)
                    }
                };
                Placed {
                    overlay: o,
                    rect,
                    sprite,
                }
            })
            .collect();
        MapEntry {
            map,
            name: map_name(game, map),
            cols: m.width as usize,
            rows: m.height as usize,
            width: img.width,
            height: img.height,
            png: Arc::new(encode_png(img.width, img.height, &rgba)),
            cells: cells(game, map),
            placed,
        }
    }

    /// The cell at `(x, y)`, `None` outside the map.
    pub fn cell(&self, (x, y): (i16, i16)) -> Option<&CellInfo> {
        if x < 0 || y < 0 || x as usize >= self.cols || y as usize >= self.rows {
            return None;
        }
        self.cells.get(y as usize * self.cols + x as usize)
    }

    /// The overlays on cell `at`.
    pub fn events_at(&self, at: (i16, i16)) -> impl Iterator<Item = &Overlay> {
        self.placed
            .iter()
            .map(|p| &p.overlay)
            .filter(move |o| overlay_cell(o) == Some(at))
    }
}

/// A `/maps` request of the Map tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapRoute {
    /// `/maps/<id>.png`: the map picture.
    Image(MapId),
    /// `/maps/<id>/<n>.png`: the sprite of overlay `n`.
    Sprite(MapId, usize),
}

pub fn parse_map_request(path: &str) -> Option<MapRoute> {
    let path = path.split('?').next()?;
    let rest = path.strip_prefix("/maps/")?.strip_suffix(".png")?;
    match rest.split_once('/') {
        None => Some(MapRoute::Image(MapId(rest.parse().ok()?))),
        Some((m, n)) => Some(MapRoute::Sprite(MapId(m.parse().ok()?), n.parse().ok()?)),
    }
}
