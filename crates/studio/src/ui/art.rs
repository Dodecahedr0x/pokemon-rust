//! Art tab: generate one tileset feature or one overworld sprite from the studio.
//!
//! The form (model, prompt, reference images) lives in a shell context, so leaving the tab
//! keeps a run going. The run is `cargo xtask silice gfx`, the same command as the CLI; its
//! output is polled like the ROM build. Pictures are served at `/art/<name>.png`.

use super::{Studio, off_thread, sleep};
use crate::art::{self, ArtState, Kind, Previews};
use crate::build::{BuildState, RomBuild};
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const POLL: Duration = Duration::from_millis(250);
const LOG_LINES: usize = 16;

/// The running or last generation. A newtype so it is not the ROM build's signal.
#[derive(Clone, Copy)]
pub struct ArtLog(pub Signal<Option<BuildState>>);

/// The pictures the `art` asset handler serves (`raw.png`, `prev.png`, `art.png`).
#[derive(Clone, Default)]
pub struct ArtImages(pub Arc<Mutex<HashMap<String, Arc<Vec<u8>>>>>);

impl ArtImages {
    fn set(&self, name: &str, bytes: Option<Vec<u8>>) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match bytes {
            Some(b) => {
                g.insert(name.into(), Arc::new(b));
            }
            None => {
                g.remove(name);
            }
        }
    }

    fn has(&self, name: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(name)
    }
}

/// `/art/<name>` from [`ArtImages`]. The query (`?r=`) only busts the webview cache.
pub fn serve(
    images: ArtImages,
) -> impl FnMut(dioxus::desktop::AssetRequest, dioxus::desktop::RequestAsyncResponder) + 'static {
    use dioxus::desktop::wry::http::{Response, StatusCode};
    move |req, responder| {
        let name = req.uri().path().trim_start_matches("/art/").to_string();
        let body = images
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&name)
            .cloned();
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
.art-tab { display: flex; gap: 12px; height: calc(100vh - 70px); }
.art-side { width: 240px; display: flex; flex-direction: column; gap: 6px; overflow-y: auto; }
.art-side ul { list-style: none; margin: 0; padding: 0; }
.art-tab button, .art-tab select, .art-tab textarea {
  background: var(--panel); color: var(--fg); border: 1px solid #333; font: inherit; cursor: pointer;
}
.art-side li button { width: 100%; text-align: left; border: 0; background: none; cursor: pointer; }
.art-tab button.active { color: var(--accent); }
.art-tab button:disabled { color: var(--muted); cursor: default; }
.art-main { flex: 1; display: flex; flex-direction: column; gap: 8px; min-width: 0; overflow-y: auto; }
.art-main textarea { width: 100%; height: 110px; cursor: text; }
.art-row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.art-hint, .art-note, .art-style { color: var(--muted); font-size: 12px; }
.art-refs { display: flex; flex-wrap: wrap; gap: 4px; }
.art-refs button { font-size: 11px; }
.art-log { margin: 0; max-height: 140px; overflow: auto; color: var(--muted); font-size: 11px; white-space: pre-wrap; }
.art-previews { display: flex; gap: 12px; align-items: flex-start; }
.art-previews figure { margin: 0; }
.art-previews img { max-width: 280px; max-height: 320px; background: #111; }
.art-previews .pixel { image-rendering: pixelated; }
.art-previews figcaption { color: var(--muted); font-size: 11px; }
.art-error { color: #ff6a6a; }
"#;

#[component]
pub fn Art() -> Element {
    let studio = use_context::<Studio>();
    let mut state = use_context::<Signal<ArtState>>();
    let log = use_context::<ArtLog>();
    let images = try_use_context::<ArtImages>().unwrap_or_default();
    let view = use_signal(|| 0u64);
    let mut requested = use_signal(String::new);
    let snap = state.read().clone();
    let key = art::preview_key(&snap);
    let seen = requested.peek().clone();
    if seen != key {
        requested.set(key);
        let workspace = studio.paths.workspace.clone();
        let games = games_dir(&studio);
        let paths = art::preview_paths(&workspace, &games, &snap);
        let images = images.clone();
        let mut view = view;
        spawn(async move {
            let loaded = off_thread(move || load_previews(paths)).await;
            if let Some((prev, raw, art_png)) = loaded {
                images.set("prev.png", prev);
                images.set("raw.png", raw);
                images.set("art.png", art_png);
            }
            view += 1;
        });
    }
    let running = matches!(*log.0.read(), Some(BuildState::Running(_)));
    let ready = snap.target().is_ok() && !snap.prompt.trim().is_empty() && !running;
    let features = snap
        .catalog
        .tileset(&snap.tileset)
        .map(|t| t.features.clone())
        .unwrap_or_default();
    let tilesets: Vec<String> = snap
        .catalog
        .tilesets
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let sprites: Vec<String> = snap
        .catalog
        .sprites
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let style = snap.style().to_string();
    let hint = art::model_hint(&snap.model);
    let lines = log_lines(&log.0.read().clone());
    let rev = view();
    let show = |name: &'static str| images.has(name);
    rsx! {
        style { {CSS} }
        div { class: "art-tab",
            div { class: "art-side",
                div { class: "art-row",
                    button {
                        class: if snap.kind == Kind::Tileset { "active" } else { "" },
                        onclick: move |_| state.write().set_kind(Kind::Tileset),
                        "Tilesets"
                    }
                    button {
                        class: if snap.kind == Kind::Sprite { "active" } else { "" },
                        onclick: move |_| state.write().set_kind(Kind::Sprite),
                        "Sprites"
                    }
                }
                if snap.kind == Kind::Tileset {
                    ul {
                        for name in tilesets {
                            li {
                                button {
                                    class: if snap.tileset == name { "active" } else { "" },
                                    onclick: {
                                        let name = name.clone();
                                        move |_| state.write().select_tileset(&name)
                                    },
                                    "{name}"
                                }
                            }
                        }
                    }
                    ul { "data-art": "features",
                        for f in features {
                            li {
                                button {
                                    class: if snap.feature == f.name { "active" } else { "" },
                                    onclick: {
                                        let name = f.name.clone();
                                        move |_| state.write().select_feature(&name)
                                    },
                                    "{f.name} {f.width}x{f.height}"
                                    if f.mirror.is_some() { " mirror" }
                                }
                            }
                        }
                    }
                } else {
                    ul { "data-art": "sprites",
                        for name in sprites {
                            li {
                                button {
                                    class: if snap.sprite == name { "active" } else { "" },
                                    onclick: {
                                        let name = name.clone();
                                        move |_| state.write().select_sprite(&name)
                                    },
                                    "{name}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "art-main",
                for e in snap.catalog.errors.clone() {
                    p { class: "art-error", "{e}" }
                }
                p { class: "art-style", "{style}" }
                textarea {
                    "data-art": "prompt",
                    title: "Subject for this run. The spec's style and size rules are added. Not written back.",
                    value: "{snap.prompt}",
                    oninput: move |e| state.write().prompt = e.value(),
                }
                div { class: "art-row",
                    select {
                        "data-art": "model",
                        onchange: move |e| state.write().set_model(&e.value()),
                        for (id, label) in art::MODELS {
                            option { value: "{id}", selected: snap.model == *id, "{label}" }
                        }
                    }
                    span { class: "art-hint", "{hint}" }
                }
                div { class: "art-row",
                    button {
                        onclick: move |_| {
                            let picked = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                                .set_title("Reference images")
                                .pick_files()
                                .unwrap_or_default();
                            state.write().add_refs(picked);
                        },
                        "Add reference images"
                    }
                    button {
                        onclick: {
                            let studio = studio.clone();
                            move |_| add_preview(&studio, &mut state, |p| p.raw.clone())
                        },
                        "Use latest raw"
                    }
                    button {
                        onclick: {
                            let studio = studio.clone();
                            move |_| add_preview(&studio, &mut state, |p| p.prev.clone())
                        },
                        "Use previous"
                    }
                    button {
                        onclick: {
                            let studio = studio.clone();
                            move |_| add_preview(&studio, &mut state, |p| p.art.clone())
                        },
                        "Use in-game art"
                    }
                }
                div { class: "art-refs",
                    for (i, r) in snap.refs.iter().cloned().enumerate() {
                        button {
                            title: "Remove",
                            onclick: move |_| {
                                state.write().refs.remove(i);
                            },
                            "{ref_label(&r)} ×"
                        }
                    }
                }
                div { class: "art-row",
                    button {
                        "data-art": "generate",
                        disabled: !ready,
                        onclick: {
                            let studio = studio.clone();
                            move |_| start(&studio, state, log)
                        },
                        if running { "Generating…" } else { "Generate" }
                    }
                    span { class: "art-note", "{snap.note}" }
                }
                if !lines.is_empty() {
                    pre { class: "art-log", "{lines}" }
                }
                div { class: "art-previews",
                    if show("prev.png") {
                        figure {
                            figcaption { "Previous" }
                            img { src: "/art/prev.png?r={rev}" }
                        }
                    }
                    if show("raw.png") {
                        figure {
                            figcaption { "Latest raw" }
                            img { src: "/art/raw.png?r={rev}" }
                        }
                    }
                    if show("art.png") {
                        figure {
                            figcaption { "In game" }
                            img { class: "pixel", src: "/art/art.png?r={rev}" }
                        }
                    }
                }
            }
        }
    }
}

fn games_dir(studio: &Studio) -> PathBuf {
    studio
        .paths
        .workspace
        .join("games")
        .join(crate::map_edit::games::DEFAULT_GAME)
}

fn add_preview(studio: &Studio, state: &mut Signal<ArtState>, pick: impl Fn(&Previews) -> PathBuf) {
    let games = games_dir(studio);
    let Some(paths) = art::preview_paths(&studio.paths.workspace, &games, &state.peek()) else {
        state.write().note = "nothing selected".into();
        return;
    };
    state.write().add_refs([pick(&paths)]);
}

fn load_previews(paths: Option<Previews>) -> (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>) {
    let Some(p) = paths else {
        return (None, None, None);
    };
    (read_png(&p.prev), read_png(&p.raw), read_png(&p.art))
}

fn read_png(path: &std::path::Path) -> Option<Vec<u8>> {
    let b = std::fs::read(path).ok()?;
    b.starts_with(b"\x89PNG").then_some(b)
}

fn ref_label(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn log_lines(state: &Option<BuildState>) -> String {
    let lines = match state {
        Some(BuildState::Running(l) | BuildState::Ok(l) | BuildState::Failed(l)) => l,
        None => return String::new(),
    };
    lines
        .iter()
        .skip(lines.len().saturating_sub(LOG_LINES))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn start(studio: &Studio, mut state: Signal<ArtState>, mut log: ArtLog) {
    if matches!(*log.0.peek(), Some(BuildState::Running(_))) {
        return;
    }
    let snap = state.peek().clone();
    let target = match snap.target() {
        Ok(t) => t,
        Err(e) => {
            log.0.set(Some(BuildState::Failed(vec![format!("{e:#}")])));
            return;
        }
    };
    let games = games_dir(studio);
    let workspace = studio.paths.workspace.clone();
    let cmd = match art::gen_command(
        &workspace,
        &games,
        &snap.model,
        &target,
        &snap.refs,
        &snap.prompt,
    ) {
        Ok(c) => c,
        Err(e) => {
            log.0.set(Some(BuildState::Failed(vec![format!("{e:#}")])));
            return;
        }
    };
    log.0.set(Some(BuildState::Running(Vec::new())));
    spawn_forever(async move {
        let started = off_thread(move || RomBuild::spawn(cmd)).await;
        let mut job = match started {
            Some(Ok(b)) => b,
            Some(Err(e)) => {
                log.0.set(Some(BuildState::Failed(vec![format!(
                    "cargo xtask silice gfx: {e}"
                )])));
                return;
            }
            None => {
                log.0.set(Some(BuildState::Failed(vec![
                    "starting generation failed".into(),
                ])));
                return;
            }
        };
        loop {
            let next = job.poll();
            let done = !matches!(next, BuildState::Running(_));
            if matches!(next, BuildState::Ok(_)) {
                state.write().generation += 1;
                state.write().note = "generated".into();
            }
            log.0.set(Some(next));
            if done {
                break;
            }
            sleep(POLL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::ui::tests::{render, temp_root, test_studio};

    #[test]
    fn the_art_tab_lists_tilesets_models_and_generate() {
        let local = temp_root("art");
        let html = render(test_studio(&local), crate::ui::Tab::Art);
        assert!(html.contains("data-art=\"generate\""), "{html}");
        assert!(html.contains("Ark Seedream"), "{html}");
        assert!(html.contains("Nano Banana"), "{html}");
        assert!(html.contains("Add reference images"), "{html}");
        assert!(html.contains("SilBricoBourg"), "{html}");
        assert!(html.contains(">Sprites<"), "{html}");
    }
}
