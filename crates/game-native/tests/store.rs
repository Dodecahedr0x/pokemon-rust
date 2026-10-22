//! Milestone 5 Task D: traces (replay on real content) and the checkpoint store (synthetic
//! checkpoints, temp dirs, a fake replayer; nothing here restores a World).

use engine::checkpoint::{
    AnimSnap, AnimTableSnap, BuildHash, Checkpoint, FaderSnap, FieldSnap, Header, IdleSnap,
    InputSnap, ObjectSnap, ObjectsSnap, RunStateSnap, SCHEMA, TransitionSnap, VarsSnap,
    WorldSnapshot,
};
use engine::data::{Behavior, Dir, FlagId, MapId, MovementType};
use engine::{Buttons, World};
use game_native::store::{Node, Replayed, Status, Store};
use game_native::trace::{
    CheckpointId, HashMismatch, Parent, ReplayReport, Run, TRACE_SCHEMA, Trace, TraceRecorder,
    replay, replay_with_hashes,
};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------------------------
// Traces on real content

/// 300 frames of the route's keys, recorded on a live session with their hashes.
fn recorded_trace() -> (Trace, Vec<u64>) {
    let rec = game_native::scenario::record_route().expect("route records");
    let game = content::Emerald::for_shells();
    let mut world = World::new(&game);
    let mut r = TraceRecorder::new(world.frame());
    let mut hashes = Vec::new();
    for i in 0..300 {
        let keys = rec.keys_at(i);
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

static DIRS: AtomicU32 = AtomicU32::new(0);

/// A fresh empty directory under the target dir (removed first if a previous run left it).
fn temp_root(name: &str) -> PathBuf {
    let n = DIRS.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("pkr-store-test-{}-{name}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

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

/// A fake replayer for `build`, adding 1 to the state of the trace ending at frame `salt_at`.
fn replayer(
    build: BuildHash,
    salt_at: Option<u32>,
    hint: Option<HashMismatch>,
) -> impl FnMut(Option<&Checkpoint>, &Trace) -> Result<Replayed, String> {
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
            hashes: vec![(trace.frames(), cp.header.world_hash)],
            checkpoint: cp,
        })
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
        assert_eq!(
            store.find(q).unwrap_err().kind(),
            io::ErrorKind::NotFound,
            "{q}"
        );
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
    let kind = |r: io::Result<CheckpointId>| r.unwrap_err().kind();
    let e = kind(store.add("a", t(Parent::Checkpoint(a), 10), &cp));
    assert_eq!(e, io::ErrorKind::AlreadyExists, "labels are unique");
    for label in ["", "7", "c00007"] {
        let e = kind(store.add(label, t(Parent::Checkpoint(a), 10), &cp));
        assert_eq!(e, io::ErrorKind::InvalidInput, "label {label:?}");
    }
    let e = kind(store.add("x", t(Parent::Checkpoint(CheckpointId(42)), 10), &cp));
    assert_eq!(e, io::ErrorKind::NotFound, "missing parent");
    let e = kind(store.add("x", t(Parent::Checkpoint(a), 9), &cp));
    assert_eq!(
        e,
        io::ErrorKind::InvalidInput,
        "start frame != parent frame"
    );
    let e = kind(store.add("x", t(Parent::Checkpoint(a), 10), &checkpoint(13, 3, BUILD)));
    assert_eq!(
        e,
        io::ErrorKind::InvalidInput,
        "end frame != checkpoint frame"
    );
    let e = kind(store.add("x", t(Parent::NewGame, 5), &checkpoint(7, 3, BUILD)));
    assert_eq!(e, io::ErrorKind::InvalidInput, "a new game starts at 0");
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
    assert_eq!(
        store.relabel(c, "bee").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(store.relabel(CheckpointId(9), "z").is_err());

    assert!(store.remove(a).is_err(), "a has children");
    assert!(store.remove(b).is_err(), "b has a child");
    store.remove(c).unwrap();
    assert!(!root.join("snapshots/c00003.ron").exists());
    store.remove(b).unwrap();
    store.remove(d).unwrap();
    store.remove(a).unwrap();
    assert!(store.nodes().unwrap().is_empty());
    assert_eq!(store.remove(a).unwrap_err().kind(), io::ErrorKind::NotFound);
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
    // accept never touches Broken nodes.
    assert_eq!(store.find("b").unwrap().meta.build, BUILD);
    assert_eq!(store.snapshot(b).unwrap().unwrap().header.build, BUILD);
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
    // Untouched: the valid nodes' files.
    assert_eq!(a_file, fs::read(root.join("nodes/c00001.ron")).unwrap());
    // Re-verifying on the new build is now valid everywhere.
    let out = store
        .verify(replayer(OTHER_BUILD, Some(30), None), false)
        .unwrap();
    assert!(out.iter().all(|(_, s)| *s == Status::Valid), "{out:?}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_root_is_per_flavor() {
    if std::env::var_os("PKR_CHECKPOINTS").is_some() {
        return;
    }
    for (stub, dir) in [(true, "stub"), (false, "emerald")] {
        let r = Store::default_root(stub);
        assert!(r.ends_with(Path::new("checkpoints").join(dir)), "{r:?}");
        assert!(
            r.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Cargo.toml")
                .is_file()
        );
    }
}
