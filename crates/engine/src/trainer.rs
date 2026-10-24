//! Trainer parties, as Gen3 `CreateNPCTrainerParty` (`battle_main.c`).

use crate::mon::{
    BattleTables, Ivs, PARTY_SIZE, PartnerData, Party, Pokemon, TrainerData,
    is_shiny_ot_personality,
};
use crate::rng::Rng;

/// Gen3 `MAX_PER_STAT_IVS`.
const MAX_IV: u32 = 31;

/// Gen3 `CreateNPCTrainerParty` (held items ignored):
/// - personality 0x80 (double battle), 0x78 (female trainer) or 0x88, plus `nameHash << 8`,
///   where `nameHash` keeps growing across the party: each mon adds the trainer name's bytes
///   and its species name's bytes (up to EOS)
/// - `fixedIV = iv * 31 / 255`
/// - OT id `random32()` until the mon is not shiny (`OT_ID_RANDOM_NO_SHINY`)
/// - the level-up moveset, replaced by the custom one when given (base PP)
pub fn create_trainer_party(t: &BattleTables, trainer: &TrainerData, rng: &mut Rng) -> Party {
    create_trainer_party_up_to(t, trainer, rng, PARTY_SIZE)
}

/// [`create_trainer_party`] with at most `max` mons (`BATTLE_TYPE_TWO_OPPONENTS` caps each
/// trainer's party at `PARTY_SIZE / 2`).
pub fn create_trainer_party_up_to(
    t: &BattleTables,
    trainer: &TrainerData,
    rng: &mut Rng,
    max: usize,
) -> Party {
    let base: u32 = if trainer.double {
        0x80
    } else if trainer.female {
        0x78
    } else {
        0x88
    };
    let mut name_hash: u32 = 0;
    let mut party = Party::default();
    for mon in trainer.party.iter().take(max.min(PARTY_SIZE)) {
        name_hash = name_hash
            .wrapping_add(byte_sum(trainer.name))
            .wrapping_add(byte_sum(t.species_info(mon.species).name));
        let personality = base.wrapping_add(name_hash << 8);
        let iv = (mon.iv as u32 * MAX_IV / 255) as u8;
        let ot_id = loop {
            let v = rng.random32();
            if !is_shiny_ot_personality(v, personality) {
                break v;
            }
        };
        let mut p = Pokemon::new(
            t,
            mon.species,
            mon.level,
            Ivs::Fixed(iv),
            personality,
            ot_id,
            rng,
        );
        if let Some(moves) = mon.moves {
            for (j, m) in moves.into_iter().enumerate() {
                p.moves[j] = m;
                p.pp[j] = m.map_or(0, |m| t.move_info(m).pp);
            }
        }
        // `SetMonData(MON_DATA_HELD_ITEM)` for a `TrainerMonItem*` party (no draw).
        p.held = mon.item;
        // `take(PARTY_SIZE)` keeps the push in bounds.
        let _ = party.push(p);
    }
    party
}

/// Gen3 `FillPartnerParty(TRAINER_STEVEN_PARTNER)` (`battle_tower.c`), each of at most
/// `MULTI_PARTY_SIZE` mons: `Random32()` until a personality that is not shiny with the
/// partner's OT id and has the mon's nature; then, without `BUGFIX`, `CreateMon` with the
/// personality `i` (the mon's index: the drawn one is lost, so the natures are not the
/// intended ones), the fixed IVs and the preset OT id; the EVs, the moves (base PP) and the
/// stats.
pub fn create_partner_party(t: &BattleTables, p: &PartnerData, rng: &mut Rng) -> Party {
    let mut party = Party::default();
    for (i, m) in p
        .mons
        .iter()
        .take(crate::battle::MULTI_PARTY_SIZE as usize)
        .enumerate()
    {
        loop {
            let j = rng.random32();
            if !is_shiny_ot_personality(p.ot_id, j) && (j % 25) as u8 == m.nature {
                break;
            }
        }
        let mut mon = Pokemon::new(
            t,
            m.species,
            m.level,
            Ivs::Fixed(m.iv),
            i as u32,
            p.ot_id,
            rng,
        );
        mon.evs = m.evs;
        for (k, mv) in m.moves.into_iter().enumerate() {
            mon.moves[k] = mv;
            mon.pp[k] = mv.map_or(0, |mv| t.move_info(mv).pp);
        }
        mon.recalc_stats(t);
        let _ = party.push(mon);
    }
    party
}

/// Sum of a Gen3 string's bytes up to EOS (0xFF).
fn byte_sum(s: &[u8]) -> u32 {
    s.iter()
        .take_while(|&&c| c != 0xFF)
        .map(|&c| c as u32)
        .sum()
}

#[cfg(test)]
mod tests;
