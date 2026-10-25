//! An e-Reader payload grants the Enigma Berry, and the two specials answer from that state.

use content::battle::STARTERS;
use content::consts::{
    ITEM_ENIGMA_BERRY, SPECIAL_DoesPartyHaveEnigmaBerry, SPECIAL_IsEnigmaBerryValid,
    VAR_ENIGMA_BERRY_AVAILABLE,
};
use content::prelude::{apply_ereader, special};
use engine::data::ItemId;
use engine::ereader::{self, ENIGMA_BERRY_LEN, STATUS_SUCCESS};
use engine::script::{Ctx, GiveMonExtra, Script};
use engine::world::World;
use std::cell::RefCell;
use std::rc::Rc;

const NAME: [u8; 6] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

fn berry() -> [u8; ENIGMA_BERRY_LEN] {
    let mut raw = [0; ENIGMA_BERRY_LEN];
    raw[..6].copy_from_slice(&NAME);
    raw[6] = 0xFF;
    raw[10] = 3;
    raw[20] = 12;
    let sum: u32 = raw[..48].iter().map(|&b| u32::from(b)).sum();
    raw[48..].copy_from_slice(&sum.to_le_bytes());
    raw
}

/// `checkcompat` (offset 0, English, RS, Emerald), `setenigmaberry` of [`berry`], `end`.
fn payload() -> Vec<u8> {
    let raw = berry();
    let mut out = Vec::new();
    out.push(1);
    out.extend(0u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(0x200u32.to_le_bytes());
    // The berry follows `end`, so its bytes are data, not opcodes.
    out.push(7);
    let at = (out.len() + 4 + 1) as u32;
    out.extend(at.to_le_bytes());
    out.push(2);
    assert_eq!(out.len() as u32, at);
    out.extend(raw);
    out
}

#[test]
fn payload_grants_the_enigma_berry_and_the_special_answers() {
    let game = content::Game::for_shells();
    let mut world = World::new(&game);
    let card = payload();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let record = seen.clone();
    let ended = world.run_script(&game, 8, move |ctx: Ctx| -> Script {
        Box::pin(async move {
            let before_valid = special(&ctx, SPECIAL_IsEnigmaBerryValid).await;
            let before_held = special(&ctx, SPECIAL_DoesPartyHaveEnigmaBerry).await;
            let before_var = ctx.var(VAR_ENIGMA_BERRY_AVAILABLE).await;
            let status = apply_ereader(&ctx, &card).await;
            let after_valid = special(&ctx, SPECIAL_IsEnigmaBerryValid).await;
            let after_held = special(&ctx, SPECIAL_DoesPartyHaveEnigmaBerry).await;
            let after_var = ctx.var(VAR_ENIGMA_BERRY_AVAILABLE).await;
            const HELD: GiveMonExtra = GiveMonExtra {
                moves: None,
                item: Some(ItemId(ITEM_ENIGMA_BERRY)),
                ot: None,
                nickname: None,
            };
            ctx.give_mon_with(STARTERS[0], 5, &HELD).await;
            let party = special(&ctx, SPECIAL_DoesPartyHaveEnigmaBerry).await;
            record.borrow_mut().extend([
                before_valid,
                before_held,
                before_var,
                status,
                after_valid,
                after_held,
                after_var,
                party,
            ]);
        })
    });
    assert!(ended, "script did not finish");
    assert_eq!(
        *seen.borrow(),
        [0, 0, 0, STATUS_SUCCESS, 1, 0, 1, 1],
        "before valid/held/var, status, after valid/held/var, held by a party mon"
    );
    assert_eq!(world.var(VAR_ENIGMA_BERRY_AVAILABLE), 1);
    let given = world.party().iter().last().expect("the given mon");
    assert_eq!(given.species, STARTERS[0]);
    assert_eq!(given.held, Some(ItemId(ITEM_ENIGMA_BERRY)));
    assert!(ereader::is_valid(&berry()));
    assert_eq!(&world.str_var(0)[..NAME.len()], &NAME);
}
