//! `gBerries` (`src/berry.c`): each berry's name, yields and growth time, for the berry trees
//! (Wave E). Written to `tables/berries.ron` ([`BerriesFile`]).
//!
//! **Locating** (language-independent bytes): a table of [`BERRY_COUNT`] `struct Berry`
//! ([`BERRY_SIZE`] bytes: `name[7]`, `firmness`, `size` (u16), `maxYield`, `minYield`, two
//! description pointers, `stageDuration`, five flavours, `smoothness`, padding) whose first entry
//! is Cheri's (soft, size 20, yields 3/2, 3 hours, spicy 10, smoothness 25) and whose every entry
//! has ROM description pointers, a firmness of at most 5, `minYield <= maxYield` and a non-zero
//! stage duration. Exactly one such table must exist.

use crate::schema::{BerriesFile, BerryEntry};
use crate::{ROM_BASE, Rom, names};
use anyhow::{Result, bail};

/// `sizeof(struct Berry)`.
pub const BERRY_SIZE: u32 = 28;
/// `ITEM_CHERI_BERRY` .. `ITEM_ENIGMA_BERRY`.
pub const BERRY_COUNT: u32 = 43;
/// `BERRY_NAME_LENGTH + 1`.
const NAME_LEN: u32 = 7;
const FIRMNESS: u32 = 7;
const MAX_YIELD: u32 = 10;
const MIN_YIELD: u32 = 11;
const DESCRIPTION1: u32 = 12;
const DESCRIPTION2: u32 = 16;
const STAGE_DURATION: u32 = 20;
/// `BERRY_FIRMNESS_SUPER_HARD`.
const FIRMNESS_MAX: u8 = 5;
/// Cheri's `firmness` (`BERRY_FIRMNESS_SOFT`), `size`, `maxYield`, `minYield` (from offset 7),
/// then `stageDuration`, the flavours and `smoothness` (from offset 20).
const CHERI_SIZES: [u8; 5] = [2, 20, 0, 3, 2];
const CHERI_TAIL: [u8; 7] = [3, 10, 0, 0, 0, 0, 25];

fn is_rom_ptr(rom: &Rom, v: u32) -> bool {
    v >= ROM_BASE && ((v - ROM_BASE) as usize) < rom.bytes().len()
}

/// Whether entry `i` of a candidate table at `base` is a plausible `struct Berry`.
fn entry_ok(rom: &Rom, base: u32, i: u32) -> bool {
    let at = base + BERRY_SIZE * i;
    let (Ok(firm), Ok(max), Ok(min), Ok(d1), Ok(d2), Ok(stage)) = (
        rom.u8(at + FIRMNESS),
        rom.u8(at + MAX_YIELD),
        rom.u8(at + MIN_YIELD),
        rom.u32(at + DESCRIPTION1),
        rom.u32(at + DESCRIPTION2),
        rom.u8(at + STAGE_DURATION),
    ) else {
        return false;
    };
    firm <= FIRMNESS_MAX && min <= max && stage > 0 && is_rom_ptr(rom, d1) && is_rom_ptr(rom, d2)
}

/// The file offset of `gBerries` (see the module doc).
pub fn locate(rom: &Rom) -> Result<u32> {
    let mut hits = Vec::new();
    for o in rom.find_all(&CHERI_SIZES) {
        let Some(base) = o.checked_sub(FIRMNESS) else {
            continue;
        };
        if base % 4 == 0
            && rom
                .slice(base + STAGE_DURATION, CHERI_TAIL.len())
                .is_ok_and(|t| t == CHERI_TAIL)
            && (0..BERRY_COUNT).all(|i| entry_ok(rom, base, i))
        {
            hits.push(base);
        }
    }
    match hits[..] {
        [b] => Ok(b),
        _ => bail!("gBerries: expected 1 table starting with Cheri's entry, found {hits:#x?}"),
    }
}

/// `tables/berries.ron` from the table at `base`, in `ITEM_*_BERRY` order.
pub fn read(rom: &Rom, base: u32) -> Result<BerriesFile> {
    let first = names::ITEMS
        .value("ITEM_CHERI_BERRY")
        .expect("ITEM_CHERI_BERRY in names");
    let mut berries = Vec::new();
    for i in 0..BERRY_COUNT {
        let at = base + BERRY_SIZE * i;
        let name = rom.slice(at, NAME_LEN as usize)?;
        let end = name.iter().position(|&b| b == 0xFF).unwrap_or(name.len());
        berries.push(BerryEntry {
            item: names::ITEMS.name_or_hex(first + i),
            name: name[..end].to_vec(),
            max_yield: rom.u8(at + MAX_YIELD)?,
            min_yield: rom.u8(at + MIN_YIELD)?,
            stage_duration: rom.u8(at + STAGE_DURATION)?,
        });
    }
    Ok(BerriesFile { berries })
}

/// [`locate`] then [`read`]; also the table's offset.
pub fn extract(rom: &Rom) -> Result<(BerriesFile, u32)> {
    let base = locate(rom)?;
    Ok((read(rom, base)?, base))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ROM with a synthetic `gBerries` at 0x200 (and a decoy Cheri-like entry at 0x104).
    fn rom() -> Rom {
        let mut b = vec![0u8; 0x200 + (BERRY_SIZE * BERRY_COUNT) as usize + 0x40];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        b[0x104 + FIRMNESS as usize..][..5].copy_from_slice(&CHERI_SIZES);
        for i in 0..BERRY_COUNT {
            let at = 0x200 + (BERRY_SIZE * i) as usize;
            b[at..at + 4].copy_from_slice(&[0xBD, 0xC2, 0xBF, 0xFF]);
            b[at + FIRMNESS as usize..][..5].copy_from_slice(&CHERI_SIZES);
            b[at + MAX_YIELD as usize] = 3 + (i % 2) as u8;
            let p = (ROM_BASE + 0x10).to_le_bytes();
            b[at + DESCRIPTION1 as usize..][..4].copy_from_slice(&p);
            b[at + DESCRIPTION2 as usize..][..4].copy_from_slice(&p);
            b[at + STAGE_DURATION as usize..][..7].copy_from_slice(&CHERI_TAIL);
            b[at + STAGE_DURATION as usize] = 3 + i as u8;
        }
        Rom::from_bytes(b).unwrap()
    }

    #[test]
    fn locates_and_reads_the_table() {
        let rom = rom();
        let (file, base) = extract(&rom).unwrap();
        assert_eq!(base, 0x200);
        assert_eq!(file.berries.len(), BERRY_COUNT as usize);
        let cheri = &file.berries[0];
        assert_eq!(cheri.item, "ITEM_CHERI_BERRY");
        assert_eq!(cheri.name, [0xBD, 0xC2, 0xBF]);
        assert_eq!((cheri.max_yield, cheri.min_yield), (3, 2));
        assert_eq!(cheri.stage_duration, 3);
        assert_eq!(file.berries[1].max_yield, 4);
        assert_eq!(file.berries[42].stage_duration, 45);
    }
}
