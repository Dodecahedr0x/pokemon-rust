//! Task G end to end on the stub world (`PKR_EXTRACTED_DIR=/nonexistent`): coverage and goals,
//! thread-count and resume determinism, fault findings that reproduce, revalidation on a build
//! change, and the CLI. Each test returns at once on the real flavor. Databases live in temp
//! dirs.

use content::{Emerald, NEW_GAME_SCRIPT};
use engine::data::GameData;
use explore::config::{Budget, Config};
use explore::testing::{PanicOnScript, Rebuilt, WallAround};
use explore::wire::{self, Explore};
use explore::{FindingKind, NodeStatus, path};
use game_native::Session;
use std::collections::BTreeSet;
use std::path::Path;

fn config(threads: usize) -> Config {
    Config {
        threads,
        ..Config::default()
    }
}

/// `generations` generations of `game` on the db at `path` (created or resumed).
fn explore<G: GameData + Clone + Send + Sync + 'static>(
    game: G,
    path: &Path,
    threads: usize,
    generations: u32,
) -> Explore<G> {
    let mut ex = Explore::open(game, path, config(threads)).unwrap();
    ex.run(Budget::Generations(generations), |_| {}).unwrap();
    ex
}

fn digest<G>(ex: &Explore<G>) -> u64 {
    ex.db.content_digest().unwrap()
}

/// The cells reachable from the new game's first capturable state.
fn reachable() -> BTreeSet<(i16, i16)> {
    let game = Emerald::for_shells();
    let mut s = Session::new(game);
    let snap = loop {
        if let Ok(snap) = s.world().snapshot(s.game()) {
            break snap;
        }
        s.step(engine::Buttons::NONE);
    };
    path::reach(&game, &snap).cells().collect()
}

#[test]
fn thirty_generations_cover_the_stub_and_reach_every_goal() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // One WalkTo per cell (`cell_shift` 0): with the default 4x4 blocks the walks between
    // block centres follow the same BFS lines and leave about half the cells unvisited
    // (Task H tunes the density).
    let mut cfg = config(0);
    cfg.key.cell_shift = 0;
    let mut ex = Explore::open(Emerald::for_shells(), &dir.path().join("explore.db"), cfg).unwrap();
    ex.run(Budget::Generations(30), |_| {}).unwrap();
    let stats = ex.stats().unwrap();
    assert_eq!(stats.max_progress as usize, explore::goals::goals().len());
    assert_eq!(stats.findings, 0, "{:?}", ex.db.findings().unwrap());
    let map = Emerald::for_shells().new_game().map;
    let covered: BTreeSet<(i16, i16)> = ex
        .db
        .coverage(Some(map))
        .unwrap()
        .into_iter()
        .map(|(_, x, y, _)| (x, y))
        .collect();
    let want = reachable();
    let missing: Vec<_> = want.difference(&covered).collect();
    assert!(missing.is_empty(), "never stood on {missing:?}");
}

#[test]
fn the_thread_count_does_not_change_the_db() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let run = |threads: usize| {
        let ex = explore(
            Emerald::for_shells(),
            &dir.path().join(format!("t{threads}.db")),
            threads,
            10,
        );
        digest(&ex)
    };
    let one = run(1);
    assert_eq!(one, run(4));
    assert_eq!(one, run(8));
}

#[test]
fn a_resumed_run_equals_a_straight_one() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let straight = explore(
        Emerald::for_shells(),
        &dir.path().join("straight.db"),
        0,
        30,
    );
    let resumed = dir.path().join("resumed.db");
    let first = explore(Emerald::for_shells(), &resumed, 0, 15);
    assert_eq!(first.runner.generation, 15);
    drop(first);
    let second = explore(Emerald::for_shells(), &resumed, 0, 15);
    assert_eq!(second.runner.generation, 30);
    assert_eq!(
        second.db.nodes().unwrap(),
        straight.db.nodes().unwrap(),
        "same rows"
    );
    assert_eq!(digest(&second), digest(&straight));
}

#[test]
fn a_new_game_panic_is_one_finding_that_reproduces() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let game = PanicOnScript {
        inner: Emerald::for_shells(),
        script: NEW_GAME_SCRIPT,
    };
    let mut ex = Explore::open(game, &dir.path().join("explore.db"), config(0)).unwrap();
    let sum = ex.run(Budget::Generations(5), |_| {}).unwrap();
    assert!(sum.stalled, "the root job never makes a node");
    assert_eq!(sum.generations, 1);
    let fs = ex.db.findings().unwrap();
    assert_eq!(fs.len(), 1, "{fs:?}");
    let f = &fs[0];
    assert_eq!(f.kind, FindingKind::Panic);
    assert_eq!(f.node, None, "the tail starts at a new game");
    assert!(f.location.as_deref().unwrap().contains("testing.rs"));
    let (_, trace) = wire::finding_trace(&game, &ex.db, f.id, 0).unwrap();
    assert_eq!(trace.frames(), 0);
    wire::reproduce(&game, &ex.db, f.id, &ex.cfg).unwrap();
    // Reopened: still stalled, nothing new.
    drop(ex);
    let mut ex = Explore::open(game, &dir.path().join("explore.db"), config(0)).unwrap();
    assert!(ex.stalled());
    assert!(ex.generation().unwrap().is_none());
}

#[test]
fn a_walled_in_start_is_one_softlock_that_reproduces() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let inner = Emerald::for_shells();
    let ng = inner.new_game();
    let game = WallAround {
        inner,
        map: ng.map,
        at: (ng.x, ng.y),
    };
    let ex = explore(game, &dir.path().join("explore.db"), 0, 4);
    let fs = ex.db.findings().unwrap();
    assert_eq!(fs.len(), 1, "{fs:?}");
    let f = &fs[0];
    assert_eq!(f.kind, FindingKind::Softlock);
    assert_eq!(f.node, Some(1), "the root is trapped");
    assert!(f.tail.is_empty());
    assert_eq!((f.map, f.pos), (ng.map, (ng.x, ng.y)));
    let (_, trace) = wire::finding_trace(&game, &ex.db, f.id, 0).unwrap();
    assert_eq!(u64::from(trace.frames()), f.total_frame);
    wire::reproduce(&game, &ex.db, f.id, &ex.cfg).unwrap();
    // Without the wall the same finding does not reproduce.
    let free = WallAround {
        inner,
        map: ng.map,
        at: (0, 0),
    };
    assert!(wire::reproduce(&free, &ex.db, f.id, &ex.cfg).is_err());
}

#[test]
fn a_rebuilt_game_revalidates_every_node() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    let inner = Emerald::for_shells();
    let before = explore(inner, &path, 0, 8);
    let rows = before.db.nodes().unwrap();
    drop(before);
    let game = Rebuilt {
        inner,
        build_id: inner.build_id() ^ 0x5A5A,
    };
    let mut ex = Explore::open(game, &path, config(0)).unwrap();
    assert!(ex.opened.build_changed);
    let r = ex.revalidated.expect("revalidated on open");
    assert_eq!((r.repaired, r.broken), (0, 0), "{r:?}");
    assert_eq!(r.valid, rows.len() as u64);
    let after = ex.db.nodes().unwrap();
    assert_eq!(after.len(), rows.len());
    for (a, b) in after.iter().zip(&rows) {
        assert_eq!((a.id, a.digest, a.status), (b.id, b.digest, b.status));
        assert_ne!(a.build, b.build);
    }
    assert!(after.iter().all(|n| n.status != NodeStatus::Stale));
    // And it explores on.
    ex.run(Budget::Generations(2), |_| {}).unwrap();
}

#[test]
fn the_cli_runs_and_reports() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("explore.db");
    let db_s = db.to_str().unwrap().to_string();
    let cli = |args: &[&str]| {
        let mut v: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        v.extend(["--db".to_string(), db_s.clone()]);
        explore::cli::main(v)
    };
    cli(&["run", "--generations", "3", "--threads", "2"]).unwrap();
    cli(&["status"]).unwrap();
    cli(&["findings", "--all"]).unwrap();
    cli(&["digest"]).unwrap();
    let map = Emerald::for_shells().new_game().map;
    cli(&["coverage", &map.0.to_string()]).unwrap();
    cli(&["coverage", Emerald::for_shells().map(map).name]).unwrap();
    assert!(cli(&["show", "1"]).is_err(), "no finding 1");
    assert!(cli(&["repro", "1"]).is_err());
    assert!(
        cli(&["run", "--nodes", "1", "--secs", "1"]).is_err(),
        "two budgets"
    );
    assert!(cli(&["bogus"]).is_err());
    assert!(cli(&["reset"]).is_err(), "needs --yes");
    cli(&["reset", "--yes"]).unwrap();
    assert!(!db.exists());
}

#[test]
fn exploring_leaves_the_work_tree_unchanged() {
    if !content::STUB {
        return;
    }
    let porcelain = || {
        let out = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output();
        out.ok().map(|o| o.stdout)
    };
    let before = porcelain();
    let dir = tempfile::tempdir().unwrap();
    explore(Emerald::for_shells(), &dir.path().join("explore.db"), 0, 3);
    assert_eq!(porcelain(), before);
}
