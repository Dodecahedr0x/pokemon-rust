//! The Dioxus desktop shell (plan: "ui/mod.rs"). Thin: components map events to pane commands
//! and render statuses; logic lives in the core.
//!
//! Frames reach the webview through the `frames` asset handler: `studio.js` pulls
//! `/frames/<pane>?after=<seq>` for every `<canvas data-pane=<pane>>` and the handler answers
//! from the [`Frames`] context (a pane is registered by putting its `FrameSlot` there). The
//! request is a long poll: it is answered when a frame newer than `after` is published (or with
//! 204 after [`LONG_POLL`]), from a short-lived thread, never blocking the UI thread.

pub mod data;
pub mod map;
pub mod play;
pub mod tree;

use crate::frames::{FrameSlot, parse_frame_request};
use crate::pane::{self, TestPattern};
use dioxus::desktop::wry::http::{Response, StatusCode};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, use_asset_handler};
use dioxus::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CSS: &str = include_str!("studio.css");
const JS: &str = include_str!("studio.js");

/// The frame slots served by the `frames` asset handler, by pane name (the canvas's
/// `data-pane`). Held in context.
#[derive(Clone, Default)]
pub struct Frames(pub Arc<HashMap<&'static str, FrameSlot>>);

/// What `studio.js` reports through `/stats/<pane>` (smoke runs).
#[derive(Clone, Default)]
pub struct Stats(Arc<Mutex<BTreeMap<String, Reports>>>);

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

/// The custom head: the stylesheet and the frame pump.
pub fn head() -> String {
    format!("<style>{CSS}</style><script>{JS}</script>")
}

/// Opens the studio window. With `STUDIO_SMOKE_SECS=<n>` it closes itself after `n` seconds
/// and prints the delivered fps per pane (`SMOKE pane=<name> fps=<f> …` lines).
pub fn launch() {
    let smoke = std::env::var("STUDIO_SMOKE_SECS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok());
    // The spike panes. Their handles live as long as this function, i.e. the app.
    let panes = [
        pane::spawn("a", TestPattern::new),
        pane::spawn("b", TestPattern::new),
    ];
    let frames = Frames(Arc::new(HashMap::from([
        ("a", panes[0].frames.clone()),
        ("b", panes[1].frames.clone()),
    ])));
    let stats = Stats::default();
    if let Some(secs) = smoke {
        let stats = stats.clone();
        let names: Vec<&'static str> = frames.0.keys().copied().collect();
        let start = Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(secs));
            print_smoke(&stats, &names, start, secs);
            std::process::exit(0);
        });
    }
    let window = WindowBuilder::new()
        .with_title("Studio")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_focused(true)
        // WebKit throttles an occluded webview (no animation frames, slow timers), so a smoke
        // run behind other windows would measure nothing.
        .with_always_on_top(smoke.is_some());
    let cfg = Config::new()
        .with_custom_head(head())
        .with_window(window)
        .with_disable_context_menu(true);
    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .with_context(frames)
        .with_context(stats)
        .launch(App);
    drop(panes);
}

/// Steady-state numbers from the reports received after a 2 s warm-up.
fn print_smoke(stats: &Stats, names: &[&str], start: Instant, secs: f64) {
    let warm = start + Duration::from_secs_f64((secs / 4.0).min(2.0));
    let all = stats.0.lock().unwrap();
    let mut names = names.to_vec();
    names.sort();
    for name in names {
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
            println!("SMOKE pane={name} fps=0.0 latency_ms=nan max_ms=nan (no reports)");
            continue;
        };
        let frames = last.drawn - first.drawn;
        let fps = frames * 1000.0 / (last.t - first.t).max(1.0);
        let fetch = (last.rtt - first.rtt) / frames.max(1.0);
        let age = (last.age - first.age) / frames.max(1.0);
        let max = reports[1..].iter().map(|r| r.max).fold(0.0, f64::max);
        println!(
            "SMOKE pane={name} fps={fps:.1} latency_ms={:.2} age_ms={age:.2} fetch_ms={fetch:.2} \
             max_ms={max:.2} frames={frames} secs={:.1}",
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

/// The desktop root: the asset handlers, then the [`Shell`].
#[component]
fn App() -> Element {
    let frames = use_context::<Frames>();
    let stats = use_context::<Stats>();
    use_asset_handler("frames", move |req, responder| {
        let path = req.uri().path_and_query().map_or("", |p| p.as_str());
        let slot = match parse_frame_request(path) {
            None => return responder.respond(respond(StatusCode::BAD_REQUEST)),
            Some((pane, after)) => match frames.0.get(pane) {
                None => return responder.respond(respond(StatusCode::NOT_FOUND)),
                Some(slot) => (slot.clone(), after),
            },
        };
        // Long poll off the UI thread: answer with the next frame as soon as it is published.
        std::thread::spawn(move || {
            let (slot, after) = slot;
            let t0 = Instant::now();
            let latest = slot.wait(after, LONG_POLL);
            let wait = t0.elapsed().as_micros() as u64;
            responder.respond(match latest {
                None => respond(StatusCode::NO_CONTENT),
                Some((seq, rgba, age)) => Response::builder()
                    .header("Content-Type", "application/octet-stream")
                    .header("Cache-Control", "no-store")
                    .header("x-seq", seq)
                    .header("x-age-us", age.as_micros() as u64)
                    .header("x-wait-us", wait)
                    .body(rgba.to_vec())
                    .unwrap(),
            });
        });
    });
    use_asset_handler("stats", move |req, responder| {
        let pane = req.uri().path().trim_start_matches("/stats/").to_string();
        if let Some(r) = req.uri().query().and_then(Report::parse) {
            let mut all = stats.0.lock().unwrap();
            all.entry(pane).or_default().push((Instant::now(), r));
        }
        responder.respond(respond(StatusCode::NO_CONTENT));
    });
    rsx! { Shell {} }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Play,
    Checkpoints,
    Map,
    Data,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Play, "Play"),
    (Tab::Checkpoints, "Checkpoints"),
    (Tab::Map, "Map"),
    (Tab::Data, "Data"),
];

/// The tabs and their views; everything but the desktop-only asset handlers (so it renders
/// under `dioxus-ssr`).
#[component]
pub fn Shell() -> Element {
    let mut tab = use_signal(|| Tab::Play);
    let frames = use_context::<Frames>();
    let mut names: Vec<&'static str> = frames.0.keys().copied().collect();
    names.sort();
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
                    div { class: "panes",
                        for name in names {
                            PaneView { name }
                        }
                    }
                    play::Play {}
                },
                Tab::Checkpoints => rsx! { tree::Tree {} },
                Tab::Map => rsx! { map::Map {} },
                Tab::Data => rsx! { data::Data {} },
            }
        }
    }
}

/// A pane's canvas (filled by `studio.js`) and its fps counter.
#[component]
pub fn PaneView(name: &'static str) -> Element {
    rsx! {
        div { class: "pane",
            header {
                span { "{name}" }
                span { "data-fps": name, "– fps" }
            }
            canvas { "data-pane": name, width: "240", height: "160" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_renders_without_a_display() {
        let frames = Frames(Arc::new(HashMap::from([
            ("native", FrameSlot::default()),
            ("rom", FrameSlot::default()),
        ])));
        let mut dom = VirtualDom::new(Shell).with_root_context(frames);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        for label in ["Play", "Checkpoints", "Map", "Data"] {
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
        }
    }

    #[test]
    fn the_head_carries_the_css_and_the_frame_pump() {
        let h = head();
        assert!(h.contains("image-rendering: pixelated"));
        assert!(h.contains("/frames/${name}?after="));
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
}
