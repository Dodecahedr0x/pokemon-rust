//! Play tab (Task G): the native and ROM panes side by side, their controls, the native
//! inspector and event list, and the ROM debug timeline with its build banner.
//!
//! A focused pane takes the keyboard ([`crate::keys`]); leaving it releases every key. The text
//! helpers live in the core ([`crate::play_view`]).

use super::tree::TreeState;
use super::{Live, PaneView, Studio, off_thread, sleep};
use crate::build::{BuildState, RomBuild};
use crate::keys::{Keyboard, native_cmds, rom_cmds};
use crate::native::{Inspector, NativeCmd};
use crate::pane::RunMode;
use crate::play_view::{event_text, run_text, shadow_text, timeline_cells};
use crate::rom::{RomCmd, RomFile, RomSource, RomStatus, boot_name};
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use game_native::trace::Parent;
use std::time::Duration;

/// Timeline rows the ROM pane shows (newest first).
pub const TIMELINE_ROWS: usize = 60;
/// Native events shown (newest first).
pub const EVENT_ROWS: usize = 64;
/// Build output lines shown.
pub const BUILD_LINES: usize = 20;
/// How often a running ROM build is polled.
const BUILD_POLL: Duration = Duration::from_millis(250);

/// The fast-forward speeds offered.
const FAST: [u8; 3] = [2, 4, 8];

#[component]
pub fn Play() -> Element {
    rsx! {
        div { class: "play",
            NativeColumn {}
            RomColumn {}
        }
    }
}

/// Runs `cargo xtask rom` (in the root scope, so leaving the tab does not kill it) and
/// reloads the ROM pane when it succeeds.
fn start_build(studio: &Studio, mut build: Signal<Option<BuildState>>) {
    if matches!(*build.peek(), Some(BuildState::Running(_))) {
        return;
    }
    build.set(Some(BuildState::Running(Vec::new())));
    let studio = studio.clone();
    spawn_forever(async move {
        let ws = studio.paths.workspace.clone();
        let started = off_thread(move || RomBuild::start(&ws)).await;
        let mut b = match started {
            Some(Ok(b)) => b,
            Some(Err(e)) => {
                build.set(Some(BuildState::Failed(vec![format!(
                    "cargo xtask rom: {e}"
                )])));
                return;
            }
            None => {
                build.set(Some(BuildState::Failed(vec![
                    "starting the build failed".into(),
                ])));
                return;
            }
        };
        loop {
            // Never blocks (the output is read by its own threads).
            let state = b.poll();
            let done = !matches!(state, BuildState::Running(_));
            if matches!(state, BuildState::Ok(_)) {
                studio.rom(RomCmd::Reload);
            }
            build.set(Some(state));
            if done {
                break;
            }
            sleep(BUILD_POLL).await;
        }
    });
}

/// Pause/play, step and fast-forward buttons for a pane whose mode is `run`.
#[component]
fn RunButtons(run: RunMode, on_mode: EventHandler<RunMode>, on_step: EventHandler<u32>) -> Element {
    let paused = run == RunMode::Pause;
    rsx! {
        button {
            title: "Space",
            onclick: move |_| on_mode.call(if paused { RunMode::Play } else { RunMode::Pause }),
            if paused { "Play" } else { "Pause" }
        }
        button { title: ". (pauses)", onclick: move |_| on_step.call(1), "Step" }
        for n in FAST {
            button {
                class: if run == RunMode::Fast(n) { "active" } else { "" },
                title: "Tab held: ×8",
                onclick: move |_| on_mode.call(if run == RunMode::Fast(n) { RunMode::Play } else { RunMode::Fast(n) }),
                "×{n}"
            }
        }
    }
}

#[component]
fn NativeColumn() -> Element {
    let studio = use_context::<Studio>();
    let live = use_context::<Live>();
    let trees = use_context::<TreeState>();
    let mut kb = use_signal(Keyboard::default);
    let s: Inspector = live.native.read().clone();
    let send = {
        let studio = studio.clone();
        move |cmds: Vec<NativeCmd>| {
            for c in cmds {
                studio.native(c);
            }
        }
    };
    let (down, up, blur) = (send.clone(), send.clone(), send.clone());
    let parent = match s.parent {
        Parent::NewGame => "new game".to_string(),
        Parent::Checkpoint(id) => match trees.model.read().row(id) {
            Some(r) => format!("{id} {}", r.label),
            None => id.to_string(),
        },
    };
    let capturable = match s.capturable {
        Ok(()) => "yes".to_string(),
        Err(why) => format!("no: {why:?}"),
    };
    let camera = s
        .camera
        .map_or("-".to_string(), |(x, y)| format!("{x}, {y}"));
    let saved = s.saved.map_or("-".to_string(), |id| id.to_string());
    let ring = s.ring_frames.len();
    let run = run_text(s.run);
    let pending = if s.pending_save { " (saving…)" } else { "" };
    let (st1, st2, st3, st4, st5, st6) = (
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
    );
    rsx! {
        section { class: "column",
            PaneView {
                name: "native",
                title: "Native",
                onkeydown: move |e: KeyboardEvent| {
                    let o = kb.write().down(&e.code().to_string(), e.is_auto_repeating(), live.native.peek().run);
                    if o.handled {
                        e.prevent_default();
                    }
                    down(native_cmds(o, &live.native.peek()));
                },
                onkeyup: move |e: KeyboardEvent| {
                    let o = kb.write().up(&e.code().to_string());
                    if o.handled {
                        e.prevent_default();
                    }
                    up(native_cmds(o, &live.native.peek()));
                },
                onblur: move |_| {
                    let o = kb.write().blur();
                    blur(native_cmds(o, &live.native.peek()));
                },
            }
            div { class: "controls",
                RunButtons {
                    run: s.run,
                    on_mode: move |m| st1.native(NativeCmd::Mode(m)),
                    on_step: move |n| st2.native(NativeCmd::Step(n)),
                }
                button { title: "F8: back at least two seconds", onclick: move |_| st3.native(NativeCmd::Rewind), "Rewind" }
                button { title: "F5: a local node (at the first idle frame)", onclick: move |_| st4.native(NativeCmd::Save), "Save" }
                button {
                    title: "F9: the node this trace starts from",
                    onclick: move |_| st5.native(match live.native.peek().parent {
                        Parent::Checkpoint(id) => NativeCmd::Load(id),
                        Parent::NewGame => NativeCmd::NewGame,
                    }),
                    "Reload parent"
                }
                button { onclick: move |_| st6.native(NativeCmd::NewGame), "New game" }
            }
            dl { class: "inspector",
                dt { "frame" } dd { "{s.frame}" }
                dt { "map" } dd { "{s.map_name} ({s.map.0})" }
                dt { "camera" } dd { "{camera}" }
                dt { "hash" } dd { class: "mono", "{s.hash:016x}" }
                dt { "mode" } dd { "{s.mode}" }
                dt { "capturable" } dd { "{capturable}" }
                dt { "run" } dd { "{run}{pending}" }
                dt { "parent" } dd { "{parent}" }
                dt { "trace" } dd { "{s.trace_frames} frames" }
                dt { "rewind ring" } dd { "{ring} states" }
                dt { "last save" } dd { "{saved}" }
                dt { "last" } dd { "{s.last}" }
            }
            h3 { "Events" }
            ol { class: "events",
                for e in s.events.iter().rev().take(EVENT_ROWS) {
                    li { span { class: "mono", "{e.frame}" } " {event_text(&e.kind)}" }
                }
            }
        }
    }
}

#[component]
fn RomColumn() -> Element {
    let studio = use_context::<Studio>();
    let live = use_context::<Live>();
    let mut kb = use_signal(Keyboard::default);
    let s: RomStatus = live.rom.read().clone();
    let build = (live.build)();
    let building = matches!(build, Some(BuildState::Running(_)));
    let worst = s.worst();
    let send = {
        let studio = studio.clone();
        move |cmds: Vec<RomCmd>| {
            for c in cmds {
                studio.rom(c);
            }
        }
    };
    let (down, up, blur) = (send.clone(), send.clone(), send.clone());
    let rom_file = match s.rom {
        RomFile::Missing => "missing (Build ROM)".to_string(),
        RomFile::Present { .. } if s.rom_changed => "rebuilt on disk: Reload".to_string(),
        RomFile::Present { .. } => "present".to_string(),
    };
    let boot = s
        .boot
        .map_or("-".to_string(), |b| format!("{} ({b})", boot_name(b)));
    let shadow = shadow_text(s.shadow);
    let run = run_text(s.run);
    let rows: Vec<[String; 9]> = s
        .timeline
        .iter()
        .rev()
        .take(TIMELINE_ROWS)
        .map(timeline_cells)
        .collect();
    let (st1, st2, st3, st4, st5, st6) = (
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
        studio.clone(),
    );
    let build_lines: Vec<String> = match &build {
        Some(BuildState::Running(l) | BuildState::Ok(l) | BuildState::Failed(l)) => {
            l.iter().rev().take(BUILD_LINES).rev().cloned().collect()
        }
        None => Vec::new(),
    };
    let build_title = match &build {
        Some(BuildState::Running(_)) => Some("Building the ROM (cargo xtask rom)…"),
        Some(BuildState::Ok(_)) => Some("ROM built; the pane reloaded"),
        Some(BuildState::Failed(_)) => Some("ROM build failed"),
        None => None,
    };
    rsx! {
        section { class: "column",
            if s.build_mismatch() {
                div { class: "banner",
                    "ROM build ≠ native build: target/game.gba was built from other sources. "
                    button {
                        disabled: building,
                        onclick: move |_| start_build(&st1, live.build),
                        "Build ROM"
                    }
                }
            }
            PaneView {
                name: "rom",
                title: "ROM (mGBA)",
                onkeydown: move |e: KeyboardEvent| {
                    let o = kb.write().down(&e.code().to_string(), e.is_auto_repeating(), live.rom.peek().run);
                    if o.handled {
                        e.prevent_default();
                    }
                    down(rom_cmds(o));
                },
                onkeyup: move |e: KeyboardEvent| {
                    let o = kb.write().up(&e.code().to_string());
                    if o.handled {
                        e.prevent_default();
                    }
                    up(rom_cmds(o));
                },
                onblur: move |_| {
                    let o = kb.write().blur();
                    blur(rom_cmds(o));
                },
            }
            div { class: "controls",
                RunButtons {
                    run: s.run,
                    on_mode: move |m| st2.rom(RomCmd::Mode(m)),
                    on_step: move |n| {
                        st3.rom(RomCmd::Mode(RunMode::Pause));
                        st3.rom(RomCmd::Step(n));
                    },
                }
                button { onclick: move |_| st4.rom(RomCmd::Reset(RomSource::NewGame)), "New game" }
                button { title: "F9: boot the last source again", onclick: move |_| st5.rom(RomCmd::Reload), "Reload" }
                button {
                    disabled: building,
                    title: "cargo xtask rom, then reload",
                    onclick: {
                        let studio = studio.clone();
                        move |_| start_build(&studio, live.build)
                    },
                    "Build ROM"
                }
                button { title: "Write the flash to target/studio/", onclick: move |_| st6.rom(RomCmd::ExportSave), "Export save" }
            }
            if let Some(t) = build_title {
                details { class: "build", open: !matches!(build, Some(BuildState::Ok(_))),
                    summary { "{t}" }
                    pre { {build_lines.join("\n")} }
                }
            }
            if let Some(e) = &s.error {
                div { class: "error", "{e}" }
            }
            dl { class: "inspector",
                dt { "ROM" } dd { "{rom_file}" }
                dt { "boot" } dd { "{boot}" }
                dt { "run" } dd { "{run}" }
                dt { "native shadow" } dd { "{shadow}" }
                if s.diverged {
                    dt { "node" } dd { "diverged on this build" }
                }
                if let Some(p) = &s.exported {
                    dt { "exported" } dd { class: "mono", "{p.display()}" }
                }
                dt { "worst" }
                dd {
                    "late {worst.late}/{worst.late_black}, lines hash {worst.hash_lines_max} step {worst.step_lines_max} loop {worst.loop_lines_max}, shadow mismatches {worst.mismatches}"
                }
            }
            h3 { "Timeline" }
            table { class: "timeline mono",
                thead {
                    tr {
                        th { "emu" }
                        th { "frame" }
                        th { "hash" }
                        th { "native" }
                        th { "late" }
                        th { "saves" }
                        th { "hash ln" }
                        th { "step ln" }
                        th { "loop ln" }
                    }
                }
                tbody {
                    for r in rows {
                        tr {
                            for c in r {
                                td { "{c}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Tab;
    use super::super::tests::{render, temp_root, test_studio};
    /// The Play tab shows both panes, their controls and the inspector, before any status.
    #[test]
    fn the_play_tab_renders_the_controls() {
        let local = temp_root("play");
        let html = render(test_studio(&local), Tab::Play);
        for label in [
            ">Native<",
            ">ROM (mGBA)<",
            ">Rewind<",
            ">Save<",
            ">Reload parent<",
            ">Build ROM<",
            ">Export save<",
            ">×8<",
            ">Events<",
            ">Timeline<",
            "missing (Build ROM)",
        ] {
            assert!(html.contains(label), "{label} in {html}");
        }
        assert!(!local.exists());
    }
}
