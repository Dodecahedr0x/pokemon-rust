//! Map tab (M6 Task H): the read-only map view and its overlays.
//!
//! - The picture is [`render_map`] with a [`MARGIN`]-cell margin, built once per map on a
//!   background thread and kept in the [`MapCache`] (held in context). The `maps` asset handler
//!   ([`serve`]) answers `/maps/<id>.png` with it, and `/maps/<id>/<n>.png` with the sprite of
//!   overlay `n` (an object). Both are uncompressed PNGs ([`png`]): an `<img>` and an SVG
//!   `<image>` can show them, and alpha keeps sprites transparent.
//! - Overlays are an SVG layer in image pixels over the `<img>`. Cell `(x, y)` covers image
//!   pixels `((x + MARGIN) * 16, (y + MARGIN) * 16)` to 16 px further.
//! - "Show live map": the [`Map`] component's `live` prop is the native pane's map and camera
//!   ([`Live`]); the camera window is drawn whenever the live map is shown, and with the box
//!   checked the view follows it.

use crate::frames::rgba_from_rgb555;
use crate::map_view::{CellInfo, Overlay, cells, map_list, map_name, overlays, render_map};
use crate::sprite::{initial_facing, object_sprite};
use content::Emerald;
use dioxus::prelude::*;
use engine::data::{Behavior, BgKind, Dir, GameData, MapId};
use engine::video::{SCREEN_H, SCREEN_W};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Cells drawn around the map on each side (border and connections).
pub const MARGIN: u16 = 8;

const CELL: i32 = 16;

/// The native pane's position, for "Show live map": its map and its camera (the map pixel at
/// the screen's top-left, as `MapImage::crop` takes it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Live {
    pub map: MapId,
    pub camera: (i32, i32),
}

impl Live {
    /// The `live` prop from the inspector's fields: `None` while the pane has no field camera
    /// (battle, starter, no game).
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    fn clip(self, w: i32, h: i32) -> Rect {
        let (x0, y0) = (self.x.clamp(0, w), self.y.clamp(0, h));
        let (x1, y1) = ((self.x + self.w).clamp(0, w), (self.y + self.h).clamp(0, h));
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
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
    let o = offset as i32 * CELL;
    let (dw, dh) = if dest.0 == 0 || dest.1 == 0 {
        (iw, ih)
    } else {
        (dest.0 as i32 * CELL, dest.1 as i32 * CELL)
    };
    let along = |len: i32| if dest.0 == 0 { (0, len) } else { (m + o, len) };
    let r = match dir {
        Dir::Up | Dir::Down => {
            let (x, w) = along(dw);
            let y = if dir == Dir::Up { 0 } else { m + h };
            Rect { x, y, w, h: m }
        }
        Dir::Left | Dir::Right => {
            let (y, h) = along(dh);
            let x = if dir == Dir::Left { 0 } else { m + w };
            Rect { x, y, w: m, h }
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
fn overlay_cell(o: &Overlay) -> Option<(i16, i16)> {
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

/// An overlay with where it is drawn and, for an object, its sprite.
#[derive(Clone, PartialEq, Debug)]
pub struct Placed {
    pub overlay: Overlay,
    pub rect: Rect,
    /// An object's sprite as PNG (served at `/maps/<map>/<index>.png`).
    pub sprite: Option<Arc<Vec<u8>>>,
}

/// Everything the Map tab shows for one map, built off the UI thread.
#[derive(Debug)]
pub struct MapEntry {
    pub map: MapId,
    pub name: &'static str,
    /// Map size in cells.
    pub cols: usize,
    pub rows: usize,
    /// Image size in pixels (map plus margin).
    pub width: usize,
    pub height: usize,
    pub png: Arc<Vec<u8>>,
    pub cells: Vec<CellInfo>,
    pub placed: Vec<Placed>,
}

impl MapEntry {
    /// Renders `map` (about 33 ms in a debug build) and places its overlays.
    pub fn build(game: &impl GameData, map: MapId) -> MapEntry {
        let img = render_map(game, map, MARGIN, &[]);
        let mut rgba = vec![0; img.width * img.height * 4];
        rgba_from_rgb555(&img.rgb555, &mut rgba);
        let m = game.map(map);
        let size = (m.width, m.height);
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
                        let mg = MARGIN as i32 * CELL;
                        let mut px = vec![0; s.pixels.len() * 4];
                        let colors: Vec<u16> = s.pixels.iter().map(|p| p.unwrap_or(0)).collect();
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
                        (rect, Some(Arc::new(png(s.width, s.height, &px))))
                    }
                    Overlay::Connection {
                        dir, offset, map, ..
                    } => {
                        let d = game.map(map);
                        let rect = connection_rect(dir, offset, size, (d.width, d.height), MARGIN);
                        (rect, None)
                    }
                    ref o => {
                        let (x, y) = overlay_cell(o).unwrap_or_default();
                        (Rect::cell(x, y, MARGIN), None)
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
            png: Arc::new(png(img.width, img.height, &rgba)),
            cells: cells(game, map),
            placed,
        }
    }

    pub fn cell(&self, (x, y): (i16, i16)) -> Option<&CellInfo> {
        self.cells.get(y as usize * self.cols + x as usize)
    }

    /// The overlays on cell `(x, y)`.
    pub fn events_at(&self, at: (i16, i16)) -> impl Iterator<Item = &Overlay> {
        self.placed
            .iter()
            .map(|p| &p.overlay)
            .filter(move |o| overlay_cell(o) == Some(at))
    }
}

#[derive(Default)]
struct CacheInner {
    done: HashMap<MapId, Arc<MapEntry>>,
    /// Maps being built, with who waits for them.
    waiting: HashMap<MapId, Vec<futures_channel::oneshot::Sender<()>>>,
}

/// Built [`MapEntry`]s by map, shared by the Map tab and the `maps` asset handler (context).
#[derive(Clone, Default)]
pub struct MapCache(Arc<Mutex<CacheInner>>);

impl MapCache {
    pub fn get(&self, map: MapId) -> Option<Arc<MapEntry>> {
        self.0.lock().unwrap().done.get(&map).cloned()
    }

    /// Resolves once `map` is in the cache, building it on a thread if nobody else is.
    pub fn load(&self, map: MapId) -> futures_channel::oneshot::Receiver<()> {
        let (tx, rx) = futures_channel::oneshot::channel();
        let mut inner = self.0.lock().unwrap();
        if inner.done.contains_key(&map) {
            let _ = tx.send(());
            return rx;
        }
        let waiters = inner.waiting.entry(map).or_default();
        waiters.push(tx);
        if waiters.len() == 1 {
            let cache = self.clone();
            std::thread::spawn(move || {
                let entry = Arc::new(MapEntry::build(&Emerald::for_shells(), map));
                let mut inner = cache.0.lock().unwrap();
                inner.done.insert(map, entry);
                for w in inner.waiting.remove(&map).unwrap_or_default() {
                    let _ = w.send(());
                }
            });
        }
        rx
    }

    /// The entry, built on the calling thread if it is not cached (for the asset handler's
    /// thread and tests).
    pub fn get_blocking(&self, map: MapId) -> Arc<MapEntry> {
        if let Some(e) = self.get(map) {
            return e;
        }
        let entry = Arc::new(MapEntry::build(&Emerald::for_shells(), map));
        self.0
            .lock()
            .unwrap()
            .done
            .entry(map)
            .or_insert(entry)
            .clone()
    }
}

/// A `/maps` request.
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

/// The `maps` asset handler: answers [`MapRoute`]s from `cache` on a short-lived thread (a
/// map not built yet is built there, never on the UI thread).
pub fn serve(
    cache: MapCache,
) -> impl FnMut(dioxus::desktop::AssetRequest, dioxus::desktop::RequestAsyncResponder) + 'static {
    use dioxus::desktop::wry::http::{Response, StatusCode};
    move |req, responder| {
        let route = parse_map_request(req.uri().path());
        let cache = cache.clone();
        std::thread::spawn(move || {
            let body = route.and_then(|r| match r {
                MapRoute::Image(m) => {
                    content::has_map(m).then(|| cache.get_blocking(m).png.clone())
                }
                MapRoute::Sprite(m, n) if content::has_map(m) => {
                    cache.get_blocking(m).placed.get(n)?.sprite.clone()
                }
                MapRoute::Sprite(..) => None,
            });
            let res = match body {
                Some(png) => Response::builder()
                    .header("Content-Type", "image/png")
                    .body(png.to_vec()),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new()),
            };
            responder.respond(res.unwrap());
        });
    }
}

/// `width x height` RGBA as a PNG with stored (uncompressed) deflate blocks: no encoder
/// dependency, about a memcpy of work.
pub fn png(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    let mut raw = Vec::with_capacity(height * (width * 4 + 1));
    for row in rgba
        .chunks_exact((width * 4).max(1))
        .take(height * (width > 0) as usize)
    {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xFFFF).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(b) = blocks.next() {
        z.push(blocks.peek().is_none() as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    };
    !data.iter().fold(!0u32, |c, &b| {
        TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8)
    })
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// The overlay toggles, in toolbar order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Layer {
    Collision,
    Behavior,
    Elevation,
    Warps,
    Objects,
    Coords,
    Bgs,
    Connections,
}

const LAYERS: [(Layer, &str, bool); 8] = [
    (Layer::Collision, "collision", false),
    (Layer::Behavior, "behavior", false),
    (Layer::Elevation, "elevation", false),
    (Layer::Warps, "warps", true),
    (Layer::Objects, "objects", true),
    (Layer::Coords, "coords", true),
    (Layer::Bgs, "bgs", true),
    (Layer::Connections, "connections", true),
];

fn layer_of(o: &Overlay) -> Layer {
    match o {
        Overlay::Object { .. } => Layer::Objects,
        Overlay::Warp { .. } => Layer::Warps,
        Overlay::Coord { .. } => Layer::Coords,
        Overlay::Bg { .. } => Layer::Bgs,
        Overlay::Connection { .. } => Layer::Connections,
    }
}

const CSS: &str = r#"
.map-tab { display: flex; gap: 12px; height: calc(100vh - 70px); }
.map-list { width: 220px; display: flex; flex-direction: column; gap: 6px; }
.map-list input, .data-tab input { background: var(--panel); color: var(--fg); border: 1px solid #333; padding: 4px 6px; font: inherit; }
.map-list ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; }
.map-list button, .data-tab .rows button { width: 100%; text-align: left; background: none; border: 0; color: var(--fg); padding: 2px 6px; font: inherit; cursor: pointer; }
.map-list button.active, .data-tab .rows button.active { background: var(--accent); color: #000; }
.map-main { flex: 1; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.map-toolbar { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; color: var(--muted); }
.map-toolbar button { background: var(--panel); color: var(--fg); border: 1px solid #333; font: inherit; cursor: pointer; }
.map-toolbar button.active { border-color: var(--accent); }
.map-body { display: flex; gap: 12px; flex: 1; min-height: 0; }
.map-scroll { overflow: auto; flex: 1; }
.map-stage { position: relative; }
.map-stage img { display: block; image-rendering: pixelated; }
.map-stage svg { position: absolute; left: 0; top: 0; }
.map-stage svg .hit { cursor: pointer; }
.map-info { width: 260px; color: var(--muted); }
.map-info b { color: var(--fg); }
"#;

/// The Map tab. `live` is the native pane's map and camera (see [`Live::from_inspector`]);
/// `None` hides "Show live map".
#[component]
pub fn Map(#[props(default)] live: Option<Live>) -> Element {
    let cache = use_hook(|| try_consume_context::<MapCache>().unwrap_or_default());
    let list = use_hook(|| Rc::new(map_list()));
    let mut selected = use_signal(|| {
        live.map(|l| l.map)
            .or_else(|| list.first().map(|m| m.0))
            .unwrap_or(MapId(0))
    });
    let mut follow = use_signal(|| false);
    let mut query = use_signal(String::new);
    let mut zoom = use_signal(|| 2i32);
    let mut layers = use_signal(|| LAYERS.map(|l| l.2));
    let mut hover = use_signal(|| None::<(i16, i16)>);
    let mut loaded = use_signal(|| 0u64);
    let requested = use_hook(|| Rc::new(Cell::new(None::<MapId>)));

    let shown = shown_map(selected(), follow(), live);
    let _ = loaded();
    let entry = cache.get(shown);
    if entry.is_none() && requested.get() != Some(shown) {
        requested.set(Some(shown));
        let rx = cache.load(shown);
        spawn(async move {
            let _ = rx.await;
            loaded += 1;
        });
    }
    let mut open = move |m: MapId| {
        follow.set(false);
        hover.set(None);
        selected.set(m);
    };
    let q = query.read().to_lowercase();
    let items: Vec<(MapId, &'static str)> = list
        .iter()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .copied()
        .collect();

    rsx! {
        style { {CSS} }
        div { class: "map-tab",
            aside { class: "map-list",
                input {
                    placeholder: "Filter maps",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                ul {
                    for (id, name) in items {
                        li { key: "{id.0}",
                            button {
                                "data-map-item": id.0,
                                class: if id == shown { "active" } else { "" },
                                onclick: move |_| open(id),
                                "{name}"
                            }
                        }
                    }
                }
            }
            section { class: "map-main",
                div { class: "map-toolbar",
                    b { {entry.as_ref().map_or("…", |e| e.name)} }
                    span {
                        for z in 1..=4 {
                            button {
                                class: if zoom() == z { "active" } else { "" },
                                onclick: move |_| zoom.set(z),
                                "{z}x"
                            }
                        }
                    }
                    for (i, (_, name, _)) in LAYERS.iter().enumerate() {
                        label {
                            input {
                                r#type: "checkbox",
                                checked: layers()[i],
                                onchange: move |e| layers.write()[i] = e.checked(),
                            }
                            "{name}"
                        }
                    }
                    if live.is_some() {
                        label {
                            input {
                                r#type: "checkbox",
                                "data-live-toggle": "1",
                                checked: follow(),
                                onchange: move |e| follow.set(e.checked()),
                            }
                            "Show live map"
                        }
                    }
                }
                match entry {
                    None => rsx! { p { "data-map-loading": true, "Rendering…" } },
                    Some(e) => rsx! {
                        MapBody {
                            entry: Shared(e),
                            zoom: zoom(),
                            layers: layers(),
                            camera: live.filter(|l| l.map == shown).map(|l| l.camera),
                            hover: hover(),
                            on_hover: move |c| hover.set(c),
                            on_open: open,
                        }
                    },
                }
            }
        }
    }
}

/// An `Arc` compared by pointer (a prop).
pub struct Shared<T>(pub Arc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The picture, its SVG overlays and the hover panel.
#[component]
fn MapBody(
    entry: Shared<MapEntry>,
    zoom: i32,
    layers: [bool; 8],
    camera: Option<(i32, i32)>,
    hover: Option<(i16, i16)>,
    on_hover: EventHandler<Option<(i16, i16)>>,
    on_open: EventHandler<MapId>,
) -> Element {
    let e = entry.0.clone();
    let on = |l: Layer| layers[LAYERS.iter().position(|x| x.0 == l).unwrap_or(0)];
    let (w, h) = (e.width as i32, e.height as i32);
    let (cols, rows) = (e.cols, e.rows);
    let map = e.map.0;

    // Cell layers: one path per group.
    let mut paths: Vec<(String, String)> = Vec::new();
    if on(Layer::Collision) {
        paths.push((
            "rgba(255,40,40,0.45)".into(),
            cells_path(&e.cells, MARGIN, |c| c.collision != 0),
        ));
    }
    if on(Layer::Elevation) {
        for lvl in 0..16u8 {
            let d = cells_path(&e.cells, MARGIN, |c| c.elevation == lvl);
            if !d.is_empty() {
                paths.push((hue(lvl as usize, 16), d));
            }
        }
    }
    if on(Layer::Behavior) {
        let mut groups: BTreeMap<String, ()> = BTreeMap::new();
        for c in e.cells.iter() {
            if let Some(l) = behavior_label(c.behavior) {
                groups.insert(l, ());
            }
        }
        let n = groups.len();
        for (i, label) in groups.keys().enumerate() {
            let d = cells_path(&e.cells, MARGIN, |c| {
                behavior_label(c.behavior).as_ref() == Some(label)
            });
            paths.push((hue(i, n), d));
        }
    }
    let layer_opacity = if on(Layer::Elevation) || on(Layer::Behavior) {
        "0.45"
    } else {
        "1"
    };

    let info = hover.and_then(|at| e.cell(at).map(|c| (at, *c)));
    let events: Vec<String> = hover
        .map(|at| e.events_at(at).map(describe).collect())
        .unwrap_or_default();
    let placed: Vec<(usize, Placed)> = e
        .placed
        .iter()
        .cloned()
        .enumerate()
        .filter(|(_, p)| on(layer_of(&p.overlay)))
        .collect();

    rsx! {
        div { class: "map-body",
            div { class: "map-scroll",
                div {
                    class: "map-stage",
                    style: "width: {w * zoom}px; height: {h * zoom}px;",
                    img {
                        src: "/maps/{map}.png",
                        width: "{w * zoom}",
                        height: "{h * zoom}",
                        draggable: "false",
                    }
                    svg {
                        width: "{w * zoom}",
                        height: "{h * zoom}",
                        view_box: "0 0 {w} {h}",
                        onmousemove: move |ev: MouseEvent| {
                            let p = ev.element_coordinates();
                            let z = zoom as f64;
                            on_hover.call(cell_at(p.x / z, p.y / z, MARGIN, cols, rows));
                        },
                        onmouseleave: move |_| on_hover.call(None),
                        g { opacity: layer_opacity, pointer_events: "none",
                            for (fill, d) in paths {
                                path { fill, d }
                            }
                        }
                        for (i, p) in placed {
                            Mark { key: "{i}", map, index: i, placed: p, on_hover, on_open }
                        }
                        if let Some(cam) = camera {
                            {
                                let r = camera_rect(cam, MARGIN);
                                rsx! {
                                    rect {
                                        "data-live": "camera",
                                        x: r.x,
                                        y: r.y,
                                        width: r.w,
                                        height: r.h,
                                        fill: "none",
                                        stroke: "#fff",
                                        stroke_width: 2,
                                        stroke_dasharray: "6 3",
                                        pointer_events: "none",
                                    }
                                }
                            }
                        }
                    }
                }
            }
            aside { class: "map-info",
                p { "{e.name}: {cols}x{rows} cells" }
                if let Some(((x, y), c)) = info {
                    p { b { "({x}, {y})" } }
                    p { "block {c.block:#06x}, metatile {c.metatile:#05x}" }
                    p { "collision {c.collision}, elevation {c.elevation}" }
                    p { "behavior {c.behavior:?} (raw {c.raw_behavior:#04x})" }
                    for ev in events {
                        p { "{ev}" }
                    }
                } else {
                    p { "Hover a cell." }
                }
            }
        }
    }
}

/// One overlay in the SVG layer.
#[component]
fn Mark(
    map: u16,
    index: usize,
    placed: Placed,
    on_hover: EventHandler<Option<(i16, i16)>>,
    on_open: EventHandler<MapId>,
) -> Element {
    let r = placed.rect;
    let target = click_target(&placed.overlay);
    let cell = overlay_cell(&placed.overlay);
    let (stroke, fill) = match placed.overlay {
        Overlay::Object { .. } => ("none", "none"),
        Overlay::Warp { .. } => ("#ff5ad1", "rgba(255,90,209,0.35)"),
        Overlay::Coord { .. } => ("#ffd84a", "rgba(255,216,74,0.30)"),
        Overlay::Bg { .. } => ("#4ac8ff", "rgba(74,200,255,0.30)"),
        Overlay::Connection { .. } => ("#7dff7a", "rgba(125,255,122,0.18)"),
    };
    let class = if target.is_some() { "hit" } else { "" };
    let events = if target.is_some() { "all" } else { "none" };
    let onclick = move |_| {
        if let Some(m) = target {
            on_open.call(m);
        }
    };
    let onmousemove = move |ev: MouseEvent| {
        ev.stop_propagation();
        on_hover.call(cell);
    };
    match placed.sprite {
        Some(_) => rsx! {
            image {
                "data-overlay": "object",
                href: "/maps/{map}/{index}.png",
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
                pointer_events: "none",
                style: "image-rendering: pixelated",
            }
        },
        None => rsx! {
            rect {
                "data-overlay": format!("{:?}", layer_of(&placed.overlay)).to_lowercase(),
                class,
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
                fill,
                stroke,
                stroke_width: 1,
                pointer_events: events,
                onclick,
                onmousemove,
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_adler_match_known_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    /// Inflates stored blocks (all `png` writes).
    fn inflate_stored(z: &[u8]) -> Vec<u8> {
        assert_eq!(&z[..2], &[0x78, 0x01]);
        let (mut i, mut out) = (2, Vec::new());
        loop {
            let last = z[i] & 1;
            assert_eq!(z[i] >> 1, 0, "stored block");
            let len = u16::from_le_bytes([z[i + 1], z[i + 2]]) as usize;
            let nlen = u16::from_le_bytes([z[i + 3], z[i + 4]]);
            assert_eq!(!(len as u16), nlen);
            out.extend_from_slice(&z[i + 5..i + 5 + len]);
            i += 5 + len;
            if last == 1 {
                break;
            }
        }
        assert_eq!(&z[i..], adler32(&out).to_be_bytes());
        out
    }

    #[test]
    fn png_round_trips() {
        // Big enough for several deflate blocks.
        let (w, h) = (300usize, 120usize);
        let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
        let p = png(w, h, &rgba);
        assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
        let mut i = 8;
        let mut kinds = Vec::new();
        let mut idat = Vec::new();
        while i < p.len() {
            let len = u32::from_be_bytes(p[i..i + 4].try_into().unwrap()) as usize;
            let kind = &p[i + 4..i + 8];
            let data = &p[i + 8..i + 8 + len];
            let crc = u32::from_be_bytes(p[i + 8 + len..i + 12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&p[i + 4..i + 8 + len]));
            if kind == b"IHDR" {
                assert_eq!(&data[..4], (w as u32).to_be_bytes());
                assert_eq!(&data[4..8], (h as u32).to_be_bytes());
                assert_eq!(&data[8..], &[8, 6, 0, 0, 0]);
            }
            if kind == b"IDAT" {
                idat.extend_from_slice(data);
            }
            kinds.push(String::from_utf8(kind.to_vec()).unwrap());
            i += 12 + len;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        let raw = inflate_stored(&idat);
        assert_eq!(raw.len(), h * (w * 4 + 1));
        for (y, row) in raw.chunks_exact(w * 4 + 1).enumerate() {
            assert_eq!(row[0], 0);
            assert_eq!(&row[1..], &rgba[y * w * 4..(y + 1) * w * 4]);
        }
        // Empty image: one empty final block.
        let e = png(0, 0, &[]);
        assert!(e.ends_with(&crc32(b"IEND").to_be_bytes()));
    }

    #[test]
    fn routes_parse() {
        assert_eq!(
            parse_map_request("/maps/12.png"),
            Some(MapRoute::Image(MapId(12)))
        );
        assert_eq!(
            parse_map_request("/maps/12/3.png?x=1"),
            Some(MapRoute::Sprite(MapId(12), 3))
        );
        for bad in ["/maps/12", "/maps/x.png", "/frames/12.png", "/maps/1/y.png"] {
            assert_eq!(parse_map_request(bad), None, "{bad}");
        }
    }

    #[test]
    fn cells_and_rects() {
        assert_eq!(
            Rect::cell(0, 0, 8),
            Rect {
                x: 128,
                y: 128,
                w: 16,
                h: 16
            }
        );
        assert_eq!(cell_at(128.0, 128.0, 8, 4, 3), Some((0, 0)));
        assert_eq!(cell_at(128.0 + 63.9, 128.0 + 47.9, 8, 4, 3), Some((3, 2)));
        assert_eq!(cell_at(127.0, 130.0, 8, 4, 3), None);
        assert_eq!(cell_at(128.0 + 64.0, 130.0, 8, 4, 3), None);
        assert_eq!(cell_at(-1.0, 0.0, 0, 4, 3), None);
        assert_eq!(
            camera_rect((-8, 16), 8),
            Rect {
                x: 120,
                y: 144,
                w: 240,
                h: 160
            }
        );
    }

    #[test]
    fn connection_rects_sit_in_the_margin() {
        // A 10x6 map with a 2-cell margin: image 224x160.
        let (map, m) = ((10, 6), 2);
        let up = connection_rect(Dir::Up, 3, map, (4, 9), m);
        assert_eq!(
            up,
            Rect {
                x: 80,
                y: 0,
                w: 64,
                h: 32
            }
        );
        let down = connection_rect(Dir::Down, -1, map, (20, 9), m);
        assert_eq!(
            down,
            Rect {
                x: 16,
                y: 128,
                w: 208,
                h: 32
            }
        );
        let left = connection_rect(Dir::Left, 0, map, (9, 2), m);
        assert_eq!(
            left,
            Rect {
                x: 0,
                y: 32,
                w: 32,
                h: 32
            }
        );
        let right = connection_rect(Dir::Right, 5, map, (9, 9), m);
        assert_eq!(
            right,
            Rect {
                x: 192,
                y: 112,
                w: 32,
                h: 48
            }
        );
        // A neighbor without cells spans the edge.
        let missing = connection_rect(Dir::Up, 3, map, (0, 0), m);
        assert_eq!(
            missing,
            Rect {
                x: 0,
                y: 0,
                w: 224,
                h: 32
            }
        );
    }

    #[test]
    fn missing_destinations_are_not_clickable() {
        let warp = |dest_name| Overlay::Warp {
            index: 0,
            x: 1,
            y: 2,
            dest: MapId(7),
            dest_name,
            dest_warp: 0,
        };
        assert_eq!(click_target(&warp("SomeMap")), Some(MapId(7)));
        assert_eq!(click_target(&warp("MISSING")), None);
        let conn = |name| Overlay::Connection {
            dir: Dir::Up,
            offset: 0,
            map: MapId(3),
            name,
        };
        assert_eq!(click_target(&conn("SomeMap")), Some(MapId(3)));
        assert_eq!(click_target(&conn("MISSING")), None);
        assert!(describe(&warp("SomeMap")).contains("SomeMap"));
    }

    #[test]
    fn the_live_map_is_followed_only_when_asked() {
        let live = Live::from_inspector(MapId(5), Some((16, 32)));
        assert_eq!(
            live,
            Some(Live {
                map: MapId(5),
                camera: (16, 32)
            })
        );
        assert_eq!(Live::from_inspector(MapId(5), None), None);
        assert_eq!(shown_map(MapId(1), true, live), MapId(5));
        assert_eq!(shown_map(MapId(1), false, live), MapId(1));
        assert_eq!(shown_map(MapId(1), true, None), MapId(1));
    }

    fn render(dom: &mut VirtualDom) -> String {
        dom.rebuild_in_place();
        dioxus_ssr::render(dom)
    }

    fn count(html: &str, needle: &str) -> usize {
        html.matches(needle).count()
    }

    /// The map with the most overlays (every kind but connections is drawn in a cell).
    fn busiest_map() -> MapId {
        let game = Emerald::for_shells();
        map_list()
            .into_iter()
            .max_by_key(|&(id, _)| overlays(&game, id).len())
            .unwrap()
            .0
    }

    #[test]
    fn the_map_tab_lists_every_map_and_draws_its_overlays() {
        let game = Emerald::for_shells();
        let map = busiest_map();
        let cache = MapCache::default();
        let entry = cache.get_blocking(map);
        assert_eq!(
            entry.png,
            Arc::new(png(entry.width, entry.height, &{
                let img = render_map(&game, map, MARGIN, &[]);
                let mut v = vec![0; img.width * img.height * 4];
                rgba_from_rgb555(&img.rgb555, &mut v);
                v
            }))
        );
        let n = overlays(&game, map).len();
        assert!(n > 0 || content::STUB, "the slice has events");
        let camera = (16, -32);
        let live = Live::from_inspector(map, Some(camera));
        let mut dom =
            VirtualDom::new_with_props(Map, MapProps { live }).with_root_context(cache.clone());
        let html = render(&mut dom);
        assert_eq!(count(&html, "data-map-item="), map_list().len(), "{html}");
        assert_eq!(count(&html, "data-overlay="), n, "{html}");
        assert!(html.contains(&format!("src=\"/maps/{}.png\"", map.0)));
        let objects = entry
            .placed
            .iter()
            .filter(|p| matches!(p.overlay, Overlay::Object { .. }))
            .count();
        assert_eq!(count(&html, "data-overlay=\"object\""), objects);
        // The live camera is drawn on the live map, and "Show live map" is offered.
        let r = camera_rect(camera, MARGIN);
        assert!(
            html.contains(&format!(
                "data-live=\"camera\" x={} y={} width=240 height=160",
                r.x, r.y
            )),
            "{html}"
        );
        assert!(html.contains("Show live map"));
    }

    #[test]
    fn an_unbuilt_map_renders_off_the_ui_thread() {
        let cache = MapCache::default();
        let mut dom = VirtualDom::new_with_props(Map, MapProps { live: None })
            .with_root_context(cache.clone());
        let html = render(&mut dom);
        assert!(html.contains("data-map-loading"), "{html}");
        assert!(!html.contains("Show live map"));
        // The render's request builds the first map on a thread.
        let first = map_list()[0].0;
        let t0 = std::time::Instant::now();
        while cache.get(first).is_none() {
            assert!(t0.elapsed().as_secs() < 60, "map never built");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn sprites_are_served_for_objects_only() {
        let map = busiest_map();
        let cache = MapCache::default();
        let e = cache.get_blocking(map);
        for p in &e.placed {
            let is_obj = matches!(p.overlay, Overlay::Object { .. });
            assert_eq!(p.sprite.is_some(), is_obj);
            if let Overlay::Object { x, y, .. } = p.overlay {
                // The sprite stands on its cell: bottoms align, centered horizontally.
                let c = Rect::cell(x, y, MARGIN);
                assert_eq!(p.rect.y + p.rect.h, c.y + c.h);
                assert_eq!(p.rect.x + p.rect.w / 2, c.x + 8);
            }
        }
    }

    #[test]
    fn layer_paths_cover_the_selected_cells() {
        let game = Emerald::for_shells();
        let map = busiest_map();
        let cs = cells(&game, map);
        let d = cells_path(&cs, MARGIN, |c| c.collision != 0);
        assert_eq!(
            d.matches('M').count(),
            cs.iter().filter(|c| c.collision != 0).count()
        );
        assert_eq!(cells_path(&cs, MARGIN, |_| false), "");
        assert_eq!(behavior_label(Behavior::Normal), None);
        assert_eq!(
            behavior_label(Behavior::Ledge(Dir::Down)).unwrap(),
            "ledge Down"
        );
    }
}
