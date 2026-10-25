//! Milestone 5 Task D: traces (replay on real content) and the checkpoint store (synthetic
//! checkpoints, temp dirs, a fake replayer; nothing here restores a World).

use engine::checkpoint::{
    AnimSnap, AnimTableSnap, BuildHash, Checkpoint, FaderSnap, FieldSnap, Header, IdleSnap,
    InputSnap, ObjectSnap, ObjectsSnap, RunStateSnap, SCHEMA, TransitionSnap, VarsSnap,
    WorldSnapshot,
};
use engine::data::{Behavior, Dir, FlagId, MapId, MovementType};
use engine::{Buttons, World};
use game_native::store::{LOCAL_IDS, Node, Replayed, Status, Store, StoreError, Verdict};
use game_native::trace::{
    CheckpointId, HASH_EVERY, HashMismatch, Parent, ReplayReport, Run, TRACE_SCHEMA, Trace,
    TraceRecorder, replay, replay_with_hashes,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::temp_root;

// ---------------------------------------------------------------------------------------------
// Traces on real content

/// 300 frames of a short walk (a new game fades in, then walks and presses A), recorded on a
/// live session with their hashes.
fn recorded_trace() -> (Trace, Vec<u64>) {
    const WALK: [Buttons; 4] = [Buttons::LEFT, Buttons::DOWN, Buttons::A, Buttons::RIGHT];
    let game = content::Emerald::for_shells();
    let mut world = World::new(&game);
    let mut r = TraceRecorder::new(world.frame());
    let mut hashes = Vec::new();
    for i in 0..300 {
        let keys = WALK[i / 30 % WALK.len()];
        world.step(&game, keys);
        r.push(keys, world.hash());
        hashes.push(world.hash());
    }
    let build = engine::checkpoint::build_hash(&game);
    (r.trace(Parent::NewGame, build), hashes)
}

#[test]
fn replay_reproduces_the_recorders_hashes() {
    let (trace, hashes) = recorded_trace();
    assert_eq!(trace.frames(), 300);
    assert_eq!(
        trace.hashes.iter().map(|h| h.0).collect::<Vec<_>>(),
        [60, 120, 180, 240, 300]
    );
    let game = content::Emerald::for_shells();
    let mut world = World::new(&game);
    let mut seen = Vec::new();
    let (report, observed) = replay_with_hashes(&game, &mut world, &trace, |w, _| {
        seen.push(w.frame());
    });
    assert_eq!(
        report,
        ReplayReport {
            frames: 300,
            end_hash: hashes[299],
            first_mismatch: None
        }
    );
    assert_eq!(
        observed, trace.hashes,
        "re-recorded hashes equal the recorder's"
    );
    assert_eq!(seen, (1..=300).collect::<Vec<_>>(), "each sees every frame");
    assert_eq!(world.hash(), hashes[299]);
}

#[test]
fn a_tampered_hash_is_reported_at_its_frame_and_replay_goes_on() {
    let (mut trace, hashes) = recorded_trace();
    trace.hashes[2].1 ^= 1; // frame 180
    trace.hashes[3].1 ^= 1; // frame 240: only the first mismatch is reported
    let game = content::Emerald::for_shells();
    let mut world = World::new(&game);
    let report = replay(&game, &mut world, &trace, |_, _| {});
    assert_eq!(
        report.first_mismatch,
        Some(HashMismatch {
            frame: 180,
            want: hashes[179] ^ 1,
            got: hashes[179]
        })
    );
    assert_eq!(report.frames, 300);
    assert_eq!(report.end_hash, hashes[299], "replay never stops early");
}

// ---------------------------------------------------------------------------------------------
// Store fixtures

const BUILD: BuildHash = BuildHash {
    engine: 0x1111,
    content: 0x2222,
};
const OTHER_BUILD: BuildHash = BuildHash {
    engine: 0x3333,
    content: 0x2222,
};

fn object(local_id: u8, pos: (i16, i16)) -> ObjectSnap {
    ObjectSnap {
        local_id,
        map: MapId(9),
        gfx: None,
        pos,
        prev: pos,
        elevation: 3,
        facing: Dir::Down,
        movement: MovementType::Face(Dir::Down),
        home: pos,
        range: (0, 0),
        visible: true,
        frozen: false,
        facing_locked: false,
        action: None,
        idle: IdleSnap::Done,
        anim: AnimSnap {
            table: AnimTableSnap::Face,
            cmd: 0,
            timer: 0,
        },
    }
}

/// A synthetic capturable state: `state` picks the player's x and the set flags.
fn checkpoint(frame: u32, state: u32, build: BuildHash) -> Checkpoint {
    let world = WorldSnapshot {
        frame,
        rng: 0x1234_5678,
        input: InputSnap {
            held: Buttons::NONE,
            pressed: Buttons::NONE,
        },
        runner_issued: 4,
        player_name: [0xBB, 0xCC, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        player_id: 77,
        field: FieldSnap {
            map: MapId(9),
            camera: (16, 32),
            drawn: Some([0, 0, 15, 10]),
            ring: (0, 0),
            overrides: vec![((3, 4), 0x201)],
            stale: None,
        },
        objects: ObjectsSnap {
            map: MapId(9),
            run_state: RunStateSnap::NotMoving,
            objs: vec![object(0xFF, (state as i16, 5)), object(1, (2, 2))],
        },
        templates: vec![],
        vars: VarsSnap {
            flags: (0..state % 7).map(|f| FlagId(0x50 + f as u16)).collect(),
            vars: vec![],
        },
        fader: FaderSnap {
            level: 0,
            target: 0,
        },
        transition: TransitionSnap {
            map: MapId(9),
            x: 0,
            y: 0,
            facing: Dir::Down,
            exit_walk: false,
            player_hidden: false,
        },
        selected: None,
        last_mover: 0,
        delay: 0,
        door: 0,
        party: vec![],
        respawn: 0,
        prev_behavior: Behavior::Normal,
        encounter_immunity: 0,
        bag: Default::default(),
        money: 0,
        str_vars: Default::default(),
        warp_slots: Default::default(),
        party_held: Vec::new(),
        options: None,
        pokedex: Default::default(),
        pc: Default::default(),
        coins: 0,
        registered_item: None,
        berry_trees: Vec::new(),
        map_layout: None,
        sky: None,
        rival_name: None,
        box_names: Vec::new(),
        audio: None,
    };
    Checkpoint {
        header: Header {
            schema: SCHEMA,
            build,
            frame,
            world_hash: 0xABCD_0000 ^ state as u64 ^ build.engine << 32,
        },
        world,
    }
}

fn trace(parent: Parent, start_frame: u32, keys: &[(Buttons, u32)]) -> Trace {
    let mut r = TraceRecorder::new(start_frame);
    for &(k, n) in keys {
        for _ in 0..n {
            r.push(k, k.0 as u64);
        }
    }
    r.trace(parent, BUILD)
}

/// The fake game: a state is the sum of the parent state and every held key bit, plus `salt`.
fn fake_state(parent: Option<&Checkpoint>, trace: &Trace, salt: u32) -> u32 {
    let base = parent.map_or(1, |p| p.world.objects.objs[0].pos.0 as u32);
    base + trace.keys().map(|k| k.0 as u32).sum::<u32>() + salt
}

/// Adds a node whose checkpoint is what the fake game reaches.
fn add(store: &Store, label: &str, parent: Parent, keys: &[(Buttons, u32)]) -> CheckpointId {
    let parent_cp = match parent {
        Parent::NewGame => None,
        Parent::Checkpoint(id) => Some(store.snapshot(id).unwrap().unwrap()),
    };
    let start = parent_cp.as_ref().map_or(0, |p| p.header.frame);
    let t = trace(parent, start, keys);
    let cp = checkpoint(
        start + t.frames(),
        fake_state(parent_cp.as_ref(), &t, 0),
        BUILD,
    );
    store.add(label, t, &cp).unwrap()
}

/// The periodic hashes the fake game observes on `build`: on `BUILD`, what [`trace`] records
/// (the keys of the frame), changed by another build or a salt.
fn fake_hashes(trace: &Trace, build: BuildHash, salt: u32) -> Vec<(u32, u64)> {
    let keys: Vec<Buttons> = trace.keys().collect();
    let n = trace.frames();
    (1..=n)
        .filter(|&i| i.is_multiple_of(HASH_EVERY) || i == n)
        .map(|i| {
            let h = keys[i as usize - 1].0 as u64 ^ (build.engine ^ BUILD.engine) << 16;
            (i, h ^ u64::from(salt))
        })
        .collect()
}

/// A fake replayer for `build`, adding 1 to the state of the trace ending at frame `salt_at`.
fn replayer(
    build: BuildHash,
    salt_at: Option<u32>,
    hint: Option<HashMismatch>,
) -> impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> {
    par_replayer(build, salt_at, hint)
}

/// [`replayer`] for `Store::verify_nodes_par`.
fn par_replayer(
    build: BuildHash,
    salt_at: Option<u32>,
    hint: Option<HashMismatch>,
) -> impl Fn(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> + Sync {
    move |parent, trace| {
        let salt = u32::from(salt_at == Some(trace.start_frame + trace.frames()));
        let start = parent.map_or(0, |p| p.header.frame);
        assert_eq!(
            start, trace.start_frame,
            "the parent state comes from this run"
        );
        let cp = checkpoint(
            start + trace.frames(),
            fake_state(parent, trace, salt),
            build,
        );
        Ok(Replayed {
            report: ReplayReport {
                frames: trace.frames(),
                end_hash: cp.header.world_hash,
                first_mismatch: if salt == 1 { hint } else { None },
            },
            hashes: fake_hashes(trace, build, salt),
            checkpoint: cp,
        })
    }
}

/// The variant of a store error, for assertions.
fn kind(e: StoreError) -> &'static str {
    match e {
        StoreError::NotFound(_) => "NotFound",
        StoreError::Label(_) => "Label",
        StoreError::Parent { .. } => "Parent",
        StoreError::Cycle(_) => "Cycle",
        StoreError::HasChildren(_) => "HasChildren",
        StoreError::ReadOnly(_) => "ReadOnly",
        StoreError::Parse { .. } => "Parse",
        StoreError::Invalid(_) => "Invalid",
        StoreError::Io(_) => "Io",
    }
}

fn read_status(root: &Path) -> BTreeMap<CheckpointId, Status> {
    ron::from_str(&fs::read_to_string(root.join("status.ron")).unwrap()).unwrap()
}

/// root ─ a (frames 0..10) ─ b (10..30) ─ c (30..35)
///                          └ d (10..12)
fn tree(name: &str) -> (PathBuf, Store, [CheckpointId; 4]) {
    let root = temp_root(name);
    let store = Store::open(&root).unwrap();
    let a = add(&store, "a", Parent::NewGame, &[(Buttons::NONE, 10)]);
    let b = add(
        &store,
        "b",
        Parent::Checkpoint(a),
        &[(Buttons::DOWN, 5), (Buttons::A, 15)],
    );
    let c = add(&store, "c", Parent::Checkpoint(b), &[(Buttons::LEFT, 5)]);
    let d = add(&store, "d", Parent::Checkpoint(a), &[(Buttons::UP, 2)]);
    (root, store, [a, b, c, d])
}

// ---------------------------------------------------------------------------------------------
// Store

#[test]
fn add_nodes_find_and_layout() {
    let (root, store, [a, b, c, d]) = tree("add");
    assert_eq!(
        [a, b, c, d],
        [1, 2, 3, 4].map(CheckpointId),
        "ids are max + 1"
    );
    let nodes = store.nodes().unwrap();
    assert_eq!(
        nodes.iter().map(|n| n.meta.id).collect::<Vec<_>>(),
        [a, b, c, d]
    );
    let nb = &nodes[1];
    assert_eq!(nb.meta.label, "b");
    assert_eq!(nb.meta.frame, 30);
    assert_eq!(nb.meta.map, MapId(9));
    assert_eq!(nb.meta.schema, SCHEMA);
    assert_eq!(nb.meta.build, BUILD);
    assert_eq!(nb.trace.parent, Parent::Checkpoint(a));
    assert_eq!(nb.trace.start_frame, 10);
    assert_eq!(nb.trace.schema, TRACE_SCHEMA);
    assert_eq!(
        nb.trace.inputs,
        [
            Run {
                keys: Buttons::DOWN,
                frames: 5
            },
            Run {
                keys: Buttons::A,
                frames: 15
            }
        ]
    );
    for q in ["c00002", "2", "b"] {
        assert_eq!(&store.find(q).unwrap(), nb, "find {q}");
    }
    for q in ["c00009", "9", "nope"] {
        assert_eq!(kind(store.find(q).unwrap_err()), "NotFound", "{q}");
    }
    assert!(root.join("nodes/c00001.ron").is_file());
    assert!(root.join("snapshots/c00004.ron").is_file());
    let leftovers: Vec<_> = ["nodes", "snapshots"]
        .iter()
        .flat_map(|d| fs::read_dir(root.join(d)).unwrap())
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| !n.ends_with(".ron"))
        .collect();
    assert!(leftovers.is_empty(), "no temp files: {leftovers:?}");
    let text = fs::read_to_string(root.join("nodes/c00002.ron")).unwrap();
    assert!(!text.contains(root.to_str().unwrap()), "no absolute paths");
    println!("nodes/c00002.ron:\n{text}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn add_checks_labels_parents_and_frames() {
    let (root, store, [a, ..]) = tree("add-checks");
    let cp = checkpoint(12, 3, BUILD);
    let t = |parent, start| trace(parent, start, &[(Buttons::NONE, 2)]);
    let err = |r: Result<CheckpointId, StoreError>| kind(r.unwrap_err());
    let e = err(store.add("a", t(Parent::Checkpoint(a), 10), &cp));
    assert_eq!(e, "Label", "labels are unique");
    for label in ["", "7", "c00007"] {
        let e = err(store.add(label, t(Parent::Checkpoint(a), 10), &cp));
        assert_eq!(e, "Label", "label {label:?}");
    }
    let e = err(store.add("x", t(Parent::Checkpoint(CheckpointId(42)), 10), &cp));
    assert_eq!(e, "Invalid", "missing parent");
    let e = err(store.add("x", t(Parent::Checkpoint(a), 9), &cp));
    assert_eq!(e, "Invalid", "start frame != parent frame");
    let e = err(store.add("x", t(Parent::Checkpoint(a), 10), &checkpoint(13, 3, BUILD)));
    assert_eq!(e, "Invalid", "end frame != checkpoint frame");
    let e = err(store.add("x", t(Parent::NewGame, 5), &checkpoint(7, 3, BUILD)));
    assert_eq!(e, "Invalid", "a new game starts at 0");
    assert_eq!(store.nodes().unwrap().len(), 4);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn relabel_and_remove() {
    let (root, store, [a, b, c, d]) = tree("relabel");
    store.relabel(b, "bee").unwrap();
    assert_eq!(store.find("bee").unwrap().meta.id, b);
    assert!(store.find("b").is_err());
    store.relabel(b, "bee").unwrap();
    assert_eq!(kind(store.relabel(c, "bee").unwrap_err()), "Label");
    assert!(store.relabel(CheckpointId(9), "z").is_err());

    assert_eq!(kind(store.remove(a).unwrap_err()), "HasChildren");
    assert_eq!(kind(store.remove(b).unwrap_err()), "HasChildren");
    store.remove(c).unwrap();
    assert!(!root.join("snapshots/c00003.ron").exists());
    store.remove(b).unwrap();
    store.remove(d).unwrap();
    store.remove(a).unwrap();
    assert!(store.nodes().unwrap().is_empty());
    assert_eq!(kind(store.remove(a).unwrap_err()), "NotFound");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_empty_or_missing_store_has_no_nodes() {
    let root = temp_root("empty");
    let store = Store::open(&root).unwrap();
    assert!(store.nodes().unwrap().is_empty());
    assert!(store.topo_order().unwrap().is_empty());
    assert_eq!(store.snapshot(CheckpointId(1)).unwrap(), None);
    assert!(
        store
            .verify(replayer(BUILD, None, None), false)
            .unwrap()
            .is_empty()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn snapshots_of_another_schema_read_as_none() {
    let (root, store, [a, ..]) = tree("schema");
    assert_eq!(store.snapshot(a).unwrap().unwrap().header.frame, 10);
    let mut cp = checkpoint(10, 1, BUILD);
    cp.header.schema = SCHEMA + 1;
    store.put_snapshot(a, &cp).unwrap();
    assert_eq!(store.snapshot(a).unwrap(), None);
    assert_eq!(store.snapshot(CheckpointId(77)).unwrap(), None, "absent");
    fs::remove_dir_all(root).unwrap();
}

/// Writes a node file by hand (what a merge or a hand edit can leave).
fn write_node(root: &Path, id: u32, parent: Parent) {
    let store = Store::open(root).unwrap();
    let mut n: Node = store.nodes().unwrap().remove(0);
    n.meta.id = CheckpointId(id);
    n.meta.label = format!("hand{id}");
    n.trace.parent = parent;
    fs::write(
        root.join(format!("nodes/c{id:05}.ron")),
        ron::to_string(&n).unwrap(),
    )
    .unwrap();
}

#[test]
fn topo_order_puts_parents_first_and_rejects_missing_parents_and_cycles() {
    let (root, store, [a, b, c, d]) = tree("topo");
    // A child with a smaller id than its parent (hand-edited) still comes after it.
    write_node(&root, 0, Parent::Checkpoint(d));
    let order: Vec<_> = store
        .topo_order()
        .unwrap()
        .iter()
        .map(|n| n.meta.id)
        .collect();
    assert_eq!(order, [a, b, c, d, CheckpointId(0)]);

    write_node(&root, 0, Parent::Checkpoint(CheckpointId(99)));
    let e = store.topo_order().unwrap_err();
    assert!(e.to_string().contains("c00099"), "{e}");

    write_node(&root, 0, Parent::Checkpoint(CheckpointId(8)));
    write_node(&root, 8, Parent::Checkpoint(CheckpointId(0)));
    let e = store.topo_order().unwrap_err();
    assert!(e.to_string().contains("cycle"), "{e}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_files_are_byte_identical_when_written_twice() {
    let (r1, s1, ids) = tree("det1");
    let (r2, _, _) = tree("det2");
    for id in ids {
        let f = format!("nodes/{id}.ron");
        let bytes = fs::read(r1.join(&f)).unwrap();
        assert_eq!(bytes, fs::read(r2.join(&f)).unwrap(), "{f}");
        let snap = format!("snapshots/{id}.ron");
        assert_eq!(
            fs::read(r1.join(&snap)).unwrap(),
            fs::read(r2.join(&snap)).unwrap()
        );
        // Rewriting a node unchanged leaves its bytes unchanged.
        let label = s1.nodes().unwrap()[id.0 as usize - 1].meta.label.clone();
        s1.relabel(id, &label).unwrap();
        assert_eq!(bytes, fs::read(r1.join(&f)).unwrap());
    }
    fs::remove_dir_all(r1).unwrap();
    fs::remove_dir_all(r2).unwrap();
}

// ---------------------------------------------------------------------------------------------
// verify

#[test]
fn verify_valid_tree() {
    let (root, store, ids) = tree("valid");
    let before: Vec<_> = ids
        .iter()
        .map(|id| fs::read(root.join(format!("nodes/{id}.ron"))).unwrap())
        .collect();
    let out = store.verify(replayer(BUILD, None, None), false).unwrap();
    assert_eq!(out, ids.map(|id| (id, Status::Valid)));
    // Same verdict on another build: equality is semantic (the digest).
    let out = store
        .verify(replayer(OTHER_BUILD, None, None), false)
        .unwrap();
    assert_eq!(out, ids.map(|id| (id, Status::Valid)));
    let after: Vec<_> = ids
        .iter()
        .map(|id| fs::read(root.join(format!("nodes/{id}.ron"))).unwrap())
        .collect();
    assert_eq!(before, after, "verify never rewrites a valid node");
    assert_eq!(
        read_status(&root),
        ids.iter().map(|&id| (id, Status::Valid)).collect()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_diverged_with_and_without_a_hash_hint() {
    let (root, store, [a, b, c, d]) = tree("diverged");
    let hint = HashMismatch {
        frame: 60,
        want: 1,
        got: 2,
    };
    // b (ending at frame 30) reaches another state on another build: b and its child diverge.
    let out = store
        .verify(replayer(OTHER_BUILD, Some(30), Some(hint)), false)
        .unwrap();
    assert_eq!(
        out,
        [
            (a, Status::Valid),
            (
                b,
                Status::Diverged {
                    hash: Some(hint),
                    parent_diverged: false
                }
            ),
            (
                c,
                Status::Diverged {
                    hash: None,
                    parent_diverged: true
                }
            ),
            (d, Status::Valid),
        ]
    );
    // Without a hint (the recorded hashes agree, only the end state differs).
    let out = store
        .verify(replayer(OTHER_BUILD, Some(30), None), false)
        .unwrap();
    assert_eq!(
        out[1],
        (
            b,
            Status::Diverged {
                hash: None,
                parent_diverged: false
            }
        )
    );
    assert_eq!(read_status(&root)[&c], out[2].1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_broken_nondeterminism_replayer_error_and_parent_broken() {
    let (root, store, [a, b, c, d]) = tree("broken");
    let files = |ids: [CheckpointId; 2]| {
        ids.map(|id| {
            (
                fs::read(root.join(format!("nodes/{id}.ron"))).unwrap(),
                store.snapshot(id).unwrap(),
            )
        })
    };
    let before = files([b, c]);
    // Same build, different state: nondeterminism.
    let out = store.verify(replayer(BUILD, Some(30), None), true).unwrap();
    let nondet = Status::Broken("nondeterministic: same build, different state".into());
    let parent = Status::Broken("parent broken".into());
    assert_eq!(
        out,
        [
            (a, Status::Valid),
            (b, nondet.clone()),
            (c, parent.clone()),
            (d, Status::Valid)
        ]
    );
    // The replayer fails on a: everything below it is broken, and nothing more is replayed.
    let mut calls = 0;
    let out = store
        .verify(
            |_: Option<&Checkpoint>, _: &Trace| {
                calls += 1;
                Err::<Replayed, _>("trace ends while Script".to_string())
            },
            true,
        )
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        out,
        [
            (a, Status::Broken("trace ends while Script".into())),
            (b, parent.clone()),
            (c, parent.clone()),
            (d, parent)
        ]
    );
    // accept never touches Broken nodes (b, c were Broken in both runs).
    assert_eq!(files([b, c]), before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_stale_schema() {
    let (root, store, [a, b, c, d]) = tree("stale");
    let mut n = store.find("b").unwrap();
    n.meta.schema = SCHEMA + 1;
    fs::write(root.join("nodes/c00002.ron"), ron::to_string(&n).unwrap()).unwrap();
    let out = store.verify(replayer(BUILD, None, None), false).unwrap();
    assert_eq!(
        out,
        [
            (a, Status::Valid),
            (b, Status::Stale { schema: SCHEMA + 1 }),
            (c, Status::Valid),
            (d, Status::Valid)
        ]
    );
    store.verify(replayer(BUILD, None, None), true).unwrap();
    assert_eq!(store.find("b").unwrap().meta.schema, SCHEMA);
    let out = store.verify(replayer(BUILD, None, None), false).unwrap();
    assert!(out.iter().all(|(_, s)| *s == Status::Valid), "{out:?}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_accept_rebaselines_diverged_nodes() {
    let (root, store, [a, b, c, d]) = tree("accept");
    let a_file = fs::read(root.join("nodes/c00001.ron")).unwrap();
    let mut run = replayer(OTHER_BUILD, Some(30), None);
    let out = store.verify(&mut run, true).unwrap();
    assert!(matches!(out[1].1, Status::Diverged { .. }), "{out:?}");
    assert!(matches!(out[2].1, Status::Diverged { .. }), "{out:?}");
    assert_eq!(
        (out[0].clone(), out[3].clone()),
        ((a, Status::Valid), (d, Status::Valid))
    );
    // status.ron describes the tree after the run: accepted nodes are valid.
    assert!(read_status(&root).values().all(|s| *s == Status::Valid));
    // b and c carry the new build's meta, re-recorded hashes and snapshots.
    for (id, label) in [(b, "b"), (c, "c")] {
        let n = store.find(label).unwrap();
        let replayed = {
            let parent = match n.trace.parent {
                Parent::NewGame => None,
                Parent::Checkpoint(p) => store.snapshot(p).unwrap(),
            };
            run(parent.as_ref(), &n.trace).unwrap()
        };
        let cp = replayed.checkpoint;
        assert_eq!(n.meta.id, id);
        assert_eq!(n.meta.label, label, "labels survive");
        assert_eq!(n.meta.build, OTHER_BUILD);
        assert_eq!(n.meta.world_hash, cp.header.world_hash);
        assert_eq!(n.meta.frame, cp.header.frame);
        assert_eq!(n.trace.build, OTHER_BUILD);
        assert_eq!(n.trace.hashes, replayed.hashes);
        assert_eq!(store.snapshot(id).unwrap(), Some(cp));
    }
    // The valid nodes keep their state and label; their build and hashes are refreshed.
    assert_ne!(a_file, fs::read(root.join("nodes/c00001.ron")).unwrap());
    let na = store.find("a").unwrap();
    assert_eq!((na.meta.build, na.trace.build), (OTHER_BUILD, OTHER_BUILD));
    // Re-verifying on the new build is now valid everywhere.
    let out = store
        .verify(replayer(OTHER_BUILD, Some(30), None), false)
        .unwrap();
    assert!(out.iter().all(|(_, s)| *s == Status::Valid), "{out:?}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn valid_nodes_of_another_build_are_flagged_and_accept_refreshes_them() {
    let (root, store, [a, b, c, d]) = tree("rehash");
    let files = |root: &Path| -> Vec<Vec<u8>> {
        [a, b, c, d]
            .iter()
            .map(|id| fs::read(root.join(format!("nodes/{id}.ron"))).unwrap())
            .collect()
    };
    let before = files(&root);
    let rehash = |out: &[Verdict]| -> Vec<(CheckpointId, Status, bool)> {
        out.iter()
            .map(|v| (v.id, v.status.clone(), v.rehash))
            .collect()
    };
    // On the recording build nothing is stale.
    let out = store
        .verify_nodes(replayer(BUILD, None, None), false)
        .unwrap();
    assert!(out.iter().all(|v| v.status == Status::Valid && !v.rehash));
    // Another build reaches the same states: Valid, flagged, and left as is without --accept.
    let out = store
        .verify_nodes(replayer(OTHER_BUILD, None, None), false)
        .unwrap();
    assert_eq!(
        rehash(&out),
        [a, b, c, d].map(|id| (id, Status::Valid, true))
    );
    assert_eq!(files(&root), before);
    // --accept refreshes build, world hash and trace hashes; the state and label stay.
    let out = store
        .verify_nodes(replayer(OTHER_BUILD, None, None), true)
        .unwrap();
    assert_eq!(
        rehash(&out),
        [a, b, c, d].map(|id| (id, Status::Valid, true))
    );
    for (old, id) in before.iter().zip([a, b, c, d]) {
        let old: Node = ron::from_str(std::str::from_utf8(old).unwrap()).unwrap();
        let n = store.find(&id.to_string()).unwrap();
        assert_eq!((n.meta.build, n.trace.build), (OTHER_BUILD, OTHER_BUILD));
        assert_eq!(n.trace.hashes, fake_hashes(&n.trace, OTHER_BUILD, 0));
        let cp = store.snapshot(id).unwrap().unwrap();
        assert_eq!(n.meta.world_hash, cp.header.world_hash);
        assert_ne!(n.meta.world_hash, old.meta.world_hash);
        assert_eq!(
            (&n.meta.label, n.meta.digest, &n.trace.inputs),
            (&old.meta.label, old.meta.digest, &old.trace.inputs)
        );
    }
    let out = store
        .verify_nodes(replayer(OTHER_BUILD, None, None), false)
        .unwrap();
    assert!(out.iter().all(|v| v.status == Status::Valid && !v.rehash));
    // Refreshed, the nodes are this build's: another state is nondeterminism, not divergence.
    let nondet = Status::Broken("nondeterministic: same build, different state".into());
    let out = store
        .verify(replayer(OTHER_BUILD, Some(30), None), false)
        .unwrap();
    assert_eq!(out[1], (b, nondet));
    // So are other hashes reaching the same state on the node's own build.
    let mut n = store.find("a").unwrap();
    n.trace.hashes[0].1 ^= 1;
    fs::write(root.join("nodes/c00001.ron"), ron::to_string(&n).unwrap()).unwrap();
    let out = store
        .verify(replayer(OTHER_BUILD, None, None), true)
        .unwrap();
    let parent = Status::Broken("parent broken".into());
    assert_eq!(
        out,
        [
            (
                a,
                Status::Broken("nondeterministic: same build, different hashes".into())
            ),
            (b, parent.clone()),
            (c, parent.clone()),
            (d, parent),
        ]
    );
    assert_eq!(
        store.find("a").unwrap(),
        n,
        "a Broken node is never refreshed"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_layered_store_reads_the_base_and_writes_only_locally() {
    let (base_root, base, [a, b, c, d]) = tree("base");
    let base_files = |root: &Path| -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = ["nodes", "snapshots"]
            .iter()
            .flat_map(|d| fs::read_dir(root.join(d)).unwrap())
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().into_string().unwrap(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    };
    let before = base_files(&base_root);
    let local_root = temp_root("local");
    let store = Store::layered(&base_root, &local_root).unwrap();
    // The base's nodes are listed and found.
    assert_eq!(store.nodes().unwrap(), base.nodes().unwrap());
    assert_eq!(store.find("b").unwrap().meta.id, b);
    // A local save from a base node gets a local id, in the local root only.
    let mine = add(&store, "mine", Parent::Checkpoint(c), &[(Buttons::UP, 3)]);
    assert_eq!(mine, CheckpointId(LOCAL_IDS + 1));
    let next = add(&store, "next", Parent::Checkpoint(mine), &[(Buttons::A, 1)]);
    assert_eq!(next, CheckpointId(LOCAL_IDS + 2));
    assert!(local_root.join(format!("nodes/{mine}.ron")).is_file());
    let order: Vec<_> = store
        .topo_order()
        .unwrap()
        .iter()
        .map(|n| n.meta.id)
        .collect();
    assert_eq!(order, [a, b, c, d, mine, next]);
    // The base alone never sees the local nodes.
    assert_eq!(base.nodes().unwrap().len(), 4);
    // Base nodes are read-only; local ones are not.
    assert_eq!(kind(store.relabel(a, "aa").unwrap_err()), "ReadOnly");
    assert_eq!(kind(store.remove(d).unwrap_err()), "ReadOnly");
    assert!(store.remove(c).is_err(), "c has a local child");
    store.relabel(mine, "mine2").unwrap();
    store.remove(next).unwrap();
    // A verify that would rewrite a base node fails; a plain one is fine.
    store.verify(replayer(BUILD, None, None), false).unwrap();
    assert!(
        store
            .verify(replayer(OTHER_BUILD, None, None), true)
            .is_err()
    );
    assert_eq!(base_files(&base_root), before, "the base is never written");
    // A local id equal to a base one is an error.
    fs::copy(
        base_root.join("nodes/c00001.ron"),
        local_root.join("nodes/c00001.ron"),
    )
    .unwrap();
    assert!(store.nodes().is_err());
    fs::remove_dir_all(base_root).unwrap();
    fs::remove_dir_all(local_root).unwrap();
}

#[test]
fn local_root_is_below_the_committed_tree_unless_overridden() {
    if std::env::var_os("PKR_CHECKPOINTS").is_some() {
        return;
    }
    for flavor in content::Flavor::ALL {
        assert_eq!(
            Store::local_root(flavor),
            Store::default_root(flavor).join("local")
        );
    }
}

#[test]
fn default_root_is_per_flavor() {
    use content::Flavor;
    for (flavor, dir) in [
        (Flavor::Stub, "stub"),
        (Flavor::Emerald, "emerald"),
        (Flavor::Silice, "silice"),
    ] {
        let r = Store::default_root(flavor);
        assert!(r.ends_with(Path::new("checkpoints").join(dir)), "{r:?}");
        assert!(
            r.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Cargo.toml")
                .is_file()
        );
        // Every flavour's tree is committed (`checkpoints/silice/` holds no node yet).
        assert!(r.is_dir(), "{r:?}");
        Store::open(&r).unwrap();
    }
}

#[test]
fn a_child_of_a_diverged_parent_on_its_own_build_is_diverged_not_broken() {
    let (root, store, [a, b, c, d]) = tree("parent-diverged");
    // b was recorded on another build; c (its child) on this one.
    let mut n = store.find("b").unwrap();
    n.meta.build = OTHER_BUILD;
    fs::write(root.join("nodes/c00002.ron"), ron::to_string(&n).unwrap()).unwrap();
    let out = store
        .verify(replayer(BUILD, Some(30), None), false)
        .unwrap();
    assert_eq!(
        out,
        [
            (a, Status::Valid),
            (
                b,
                Status::Diverged {
                    hash: None,
                    parent_diverged: false
                }
            ),
            (
                c,
                Status::Diverged {
                    hash: None,
                    parent_diverged: true
                }
            ),
            (d, Status::Valid),
        ]
    );
    store.verify(replayer(BUILD, Some(30), None), true).unwrap();
    let out = store
        .verify(replayer(BUILD, Some(30), None), false)
        .unwrap();
    assert!(out.iter().all(|(_, s)| *s == Status::Valid), "{out:?}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_trace_of_another_schema_is_broken_and_add_rejects_other_schemas() {
    let (root, store, [a, b, c, d]) = tree("trace-schema");
    let mut n = store.find("b").unwrap();
    n.trace.schema = TRACE_SCHEMA + 1;
    fs::write(root.join("nodes/c00002.ron"), ron::to_string(&n).unwrap()).unwrap();
    let out = store.verify(replayer(BUILD, None, None), true).unwrap();
    let parent = Status::Broken("parent broken".into());
    assert_eq!(
        out,
        [
            (a, Status::Valid),
            (
                b,
                Status::Broken(format!("trace schema {}", TRACE_SCHEMA + 1))
            ),
            (c, parent),
            (d, Status::Valid),
        ]
    );
    let t = trace(Parent::Checkpoint(a), 10, &[(Buttons::NONE, 2)]);
    let mut wrong = t.clone();
    wrong.schema = TRACE_SCHEMA + 1;
    let e = store
        .add("x", wrong, &checkpoint(12, 3, BUILD))
        .unwrap_err();
    assert_eq!(kind(e), "Invalid");
    let mut cp = checkpoint(12, 3, BUILD);
    cp.header.schema = SCHEMA + 1;
    let e = store.add("x", t, &cp).unwrap_err();
    assert_eq!(kind(e), "Invalid");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn badly_named_node_files_are_errors() {
    let (root, store, _) = tree("names");
    fs::copy(root.join("nodes/c00001.ron"), root.join("nodes/c1.ron")).unwrap();
    let e = store.nodes().unwrap_err();
    assert!(e.to_string().contains("not a node file name"), "{e}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_snapshots_read_as_none() {
    let (root, store, [a, ..]) = tree("corrupt");
    fs::write(
        root.join("snapshots/c00001.ron"),
        "(header: (schema: 1, buil",
    )
    .unwrap();
    assert_eq!(store.snapshot(a).unwrap(), None);
    fs::write(root.join("snapshots/c00001.ron"), "garbage").unwrap();
    assert_eq!(store.snapshot(a).unwrap(), None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remove_forgets_the_status_of_the_id() {
    let (root, store, [_, _, c, d]) = tree("rm-status");
    store.verify(replayer(BUILD, None, None), false).unwrap();
    store.remove(d).unwrap();
    let status = read_status(&root);
    assert!(!status.contains_key(&d) && status.contains_key(&c));
    assert_eq!(store.statuses().unwrap(), status);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_index_has_children_chains_and_order() {
    let (root, store, [a, b, c, d]) = tree("index");
    let index = store.index().unwrap();
    assert_eq!(index.len(), 4);
    assert_eq!(index.children(Parent::NewGame), [a]);
    assert_eq!(index.children(Parent::Checkpoint(a)), [b, d]);
    assert_eq!(index.children(Parent::Checkpoint(b)), [c]);
    assert!(index.children(Parent::Checkpoint(c)).is_empty());
    assert_eq!(index.topo(), [a, b, c, d]);
    assert_eq!(index.chain(c).unwrap(), [a, b, c]);
    assert_eq!(index.chain(a).unwrap(), [a]);
    assert_eq!(kind(index.chain(CheckpointId(9)).unwrap_err()), "NotFound");
    assert_eq!(index.node(b).unwrap().meta.label, "b");
    assert_eq!(index.labelled("d").unwrap().meta.id, d);
    assert!(!index.is_base(a));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_parent_and_a_cycle_are_typed_errors() {
    let (root, store, _) = tree("index-errors");
    write_node(&root, 7, Parent::Checkpoint(CheckpointId(99)));
    match store.index().unwrap_err() {
        StoreError::Parent { id, parent } => {
            assert_eq!((id, parent), (CheckpointId(7), CheckpointId(99)))
        }
        e => panic!("{e}"),
    }
    // A broken tree's nodes are still found by label.
    assert_eq!(store.find("hand7").unwrap().meta.id, CheckpointId(7));
    let parent = Parent::Checkpoint(CheckpointId(99));
    let e = store.add(
        "x",
        trace(parent, 10, &[(Buttons::NONE, 2)]),
        &checkpoint(12, 3, BUILD),
    );
    assert_eq!(e.unwrap_err().to_string(), "parent c00099 does not exist");
    // 7 -> 8 -> 7, and 9 below the cycle.
    write_node(&root, 7, Parent::Checkpoint(CheckpointId(8)));
    write_node(&root, 8, Parent::Checkpoint(CheckpointId(7)));
    write_node(&root, 9, Parent::Checkpoint(CheckpointId(8)));
    match store.index().unwrap_err() {
        StoreError::Cycle(ids) => assert_eq!(ids, [CheckpointId(7), CheckpointId(8)]),
        e => panic!("{e}"),
    }
    // A broken tree can still be listed and repaired.
    assert_eq!(store.nodes().unwrap().len(), 7);
    store.remove(CheckpointId(9)).unwrap();
    assert_eq!(
        kind(store.remove(CheckpointId(8)).unwrap_err()),
        "HasChildren"
    );
    write_node(&root, 7, Parent::NewGame);
    store.remove(CheckpointId(8)).unwrap();
    store.remove(CheckpointId(7)).unwrap();
    assert_eq!(store.index().unwrap().len(), 4);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_unparseable_node_is_a_parse_error_naming_its_file() {
    let (root, store, [_, b, ..]) = tree("parse");
    let file = root.join(format!("nodes/{b}.ron"));
    fs::write(&file, "(meta: (id: 2, lab").unwrap();
    for e in [
        store.index().unwrap_err(),
        store.nodes().unwrap_err(),
        store.find("b00").unwrap_err(),
        store.find("2").unwrap_err(),
    ] {
        match e {
            StoreError::Parse { file: f, .. } => assert_eq!(f, file),
            e => panic!("{e}"),
        }
    }
    // A node file holding another node's id.
    let a = fs::read(root.join("nodes/c00001.ron")).unwrap();
    fs::write(&file, a).unwrap();
    assert_eq!(kind(store.find("2").unwrap_err()), "Parse");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn merge_statuses_keeps_other_entries() {
    let (root, store, [a, b, c, d]) = tree("merge");
    assert!(store.statuses().unwrap().is_empty(), "absent is empty");
    store.merge_statuses(&[(a, Status::Valid)]).unwrap();
    let stale = Status::Stale { schema: 1 };
    store.merge_statuses(&[(b, stale.clone())]).unwrap();
    assert_eq!(
        store.statuses().unwrap(),
        BTreeMap::from([(a, Status::Valid), (b, stale.clone())])
    );
    // A verdict for an absent node (removed during a background verify) records nothing.
    store
        .merge_statuses(&[(CheckpointId(77), stale.clone())])
        .unwrap();
    assert!(!read_status(&root).contains_key(&CheckpointId(77)));
    store.verify(replayer(BUILD, None, None), false).unwrap();
    let want: BTreeMap<_, _> = [a, b, c, d].map(|id| (id, Status::Valid)).into();
    assert_eq!(store.statuses().unwrap(), want);
    // An unreadable status.ron (not RON, or not UTF-8) is an error to read and replaced by a
    // merge.
    for bytes in [&b"garbage"[..], &[0xff, 0xfe, 0x00][..]] {
        fs::write(root.join("status.ron"), bytes).unwrap();
        assert!(store.statuses().unwrap_err().is_corrupt());
        store.merge_statuses(&[(a, Status::Valid)]).unwrap();
        assert_eq!(store.statuses().unwrap(), [(a, Status::Valid)].into());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_reused_id_never_inherits_a_verdict() {
    let (root, store, [a, .., d]) = tree("reuse");
    store.verify(replayer(BUILD, None, None), false).unwrap();
    // A stale entry for the next id (a merge that raced a remove, an older build's file).
    let mut status = read_status(&root);
    status.insert(CheckpointId(5), Status::Broken("old".into()));
    fs::write(root.join("status.ron"), ron::to_string(&status).unwrap()).unwrap();
    let e = add(&store, "e", Parent::Checkpoint(a), &[(Buttons::B, 1)]);
    assert_eq!(e, CheckpointId(5));
    assert!(!store.statuses().unwrap().contains_key(&e));
    // Removed, then reused: the new node starts unverified.
    store.remove(e).unwrap();
    store.remove(d).unwrap();
    let again = add(&store, "again", Parent::Checkpoint(a), &[(Buttons::B, 1)]);
    assert_eq!(again, d);
    assert!(!store.statuses().unwrap().contains_key(&again));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_layered_store_indexes_both_and_overlays_statuses() {
    let (base_root, base, [a, b, c, _]) = tree("lbase");
    base.verify(replayer(BUILD, None, None), false).unwrap();
    let local_root = temp_root("llocal");
    let store = Store::layered(&base_root, &local_root).unwrap();
    assert_eq!(store.base(), Some(base_root.as_path()));
    let mine = add(&store, "mine", Parent::Checkpoint(c), &[(Buttons::UP, 3)]);
    let index = store.index().unwrap();
    assert!(index.is_base(a) && store.is_base(a));
    assert!(!index.is_base(mine) && !store.is_base(mine));
    assert_eq!(index.children(Parent::Checkpoint(c)), [mine]);
    assert_eq!(index.chain(mine).unwrap(), [a, b, c, mine]);
    // The base's status.ron, overlaid with the local one; merges go to the local one.
    let diverged = Status::Diverged {
        hash: None,
        parent_diverged: false,
    };
    store
        .merge_statuses(&[(b, diverged.clone()), (mine, Status::Valid)])
        .unwrap();
    let all = store.statuses().unwrap();
    assert_eq!(
        (&all[&a], &all[&b], &all[&mine]),
        (&Status::Valid, &diverged, &Status::Valid)
    );
    assert_eq!(
        read_status(&base_root)[&b],
        Status::Valid,
        "the base is untouched"
    );
    fs::remove_dir_all(base_root).unwrap();
    fs::remove_dir_all(local_root).unwrap();
}

#[test]
fn replay_reports_a_recorded_hash_past_the_end() {
    let (mut trace, hashes) = recorded_trace();
    trace.hashes.push((400, 7));
    let game = content::Emerald::for_shells();
    let report = replay(&game, &mut World::new(&game), &trace, |_, _| {});
    assert_eq!(
        report.first_mismatch,
        Some(HashMismatch {
            frame: 400,
            want: 7,
            got: hashes[299]
        })
    );
}

#[test]
#[should_panic(expected = "start frame")]
fn replay_requires_the_world_at_the_start_frame() {
    let game = content::Emerald::for_shells();
    let mut t = trace(Parent::NewGame, 0, &[(Buttons::NONE, 1)]);
    t.start_frame = 5;
    replay(&game, &mut World::new(&game), &t, |_, _| {});
}

/// Every file of a store dir, by relative path.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

#[test]
fn verify_in_parallel_gives_the_serial_verdicts_and_writes() {
    let hint = HashMismatch {
        frame: 60,
        want: 1,
        got: 2,
    };
    let cases = [
        (BUILD, None, None, false),
        (OTHER_BUILD, None, None, true),
        (OTHER_BUILD, Some(30), Some(hint), false),
        (OTHER_BUILD, Some(30), Some(hint), true),
        // b replays to another state on its own build: broken, and so is its child.
        (BUILD, Some(30), None, false),
    ];
    for (i, (build, salt, hint, accept)) in cases.into_iter().enumerate() {
        let (r1, s1, _) = tree(&format!("par-serial-{i}"));
        let (r2, s2, _) = tree(&format!("par-par-{i}"));
        // Twice: the second pass starts from the snapshots the first one cached.
        for pass in 0..2 {
            let serial = s1
                .verify_nodes(replayer(build, salt, hint), accept)
                .unwrap();
            let replays = std::sync::atomic::AtomicU32::new(0);
            let counted = |p: Option<&Checkpoint>, t: &Trace| {
                replays.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                par_replayer(build, salt, hint)(p, t)
            };
            let par = s2.verify_nodes_par(counted, accept, 4).unwrap();
            assert_eq!(par, serial, "case {i} pass {pass}");
            let (f1, f2) = (files(&r1), files(&r2));
            assert_eq!(
                f1.keys().collect::<Vec<_>>(),
                f2.keys().collect::<Vec<_>>(),
                "case {i}"
            );
            for (k, v) in &f1 {
                assert_eq!(v, &f2[k], "case {i} pass {pass}: {}", k.display());
            }
            let replays = replays.into_inner();
            // At most one speculative and one checked replay per node.
            assert!(replays <= 8, "case {i}: {replays} replays");
            if pass == 1 && i == 0 {
                // Every parent's snapshot is cached and right: one replay per node.
                assert_eq!(replays, 4, "case {i}");
            }
        }
        fs::remove_dir_all(r1).unwrap();
        fs::remove_dir_all(r2).unwrap();
    }
}
