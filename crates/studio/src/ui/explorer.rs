//! Explorer tab (M7 Task S2): start and stop an `explore run`, its live stats, and the findings
//! table with "Open at frame before", "Export repro" and "Copy repro command" per finding.
//!
//! - The run is the [`Studio`]'s [`RunSlot`] (app state: leaving the tab keeps it; closing the
//!   window drops it, which kills the run). Starting, stopping and reading it happen on worker
//!   threads ([`off_thread`]).
//! - The [`ExplorerState`] (context, provided by the [`Shell`](super::Shell)) holds the last
//!   [`poll`]: the db read read-only, the run's state and this build. The shell polls every
//!   [`POLL`] while this tab is shown or a run is going; each poll that changes the db's totals
//!   bumps [`ExplorerState::tick`], which refreshes the Map tab's coverage layer.
//! - The text (stats, rows, run state) and the form's validation are the core's
//!   ([`crate::explorer`]).

use super::{Studio, Tab, focus_pane, off_thread, sleep};
use crate::explorer::{
    self, BudgetKind, OPEN_BEFORE, Poll, RunForm, RunSlot, RunState, export_repro, finding_cells,
    nodes_per_sec, open_finding_cmds, opened_text, parse_before, poll, repro_command,
    run_state_text, stats_rows,
};
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use engine::checkpoint::BuildHash;
use explore::db::FindingRow;
use std::time::{Duration, Instant};

/// How often the db and the run are polled (while the tab is shown or a run is going).
pub const POLL: Duration = Duration::from_secs(2);

/// The last poll, with the node rate since the one before.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Polled {
    /// `None` until the first poll.
    pub poll: Option<Poll>,
    pub rate: Option<f64>,
    pub at: Option<Instant>,
}

impl Polled {
    pub fn running(&self) -> bool {
        self.poll.as_ref().is_some_and(|p| p.run.running())
    }
}

/// The Explorer tab's signals, in context.
#[derive(Clone, Copy)]
pub struct ExplorerState {
    pub polled: Signal<Polled>,
    /// Bumped by each poll that changes the db's totals (the Map tab refetches coverage).
    pub tick: Signal<u64>,
}

impl ExplorerState {
    /// Records poll `p` taken at `at`: the rate since the previous poll, and a tick when the
    /// totals changed.
    pub fn apply(mut self, p: Poll, at: Instant) {
        let prev = self.polled.peek().clone();
        let stats = |p: &Option<Poll>| {
            p.as_ref()
                .and_then(|p| p.model.as_ref().ok())
                .map(|m| m.stats.clone())
        };
        let (before, now) = (stats(&prev.poll), stats(&Some(p.clone())));
        let rate = match (&before, &now, prev.at) {
            (Some(b), Some(n), Some(t)) => nodes_per_sec(b.nodes, n.nodes, at - t),
            _ => None,
        };
        if before != now {
            self.tick += 1;
        }
        self.polled.set(Polled {
            poll: Some(p),
            rate,
            at: Some(at),
        });
    }

    /// Polls once on a worker thread and records it. Runs in the root scope, so leaving the tab
    /// does not cancel it.
    pub fn refresh(self, studio: &Studio) {
        let (db, run) = (studio.paths.explore_db.clone(), studio.explorer.clone());
        spawn_forever(async move {
            if let Some(p) = off_thread(move || poll(&db, &run)).await {
                self.apply(p, Instant::now());
            }
        });
    }
}

/// Provides the [`ExplorerState`] and polls every [`POLL`] while `tab` is the Explorer or a
/// run is going.
pub fn use_explorer_state(studio: &Studio, tab: Signal<Tab>) -> ExplorerState {
    let state = use_context_provider(|| ExplorerState {
        polled: Signal::new(Polled::default()),
        tick: Signal::new(0),
    });
    use_future({
        let studio = studio.clone();
        move || {
            let (db, run) = (studio.paths.explore_db.clone(), studio.explorer.clone());
            async move {
                loop {
                    if *tab.peek() == Tab::Explorer || state.polled.peek().running() {
                        let (db, run) = (db.clone(), run.clone());
                        if let Some(p) = off_thread(move || poll(&db, &run)).await {
                            state.apply(p, Instant::now());
                        }
                    }
                    sleep(POLL).await;
                }
            }
        }
    });
    state
}

const CSS: &str = r#"
.explorer { display: flex; flex-direction: column; gap: 8px; }
.explorer .toolbar input { width: 7em; background: var(--panel); color: var(--fg); border: 1px solid #343944; padding: 2px 4px; font: inherit; }
.explorer .toolbar select { background: var(--panel); color: var(--fg); border: 1px solid #343944; font: inherit; }
.explorer .run-state { color: var(--muted); }
.explorer .run-state.running { color: #7dff7a; }
.explorer .muted { color: var(--muted); }
.findings td { vertical-align: top; }
.findings td.message { max-width: 380px; overflow-wrap: anywhere; }
.findings tr.other-build td.build { color: #ffb347; }
.findings tr.gone, .findings tr.stale { opacity: 0.65; }
.findings .actions { white-space: nowrap; }
.findings .actions input { width: 4.5em; background: var(--panel); color: var(--fg); border: 1px solid #343944; padding: 1px 4px; font: inherit; }
.findings tr.note td { color: var(--muted); padding-bottom: 6px; }
.findings tr.note td.error { color: #ff6a6a; background: none; }
"#;

/// The JS that copies `text` to the clipboard (with a fallback where `navigator.clipboard` is
/// not offered to the page).
fn copy_js(text: &str) -> String {
    let t = format!("{text:?}"); // ASCII commands: Rust's escaping is JS's
    format!(
        "(async () => {{ try {{ await navigator.clipboard.writeText({t}); }} catch (_) {{ \
         const a = document.createElement('textarea'); a.value = {t}; document.body.appendChild(a); \
         a.select(); document.execCommand('copy'); a.remove(); }} }})()"
    )
}

#[component]
pub fn Explorer() -> Element {
    let studio = use_context::<Studio>();
    let state = use_context::<ExplorerState>();
    let mut form = use_signal(RunForm::default);
    let mut busy = use_signal(|| false);
    // A fresh poll when the tab opens (the shell's loop may be sleeping).
    use_hook(|| state.refresh(&studio));
    let polled = state.polled.read();
    let run = polled
        .poll
        .as_ref()
        .map_or(RunState::Idle, |p| p.run.clone());
    let running = run.running();
    let form_error = form.read().args(&studio.paths.explore_db).err();
    let form_ok = form_error.is_none();
    let start = {
        let studio = studio.clone();
        move |_| {
            let Ok(args) = form.peek().args(&studio.paths.explore_db) else {
                return;
            };
            busy.set(true);
            let (slot, log) = (studio.explorer.clone(), studio.paths.explore_log.clone());
            let studio = studio.clone();
            spawn(async move {
                let _ = off_thread(move || slot.start(&args, &log)).await;
                busy.set(false);
                state.refresh(&studio);
            });
        }
    };
    let stop = {
        let studio = studio.clone();
        move |_| {
            busy.set(true);
            let slot: RunSlot = studio.explorer.clone();
            let studio = studio.clone();
            spawn(async move {
                let _ = off_thread(move || slot.stop()).await;
                busy.set(false);
                state.refresh(&studio);
            });
        }
    };
    let state_class = if running {
        "run-state running"
    } else {
        "run-state"
    };
    let run_text = run_state_text(&run);
    let db = studio.paths.explore_db.display().to_string();
    rsx! {
        style { {CSS} }
        div { class: "explorer",
            div { class: "toolbar",
                label { "budget "
                    input {
                        "data-field": "budget",
                        value: "{form.read().budget}",
                        oninput: move |e| form.write().budget = e.value(),
                    }
                }
                select {
                    "data-field": "kind",
                    onchange: move |e| {
                        if let Some(k) = BudgetKind::from_key(&e.value()) {
                            form.write().kind = k;
                        }
                    },
                    for k in BudgetKind::ALL {
                        option { value: k.key(), selected: form.read().kind == k, "{k.label()}" }
                    }
                }
                label { "threads "
                    input {
                        "data-field": "threads",
                        placeholder: "default",
                        value: "{form.read().threads}",
                        oninput: move |e| form.write().threads = e.value(),
                    }
                }
                label { "seed "
                    input {
                        "data-field": "seed",
                        placeholder: "config",
                        value: "{form.read().seed}",
                        oninput: move |e| form.write().seed = e.value(),
                    }
                }
                button {
                    disabled: running || busy() || !form_ok,
                    title: "cargo run -p explore --release -- run … (a first run builds explore)",
                    onclick: start,
                    "Start"
                }
                button { disabled: !running || busy(), onclick: stop, "Stop" }
                span { class: "{state_class}", "data-run-state": "1", "{run_text}" }
            }
            if let Some(e) = form_error {
                div { class: "error", "{e}" }
            }
            if let RunState::Failed { status, tail } = &run {
                details { class: "build", open: true,
                    summary { "explore run failed ({status}): {studio.paths.explore_log.display()}" }
                    pre { {tail.join("\n")} }
                }
            }
            match &polled.poll {
                None => rsx! { p { class: "muted", "Reading {db}…" } },
                Some(Poll { model: Err(e), .. }) => rsx! {
                    p { class: "muted", "data-explorer-error": "1", "{e}" }
                },
                Some(Poll { model: Ok(m), build, .. }) => {
                    let rows = stats_rows(m, polled.rate);
                    let this_build = *build;
                    rsx! {
                        dl { class: "inspector", "data-explorer-stats": "1",
                            for (k, v) in rows {
                                dt { "{k}" } dd { "{v}" }
                            }
                        }
                        h3 { "Findings" }
                        if m.findings.is_empty() {
                            p { class: "muted", "No findings." }
                        } else {
                            table { class: "findings",
                                thead {
                                    tr {
                                        th { "id" }
                                        th { "kind" }
                                        th { "map" }
                                        th { "cell" }
                                        th { "message" }
                                        th { "hits" }
                                        th { "status" }
                                        th { "build" }
                                        th { "" }
                                    }
                                }
                                tbody {
                                    for f in m.findings.iter().cloned() {
                                        Finding { key: "{f.id}", row: f, this_build }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One finding and its actions; a note row under it shows an action's outcome.
#[component]
fn Finding(row: FindingRow, this_build: BuildHash) -> Element {
    let studio = use_context::<Studio>();
    let mut tab = use_context::<Signal<Tab>>();
    let mut before = use_signal(|| OPEN_BEFORE.to_string());
    let mut note = use_signal(|| None::<Result<String, String>>);
    let mut busy = use_signal(|| false);
    let c = finding_cells(&row, this_build);
    let id = row.id;
    let class = format!(
        "finding {}{}",
        c.status,
        if c.other_build { " other-build" } else { "" }
    );
    let open = {
        let studio = studio.clone();
        move |_| {
            let b = match parse_before(&before.peek()) {
                Ok(b) => b,
                Err(e) => return note.set(Some(Err(e))),
            };
            busy.set(true);
            note.set(Some(Ok("replaying the finding…".into())));
            let db = studio.paths.explore_db.clone();
            let studio = studio.clone();
            spawn(async move {
                let r = off_thread(move || {
                    explorer::finding_trace(&explorer::open(&db)?, id, b)
                        .map_err(|e| e.context(format!("finding {id}")))
                })
                .await;
                busy.set(false);
                match r {
                    Some(Ok(ft)) => {
                        note.set(Some(Ok(opened_text(&ft))));
                        for cmd in open_finding_cmds(ft) {
                            studio.native(cmd);
                        }
                        tab.set(Tab::Play);
                        focus_pane("native");
                    }
                    Some(Err(e)) => note.set(Some(Err(format!("open failed: {e:#}")))),
                    None => note.set(Some(Err("open failed (panic)".into()))),
                }
            });
        }
    };
    let export = {
        let studio = studio.clone();
        move |_| {
            busy.set(true);
            note.set(Some(Ok(
                "exporting (cargo run -p explore --release -- export)…".into(),
            )));
            let db = studio.paths.explore_db.clone();
            spawn(async move {
                let r = off_thread(move || export_repro(id, &db)).await;
                busy.set(false);
                note.set(Some(match r {
                    Some(Ok(out)) => Ok(format!("exported: {out}")),
                    Some(Err(e)) => Err(format!("export failed: {e}")),
                    None => Err("export failed (panic)".into()),
                }));
            });
        }
    };
    let repro = repro_command(id);
    let copy = {
        let repro = repro.clone();
        move |_| {
            document::eval(&copy_js(&repro));
            note.set(Some(Ok(format!("copied: {repro}"))));
        }
    };
    rsx! {
        tr { class: "{class}", "data-finding": "{id}",
            td { class: "mono", "{c.id}" }
            td { "{c.kind}" }
            td { "{c.map}" }
            td { class: "mono", "{c.cell}" }
            td { class: "message", title: "{row.message}", "{c.message}" }
            td { class: "mono", "{c.hits}" }
            td { "{c.status}" }
            td { class: "build", "{c.build}" }
            td { class: "actions",
                input {
                    r#type: "number",
                    min: "0",
                    title: "frames before the finding's last frame",
                    value: "{before}",
                    oninput: move |e| before.set(e.value()),
                }
                button {
                    disabled: busy(),
                    title: "Replay the finding into the native pane, paused this many frames before it",
                    onclick: open,
                    "Open at frame before"
                }
                button {
                    disabled: busy(),
                    title: "explore export {id}: a NewGame trace under target/explore/",
                    onclick: export,
                    "Export repro"
                }
                button { title: "{repro}", onclick: copy, "Copy repro command" }
            }
        }
        if let Some(n) = note() {
            tr { class: "note",
                match n {
                    Ok(t) => rsx! { td { colspan: "9", "{t}" } },
                    Err(e) => rsx! { td { colspan: "9", class: "error", "{e}" } },
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::{temp_root, test_studio};
    use super::*;
    use crate::explorer::finding_cells;
    use content::Emerald;
    use engine::data::GameData;
    use explore::config::{Budget, Config};
    use explore::testing::WallAround;
    use explore::wire::Explore;
    use std::path::{Path, PathBuf};

    /// A db at `<dir>/explore.db` filled through the explore API: 4 generations of the game
    /// walled in at its new-game cell (a `Softlock` finding on the stub world).
    pub(crate) fn filled_db(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("explore.db");
        let inner = Emerald::for_shells();
        let ng = inner.new_game();
        let game = WallAround {
            inner,
            map: ng.map,
            at: (ng.x, ng.y),
        };
        let mut ex = Explore::open(game, &path, Config::default()).unwrap();
        ex.run(Budget::Generations(4), |_| {}).unwrap();
        path
    }

    /// The Explorer tab over `polled` (the shell's poll, already taken).
    #[component]
    fn Harness(polled: Polled) -> Element {
        use_context_provider(|| Signal::new(Tab::Explorer));
        use_context_provider(|| ExplorerState {
            polled: Signal::new(polled.clone()),
            tick: Signal::new(0),
        });
        rsx! { Explorer {} }
    }

    /// The tab's html, in a studio whose explorer db is `db`.
    fn render(db: &Path, polled: Polled) -> String {
        let local = temp_root("explorer-local");
        let mut studio = test_studio(&local);
        studio.paths.explore_db = db.to_path_buf();
        let mut dom =
            VirtualDom::new_with_props(Harness, HarnessProps { polled }).with_root_context(studio);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(!local.exists(), "rendering writes nothing");
        html
    }

    fn polled(p: Poll) -> Polled {
        Polled {
            poll: Some(p),
            ..Polled::default()
        }
    }

    #[test]
    fn the_explorer_tab_shows_the_stats_and_a_row_per_finding() {
        let dir = temp_root("explorer-db");
        let db = filled_db(&dir);
        let p = poll(&db, &RunSlot::default());
        let m = p.model.clone().unwrap();
        assert!(!m.findings.is_empty(), "the walled-in game has a finding");
        let html = render(&db, polled(p.clone()));
        for (k, v) in stats_rows(&m, None) {
            assert!(
                html.contains(&format!("<dt>{k}</dt><dd>{v}</dd>")),
                "{k} {v}: {html}"
            );
        }
        assert_eq!(
            html.matches("data-finding=").count(),
            m.findings.len(),
            "{html}"
        );
        for f in &m.findings {
            let c = finding_cells(f, p.build);
            assert!(!c.other_build, "found on this build");
            assert!(
                html.contains(&format!("data-finding=\"{}\"", f.id)),
                "{html}"
            );
            for cell in [&c.kind, &c.map, &c.cell, &c.hits, &c.status, &c.build] {
                assert!(html.contains(&format!(">{cell}<")), "{cell} in {html}");
            }
            assert!(html.contains(&repro_command(f.id)), "{html}");
        }
        for label in [
            ">Open at frame before<",
            ">Export repro<",
            ">Copy repro command<",
        ] {
            assert_eq!(html.matches(label).count(), m.findings.len(), "{label}");
        }
        assert!(html.contains(&format!("value=\"{OPEN_BEFORE}\"")), "{html}");
        assert!(html.contains(&run_state_text(&RunState::Idle)), "{html}");
        assert!(
            html.contains(">Start<") && html.contains(">Stop<"),
            "{html}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_a_db_the_tab_shows_why() {
        let dir = temp_root("explorer-nodb");
        let db = dir.join("explore.db");
        let p = poll(&db, &RunSlot::default());
        let e = p.model.clone().unwrap_err();
        assert!(e.contains("no explorer db"), "{e}");
        let html = render(&db, polled(p));
        assert!(html.contains("data-explorer-error"), "{html}");
        assert!(html.contains("no explorer db"), "{html}");
        assert!(!html.contains("data-finding="));
        // Before the first poll.
        let html = render(&db, Polled::default());
        assert!(html.contains("Reading "), "{html}");
        assert!(!dir.exists(), "never created");
    }

    #[test]
    fn a_failed_run_shows_its_log_tail() {
        let dir = temp_root("explorer-failed");
        let db = dir.join("explore.db");
        let p = Poll {
            run: RunState::Failed {
                status: "exit code 101".into(),
                tail: vec!["error[E0425]: something".into()],
            },
            ..poll(&db, &RunSlot::default())
        };
        let html = render(&db, polled(p));
        assert!(html.contains("failed: exit code 101"), "{html}");
        assert!(html.contains("error[E0425]: something"), "{html}");
        assert!(!dir.exists());
    }

    #[test]
    fn a_poll_ticks_when_the_totals_change() {
        let dir = temp_root("explorer-tick");
        let db = filled_db(&dir);
        let p = poll(&db, &RunSlot::default());
        let mut dom = VirtualDom::new(|| {
            use_context_provider(|| ExplorerState {
                polled: Signal::new(Polled::default()),
                tick: Signal::new(0),
            });
            rsx! {}
        });
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let state = consume_context::<ExplorerState>();
            let t0 = Instant::now();
            state.apply(p.clone(), t0);
            assert_eq!(*state.tick.peek(), 1, "the first totals");
            assert_eq!(state.polled.peek().rate, None);
            state.apply(p.clone(), t0 + Duration::from_secs(2));
            assert_eq!(*state.tick.peek(), 1, "the same totals");
            assert_eq!(state.polled.peek().rate, Some(0.0));
            let mut more = p.clone();
            if let Ok(m) = &mut more.model {
                m.stats.nodes += 10;
                m.stats.cells += 1;
            }
            state.apply(more, t0 + Duration::from_secs(4));
            assert_eq!(*state.tick.peek(), 2);
            assert_eq!(state.polled.peek().rate, Some(5.0));
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_copy_script_quotes_the_command() {
        let js = copy_js(&repro_command(7));
        assert!(
            js.contains("writeText(\"cargo xtask explore repro 7\")"),
            "{js}"
        );
        assert!(js.contains("execCommand('copy')"));
    }
}
