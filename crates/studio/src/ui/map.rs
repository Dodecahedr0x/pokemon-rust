//! Map tab (M6 Task H): the read-only map view and its overlays. The model (placement,
//! hit-testing, routes) is `map_view`'s; this file holds the cache, the asset handler and the
//! components.
//!
//! - Each map's [`MapEntry`] is built once on a background thread and kept in the
//!   [`MapCache`] (context; an LRU of [`CACHE_MAPS`] maps). The `maps` asset handler
//!   ([`serve`]) answers `/maps/<id>.png` with its picture and `/maps/<id>/<n>.png` with the
//!   sprite of overlay `n` (an object).
//! - Overlays are an SVG layer in image pixels over the `<img>`.
//! - "Show live map": [`Map`]'s `live` prop is the native pane's map and camera ([`Live`]).
//!   The camera window is drawn whenever the live map is shown; with the box checked the view
//!   follows the live map and keeps the camera centred.
//! - Re-renders stay cheap: the cell layers and the overlay marks are memoized child
//!   components, so a camera move (60 Hz) or a hover re-renders neither.

use crate::map_view::{
    CellLayers, MapEntry, MapRoute, Overlay, Placed, camera_cell, camera_rect, cell_at,
    click_target, describe, layer_paths, map_list, overlay_cell, parse_map_request, shown_map,
};
pub use crate::map_view::{Live, VIEW_MARGIN as MARGIN};
use content::Emerald;
use dioxus::prelude::*;
use engine::data::MapId;
use std::cell::Cell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

/// Maps kept by the [`MapCache`] (about 1.5 MB of picture each, PNG-compressed less).
pub const CACHE_MAPS: usize = 32;

/// How a map's entry is made (a parameter so tests can make it fail).
type Builder = fn(MapId) -> MapEntry;

fn build_shells(map: MapId) -> MapEntry {
    MapEntry::build(&Emerald::for_shells(), map)
}

#[derive(Default)]
struct CacheInner {
    /// Least recently used first.
    done: Vec<(MapId, Arc<MapEntry>)>,
    /// Maps being built, with the UI tasks waiting for them.
    building: HashMap<MapId, Vec<futures_channel::oneshot::Sender<()>>>,
    /// Maps whose build panicked, with the message.
    failed: HashMap<MapId, String>,
}

struct CacheShared {
    inner: Mutex<CacheInner>,
    /// Signalled when a build ends ([`MapCache::get_blocking`] waits on it).
    ended: Condvar,
    build: Builder,
}

/// Built [`MapEntry`]s by map, shared by the Map tab and the `maps` asset handler (context).
#[derive(Clone)]
pub struct MapCache(Arc<CacheShared>);

impl Default for MapCache {
    fn default() -> Self {
        MapCache::with_builder(build_shells)
    }
}

impl MapCache {
    fn with_builder(build: Builder) -> MapCache {
        MapCache(Arc::new(CacheShared {
            inner: Mutex::default(),
            ended: Condvar::new(),
            build,
        }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheInner> {
        self.0.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn hit(inner: &mut CacheInner, map: MapId) -> Option<Arc<MapEntry>> {
        let i = inner.done.iter().position(|(m, _)| *m == map)?;
        let e = inner.done.remove(i);
        inner.done.push(e.clone());
        Some(e.1)
    }

    pub fn get(&self, map: MapId) -> Option<Arc<MapEntry>> {
        Self::hit(&mut self.lock(), map)
    }

    /// Why `map` could not be built, if its build panicked.
    pub fn error(&self, map: MapId) -> Option<String> {
        self.lock().failed.get(&map).cloned()
    }

    /// Builds `map` on this thread and records the outcome: the entry (evicting the least
    /// recently used beyond [`CACHE_MAPS`]) or the panic. Waiters get `Ok` on success; on a
    /// panic they are dropped, so their receivers resolve `Err`.
    fn run_build(&self, map: MapId) -> Option<Arc<MapEntry>> {
        let build = self.0.build;
        let out = catch_unwind(AssertUnwindSafe(|| build(map))).map(Arc::new);
        let mut inner = self.lock();
        let waiters = inner.building.remove(&map).unwrap_or_default();
        let entry = match out {
            Ok(e) => {
                inner.done.push((map, e.clone()));
                if inner.done.len() > CACHE_MAPS {
                    inner.done.remove(0);
                }
                for w in waiters {
                    let _ = w.send(());
                }
                Some(e)
            }
            Err(p) => {
                let msg = p
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic".into());
                inner.failed.insert(map, msg);
                None
            }
        };
        drop(inner);
        self.0.ended.notify_all();
        entry
    }

    /// Resolves `Ok` once `map` is in the cache (building it on a thread unless a build is
    /// running), `Err` if its build failed.
    pub fn load(&self, map: MapId) -> futures_channel::oneshot::Receiver<()> {
        let (tx, rx) = futures_channel::oneshot::channel();
        let mut inner = self.lock();
        if Self::hit(&mut inner, map).is_some() {
            let _ = tx.send(());
            return rx;
        }
        if inner.failed.contains_key(&map) {
            return rx; // `tx` dropped: Err
        }
        let start = !inner.building.contains_key(&map);
        inner.building.entry(map).or_default().push(tx);
        drop(inner);
        if start {
            let cache = self.clone();
            std::thread::spawn(move || cache.run_build(map));
        }
        rx
    }

    /// The entry, waiting for a running build or building it on this thread; `None` if the
    /// build failed. For the asset handler's thread and tests.
    pub fn get_blocking(&self, map: MapId) -> Option<Arc<MapEntry>> {
        let mut inner = self.lock();
        loop {
            if let Some(e) = Self::hit(&mut inner, map) {
                return Some(e);
            }
            if inner.failed.contains_key(&map) {
                return None;
            }
            if let std::collections::hash_map::Entry::Vacant(v) = inner.building.entry(map) {
                v.insert(Vec::new());
                drop(inner);
                return self.run_build(map);
            }
            inner = self.0.ended.wait(inner).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// The PNG a route names, from `entry`.
fn route_body(entry: &MapEntry, route: MapRoute) -> Option<Arc<Vec<u8>>> {
    match route {
        MapRoute::Image(_) => Some(entry.png.clone()),
        MapRoute::Sprite(_, n) => entry.placed.get(n)?.sprite.clone(),
    }
}

/// The `maps` asset handler: answers a cached map at once, and builds a missing one on a
/// short-lived thread (never on the UI thread).
pub fn serve(
    cache: MapCache,
) -> impl FnMut(dioxus::desktop::AssetRequest, dioxus::desktop::RequestAsyncResponder) + 'static {
    use dioxus::desktop::wry::http::{Response, StatusCode};
    fn respond(body: Option<Arc<Vec<u8>>>) -> Response<Vec<u8>> {
        match body {
            Some(png) => Response::builder()
                .header("Content-Type", "image/png")
                .body(png.to_vec()),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        }
        .unwrap()
    }
    move |req, responder| {
        let Some(route) = parse_map_request(req.uri().path()) else {
            return responder.respond(respond(None));
        };
        let map = match route {
            MapRoute::Image(m) | MapRoute::Sprite(m, _) => m,
        };
        if !content::has_map(map) {
            return responder.respond(respond(None));
        }
        if let Some(e) = cache.get(map) {
            return responder.respond(respond(route_body(&e, route)));
        }
        let cache = cache.clone();
        std::thread::spawn(move || {
            let body = cache.get_blocking(map).and_then(|e| route_body(&e, route));
            responder.respond(respond(body));
        });
    }
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

fn layer_on(layers: [bool; 8], l: Layer) -> bool {
    LAYERS
        .iter()
        .position(|x| x.0 == l)
        .is_some_and(|i| layers[i])
}

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
.map-list input { background: var(--panel); color: var(--fg); border: 1px solid #333; padding: 4px 6px; font: inherit; }
.map-list ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; }
.map-list button { width: 100%; text-align: left; background: none; border: 0; color: var(--fg); padding: 2px 6px; font: inherit; cursor: pointer; }
.map-list button.active { background: var(--accent); color: #000; }
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
.map-error { color: #ff6a6a; }
"#;

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

/// The Map tab. `live` is the native pane's map and camera (see [`Live::from_inspector`]);
/// `None` hides "Show live map".
#[component]
pub fn Map(#[props(default)] live: Option<Live>) -> Element {
    let cache = use_hook(|| {
        try_consume_context::<MapCache>().unwrap_or_else(|| {
            eprintln!(
                "studio: no MapCache in context: the Map tab builds into its own cache, which \
                 the `maps` asset handler does not see"
            );
            MapCache::default()
        })
    });
    let mut selected = use_signal(|| {
        live.map(|l| l.map)
            .or_else(|| map_list().first().map(|m| m.0))
            .unwrap_or(MapId(0))
    });
    let mut follow = use_signal(|| false);
    let mut zoom = use_signal(|| 2i32);
    let mut layers = use_signal(|| LAYERS.map(|l| l.2));
    let mut loaded = use_signal(|| 0u64);
    let requested = use_hook(|| Rc::new(Cell::new(None::<MapId>)));

    let shown = shown_map(selected(), follow(), live);
    let _ = loaded();
    let entry = cache.get(shown);
    let error = cache.error(shown);
    if entry.is_none() && error.is_none() && requested.get() != Some(shown) {
        requested.set(Some(shown));
        let rx = cache.load(shown);
        spawn(async move {
            let _ = rx.await; // Ok or Err (failed build): either way, re-render
            loaded += 1;
        });
    }
    let open = move |m: MapId| {
        follow.set(false);
        selected.set(m);
    };

    rsx! {
        style { {CSS} }
        div { class: "map-tab",
            MapList { shown, on_open: open }
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
                match (entry, error) {
                    (Some(e), _) => rsx! {
                        MapBody {
                            key: "{shown.0}",
                            entry: Shared(e),
                            zoom: zoom(),
                            layers: layers(),
                            camera: live.filter(|l| l.map == shown).map(|l| l.camera),
                            follow: follow(),
                            on_open: open,
                        }
                    },
                    (None, Some(err)) => rsx! {
                        p { class: "map-error", "data-map-error": "1", "This map could not be drawn: {err}" }
                    },
                    (None, None) => rsx! { p { "data-map-loading": "1", "Rendering…" } },
                }
            }
        }
    }
}

/// The map list and its filter.
#[component]
fn MapList(shown: MapId, on_open: EventHandler<MapId>) -> Element {
    let list = use_hook(|| Rc::new(map_list()));
    let mut query = use_signal(String::new);
    let q = query.read().to_lowercase();
    let items: Vec<(MapId, &'static str)> = list
        .iter()
        .filter(|(_, n)| q.is_empty() || n.to_lowercase().contains(&q))
        .copied()
        .collect();
    rsx! {
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
                            onclick: move |_| on_open.call(id),
                            "{name}"
                        }
                    }
                }
            }
        }
    }
}

/// The JS that scrolls the map view so image pixel `center` (at `zoom`) is in the middle.
fn center_js(center: (i32, i32), zoom: i32) -> String {
    format!(
        "requestAnimationFrame(() => {{ const e = document.getElementById('map-scroll'); \
         if (e) e.scrollTo({{ left: {x} - e.clientWidth / 2, top: {y} - e.clientHeight / 2 }}); }});",
        x = center.0 * zoom,
        y = center.1 * zoom,
    )
}

/// The picture, its SVG overlays and the hover panel. Keyed by map, so the hover resets when
/// the map changes.
#[component]
fn MapBody(
    entry: Shared<MapEntry>,
    zoom: i32,
    layers: [bool; 8],
    camera: Option<(i32, i32)>,
    follow: bool,
    on_open: EventHandler<MapId>,
) -> Element {
    let e = entry.0.clone();
    let mut hover = use_signal(|| None::<(i16, i16)>);
    // Re-centre on the camera when following and the camera's cell (or the zoom) changes.
    let centred = use_hook(|| Rc::new(Cell::new(None::<((i32, i32), i32)>)));
    match camera.filter(|_| follow) {
        Some(cam) => {
            let key = (camera_cell(cam), zoom);
            if centred.get() != Some(key) {
                centred.set(Some(key));
                let _ = document::eval(&center_js(camera_rect(cam, MARGIN).center(), zoom));
            }
        }
        None => centred.set(None),
    }
    let mut set_hover = move |c: Option<(i16, i16)>| {
        if *hover.peek() != c {
            hover.set(c);
        }
    };
    let (w, h) = (e.width as i32, e.height as i32);
    let (cols, rows) = (e.cols, e.rows);
    let cell_layers = CellLayers {
        collision: layer_on(layers, Layer::Collision),
        elevation: layer_on(layers, Layer::Elevation),
        behavior: layer_on(layers, Layer::Behavior),
    };
    rsx! {
        div { class: "map-body",
            div { class: "map-scroll", id: "map-scroll",
                div {
                    class: "map-stage",
                    style: "width: {w * zoom}px; height: {h * zoom}px;",
                    img {
                        src: "/maps/{e.map.0}.png",
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
                            set_hover(cell_at(p.x / z, p.y / z, MARGIN, cols, rows));
                        },
                        onmouseleave: move |_| set_hover(None),
                        CellPaths { entry: entry.clone(), on: cell_layers }
                        Marks { entry: entry.clone(), layers, on_hover: set_hover, on_open }
                        if let Some(cam) = camera {
                            CameraRect { camera: cam }
                        }
                    }
                }
            }
            HoverPanel { entry, hover: hover() }
        }
    }
}

/// The collision, elevation and behavior layers (memoized: re-rendered only when the map or
/// the toggles change).
#[component]
fn CellPaths(entry: Shared<MapEntry>, on: CellLayers) -> Element {
    let paths = layer_paths(&entry.0.cells, MARGIN, on);
    let opacity = if on.elevation || on.behavior {
        "0.45"
    } else {
        "1"
    };
    rsx! {
        g { opacity, pointer_events: "none",
            for (fill, d) in paths {
                path { fill, d }
            }
        }
    }
}

/// The live camera's window.
#[component]
fn CameraRect(camera: (i32, i32)) -> Element {
    let r = camera_rect(camera, MARGIN);
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

/// The event overlays (memoized like [`CellPaths`]).
#[component]
fn Marks(
    entry: Shared<MapEntry>,
    layers: [bool; 8],
    on_hover: EventHandler<Option<(i16, i16)>>,
    on_open: EventHandler<MapId>,
) -> Element {
    let e = entry.0.clone();
    let map = e.map.0;
    rsx! {
        for (i, p) in e.placed.iter().enumerate().filter(|(_, p)| layer_on(layers, layer_of(&p.overlay))) {
            {mark(map, i, p, on_hover, on_open)}
        }
    }
}

/// One overlay in the SVG layer.
fn mark(
    map: u16,
    index: usize,
    p: &Placed,
    on_hover: EventHandler<Option<(i16, i16)>>,
    on_open: EventHandler<MapId>,
) -> Element {
    let r = p.rect;
    let target = click_target(&p.overlay);
    let cell = overlay_cell(&p.overlay);
    let (stroke, fill) = match p.overlay {
        Overlay::Object { .. } => ("none", "none"),
        Overlay::Warp { .. } => ("#ff5ad1", "rgba(255,90,209,0.35)"),
        Overlay::Coord { .. } => ("#ffd84a", "rgba(255,216,74,0.30)"),
        Overlay::Bg { .. } => ("#4ac8ff", "rgba(74,200,255,0.30)"),
        Overlay::Connection { .. } => ("#7dff7a", "rgba(125,255,122,0.18)"),
    };
    let class = if target.is_some() { "hit" } else { "" };
    let events = if target.is_some() { "all" } else { "none" };
    if p.sprite.is_some() {
        return rsx! {
            image {
                key: "{index}",
                "data-overlay": "object",
                href: "/maps/{map}/{index}.png",
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
                pointer_events: "none",
                style: "image-rendering: pixelated",
            }
        };
    }
    rsx! {
        rect {
            key: "{index}",
            "data-overlay": format!("{:?}", layer_of(&p.overlay)).to_lowercase(),
            class,
            x: r.x,
            y: r.y,
            width: r.w,
            height: r.h,
            fill,
            stroke,
            stroke_width: 1,
            pointer_events: events,
            onclick: move |_| {
                if let Some(m) = target {
                    on_open.call(m);
                }
            },
            onmousemove: move |ev: MouseEvent| {
                ev.stop_propagation();
                on_hover.call(cell);
            },
        }
    }
}

/// The hovered cell's data and events.
#[component]
fn HoverPanel(entry: Shared<MapEntry>, hover: Option<(i16, i16)>) -> Element {
    let e = entry.0.clone();
    let info = hover.and_then(|at| e.cell(at).map(|c| (at, *c)));
    let events: Vec<String> = hover
        .map(|at| e.events_at(at).map(describe).collect())
        .unwrap_or_default();
    rsx! {
        aside { class: "map-info",
            p { "{e.name}: {e.cols}x{e.rows} cells" }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_view::overlays;
    use std::time::{Duration, Instant};

    fn render(dom: &mut VirtualDom) -> String {
        dom.rebuild_in_place();
        dioxus_ssr::render(dom)
    }

    fn count(html: &str, needle: &str) -> usize {
        html.matches(needle).count()
    }

    /// The map with the most overlays.
    fn busiest_map() -> MapId {
        let game = Emerald::for_shells();
        map_list()
            .into_iter()
            .max_by_key(|&(id, _)| overlays(&game, id).len())
            .unwrap()
            .0
    }

    fn wait(mut done: impl FnMut() -> bool) {
        let t0 = Instant::now();
        while !done() {
            assert!(t0.elapsed() < Duration::from_secs(60), "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_map_tab_lists_every_map_and_draws_its_overlays() {
        let game = Emerald::for_shells();
        let map = busiest_map();
        let cache = MapCache::default();
        let entry = cache.get_blocking(map).unwrap();
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
        let rect = format!(
            "data-live=\"camera\" x={} y={} width=240 height=160",
            r.x, r.y
        );
        assert!(html.contains(&rect), "{html}");
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
        wait(|| cache.get(first).is_some());
    }

    fn panics(_: MapId) -> MapEntry {
        panic!("no picture")
    }

    #[test]
    fn a_failed_build_resolves_its_waiters_and_shows_an_error() {
        let cache = MapCache::with_builder(panics);
        let map = map_list()[0].0;
        let mut rx = cache.load(map);
        wait(|| cache.error(map).is_some());
        assert!(rx.try_recv().is_err(), "the waiter was dropped");
        assert_eq!(cache.error(map).as_deref(), Some("no picture"));
        assert!(cache.get_blocking(map).is_none());
        let mut dom = VirtualDom::new_with_props(Map, MapProps { live: None })
            .with_root_context(cache.clone());
        let html = render(&mut dom);
        assert!(html.contains("data-map-error"), "{html}");
        assert!(!html.contains("data-map-loading"));
    }

    static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn slow_build(map: MapId) -> MapEntry {
        BUILDS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        build_shells(map)
    }

    #[test]
    fn concurrent_requests_share_one_build_and_the_cache_is_bounded() {
        let cache = MapCache::with_builder(slow_build);
        let map = map_list()[0].0;
        let _rx = cache.load(map);
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let c = cache.clone();
                std::thread::spawn(move || c.get_blocking(map).is_some())
            })
            .collect();
        for t in threads {
            assert!(t.join().unwrap());
        }
        assert_eq!(BUILDS.load(std::sync::atomic::Ordering::SeqCst), 1);
        // LRU: insert more than CACHE_MAPS small entries by hand.
        let small = MapCache::default();
        for i in 0..CACHE_MAPS as u16 + 3 {
            let mut inner = small.lock();
            let e = Arc::new(build_shells(MapId(u16::MAX)));
            inner.done.push((MapId(10_000 + i), e));
            if inner.done.len() > CACHE_MAPS {
                inner.done.remove(0);
            }
        }
        assert_eq!(small.lock().done.len(), CACHE_MAPS);
        assert!(small.get(MapId(10_000)).is_none());
        assert!(small.get(MapId(10_000 + CACHE_MAPS as u16 + 2)).is_some());
    }

    #[test]
    fn routes_answer_pictures_and_object_sprites() {
        let cache = MapCache::default();
        let map = busiest_map();
        let e = cache.get_blocking(map).unwrap();
        assert_eq!(route_body(&e, MapRoute::Image(map)), Some(e.png.clone()));
        for (i, p) in e.placed.iter().enumerate() {
            let is_obj = matches!(p.overlay, Overlay::Object { .. });
            assert_eq!(route_body(&e, MapRoute::Sprite(map, i)).is_some(), is_obj);
        }
        assert_eq!(route_body(&e, MapRoute::Sprite(map, e.placed.len())), None);
    }

    #[test]
    fn the_centering_script_targets_the_scroll_view() {
        let js = center_js((100, 50), 2);
        assert!(js.contains("getElementById('map-scroll')"));
        assert!(js.contains("left: 200 - e.clientWidth / 2"));
        assert!(js.contains("top: 100 - e.clientHeight / 2"));
    }
}
