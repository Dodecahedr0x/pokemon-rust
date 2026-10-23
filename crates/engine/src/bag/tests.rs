use super::*;

/// Gen3 Emerald's pockets (`include/constants/global.h`), as content gives them.
const SIZES: [u8; 5] = [30, 30, 16, 64, 46];

#[test]
fn pockets_have_the_games_sizes() {
    let bag = Bag::new(SIZES);
    assert_eq!(bag.slots.len(), 186);
    for (p, n) in Pocket::ALL.into_iter().zip(SIZES) {
        assert_eq!(bag.pocket(p).len(), n as usize);
        assert_eq!(bag.pocket_len(p), n as usize);
    }
    assert!(Bag::new([0; 5]).slots.is_empty());
}

#[test]
fn digest_follows_writes_and_snapshots_round_trip() {
    let mut bag = Bag::new(SIZES);
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
    let back = Bag::from_snapshot(&bag.snapshot(), SIZES).unwrap();
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
        vec![slot(Pocket::Berries, 0, 1000)],
        vec![slot(Pocket::Items, 0, 1), slot(Pocket::Items, 0, 2)],
    ] {
        assert!(Bag::from_snapshot(&BagSnap { slots: bad }, SIZES).is_err());
    }
}

// ---- Gen3 `item.c` rules (Milestone 8 Task H) ----

const POTION: ItemId = ItemId(13);
const ANTIDOTE: ItemId = ItemId(14);

fn qtys(bag: &Bag, pocket: Pocket) -> alloc::vec::Vec<(u16, u16)> {
    (bag.pocket(pocket).iter())
        .filter(|s| s.qty > 0)
        .map(|s| (s.item.0, s.qty))
        .collect()
}

#[test]
fn adding_stacks_on_the_existing_slot_then_overflows_into_new_ones() {
    let mut bag = Bag::new(SIZES);
    assert!(bag.add(Pocket::Items, POTION, 3));
    assert!(bag.add(Pocket::Items, ANTIDOTE, 1));
    assert!(bag.add(Pocket::Items, POTION, 5));
    assert_eq!(qtys(&bag, Pocket::Items), [(13, 8), (14, 1)]);
    // 8 + 200 = 99 + 99 + 10: the stack fills, then the first empty slots.
    assert!(bag.add(Pocket::Items, POTION, 200));
    assert_eq!(
        qtys(&bag, Pocket::Items),
        [(13, 99), (14, 1), (13, 99), (13, 10)]
    );
    assert_eq!(bag.count(POTION), 208);
    assert_eq!(bag.digest, bag.full_digest());
}

#[test]
fn a_full_pocket_refuses_new_items_but_tops_up_stacks() {
    let mut bag = Bag::new(SIZES);
    for i in 0..16 {
        assert!(bag.add(Pocket::PokeBalls, ItemId(1 + i), 1));
    }
    let before = bag.clone();
    assert!(!bag.has_space(Pocket::PokeBalls, ItemId(17), 1));
    assert!(!bag.add(Pocket::PokeBalls, ItemId(17), 1));
    assert_eq!(bag, before, "a refused add changes nothing");
    assert!(bag.has_space(Pocket::PokeBalls, ItemId(1), 98));
    assert!(bag.add(Pocket::PokeBalls, ItemId(1), 98));
    assert!(
        !bag.has_space(Pocket::PokeBalls, ItemId(1), 1),
        "99 and no empty slot"
    );
    assert!(!bag.add(Pocket::PokeBalls, ItemId(1), 1));
}

#[test]
fn an_add_that_does_not_fit_whole_changes_nothing() {
    let mut bag = Bag::new(SIZES);
    for i in 0..29 {
        assert!(bag.add(Pocket::Items, ItemId(100 + i), 1));
    }
    assert!(bag.add(Pocket::Items, POTION, 90));
    let before = bag.clone();
    // 9 tops the stack up, then 100 more need two slots: none left.
    assert!(!bag.has_space(Pocket::Items, POTION, 9 + 100));
    assert!(!bag.add(Pocket::Items, POTION, 9 + 100));
    assert_eq!(bag, before);
    assert!(bag.add(Pocket::Items, POTION, 9));
}

#[test]
fn tms_and_berries_never_take_a_second_slot() {
    let mut bag = Bag::new(SIZES);
    assert!(bag.add(Pocket::TmHm, ItemId(289), 99));
    assert!(!bag.has_space(Pocket::TmHm, ItemId(289), 1));
    assert!(!bag.add(Pocket::TmHm, ItemId(289), 1));
    assert!(!bag.add(Pocket::TmHm, ItemId(290), 100), "more than a slot");
    // Berries stack to 999 (Gen3 `MAX_BERRY_CAPACITY`).
    assert!(bag.add(Pocket::Berries, ItemId(133), 998));
    assert!(bag.add(Pocket::Berries, ItemId(133), 1));
    assert!(!bag.add(Pocket::Berries, ItemId(133), 1));
    assert_eq!(qtys(&bag, Pocket::Berries), [(133, 999)]);
    let back = Bag::from_snapshot(&bag.snapshot(), SIZES).unwrap();
    assert_eq!(back, bag, "999 berries survive a snapshot");
}

#[test]
fn key_items_follow_the_same_rules() {
    let mut bag = Bag::new(SIZES);
    assert!(bag.add(Pocket::KeyItems, ItemId(259), 1));
    assert!(bag.add(Pocket::KeyItems, ItemId(259), 1));
    assert_eq!(qtys(&bag, Pocket::KeyItems), [(259, 2)]);
    assert!(bag.has(Pocket::KeyItems, ItemId(259), 2));
    assert!(!bag.has(Pocket::KeyItems, ItemId(259), 3));
}

#[test]
fn removing_takes_from_slots_in_order_and_needs_the_whole_amount() {
    let mut bag = Bag::new(SIZES);
    assert!(bag.add(Pocket::Items, POTION, 150)); // 99 + 51
    assert!(bag.add(Pocket::Items, ANTIDOTE, 2));
    assert!(bag.has(Pocket::Items, POTION, 150));
    assert!(!bag.has(Pocket::Items, POTION, 151));
    let before = bag.clone();
    assert!(!bag.remove(Pocket::Items, POTION, 151));
    assert_eq!(bag, before);
    assert!(bag.remove(Pocket::Items, POTION, 100));
    // The first slot empties in place (no compaction without a bag menu).
    assert_eq!(qtys(&bag, Pocket::Items), [(13, 50), (14, 2)]);
    assert_eq!(bag.pocket(Pocket::Items)[0], Slot::default());
    assert!(bag.remove(Pocket::Items, ANTIDOTE, 2));
    assert!(!bag.has(Pocket::Items, ANTIDOTE, 1));
    // A freed slot is reused first.
    assert!(bag.add(Pocket::Items, ItemId(15), 1));
    assert_eq!(bag.pocket(Pocket::Items)[0].item, ItemId(15));
    assert_eq!(bag.digest, bag.full_digest());
}

#[test]
fn item_none_is_never_added_removed_or_had() {
    let mut bag = Bag::new(SIZES);
    assert!(!bag.add(Pocket::Items, ItemId(0), 1));
    assert!(!bag.has_space(Pocket::Items, ItemId(0), 1));
    assert!(!bag.remove(Pocket::Items, ItemId(0), 0));
    assert!(!bag.has(Pocket::Items, ItemId(0), 1));
    assert_eq!(bag, Bag::new(SIZES));
}

/// `has_space` predicts `add` (Gen3 `CheckBagHasSpace` / `AddBagItem`) over a pseudo-random
/// sequence that fills and empties two small pockets.
#[test]
fn has_space_predicts_add() {
    let mut bag = Bag::new(SIZES);
    let mut x: u32 = 12345;
    for _ in 0..4000 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let item = ItemId(1 + (x >> 16) as u16 % 20);
        let qty = (x >> 8) as u16 % 120;
        let pocket = [Pocket::PokeBalls, Pocket::TmHm][(x >> 28) as usize % 2];
        if x & 3 == 0 {
            // Gen3 removes 0 of anything, had or not.
            let had = bag.has(pocket, item, qty) || qty == 0;
            assert_eq!(bag.remove(pocket, item, qty), had);
        } else {
            let space = bag.has_space(pocket, item, qty);
            let before = bag.clone();
            assert_eq!(
                bag.add(pocket, item, qty),
                space,
                "{item:?} x{qty} in {pocket:?}"
            );
            if !space {
                assert_eq!(bag, before);
            }
        }
        assert_eq!(bag.digest, bag.full_digest());
    }
}
