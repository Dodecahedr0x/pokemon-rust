use super::*;

#[test]
fn blank_trees_hash_nothing_and_unknown_ids_are_ignored() {
    let mut t = BerryTrees::new(4);
    let mut h = Fnv64::new();
    t.hash(&mut h);
    assert_eq!(h.finish(), Fnv64::new().finish());
    let oran = BerryTree {
        berry: 7,
        stage: 5,
        berry_yield: 2,
        ..BerryTree::default()
    };
    t.set(2, oran);
    t.set(9, oran);
    assert_eq!(t.get(2), oran);
    assert_eq!(t.get(9), BerryTree::default());
    assert_eq!(t.entries().collect::<Vec<_>>(), [(2, oran)]);
}

#[test]
fn the_cached_digest_follows_every_set() {
    let oran = BerryTree {
        berry: 7,
        stage: 5,
        berry_yield: 2,
        ..BerryTree::default()
    };
    let digest = |t: &BerryTrees| {
        let mut h = Fnv64::new();
        t.hash(&mut h);
        h.finish()
    };
    let mut a = BerryTrees::new(4);
    a.set(1, oran);
    let first = digest(&a);
    assert_eq!(digest(&a), first, "cached");
    a.set(1, BerryTree { stage: 4, ..oran });
    assert_ne!(digest(&a), first, "a set clears the cache");
    let mut b = BerryTrees::new(4);
    b.set(1, BerryTree { stage: 4, ..oran });
    assert_eq!(digest(&a), digest(&b));
    assert_eq!(a, b);
    a.set(1, BerryTree::default());
    assert_eq!(digest(&a), Fnv64::new().finish(), "blank again: nothing");
}
