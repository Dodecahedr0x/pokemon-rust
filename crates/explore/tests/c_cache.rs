//! Task C: `cache::reach` on a 3-node lineage built by hand with `Session` (either flavor).

mod c_common;

use c_common::{build_lineage, lineage_rows};
use content::Emerald;
use engine::Buttons;
use engine::checkpoint::{build_hash, state_digest};
use explore::GenerationDelta;
use explore::cache::{ReachError, SnapCache, reach};
use explore::db::Db;

fn setup() -> (Emerald, Db, Vec<u64>, Vec<engine::checkpoint::Checkpoint>) {
    let game = Emerald::for_shells();
    let built = build_lineage(&game, &[Buttons::RIGHT, Buttons::DOWN]);
    let rows = lineage_rows(&built, build_hash(&game));
    let (mut db, _) = Db::open_in_memory(build_hash(&game), content::STUB).unwrap();
    db.apply(&GenerationDelta {
        nodes: rows.clone(),
        ..Default::default()
    })
    .unwrap();
    let digests: Vec<u64> = rows.iter().map(|n| n.digest).collect();
    assert!(digests[0] != digests[1] && digests[1] != digests[2]);
    (game, db, digests, built.into_iter().map(|b| b.cp).collect())
}

#[test]
fn reach_replays_the_lineage_from_an_empty_cache() {
    let (game, db, digests, cps) = setup();
    let mut cache = SnapCache::new(16);
    let cp = reach(&game, &db, &mut cache, 3).unwrap();
    assert_eq!(state_digest(&cp.world), digests[2]);
    assert_eq!(*cp, cps[2]);
    // Every state on the way is cached; the next reach of an ancestor is a hit.
    assert_eq!(cache.len(), 3);
    let (hits, _) = cache.hit_stats();
    let cp1 = reach(&game, &db, &mut cache, 1).unwrap();
    assert_eq!(state_digest(&cp1.world), digests[0]);
    assert_eq!(cache.hit_stats().0, hits + 1);
}

#[test]
fn reach_starts_from_a_trusted_db_snapshot() {
    let (game, mut db, digests, cps) = setup();
    // A snapshot whose digest is not the node's is ignored (replay from a new game).
    db.put_snapshot(2, &cps[0]).unwrap();
    let cp = reach(&game, &db, &mut SnapCache::new(16), 3).unwrap();
    assert_eq!(state_digest(&cp.world), digests[2]);
    // A good snapshot of node 2: only node 3 is replayed.
    db.put_snapshot(2, &cps[1]).unwrap();
    let mut cache = SnapCache::new(16);
    let cp = reach(&game, &db, &mut cache, 3).unwrap();
    assert_eq!(state_digest(&cp.world), digests[2]);
    assert!(!cache.contains(1) && cache.contains(2) && cache.contains(3));
    // A snapshot of another build is not used.
    let mut other = cps[1].clone();
    other.header.build.engine ^= 1;
    db.put_snapshot(2, &other).unwrap();
    let mut cache = SnapCache::new(16);
    reach(&game, &db, &mut cache, 3).unwrap();
    assert!(cache.contains(1));
}

#[test]
fn a_wrong_digest_diverges() {
    let (game, mut db, digests, _) = setup();
    let mut n3 = db.node(3).unwrap().unwrap();
    n3.digest ^= 1;
    db.set_status(&[n3]).unwrap();
    match reach(&game, &db, &mut SnapCache::new(16), 3) {
        Err(ReachError::Diverged { node: 3, want, got }) => {
            assert_eq!((want, got), (digests[2] ^ 1, digests[2]));
        }
        r => panic!("{:?}", r.map(|_| ())),
    }
    assert!(matches!(
        reach(&game, &db, &mut SnapCache::new(16), 9),
        Err(ReachError::Missing(9))
    ));
}
