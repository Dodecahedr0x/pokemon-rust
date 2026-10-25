use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{self, Game, mon};

fn party_of(p: Pokemon) -> Party {
    let mut party = Party::default();
    party.push(p).unwrap();
    party
}

fn named(bytes: &[u8]) -> [u8; 8] {
    let mut n = [0xFF; 8];
    n[..bytes.len()].copy_from_slice(bytes);
    n
}

#[test]
fn a_pokemon_block_roundtrips_through_the_substruct_order_and_the_xor() {
    let mut p = mon(
        test_data::FIRE,
        50,
        [40, 30, 20, 25, 15, 10],
        &[test_data::TACKLE],
    );
    p.personality = 7;
    p.ot_id = 0x1111_1111;
    p.friendship = 70;
    let bytes = encode_mon(&p);
    let back = decode_mon(&bytes).unwrap().unwrap();
    assert_eq!(back.species, p.species);
    assert_eq!(back.personality, p.personality);
    assert_eq!(back.ot_id, p.ot_id);
    assert_eq!(back.moves, p.moves);
    assert_eq!(back.pp, p.pp);
    assert_eq!(back.hp, p.hp);
    assert_eq!(back.stats, p.stats);
    assert_eq!(back.level, p.level);
    assert_eq!(back.exp, p.exp);
    assert_eq!(back.nickname[..10], p.nickname[..10]);
    assert!(decode_mon(&[0; MON_BYTES]).unwrap().is_none());
    let mut bad = bytes;
    bad[40] ^= 0xFF;
    assert_eq!(decode_mon(&bad), Err(LinkError::Checksum));
}

#[test]
fn a_player_block_without_the_gamefreak_magic_is_rejected() {
    let side = LinkSide {
        name: named(&[0xC1]),
        trainer_id: 1,
        gender: 0,
        party: Party::default(),
        move_slot: 0,
    };
    let mut block = player_block(&side, 0).to_vec();
    assert!(check_magic(&block).is_ok());
    block[0] = 0;
    assert_eq!(check_magic(&block), Err(LinkError::BadMagic));
}

#[test]
fn two_peers_finish_a_single_link_battle() {
    let local = LinkSide {
        name: named(b"RED"),
        trainer_id: 0x10,
        gender: 0,
        party: party_of(mon(
            test_data::FIRE,
            100,
            [100, 200, 50, 200, 50, 50],
            &[test_data::MEGA_HIT],
        )),
        move_slot: 0,
    };
    let partner = LinkSide {
        name: named(&[0xC1, 0xC2, 0xC3, 0xC4]),
        trainer_id: 0x20,
        gender: 1,
        party: party_of(mon(
            test_data::FIRE,
            100,
            [20, 10, 10, 10, 10, 10],
            &[test_data::TACKLE],
        )),
        move_slot: 0,
    };
    let done = single_battle(&Game, &mut Rng::new(1), &local, &partner).unwrap();
    assert_eq!(done.outcome, BattleOutcome::Won);
    assert_eq!(done.partner_species, test_data::FIRE.0);
    assert!(done.wire.contains(&MASTER_HANDSHAKE));
    assert!(done.wire.contains(&SLAVE_HANDSHAKE));
    assert!(done.wire.contains(&LINKCMD_SEND_LINK_TYPE));
    assert!(done.wire.contains(&LINKTYPE_SINGLE_BATTLE));
    assert!(done.wire.contains(&LINKCMD_INIT_BLOCK));
    assert!(done.wire.contains(&LINKCMD_CONT_BLOCK));
    assert!(done.wire.contains(&LINKCMD_READY_CLOSE_LINK));
    assert_eq!(&done.partner_block[..14], b"GameFreak inc.");
    assert_eq!(done.partner_block[14], 0);
    assert_eq!(&done.partner_block[44..58], b"GameFreak inc.");
    assert_eq!(done.partner_block[58], 0);
    assert_eq!(done.partner_action[0], B_COMM_TO_ENGINE);
    assert_eq!(done.partner_action[8], CONTROLLER_TWORETURNVALUES);
    assert_eq!(done.partner_action[9], B_ACTION_USE_MOVE);
    assert_eq!(done.partner_action[10], 0);
    assert!(done.player.get(0).unwrap().hp > 0);
}

#[test]
fn two_peers_finish_a_union_room_exchange() {
    let name = named(&[0xC1, 0xC2, 0xC3]);
    let local = LinkSide {
        name: named(b"RED"),
        trainer_id: 1,
        gender: 0,
        party: party_of(mon(
            test_data::FIRE,
            10,
            [20, 10, 10, 10, 10, 10],
            &[test_data::TACKLE],
        )),
        move_slot: 0,
    };
    let partner = LinkSide {
        name,
        trainer_id: 2,
        gender: 1,
        party: party_of(mon(
            test_data::FIRE,
            10,
            [20, 10, 10, 10, 10, 10],
            &[test_data::TACKLE],
        )),
        move_slot: 0,
    };
    let done = union_room(&local, &partner).unwrap();
    assert!(done.accepted);
    assert_eq!(done.partner_name, name);
    assert_eq!(done.accept_block[0], UNION_ACCEPT);
    assert_eq!(&done.partner_block[..14], b"GameFreak inc.");
    assert_eq!(done.partner_block[14], 0);
    assert!(done.wire.contains(&MASTER_HANDSHAKE));
    assert!(done.wire.contains(&SLAVE_HANDSHAKE));
    assert!(done.wire.contains(&LINKCMD_INIT_BLOCK));
    assert!(done.wire.contains(&LINKCMD_READY_CLOSE_LINK));
}

#[test]
fn close_link_exchanges_the_ready_word() {
    assert!(ready_close().contains(&LINKCMD_READY_CLOSE_LINK));
}
