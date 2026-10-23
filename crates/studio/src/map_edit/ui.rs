//! The studio's Edit tab (feature `ui`): thin over [`Session`]. The map picture and the
//! metatile palette are rendered by [`super::render`] into [`EditImages`] and served by the
//! `edit` asset handler at `/edit/map.png` and `/edit/palette.png` (`?r=<rev>` makes the
//! webview fetch a new picture). An SVG layer draws collision, elevation, events and the
//! selection, and takes the mouse.

use super::catalog::Catalog;
use super::games::GamesDir;
use super::model::{Doc, Editor, NewMap};
use super::render::{Palette, palette, png, render};
use super::session::{Mode, Session, Tool, cell_at, cells_path, marks};
use super::validate::{Issue, Severity, id_changes, validate};
use dioxus::prelude::*;
use engine::data::{block_collision, block_elevation};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Cells drawn around the edited map (its border and connections).
pub const MARGIN: u16 = 1;

/// The pictures the `edit` asset handler serves, by file name.
#[derive(Clone, Default)]
pub struct EditImages(pub Arc<Mutex<HashMap<String, Arc<Vec<u8>>>>>);

impl EditImages {
    fn put(&self, name: &str, png: Vec<u8>) {
        self.0.lock().unwrap().insert(name.into(), Arc::new(png));
    }
}

/// The `edit` asset handler: `/edit/<name>` from [`EditImages`].
pub fn serve(
    images: EditImages,
) -> impl FnMut(dioxus::desktop::AssetRequest, dioxus::desktop::RequestAsyncResponder) + 'static {
    use dioxus::desktop::wry::http::{Response, StatusCode};
    move |req, responder| {
        let name = req.uri().path().trim_start_matches("/edit/").to_string();
        let body = images.0.lock().unwrap().get(&name).cloned();
        responder.respond(
            match body {
                Some(png) => Response::builder()
                    .header("Content-Type", "image/png")
                    .header("Cache-Control", "no-store")
                    .body(png.to_vec()),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new()),
            }
            .unwrap(),
        );
    }
}

const CSS: &str = r#"
.edit-tab { display: flex; gap: 12px; height: calc(100vh - 70px); }
.edit-side { width: 240px; display: flex; flex-direction: column; gap: 6px; overflow-y: auto; }
.edit-side ul { list-style: none; margin: 0; padding: 0; }
.edit-tab button { background: var(--panel); color: var(--fg); border: 1px solid #333; font: inherit; cursor: pointer; }
.edit-side li button { width: 100%; text-align: left; border: 0; background: none; }
.edit-tab button.active { border-color: var(--accent); color: var(--accent); }
.edit-tab input, .edit-tab select, .edit-tab textarea { background: var(--panel); color: var(--fg); border: 1px solid #333; font: inherit; }
.edit-tab input { width: 70px; }
.edit-side textarea { width: 100%; height: 170px; font-size: 11px; }
.edit-main { flex: 1; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.edit-toolbar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; color: var(--muted); }
.edit-scroll { overflow: auto; flex: 1; }
.edit-stage { position: relative; user-select: none; }
.edit-stage img { display: block; image-rendering: pixelated; }
.edit-stage svg { position: absolute; left: 0; top: 0; }
.edit-palette { width: 280px; overflow-y: auto; }
.edit-issues { color: var(--muted); font-size: 12px; max-height: 140px; overflow-y: auto; }
.edit-issues .error { color: #ff6a6a; }
.edit-status { color: #ffb86a; }
"#;

/// Everything the tab edits, in one signal.
#[derive(Default)]
struct State {
    session: Option<Session>,
    /// The palette and the tilesets it is for.
    palette: Option<(String, String, Rc<Palette>)>,
    issues: Vec<Issue>,
    /// Bumped on every new map picture.
    rev: u64,
    /// Bumped on every new palette picture.
    palette_rev: u64,
}

impl State {
    /// After a change: the issues, the map picture and, for new tilesets, the palette.
    fn refresh(&mut self, catalog: &Catalog, images: &EditImages) {
        let Some(s) = &self.session else {
            return;
        };
        let d = s.doc();
        self.issues = validate(d, catalog);
        if let Some(before) = s.editor.saved() {
            self.issues.extend(id_changes(before, d));
        }
        match render(d, catalog, MARGIN) {
            Ok(img) => images.put("map.png", png(&img)),
            Err(_) => images.put("map.png", Vec::new()),
        }
        let key = (d.primary.clone(), d.secondary.clone());
        if self.palette.as_ref().map(|p| (&p.0, &p.1)) != Some((&key.0, &key.1)) {
            self.palette = palette(catalog, &key.0, &key.1).ok().map(|p| {
                images.put("palette.png", png(&p.image));
                (key.0, key.1, Rc::new(p))
            });
            self.palette_rev += 1;
        }
        self.rev += 1;
    }
}

/// The new map form's fields.
#[derive(Clone, PartialEq)]
struct NewForm {
    name: String,
    width: String,
    height: String,
    primary: String,
    secondary: String,
    fill: String,
}

impl NewForm {
    fn spec(&self) -> Result<NewMap, String> {
        let num = |what: &str, v: &str| {
            v.trim()
                .parse::<u16>()
                .map_err(|_| format!("{what}: {v:?}"))
        };
        Ok(NewMap {
            name: self.name.trim().to_string(),
            layout: None,
            width: num("width", &self.width)?,
            height: num("height", &self.height)?,
            primary: self.primary.clone(),
            secondary: self.secondary.clone(),
            fill: num("fill", &self.fill)?,
            fill_collision: 0,
            fill_elevation: super::model::GROUND,
            border: None,
            shared_layout: false,
        })
    }
}

/// The layer toggles: metatiles (the picture), collision, elevation, events.
const LAYERS: [&str; 4] = ["metatiles", "collision", "elevation", "events"];

/// A session on `doc`: a saved map's, or (`saved` false) a new map no file holds yet.
fn new_session(doc: Doc, saved: bool, catalog: &Catalog) -> Session {
    let mut s = Session::new(if saved {
        Editor::new(doc)
    } else {
        Editor::unsaved(doc)
    });
    if let Some(g) = catalog.gfx.iter().next() {
        s.default_gfx = g.clone();
    }
    s
}

/// The Edit tab over the maps of `games`, with map `start` open.
#[component]
pub fn Edit(games: GamesDir, #[props(default)] start: Option<String>) -> Element {
    let images = use_hook(|| try_consume_context::<EditImages>().unwrap_or_default());
    let catalog = use_hook(|| {
        Catalog::for_games(&games)
            .map(Rc::new)
            .map_err(|e| format!("{e:#}"))
    });
    let mut list = use_signal(|| games.map_names().unwrap_or_default());
    let mut state = use_signal(|| {
        let mut st = State::default();
        if let (Some(name), Ok(c)) = (&start, &catalog)
            && let Ok(d) = games.load(name)
        {
            st.session = Some(new_session(d, true, c));
            st.refresh(c, &images);
        }
        st
    });
    let mut zoom = use_signal(|| 2i32);
    let mut layers = use_signal(|| [true, true, false, true]);
    let mut inspector = use_signal(String::new);
    let mut conns = use_signal(|| {
        (state.peek().session.as_ref())
            .map(|s| s.connections_ron())
            .unwrap_or_default()
    });
    let mut message = use_signal(String::new);
    let mut form = use_signal(|| NewForm {
        name: String::new(),
        width: "20".into(),
        height: "20".into(),
        primary: String::new(),
        secondary: String::new(),
        fill: "1".into(),
    });
    let catalog = match catalog {
        Ok(c) => c,
        Err(e) => return rsx! { div { class: "map-error", "catalog: {e}" } },
    };
    let primaries = catalog.tileset_names(false);
    let secondaries = catalog.tileset_names(true);

    // A map waiting for its Discard click: the open one has unsaved changes.
    let mut pending = use_signal(|| None::<(Doc, bool)>);
    // Opens `doc` (`saved`: a file holds it) in a new session; with unsaved changes in the
    // open map, only when `discard` (else it waits in `pending`).
    let open = {
        let (catalog, images) = (catalog.clone(), images.clone());
        move |doc: Doc, saved: bool, discard: bool| {
            let mut st = state.write();
            if !discard
                && let Some(s) = st.session.as_ref()
                && s.editor.is_dirty()
            {
                message.set(format!(
                    "{} has unsaved changes: Save them, or Discard them to open {}",
                    s.doc().map.name,
                    doc.map.name
                ));
                pending.set(Some((doc, saved)));
                return;
            }
            pending.set(None);
            message.set(if saved {
                String::new()
            } else {
                "new map: Save writes it".into()
            });
            let s = new_session(doc, saved, &catalog);
            conns.set(s.connections_ron());
            inspector.set(String::new());
            st.session = Some(s);
            st.refresh(&catalog, &images);
        }
    };
    // Runs `f` on the session; refreshes when it reports a change.
    let edit = {
        let (catalog, images) = (catalog.clone(), images.clone());
        move |f: &dyn Fn(&mut Session) -> bool| {
            let mut st = state.write();
            let Some(s) = st.session.as_mut() else {
                return;
            };
            let sel = s.selected;
            let changed = f(s);
            if s.selected != sel || changed {
                inspector.set(s.selected_ron().unwrap_or_default());
            }
            if changed {
                conns.set(s.connections_ron());
                st.refresh(&catalog, &images);
            }
        }
    };

    let st = state.read();
    let z = zoom();
    let on = layers();
    let names = list();
    let current = st.session.as_ref().map(|s| s.doc().map.name.clone());
    let (rev, palette_rev) = (st.rev, st.palette_rev);

    let side = rsx! {
        div { class: "edit-side",
            b { "Maps in {games.root.display()}" }
            ul {
                for name in names {
                    li {
                        button {
                            class: if current.as_deref() == Some(name.as_str()) { "active" } else { "" },
                            onclick: {
                                let (games, open, name) = (games.clone(), open.clone(), name.clone());
                                move |_| match games.load(&name) {
                                    Ok(d) => open.clone()(d, true, false),
                                    Err(e) => message.set(format!("{e:#}")),
                                }
                            },
                            "{name}"
                        }
                    }
                }
            }
            b { "New map" }
            input { placeholder: "Name", value: "{form.read().name}", oninput: move |e| form.write().name = e.value() }
            div {
                input { placeholder: "width", value: "{form.read().width}", oninput: move |e| form.write().width = e.value() }
                " x "
                input { placeholder: "height", value: "{form.read().height}", oninput: move |e| form.write().height = e.value() }
            }
            select {
                onchange: move |e| form.write().primary = e.value(),
                option { value: "", "primary…" }
                for t in primaries {
                    option { value: "{t}", selected: form.read().primary == t, "{t}" }
                }
            }
            select {
                onchange: move |e| form.write().secondary = e.value(),
                option { value: "", "secondary…" }
                for t in secondaries {
                    option { value: "{t}", selected: form.read().secondary == t, "{t}" }
                }
            }
            label { "fill metatile " input { value: "{form.read().fill}", oninput: move |e| form.write().fill = e.value() } }
            button {
                onclick: {
                    let (games, open) = (games.clone(), open.clone());
                    move |_| {
                        let r = form.read().spec().and_then(|s| games.new_doc(&s).map_err(|e| format!("{e:#}")));
                        match r {
                            Ok(d) => open.clone()(d, false, false),
                            Err(e) => message.set(e),
                        }
                    }
                },
                "Create"
            }
            if pending.read().is_some() {
                button {
                    onclick: {
                        let open = open.clone();
                        move |_| {
                            if let Some((d, saved)) = pending.take() {
                                open.clone()(d, saved, true);
                            }
                        }
                    },
                    "Discard changes"
                }
            }
        }
    };

    let Some(s) = st.session.as_ref() else {
        return rsx! {
            style { {CSS} }
            div { class: "edit-tab",
                {side}
                div { class: "edit-main",
                    p { "Pick a map or create one. Maps are saved under {games.root.display()}." }
                    span { class: "edit-status", "{message}" }
                }
            }
        };
    };
    let d = s.doc();
    let (w, h) = (
        (d.width as i32 + 2 * MARGIN as i32) * 16,
        (d.height as i32 + 2 * MARGIN as i32) * 16,
    );
    let m = MARGIN as i32 * 16;
    let tool = s.tool;
    let mode = s.mode;
    let dirty = s.editor.is_dirty();
    let (can_undo, can_redo) = (s.editor.can_undo(), s.editor.can_redo());
    let collision = if on[1] {
        cells_path(d, MARGIN, |b| block_collision(b) != 0)
    } else {
        String::new()
    };
    let elevations: Vec<(i32, i32, u8)> = if on[2] {
        (0..d.height as i32)
            .flat_map(|y| (0..d.width as i32).map(move |x| (x, y)))
            .map(|(x, y)| (x, y, block_elevation(d.block(x, y).unwrap_or_default())))
            .collect()
    } else {
        Vec::new()
    };
    let event_marks = if on[3] { marks(d) } else { Vec::new() };
    let selected = s.selected;
    let preview = s.rect_preview();
    let brush_text = format!(
        "metatile {} · collision {} · elevation {}",
        s.metatile, s.collision, s.elevation
    );
    let status = s.status.clone();
    let issues = st.issues.clone();
    let pal = st.palette.as_ref().map(|p| p.2.clone());
    let pal_cell = pal.as_ref().and_then(|p| p.cell_of(s.metatile));
    let name = d.map.name.clone();
    drop(st);

    let cell = move |e: &MouseEvent| {
        let p = e.element_coordinates();
        let z = z as f64;
        cell_at(p.x / z, p.y / z, MARGIN)
    };
    let mut edit_down = edit.clone();
    let mut edit_move = edit.clone();
    let mut edit_up = edit.clone();
    let mut edit_leave = edit.clone();
    let save = {
        let (games, catalog, images) = (games.clone(), catalog.clone(), images.clone());
        move |_| {
            let mut st = state.write();
            let Some(s) = st.session.as_mut() else {
                return;
            };
            pending.set(None);
            // A new map is created (never replacing a saved map or layout); a saved one saved.
            let r = match s.editor.saved() {
                None => games.create(s.doc()),
                Some(_) => games.save(s.doc()),
            };
            match r {
                Ok(()) => {
                    s.editor.mark_saved();
                    message.set(format!(
                        "saved {}",
                        games.map_path(&s.doc().map.name).display()
                    ));
                    list.set(games.map_names().unwrap_or_default());
                    st.refresh(&catalog, &images);
                }
                Err(e) => message.set(format!("{e:#}")),
            }
        }
    };

    rsx! {
        style { {CSS} }
        div { class: "edit-tab",
            {side}
            section { class: "edit-main",
                div { class: "edit-toolbar",
                    b { "{name}" if dirty { " *" } }
                    for (t, label) in Tool::ALL {
                        button {
                            class: if tool == t { "active" } else { "" },
                            onclick: {
                                let mut edit = edit.clone();
                                move |_| edit(&|s: &mut Session| { s.tool = t; false })
                            },
                            "{label}"
                        }
                    }
                    "| paint"
                    for (md, label) in Mode::ALL {
                        button {
                            class: if mode == md { "active" } else { "" },
                            onclick: {
                                let mut edit = edit.clone();
                                move |_| edit(&|s: &mut Session| { s.mode = md; false })
                            },
                            "{label}"
                        }
                    }
                    label { "collision "
                        input {
                            value: "{brush_collision(&state)}",
                            oninput: {
                                let mut edit = edit.clone();
                                move |e: FormEvent| {
                                    if let Ok(v) = e.value().parse::<u8>() {
                                        edit(&|s: &mut Session| { s.collision = v.min(3); false });
                                    }
                                }
                            },
                        }
                    }
                    label { "elevation "
                        input {
                            value: "{brush_elevation(&state)}",
                            oninput: {
                                let mut edit = edit.clone();
                                move |e: FormEvent| {
                                    if let Ok(v) = e.value().parse::<u8>() {
                                        edit(&|s: &mut Session| { s.elevation = v.min(15); false });
                                    }
                                }
                            },
                        }
                    }
                    span { "{brush_text}" }
                }
                div { class: "edit-toolbar",
                    button { disabled: !can_undo, onclick: { let mut edit = edit.clone(); move |_| edit(&|s: &mut Session| s.history(false)) }, "Undo" }
                    button { disabled: !can_redo, onclick: { let mut edit = edit.clone(); move |_| edit(&|s: &mut Session| s.history(true)) }, "Redo" }
                    button { onclick: save, "Save" }
                    for zz in 1..=4 {
                        button { class: if z == zz { "active" } else { "" }, onclick: move |_| zoom.set(zz), "{zz}x" }
                    }
                    for (i, l) in LAYERS.iter().enumerate() {
                        label {
                            input { r#type: "checkbox", checked: on[i], onchange: move |e| layers.write()[i] = e.checked() }
                            "{l}"
                        }
                    }
                    span { class: "edit-status", "{status} {message}" }
                }
                div { class: "edit-scroll",
                    div { class: "edit-stage", style: "width: {w * z}px; height: {h * z}px;",
                        if on[0] {
                            img { src: "/edit/map.png?r={rev}", width: "{w * z}", height: "{h * z}", draggable: "false" }
                        }
                        svg {
                            width: "{w * z}",
                            height: "{h * z}",
                            view_box: "0 0 {w} {h}",
                            onmousedown: move |e: MouseEvent| {
                                let (x, y) = cell(&e);
                                edit_down(&|s: &mut Session| s.press(x, y));
                            },
                            onmousemove: move |e: MouseEvent| {
                                let (x, y) = cell(&e);
                                edit_move(&|s: &mut Session| s.drag(x, y));
                            },
                            onmouseup: move |e: MouseEvent| {
                                let at = cell(&e);
                                edit_up(&|s: &mut Session| s.release(Some(at)));
                            },
                            onmouseleave: move |_| edit_leave(&|s: &mut Session| s.release(None)),
                            rect { x: "{m}", y: "{m}", width: "{w - 2 * m}", height: "{h - 2 * m}", fill: "none", stroke: "rgba(255,255,255,0.5)", stroke_width: "0.5", pointer_events: "none" }
                            if !collision.is_empty() {
                                path { d: "{collision}", fill: "rgba(255,40,40,0.4)", pointer_events: "none" }
                            }
                            for (x, y, el) in elevations {
                                text { x: "{m + x * 16 + 8}", y: "{m + y * 16 + 11}", font_size: "8", text_anchor: "middle", fill: "#fff", pointer_events: "none", "{el}" }
                            }
                            for mk in event_marks {
                                g { pointer_events: "none",
                                    rect {
                                        x: "{m + mk.x as i32 * 16 + 1}", y: "{m + mk.y as i32 * 16 + 1}", width: "14", height: "14",
                                        fill: "rgba(0,0,0,0.35)",
                                        stroke: if selected == Some((mk.kind, mk.index)) { "#ffd700" } else { "#7fd4ff" },
                                        stroke_width: "1.5",
                                    }
                                    text { x: "{m + mk.x as i32 * 16 + 8}", y: "{m + mk.y as i32 * 16 + 12}", font_size: "10", text_anchor: "middle", fill: "#fff", "{mk.letter}" }
                                }
                            }
                            if let Some((x, y, rw, rh)) = preview {
                                rect { x: "{m + x * 16}", y: "{m + y * 16}", width: "{rw * 16}", height: "{rh * 16}", fill: "rgba(255,215,0,0.25)", stroke: "#ffd700", pointer_events: "none" }
                            }
                        }
                    }
                }
                div { class: "edit-issues",
                    for i in issues {
                        div { class: if i.severity == Severity::Error { "error" } else { "" }, "{i}" }
                    }
                }
            }
            div { class: "edit-palette",
                b { "Palette" }
                if let Some(p) = pal {
                    {
                        let (pw, ph) = (p.image.width as i32, p.image.height as i32);
                        let pz = 2;
                        let p2 = p.clone();
                        let mut edit = edit.clone();
                        rsx! {
                            div { class: "edit-stage", style: "width: {pw * pz}px; height: {ph * pz}px;",
                                img { src: "/edit/palette.png?r={palette_rev}", width: "{pw * pz}", height: "{ph * pz}", draggable: "false" }
                                svg {
                                    width: "{pw * pz}", height: "{ph * pz}", view_box: "0 0 {pw} {ph}",
                                    onclick: move |e: MouseEvent| {
                                        let q = e.element_coordinates();
                                        let (x, y) = cell_at(q.x / pz as f64, q.y / pz as f64, 0);
                                        if let Some(mt) = p2.at(x, y) {
                                            edit(&|s: &mut Session| { s.metatile = mt; s.mode = Mode::Metatile; if !matches!(s.tool, Tool::Pencil | Tool::Rect | Tool::Fill) { s.tool = Tool::Pencil; } false });
                                        }
                                    },
                                    if let Some((x, y)) = pal_cell {
                                        rect { x: "{x * 16}", y: "{y * 16}", width: "16", height: "16", fill: "none", stroke: "#ffd700", stroke_width: "2", pointer_events: "none" }
                                    }
                                }
                            }
                        }
                    }
                }
                b { "Selected event" }
                textarea { value: "{inspector}", oninput: move |e| inspector.set(e.value()) }
                div {
                    button { disabled: selected.is_none(), onclick: { let mut edit = edit.clone(); move |_| { let t = inspector(); edit(&move |s: &mut Session| s.replace_selected(&t)) } }, "Apply" }
                    button { disabled: selected.is_none(), onclick: { let mut edit = edit.clone(); move |_| edit(&|s: &mut Session| s.delete_selected()) }, "Delete" }
                }
                b { "Connections" }
                textarea { value: "{conns}", oninput: move |e| conns.set(e.value()) }
                button { onclick: { let mut edit = edit.clone(); move |_| { let t = conns(); edit(&move |s: &mut Session| s.set_connections(&t)) } }, "Apply connections" }
            }
        }
    }
}

fn brush_collision(state: &Signal<State>) -> u8 {
    state.read().session.as_ref().map_or(0, |s| s.collision)
}

fn brush_elevation(state: &Signal<State>) -> u8 {
    state.read().session.as_ref().map_or(0, |s| s.elevation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_edit(games: GamesDir, start: Option<String>, images: EditImages) -> String {
        let mut dom =
            VirtualDom::new_with_props(Edit, EditProps { games, start }).with_root_context(images);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn fixture() -> GamesDir {
        GamesDir::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/games/fixture"),
        )
    }

    #[test]
    fn the_edit_tab_renders_without_a_display() {
        let dir = std::env::temp_dir().join(format!("pkr-edit-ui-{}", std::process::id()));
        let html = render_edit(GamesDir::new(&dir), None, EditImages::default());
        assert!(html.contains("New map"), "{html}");
        assert!(html.contains("Pick a map or create one"), "{html}");
        assert!(!dir.exists(), "rendering writes nothing");
    }

    /// The fixture open: its tools, its four events, its picture (on the extract, whose
    /// tilesets it uses) and no write.
    #[test]
    fn the_edit_tab_opens_a_map() {
        let images = EditImages::default();
        let before = std::fs::read(fixture().map_path("FixtureTown")).unwrap();
        let html = render_edit(fixture(), Some("FixtureTown".into()), images.clone());
        for t in [
            "pencil",
            "rect",
            "fill",
            "picker",
            "+object",
            "Undo",
            "Save",
            "metatiles",
        ] {
            assert!(html.contains(&format!(">{t}<")), "{t}: {html}");
        }
        assert!(html.contains("/edit/map.png?r=1"), "{html}");
        // Objects, warp, coord, bg events: one mark each (letters O O W C B B).
        assert_eq!(html.matches(">O<").count(), 2, "{html}");
        assert_eq!(html.matches(">B<").count(), 2, "{html}");
        let map_png = images.0.lock().unwrap().get("map.png").cloned().unwrap();
        if content::STUB {
            assert!(map_png.is_empty(), "no Emerald tilesets on the stub");
        } else {
            assert_eq!(&map_png[..4], b"\x89PNG");
            assert!(images.0.lock().unwrap().contains_key("palette.png"));
        }
        assert_eq!(
            std::fs::read(fixture().map_path("FixtureTown")).unwrap(),
            before
        );
    }
}
