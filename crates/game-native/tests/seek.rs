//! Milestone 6 Task C: the `Materializer` (LRU, per-link divergence), seek keyframes, `seek`
//! against a straight replay, and `Controls::from_seek` saves, on the route's nodes (stub or
//! extracted data).

use engine::Buttons;
use engine::checkpoint::state_digest;
use game_native::Session;
use game_native::checkpoints::{
    KEYFRAME_EVERY, Link, Materializer, SeekPrep, add_save, replayer, seed, seek,
};
use game_native::controls::Controls;
use game_native::scenario::cached_route;
use game_native::store::{Node, Status, Store};
use game_native::trace::{CheckpointId, Parent};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

mod common;
use common::temp_root;

type Game = content::Emerald;

fn game() -> Game {
    content::Emerald::for_shells()
}

/// The route seeded once per process into a store whose snapshots are copied by each test.
fn seeded() -> &'static (PathBuf, Vec<CheckpointId>) {
    static SEEDED: OnceLock<(PathBuf, Vec<CheckpointId>)> = OnceLock::new();
    SEEDED.get_or_init(|| {
        let root = temp_root("seek-seed");
        let store = Store::open(&root).unwrap();
        let ids = seed(&store, &game(), cached_route()).unwrap();
        (root, ids)
    })
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

/// A private copy of the seeded store.
fn store(tag: &str) -> (PathBuf, Store, Vec<CheckpointId>) {
    let (src, ids) = seeded();
    let root = temp_root(tag);
    copy_dir(src, &root);
    (root.clone(), Store::open(&root).unwrap(), ids.clone())
}

fn node(store: &Store, id: CheckpointId) -> Node {
    store.find(&id.to_string()).unwrap()
}

fn link(id: CheckpointId, cached: bool, diverged: bool) -> Link {
    Link {
        id,
        cached,
        diverged,
    }
}

#[test]
fn lru_hits_without_reading_snapshots_and_evicts_the_least_recent() {
    let (root, store, ids) = store("seek-lru");
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let mut m = Materializer::new(game(), 2);
    let last = *ids.last().unwrap();
    let first = m.get(&store, last).unwrap();
    assert!(!first.diverged);
    let replayed: Vec<Link> = ids.iter().map(|&id| link(id, false, false)).collect();
    assert_eq!(first.chain, replayed);
    assert_eq!(m.len(), 2, "capacity");
    // A hit reads no snapshot.
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let hit = m.get(&store, last).unwrap();
    assert_eq!(hit.checkpoint, first.checkpoint);
    let mut want: Vec<Link> = ids.iter().map(|&id| link(id, true, false)).collect();
    assert_eq!(hit.chain, want);
    assert!(!root.join("snapshots").exists());
    // The one before the last is in memory too; the root was evicted.
    let prev = ids[ids.len() - 2];
    want.pop();
    assert_eq!(m.get(&store, prev).unwrap().chain, want);
    let root_node = m.get(&store, ids[0]).unwrap();
    assert_eq!(root_node.chain, [link(ids[0], false, false)], "replayed");
    // That evicted the least recently used: `last` (prev was used after it).
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let again = m.get(&store, last).unwrap();
    assert!(!again.chain.last().unwrap().cached, "evicted");
    assert_eq!(again.checkpoint, first.checkpoint);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_reused_id_misses_the_lru() {
    let (root, store, ids) = store("seek-reuse");
    let mut m = Materializer::new(game(), 8);
    let last = *ids.last().unwrap();
    let cp = m.get(&store, last).unwrap().checkpoint;
    // Node `last` rewritten with another state's digest: not served from memory.
    let mut n = node(&store, last);
    n.meta.digest ^= 1;
    std::fs::write(
        root.join(format!("nodes/{last}.ron")),
        ron::to_string(&n).unwrap(),
    )
    .unwrap();
    let got = m.get(&store, last).unwrap();
    assert!(got.diverged, "replayed and found diverged");
    assert!(!got.chain.last().unwrap().cached);
    assert_eq!(got.checkpoint, cp);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn chain_flags_a_forged_diverged_middle_node_only() {
    let (root, store, ids) = store("seek-chain");
    assert!(ids.len() >= 3);
    let mid = ids[1];
    let mut n = node(&store, mid);
    n.meta.digest ^= 1;
    std::fs::write(
        root.join(format!("nodes/{mid}.ron")),
        ron::to_string(&n).unwrap(),
    )
    .unwrap();
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let mut m = Materializer::new(game(), 8);
    let last = *ids.last().unwrap();
    let got = m.get(&store, last).unwrap();
    let want: Vec<Link> = (ids.iter()).map(|&id| link(id, false, id == mid)).collect();
    assert_eq!(got.chain, want);
    assert!(!got.diverged, "the target still reaches its digest");
    assert!(
        store.snapshot(mid).unwrap().is_none(),
        "a diverged state is not cached"
    );
    assert!(store.snapshot(last).unwrap().is_some());
    // From memory the flags are kept.
    let again = m.get(&store, last).unwrap();
    let want: Vec<Link> = (ids.iter()).map(|&id| link(id, true, id == mid)).collect();
    assert_eq!(again.chain, want);
    let m2 = m.get(&store, mid).unwrap();
    assert!(m2.diverged);
    let _ = std::fs::remove_dir_all(root);
}

/// The node of the route whose trace is longest (it has keyframes).
fn longest(store: &Store, ids: &[CheckpointId]) -> Node {
    (ids.iter())
        .map(|&id| node(store, id))
        .max_by_key(|n| n.trace.frames())
        .unwrap()
}

/// A straight replay of `n`'s trace from its parent.
fn straight(m: &mut Materializer<Game>, store: &Store, n: &Node) -> Session<Game> {
    match n.trace.parent {
        Parent::NewGame => Session::new(game()),
        Parent::Checkpoint(p) => {
            Session::restore(game(), &m.get(store, p).unwrap().checkpoint).unwrap()
        }
    }
}

#[test]
fn seek_equals_a_straight_replay_at_0_k_k1_and_the_end() {
    let (root, store, ids) = store("seek-straight");
    let mut m = Materializer::new(game(), 8);
    let n = longest(&store, &ids);
    let id = n.meta.id;
    let len = n.trace.frames();
    assert!(len > KEYFRAME_EVERY + 1, "{len} frames");
    let kfs = m.keyframes(&store, id).unwrap();
    assert!(!kfs.is_empty());
    for w in kfs.windows(2) {
        assert!(w[1].0 > w[0].0 && w[1].0 / KEYFRAME_EVERY > w[0].0 / KEYFRAME_EVERY);
    }
    let k = kfs[0].0;
    assert!(k >= KEYFRAME_EVERY);
    let keys: Vec<Buttons> = n.trace.keys().collect();
    let mut live = straight(&mut m, &store, &n);
    let mut at = 0u32;
    let mut targets = [0, 1, k - 1, k, k + 1, len / 2, len - 1, len, len + 100];
    targets.sort_unstable();
    for target in targets {
        while at < target.min(len) {
            live.step(keys[at as usize]);
            at += 1;
        }
        let mut s = seek(&mut m, &store, id, target).unwrap();
        assert!(!s.diverged, "seek {target}");
        assert_eq!(s.parent, n.trace.parent);
        assert_eq!(s.recorder.frames(), target.min(len));
        assert_eq!(
            s.session.world().frame(),
            live.world().frame(),
            "seek {target}"
        );
        assert_eq!(
            s.session.world().hash(),
            live.world().hash(),
            "seek {target}"
        );
        assert_eq!(s.session.framebuffer(), live.framebuffer(), "seek {target}");
        // The recorder holds the trace's prefix: its hashes are the trace's.
        let t = s.recorder.trace(n.trace.parent, n.trace.build);
        assert_eq!(t.keys().collect::<Vec<_>>(), keys[..at as usize]);
        let want: Vec<(u32, u64)> = (n.trace.hashes.iter())
            .copied()
            .filter(|&(i, _)| i < at && i % 60 == 0)
            .collect();
        let got: Vec<(u32, u64)> = t.hashes.iter().copied().filter(|&(i, _)| i < at).collect();
        assert_eq!(got, want, "seek {target}");
        if at > 0 {
            assert_eq!(t.hashes.last().unwrap(), &(at, live.world().hash()));
        }
    }
    // At the end the session is the node's state.
    let s = seek(&mut m, &store, id, len).unwrap();
    let cp = s.session.checkpoint().unwrap();
    assert_eq!(state_digest(&cp.world), n.meta.digest);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn seek_into_a_battle_replays_from_the_last_capturable_keyframe() {
    let (root, store, ids) = store("seek-battle");
    let mut m = Materializer::new(game(), 8);
    // The first battle frame past a keyframe, in any node (the stub's one battle comes
    // before its node's first keyframe: then the first battle frame).
    let mut found = None;
    for after_keyframe in [true, false] {
        'nodes: for &id in &ids {
            let n = node(&store, id);
            let kfs = m.keyframes(&store, id).unwrap();
            let k = match kfs.first() {
                Some(&(k, _)) if after_keyframe => k,
                _ if after_keyframe => continue,
                _ => 0,
            };
            let mut live = straight(&mut m, &store, &n);
            let keys: Vec<Buttons> = n.trace.keys().collect();
            for (i, &keys) in (1..).zip(&keys) {
                live.step(keys);
                if i > k && live.world().in_battle() {
                    found = Some((n, i, live, after_keyframe));
                    break 'nodes;
                }
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (n, battle, mut live, after_keyframe) = found.expect("a battle");
    assert!(
        after_keyframe || content::STUB,
        "the extracted route battles late"
    );
    // 30 frames into the battle (a trace ends capturable, so never inside one).
    let target = (battle + 30).min(n.trace.frames());
    let keys: Vec<Buttons> = n.trace.keys().collect();
    for i in battle..target {
        live.step(keys[i as usize]);
    }
    assert!(live.world().in_battle(), "frame {target}");
    let start = m.seek_start(&store, n.meta.id, target).unwrap();
    assert!(start.from < target);
    assert_eq!(start.from > 0, after_keyframe, "from a keyframe");
    let mut s = seek(&mut m, &store, n.meta.id, target).unwrap();
    assert!(s.session.world().in_battle());
    assert_eq!(s.session.world().hash(), live.world().hash());
    assert_eq!(s.session.framebuffer(), live.framebuffer());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn from_seek_then_30_frames_then_save_replays_valid_in_a_layered_store() {
    let (root, base, ids) = store("seek-save");
    let local = temp_root("seek-save-local");
    let store = Store::layered(&root, &local).unwrap();
    let mut m = Materializer::new(game(), 8);
    let n = longest(&base, &ids);
    let mid = n.trace.frames() / 2;
    let s = seek(&mut m, &store, n.meta.id, mid).unwrap();
    let mut c = Controls::from_seek(s);
    assert_eq!(c.parent(), n.trace.parent);
    assert_eq!(c.trace().frames(), mid);
    let rec = cached_route();
    let step = |c: &mut Controls<Game>| {
        let f = c.session().world().frame();
        c.step(rec.keys_at(f))
    };
    for _ in 0..30 {
        assert_eq!(step(&mut c), None);
    }
    let mut req = c.save();
    for _ in 0..3000 {
        if req.is_some() {
            break;
        }
        req = step(&mut c);
    }
    let req = req.expect("a capturable frame");
    assert_eq!(req.trace.start_frame, n.trace.start_frame);
    let id = add_save(&store, &req).unwrap();
    c.saved(id);
    assert!(local.join(format!("nodes/{id}.ron")).exists());
    let verdicts = store.verify(replayer(&game()), false).unwrap();
    let v = verdicts.iter().find(|v| v.0 == id).expect("verified");
    assert_eq!(v.1, Status::Valid);
    // The same label again gets a suffix.
    let again = add_save(&store, &req).unwrap();
    assert_eq!(node(&store, again).meta.label, format!("{} #2", req.label));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(local);
}

#[test]
fn prepare_seek_plans_the_keyframe_replay_then_serves_it_from_the_cache() {
    let (root, store, ids) = store("seek-prepare");
    let mut m = Materializer::new(game(), 8);
    let n = longest(&store, &ids);
    // The end: at or past the first keyframe (the stub's only one may be the last frame).
    let target = n.trace.frames();
    let SeekPrep::Keyframes(plan) = m.prepare_seek(&store, n.meta.id, target).unwrap() else {
        panic!("cold: a keyframe replay to run");
    };
    assert_eq!(plan.frames, target);
    let built = plan.build(&game()).unwrap();
    let start = built.start();
    assert!(start.from > 0, "from a keyframe");
    m.insert_keyframes(built);
    let SeekPrep::Ready(again) = m.prepare_seek(&store, n.meta.id, target).unwrap() else {
        panic!("warm: ready");
    };
    assert_eq!(again, start);
    // Below the first keyframe: from the parent, without keyframes.
    let SeekPrep::Ready(early) = m.prepare_seek(&store, n.meta.id, 10).unwrap() else {
        panic!("ready");
    };
    assert_eq!(early.from, 0);
    let mut a = start.run(game()).unwrap();
    let mut b = seek(&mut m, &store, n.meta.id, target).unwrap();
    assert_eq!(a.session.world().hash(), b.session.world().hash());
    assert_eq!(a.session.framebuffer(), b.session.framebuffer());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_late_hash_mismatch_flags_only_seeks_past_it() {
    let (root, store, ids) = store("seek-mismatch");
    let n = longest(&store, &ids);
    let len = n.trace.frames();
    // The recorded end hash is forged; the state (digest) is still the node's.
    let mut forged = n.clone();
    forged.trace.hashes.last_mut().unwrap().1 ^= 1;
    assert_eq!(forged.trace.hashes.last().unwrap().0, len);
    std::fs::write(
        root.join(format!("nodes/{}.ron", n.meta.id)),
        ron::to_string(&forged).unwrap(),
    )
    .unwrap();
    let mut m = Materializer::new(game(), 8);
    for (target, diverged) in [(len - 1, false), (KEYFRAME_EVERY + 1, false), (len, true)] {
        let s = seek(&mut m, &store, n.meta.id, target).unwrap();
        assert_eq!(s.diverged, diverged, "seek {target}");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Seek timings (cold: keyframes built; warm: cached). Printed, not asserted: `cargo test
/// --release -p game-native --test seek -- --ignored --nocapture seek_timings`.
#[test]
#[ignore = "timings: cargo test --release -p game-native --test seek -- --ignored --nocapture"]
fn seek_timings() {
    let (root, store, ids) = store("seek-time");
    let mut m = Materializer::new(game(), 8);
    let n = match store.find("route:after_wild") {
        Ok(n) => n,
        Err(_) => longest(&store, &ids),
    };
    let mid = n.trace.frames() / 2;
    let t = Instant::now();
    let s = seek(&mut m, &store, n.meta.id, mid).unwrap();
    let cold = t.elapsed();
    let t = Instant::now();
    let s2 = seek(&mut m, &store, n.meta.id, mid).unwrap();
    let warm = t.elapsed();
    assert_eq!(s.session.world().hash(), s2.session.world().hash());
    let t = Instant::now();
    let _ = seek(&mut m, &store, n.meta.id, mid + 300).unwrap();
    let warm_far = t.elapsed();
    println!(
        "seek {} ({} frames) to {mid}: cold {cold:?}, warm {warm:?}, warm +300 {warm_far:?}",
        n.meta.label,
        n.trace.frames()
    );
    // Cold without the disk cache: the parent chain is replayed too.
    std::fs::remove_dir_all(root.join("snapshots")).unwrap();
    let mut m = Materializer::new(game(), 8);
    let t = Instant::now();
    let _ = seek(&mut m, &store, n.meta.id, mid).unwrap();
    println!("  cold, no snapshots: {:?}", t.elapsed());
    let _ = std::fs::remove_dir_all(root);
}
