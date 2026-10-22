//! Task B: novelty archive, integer fitness and the selection score (both flavors).

use content::Emerald;
use engine::checkpoint::BuildHash;
use engine::data::{MapId, ScriptId};
use engine::input::Buttons;
use explore::config::{Config, Weights};
use explore::fitness::{Archive, Evaluator, Novelty, score, selection_score};
use explore::key;
use explore::model::*;
use game_native::Session;
use std::sync::Arc;

/// The root's step: a new game settled to its first capturable frame, summarised by hand.
fn root_step() -> StepResult {
    let mut s = Session::new(Emerald::for_shells());
    let mut summary = SimSummary::default();
    let mut frames = 0;
    while s.world().capturable().is_err() {
        assert!(frames < 2000, "a new game never settles");
        s.step(Buttons::NONE);
        frames += 1;
        let w = s.world();
        if summary.maps.last() != Some(&w.map()) {
            summary.maps.push(w.map());
        }
        let p = w.objects().player();
        summary.cells.insert((w.map(), p.x, p.y));
        summary
            .scripts
            .extend(w.events().iter().filter_map(|e| match e {
                &engine::world::Event::ScriptStarted(id) => Some(id),
                _ => None,
            }));
    }
    summary.frames = frames;
    StepResult {
        macro_: Macro {
            kind: MacroKind::Settle,
            opts: Opts::default(),
        },
        inputs: Vec::new(),
        end: Arc::new(s.checkpoint().unwrap()),
        summary,
        bug: false,
    }
}

fn evaluator() -> Evaluator {
    let cfg = Config {
        key: key::default_key_config(),
        ..Config::default()
    };
    Evaluator::new(Archive::default(), cfg)
}

#[test]
fn a_fresh_archive_scores_the_root_and_a_repeat_scores_less() {
    let step = root_step();
    let mut ev = evaluator();
    let w = ev.cfg.weights.clone();
    let first = ev.evaluate(None, &step);

    let snap = &step.end.world;
    let story_flags = snap
        .vars
        .flags
        .iter()
        .filter(|&&f| key::story_flag(f, &ev.cfg.key))
        .count() as i64;
    let story_vars = snap
        .vars
        .vars
        .iter()
        .filter(|&&(v, x)| x != 0 && key::story_var(v.0, &ev.cfg.key))
        .count() as i64;
    let maps = 1; // the new game never leaves its map
    assert_eq!(step.summary.maps, [snap.field.map]);
    let want = w.flag * story_flags
        + w.var_value * story_vars
        + w.map * maps
        + w.script * step.summary.scripts.len() as i64
        + w.cell * step.summary.cells.len() as i64
        + w.new_key;
    assert_eq!(first.fitness.novelty, want);
    assert_eq!(first.fitness.progress, 0);
    assert_eq!(first.fitness.total, want);
    assert_eq!(first.progress, 0);
    let parts = key::key_parts(snap, &ev.cfg.key);
    assert_eq!(first.key, key::key(&parts));
    assert!(
        first
            .seen
            .contains(&(SeenKind::Map, i64::from(snap.field.map.0)))
    );
    assert!(first.touches.contains(&Touch::Map(snap.field.map)));
    for s in &step.summary.scripts {
        assert!(first.touches.contains(&Touch::Script(*s)));
    }
    if !content::STUB {
        assert!(story_flags > 0, "the new-game script sets story flags");
    }

    let second = ev.evaluate(None, &step);
    assert!(second.fitness.total < first.fitness.total);
    assert_eq!(second.fitness.novelty, 0, "nothing new the second time");
    assert_eq!(second.key, first.key);
    assert_eq!(
        second.seen, first.seen,
        "every sighting is reported, not only new ones"
    );
    assert!(ev.archive.has_key(first.key));
    assert_eq!(
        ev.archive
            .seen_hits(SeenKind::Map, i64::from(snap.field.map.0)),
        2
    );
}

#[test]
fn progress_chains_from_the_parent_row() {
    let step = root_step();
    let mut ev = evaluator();
    let all = explore::goals::goals().len() as u32;
    let mut parent = NodeRow {
        id: 1,
        parent: None,
        depth: 0,
        macro_: step.macro_,
        inputs: Vec::new(),
        frames: 0,
        total_frames: 0,
        end_frame: 0,
        key: AbstractKey(0),
        digest: 0,
        world_hash: 0,
        map: MapId(0),
        pos: (0, 0),
        progress: all,
        fitness: Fitness::default(),
        generation: 0,
        expanded: 0,
        status: NodeStatus::Valid,
        merged_into: None,
        build: BuildHash {
            engine: 0,
            content: 0,
        },
    };
    let e = ev.evaluate(Some(&parent), &step);
    assert_eq!(e.progress, all, "progress never goes back");
    assert_eq!(e.fitness.progress, 0);
    parent.progress = 0;
    assert_eq!(ev.evaluate(Some(&parent), &step).progress, 0);
}

#[test]
fn score_is_a_weighted_integer_sum() {
    let w = Weights::default();
    let n = Novelty {
        flags: 1,
        var_values: 2,
        maps: 3,
        scripts: 4,
        cells: 5,
        new_key: true,
    };
    let f = score(&n, 2, 1, &w);
    let novelty = w.flag + 2 * w.var_value + 3 * w.map + 4 * w.script + 5 * w.cell + w.new_key;
    assert_eq!(f.novelty, novelty);
    assert_eq!(f.progress, 2 * w.goal);
    assert_eq!(f.bug, w.bug);
    assert_eq!(f.balance, 0);
    assert_eq!(f.total, novelty + 2 * w.goal + w.bug);
    let none = score(&Novelty::default(), 0, 0, &w);
    assert_eq!(none, Fitness::default());

    // Extreme weights saturate instead of overflowing.
    let huge = Weights {
        flag: i64::MAX,
        goal: i64::MAX,
        bug: i64::MAX,
        ..Weights::default()
    };
    let f = score(&n, 2, 1, &huge);
    assert_eq!(
        (f.novelty, f.progress, f.bug, f.total),
        (i64::MAX, i64::MAX, i64::MAX, i64::MAX)
    );
    let low = Weights {
        flag: i64::MIN,
        ..Weights::default()
    };
    assert!(score(&n, 0, 0, &low).novelty < i64::MIN / 2);
}

#[test]
fn selection_decays_with_hits_age_and_children() {
    let step = root_step();
    let mut ev = evaluator();
    let e = ev.evaluate(None, &step);
    let cfg = Config {
        age_penalty: 3,
        expanded_penalty: 7,
        ..Config::default()
    };
    let row = NodeRow {
        id: 1,
        parent: None,
        depth: 0,
        macro_: step.macro_,
        inputs: Vec::new(),
        frames: 0,
        total_frames: 0,
        end_frame: 0,
        key: e.key,
        digest: 0,
        world_hash: 0,
        map: MapId(0),
        pos: (0, 0),
        progress: 0,
        fitness: Fitness {
            total: 1000,
            ..Fitness::default()
        },
        generation: 5,
        expanded: 2,
        status: NodeStatus::Valid,
        merged_into: None,
        build: BuildHash {
            engine: 0,
            content: 0,
        },
    };
    assert_eq!(selection_score(&row, 5, 0, &cfg), 1000 - 2 * 7);
    assert_eq!(selection_score(&row, 9, 0, &cfg), 1000 - 4 * 3 - 2 * 7);
    assert_eq!(selection_score(&row, 5, 1, &cfg), 500 - 2 * 7);
    assert_eq!(selection_score(&row, 5, 3, &cfg), 250 - 2 * 7);
    // A generation before the node's own (never happens) counts as age 0.
    assert_eq!(selection_score(&row, 0, 0, &cfg), 1000 - 2 * 7);
    // Old enough, a strong node loses to a fresh weak one.
    let fresh = NodeRow {
        generation: 400,
        fitness: Fitness {
            total: 10,
            ..Fitness::default()
        },
        expanded: 0,
        ..row.clone()
    };
    assert!(selection_score(&row, 400, 0, &cfg) < selection_score(&fresh, 400, 0, &cfg));
}

#[test]
fn archive_from_db_round_trips() {
    let step = root_step();
    let mut ev = evaluator();
    ev.evaluate(None, &step);
    ev.evaluate(None, &step);
    let a = &ev.archive;
    let seen = a.seen_rows();
    let keys: Vec<(AbstractKey, u32)> = a.keys().into_iter().map(|k| (k, 1)).collect();
    let cells = a.cell_rows();
    assert!(!seen.is_empty() && !keys.is_empty() && !cells.is_empty());
    let b = Archive::from_db(&seen, &keys, &cells);
    assert_eq!(&b, a);

    // Every kind, including a var value with its high bit set.
    let rows = [
        (SeenKind::Flag, 0x52, 3),
        (SeenKind::VarValue, (0x4050i64 << 16) | 0xFFFF, 1),
        (SeenKind::Map, 0x0010, 2),
        (SeenKind::Script, i64::from(ScriptId(7).0), 4),
    ];
    let c = Archive::from_db(
        &rows,
        &[(AbstractKey(u64::MAX), 9)],
        &[(MapId(1), -1, 2, 5)],
    );
    assert_eq!(c.seen_rows(), rows);
    for (kind, id, hits) in rows {
        assert_eq!(c.seen_hits(kind, id), hits);
    }
    assert!(c.has_key(AbstractKey(u64::MAX)));
    assert!(!c.has_key(AbstractKey(0)));

    // Ids out of range for their kind are skipped.
    let bad = [
        (SeenKind::Flag, 0x1_0000, 1),
        (SeenKind::Map, -1, 1),
        (SeenKind::Script, i64::MAX, 1),
        (SeenKind::VarValue, -5, 1),
        (SeenKind::VarValue, 1i64 << 40, 1),
    ];
    let d = Archive::from_db(&bad, &[], &[]);
    assert!(d.seen_rows().is_empty());
    for (kind, id, _) in bad {
        assert_eq!(d.seen_hits(kind, id), 0);
    }
    assert_eq!(c.cell_hits(MapId(1), -1, 2), 5);
    assert_eq!(c.cell_hits(MapId(1), 0, 0), 0);
}

#[test]
fn a_loaded_archive_scores_like_the_live_one() {
    let step = root_step();
    let mut live = evaluator();
    live.evaluate(None, &step);
    let mut loaded = evaluator();
    loaded.archive = Archive::from_db(
        &live.archive.seen_rows(),
        &live
            .archive
            .keys()
            .into_iter()
            .map(|k| (k, 1))
            .collect::<Vec<_>>(),
        &live.archive.cell_rows(),
    );
    let a = live.evaluate(None, &step);
    let b = loaded.evaluate(None, &step);
    assert_eq!(
        (a.key, a.fitness, a.progress),
        (b.key, b.fitness, b.progress)
    );
}
