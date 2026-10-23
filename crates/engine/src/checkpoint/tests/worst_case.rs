//! Milestone 9 Step 0: the worst-case save, against `MAX_ENCODED` (decision 5).

use super::*;

/// A mon with every field at its widest postcard varint.
fn widest_mon() -> Pokemon {
    Pokemon {
        species: SpeciesId(0xFFFF),
        personality: u32::MAX,
        ot_id: u32::MAX,
        nickname: [0xFE; NAME_LEN],
        level: 100,
        exp: u32::MAX,
        ivs: [31; 6],
        evs: [255; 6],
        moves: [Some(MoveId(0xFFFF)); 4],
        pp: [255; 4],
        pp_bonuses: 255,
        friendship: 255,
        ability_slot: 1,
        status: Status::Sleep(7),
        hp: 0xFFFF,
        stats: [0xFFFF; 6],
        held: Some(ItemId(0xFFFF)),
    }
}

/// The worst-case Emerald save (Gen3 counts: 2,400 flags, 256 vars, the five pockets full, 50
/// PC items, a full party, 16 objects, the string vars at their longest, every Milestone 9
/// field set) with `boxes` full PC boxes.
pub(super) fn worst_case(boxes: u8) -> Checkpoint {
    let mut cp = sample();
    let w = &mut cp.world;
    // Held items travel in `party_held`.
    w.party = vec![
        Pokemon {
            held: None,
            ..widest_mon()
        };
        6
    ];
    w.party_held = vec![Some(ItemId(0xFFFF)); 6];
    w.vars.flags = (0..0x960).map(FlagId).collect();
    w.vars.vars = (0x4000..=0x40FF).map(|v| (VarId(v), 0xFFFF)).collect();
    let pockets = [
        (Pocket::Items, 30),
        (Pocket::KeyItems, 30),
        (Pocket::PokeBalls, 16),
        (Pocket::TmHm, 64),
        (Pocket::Berries, 46),
    ];
    w.bag.slots = (pockets.iter())
        .flat_map(|&(pocket, n)| {
            (0..n).map(move |index| BagSlotSnap {
                pocket,
                index,
                item: ItemId(0xFFFF),
                qty: 99,
            })
        })
        .collect();
    w.money = 999_999;
    w.str_vars = [vec![0xA1; 255], vec![0xA1; 255], vec![0xA1; 255]];
    let mut objs = vec![w.objects.objs[0].clone()];
    objs.extend((1..16).map(|i| object(i, (0x7FFF, 0x7FFF), None)));
    w.objects.objs = objs;
    w.options = Some(crate::options::Options {
        text_speed: crate::options::TextSpeed::Fast,
        ..Default::default()
    });
    // Emerald's species ids 1..=411: 52 bytes per bitset, all set.
    let mut bits = vec![0xFF; 52];
    bits[0] = 0xFE;
    w.pokedex = PokedexSnap {
        seen: bits.clone(),
        caught: bits,
    };
    w.pc = PcSnap {
        items: (0..50)
            .map(|index| PcItemSnap {
                index,
                item: ItemId(0xFFFF),
                qty: 999,
            })
            .collect(),
        mons: (0..boxes)
            .flat_map(|pc_box| {
                (0..30).map(move |slot| PcMonSnap {
                    pc_box,
                    slot,
                    mon: widest_mon().to_boxed(),
                })
            })
            .collect(),
        current_box: boxes.saturating_sub(1),
    };
    w.coins = 9999;
    w.registered_item = Some(ItemId(0xFFFF));
    cp
}

/// Emerald's `content::PC_BOXES` (content is not a dependency of the engine).
const EMERALD_PC_BOXES: u8 = 7;

/// The worst-case save fits `MAX_ENCODED` with Emerald's box count; prints the size for every
/// box count (the numbers of "Wave 0 as built").
#[test]
fn worst_case_save_fits_max_encoded() {
    for boxes in 0..=14 {
        std::println!(
            "worst-case save with {boxes:2} full boxes: {} bytes",
            encoded_len(&worst_case(boxes))
        );
    }
    let per_box = encoded_len(&worst_case(1)) - encoded_len(&worst_case(0));
    std::println!("one full box: {per_box} bytes");
    let cp = worst_case(EMERALD_PC_BOXES);
    let len = encoded_len(&cp);
    assert!(len <= MAX_ENCODED, "{len} > {MAX_ENCODED}");
    assert_eq!(decode(&encode(&cp)), Ok(cp));
}
