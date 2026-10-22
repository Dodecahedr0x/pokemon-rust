//! `xtask rom` memory budgets: ELF section sizes of the GBA build (Milestone 5 Task H).

use anyhow::Result;
use std::path::Path;

/// Static memory of the ROM build, in bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Budget {
    pub rom: u64,
    /// `.iwram*`, plus the `.bss`/`.data` agb's `gba.ld` places in IWRAM.
    pub iwram: u64,
    /// `.ewram*` and `.data`.
    pub ewram: u64,
}

/// Sums the IWRAM and EWRAM sections of the GBA ELF `elf`.
// Wave 0 stub: `rom()` calls it in Task H.
#[allow(dead_code)]
pub fn report(elf: &Path) -> Result<Budget> {
    let _ = elf;
    todo!("Milestone 5 Task H")
}
