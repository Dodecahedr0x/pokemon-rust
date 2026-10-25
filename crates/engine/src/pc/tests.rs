use super::*;
use crate::data::SpeciesId;

fn mon(species: u16) -> BoxedMon {
    BoxedMon {
        species: SpeciesId(species),
        personality: 1,
        ot_id: 2,
        nickname: [0xFF; 11],
        exp: 100,
        ivs: 0,
        evs: [0; 6],
        moves: [1, 0, 0, 0],
        pp: [35, 0, 0, 0],
        pp_bonuses: 0,
        friendship: 70,
        ability_slot: 0,
        held: None,
        modern_fateful: false,
    }
}

#[test]
fn digest_returns_to_empty() {
    let mut pc = PcStorage::new(50, 14);
    assert!(pc.is_empty());
    pc.set_item(
        3,
        Slot {
            item: ItemId(13),
            qty: 5,
        },
    );
    pc.set_mon(2, 7, Some(mon(25)));
    assert!(!pc.is_empty());
    assert_eq!(pc.mon(2, 7), Some(&mon(25)));
    assert_eq!(pc.boxes().len(), 3);
    pc.set_item(3, Slot::default());
    pc.set_mon(2, 7, None);
    assert!(pc.is_empty());
    // A box never written stays unallocated.
    pc.set_mon(9, 0, None);
    assert_eq!(pc.boxes().len(), 3);
}

/// Gen3 `AddPCItem`: tops up the item's slots to 999, then takes the first free slot; nothing
/// changes when it does not fit.
#[test]
fn add_item_stacks_then_takes_a_free_slot() {
    let mut pc = PcStorage::new(2, 1);
    assert!(pc.add_item(ItemId(13), 1));
    assert_eq!(pc.item_entries().collect::<Vec<_>>(), [(0, ItemId(13), 1)]);
    assert!(pc.add_item(ItemId(13), 998));
    assert_eq!(pc.items()[0].qty, 999);
    assert!(pc.add_item(ItemId(13), 5));
    assert_eq!(pc.item_entries().nth(1), Some((1, ItemId(13), 5)));
    let before = pc.clone();
    assert!(!pc.add_item(ItemId(14), 1));
    assert_eq!(pc, before);
    assert!(pc.add_item(ItemId(13), 994));
    assert_eq!(pc.items()[1].qty, 999);
}
