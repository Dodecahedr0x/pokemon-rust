//! Milestone 8 Task K phase 2: the explorer reads the engine's events (`World::events`), not a
//! `GameData` wrapper. `Unsupported` findings come from script reports, the stuck and
//! round-trip signatures carry the last script, and every test wrapper forwards every provided
//! `GameData` method.

use content::Emerald;
use engine::data::{GameData, MapId, ScriptId};
use engine::script::{Ctx, Report, Script};
use explore::FindingKind;
use explore::config::{Budget, Config, Limits};
use explore::expand;
use explore::findings::{self, signature, signature_of};
use explore::model::{FindingDraft, Macro, MacroKind, Opts};
use explore::observe::NoObserver;
use explore::policy::Policy;
use explore::testing::{
    Answers, DRIFT_CALLS, Drifting, FADE_TRAP_SCRIPT, FadeTrap, GrassPocket, INJECTED_REPORT,
    PanicOnScript, Rebuilt, Reporting, WallAround, same_answers,
};
use explore::wire::{self, Explore};
use game_native::Session;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn every_wrapper_answers_every_provided_method_like_its_inner_game() {
    let inner = Answers {
        inner: Emerald::for_shells(),
    };
    assert!(
        same_answers(&Emerald::for_shells(), &inner).is_err(),
        "the fixture answers on its own"
    );
    let map = inner.new_game().map;
    let drifting = Drifting::new(inner);
    // Past the drift: then it answers like the inner game.
    for _ in 0..DRIFT_CALLS {
        assert_eq!(drifting.wild_encounters(map, 1), None);
    }
    let checks: [(&str, Result<(), String>); 8] = [
        (
            "PanicOnScript",
            same_answers(
                &PanicOnScript {
                    inner,
                    script: ScriptId(u16::MAX),
                },
                &inner,
            ),
        ),
        (
            "WallAround",
            same_answers(
                &WallAround {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        (
            "GrassPocket",
            same_answers(
                &GrassPocket {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        ("Drifting", same_answers(&drifting, &inner)),
        (
            "FadeTrap",
            same_answers(
                &FadeTrap {
                    inner,
                    map,
                    at: (1, 1),
                },
                &inner,
            ),
        ),
        (
            "Rebuilt",
            same_answers(
                &Rebuilt {
                    inner,
                    build_id: inner.build_id(),
                },
                &inner,
            ),
        ),
        ("Reporting", same_answers(&Reporting { inner }, &inner)),
        ("itself", same_answers(&inner, &inner)),
    ];
    for (name, r) in checks {
        assert_eq!(r, Ok(()), "{name}");
    }
}

fn draft(kind: FindingKind, script: Option<ScriptId>) -> FindingDraft {
    FindingDraft {
        kind,
        message: "idle but not capturable for 41 frames: Fade".into(),
        location: None,
        macro_: None,
        tail: Vec::new(),
        map: MapId(0x0101),
        pos: (3, 4),
        script,
        log: Vec::new(),
    }
}

#[test]
fn stuck_and_round_trip_signatures_carry_the_last_script() {
    for kind in [FindingKind::CapturableStuck, FindingKind::RoundTrip] {
        let a = signature(&draft(kind, Some(ScriptId(5))));
        assert_ne!(a, signature(&draft(kind, Some(ScriptId(6)))), "{kind:?}");
        assert_ne!(a, signature(&draft(kind, None)), "{kind:?}");
    }
    for kind in [
        FindingKind::Panic,
        FindingKind::PlannerDivergence,
        FindingKind::Unsupported,
    ] {
        assert_eq!(
            signature(&draft(kind, Some(ScriptId(5)))),
            signature(&draft(kind, None)),
            "{kind:?}"
        );
    }
    let f = draft(FindingKind::Unsupported, None);
    assert_eq!(
        signature(&f),
        signature_of(f.kind, &f.message, None, f.map, Some(ScriptId(9)))
    );
}

fn config(threads: usize) -> Config {
    Config {
        threads,
        batch: 0,
        ..Config::default()
    }
}

/// `generations` generations of `game` on a fresh db at `path`.
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

#[test]
fn unsupported_findings_come_from_script_reports_and_reproduce() {
    let dir = tempfile::tempdir().unwrap();
    let game = Reporting {
        inner: Emerald::for_shells(),
    };
    let ex = explore(game, &dir.path().join("t2.db"), 2, 2);
    let fs: Vec<_> = ex
        .db
        .findings()
        .unwrap()
        .into_iter()
        .filter(|f| f.kind == FindingKind::Unsupported)
        .collect();
    assert!(!fs.is_empty(), "the new game's script reports");
    assert_eq!(ex.db.report_count().unwrap(), fs.len() as u64);
    let mut maps = BTreeSet::new();
    for f in &fs {
        assert_eq!(f.message, format!("unsupported: {INJECTED_REPORT}"));
        assert!(maps.insert(f.map), "one finding per map: {fs:?}");
        wire::reproduce(&game, &ex.db, f.id, &ex.cfg)
            .unwrap()
            .unwrap_or_else(|e| panic!("{f:?}: {e}"));
        // Not on the inner game, which reports nothing.
        assert!(
            wire::reproduce(&game.inner, &ex.db, f.id, &ex.cfg)
                .unwrap()
                .is_err()
        );
    }
    // The run goes on: report-only findings never stop a job.
    assert!(ex.stats().unwrap().nodes > 1);
    // And they are deterministic like the rest of the db.
    let one = explore(game, &dir.path().join("t1.db"), 1, 2);
    assert_eq!(
        one.db.content_digest().unwrap(),
        ex.db.content_digest().unwrap()
    );
}

/// The new game's script reports [`NAMES`] (more than `expand::MAX_REPORTS`) on its first
/// frame.
#[derive(Clone, Copy)]
struct ManyReports<G> {
    inner: G,
}

const NAMES: [&str; 20] = [
    "special A",
    "special B",
    "special C",
    "special D",
    "special E",
    "special F",
    "special G",
    "special H",
    "special I",
    "special J",
    "special K",
    "special L",
    "special M",
    "special N",
    "special O",
    "special P",
    "special Q",
    "special R",
    "special S",
    "special T",
];

impl<G: GameData> GameData for ManyReports<G> {
    explore::forward_game_data!(inner; map has_map special_ids sprite player_sprite font window
        text ui_text behavior new_game build_id battle encounters trainer battle_text battle_gfx
        heal_location wild_encounters m8);

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        let inner = self.inner.script(id, ctx.clone());
        if id != content::NEW_GAME_SCRIPT {
            return inner;
        }
        Box::pin(async move {
            for name in NAMES {
                ctx.report(Report::Unsupported(name)).await;
            }
            inner.await;
        })
    }
}

#[test]
fn reports_past_max_reports_are_counted_in_a_note() {
    let game = ManyReports {
        inner: Emerald::for_shells(),
    };
    let mut s = Session::new(game);
    let settle = Macro {
        kind: MacroKind::Settle,
        opts: Opts::default(),
    };
    let ex = match expand::expand(
        &mut s,
        &settle,
        &mut Policy::new(),
        &mut NoObserver,
        &Limits::default(),
    ) {
        Ok(ex) => ex,
        Err((e, _)) => panic!("{e:?}"),
    };
    assert_eq!(ex.reports.len(), expand::MAX_REPORTS);
    let dropped = NAMES.len() - expand::MAX_REPORTS;
    assert!(
        ex.notes
            .contains(&format!("{dropped} script reports dropped (MAX_REPORTS)")),
        "{:?}",
        ex.notes
    );
}

/// A walkable neighbour of the new game's cell.
fn trap_cell(game: &impl GameData) -> (i16, i16) {
    let ng = game.new_game();
    let m = game.map(ng.map);
    [(0, 1), (-1, 0), (1, 0), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| (ng.x + dx, ng.y + dy))
        .find(|&(x, y)| {
            x >= 0
                && y >= 0
                && x < m.width as i16
                && y < m.height as i16
                && engine::data::block_collision(
                    m.blocks[y as usize * m.width as usize + x as usize],
                ) == 0
        })
        .expect("a walkable neighbour")
}

#[test]
fn a_fade_trap_is_a_capturable_stuck_finding_signed_with_its_script() {
    if !content::STUB {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let inner = Emerald::for_shells();
    let game = FadeTrap {
        inner,
        map: inner.new_game().map,
        at: trap_cell(&inner),
    };
    let ex = explore(game, &dir.path().join("explore.db"), 2, 3);
    let fs = ex.db.findings().unwrap();
    let f = fs
        .iter()
        .find(|f| f.kind == FindingKind::CapturableStuck)
        .unwrap_or_else(|| panic!("a CapturableStuck row: {fs:?}"));
    assert_eq!(f.script, Some(FADE_TRAP_SCRIPT));
    wire::reproduce(&game, &ex.db, f.id, &ex.cfg)
        .unwrap()
        .unwrap();
    // The same row without its script is another signature: it does not reproduce.
    let (row, trace) = wire::finding_trace(&game, &ex.db, f.id, 0).unwrap();
    let mut want = wire::draft_of(&row);
    findings::reproduce(&game, &trace, &want, &ex.cfg.detect).unwrap();
    want.script = None;
    assert!(findings::reproduce(&game, &trace, &want, &ex.cfg.detect).is_err());
}
