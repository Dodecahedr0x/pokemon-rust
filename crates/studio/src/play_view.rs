//! Display helpers for the Play and Checkpoints tabs (Task G): the text of events, run modes,
//! the ROM shadow and timeline rows, status badges, and the `nodes/` change stamp the tree
//! watches. Pure, so they are tested without the `ui` feature.

use crate::native::EventKind;
use crate::pane::RunMode;
use crate::rom::{Shadow, TimelineEntry};
use engine::data::GameData;
use game_native::store::{Status, Store};
use std::path::Path;
use std::time::SystemTime;

/// An event's line in the event list.
pub fn event_text(kind: &EventKind) -> String {
    match kind {
        EventKind::MapEntered(m) => {
            let name = if content::has_map(*m) {
                content::Emerald::for_shells().map(*m).name
            } else {
                "?"
            };
            format!("entered {name} ({})", m.0)
        }
        EventKind::ScriptStarted => "script started".into(),
        EventKind::ScriptEnded => "script ended".into(),
        EventKind::BattleStarted => "battle started".into(),
        EventKind::BattleEnded => "battle ended".into(),
        EventKind::Log(line) => format!("log: {line}"),
        EventKind::Saved(id) => format!("saved {id}"),
        EventKind::Loaded(id) => format!("loaded {id}"),
        EventKind::Seeked(id, f) => format!("opened {id} at +{f}"),
        EventKind::Rewound(n) => format!("rewound {n} frames"),
        EventKind::NewGame => "new game".into(),
    }
}

/// The run mode's label.
pub fn run_text(run: RunMode) -> String {
    match run {
        RunMode::Play => "playing".into(),
        RunMode::Pause => "paused".into(),
        RunMode::Fast(n) => format!("fast ×{n}"),
    }
}

/// The native shadow's line.
pub fn shadow_text(s: Shadow) -> String {
    match s {
        Shadow::Off => "off".into(),
        Shadow::Matching => "matching".into(),
        Shadow::Mismatch(f) => format!("differs since frame {f}"),
        Shadow::Lost(f) => format!("lost at frame {f}"),
    }
}

/// One timeline row's cells: emulator frame, engine frame, hash, the native shadow's hash
/// (`=` when equal, `-` without a shadow), late/late_black, saves/save_status, and the
/// hash, step and loop scanline maxima.
pub fn timeline_cells(e: &TimelineEntry) -> [String; 9] {
    let native = match e.native_hash {
        None => "-".to_string(),
        Some(h) if h == e.hash => "=".to_string(),
        Some(h) => format!("{h:016x}"),
    };
    [
        e.emu_frame.to_string(),
        e.frame.to_string(),
        format!("{:016x}", e.hash),
        native,
        format!("{}/{}", e.late, e.late_black),
        format!("{}/{}", e.saves, e.save_status),
        e.hash_lines_max.to_string(),
        e.step_lines_max.to_string(),
        e.loop_lines_max.to_string(),
    ]
}

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

/// The modification times of the store's `nodes/` directories (local, then the base), then of
/// its `status.ron`: a node added, rewritten or removed, or a verify (here or from the CLI),
/// changes one of them.
pub fn nodes_stamp(store: &Store) -> Vec<Option<SystemTime>> {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let dirs = std::iter::once(store.root()).chain(store.base());
    dirs.map(|d: &Path| mtime(&d.join("nodes")))
        .chain([mtime(&store.root().join("status.ron"))])
        .collect()
}

/// The world frame of "Open at frame": a node's trace starts at its parent's frame.
pub fn seek_world_frame(node_frame: u32, trace_frames: u32, frames: u32) -> u32 {
    node_frame - trace_frames.min(node_frame) + frames
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::data::MapId;
    use game_native::trace::{CheckpointId, HashMismatch};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root(tag: &str) -> std::path::PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pkr-studio-view-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn event_lines() {
        let m = content::data::MAP_IDS[0];
        let name = content::Emerald::for_shells().map(m).name;
        let line = event_text(&EventKind::MapEntered(m));
        assert!(
            line.contains(name) && line.contains(&m.0.to_string()),
            "{line}"
        );
        assert_eq!(
            event_text(&EventKind::Saved(CheckpointId(10001))),
            format!("saved {}", CheckpointId(10001))
        );
        assert_eq!(event_text(&EventKind::Rewound(120)), "rewound 120 frames");
        assert!(event_text(&EventKind::MapEntered(MapId(u16::MAX))).contains('?'));
        assert_eq!(run_text(RunMode::Fast(4)), "fast ×4");
        assert_eq!(shadow_text(Shadow::Mismatch(7)), "differs since frame 7");
    }

    #[test]
    fn timeline_cells_mark_the_shadow() {
        let mut e = TimelineEntry {
            emu_frame: 1,
            frame: 2,
            hash: 0xab,
            late: 0,
            late_black: 0,
            saves: 0,
            save_status: 0,
            hash_lines_max: 60,
            step_lines_max: 200,
            loop_lines_max: 210,
            native_hash: Some(0xab),
        };
        assert_eq!(timeline_cells(&e)[3], "=");
        e.native_hash = None;
        assert_eq!(timeline_cells(&e)[3], "-");
        e.native_hash = Some(1);
        assert_eq!(timeline_cells(&e)[3], format!("{:016x}", 1));
        assert_eq!(timeline_cells(&e)[8], "210");
    }

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
        assert_eq!(before.len(), 3);
        assert!(before[0].is_none() && before[1].is_some() && before[2].is_none());
        std::fs::create_dir_all(local.join("nodes")).unwrap();
        assert!(nodes_stamp(&store)[0].is_some());
        std::fs::write(local.join("status.ron"), "{}").unwrap();
        assert!(
            nodes_stamp(&store)[2].is_some(),
            "a verify changes the stamp"
        );
        std::fs::remove_dir_all(local).unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn seek_frames_are_world_frames() {
        assert_eq!(seek_world_frame(1000, 400, 0), 600);
        assert_eq!(seek_world_frame(1000, 400, 400), 1000);
        assert_eq!(
            seek_world_frame(10, 400, 5),
            5,
            "a trace longer than the frame"
        );
    }
}
