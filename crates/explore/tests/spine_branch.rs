//! The spine and its branches on the stub world (`PKR_EXTRACTED_DIR=/nonexistent`) and
//! synthetic faults around it: target enumeration, recovery to the spine, rejoined, expected
//! and unexpected divergences, stranded, aborts (budget, novelty), bugs and reports that
//! reproduce, determinism across job counts, the db round trip and the CLI. Each test returns
//! at once on a real flavour (`spine_real.rs` plays those spines). Dbs live in temp dirs.

use content::{Emerald, NEW_GAME_SCRIPT};
use engine::data::GameData;
use explore::branch::{self, BranchResult, Explored, Outcome, Target, TargetKind};
use explore::config::Config;
use explore::db::{Db, RunRow};
use explore::spine::{self, SpineRun, Step};
use explore::testing::{FadeTrap, INJECTED_REPORT, PanicOnScript, Rebuilt, Reporting, VarTrap};
use explore::{FindingKind, cli};
use std::collections::BTreeSet;
use std::path::Path;

fn cfg(jobs: usize) -> Config {
    Config {
        jobs,
        ..Config::default()
    }
}

fn spine_of<G: GameData + Clone + 'static>(game: &G) -> SpineRun {
    let s = spine::run(game, spine::steps(), &cfg(1));
    assert!(s.complete(), "{:?}", s.error);
    s
}

/// Every target of every origin, run on `jobs` workers.
fn branches<G: GameData + Clone + Send + Sync + 'static>(
    game: &G,
    s: &SpineRun,
    c: &Config,
) -> (Vec<Target>, Vec<BranchResult>) {
    let origins = branch::origins(s, &c.branch, None);
    let targets = branch::all_targets(game, s, &origins, c);
    let results = branch::run_all(game, s, &targets, c);
    (targets, results)
}

/// A comparable digest of results (everything but the coverage sets' order, which is sorted).
fn summary(r: &[BranchResult]) -> Vec<String> {
    r.iter()
        .map(|r| {
            format!(
                "{:?} {:?} {:?} {:?} {} {} {} {:?} {} {} {}",
                r.target,
                r.outcome,
                r.explored,
                r.macros,
                r.explore_frames,
                r.recover_frames,
                r.window_frames,
                r.gain,
                r.points,
                r.findings.len(),
                r.message
            )
        })
        .collect()
}

/// A reachable cell of the new game's map that the spine never stood on, at least `far` steps
/// (Manhattan) from every spine cell.
fn off_spine(s: &SpineRun, far: i16) -> (i16, i16) {
    let map = s.positions[0].map();
    let snap = &s.positions[0].end.world;
    let cells: Vec<(i16, i16)> = (s.coverage.cells.iter())
        .filter(|c| c.0 == map)
        .map(|c| (c.1, c.2))
        .collect();
    explore::path::reach(&Emerald::for_shells(), snap)
        .cells()
        .find(|&(x, y)| {
            cells
                .iter()
                .all(|&(a, b)| (a - x).abs() + (b - y).abs() >= far)
        })
        .expect("a cell off the spine")
}

fn store<G: GameData>(game: &G, path: &Path, s: &SpineRun, r: &[BranchResult]) -> Db {
    let (mut db, _) =
        Db::open(path, engine::checkpoint::build_hash(game), content::FLAVOR).unwrap();
    let row = |command: &str| RunRow {
        started: "test".into(),
        command: command.into(),
        config: Config::default().to_ron(),
        threads: 1,
        branches: r.len() as u64,
        frames: 0,
        wall_ms: 0,
    };
    let run = db.record_run(&row("spine")).unwrap();
    db.put_spine(s, 0, run).unwrap();
    let run = db.record_run(&row("branch")).unwrap();
    db.put_branches(s, r, run).unwrap();
    db
}

#[test]
fn the_stub_spine_plays_to_its_end_and_labels_its_checkpoints() {
    if !content::STUB {
        return;
    }
    let s = spine_of(&Emerald::for_shells());
    assert_eq!(s.labels(), ["start", "after_battle", "end"]);
    assert_eq!(s.positions.len(), s.steps.len() + 1);
    assert_eq!(s.frames, 1520, "the stub spine's frame count");
    let frames: u64 = s.inputs.iter().map(|r| u64::from(r.frames)).sum();
    assert_eq!(frames, s.frames);
    assert_eq!(s.coverage.maps.len(), 1);
    assert!(s.coverage.wild_maps.contains(&s.positions[0].map()));
    // Positions are in frame order and the checkpoints are where the steps left the player.
    assert!(
        s.positions
            .windows(2)
            .all(|w| w[0].end.header.frame <= w[1].end.header.frame)
    );
    assert_eq!(s.labelled("end").unwrap().pos(), (5, 12));
    // Deterministic.
    let again = spine_of(&Emerald::for_shells());
    assert_eq!(again.inputs, s.inputs);
}

#[test]
fn targets_are_new_against_the_spine_and_unique() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let s = spine_of(&game);
    let c = cfg(1);
    let origins = branch::origins(&s, &c.branch, None);
    let labelled: Vec<usize> = (s.positions.iter())
        .filter(|p| p.label.is_some())
        .map(|p| p.index)
        .collect();
    // Every checkpoint is an origin (or the position of the same frame before it).
    let frame = |i: usize| s.positions[i].end.header.frame;
    for &i in &labelled {
        assert!(
            origins.iter().any(|&o| frame(o) == frame(i)),
            "checkpoint {i}: {origins:?}"
        );
    }
    let from = branch::origins(&s, &c.branch, Some("after_battle"));
    assert!(
        !from.is_empty()
            && from
                .iter()
                .all(|&i| s.positions[i].segment == Some("after_battle"))
    );
    let targets = branch::all_targets(&game, &s, &origins, &c);
    assert!(!targets.is_empty());
    let unique: BTreeSet<_> = targets.iter().map(|t| (t.origin, t.action)).collect();
    assert_eq!(unique.len(), targets.len());
    // No warp, edge, script or choice to find on the stub: new cells and a RUN.
    for t in &targets {
        assert!(
            matches!(t.kind, TargetKind::Cells | TargetKind::Run),
            "{t:?}"
        );
    }
    assert!(targets.iter().any(|t| t.kind == TargetKind::Cells));
    // Capped per origin.
    let one = Config {
        branch: explore::config::BranchConfig {
            max_targets: 1,
            ..c.branch.clone()
        },
        ..c.clone()
    };
    for &o in &origins {
        assert!(branch::targets(&game, &s, o, &one).len() <= 1);
    }
}

#[test]
fn branches_recover_rejoin_and_do_not_depend_on_the_job_count() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let s = spine_of(&game);
    let (targets, one) = branches(&game, &s, &cfg(1));
    let (_, four) = branches(&game, &s, &cfg(4));
    assert_eq!(one.len(), targets.len());
    assert_eq!(summary(&one), summary(&four));
    for r in &one {
        assert_eq!(r.outcome, Outcome::Rejoined, "{r:?}");
        assert!(r.window_frames > 0, "the window ran");
        assert!(r.findings.is_empty());
    }
    assert!(one.iter().any(|r| r.gain.cells > 0), "new cells");
    // The dbs are the same too.
    let dir = tempfile::tempdir().unwrap();
    let a = store(&game, &dir.path().join("a.db"), &s, &one);
    let b = store(&game, &dir.path().join("b.db"), &s, &four);
    assert_eq!(a.content_digest().unwrap(), b.content_digest().unwrap());
    let st = a.stats().unwrap();
    assert_eq!(st.branches, one.len() as u64);
    assert_eq!(st.outcomes["rejoined"], one.len() as u64);
    assert_eq!(st.positions, s.positions.len() as u64);
    assert!(st.cells > st.spine_cells, "{st:?}");
    // Branches round-trip: the action and kind parse back.
    for row in a.branches().unwrap() {
        let action = branch::Action::from_text(&row.action).expect("an action");
        let kind = TargetKind::parse(&row.kind).expect("a kind");
        assert!(targets.iter().any(|t| t.action == action && t.kind == kind));
    }
}

#[test]
fn a_tiny_budget_recovers_and_a_high_novelty_bar_aborts_the_chain() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let s = spine_of(&game);
    // The budget stops the exploration after the target; the branch still recovers.
    let mut c = cfg(2);
    c.branch.budget_frames = 1;
    let (_, r) = branches(&game, &s, &c);
    let played: Vec<_> = r.iter().filter(|r| r.explore_frames > 1).collect();
    assert!(!played.is_empty());
    for r in played {
        assert_eq!(r.explored, Some(Explored::Budget), "{r:?}");
        assert_eq!(r.macros.len(), 1, "only the target");
        assert_eq!(r.outcome, Outcome::Rejoined, "it still recovers: {r:?}");
        assert!(r.window_frames > 0, "the window ran");
    }
    // Too little novelty for the frames: the target plays, one chained macro, then the branch
    // is aborted without a recovery.
    let mut c = cfg(2);
    c.branch.min_novelty = u32::MAX;
    let (_, r) = branches(&game, &s, &c);
    let mut aborted = 0;
    for r in r.iter().filter(|r| r.explored.is_some()) {
        match r.explored {
            Some(Explored::LowNovelty) => {
                aborted += 1;
                assert_eq!(r.macros.len(), 2, "the target and one more: {r:?}");
                assert_eq!(r.outcome, Outcome::Aborted, "{r:?}");
                assert_eq!((r.recover_frames, r.window_frames), (0, 0), "no recovery");
            }
            // Nothing new after the target: it recovers.
            _ => assert_eq!(r.outcome, Outcome::Rejoined, "{r:?}"),
        }
    }
    assert!(aborted > 0, "{:#?}", summary(&r));
    let mut c = cfg(2);
    c.branch.max_steps = 1;
    let (_, r) = branches(&game, &s, &c);
    assert!(r.iter().all(|r| r.macros.len() <= 1));
}

/// A spine whose next step after `cell` needs the player's exact cell and facing.
static CELL_STEPS: &[Step] = &[
    Step::Checkpoint("start"),
    Step::Goto {
        map: spine::NEW_GAME_MAP,
        x: 10,
        y: 9,
    },
    Step::Checkpoint("cell"),
    Step::Face(engine::data::Dir::Up),
    Step::Wait(10),
    Step::Checkpoint("end"),
];

#[test]
fn recovery_walks_back_to_the_cell_or_strands_the_branch() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let s = spine::run(&game, CELL_STEPS, &cfg(1));
    assert!(s.complete(), "{:?}", s.error);
    let origin = s.labelled("cell").unwrap().index;
    let c = cfg(2);
    let targets = branch::targets(&game, &s, origin, &c);
    let plain = branch::run_all(&game, &s, &targets, &c);
    // Those that moved walk back to (10, 9), face up and rejoin.
    let moved: Vec<usize> = (0..plain.len())
        .filter(|&i| plain[i].recover_frames > 1)
        .collect();
    assert!(!moved.is_empty(), "{:#?}", summary(&plain));
    for &i in &moved {
        assert_eq!(plain[i].outcome, Outcome::Rejoined, "{:?}", plain[i]);
    }
    // Without the frames to walk back, they are stranded (and no window runs).
    let mut tight = c.clone();
    tight.branch.recover_frames = 1;
    let r = branch::run_all(&game, &s, &targets, &tight);
    for &i in &moved {
        assert_eq!(r[i].outcome, Outcome::Stranded, "{:?}", r[i]);
        assert!(r[i].message.contains("recovery budget"), "{}", r[i].message);
        assert_eq!(r[i].window_frames, 0);
        assert!(r[i].findings.is_empty(), "the open map is no softlock");
    }
}

#[test]
fn a_branch_that_changes_the_story_diverges_as_expected() {
    if !content::STUB {
        return;
    }
    let plain = Emerald::for_shells();
    let at = off_spine(&spine_of(&plain), 3);
    let game = VarTrap {
        inner: plain,
        map: plain.new_game().map,
        at,
    };
    let s = spine_of(&game);
    let (targets, r) = branches(&game, &s, &cfg(2));
    assert!(
        targets.iter().any(|t| t.kind == TargetKind::Coord),
        "the trap's coord event is a target: {targets:?}"
    );
    let hit: Vec<_> = (r.iter())
        .filter(|r| matches!(r.outcome, Outcome::Diverged { .. }))
        .collect();
    assert!(!hit.is_empty(), "{:#?}", summary(&r));
    for r in hit {
        assert_eq!(r.outcome, Outcome::Diverged { expected: true }, "{r:?}");
        assert!(
            r.message.contains("the branch changed var 0x40f0"),
            "{}",
            r.message
        );
        assert!(
            r.findings.is_empty(),
            "an expected divergence is no finding"
        );
    }
}

#[test]
fn a_window_that_lands_elsewhere_is_an_unexpected_divergence() {
    if !content::STUB {
        return;
    }
    let game = Emerald::for_shells();
    let mut s = spine_of(&game);
    // The spine as recorded on another day: every state after the first checkpoint names
    // another respawn point. Branches from there replay the window and land on the true one.
    let origin = s.labelled("after_battle").unwrap().index;
    for p in &mut s.positions[origin + 1..] {
        std::sync::Arc::make_mut(&mut p.end).world.respawn ^= 1;
    }
    let c = cfg(2);
    let targets: Vec<Target> = branch::targets(&game, &s, origin, &c);
    assert!(!targets.is_empty());
    let r = branch::run_all(&game, &s, &targets, &c);
    for r in &r {
        assert_eq!(r.outcome, Outcome::Diverged { expected: false }, "{r:?}");
        assert!(r.message.contains("respawn"), "{}", r.message);
        assert_eq!(r.findings.len(), 1);
        let f = &r.findings[0];
        assert_eq!(f.kind, FindingKind::Divergence);
        assert!(
            f.message.starts_with("after after_battle:"),
            "{}",
            f.message
        );
        assert!(!f.tail.is_empty());
    }
    // One finding per (kind, signature) in the db, its inputs from a new game.
    let dir = tempfile::tempdir().unwrap();
    let db = store(&game, &dir.path().join("explore.db"), &s, &r);
    let found = db.findings().unwrap();
    assert!(!found.is_empty());
    for f in &found {
        assert_eq!(f.kind, FindingKind::Divergence);
        assert_eq!(f.origin, Some(origin as u32));
        let frames: u64 = f.inputs.iter().map(|r| u64::from(r.frames)).sum();
        assert_eq!(frames, f.total_frame);
        assert!(f.total_frame > s.positions[origin].end.header.frame.into());
    }
}

#[test]
fn bugs_on_a_branch_are_findings_that_reproduce() {
    if !content::STUB {
        return;
    }
    let plain = Emerald::for_shells();
    let at = off_spine(&spine_of(&plain), 3);
    let game = FadeTrap {
        inner: plain,
        map: plain.new_game().map,
        at,
    };
    let s = spine_of(&game);
    let (_, r) = branches(&game, &s, &cfg(2));
    let bugs: Vec<_> = r.iter().filter(|r| r.outcome == Outcome::Bug).collect();
    assert!(!bugs.is_empty(), "{:#?}", summary(&r));
    assert!(
        bugs.iter()
            .flat_map(|r| &r.findings)
            .all(|f| f.kind == FindingKind::CapturableStuck)
    );
    let dir = tempfile::tempdir().unwrap();
    let db = store(&game, &dir.path().join("explore.db"), &s, &r);
    let found = db.findings().unwrap();
    assert_eq!(found.len(), 1, "one signature: {found:?}");
    let f = &found[0];
    assert_eq!(f.kind, FindingKind::CapturableStuck);
    assert_eq!(f.pos, at);
    assert!(f.branch.is_some());
    assert_eq!(cli::reproduce(&game, &db, f.id, &cfg(1)).unwrap(), Ok(()));
    // The plain game has no trap: it does not reproduce there.
    assert!(cli::reproduce(&plain, &db, f.id, &cfg(1)).unwrap().is_err());
}

#[test]
fn a_spine_panic_stops_the_spine_with_a_finding() {
    if !content::STUB {
        return;
    }
    let game = PanicOnScript {
        inner: Emerald::for_shells(),
        script: NEW_GAME_SCRIPT,
    };
    let s = spine::run(&game, spine::steps(), &cfg(1));
    assert!(!s.complete());
    let e = s.error.as_ref().expect("stopped");
    let f = e.finding.as_ref().expect("a finding");
    assert_eq!(f.kind, FindingKind::Panic);
    let dir = tempfile::tempdir().unwrap();
    let db = store(&game, &dir.path().join("explore.db"), &s, &[]);
    let found = db.findings().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, FindingKind::Panic);
    assert!(found[0].branch.is_none() && found[0].origin.is_none());
    assert_eq!(
        cli::reproduce(&game, &db, found[0].id, &cfg(1)).unwrap(),
        Ok(())
    );
    let st = db.stats().unwrap();
    assert!(st.spine_error.is_some());
    assert!(st.spine_done < st.spine_steps);
}

#[test]
fn reports_along_the_spine_are_stored_as_report_findings() {
    if !content::STUB {
        return;
    }
    let game = Reporting {
        inner: Emerald::for_shells(),
    };
    let s = spine_of(&game);
    assert!(
        s.reports
            .iter()
            .any(|f| f.message.contains(INJECTED_REPORT)),
        "{:?}",
        s.reports
    );
    let dir = tempfile::tempdir().unwrap();
    let db = store(&game, &dir.path().join("explore.db"), &s, &[]);
    let found = db.findings().unwrap();
    assert!(found.iter().any(|f| f.kind == FindingKind::Unsupported));
    let f = found
        .iter()
        .find(|f| f.kind == FindingKind::Unsupported)
        .unwrap();
    assert_eq!(cli::reproduce(&game, &db, f.id, &cfg(1)).unwrap(), Ok(()));
}

#[test]
fn a_new_build_replaces_the_spine_and_marks_old_findings_stale() {
    if !content::STUB {
        return;
    }
    let plain = Emerald::for_shells();
    let at = off_spine(&spine_of(&plain), 3);
    let game = FadeTrap {
        inner: plain,
        map: plain.new_game().map,
        at,
    };
    let s = spine_of(&game);
    let (_, r) = branches(&game, &s, &cfg(2));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explore.db");
    drop(store(&game, &path, &s, &r));
    // Another build with the same spine: the old finding is stale until found again.
    let rebuilt = Rebuilt {
        inner: game,
        build_id: game.build_id() ^ 1,
    };
    let (mut db, st) = Db::open(
        &path,
        engine::checkpoint::build_hash(&rebuilt),
        content::FLAVOR,
    )
    .unwrap();
    assert!(st.build_changed);
    db.put_spine(&s, 0, 1).unwrap();
    let found = db.findings().unwrap();
    assert!(found.iter().all(|f| f.status == "stale"), "{found:?}");
    db.put_branches(&s, &r, 1).unwrap();
    let found = db.findings().unwrap();
    assert!(found.iter().all(|f| f.status == "open"), "{found:?}");
    assert!(found.iter().all(|f| f.hits >= 2));
}

#[test]
fn the_cli_plays_the_spine_branches_and_reports() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("explore.db");
    let db_s = db.to_str().unwrap().to_string();
    let args = |v: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = v.iter().map(|a| a.to_string()).collect();
        v.extend(["--db".to_string(), db_s.clone()]);
        v
    };
    let cli = |v: &[&str]| cli::main(args(v));
    cli(&["spine"]).unwrap();
    cli(&["spine", "--trace"]).unwrap();
    cli(&["branch", "--jobs", "2", "--budget", "3000"]).unwrap();
    cli(&["branch", "--from", "after_battle", "--limit", "2"]).unwrap();
    assert!(cli(&["branch", "--from", "nowhere"]).is_err());
    cli(&["branches"]).unwrap();
    cli(&["branches", "--outcome", "rejoined"]).unwrap();
    cli(&["status"]).unwrap();
    cli(&["findings", "--all"]).unwrap();
    cli(&["digest"]).unwrap();
    let map = Emerald::for_shells().new_game().map;
    cli(&["coverage", &map.0.to_string()]).unwrap();
    assert!(cli(&["show", "1"]).is_err(), "no finding 1");
    let code = |v: &[&str]| cli::exit_code(&args(v), &cli::main(args(v)));
    assert_eq!(code(&["repro", "7"]), 2, "no finding 7");
    assert_eq!(code(&["digest"]), 0);
    let unknown = cli(&["run"]).unwrap_err().to_string();
    assert!(unknown.contains("unknown command"), "{unknown}");
    assert!(cli(&["--help"]).is_ok());
    assert!(cli(&["bogus"]).is_err());
    assert!(cli(&["reset"]).is_err(), "needs --yes");
    cli(&["reset", "--yes"]).unwrap();
    assert!(!db.exists());
}

#[test]
fn reset_refuses_a_file_that_is_not_a_db_and_a_busy_db() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let reset = |p: &Path| {
        cli::main(vec![
            "reset".into(),
            "--yes".into(),
            "--db".into(),
            p.to_str().unwrap().into(),
        ])
    };
    let junk = dir.path().join("notes.txt");
    std::fs::write(&junk, "not a database").unwrap();
    assert!(reset(&junk).is_err());
    assert!(junk.exists());

    let game = Emerald::for_shells();
    let db = dir.path().join("explore.db");
    drop(store(&game, &db, &spine_of(&game), &[]));
    let writer = rusqlite::Connection::open(&db).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(reset(&db).is_err(), "a writer holds the db");
    assert!(db.exists());
    writer.execute_batch("ROLLBACK").unwrap();
    drop(writer);
    reset(&db).unwrap();
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
    let game = Emerald::for_shells();
    let s = spine_of(&game);
    let (_, r) = branches(&game, &s, &cfg(2));
    drop(store(&game, &dir.path().join("explore.db"), &s, &r));
    assert_eq!(porcelain(), before);
}
