//! Wave C: `Request::Query`, the reads of the player's state Gen3 specials make.

use super::*;
use crate::battle::test_data::{EMBER, FIRE, NORMAL, WATER};
use crate::pokedex::DexFlag;
use crate::script::{PARTY_SIZE, Query};

/// Runs `q` in a script and returns its answer (through `VAR_RESULT`).
fn ask(w: &mut World, q: Query) -> u16 {
    start(w, move |ctx| {
        Box::pin(async move {
            let v = ctx.query(q).await;
            ctx.set_var(VAR_RESULT, v).await;
        })
    });
    until_done(w, 3);
    w.var(VAR_RESULT)
}

static AMONG: [SpeciesId; 2] = [WATER, FIRE];

#[test]
fn dex_counts_all_species_or_the_listed_ones() {
    let mut w = started();
    w.s.pokedex.set_caught(WATER);
    w.s.pokedex.set(DexFlag::Seen, NORMAL);
    let all_seen = Query::DexCount {
        caught: false,
        among: &[],
    };
    assert_eq!(ask(&mut w, all_seen), 2);
    let caught = Query::DexCount {
        caught: true,
        among: &[],
    };
    assert_eq!(ask(&mut w, caught), 1);
    let listed = Query::DexCount {
        caught: false,
        among: &AMONG,
    };
    assert_eq!(ask(&mut w, listed), 1);
}

#[test]
fn party_reads() {
    let mut w = started();
    assert_eq!(ask(&mut w, Query::PartySpecies(0)), 0);
    assert_eq!(ask(&mut w, Query::PartyMoveSlot(EMBER)), PARTY_SIZE);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(WATER, 5).await;
            ctx.give_mon(FIRE, 5).await;
        })
    });
    until_done(&mut w, 3);
    w.s.party.get_mut(0).unwrap().moves = [None; 4];
    w.s.party.get_mut(1).unwrap().moves[3] = Some(EMBER);
    w.s.party.get_mut(1).unwrap().evs = [10, 0, 0, 0, 0, 255];
    assert_eq!(ask(&mut w, Query::PartySpecies(1)), FIRE.0);
    assert_eq!(ask(&mut w, Query::PartyMoveSlot(EMBER)), 1);
    assert_eq!(ask(&mut w, Query::PartyEvTotal(1)), 265);
    assert_eq!(ask(&mut w, Query::PartyEvTotal(5)), 0);
    w.s.party.get_mut(1).unwrap().friendship = 77;
    w.s.party.get_mut(1).unwrap().hp = 9;
    assert_eq!(ask(&mut w, Query::PartyFriendship(1)), 77);
    assert_eq!(ask(&mut w, Query::PartyHp(1)), 9);
    assert_eq!(ask(&mut w, Query::PartyHp(2)), 0);
    assert_eq!(ask(&mut w, Query::PartyHeldItem(1)), 0);
    w.s.party.get_mut(1).unwrap().held = Some(ItemId(77));
    assert_eq!(ask(&mut w, Query::PartyHeldItem(1)), 77);
    assert_eq!(ask(&mut w, Query::PartyHeldItem(4)), 0);
    w.s.party.get_mut(1).unwrap().personality = 0xABCD_1234;
    assert_eq!(ask(&mut w, Query::PartyPersonalityLow(1)), 0x1234);
}

#[test]
fn player_id_and_pockets() {
    let mut w = started();
    w.s.player_id = 0x1234_5678;
    assert_eq!(ask(&mut w, Query::PlayerIdLow), 0x5678);
    assert_eq!(ask(&mut w, Query::PocketNonEmpty(Pocket::Berries)), 0);
    assert!(w.s.bag.add(Pocket::Berries, ItemId(133), 1));
    assert_eq!(ask(&mut w, Query::PocketNonEmpty(Pocket::Berries)), 1);
    assert_eq!(ask(&mut w, Query::PocketNonEmpty(Pocket::Items)), 0);
}

#[test]
fn metatile_reads_the_map_with_setmetatile_changes() {
    let mut w = started();
    let before = ask(&mut w, Query::Metatile { x: 1, y: 1 });
    let want = (before + 1) & crate::data::BLOCK_METATILE_MASK;
    start(&mut w, move |ctx| {
        Box::pin(async move {
            ctx.set_metatile(1, 1, want, true).await;
        })
    });
    until_done(&mut w, 3);
    assert_eq!(ask(&mut w, Query::Metatile { x: 1, y: 1 }), want);
}
