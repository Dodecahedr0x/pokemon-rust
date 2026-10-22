//! Checkpoints tab (Task G): the layered tree (committed nodes, then personal ones), with
//! statuses, a background Verify, and per-row load, seek, branch, label and remove.
//!
//! Committed nodes are the layered store's read-only base: the studio loads, seeks and branches
//! from them, but relabels and removes only local nodes (the committed tree changes through
//! `cargo xtask checkpoints`, decisions 7, 8 and 14).

use super::{Live, Studio, Tab, focus_pane, off_thread, sleep};
use crate::native::NativeCmd;
use crate::pane::RunMode;
use crate::rom::{RomCmd, RomSource};
use crate::tree::{Origin, TreeModel, TreeRow, relabel, remove, verify_in_background};
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use game_native::store::{Status, Store};
use game_native::trace::Parent;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// How often the tree checks its `nodes/` directories for changes (a save or a CLI command).
pub const WATCH: Duration = Duration::from_secs(2);

/// A status badge: its CSS class and its text.
pub fn badge(status: Option<&Status>) -> (&'static str, String) {
    match status {
        None => ("unverified", "unverified".into()),
        Some(Status::Valid) => ("valid", "valid".into()),
        Some(Status::Stale { schema }) => ("stale", format!("stale (schema {schema})")),
        Some(Status::Diverged {
            hash,
            parent_diverged,
        }) => {
            let mut s = match hash {
                Some(m) => format!("diverged at frame {}", m.frame),
                None => "diverged (end state)".into(),
            };
            if *parent_diverged {
                s.push_str(", parent diverged");
            }
            ("diverged", s)
        }
        Some(Status::Broken(msg)) => ("broken", format!("broken: {msg}")),
    }
}

/// The modification times of the store's `nodes/` directories (local, then the base): a node
/// added, rewritten or removed changes one of them.
pub fn nodes_stamp(store: &Store) -> Vec<Option<SystemTime>> {
    let dirs = std::iter::once(store.root()).chain(store.base());
    dirs.map(|d: &Path| {
        std::fs::metadata(d.join("nodes"))
            .and_then(|m| m.modified())
            .ok()
    })
    .collect()
}

/// The tree's signals, in context (provided by the [`Shell`](super::Shell)).
#[derive(Clone, Copy)]
pub struct TreeState {
    pub model: Signal<TreeModel>,
    /// The last load, verify or edit error.
    pub error: Signal<Option<String>>,
    pub verifying: Signal<bool>,
}

impl TreeState {
    /// Re-reads the tree on a worker thread. Runs in the root scope, so leaving the tab does
    /// not cancel it.
    pub fn refresh(self, studio: &Studio) {
        let store = studio.store.clone();
        let (mut model, mut error) = (self.model, self.error);
        spawn_forever(async move {
            match off_thread(move || TreeModel::load(&store)).await {
                Some(Ok(m)) => {
                    model.set(m);
                    error.set(None);
                }
                Some(Err(e)) => error.set(Some(format!("reading the tree: {e}"))),
                None => error.set(Some("reading the tree failed".into())),
            }
        });
    }

    /// Replays every node on a worker thread (`Store::verify`, merging the verdicts into the
    /// local `status.ron`), then shows the verdicts.
    pub fn verify(self, studio: &Studio) {
        let (mut verifying, mut error, mut model) = (self.verifying, self.error, self.model);
        if *verifying.peek() {
            return;
        }
        verifying.set(true);
        let store = studio.store.clone();
        let studio = studio.clone();
        spawn_forever(async move {
            let r = off_thread(move || {
                verify_in_background(store, content::Emerald::for_shells())
                    .join()
                    .map_err(|_| "the verify thread panicked".to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
            })
            .await;
            match r {
                Some(Ok(verdicts)) => {
                    model.write().set_statuses(&verdicts);
                    error.set(None);
                    self.refresh(&studio);
                }
                Some(Err(e)) => error.set(Some(format!("verify: {e}"))),
                None => error.set(Some("verify failed".into())),
            }
            verifying.set(false);
        });
    }

    /// Runs a store edit on a worker thread, then refreshes (or shows its error).
    fn edit(
        self,
        studio: &Studio,
        what: &'static str,
        f: impl FnOnce(&Store) -> Result<(), game_native::store::StoreError> + Send + 'static,
    ) {
        let store = studio.store.clone();
        let studio = studio.clone();
        let mut error = self.error;
        spawn_forever(async move {
            match off_thread(move || f(&store)).await {
                Some(Ok(())) => self.refresh(&studio),
                Some(Err(e)) => error.set(Some(format!("{what}: {e}"))),
                None => error.set(Some(format!("{what} failed"))),
            }
        });
    }
}

/// Provides the [`TreeState`] (from the tree read at start-up) and watches the `nodes/`
/// directories every [`WATCH`].
pub fn use_tree_state(studio: &Studio) -> TreeState {
    let state = use_context_provider(|| {
        let (model, error) = match &studio.tree {
            Ok(m) => (m.clone(), None),
            Err(e) => (TreeModel::default(), Some(format!("reading the tree: {e}"))),
        };
        TreeState {
            model: Signal::new(model),
            error: Signal::new(error),
            verifying: Signal::new(false),
        }
    });
    use_future({
        let studio = studio.clone();
        move || {
            let studio = studio.clone();
            async move {
                let store = studio.store.clone();
                let mut last = off_thread({
                    let store = store.clone();
                    move || nodes_stamp(&store)
                })
                .await;
                loop {
                    sleep(WATCH).await;
                    let store = store.clone();
                    let now = off_thread(move || nodes_stamp(&store)).await;
                    if now.is_some() && now != last {
                        last = now;
                        state.refresh(&studio);
                    }
                }
            }
        }
    });
    state
}

/// What a row is showing besides its buttons.
#[derive(Clone, Debug, PartialEq)]
enum RowEdit {
    Label(String),
    ConfirmRemove,
    /// "Open at frame": the slider's value.
    Seek(u32),
}

#[component]
pub fn Tree() -> Element {
    let studio = use_context::<Studio>();
    let state = use_context::<TreeState>();
    let model = state.model.read();
    let rows = model.rows();
    let (shared, local) = (
        rows.iter().filter(|r| r.origin == Origin::Shared).count(),
        rows.iter().filter(|r| r.origin == Origin::Local).count(),
    );
    let verifying = (state.verifying)();
    let error = (state.error)();
    let local_root = studio.store.root().display().to_string();
    rsx! {
        div { class: "tree",
            div { class: "toolbar",
                span { "{shared} committed, {local} local nodes (saves go to {local_root})" }
                button {
                    disabled: verifying,
                    title: "Replay every node on this build (in the background)",
                    onclick: {
                        let studio = studio.clone();
                        move |_| state.verify(&studio)
                    },
                    if verifying { "Verifying…" } else { "Verify" }
                }
                button {
                    onclick: {
                        let studio = studio.clone();
                        move |_| state.refresh(&studio)
                    },
                    "Refresh"
                }
            }
            if let Some(e) = error {
                div { class: "error", "{e}" }
            }
            table { class: "nodes",
                thead {
                    tr {
                        th { "node" }
                        th { "label" }
                        th { "frame" }
                        th { "trace" }
                        th { "map" }
                        th { "status" }
                        th { "" }
                    }
                }
                tbody {
                    for row in rows.iter().cloned() {
                        Row { key: "{row.id}", row }
                    }
                }
            }
        }
    }
}

#[component]
fn Row(row: TreeRow) -> Element {
    let studio = use_context::<Studio>();
    let state = use_context::<TreeState>();
    let mut tab = use_context::<Signal<Tab>>();
    let live = use_context::<Live>();
    let mut edit = use_signal(|| None::<RowEdit>);
    let id = row.id;
    let local = row.origin == Origin::Local;
    let (class, text) = badge(row.status.as_ref());
    let indent = format!("padding-left: {}em", f32::from(row.depth) * 1.2);
    let parent = match row.parent {
        Parent::NewGame => "new game".to_string(),
        Parent::Checkpoint(p) => p.to_string(),
    };
    let origin = match row.origin {
        Origin::Shared => "committed",
        Origin::Local => "local",
    };
    let read_only = "committed node: change it with cargo xtask checkpoints";
    let s = studio.clone();
    let load_native = move |focus: bool| {
        s.native(NativeCmd::Load(id));
        if live.native.peek().run == RunMode::Pause {
            s.native(NativeCmd::Mode(RunMode::Play));
        }
        tab.set(Tab::Play);
        if focus {
            focus_pane("native");
        }
    };
    let (mut l1, mut l2) = (load_native.clone(), load_native);
    let s = studio.clone();
    rsx! {
        tr { class: "node {origin}",
            td { class: "id", title: "parent: {parent}", "{id}" }
            td { style: "{indent}", class: "label",
                match edit() {
                    Some(RowEdit::Label(text)) => rsx! {
                        input {
                            value: "{text}",
                            autofocus: true,
                            oninput: move |e| edit.set(Some(RowEdit::Label(e.value()))),
                            onkeydown: {
                                let studio = studio.clone();
                                move |e: KeyboardEvent| match e.key() {
                                    Key::Enter => {
                                        if let Some(RowEdit::Label(l)) = edit() {
                                            state.edit(&studio, "label", move |st| relabel(st, id, &l));
                                        }
                                        edit.set(None);
                                    }
                                    Key::Escape => edit.set(None),
                                    _ => {}
                                }
                            },
                        }
                    },
                    _ => rsx! { "{row.label}" },
                }
                span { class: "origin", " {origin}" }
            }
            td { "{row.frame}" }
            td { "{row.trace_frames}" }
            td { "{row.map_name}" }
            td { span { class: "badge {class}", "{text}" } }
            td { class: "actions",
                button { title: "Load into the native pane", onclick: move |_| l1(false), "Native" }
                button {
                    title: "Boot the ROM pane from this node",
                    onclick: {
                        let s = s.clone();
                        move |_| {
                            s.rom(RomCmd::Reset(RomSource::Node(id)));
                            tab.set(Tab::Play);
                        }
                    },
                    "ROM"
                }
                button {
                    title: "Load into the native pane and play from here (F5 saves a local child)",
                    onclick: move |_| l2(true),
                    "Branch"
                }
                button {
                    title: "Open the native pane inside this node's trace",
                    disabled: row.trace_frames == 0,
                    onclick: move |_| edit.set(match edit() {
                        Some(RowEdit::Seek(_)) => None,
                        _ => Some(RowEdit::Seek(row.trace_frames / 2)),
                    }),
                    "Open at…"
                }
                button {
                    disabled: !local,
                    title: if local { "Rename" } else { read_only },
                    onclick: move |_| edit.set(Some(RowEdit::Label(row.label.clone()))),
                    "Label"
                }
                button {
                    disabled: !local || row.children > 0,
                    title: if !local { read_only } else if row.children > 0 { "remove its children first" } else { "Remove this node" },
                    onclick: move |_| edit.set(Some(RowEdit::ConfirmRemove)),
                    "Remove"
                }
            }
        }
        match edit() {
            Some(RowEdit::ConfirmRemove) => rsx! {
                tr { class: "confirm",
                    td { colspan: "7",
                        "Remove {id} ({row.label})? Its trace is deleted. "
                        button {
                            class: "danger",
                            onclick: {
                                let studio = studio.clone();
                                move |_| {
                                    state.edit(&studio, "remove", move |st| remove(st, id));
                                    edit.set(None);
                                }
                            },
                            "Remove"
                        }
                        button { onclick: move |_| edit.set(None), "Cancel" }
                    }
                }
            },
            Some(RowEdit::Seek(frames)) => {
                // The world frame: the trace starts at the parent's frame.
                let at = row.frame - row.trace_frames.min(row.frame) + frames;
                rsx! {
                tr { class: "seek",
                    td { colspan: "7",
                        "Open {id} at frame "
                        input {
                            r#type: "range",
                            min: "0",
                            max: "{row.trace_frames}",
                            value: "{frames}",
                            oninput: move |e| {
                                if let Ok(f) = e.value().parse::<u32>() {
                                    edit.set(Some(RowEdit::Seek(f)));
                                }
                            },
                        }
                        " {frames} / {row.trace_frames} (world frame {at}) "
                        button {
                            onclick: {
                                let studio = studio.clone();
                                move |_| {
                                    studio.native(NativeCmd::Seek { id, frames });
                                    studio.native(NativeCmd::Mode(RunMode::Pause));
                                    edit.set(None);
                                    tab.set(Tab::Play);
                                    focus_pane("native");
                                }
                            },
                            "Open"
                        }
                        button { onclick: move |_| edit.set(None), "Cancel" }
                    }
                }
                }
            }
            _ => rsx! {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{render, temp_root, test_studio};
    use super::*;
    use game_native::trace::HashMismatch;

    #[test]
    fn badges() {
        assert_eq!(badge(None).0, "unverified");
        assert_eq!(badge(Some(&Status::Valid)), ("valid", "valid".into()));
        assert_eq!(
            badge(Some(&Status::Stale { schema: 3 })).1,
            "stale (schema 3)"
        );
        let d = Status::Diverged {
            hash: Some(HashMismatch {
                frame: 120,
                want: 1,
                got: 2,
            }),
            parent_diverged: true,
        };
        assert_eq!(
            badge(Some(&d)),
            ("diverged", "diverged at frame 120, parent diverged".into())
        );
        let d = Status::Diverged {
            hash: None,
            parent_diverged: false,
        };
        assert_eq!(badge(Some(&d)).1, "diverged (end state)");
        assert_eq!(
            badge(Some(&Status::Broken("boom".into()))),
            ("broken", "broken: boom".into())
        );
    }

    #[test]
    fn the_stamp_follows_the_nodes_dirs() {
        let local = temp_root("stamp");
        let base = temp_root("stamp-base");
        std::fs::create_dir_all(base.join("nodes")).unwrap();
        let store = Store::layered(&base, &local).unwrap();
        let before = nodes_stamp(&store);
        assert_eq!(before.len(), 2);
        assert!(before[0].is_none() && before[1].is_some());
        std::fs::create_dir_all(local.join("nodes")).unwrap();
        assert!(nodes_stamp(&store)[0].is_some());
        std::fs::remove_dir_all(local).unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }

    /// The Checkpoints tab lists every node of the committed tree (labels and ids), with the
    /// committed nodes read-only.
    #[test]
    fn the_tree_tab_renders_the_rows() {
        let local = temp_root("tree");
        let studio = test_studio(&local);
        let model = studio.tree.clone().unwrap();
        let html = render(studio, Tab::Checkpoints);
        assert!(html.contains(">Verify<"), "{html}");
        assert!(!model.rows().is_empty(), "the committed tree has nodes");
        for row in model.rows() {
            assert!(
                html.contains(&format!(">{}<", row.id)),
                "{} in {html}",
                row.id
            );
            assert!(html.contains(&row.label), "{} in {html}", row.label);
        }
        let shared = model
            .rows()
            .iter()
            .filter(|r| r.origin == Origin::Shared)
            .count();
        assert_eq!(
            html.matches("committed node: change it with cargo xtask")
                .count(),
            2 * shared,
            "Label and Remove are read-only on committed nodes"
        );
        assert!(!local.exists(), "rendering writes nothing");
    }
}
