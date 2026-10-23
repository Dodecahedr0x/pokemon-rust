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
