//! Task C: `NativePane` driven through the `Pane` methods (no thread, no window), over a
//! layered store whose base is the committed tree and whose local root is a temp dir.

#[cfg(not(pkr_silice))]
use game_native::scenario::cached_route;
use game_native::store::Store;
#[cfg(not(pkr_silice))]
use game_native::trace::Parent;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use studio::native::{EventKind, Inspector, NativeCmd, NativePane};
use studio::pane::{Pane, RunMode};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("pkr-studio-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn pane(tag: &str) -> (NativePane, PathBuf, Store) {
    let local = temp_dir(tag);
    let store = Store::layered(Store::default_root(content::FLAVOR), &local).unwrap();
    (NativePane::new(store.clone()), local, store)
}

fn status(p: &NativePane) -> Inspector {
    p.status()
}

#[test]
fn ticks_follow_the_run_mode() {
    let (mut p, local, _) = pane("modes");
    let f = |p: &NativePane| status(p).frame;
    assert_eq!(status(&p).run, RunMode::Pause);
    p.tick();
    assert_eq!(f(&p), 0, "paused");
    p.handle(NativeCmd::Step(3));
    p.tick();
    assert_eq!(f(&p), 3);
    p.tick();
    assert_eq!(f(&p), 3, "a step is taken once");
    p.handle(NativeCmd::Mode(RunMode::Fast(4)));
    p.tick();
    assert_eq!(f(&p), 7);
    p.handle(NativeCmd::Mode(RunMode::Play));
    p.tick();
    assert_eq!(f(&p), 8);
    p.handle(NativeCmd::Step(2));
    assert_eq!(status(&p).run, RunMode::Pause);
    p.tick();
    assert_eq!(f(&p), 10);
    assert_eq!(
        p.framebuffer().len(),
        game_native::SCREEN_W * game_native::SCREEN_H
    );
    assert_eq!(status(&p).trace_frames, 10);
    // A step not yet taken is dropped by a mode change: a later pause steps nothing.
    p.handle(NativeCmd::Step(5));
    p.handle(NativeCmd::Mode(RunMode::Play));
    p.handle(NativeCmd::Mode(RunMode::Pause));
    p.tick();
    assert_eq!(f(&p), 10);
    let _ = std::fs::remove_dir_all(local);
}

#[test]
fn rewind_delegates_to_controls() {
    let (mut p, local, _) = pane("rewind");
    p.handle(NativeCmd::Mode(RunMode::Fast(16)));
    for _ in 0..25 {
        p.tick();
    }
    let before = status(&p).frame;
    assert_eq!(before, 400);
    p.handle(NativeCmd::Rewind);
    let s = status(&p);
    assert!(s.frame + 120 <= before, "{} -> {}", before, s.frame);
    assert_eq!(s.trace_frames, s.frame);
    assert_eq!(
        s.events.last().unwrap().kind,
        EventKind::Rewound(before - s.frame)
    );
    let _ = std::fs::remove_dir_all(local);
}

#[cfg(not(pkr_silice))] // the Silice boot route stays on its map and never battles
#[test]
fn the_routes_first_map_change_or_battle_is_an_event() {
    // The extracted route changes map first; the stub route stays on its map and battles.
    let (mut p, local, _) = pane("events");
    let rec = cached_route();
    p.handle(NativeCmd::Mode(RunMode::Play));
    let start = status(&p).map;
    let mut found = None;
    for i in 0..rec.frames() {
        p.handle(NativeCmd::Keys(rec.keys_at(i)));
        p.tick();
        let s = status(&p);
        assert_eq!(s.camera.is_some(), s.mode == "field", "frame {}", s.frame);
        if s.map != start || s.mode == "battle" {
            found = Some(s);
            break;
        }
    }
    let s = found.expect("the route changes map or battles");
    let ev = s.events.last().unwrap();
    let want = if s.map != start {
        EventKind::MapEntered(s.map)
    } else {
        EventKind::BattleStarted
    };
    assert_eq!(ev.kind, want);
    if !content::STUB {
        assert_eq!(ev.kind, EventKind::MapEntered(s.map));
    }
    assert_eq!(ev.frame, s.frame);
    assert!(!s.map_name.is_empty());
    assert!(s.events.len() <= studio::native::EVENTS);
    let frames: Vec<u32> = s.events.iter().map(|e| e.frame).collect();
    assert!(frames.windows(2).all(|w| w[0] <= w[1]), "{frames:?}");
    let _ = std::fs::remove_dir_all(local);
}

#[cfg(not(pkr_silice))] // loads a committed node: Silice has none yet (SIL1 Wave 3)
#[test]
fn load_seek_and_save_into_the_local_store() {
    let (mut p, local, store) = pane("load");
    let nodes = store.topo_order().unwrap();
    let first = &nodes[0];
    p.handle(NativeCmd::Load(first.meta.id));
    let s = status(&p);
    assert_eq!(s.frame, first.meta.frame, "{}", s.last);
    assert_eq!(s.hash, first.meta.world_hash);
    assert_eq!(s.parent, Parent::Checkpoint(first.meta.id));
    assert_eq!(s.capturable, Ok(()));
    assert_eq!(s.mode, "field");
    let session = p.controls().session();
    let snap = session.world().snapshot(session.game()).unwrap();
    assert_eq!(s.camera, Some(snap.field.camera), "the field camera");
    assert_eq!(
        s.events.last().unwrap().kind,
        EventKind::Loaded(first.meta.id)
    );
    // Seek into the second node's trace and branch from there.
    let second = &nodes[1];
    let mid = second.trace.frames() / 2;
    p.handle(NativeCmd::Seek {
        id: second.meta.id,
        frames: mid,
    });
    let s = status(&p);
    assert_eq!(s.frame, second.trace.start_frame + mid, "{}", s.last);
    assert_eq!(s.trace_frames, mid);
    assert_eq!(s.parent, second.trace.parent);
    assert_eq!(
        s.events.last().unwrap().kind,
        EventKind::Seeked(second.meta.id, mid)
    );
    // The route's keys from here (the seek may land in a battle), saving after 5 frames.
    let rec = cached_route();
    for i in 0..3000 {
        if status(&p).saved.is_some() {
            break;
        }
        if i == 5 {
            p.handle(NativeCmd::Save);
        }
        p.handle(NativeCmd::Keys(rec.keys_at(status(&p).frame)));
        p.handle(NativeCmd::Step(1));
        p.tick();
    }
    let s = status(&p);
    let id = s.saved.expect("saved once capturable");
    assert!(local.join(format!("nodes/{id}.ron")).exists());
    assert_eq!(s.parent, Parent::Checkpoint(id));
    assert_eq!(s.trace_frames, 0);
    assert_eq!(
        store.find(&id.to_string()).unwrap().trace.parent,
        second.trace.parent
    );
    let verdicts = store
        .verify(
            game_native::checkpoints::replayer(&content::Emerald::for_shells()),
            false,
        )
        .unwrap();
    let v = verdicts.iter().find(|v| v.0 == id).expect("verified");
    assert_eq!(v.1, game_native::store::Status::Valid);
    // New game.
    p.handle(NativeCmd::NewGame);
    let s = status(&p);
    assert_eq!((s.frame, s.parent), (0, Parent::NewGame));
    assert_eq!(s.events.last().unwrap().kind, EventKind::NewGame);
    // A failed load is a note, not a panic.
    p.handle(NativeCmd::Load(game_native::trace::CheckpointId(99_999)));
    assert!(status(&p).last.contains("failed"), "{}", status(&p).last);
    let _ = std::fs::remove_dir_all(local);
}

/// `Note` adds a line after what the inspector's `last` holds (M7 S2: the stale-build warning
/// after an `OpenTrace`).
#[test]
fn a_note_goes_after_the_last_line() {
    let (mut p, local, _) = pane("note");
    p.handle(NativeCmd::NewGame);
    let before = status(&p).last;
    p.handle(NativeCmd::Note("first".into()));
    let want = if before.is_empty() {
        "first".to_string()
    } else {
        format!("{before}; first")
    };
    assert_eq!(status(&p).last, want);
    p.handle(NativeCmd::Note("second".into()));
    assert_eq!(status(&p).last, format!("{want}; second"));
    assert!(!local.exists(), "a note writes nothing");
}
