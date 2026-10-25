//! The Pokédex's seen and caught flags (Gen3 `GetSetPokedexFlag`; Milestone 9 Wave 0).
//!
//! Recorded from Wave 0 on, so the Pokédex screen (M13) needs no state change: seen for each
//! opponent at the battle intro and on each opponent switch-in (Gen3
//! `HandleSetPokedexFlag(.., FLAG_SET_SEEN, ..)`), caught on a catch, both on `givemon` and on
//! an in-game trade. Task E adds the evolution scene's own calls.
//!
//! Deviation: Gen3 indexes the flags by national Pokédex number
//! (`SpeciesToNationalPokedexNum`); the engine holds no national numbering, so the bitsets are
//! indexed by species id, and the screen maps species to its order. A bit is set at most once,
//! so the counts are the numbers of set bits.

use crate::data::SpeciesId;
use crate::hash::Fnv64;
use alloc::vec::Vec;

/// Species ids at or past this are ignored (a 256-byte bitset each at most).
pub const MAX_SPECIES: u16 = 2048;

/// What Gen3 `EnableNationalPokedex` writes to `VAR_NATIONAL_DEX`. `IsNationalPokedexEnabled`
/// is this value together with `FLAG_SYS_NATIONAL_DEX`.
pub const NATIONAL_DEX_MAGIC: u16 = 0x302;

/// Which flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DexFlag {
    Seen,
    Caught,
}

/// Seen and caught bitsets over species ids, with an order-independent digest kept on write.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Pokedex {
    seen: Vec<u8>,
    caught: Vec<u8>,
    digest: u64,
}

fn bit(bits: &[u8], i: u16) -> bool {
    bits.get(i as usize / 8)
        .is_some_and(|b| b & (1 << (i % 8)) != 0)
}

fn entry_digest(flag: DexFlag, species: u16) -> u64 {
    let mut h = Fnv64::new();
    h.write(&[flag as u8]);
    h.write_u16(species);
    h.finish()
}

impl Pokedex {
    /// Whether nothing is recorded (the hash and the snapshot leave the Pokédex out).
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty() && self.caught.is_empty()
    }

    pub fn get(&self, flag: DexFlag, species: SpeciesId) -> bool {
        bit(self.bits(flag), species.0)
    }

    fn bits(&self, flag: DexFlag) -> &[u8] {
        match flag {
            DexFlag::Seen => &self.seen,
            DexFlag::Caught => &self.caught,
        }
    }

    /// Sets the flag (Gen3 `FLAG_SET_*`). Species 0 (`SPECIES_NONE`) and ids past
    /// [`MAX_SPECIES`] are ignored.
    pub fn set(&mut self, flag: DexFlag, species: SpeciesId) {
        let i = species.0;
        if i == 0 || i >= MAX_SPECIES || self.get(flag, species) {
            return;
        }
        let bits = match flag {
            DexFlag::Seen => &mut self.seen,
            DexFlag::Caught => &mut self.caught,
        };
        let byte = i as usize / 8;
        if bits.len() <= byte {
            bits.resize(byte + 1, 0);
        }
        bits[byte] |= 1 << (i % 8);
        self.digest = self.digest.wrapping_add(entry_digest(flag, i));
    }

    /// Seen and caught (Gen3 `givemon`, trades).
    pub fn set_caught(&mut self, species: SpeciesId) {
        self.set(DexFlag::Seen, species);
        self.set(DexFlag::Caught, species);
    }

    /// The number of species with the flag set.
    pub fn count(&self, flag: DexFlag) -> u16 {
        self.bits(flag).iter().map(|b| b.count_ones() as u16).sum()
    }

    /// The bitset bytes (bit `i % 8` of byte `i / 8` is species `i`), without trailing zeros.
    pub fn raw(&self, flag: DexFlag) -> &[u8] {
        self.bits(flag)
    }

    /// The Pokédex of two bitsets (checkpoint restore). `Err` for a set bit at species 0 or a
    /// bitset longer than [`MAX_SPECIES`] allows or ending in a zero byte (not what `set`
    /// leaves).
    pub fn from_raw(seen: &[u8], caught: &[u8]) -> Result<Pokedex, &'static str> {
        let mut dex = Pokedex::default();
        for (flag, bits) in [(DexFlag::Seen, seen), (DexFlag::Caught, caught)] {
            if bits.len() > MAX_SPECIES as usize / 8 {
                return Err("pokedex bitset too long");
            }
            if bits.last() == Some(&0) {
                return Err("pokedex bitset with a trailing zero byte");
            }
            if bits.first().is_some_and(|b| b & 1 != 0) {
                return Err("pokedex flag set for species 0");
            }
            for i in 1..(bits.len() * 8) as u16 {
                if bit(bits, i) {
                    dex.set(flag, SpeciesId(i));
                }
            }
        }
        Ok(dex)
    }

    /// Feeds the digest into `h`, only when something is recorded.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        if !self.is_empty() {
            h.write(&[0xA3]);
            h.write(&self.digest.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests;
