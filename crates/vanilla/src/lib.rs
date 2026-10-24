//! The vanilla oracle (Emerald parity plan, Task O1): the original **BPEE** (Pokémon Emerald,
//! US) running in mGBA through `emu`, observed from RAM with the decomp's symbols.
//!
//! - [`sym`]: the symbol addresses (a committed BPEE table, or a decomp build's listing);
//! - [`state`]: [`State`] (map, cell, flags, vars, party, bag, money, objects, badges,
//!   respawn) and [`Mode`] (what the game waits for), and `State::semantic` for the shared
//!   `explore::semantic::Semantic`;
//! - [`boot`]: power-on to the first control in the truck, from RAM;
//! - [`drive`]: settling, walking and the battle policy, from RAM; [`intro`]: the truck to
//!   our new game's start;
//! - [`live`]: the spine's steps and the branches' macros on BPEE (Task O2);
//! - [`oracle`]: both spines side by side, the diffs, the `Parity` findings, the branches.
//!
//! Nothing ROM-derived is committed: the ROM comes from `PKR_ROM_US` at run time and is
//! checked against [`BPEE_SHA1`] (the decomp's `rom.sha1`).

pub mod boot;
pub mod drive;
pub mod gates;
pub mod intro;
pub mod live;
pub mod oracle;
pub mod state;
pub mod sym;

pub use state::{Mode, State, Vanilla};

use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

/// SHA-1 of BPEE rev 0, as the decomp's `rom.sha1`: the symbols hold for this ROM only.
pub const BPEE_SHA1: &str = "f3ae088181bf583e55daf962a92bb46f4f1d07b7";

/// The ROM's SHA-1, hex.
pub fn sha1_hex(rom: &[u8]) -> String {
    sha1_smol::Sha1::from(rom).digest().to_string()
}

/// `PKR_ROM_US`, when set.
pub fn rom_from_env() -> Option<PathBuf> {
    std::env::var_os("PKR_ROM_US").map(PathBuf::from)
}

impl Vanilla {
    /// Powers BPEE on (not stepped yet): checks the ROM's SHA-1, opens it as a retail
    /// cartridge (fixed RTC, no rendering) and resolves the symbols ([`sym::Sym::from_env`]). Returns the
    /// symbol notes too.
    pub fn open(rom: &Path) -> Result<(Vanilla, Vec<String>)> {
        let bytes = std::fs::read(rom).with_context(|| format!("read {}", rom.display()))?;
        let sha = sha1_hex(&bytes);
        ensure!(
            sha == BPEE_SHA1,
            "{}: SHA-1 {sha}, not BPEE's {BPEE_SHA1}",
            rom.display()
        );
        let (sym, notes) = sym::Sym::from_env()?;
        let mut emu = emu::Emu::open_retail(rom)?;
        // The oracle reads RAM only: no scanline rendering (`PKR_VANILLA_RENDER` keeps it,
        // for screenshots while debugging).
        if std::env::var_os("PKR_VANILLA_RENDER").is_none() {
            emu.set_frameskip(1 << 20);
        }
        Ok((Vanilla::new(emu, sym), notes))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha1_is_the_reference() {
        assert_eq!(
            super::sha1_hex(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }
}
