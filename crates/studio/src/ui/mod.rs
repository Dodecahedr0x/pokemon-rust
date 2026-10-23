//! The Dioxus desktop shell (plan: "ui/mod.rs"). Thin: components map events to pane commands
//! and render statuses; logic lives in the core.
//!
//! Frames reach the webview through the `frames` asset handler: `studio.js` pulls
//! `/frames/<pane>?after=<seq>` for every `<canvas data-pane=<pane>>` and the handler answers
//! from the [`Frames`] context (a pane is registered by putting its `FrameSlot` there). The
//! request is a long poll: it is answered as soon as the slot holds a frame whose seq differs
//! from `after` (or with 204 after [`LONG_POLL`]), by the slot's waiter thread
//! ([`FrameSlot::wait_then`]), never blocking the UI thread.
//!
//! The [`Studio`] context holds the paths, the layered store and the two pane threads (the
//! native pane and the ROM pane, each with its own `Materializer` over the store's shared disk
//! cache), spawned once at launch. The [`Shell`] polls their statuses at about 15 Hz into
//! signals ([`Live`]) and keeps the checkpoint tree ([`tree::TreeState`]) fresh. Store work,
//! verifies and ROM builds run on worker threads ([`off_thread`]); the UI thread only sends
//! commands and renders. The Explorer tab's `explore run` lives in the [`Studio`] too
//! ([`Studio::explorer`]), and the shell polls the explorer db
//! ([`explorer::use_explorer_state`]).

pub mod data;
pub mod explorer;
pub mod map;
pub mod play;
pub mod tree;

use crate::build::BuildState;
use crate::explorer::{Children, RunSlot};
use crate::frames::{FrameSlot, parse_frame_request};
use crate::native::{Inspector, NativeCmd, NativePane};
use crate::pane::{self, PaneHandle, RunMode, TestPattern};
use crate::paths::StudioPaths;
use crate::rom::{RomCmd, RomPane, RomSource, RomStatus};
use crate::tree::TreeModel;
use dioxus::desktop::wry::http::{Response, StatusCode};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, use_asset_handler};
use dioxus::prelude::*;
use game_native::store::Store;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CSS: &str = include_str!("studio.css");
const JS: &str = include_str!("studio.js");

pub use crate::native::MATERIALIZER_CAPACITY;
/// How often the UI copies the panes' statuses into its signals (about 15 Hz).
pub const STATUS_POLL: Duration = Duration::from_millis(66);

/// The frame slots served by the `frames` asset handler, by pane name (the canvas's
/// `data-pane`). Held in context.
#[derive(Clone, Default)]
pub struct Frames(pub Arc<HashMap<&'static str, FrameSlot>>);

/// The two play panes, spawned once.
pub struct Panes {
    pub native: PaneHandle<NativeCmd, Inspector>,
    pub rom: PaneHandle<RomCmd, RomStatus>,
}

/// The app's state, in context. `panes` is `None` in the display-free tests (commands are
/// dropped and the statuses stay at their defaults).
#[derive(Clone)]
pub struct Studio {
    pub paths: StudioPaths,
    /// The layered store: the committed tree as a read-only base, personal saves in `local`.
    pub store: Store,
    panes: Option<Arc<Panes>>,
    /// The tree read at start-up, or why it could not be read.
    tree: Result<TreeModel, String>,
    /// The Explorer tab's `explore run` (M7 S2). Kept here, in the app state, so that leaving
    /// the tab keeps the run; quitting stops it ([`on_quit`]), and so does dropping the studio.
    pub explorer: RunSlot,
    /// The Explorer tab's exports (`explore export`), stopped at quit too.
    pub exports: Children,
}

impl Studio {
    /// The state without panes, with the tree read from `store` (a quick read of `nodes/`,
    /// done before the window opens).
    pub fn new(paths: StudioPaths, store: Store) -> Studio {
        let tree = TreeModel::load(&store).map_err(|e| e.to_string());
        Studio {
            paths,
            store,
            panes: None,
            tree,
            explorer: RunSlot::default(),
            exports: Children::default(),
        }
    }

    /// The studio of this checkout: `StudioPaths::detect` and its layered store.
    pub fn detect() -> Result<Studio, String> {
        let paths = StudioPaths::detect();
        let store = Store::layered(&paths.shared, &paths.local).map_err(|e| e.to_string())?;
        Ok(Studio::new(paths, store))
    }

    /// Spawns the native pane (playing a new game) and the ROM pane (booting `target/game.gba`
    /// as a new game; a missing ROM is a status).
    pub fn spawn_panes(mut self) -> Studio {
        let store = self.store.clone();
        let native = pane::spawn("native", move || NativePane::new(store));
        native.send(NativeCmd::Mode(RunMode::Play));
        let (rom_path, scratch) = (self.paths.rom.clone(), self.paths.scratch.clone());
        let store = self.store.clone();
        let rom = pane::spawn("rom", move || RomPane::new(rom_path, scratch, store));
        rom.send(RomCmd::Reset(RomSource::NewGame));
        self.panes = Some(Arc::new(Panes { native, rom }));
        self
    }

    pub fn native(&self, cmd: NativeCmd) {
        if let Some(p) = &self.panes {
            p.native.send(cmd);
        }
    }

    pub fn rom(&self, cmd: RomCmd) {
        if let Some(p) = &self.panes {
            p.rom.send(cmd);
        }
    }

    /// The panes' frame slots, by canvas name.
    pub fn frames(&self) -> Vec<(&'static str, FrameSlot)> {
        self.panes.as_ref().map_or_else(Vec::new, |p| {
            vec![
                ("native", p.native.frames.clone()),
                ("rom", p.rom.frames.clone()),
            ]
        })
    }

    /// The play panes and whether their threads still run (empty without panes).
    pub fn alive(&self) -> Vec<(&'static str, bool)> {
        self.panes.as_ref().map_or_else(Vec::new, |p| {
            vec![
                ("native", !p.native.is_finished()),
                ("rom", !p.rom.is_finished()),
            ]
        })
    }

    fn statuses(&self) -> Option<(Inspector, RomStatus)> {
        let p = self.panes.as_ref()?;
        Some((p.native.status.get(), p.rom.status.get()))
    }
}

/// The window loop's handler that stops the explorer's run and exports when the loop ends.
///
/// tao ends the loop (`LoopDestroyed`) on every quit, Cmd+Q included, and then exits the
/// process without dropping the loop's closure, so the [`Studio`]'s drop never runs there: the
/// `explore` process group would outlive the studio and a later run would make a second writer
/// on the db.
pub fn on_quit<T>(
    run: RunSlot,
    exports: Children,
) -> impl for<'a> FnMut(&dioxus::desktop::tao::event::Event<'a, T>) + 'static {
    move |e| {
        if matches!(e, dioxus::desktop::tao::event::Event::LoopDestroyed) {
            run.stop();
            exports.stop_all();
        }
    }
}

/// The panes' latest statuses (and the ROM build's), polled by the [`Shell`]; in context.
#[derive(Clone, Copy)]
pub struct Live {
    pub native: Signal<Inspector>,
    pub rom: Signal<RomStatus>,
    /// The running or last `cargo xtask rom` (Play tab).
    pub build: Signal<Option<BuildState>>,
}

/// Runs `f` on a new thread and resolves to its result (`None` if it panicked), so the UI
/// thread never waits on the disk, a replay or a child process.
pub async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::Builder::new()
        .name("studio-worker".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .ok()?;
    rx.await.ok()
}

/// Resolves after `d`, on the tokio timer of the runtime dioxus-desktop runs in (no thread per
/// call). Only awaited inside the desktop app.
pub async fn sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// What `studio.js` reports through `/stats/<pane>`. Smoke runs only: it is in context, the
/// `stats` handler registered and the JS reporter on only with `STUDIO_SMOKE_SECS`.
#[derive(Clone, Default)]
pub struct Stats(Arc<Mutex<BTreeMap<String, Reports>>>);

/// A smoke run: how long it lasts, and the reports it prints at the end.
#[derive(Clone)]
struct Smoke {
    secs: f64,
    stats: Stats,
    /// The panes the smoke gate measures (`SMOKE` lines): the transport test patterns. The
    /// play panes only publish changed pictures, so their rate is reported as `INFO` lines.
    gated: Vec<&'static str>,
}

/// A pane's reports, with when each arrived.
type Reports = Vec<(Instant, Report)>;

/// One `/stats` report. Counters are cumulative except `max`, the worst latency since the
/// previous report.
#[derive(Clone, Copy, Debug, Default)]
struct Report {
    /// JS `performance.now()`, ms.
    t: f64,
    drawn: f64,
    /// Sum of request-to-drawn times, ms.
    rtt: f64,
    /// Sum of frame ages when served (publish to response), ms.
    age: f64,
    max: f64,
}

impl Report {
    fn parse(query: &str) -> Option<Report> {
        let mut r = Report::default();
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=')?;
            let v: f64 = v.parse().ok()?;
            match k {
                "t" => r.t = v,
                "drawn" => r.drawn = v,
                "rtt" => r.rtt = v,
                "age" => r.age = v,
                "max" => r.max = v,
                _ => {}
            }
        }
        Some(r)
    }
}

/// The custom head: the stylesheet and the frame pump. `smoke` sets `window.STUDIO_SMOKE`,
/// which turns on the pump's `/stats` reports.
pub fn head(smoke: bool) -> String {
    let flag = if smoke {
        "<script>window.STUDIO_SMOKE = true;</script>"
    } else {
        ""
    };
    format!("<style>{CSS}</style>{flag}<script>{JS}</script>")
}

/// The transport test panes of a smoke run.
const SMOKE_PANES: [&str; 2] = ["smoke-a", "smoke-b"];

/// Opens the studio window. With `STUDIO_SMOKE_SECS=<n>` it also runs two test-pattern panes,
/// and after `n` seconds a thread outside the VirtualDom prints the delivered fps per pane
/// (`SMOKE pane=<name> fps=<f> …` lines for the test patterns, `INFO …` for the play panes)
/// and exits ([`start_smoke_deadline`]).
pub fn launch() {
    let smoke = std::env::var("STUDIO_SMOKE_SECS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|secs| Smoke {
            secs,
            stats: Stats::default(),
            gated: SMOKE_PANES.to_vec(),
        });
    let studio = match Studio::detect() {
        Ok(s) => s.spawn_panes(),
        Err(e) => {
            eprintln!("studio: {e}");
            std::process::exit(1);
        }
    };
    let mut slots: HashMap<&'static str, FrameSlot> = studio.frames().into_iter().collect();
    if smoke.is_some() {
        for name in SMOKE_PANES {
            // The desktop event loop never returns (it exits the process), so the handles may
            // be leaked: the threads live as long as the app.
            let handle = pane::spawn(name, TestPattern::new);
            slots.insert(name, handle.frames.clone());
            std::mem::forget(handle);
        }
    }
    let frames = Frames(Arc::new(slots));
    let window = WindowBuilder::new()
        .with_title("Studio")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_focused(true)
        // WebKit throttles an occluded webview (no animation frames, slow timers), so a smoke
        // run behind other windows would measure nothing.
        .with_always_on_top(smoke.is_some());
    let mut quit = on_quit(studio.explorer.clone(), studio.exports.clone());
    let cfg = Config::new()
        .with_custom_event_handler(move |e, _| quit(e))
        .with_custom_head(head(smoke.is_some()))
        .with_window(window)
        .with_disable_context_menu(true);
    if let Some(smoke) = &smoke {
        start_smoke_deadline(smoke.clone(), frames.clone(), studio.clone());
    }
    let mut app = dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .with_context(frames)
        .with_context(studio);
    if let Some(smoke) = smoke {
        app = app.with_context(smoke);
    }
    app.launch(App);
}

/// Iterations of the [`Shell`]'s status loop: a smoke run reports it, to tell a stalled
/// VirtualDom (dioxus-desktop stops polling its tasks while an edit batch is unacknowledged)
/// from a slow webview.
static VDOM_POLLS: AtomicU64 = AtomicU64::new(0);
/// Set by the smoke deadline thread once it has printed: the VirtualDom then closes the window
/// (a nicety; the thread exits the process anyway).
static SMOKE_DONE: AtomicBool = AtomicBool::new(false);
/// How long after the deadline the watchdog gives the process to exit.
const SMOKE_WATCHDOG: Duration = Duration::from_secs(10);
/// How long the deadline thread lets the window close before it exits the process.
const SMOKE_CLOSE: Duration = Duration::from_millis(300);

/// The smoke deadline, outside the VirtualDom so a stalled webview cannot hold it: after
/// `secs` a thread prints the `SMOKE`/`INFO` lines and exits the process (status 0). A
/// watchdog prints `STALL vdom-not-polled` and exits with status 3 if the process is still
/// alive [`SMOKE_WATCHDOG`] later.
fn start_smoke_deadline(smoke: Smoke, frames: Frames, studio: Studio) {
    let start = Instant::now();
    let secs = Duration::from_secs_f64(smoke.secs);
    std::thread::spawn(move || {
        std::thread::sleep(secs);
        let names: Vec<&str> = frames.0.keys().copied().collect();
        print_smoke(
            &smoke.stats,
            &names,
            &smoke.gated,
            &studio.alive(),
            start,
            smoke.secs,
        );
        println!("INFO vdom_polls={}", VDOM_POLLS.load(Ordering::Relaxed));
        let _ = std::io::Write::flush(&mut std::io::stdout());
        SMOKE_DONE.store(true, Ordering::Relaxed);
        std::thread::sleep(SMOKE_CLOSE);
        std::process::exit(0);
    });
    std::thread::spawn(move || {
        std::thread::sleep(secs + SMOKE_WATCHDOG);
        println!(
            "STALL vdom-not-polled vdom_polls={}",
            VDOM_POLLS.load(Ordering::Relaxed)
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::process::exit(3);
    });
}

/// Steady-state numbers from the reports received after a 2 s warm-up. `alive`: the play
/// panes and whether their threads still run.
fn print_smoke(
    stats: &Stats,
    names: &[&str],
    gated: &[&str],
    alive: &[(&str, bool)],
    start: Instant,
    secs: f64,
) {
    let warm = start + Duration::from_secs_f64((secs / 4.0).min(2.0));
    let all = stats.0.lock().unwrap();
    let mut names = names.to_vec();
    names.sort();
    for name in names {
        let tag = if gated.contains(&name) {
            "SMOKE"
        } else {
            "INFO"
        };
        // Frames drawn since the start (warm-up included), and whether the pane thread lives
        // (the play panes; the test patterns' handles are not kept).
        let total = all
            .get(name)
            .and_then(|v| v.last())
            .map_or(0.0, |(_, r)| r.drawn);
        let alive = alive
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(String::new(), |(_, a)| format!(" alive={a}"));
        let reports: Vec<Report> = all
            .get(name)
            .map(|v| {
                v.iter()
                    .filter(|(at, _)| *at >= warm)
                    .map(|(_, r)| *r)
                    .collect()
            })
            .unwrap_or_default();
        let (Some(first), Some(last)) = (reports.first(), reports.last()) else {
            println!(
                "{tag} pane={name} fps=0.0 latency_ms=nan max_ms=nan total={total}{alive} \
                 (no reports)"
            );
            continue;
        };
        let frames = last.drawn - first.drawn;
        let fps = frames * 1000.0 / (last.t - first.t).max(1.0);
        let fetch = (last.rtt - first.rtt) / frames.max(1.0);
        let age = (last.age - first.age) / frames.max(1.0);
        let max = reports[1..].iter().map(|r| r.max).fold(0.0, f64::max);
        println!(
            "{tag} pane={name} fps={fps:.1} latency_ms={:.2} age_ms={age:.2} fetch_ms={fetch:.2} \
             max_ms={max:.2} frames={frames} total={total} secs={:.1}{alive}",
            age + fetch,
            (last.t - first.t) / 1000.0
        );
    }
}

/// How long a `/frames` request waits for a new frame before answering 204.
const LONG_POLL: Duration = Duration::from_millis(250);

fn respond(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Cache-Control", "no-store")
        .body(Vec::new())
        .unwrap()
}

/// The desktop root: the asset handlers (and in a smoke run, a task that closes the window
/// once the deadline thread has printed), then the [`Shell`].
#[component]
fn App() -> Element {
    let frames = use_context::<Frames>();
    use_asset_handler("frames", move |req, responder| {
        let path = req.uri().path_and_query().map_or("", |p| p.as_str());
        let (pane, after) = match parse_frame_request(path) {
            None => return responder.respond(respond(StatusCode::BAD_REQUEST)),
            Some(r) => r,
        };
        let Some(slot) = frames.0.get(pane) else {
            return responder.respond(respond(StatusCode::NOT_FOUND));
        };
        let t0 = Instant::now();
        slot.wait_then(after, LONG_POLL, move |answer| {
            responder.respond(match answer {
                Err(_) => respond(StatusCode::SERVICE_UNAVAILABLE),
                Ok(None) => respond(StatusCode::NO_CONTENT),
                Ok(Some((seq, rgba, age))) => Response::builder()
                    .header("Content-Type", "application/octet-stream")
                    .header("Cache-Control", "no-store")
                    .header("x-seq", seq)
                    .header("x-age-us", age.as_micros() as u64)
                    .header("x-wait-us", t0.elapsed().as_micros() as u64)
                    .body(rgba.to_vec())
                    .unwrap(),
            })
        });
    });
    // The Map tab's pictures and sprites (`/maps/…`), from the cache it shares (Task H).
    let maps = use_context_provider(map::MapCache::default);
    use_asset_handler("maps", map::serve(maps));
    // The Edit tab's map and palette pictures (`/edit/…`, SIL1 Task C).
    let edits = use_context_provider(crate::map_edit::ui::EditImages::default);
    use_asset_handler("edit", crate::map_edit::ui::serve(edits));
    // The context never changes, so these hooks run on every render or on none.
    if let Some(smoke) = try_use_context::<Smoke>() {
        let stats = smoke.stats.clone();
        use_asset_handler("stats", move |req, responder| {
            let pane = req.uri().path().trim_start_matches("/stats/").to_string();
            if let Some(r) = req.uri().query().and_then(Report::parse) {
                let mut all = stats.0.lock().unwrap();
                all.entry(pane).or_default().push((Instant::now(), r));
            }
            responder.respond(respond(StatusCode::NO_CONTENT));
        });
        // A nicety: close the window once the deadline thread has printed (it exits the
        // process itself, whether or not this task is polled).
        use_future(move || async move {
            while !SMOKE_DONE.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(50)).await;
            }
            dioxus::desktop::window().close();
        });
    }
    rsx! { Shell {} }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Play,
    Checkpoints,
    Map,
    Data,
    Explorer,
    Edit,
}

const TABS: [(Tab, &str); 6] = [
    (Tab::Play, "Play"),
    (Tab::Checkpoints, "Checkpoints"),
    (Tab::Map, "Map"),
    (Tab::Data, "Data"),
    (Tab::Explorer, "Explorer"),
    (Tab::Edit, "Edit"),
];

/// Focuses a play pane (its `data-focus` wrapper) once the Play tab is drawn.
pub fn focus_pane(name: &str) {
    document::eval(&format!(
        "setTimeout(() => document.querySelector('[data-focus=\"{name}\"]')?.focus(), 50)"
    ));
}

/// The tabs and their views; everything but the desktop-only asset handlers (so it renders
/// under `dioxus-ssr`). Needs the [`Studio`] and [`Frames`] contexts; `start` is the first tab.
#[component]
pub fn Shell(#[props(default)] start: Tab) -> Element {
    let studio = use_context::<Studio>();
    let tab = use_context_provider(|| Signal::new(start));
    let live = use_context_provider(|| Live {
        native: Signal::new(Inspector::default()),
        rom: Signal::new(RomStatus::default()),
        build: Signal::new(None),
    });
    let trees = tree::use_tree_state(&studio);
    explorer::use_explorer_state(&studio, tab);
    // The panes' statuses at about 15 Hz; a new save refreshes the tree.
    use_future({
        let studio = studio.clone();
        move || {
            let studio = studio.clone();
            let Live {
                mut native,
                mut rom,
                ..
            } = live;
            async move {
                loop {
                    sleep(STATUS_POLL).await;
                    VDOM_POLLS.fetch_add(1, Ordering::Relaxed);
                    let Some((n, r)) = studio.statuses() else {
                        continue;
                    };
                    if *native.peek() != n {
                        let saved = native.peek().saved != n.saved;
                        native.set(n);
                        if saved {
                            trees.refresh(&studio);
                        }
                    }
                    if *rom.peek() != r {
                        rom.set(r);
                    }
                }
            }
        }
    });
    let mut tab = tab;
    let frames = use_context::<Frames>();
    // Panes other than the play panes: a smoke run's transport test patterns.
    let mut extra: Vec<&'static str> = frames
        .0
        .keys()
        .copied()
        .filter(|n| !["native", "rom"].contains(n))
        .collect();
    extra.sort();
    rsx! {
        nav { class: "tabs",
            for (t, label) in TABS {
                button {
                    class: if tab() == t { "active" } else { "" },
                    onclick: move |_| tab.set(t),
                    "{label}"
                }
            }
        }
        div { class: "content",
            match tab() {
                Tab::Play => rsx! {
                    play::Play {}
                    if !extra.is_empty() {
                        div { class: "panes",
                            for name in extra {
                                PaneView { name, title: name }
                            }
                        }
                    }
                },
                Tab::Checkpoints => rsx! { tree::Tree {} },
                Tab::Map => rsx! { LiveMap {} },
                Tab::Data => rsx! { data::Data {} },
                Tab::Explorer => rsx! { explorer::Explorer {} },
                Tab::Edit => rsx! {
                    crate::map_edit::ui::Edit {
                        games: crate::map_edit::GamesDir::for_game(
                            &studio.paths.workspace,
                            crate::map_edit::games::DEFAULT_GAME,
                        ),
                    }
                },
            }
        }
    }
}

/// The Map tab, following the native pane's map and camera ("Show live map"). Its own
/// component, so the 15 Hz inspector updates re-render only the map view, not the shell.
#[component]
fn LiveMap() -> Element {
    let studio = use_context::<Studio>();
    let live = use_context::<Live>();
    let s = live.native.read();
    let live = map::Live::from_inspector(s.map, s.camera);
    let coverage_db = Some(studio.paths.explore_db.clone());
    rsx! { map::Map { live, coverage_db } }
}

/// A pane's canvas (filled by `studio.js`) and its fps counter. With `keys`, the canvas is
/// focusable and its key events go to the handlers (the play panes).
#[component]
pub fn PaneView(
    name: &'static str,
    title: String,
    #[props(default)] onkeydown: EventHandler<KeyboardEvent>,
    #[props(default)] onkeyup: EventHandler<KeyboardEvent>,
    #[props(default)] onblur: EventHandler<FocusEvent>,
) -> Element {
    rsx! {
        div {
            class: "pane",
            tabindex: "0",
            "data-focus": name,
            onkeydown: move |e: KeyboardEvent| onkeydown.call(e),
            onkeyup: move |e: KeyboardEvent| onkeyup.call(e),
            onfocusout: move |e: FocusEvent| onblur.call(e),
            header {
                span { "{title}" }
                span { "data-fps": name, "– fps" }
            }
            canvas { "data-pane": name, width: "240", height: "160" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh directory (`std::env::temp_dir()` + pid + counter).
    pub(crate) fn temp_root(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pkr-studio-ui-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A studio without panes over the committed tree, with a temp local root.
    pub(crate) fn test_studio(local: &std::path::Path) -> Studio {
        let mut paths = StudioPaths::detect();
        paths.local = local.to_path_buf();
        let store = Store::layered(&paths.shared, &paths.local).unwrap();
        Studio::new(paths, store)
    }

    pub(crate) fn render(studio: Studio, start: Tab) -> String {
        let frames = Frames(Arc::new(HashMap::from([
            ("native", FrameSlot::default()),
            ("rom", FrameSlot::default()),
        ])));
        let mut dom = VirtualDom::new_with_props(Shell, ShellProps { start })
            .with_root_context(frames)
            .with_root_context(studio);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn the_shell_renders_without_a_display() {
        let local = temp_root("shell");
        let html = render(test_studio(&local), Tab::Play);
        for label in ["Play", "Checkpoints", "Map", "Data", "Explorer", "Edit"] {
            assert!(
                html.contains(&format!(">{label}<")),
                "{label} tab in {html}"
            );
        }
        for pane in ["native", "rom"] {
            assert!(
                html.contains(&format!("data-pane=\"{pane}\"")),
                "{pane} canvas in {html}"
            );
            assert!(
                html.contains(&format!("data-fps=\"{pane}\"")),
                "{pane} fps in {html}"
            );
            assert!(
                html.contains(&format!("data-focus=\"{pane}\"")),
                "{pane} is focusable in {html}"
            );
        }
        assert!(!local.exists(), "rendering writes nothing");
    }

    /// The Map, Data and Explorer tabs render inside the shell (the Map tab through the
    /// live-map wrapper, with the coverage toggle; the Explorer tab before its first poll).
    #[test]
    fn the_map_data_and_explorer_tabs_render_in_the_shell() {
        let local = temp_root("tabs");
        for tab in [Tab::Map, Tab::Data, Tab::Explorer, Tab::Edit] {
            let html = render(test_studio(&local), tab);
            assert!(html.contains("class=\"active\">"), "{tab:?}: {html}");
            match tab {
                Tab::Map => assert!(html.contains(">coverage<"), "{html}"),
                Tab::Explorer => assert!(html.contains(">Start<"), "{html}"),
                Tab::Edit => assert!(html.contains(">New map<"), "{html}"),
                _ => {}
            }
        }
        assert!(!local.exists());
    }

    #[test]
    fn the_head_carries_the_css_and_the_frame_pump() {
        let h = head(false);
        assert!(h.contains("image-rendering: pixelated"));
        assert!(h.contains("/frames/${name}?after="));
        assert!(
            !h.contains("STUDIO_SMOKE = true"),
            "no /stats reports by default"
        );
        assert!(head(true).contains("<script>window.STUDIO_SMOKE = true;</script><script>"));
    }

    /// The quit handler stops a running explorer (and its exports) on `LoopDestroyed` only.
    #[cfg(unix)]
    #[test]
    fn quitting_stops_the_explorer_run() {
        use crate::explorer::{ExplorerProc, RunState};
        use dioxus::desktop::tao::event::{Event, StartCause};
        let dir = temp_root("quit");
        let slot = RunSlot::default();
        slot.start_with(|| {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", "sleep 30"]);
            ExplorerProc::spawn(c, &dir.join("explore.log"))
        })
        .unwrap();
        let kids = Children::default();
        let k2 = kids.clone();
        let export = std::thread::spawn(move || {
            let mut c = std::process::Command::new("sleep");
            c.arg("30");
            k2.output(c).unwrap().0
        });
        let t0 = Instant::now();
        while kids.is_empty() {
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "the export never started"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut quit = on_quit::<()>(slot.clone(), kids.clone());
        quit(&Event::NewEvents(StartCause::Init));
        assert_eq!(slot.state(), RunState::Running);
        quit(&Event::LoopDestroyed);
        assert_eq!(slot.state(), RunState::Stopped);
        assert_eq!(export.join().unwrap(), None, "the export was stopped");
        assert!(kids.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stats_reports_parse() {
        let r = Report::parse("t=1.5&drawn=10&rtt=2&age=3&max=4&x=0").unwrap();
        assert_eq!(
            (r.t, r.drawn, r.rtt, r.age, r.max),
            (1.5, 10.0, 2.0, 3.0, 4.0)
        );
        assert!(Report::parse("t=x").is_none());
    }

    #[test]
    fn a_studio_without_panes_drops_commands() {
        let local = temp_root("nopanes");
        let s = test_studio(&local);
        s.native(NativeCmd::Save);
        s.rom(RomCmd::ExportSave);
        assert!(s.frames().is_empty());
        assert!(s.statuses().is_none());
        assert!(!local.exists());
    }

    #[test]
    fn off_thread_resolves_with_the_result_or_none() {
        let v = futures_executor_block_on(off_thread(|| 6 * 7));
        assert_eq!(v, Some(42));
        let p = futures_executor_block_on(off_thread(|| -> u8 { panic!("worker") }));
        assert_eq!(p, None);
    }

    /// A minimal `block_on` (no executor crate): parks the thread until woken.
    pub(crate) fn futures_executor_block_on<F: std::future::Future>(f: F) -> F::Output {
        use std::task::{Context, Poll, Wake, Waker};
        struct Unpark(std::thread::Thread);
        impl Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut f = std::pin::pin!(f);
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
            std::thread::park();
        }
    }
}
