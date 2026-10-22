use super::*;

#[test]
fn capacities_are_gen3_emeralds() {
    assert_eq!(POCKET_CAPACITY.iter().sum::<usize>(), BAG_SLOTS);
    let bag = Bag::new();
    for (p, cap) in Pocket::ALL.into_iter().zip(POCKET_CAPACITY) {
        assert_eq!(bag.pocket(p).len(), cap);
    }
}

#[test]
fn digest_follows_writes_and_snapshots_round_trip() {
    let mut bag = Bag::new();
    let empty = bag.digest;
    let potion = Slot {
        item: ItemId(13),
        qty: 3,
    };
    bag.set_slot(Pocket::Items, 2, potion);
    bag.set_slot(
        Pocket::KeyItems,
        0,
        Slot {
            item: ItemId(259),
            qty: 1,
        },
    );
    assert_eq!(bag.digest, bag.full_digest());
    assert_eq!(bag.count(ItemId(13)), 3);
    let back = Bag::from_snapshot(&bag.snapshot()).unwrap();
    assert_eq!(back, bag);
    bag.set_slot(Pocket::Items, 2, Slot::default());
    bag.set_slot(Pocket::KeyItems, 0, Slot::default());
    assert_eq!(bag.digest, empty);
}

#[test]
fn forged_snapshots_are_refused() {
    let slot = |pocket, index, qty| BagSlotSnap {
        pocket,
        index,
        item: ItemId(1),
        qty,
    };
    for bad in [
        vec![slot(Pocket::PokeBalls, 16, 1)],
        vec![slot(Pocket::Items, 0, 0)],
        vec![slot(Pocket::Items, 0, 100)],
        vec![slot(Pocket::Items, 0, 1), slot(Pocket::Items, 0, 2)],
    ] {
        assert!(Bag::from_snapshot(&BagSnap { slots: bad }).is_err());
    }
}
