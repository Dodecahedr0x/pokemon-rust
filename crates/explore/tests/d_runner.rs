//! Task D: graph, selection, mutation, parallel generations and merge, with a fake `Evaluate`
//! and a fake simulate closure (no game logic; one real checkpoint is the template of every
//! fake end state).

use content::Emerald;
use engine::Buttons;
use engine::checkpoint::{Checkpoint, state_digest};
use engine::data::MapId;
use explore::config::Config;
use explore::graph::{Graph, Rng64, mix};
use explore::mutate::{MutationOp, apply, choose, mutate};
use explore::runner::{DEFAULT_BATCH, ROOT_MACRO, Runner, batch_size, jobs, run_jobs};
use explore::select::{selection_score, tournament};
use explore::*;
use game_native::Session;
use game_native::trace::Run;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

/// A capturable checkpoint of a new game (the template of the fake states).
fn base() -> &'static Checkpoint {
    static BASE: OnceLock<Checkpoint> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut s = Session::new(Emerald::for_shells());
        for _ in 0..20_000 {
            if let Ok(cp) = s.checkpoint() {
                return cp;
            }
            s.step(Buttons::NONE);
        }
        panic!("a new game never became capturable");
    })
}

/// A fake end state: `tag` picks the digest (via `player_id`), `key` the player's x (the fake
/// key).
fn state(tag: u64, key: u64) -> Arc<Checkpoint> {
    let mut cp = base().clone();
    cp.world.player_id = tag as u32 ^ (tag >> 32) as u32;
    cp.world.objects.objs[0].pos = (key as i16, 0);
    Arc::new(cp)
}

fn step(tag: u64, key: u64, frames: u32) -> StepResult {
    let mut summary = SimSummary {
        frames,
        ..SimSummary::default()
    };
    summary.cells.insert((MapId(1), key as i16, 0));
    StepResult {
        macro_: Macro {
            kind: MacroKind::WalkTo {
                x: key as i16,
                y: 0,
            },
            opts: Opts::default(),
        },
        inputs: vec![Run {
            keys: Buttons::NONE,
            frames,
        }],
        end: state(tag, key),
        summary,
        bug: false,
    }
}

/// Key = the end x; fitness = the tag's low bits; seen = first sightings of the key.
#[derive(Default)]
struct FakeEval {
    seen: BTreeSet<i64>,
}

impl Evaluate for FakeEval {
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        let x = step.end.world.objects.objs[0].pos.0;
        let tag = u64::from(step.end.world.player_id);
        let first = self.seen.insert(i64::from(x));
        Evaluation {
            key: AbstractKey(x as u64),
            progress: parent.map_or(0, |p| p.progress),
            fitness: Fitness {
                novelty: i64::from(first) * 50,
                total: (tag % 997) as i64 + i64::from(first) * 50,
                ..Fitness::default()
            },
            seen: if first {
                vec![(SeenKind::Map, i64::from(x))]
            } else {
                vec![]
            },
            touches: vec![Touch::Map(MapId(1))],
        }
    }
}

/// A deterministic fake job: a function of the job only.
fn fake_sim(job: &Job) -> JobResult {
    let base_tag = mix(job.parent.unwrap_or(0) as u64 ^ 0xF00D);
    let mut steps = Vec::new();
    let mut stop = None;
    match &job.plan {
        Plan::Random { len, seed } => {
            for i in 0..u64::from(*len) {
                let t = mix(seed ^ base_tag ^ i);
                steps.push(step(t, t % 48, 1 + (t >> 16) as u32 % 300));
            }
            match seed % 11 {
                0 => stop = Some(Stop::Pruned("no path".into())),
                1 => stop = Some(Stop::Finding(finding(*seed, &steps))),
                _ => {}
            }
        }
        Plan::Given(ms) => {
            for (i, m) in ms.iter().enumerate() {
                let t = mix(base_tag ^ (i as u64) ^ u64::from(m.opts.choice) << 8)
                    ^ u64::from(m.opts.run_wild);
                let key = if job.parent.is_none() { 0 } else { t % 48 };
                let mut s = step(t, key, 10 + (t >> 20) as u32 % 200);
                s.macro_ = *m;
                steps.push(s);
            }
        }
        Plan::Softlock { .. } => stop = Some(Stop::NotTrapped),
    }
    // Uneven work, so a pool finishes jobs out of order.
    let mut spin = base_tag;
    for _ in 0..(base_tag % 20_000) {
        spin = mix(spin);
    }
    std::hint::black_box(spin);
    JobResult {
        index: job.index,
        parent: job.parent,
        steps,
        stop,
        probe_frames: 0,
        slow_frames: 0,
        notes: Vec::new(),
    }
}

/// A finding after `steps`: its tail starts at the job parent's end state, so it holds the
/// steps' inputs, then 7 frames of the failing macro.
fn finding(seed: u64, steps: &[StepResult]) -> FindingDraft {
    let mut tail: Vec<Run> = steps.iter().flat_map(|s| s.inputs.clone()).collect();
    tail.push(Run {
        keys: Buttons::A,
        frames: 7,
    });
    FindingDraft {
        kind: FindingKind::Panic,
        message: format!("fake {seed}"),
        location: None,
        macro_: None,
        tail,
        map: MapId(1),
        pos: (0, 0),
        script: None,
        log: vec![],
    }
}

fn cfg(seed: u64) -> Config {
    Config {
        seed,
        batch: 16,
        snapshot_every_depth: 2,
        ..Config::default()
    }
}

/// Runs `n` generations in `pool`; returns the runner and every delta.
fn run(
    seed: u64,
    n: u32,
    pool: Option<&rayon::ThreadPool>,
) -> (Runner, Vec<GenerationDelta>, FakeEval) {
    run_with(cfg(seed), n, pool)
}

fn run_with(
    cfg: Config,
    n: u32,
    pool: Option<&rayon::ThreadPool>,
) -> (Runner, Vec<GenerationDelta>, FakeEval) {
    let mut r = Runner::new(cfg, vec![], 0);
    let mut eval = FakeEval::default();
    let mut deltas = Vec::new();
    for _ in 0..n {
        let js = jobs(r.plan(), |_| None);
        let results = match pool {
            Some(p) => p.install(|| run_jobs(&js, fake_sim)),
            None => run_jobs(&js, fake_sim),
        };
        let indices: Vec<u32> = results.iter().map(|r| r.index).collect();
        assert_eq!(
            indices,
            (0..js.len() as u32).collect::<Vec<_>>(),
            "job order"
        );
        deltas.push(r.merge(results, &mut eval));
    }
    (r, deltas, eval)
}

/// The rows a db holds after applying `deltas` in order (Task C's `apply` semantics).
fn applied(deltas: &[GenerationDelta]) -> Vec<NodeRow> {
    let mut rows: BTreeMap<NodeId, NodeRow> = BTreeMap::new();
    for d in deltas {
        for &(old, new) in &d.merged {
            let r = rows.get_mut(&old).unwrap();
            r.status = NodeStatus::Merged;
            r.merged_into = Some(new);
        }
        for &(id, n) in &d.expanded {
            rows.get_mut(&id).unwrap().expanded += n;
        }
        for n in &d.nodes {
            assert!(rows.insert(n.id, n.clone()).is_none(), "id reused");
        }
    }
    rows.into_values().collect()
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
}

#[test]
fn rng_is_splitmix_and_below_stays_in_range() {
    // splitmix64 reference values for state 0.
    let mut r = Rng64(0);
    assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    let mut r = Rng64(42);
    let mut hits = [0u32; 3];
    for _ in 0..3000 {
        hits[r.below(3) as usize] += 1;
    }
    assert!(hits.iter().all(|&h| (900..1100).contains(&h)), "{hits:?}");
    assert_eq!(r.below(0), 0);
    assert_eq!(r.below(1), 0);
}

#[test]
fn an_empty_graph_plans_the_root() {
    let mut r = Runner::new(cfg(1), vec![], 0);
    assert_eq!(r.plan(), vec![(None, Plan::Given(vec![ROOT_MACRO]))]);
    let js = jobs(r.plan(), |_| None);
    let d = r.merge(run_jobs(&js, fake_sim), &mut FakeEval::default());
    assert_eq!(d.generation, 0);
    assert_eq!(r.generation, 1);
    assert_eq!(d.nodes.len(), 1);
    let root = &d.nodes[0];
    assert_eq!((root.id, root.parent, root.depth), (1, None, 0));
    assert_eq!(root.macro_.kind, MacroKind::Settle);
    assert_eq!(root.status, NodeStatus::Valid);
    // Depth 0 is a multiple of snapshot_every_depth: the root goes to the db.
    assert_eq!(d.snapshots.iter().map(|s| s.0).collect::<Vec<_>>(), [1]);
    assert_eq!(r.take_fresh().len(), 1);
    assert!(d.expanded.is_empty(), "the root job has no parent");
    // The next plan selects from the root.
    let planned = r.plan();
    assert_eq!(planned.len(), 16);
    assert!(planned.iter().all(|(p, _)| *p == Some(1)));
}

#[test]
fn plan_and_merge_are_deterministic_for_a_seed() {
    let (a, da, _) = run(7, 12, None);
    let (b, db, _) = run(7, 12, None);
    assert_eq!(format!("{da:?}"), format!("{db:?}"));
    assert_eq!(a.graph, b.graph);
    assert!(a.graph.len() > 40, "{} nodes", a.graph.len());
    let (c, _, _) = run(8, 12, None);
    assert_ne!(a.graph, c.graph, "another seed explores differently");
}

#[test]
fn one_and_four_threads_give_identical_deltas() {
    let (a, da, _) = run(0x5EED, 15, Some(&pool(1)));
    let (b, db, _) = run(0x5EED, 15, Some(&pool(4)));
    assert_eq!(da.len(), db.len());
    for (x, y) in da.iter().zip(&db) {
        assert_eq!(
            format!("{x:?}"),
            format!("{y:?}"),
            "generation {}",
            x.generation
        );
    }
    assert_eq!(a.graph, b.graph);
    let stats: u32 = da.iter().map(|d| d.stats.duplicates).sum();
    assert!(stats > 0, "the fake key space makes duplicates");
    assert!(da.iter().any(|d| !d.merged.is_empty()), "and supersessions");
    assert!(da.iter().any(|d| !d.findings.is_empty()), "and findings");
}

#[test]
fn the_shipped_default_config_gives_identical_deltas_on_one_and_eight_threads() {
    // `Config::default()` as shipped (batch 128 since Task H).
    let cfg = Config::default();
    assert_eq!(batch_size(&cfg), 128);
    let (a, da, _) = run_with(cfg.clone(), 8, Some(&pool(1)));
    let (b, db, _) = run_with(cfg, 8, Some(&pool(8)));
    assert_eq!(format!("{da:?}"), format!("{db:?}"));
    assert_eq!(a.graph, b.graph);
    assert_eq!(da[1].stats.jobs, 128);
}

#[test]
fn the_default_batch_gives_identical_deltas_on_one_and_eight_threads() {
    // `batch: 0` is the runner's default batch (`Config::default` sets 128 since Task H).
    let cfg = Config {
        batch: 0,
        ..Config::default()
    };
    assert_eq!(batch_size(&cfg), DEFAULT_BATCH);
    let (a, da, _) = run_with(cfg.clone(), 8, Some(&pool(1)));
    let (b, db, _) = run_with(cfg, 8, Some(&pool(8)));
    assert_eq!(format!("{da:?}"), format!("{db:?}"));
    assert_eq!(a.graph, b.graph);
    assert_eq!(da[1].stats.jobs as usize, DEFAULT_BATCH);
}

#[test]
fn from_rows_of_the_merge_output_equals_the_graph() {
    let (r, deltas, _) = run(3, 10, None);
    // The applied rows and key hits (the `key_hit` table) rebuild the graph.
    let hits: Vec<(AbstractKey, u32)> = deltas.iter().flat_map(|d| d.key_hits.clone()).collect();
    assert!(!hits.is_empty(), "duplicates count key hits without rows");
    let mut g = Graph::from_rows(applied(&deltas));
    g.add_key_hits(&hits);
    assert_eq!(g, r.graph);
    assert!(r.graph.nodes().any(|n| r.graph.key_hits(n.key) > 0));
    // A runner resumed from them plans and merges what a straight one does.
    let mut resumed = Runner::new(cfg(3), applied(&deltas), r.generation);
    resumed.graph.add_key_hits(&hits);
    let mut straight = r;
    let (mut ea, mut eb) = (FakeEval::default(), FakeEval::default());
    for d in &deltas {
        // The fake evaluator's archive is the keys seen, rebuilt from the delta's sightings.
        for &(_, x, _) in &d.seen {
            ea.seen.insert(x);
            eb.seen.insert(x);
        }
    }
    for _ in 0..3 {
        let (pa, pb) = (resumed.plan(), straight.plan());
        assert_eq!(pa, pb);
        let da = resumed.merge(run_jobs(&jobs(pa, |_| None), fake_sim), &mut ea);
        let db = straight.merge(run_jobs(&jobs(pb, |_| None), fake_sim), &mut eb);
        assert_eq!(format!("{da:?}"), format!("{db:?}"));
    }
    assert_eq!(resumed.graph, straight.graph);
}

#[test]
fn frontier_ids_ascend_and_hold_only_valid_nodes() {
    let (r, _, _) = run(5, 8, None);
    let ids: Vec<NodeId> = r.graph.frontier().map(|n| n.id).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]));
    assert!(
        r.graph
            .frontier()
            .all(|n| n.status == NodeStatus::Valid && n.merged_into.is_none())
    );
    // One live node per key.
    let mut live = BTreeSet::new();
    for n in r.graph.nodes() {
        if matches!(n.status, NodeStatus::Valid | NodeStatus::Stale) {
            assert!(live.insert(n.key), "two live nodes for {:?}", n.key);
            assert_eq!(r.graph.best(n.key), Some(n.id));
        }
    }
}

/// A runner holding the root (node 1, 100 frames, key 0).
fn rooted() -> (Runner, FakeEval) {
    let mut r = Runner::new(cfg(1), vec![], 0);
    let mut eval = FakeEval::default();
    let root = JobResult {
        index: 0,
        parent: None,
        steps: vec![step(1, 0, 100)],
        stop: None,
        probe_frames: 0,
        slow_frames: 0,
        notes: Vec::new(),
    };
    r.merge(vec![root], &mut eval);
    (r, eval)
}

fn job(index: u32, parent: NodeId, steps: Vec<StepResult>, stop: Option<Stop>) -> JobResult {
    JobResult {
        index,
        parent: Some(parent),
        steps,
        stop,
        probe_frames: 0,
        slow_frames: 0,
        notes: Vec::new(),
    }
}

#[test]
fn fewer_frames_supersede_and_more_frames_are_duplicates() {
    let (mut r, mut eval) = rooted();
    // Generation 1: key 5 at 100 + 50 frames.
    let d = r.merge(vec![job(0, 1, vec![step(10, 5, 50)], None)], &mut eval);
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(r.graph.best(AbstractKey(5)), Some(2));
    assert_eq!(d.expanded, [(1, 1)]);

    // More frames: a duplicate, no row, but a key hit (Milestone 8 Task K: the `key_hit`
    // table counts every duplicate, not only the merged rows).
    let d = r.merge(vec![job(0, 1, vec![step(11, 5, 80)], None)], &mut eval);
    assert!(d.nodes.is_empty());
    assert_eq!(d.stats.duplicates, 1);
    assert_eq!(d.key_hits, [(AbstractKey(5), 1)]);
    assert_eq!(r.graph.key_hits(AbstractKey(5)), 1);
    assert_eq!(r.graph.best(AbstractKey(5)), Some(2));
    // Its cells are attributed to the live node.
    assert_eq!(d.cells, [(MapId(1), 5, 0, 2)]);

    // Equal frames: still a duplicate.
    let d = r.merge(vec![job(0, 1, vec![step(12, 5, 50)], None)], &mut eval);
    assert!(d.nodes.is_empty());
    assert_eq!(r.graph.key_hits(AbstractKey(5)), 2);

    // Fewer frames: a new live node, the old one merged into it.
    let d = r.merge(vec![job(0, 1, vec![step(13, 5, 20)], None)], &mut eval);
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(d.nodes[0].id, 3);
    assert_eq!(d.nodes[0].total_frames, 120);
    assert_eq!(d.merged, [(2, 3)]);
    assert_eq!(r.graph.best(AbstractKey(5)), Some(3));
    let old = r.graph.node(2).unwrap();
    assert_eq!((old.status, old.merged_into), (NodeStatus::Merged, Some(3)));
    // Two duplicates and the superseded row.
    assert_eq!(r.graph.key_hits(AbstractKey(5)), 3);
    assert_eq!(r.graph.frontier().map(|n| n.id).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(r.graph.node(1).unwrap().expanded, 4);
}

/// [`FakeEval`] with progress 1 for the step of tag 99 (Task G: progress before frames).
struct ProgressEval(FakeEval);

impl Evaluate for ProgressEval {
    fn evaluate(&mut self, parent: Option<&NodeRow>, step: &StepResult) -> Evaluation {
        let mut e = self.0.evaluate(parent, step);
        if step.end.world.player_id == 99 {
            e.progress = 1;
        }
        e
    }
}

#[test]
fn more_goals_supersede_even_with_more_frames() {
    let (mut r, eval) = rooted();
    let mut eval = ProgressEval(eval);
    r.merge(vec![job(0, 1, vec![step(10, 5, 50)], None)], &mut eval);
    let d = r.merge(vec![job(0, 1, vec![step(99, 5, 80)], None)], &mut eval);
    assert_eq!(d.merged, [(2, 3)]);
    assert_eq!(r.graph.best(AbstractKey(5)), Some(3));
    // Fewer frames but fewer goals: a duplicate.
    let d = r.merge(vec![job(0, 1, vec![step(11, 5, 20)], None)], &mut eval);
    assert!(d.nodes.is_empty());
    assert_eq!(d.stats.duplicates, 1);
}

#[test]
fn a_supersession_inside_one_delta_writes_the_old_row_merged() {
    let (mut r, mut eval) = rooted();
    let d = r.merge(
        vec![
            job(0, 1, vec![step(20, 9, 90)], None),
            job(1, 1, vec![step(21, 9, 30)], None),
        ],
        &mut eval,
    );
    assert!(d.merged.is_empty(), "nothing to update before the inserts");
    assert_eq!(d.nodes.len(), 2);
    assert_eq!(
        (d.nodes[0].status, d.nodes[0].merged_into),
        (NodeStatus::Merged, Some(3))
    );
    assert_eq!(d.nodes[1].status, NodeStatus::Valid);
    assert_eq!(d.expanded, [(1, 2)]);
    assert_eq!(d.stats.inserted, 2);
}

#[test]
fn steps_after_a_duplicate_replay_from_its_real_end_state() {
    let (mut r, mut eval) = rooted();
    r.merge(vec![job(0, 1, vec![step(30, 5, 10)], None)], &mut eval); // node 2, key 5

    // Same key, more frames, another state, a step follows: a merged history row chains it.
    let d = r.merge(
        vec![job(0, 1, vec![step(31, 5, 40), step(32, 6, 10)], None)],
        &mut eval,
    );
    assert_eq!(d.nodes.len(), 2);
    let (hist, next) = (&d.nodes[0], &d.nodes[1]);
    assert_eq!(
        (hist.status, hist.merged_into, hist.key),
        (NodeStatus::Merged, Some(2), AbstractKey(5))
    );
    assert_eq!(
        (next.parent, next.depth, next.total_frames),
        (Some(hist.id), 2, 150)
    );
    assert_eq!(d.stats.duplicates, 1);
    assert_eq!(d.stats.inserted, 1);
    assert_eq!(r.graph.best(AbstractKey(5)), Some(2));

    // The same state as the live node (equal digest): chain from it, no row.
    let d = r.merge(
        vec![job(0, 1, vec![step(30, 5, 10), step(33, 7, 10)], None)],
        &mut eval,
    );
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(d.nodes[0].parent, Some(2));
    assert_eq!(d.nodes[0].key, AbstractKey(7));
}

/// The root (node 1), node 2 on key 5 and node 3 on key 6, both 10 frames below the root.
fn two_keys() -> (Runner, FakeEval) {
    let (mut r, mut eval) = rooted();
    r.merge(
        vec![
            job(0, 1, vec![step(30, 5, 10)], None),
            job(1, 1, vec![step(40, 6, 10)], None),
        ],
        &mut eval,
    );
    assert_eq!(r.graph.best(AbstractKey(6)), Some(3));
    (r, eval)
}

#[test]
fn held_duplicates_with_nothing_inserted_below_leave_only_key_hits() {
    let (mut r, mut eval) = two_keys();
    // Key 5 again (another state, a step follows: held), then key 6 again as the last step.
    let d = r.merge(
        vec![job(0, 1, vec![step(31, 5, 40), step(41, 6, 40)], None)],
        &mut eval,
    );
    assert!(d.nodes.is_empty());
    assert_eq!(d.stats.duplicates, 2);
    assert_eq!(d.key_hits, [(AbstractKey(5), 1), (AbstractKey(6), 1)]);
    assert_eq!(
        (
            r.graph.key_hits(AbstractKey(5)),
            r.graph.key_hits(AbstractKey(6))
        ),
        (1, 1)
    );
    // Their cells are credited to the live nodes.
    assert_eq!(d.cells, [(MapId(1), 5, 0, 2), (MapId(1), 6, 0, 3)]);
}

#[test]
fn chained_held_duplicates_are_written_parents_first_below_an_inserted_step() {
    let (mut r, mut eval) = two_keys();
    let d = r.merge(
        vec![job(
            0,
            1,
            vec![step(32, 5, 40), step(42, 6, 40), step(50, 8, 10)],
            None,
        )],
        &mut eval,
    );
    let rows: Vec<_> = d
        .nodes
        .iter()
        .map(|n| (n.id, n.parent, n.key.0, n.status, n.merged_into))
        .collect();
    assert_eq!(
        rows,
        [
            (4, Some(1), 5, NodeStatus::Merged, Some(2)),
            (5, Some(4), 6, NodeStatus::Merged, Some(3)),
            (6, Some(5), 8, NodeStatus::Valid, None),
        ]
    );
    assert_eq!(d.nodes[2].total_frames, 100 + 40 + 40 + 10);
    // Written rows count as rows, not as `key_hit`s.
    assert!(d.key_hits.is_empty());
    assert_eq!(r.graph.key_hits(AbstractKey(5)), 1);
}

#[test]
fn a_held_duplicate_bypassed_by_an_equal_state_counts_a_key_hit() {
    let (mut r, mut eval) = two_keys();
    // Held on key 5, then node 3's own state (tag 40, key 6: equal digest) chains from node 3,
    // so the held step is never needed as a parent.
    let d = r.merge(
        vec![job(
            0,
            1,
            vec![step(33, 5, 40), step(40, 6, 10), step(51, 9, 10)],
            None,
        )],
        &mut eval,
    );
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(
        (d.nodes[0].parent, d.nodes[0].key),
        (Some(3), AbstractKey(9))
    );
    assert_eq!(d.key_hits, [(AbstractKey(5), 1), (AbstractKey(6), 1)]);
}

#[test]
fn findings_attach_to_the_node_their_tail_starts_from() {
    let (mut r, mut eval) = rooted();
    let d = r.merge(
        vec![
            {
                let steps = vec![step(40, 3, 25)];
                let f = finding(1, &steps);
                job(0, 1, steps, Some(Stop::Finding(f)))
            },
            job(1, 1, vec![], Some(Stop::Finding(finding(2, &[])))),
            job(2, 1, vec![], Some(Stop::Pruned("no candidate".into()))),
            job(3, 1, vec![], Some(Stop::NotTrapped)),
        ],
        &mut eval,
    );
    let at: Vec<(Option<NodeId>, u64)> = d.findings.iter().map(|f| (f.1, f.2)).collect();
    // Tails start at the job parent (the root ends at 100 frames): 25 step frames + 7, or 7.
    assert_eq!(at, [(Some(1), 100 + 25 + 7), (Some(1), 100 + 7)]);
    assert_eq!(
        d.nodes.len(),
        1,
        "the step before the finding is still a node"
    );
    assert_eq!(d.stats.pruned, 1);
    assert_eq!(d.stats.jobs, 4);
    assert_eq!(d.stats.frames, 25 + 7 + 7);
    assert_eq!(d.expanded, [(1, 4)]);

    // A root job that fails before its first step has no node.
    let mut r = Runner::new(cfg(1), vec![], 0);
    let d = r.merge(
        vec![JobResult {
            index: 0,
            parent: None,
            steps: vec![],
            stop: Some(Stop::Finding(finding(3, &[]))),
            probe_frames: 0,
            slow_frames: 0,
            notes: Vec::new(),
        }],
        &mut eval,
    );
    let at: Vec<(Option<NodeId>, u64)> = d.findings.iter().map(|f| (f.1, f.2)).collect();
    assert_eq!(at, [(None, 7)], "from a new game");
    assert!(r.graph.is_empty());
    assert!(r.stalled());
    assert!(r.plan().is_empty());
    // Resumed from that db (no rows, next generation 1), it is still stalled.
    assert!(Runner::new(cfg(1), vec![], 1).stalled());
    assert!(!Runner::new(cfg(1), vec![], 0).stalled());
}

#[test]
fn an_empty_frontier_stalls() {
    let mut root = row(1, None, 0, 0);
    root.status = NodeStatus::Broken;
    let mut r = Runner::new(cfg(1), vec![root], 4);
    assert!(r.stalled());
    assert!(r.plan().is_empty());
    let mut r = Runner::new(cfg(1), vec![row(1, None, 0, 0)], 4);
    assert!(!r.stalled());
    assert_eq!(r.plan().len(), 16);
}

#[test]
fn a_barren_node_is_force_probed_once_per_plan() {
    let mut root = row(1, None, 0, 0);
    root.expanded = 3;
    let mut r = Runner::new(cfg(1), vec![root], 4);
    let planned = r.plan();
    assert_eq!(planned.len(), 16, "the only node is selected every time");
    assert!(
        matches!(planned[0].1, Plan::Softlock { .. }),
        "forced first"
    );
    let probes = planned
        .iter()
        .filter(|(_, p)| matches!(p, Plan::Softlock { .. }))
        .count();
    assert!(probes < 5, "then only the 5% draw ({probes})");
}

#[test]
fn merge_follows_job_index_order() {
    let (mut a, mut ea) = rooted();
    let (mut b, mut eb) = rooted();
    let results = vec![
        job(0, 1, vec![step(50, 8, 60)], None),
        job(1, 1, vec![step(51, 8, 30)], None),
        job(2, 1, vec![step(52, 4, 30)], None),
    ];
    let mut shuffled = results.clone();
    shuffled.reverse();
    let da = a.merge(results, &mut ea);
    let db = b.merge(shuffled, &mut eb);
    assert_eq!(format!("{da:?}"), format!("{db:?}"));
}

#[test]
fn new_rows_carry_the_end_state() {
    let (mut r, mut eval) = rooted();
    let s = step(60, 11, 33);
    let end = s.end.clone();
    let d = r.merge(vec![job(0, 1, vec![s], None)], &mut eval);
    let n = &d.nodes[0];
    assert_eq!(n.digest, state_digest(&end.world));
    assert_eq!(n.end_frame, end.header.frame);
    assert_eq!(n.world_hash, end.header.world_hash);
    assert_eq!(n.build, end.header.build);
    assert_eq!(n.map, end.world.field.map);
    assert_eq!(n.pos, (11, 0));
    assert_eq!(
        (n.frames, n.total_frames, n.depth, n.generation),
        (33, 133, 1, 1)
    );
    assert_eq!(d.touches, [(n.id, Touch::Map(MapId(1)))]);
    assert_eq!(d.seen, [(SeenKind::Map, 11, n.id)]);
    // Depth 1 is not a multiple of 2: cache only.
    assert!(d.snapshots.is_empty());
    let fresh = r.take_fresh();
    assert_eq!(fresh.len(), 2, "root and node 2");
    assert_eq!(fresh[1].0, n.id);
}

fn row(id: NodeId, parent: Option<NodeId>, total: i64, generation: u32) -> NodeRow {
    NodeRow {
        id,
        parent,
        depth: u32::from(parent.is_some()),
        macro_: ROOT_MACRO,
        inputs: vec![],
        frames: 0,
        total_frames: 0,
        end_frame: 0,
        key: AbstractKey(id as u64),
        digest: id as u64,
        world_hash: 0,
        map: MapId(1),
        pos: (0, 0),
        progress: 0,
        fitness: Fitness {
            total,
            ..Fitness::default()
        },
        generation,
        expanded: 0,
        status: NodeStatus::Valid,
        merged_into: None,
        build: Default::default(),
    }
}

#[test]
fn the_age_penalty_makes_an_old_champion_lose_to_a_fresh_node() {
    let cfg = Config {
        tournament: 32,
        age_penalty: 10,
        ..Config::default()
    };
    // Node 1: fitness 300 from generation 0. Node 2: fitness 100, created at generation `n`
    // and picked then: node 1 has aged by n generations (10 each), node 2 not at all.
    let pick = |n| {
        let g = Graph::from_rows(vec![row(1, None, 300, 0), row(2, Some(1), 100, n)]);
        tournament(&g, &mut Rng64(9), n, &cfg)
    };
    assert_eq!(pick(0), Some(1));
    assert_eq!(pick(19), Some(1), "300 - 190 > 100");
    assert_eq!(pick(20), Some(1), "a tie: the lower id");
    assert_eq!(pick(21), Some(2), "the old champion finally loses");
    assert_eq!(pick(40), Some(2));
}

#[test]
fn selection_decays_with_key_hits_and_expansions() {
    let cfg = Config {
        age_penalty: 1,
        expanded_penalty: 4,
        ..Config::default()
    };
    let mut n = row(1, None, 120, 0);
    assert_eq!(selection_score(&n, 0, 0, &cfg), 120);
    assert_eq!(selection_score(&n, 0, 2, &cfg), 40);
    n.expanded = 5;
    assert_eq!(selection_score(&n, 3, 2, &cfg), 40 - 3 - 20);
    let empty = Graph::from_rows(vec![]);
    assert_eq!(tournament(&empty, &mut Rng64(1), 0, &cfg), None);
}

#[test]
fn mutation_ops_follow_their_weights_and_plans() {
    let cfg = Config::default();
    let g = Graph::from_rows(vec![row(1, None, 0, 0), row(2, Some(1), 0, 0), {
        let mut r = row(3, Some(2), 0, 0);
        r.depth = 2;
        r.macro_ = Macro {
            kind: MacroKind::Talk { local_id: 2 },
            opts: Opts::default(),
        };
        r
    }]);
    let mut rng = Rng64(77);
    let mut counts = BTreeMap::new();
    for _ in 0..10_000 {
        let op = match choose(&g, &mut rng, 3) {
            MutationOp::ReplaceSuffix { up } => {
                assert!((1..=2).contains(&up));
                "suffix"
            }
            MutationOp::Extend => "extend",
            MutationOp::FlipOpts => "flip",
            MutationOp::Probe => "probe",
        };
        *counts.entry(op).or_insert(0u32) += 1;
    }
    let near = |op: &str, want: u32| counts[op].abs_diff(want) < want / 8;
    assert!(near("extend", 6000) && near("suffix", 2000), "{counts:?}");
    assert!(near("flip", 1500) && near("probe", 500), "{counts:?}");

    let (start, plan) = apply(&g, &mut rng, 3, MutationOp::Extend, &cfg);
    assert_eq!(start, Some(3));
    assert!(matches!(plan, Plan::Random { len: 1..=3, .. }));
    let (start, _) = apply(&g, &mut rng, 3, MutationOp::ReplaceSuffix { up: 2 }, &cfg);
    assert_eq!(start, Some(1));
    let (start, _) = apply(&g, &mut rng, 2, MutationOp::ReplaceSuffix { up: 2 }, &cfg);
    assert_eq!(start, Some(1), "stops at the root");
    let (start, plan) = apply(&g, &mut rng, 3, MutationOp::FlipOpts, &cfg);
    assert_eq!(start, Some(2));
    let Plan::Given(ms) = plan else {
        panic!("{plan:?}")
    };
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].kind, MacroKind::Talk { local_id: 2 });
    assert_ne!(ms[0].opts, Opts::default());
    let (start, plan) = apply(&g, &mut rng, 1, MutationOp::FlipOpts, &cfg);
    assert_eq!(start, Some(1), "the root extends instead");
    assert!(matches!(plan, Plan::Random { .. }));
    let (start, plan) = apply(&g, &mut rng, 3, MutationOp::Probe, &cfg);
    assert_eq!(start, Some(3));
    assert_eq!(plan, Plan::Softlock { k: 3, breadth: 64 });
}

#[test]
fn a_barren_node_is_probed_with_its_maps_k() {
    let mut cfg = Config::default();
    cfg.detect.softlock_k_per_map.insert(1, 5);
    let mut barren = row(2, Some(1), 0, 0);
    barren.expanded = 3;
    let mut tried = row(3, Some(1), 0, 0);
    tried.expanded = 3;
    let child = row(4, Some(3), 0, 0);
    let g = Graph::from_rows(vec![row(1, None, 0, 0), barren, tried, child]);
    let mut rng = Rng64(1);
    for _ in 0..50 {
        assert_eq!(
            mutate(&g, &mut rng, 2, &cfg),
            (Some(2), Plan::Softlock { k: 5, breadth: 64 })
        );
    }
    let probes = (0..200)
        .filter(|_| matches!(mutate(&g, &mut rng, 3, &cfg).1, Plan::Softlock { .. }))
        .count();
    assert!(probes < 50, "a node with a child is not forced ({probes})");
}
