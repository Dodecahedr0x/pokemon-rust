//! `xtask rom` memory budgets: the ROM size and the ELF's IWRAM and EWRAM statics, gated
//! against the Milestone 5 baseline.

use anyhow::{Context, Result, bail};
use object::{Object, ObjectSection, SectionKind};
use std::path::Path;

/// Static memory of the ROM build, in bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Budget {
    /// The fixed `.gba` file.
    pub rom: u64,
    /// Allocated sections linked into IWRAM (agb's `gba.ld`: `.iwram*` and `.text_iwram*`).
    pub iwram: u64,
    /// Allocated sections linked into EWRAM (`.ewram*`, `.data*` and `.bss*`; the heap is the
    /// rest of EWRAM).
    pub ewram: u64,
}

const IWRAM: core::ops::Range<u64> = 0x0300_0000..0x0300_8000;
const EWRAM: core::ops::Range<u64> = 0x0200_0000..0x0204_0000;

/// The M5 baseline (Wave 1 merge, `02acb5c`, built from the main checkout). Milestone 4 shipped
/// 1875 KiB; the +72 KiB is almost all `.text`: postcard/serde code for the checkpoint DTOs
/// (~31 KB, `Checkpoint::serialize` alone 12 KB), the shell's main loop with the boot, restore
/// and save paths inlined (~16 KB), agb_save and agb's flash driver (~8 KB), `core` sorts
/// instantiated for `BTreeMap` collection (~8 KB), and the engine's snapshot conversions and
/// checks (~8 KB). The ROM size also moves with the checkout path (panic locations).
pub const BASELINE: Budget = Budget {
    rom: 1_995_024,
    iwram: 596,
    ewram: 460,
};

/// Growth allowed over [`BASELINE`].
pub const GROWTH: Budget = Budget {
    rom: 64 << 10,
    iwram: 2 << 10,
    ewram: 16 << 10,
};

/// Reads the ROM size from `rom` and sums the IWRAM and EWRAM sections of the GBA ELF `elf`
/// (sections are classified by their link address, so every `gba.ld` input section counts).
pub fn report(elf: &Path, rom: &Path) -> Result<Budget> {
    let data = std::fs::read(elf).with_context(|| format!("read {}", elf.display()))?;
    let file = object::File::parse(&*data).with_context(|| format!("parse {}", elf.display()))?;
    let mut b = Budget {
        rom: std::fs::metadata(rom)
            .with_context(|| format!("stat {}", rom.display()))?
            .len(),
        ..Budget::default()
    };
    for s in file.sections() {
        if matches!(s.kind(), SectionKind::Metadata | SectionKind::Other) || s.size() == 0 {
            continue;
        }
        if IWRAM.contains(&s.address()) {
            b.iwram += s.size();
        } else if EWRAM.contains(&s.address()) {
            b.ewram += s.size();
        }
    }
    Ok(b)
}

/// Prints `b` against the baseline and fails past the allowed growth.
pub fn check(b: &Budget) -> Result<()> {
    let rows = [
        ("ROM", b.rom, BASELINE.rom, GROWTH.rom),
        ("IWRAM", b.iwram, BASELINE.iwram, GROWTH.iwram),
        ("EWRAM", b.ewram, BASELINE.ewram, GROWTH.ewram),
    ];
    let mut over = Vec::new();
    for (name, now, base, growth) in rows {
        let limit = base + growth;
        println!(
            "{name:<5} {now:>8} bytes  baseline {base:>8}  {:+7}  limit {limit:>8}  ({} bytes left)",
            now as i64 - base as i64,
            limit as i64 - now as i64,
        );
        if now > limit {
            over.push(format!("{name} is {now} bytes, over {base} + {growth}"));
        }
    }
    if !over.is_empty() {
        bail!("budget exceeded: {}", over.join("; "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baseline_and_its_growth_pass() {
        let at_limit = Budget {
            rom: BASELINE.rom + GROWTH.rom,
            iwram: BASELINE.iwram + GROWTH.iwram,
            ewram: BASELINE.ewram + GROWTH.ewram,
        };
        check(&BASELINE).unwrap();
        check(&at_limit).unwrap();
    }

    #[test]
    fn each_overrun_fails() {
        for b in [
            Budget {
                rom: BASELINE.rom + GROWTH.rom + 1,
                ..BASELINE
            },
            Budget {
                iwram: BASELINE.iwram + GROWTH.iwram + 1,
                ..BASELINE
            },
            Budget {
                ewram: BASELINE.ewram + GROWTH.ewram + 1,
                ..BASELINE
            },
        ] {
            assert!(check(&b).is_err(), "{b:?}");
        }
    }
}
