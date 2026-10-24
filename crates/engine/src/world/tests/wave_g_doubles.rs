//! Wave G: the multi battle with the in-game partner (Gen3 `DoSpecialTrainerBattle`
//! `SPECIAL_BATTLE_STEVEN`, `FillPartnerParty`) on the synthetic game.

use super::battles::mash;
use super::*;
use crate::battle::BattleOutcome;
use crate::battle::test_data::{NORMAL, TACKLE, WATER};
use crate::mon::{PartnerData, PartnerMon};

/// The synthetic game's partner: three level-30 Water mons with fixed IVs 31 and natures 2, 8
/// and 3 (`sStevenMons`' Brave, Impish and Adamant).
pub(super) static PARTNER: PartnerData = PartnerData {
    trainer: TRAINER,
    ot_id: 61226,
    mons: &[
        partner_mon(2, [0, 252, 252, 0, 6, 0]),
        partner_mon(8, [252, 0, 0, 0, 6, 252]),
        partner_mon(3, [0, 252, 0, 0, 252, 6]),
    ],
};

const fn partner_mon(nature: u8, evs: [u8; 6]) -> PartnerMon {
    PartnerMon {
        species: WATER,
        level: 30,
        iv: 31,
        nature,
        evs,
        moves: [Some(TACKLE), None, None, None],
    }
}

#[test]
fn the_multi_battle_puts_the_partner_beside_the_player_and_takes_its_mons_back() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.give_mon(NORMAL, 40).await;
            ctx.give_mon(NORMAL, 41).await;
            let o = ctx
                .battle_multi((TRAINER, TextId(0)), (TRAINER, TextId(0)))
                .await;
            ctx.set_flag(FLAG_DONE, o == BattleOutcome::Won).await;
        })
    });
    for _ in 0..200 {
        if w.battle().is_some() {
            break;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    let (_, b, _) = w.battle().expect("a battle");
    let d = b.setup().double.expect("a double battle");
    assert_eq!(d.partner, Some(TRAINER));
    assert_eq!(d.second.map(|s| s.0), Some(TRAINER));
    // The player's two mons, then the partner's from slot 2 (`MULTI_PARTY_SIZE` is the cap).
    assert_eq!(d.split[0], 2);
    assert_eq!(b.party_range(crate::battle::Side::Player), 0..2);
    let party = b.party(crate::battle::Side::Player);
    assert_eq!(party.len(), 5);
    for (i, m) in party.iter().skip(2).enumerate() {
        // `FillPartnerParty` without `BUGFIX`: the personality is the index, the OT preset.
        assert_eq!((m.species, m.level), (WATER, 30));
        assert_eq!((m.personality, m.ot_id), (i as u32, 61226));
        assert_eq!(m.evs, PARTNER.mons[i].evs);
        assert_eq!(m.moves[0], Some(TACKLE));
    }
    mash(&mut w, 20_000, |w| !w.script_running());
    assert!(w.flag(FLAG_DONE), "won");
    let levels: Vec<u8> = w.party().iter().map(|m| m.level).collect();
    assert_eq!(
        levels.len(),
        2,
        "the partner's mons left with it: {levels:?}"
    );
    assert!(w.party().iter().all(|m| m.species == NORMAL));
}
