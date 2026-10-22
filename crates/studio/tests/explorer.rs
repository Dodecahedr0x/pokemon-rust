//! M7 Task S1: the Explorer core on temp dbs filled through the explore API (`ExplorerModel`,
//! `CoverageGrid`, `finding_trace`), `NativeCmd::OpenTrace` against a straight replay, and the
//! coverage overlay's heat buckets. Every db and store lives in a temp dir.

use content::Emerald;
use engine::data::{GameData, MapId};
use explore::config::{Budget, Config};
use explore::testing::{FadeTrap, INJECTED_PANIC, PanicOnScript, WallAround};
use explore::wire::Explore;
use explore::{FindingKind, NodeStatus, findings};
use game_native::Session;
use game_native::checkpoints::Materializer;
use game_native::store::{Status, Store};
use game_native::trace::{Parent, Run, Trace};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use studio::explorer::{self, CoverageGrid, ExplorerModel, OPEN_BEFORE, sort_findings};
use studio::map_view::{HEAT_CLASSES, coverage_paths, heat_bucket};
use studio::native::{EventKind, NativeCmd, NativePane, open_trace};
use studio::pane::Pane;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("pkr-studio-x-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `generations` generations of `game` on a new db at `path`.
fn explore<G: GameData + Clone + Send + Sync + 'static>(
    game: G,
    path: &Path,
    generations: u32,
) -> Explore<G> {
    let mut ex = Explore::open(game, path, Config::default()).unwrap();
    ex.run(Budget::Generations(generations), |_| {}).unwrap();
    ex
}

/// The plain game walled in at its new-game cell: one `Softlock` finding on the root (stub).
fn walled() -> WallAround<Emerald> {
    let inner = Emerald::for_shells();
    let ng = inner.new_game();
    WallAround {
        inner,
        map: ng.map,
        at: (ng.x, ng.y),
    }
}

/// The plain game with a fade trap (a `CapturableStuck` finding) on the first cell at least
/// `far` steps from the new-game cell that the player can reach.
fn fade_trapped(far: i16) -> FadeTrap<Emerald> {
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let snap = loop {
        if let Ok(snap) = s.world().snapshot(s.game()) {
            break snap;
        }
        s.step(engine::Buttons::NONE);
    };
    let ng = game.new_game();
    let at = (explore::path::reach(&game, &snap).cells())
        .find(|&(x, y)| (x - ng.x).abs() + (y - ng.y).abs() >= far)
        .expect("a far cell");
    FadeTrap {
        inner: game,
        map: ng.map,
        at,
    }
}

fn pane(local: &Path) -> (NativePane, Store) {
    let store = Store::layered(Store::default_root(content::STUB), local).unwrap();
    let m = Arc::new(Mutex::new(Materializer::new(Emerald::for_shells(), 8)));
    (NativePane::new(store.clone(), m), store)
}

/// Frame and hash after the trace's first `frames` frames from a new game.
fn straight(trace: &Trace, frames: u32) -> (u32, u64) {
    let mut s = Session::new(Emerald::for_shells());
    for k in trace.keys().take(frames as usize) {
        s.step(k);
    }
    (s.world().frame(), s.world().hash())
}

/// Opens `trace` at `frames` in a pane, checks the landing against a straight replay, then
/// saves (holding no key until capturable) and checks that the save is a valid node with a
/// NewGame parent.
fn open_and_save(trace: &Trace, frames: u32, tag: &str) {
    let local = temp_dir(tag);
    let (mut p, store) = pane(&local);
    p.handle(NativeCmd::OpenTrace {
        trace: trace.clone(),
        frames,
    });
    let s = p.status();
    let (frame, hash) = straight(trace, frames);
    assert_eq!((s.frame, s.hash), (frame, hash), "{}", s.last);
    assert_eq!(s.frame, frames);
    assert_eq!(s.parent, Parent::NewGame);
    assert_eq!(s.trace_frames, frames);
    assert!(!s.last.contains("diverged"), "{}", s.last);
    assert_eq!(
        s.events.last().unwrap().kind,
        EventKind::OpenedTrace(frames)
    );
    p.handle(NativeCmd::Save);
    for _ in 0..3000 {
        if p.status().saved.is_some() {
            break;
        }
        p.handle(NativeCmd::Step(1));
        p.tick();
    }
    let id = p.status().saved.expect("saved once capturable");
    let node = store.find(&id.to_string()).unwrap();
    assert_eq!(node.trace.parent, Parent::NewGame);
    assert!(node.trace.frames() >= frames);
    assert_eq!(
        node.trace.keys().take(frames as usize).collect::<Vec<_>>(),
        trace.keys().take(frames as usize).collect::<Vec<_>>(),
        "the save starts with the opened prefix"
    );
    let verdicts = store
        .verify(
            game_native::checkpoints::replayer(&Emerald::for_shells()),
            false,
        )
        .unwrap();
    let v = verdicts.iter().find(|v| v.0 == id).expect("verified");
    assert_eq!(v.1, Status::Valid);
    let _ = std::fs::remove_dir_all(local);
}

#[test]
fn the_model_reads_stats_findings_and_coverage_while_the_writer_is_open() {
    let dir = temp_dir("model");
    let path = dir.join("explore.db");
    // The stub walled in gives one finding; the real flavor explores plainly.
    let check = |stats: explore::db::DbStats| {
        let m = ExplorerModel::load(&path).unwrap();
        assert_eq!(m.stats, stats);
        m
    };
    let (m, mut want, cov) = if content::STUB {
        let ex = explore(walled(), &path, 4);
        let m = check(ex.stats().unwrap());
        (m, ex.db.findings().unwrap(), ex.db.coverage(None).unwrap())
    } else {
        let ex = explore(Emerald::for_shells(), &path, 4);
        let m = check(ex.stats().unwrap());
        (m, ex.db.findings().unwrap(), ex.db.coverage(None).unwrap())
    };
    assert!(m.stats.nodes > 0 && m.stats.valid > 0, "{:?}", m.stats);
    sort_findings(&mut want);
    assert_eq!(m.findings, want);
    if content::STUB {
        assert_eq!(m.findings.len(), 1);
        assert_eq!(m.findings[0].kind, FindingKind::Softlock);
    }
    assert_eq!(m.goals().len() as u32, m.stats.max_progress);
    // Reloaded once the writer is gone: the same rows.
    assert_eq!(ExplorerModel::load(&path).unwrap(), m);

    // Coverage grids round-trip the db's rows of every map covered.
    let game = Emerald::for_shells();
    let maps: std::collections::BTreeSet<MapId> = cov.iter().map(|r| r.0).collect();
    assert!(!maps.is_empty());
    let mut cells = 0u64;
    for map in maps {
        let g = CoverageGrid::load(&path, &game, map).unwrap();
        let mm = game.map(map);
        assert_eq!((g.w, g.h), (mm.width, mm.height));
        let rows: Vec<(i16, i16, u32)> = (cov.iter())
            .filter(|r| r.0 == map && r.1 >= 0 && r.2 >= 0)
            .filter(|r| (r.1 as u16) < mm.width && (r.2 as u16) < mm.height)
            .map(|r| (r.1, r.2, r.3))
            .collect();
        let mut sorted = rows.clone();
        sorted.sort_by_key(|&(x, y, _)| (y, x));
        assert_eq!(g.rows(), sorted);
        assert_eq!(CoverageGrid::from_rows(g.w, g.h, g.rows()), g);
        cells += g.visited() as u64;
    }
    assert_eq!(cells, m.stats.cells);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_stub_finding_opens_before_its_end_at_a_straight_replay_hash() {
    if !content::STUB {
        return;
    }
    let dir = temp_dir("finding");
    let path = dir.join("explore.db");
    let ex = explore(fade_trapped(6), &path, 20);
    let f = (ex.db.findings().unwrap().into_iter())
        .find(|f| f.kind == FindingKind::CapturableStuck)
        .expect("the fade trap is found");
    drop(ex);
    let db = explorer::open(&path).unwrap();
    let ft = explorer::finding_trace(&db, f.id, OPEN_BEFORE).unwrap();
    assert_eq!(ft.row, f);
    assert!(!ft.stale, "found on this build");
    let (trace, frames) = (ft.trace, ft.frames);
    assert_eq!(trace.parent, Parent::NewGame);
    assert_eq!(u64::from(trace.frames()), f.total_frame);
    assert_eq!(u64::from(frames), f.total_frame.saturating_sub(120));
    // Clamped at the new game.
    assert_eq!(
        explorer::finding_trace(&db, f.id, u32::MAX).unwrap().frames,
        0
    );
    assert!(
        explorer::finding_trace(&db, 99, 0).is_err(),
        "no finding 99"
    );
    assert!(trace.frames() > 2 * OPEN_BEFORE, "{}", trace.frames());
    open_and_save(&trace, frames, "finding");
    open_and_save(&trace, trace.frames() / 2, "finding-mid");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_node_lineage_opens_at_frame_before_at_a_straight_replay_hash() {
    let dir = temp_dir("lineage");
    let game = Emerald::for_shells();
    let ex = explore(game, &dir.join("explore.db"), 4);
    // Its own last step spans the opened frames, so the save holds no key over a text box.
    let deepest = (ex.db.nodes().unwrap().into_iter())
        .filter(|n| n.status == NodeStatus::Valid && n.frames >= OPEN_BEFORE)
        .max_by_key(|n| (n.total_frames, n.id))
        .unwrap();
    let lineage = ex.db.lineage(deepest.id).unwrap();
    let runs: Vec<&[Run]> = lineage.iter().map(|n| n.inputs.as_slice()).collect();
    let trace = findings::repro_trace(&game, &runs, &[]).unwrap();
    assert_eq!(u64::from(trace.frames()), deepest.total_frames);
    assert_eq!(straight(&trace, trace.frames()).1, deepest.world_hash);
    let frames = trace.frames().saturating_sub(OPEN_BEFORE);
    open_and_save(&trace, frames, "lineage");
    // Past the end is clamped to the end.
    let local = temp_dir("lineage-end");
    let (mut p, _) = pane(&local);
    p.handle(NativeCmd::OpenTrace {
        trace: trace.clone(),
        frames: u32::MAX,
    });
    let s = p.status();
    assert_eq!((s.frame, s.hash), straight(&trace, trace.frames()));
    // Another parent is refused with a note; the pane keeps its session.
    let mut other = trace.clone();
    other.parent = Parent::Checkpoint(game_native::trace::CheckpointId(1));
    p.handle(NativeCmd::OpenTrace {
        trace: other,
        frames: 10,
    });
    let after = p.status();
    assert!(after.last.contains("failed"), "{}", after.last);
    assert_eq!(after.frame, s.frame);
    // A changed recorded hash is reported as a divergence.
    let mut bent = trace.clone();
    bent.hashes[0].1 ^= 1;
    p.handle(NativeCmd::OpenTrace {
        trace: bent.clone(),
        frames: bent.hashes[0].0,
    });
    assert!(p.status().last.contains("diverged"), "{}", p.status().last);
    let _ = std::fs::remove_dir_all(local);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_panicking_replay_is_an_error_not_a_dead_pane() {
    if !content::STUB {
        return;
    }
    let game = PanicOnScript {
        inner: Emerald::for_shells(),
        script: content::NEW_GAME_SCRIPT,
    };
    let empty = Trace {
        inputs: Vec::new(),
        hashes: Vec::new(),
        ..findings::repro_trace(&Emerald::for_shells(), &[], &[]).unwrap()
    };
    let e = open_trace(game, empty, 0).err().expect("panics");
    assert!(e.contains(INJECTED_PANIC), "{e}");
}

#[test]
fn coverage_paths_draw_four_heat_buckets() {
    assert_eq!(
        [0, 1, 2, 9, 10, 99, 100, u32::MAX].map(heat_bucket),
        [0, 1, 2, 2, 3, 3, 4, 4]
    );
    let g = CoverageGrid::from_rows(
        4,
        2,
        [(0, 0, 1), (1, 0, 5), (2, 0, 50), (3, 1, 500), (0, 1, 7)],
    );
    let paths = coverage_paths(&g, 8);
    let classes: Vec<&str> = paths.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(classes, HEAT_CLASSES);
    assert_eq!(paths[0].1, "M128 128h16v16h-16z");
    assert_eq!(paths[1].1, "M144 128h16v16h-16zM128 144h16v16h-16z");
    assert_eq!(paths[3].1, "M176 144h16v16h-16z");
    // Empty buckets are left out; an empty grid draws nothing.
    let hot = CoverageGrid::from_rows(1, 1, [(0, 0, 1000)]);
    assert_eq!(
        coverage_paths(&hot, 0),
        [(HEAT_CLASSES[3].to_string(), "M0 0h16v16h-16z".into())]
    );
    assert!(coverage_paths(&CoverageGrid::default(), 8).is_empty());
}
