//! The screen labels (`gText_*`, `gMenuText_*` by decomp name), item and move descriptions,
//! natures, TM/HM data and in-game trades (`tables/{ui_text,item_text,move_text,natures,tmhm,
//! ingame_trades,item_effects}.ron`). Task X2 owns the file.
//!
//! So far: the TM/HM data (`tables/tmhm.ron`, pulled forward by Tasks K/PT for the teach flow).
//!
//! **Locating** (language-independent bytes; the US decomp is the oracle, `tests/tmhm.rs`):
//! - `sTMHMMoves` (`src/data/party_menu.h`, `FOREACH_TMHM` order): 58 `u16` moves whose last
//!   eight are the HM moves (Cut, Fly, Surf, Strength, Flash, Rock Smash, Waterfall, Dive),
//!   the fifty before them distinct valid moves, and the table referenced by a pointer.
//! - `gTMHMLearnsets` (`src/data/pokemon/tmhm_learnsets.h`): `NUM_SPECIES` 8-byte bitsets (bit
//!   `n` = the `n`th TM/HM) with `SPECIES_NONE` and the old Unown (`SPECIES_OLD_UNOWN_B..Z`)
//!   empty, no bit above the 58th, most species with some bit, referenced by a pointer.

use crate::schema::TmHmFile;
use crate::{Rom, names};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// `NUM_TECHNICAL_MACHINES` + `NUM_HIDDEN_MACHINES`.
pub const TMHM_COUNT: usize = 58;
/// `NUM_TECHNICAL_MACHINES`.
pub const TM_COUNT: usize = 50;
/// The HM moves, HM01..HM08 (`FOREACH_HM`).
pub const HM_MOVES: [&str; 8] = [
    "MOVE_CUT",
    "MOVE_FLY",
    "MOVE_SURF",
    "MOVE_STRENGTH",
    "MOVE_FLASH",
    "MOVE_ROCK_SMASH",
    "MOVE_WATERFALL",
    "MOVE_DIVE",
];
/// The least number of species with a TM/HM bit in `gTMHMLearnsets` (Emerald: 372).
const MIN_LEARNING_SPECIES: usize = 300;
/// `SPECIES_OLD_UNOWN_B` .. `SPECIES_OLD_UNOWN_Z`.
const OLD_UNOWN: core::ops::RangeInclusive<usize> = 252..=276;

/// `NUM_SPECIES`.
pub fn species_count() -> usize {
    crate::battle_data::counts().species as usize
}

fn value(t: &names::Table, name: &str) -> Result<u32> {
    t.value(name)
        .with_context(|| format!("{name} is not in names"))
}

/// File offsets of `sTMHMMoves` and `gTMHMLearnsets`.
pub fn locate_tmhm(rom: &Rom) -> Result<(u32, u32)> {
    let mut hms = Vec::with_capacity(16);
    for n in HM_MOVES {
        hms.extend_from_slice(&(value(&names::MOVES, n)? as u16).to_le_bytes());
    }
    let move_count = crate::battle_data::counts().moves as u16;
    let moves: Vec<u32> = rom
        .find_all(&hms)
        .into_iter()
        .filter_map(|h| h.checked_sub(2 * TM_COUNT as u32))
        .filter(|&at| {
            rom.u16s(at, TM_COUNT).is_ok_and(|tms| {
                let distinct: std::collections::BTreeSet<_> = tms.iter().collect();
                distinct.len() == TM_COUNT && tms.iter().all(|&m| m != 0 && m < move_count)
            }) && !rom.pointers_to(at).is_empty()
        })
        .collect();
    let [moves] = moves[..] else {
        bail!("sTMHMMoves: {} referenced candidates", moves.len());
    };
    let learnsets = locate_learnsets(rom)?;
    Ok((moves, learnsets))
}

fn locate_learnsets(rom: &Rom) -> Result<u32> {
    let n = species_count();
    let b = rom.bytes();
    let word = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    let mut found = Vec::new();
    for off in (0..b.len().saturating_sub(n * 8)).step_by(4) {
        if word(off) != 0 || word(off + 8) == 0 {
            continue;
        }
        let e = |s: usize| word(off + 8 * s);
        if (0..n).any(|s| e(s) >> TMHM_COUNT != 0) || OLD_UNOWN.clone().any(|s| e(s) != 0) {
            continue;
        }
        if (0..n).filter(|&s| e(s) != 0).count() < MIN_LEARNING_SPECIES {
            continue;
        }
        if !rom.pointers_to(off as u32).is_empty() {
            found.push(off as u32);
        }
    }
    let [at] = found[..] else {
        bail!("gTMHMLearnsets: {} referenced candidates", found.len());
    };
    Ok(at)
}

/// `tables/tmhm.ron` from the tables at `moves` and `learnsets`: `(ITEM_TMnn / ITEM_HMnn,
/// MOVE_*)` in TM order, and each species with a bit, by `SPECIES_*`.
pub fn read_tmhm(rom: &Rom, moves: u32, learnsets: u32) -> Result<TmHmFile> {
    let tm01 = value(&names::ITEMS, "ITEM_TM01")?;
    let ids = rom.u16s(moves, TMHM_COUNT)?;
    let moves = ids
        .iter()
        .enumerate()
        .map(|(i, &m)| {
            let item = names::ITEMS
                .name(tm01 + i as u32)
                .with_context(|| format!("TM/HM item {i}"))?;
            ensure!(
                item.starts_with("ITEM_TM") || item.starts_with("ITEM_HM"),
                "item {item} where TM/HM {i} is expected"
            );
            let mv = names::MOVES
                .name(m as u32)
                .with_context(|| format!("move {m} has no decomp name"))?;
            Ok((item.to_string(), mv.to_string()))
        })
        .collect::<Result<_>>()?;
    let mut sets = BTreeMap::new();
    for s in 0..species_count() {
        let at = learnsets + 8 * s as u32;
        let bits = u64::from_le_bytes(rom.slice(at, 8)?.try_into().unwrap());
        if bits != 0 {
            let name = names::SPECIES
                .name(s as u32)
                .with_context(|| format!("species {s} has no decomp name"))?;
            sets.insert(name.to_string(), bits);
        }
    }
    Ok(TmHmFile {
        moves,
        learnsets: sets,
    })
}

/// [`locate_tmhm`] then [`read_tmhm`].
pub fn extract_tmhm(rom: &Rom) -> Result<(TmHmFile, u32, u32)> {
    let (moves, learnsets) = locate_tmhm(rom)?;
    Ok((read_tmhm(rom, moves, learnsets)?, moves, learnsets))
}
