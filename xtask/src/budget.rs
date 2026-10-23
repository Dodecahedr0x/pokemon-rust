//! `xtask rom` memory budgets: the ROM size, the ELF's IWRAM and EWRAM statics, and the code and
//! data by symbol prefix ([`sections`]).
//!
//! Gates (Milestone 8 Task M re-baselined them on the full import):
//! - on a full import (more maps than `export::SLICE_MAPS`): the ROM at most 16 MiB (decision
//!   Q4);
//! - always: IWRAM and EWRAM statics at most the full-import [`BASELINE`] + [`GROWTH`] (2 KiB
//!   and 16 KiB);
//! - on the slice extract (fixtures): engine `.text` at most its Wave 0 baseline + 128 KiB; the
//!   ROM is reported, not gated;
//! - on Silice (SIL1 Task D, provisional until SIL1 Wave 2): the ROM at most
//!   [`SILICE_ROM_LIMIT`].
//!
//! The heap and the IWRAM stack are gated at run time, on the debug region (`emu::HEAP_MAX`,
//! `emu::STACK_UNTOUCHED_MIN`, by `cargo xtask parity`).

use anyhow::{Context, Result, bail, ensure};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
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

/// The full-import baseline: Milestone 8 Task M built by `cargo xtask rom` with every map
/// extracted (`cargo xtask extract`, content at opt-level "s"). To re-measure: on the commit to
/// baseline, `cargo xtask extract`, then `cargo xtask rom`, and copy the printed sizes here.
/// The build remaps the checkout, `CARGO_HOME` and sysroot paths (`remap_rustflags` in
/// `main.rs`), so any checkout or worktree gives the same sizes.
///
/// The ROM is printed against it; its gate is [`FULL_ROM_LIMIT`] (module docs). History:
/// Milestone 4 shipped 1875 KiB; Milestone 5 measured 1992712 bytes (IWRAM 596, EWRAM 460);
/// Wave 0's slice build 1761240 (IWRAM 788, EWRAM 460); Task L's full import 9517904; Task M
/// (debug region v3: EWRAM +16; agb's backtrace screen off; one box per script thunk)
/// 9211952 (IWRAM 788, EWRAM 476); after the Task M review (master's Silice battle engine,
/// Task S and SIL1 Wave 0 merged, the panic message printed, agb's IWRAM allocator linked by
/// the stack probe: EWRAM +16) the values below.
pub const BASELINE: Budget = Budget {
    rom: 9_293_736,
    iwram: 788,
    ewram: 492,
};

/// Which content the ROM was built with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Content {
    /// The built-in stub world: no ROM or section gate (the baselines are extracted builds).
    Stub,
    /// The slice extract (`export::SLICE_MAPS`).
    Slice,
    /// A full import.
    Full,
    /// The Silice flavour (SIL1 Task D): the ROM at most [`SILICE_ROM_LIMIT`].
    Silice,
}

/// The Silice ROM limit, provisional (SIL1 Task D): Task D's Silice ROM (the Wave 0 maps, no
/// scene written) measured [`SILICE_ROM_MEASURED`]; the limit, about twice that, leaves room for
/// SIL1 Wave 1's content until Wave 2 re-baselines it on the Prologue. IWRAM and EWRAM keep the
/// shared gates (Task D measured the slice baseline's 788 and 460 bytes).
pub const SILICE_ROM_LIMIT: u64 = 8 << 20;
/// The Silice ROM size `cargo xtask rom --game silice` measured at SIL1 Task D.
pub const SILICE_ROM_MEASURED: u64 = 4_103_320;

/// Engine `.text` of the Wave 0 slice build (`cargo xtask rom`, `engine` row of [`sections`]).
pub const ENGINE_TEXT_BASELINE: u64 = 205_448;
/// Growth of engine `.text` allowed over [`ENGINE_TEXT_BASELINE`] on the slice.
pub const ENGINE_TEXT_GROWTH: u64 = 128 << 10;
/// The full import's ROM limit.
pub const FULL_ROM_LIMIT: u64 = 16 << 20;

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
        if s.address() == 0 || s.size() == 0 {
            continue;
        }
        if IWRAM.contains(&s.address()) {
            b.iwram += s.size();
        } else if EWRAM.contains(&s.address()) {
            b.ewram += s.size();
        }
    }
    ensure!(
        b.iwram > 0 && b.ewram > 0,
        "no IWRAM or EWRAM sections in {} ({b:?}): did the link script change?",
        elf.display()
    );
    Ok(b)
}

/// Symbol buckets of [`sections`], most specific first: a symbol counts under the first
/// prefix its demangled path starts with (`<engine::script::Call as …>::poll` is `engine`); a
/// foreign generic instantiated for one of them (`core::ptr::drop_in_place::<engine::…>`) counts
/// under `other(<prefix>)`.
const PREFIXES: [&str; 5] = [
    "content::scripts",
    "content::data",
    "content::battle",
    "content",
    "engine",
];

/// The demangled path of a Rust symbol, legacy or v0, without its hash
/// (`_ZN6engine5world5World4step17h…E` -> `engine::world::World::step`), or the name as is.
fn demangle(name: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(name))
}

/// The bucket of a demangled symbol path.
fn bucket(path: &str) -> String {
    let path = path.trim_start_matches(['_', '<']);
    for p in PREFIXES {
        if path == p || path.starts_with(&format!("{p}::")) {
            return p.to_string();
        }
    }
    for p in PREFIXES {
        if path.contains(&format!("{p}::")) {
            return format!("other({p})");
        }
    }
    "other".to_string()
}

/// Sizes in `elf`: every allocated section (`section .text`, …), then the `.text` and `.rodata`
/// symbols by bucket (`engine .text`, `content::scripts .rodata`, `other(engine) .text`, …), in
/// name order.
pub fn sections(elf: &Path) -> Result<Vec<(String, u64)>> {
    let data = std::fs::read(elf).with_context(|| format!("read {}", elf.display()))?;
    let file = object::File::parse(&*data).with_context(|| format!("parse {}", elf.display()))?;
    let mut out = std::collections::BTreeMap::<String, u64>::new();
    let mut names = std::collections::BTreeMap::new();
    for s in file.sections() {
        if s.address() == 0 || s.size() == 0 {
            continue;
        }
        let name = s.name().unwrap_or("?").to_string();
        *out.entry(format!("section {name}")).or_default() += s.size();
        names.insert(s.index().0, name);
    }
    for sym in file.symbols() {
        if !matches!(sym.kind(), SymbolKind::Text | SymbolKind::Data) || sym.size() == 0 {
            continue;
        }
        let Some(section) = sym.section_index().and_then(|i| names.get(&i.0)) else {
            continue;
        };
        let kind = if section.starts_with(".text") {
            ".text"
        } else if section.starts_with(".rodata") {
            ".rodata"
        } else {
            continue;
        };
        let path = demangle(sym.name().unwrap_or(""));
        *out.entry(format!("{} {kind}", bucket(&path))).or_default() += sym.size();
    }
    Ok(out.into_iter().collect())
}

/// Prints `b` and the section sizes `secs` against the interim gates (module docs) and fails
/// past them.
pub fn check(b: &Budget, secs: &[(String, u64)], content: Content) -> Result<()> {
    let mut over = Vec::new();
    let rom_limit = match content {
        Content::Full => Some(FULL_ROM_LIMIT),
        Content::Silice => Some(SILICE_ROM_LIMIT),
        Content::Slice | Content::Stub => None,
    };
    match rom_limit {
        Some(limit) => {
            println!(
                "ROM   {:>8} bytes  limit {limit:>8}  ({} bytes left)",
                b.rom,
                limit as i64 - b.rom as i64
            );
            if content == Content::Silice {
                println!(
                    "      (Silice: provisional limit; {SILICE_ROM_MEASURED} bytes at SIL1 Task D, \
                     {:+} since)",
                    b.rom as i64 - SILICE_ROM_MEASURED as i64
                );
            }
            if b.rom > limit {
                over.push(format!("ROM is {} bytes, over {limit}", b.rom));
            }
        }
        None => println!(
            "ROM   {:>8} bytes  (full-import baseline {}; not gated on the {content:?} content)",
            b.rom, BASELINE.rom
        ),
    }
    let rows = [
        ("IWRAM", b.iwram, BASELINE.iwram, GROWTH.iwram),
        ("EWRAM", b.ewram, BASELINE.ewram, GROWTH.ewram),
    ];
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
    for (name, size) in secs {
        println!("      {name:<32} {size:>9}");
    }
    let engine_text = secs
        .iter()
        .find(|(n, _)| n == "engine .text")
        .map_or(0, |&(_, s)| s);
    if content == Content::Slice {
        let limit = ENGINE_TEXT_BASELINE + ENGINE_TEXT_GROWTH;
        println!(
            "engine .text {engine_text} bytes  baseline {ENGINE_TEXT_BASELINE}  limit {limit}  ({} bytes left)",
            limit as i64 - engine_text as i64
        );
        if engine_text > limit {
            over.push(format!("engine .text is {engine_text} bytes, over {ENGINE_TEXT_BASELINE} + {ENGINE_TEXT_GROWTH}"));
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

    fn secs(engine: u64) -> Vec<(String, u64)> {
        vec![("engine .text".to_string(), engine)]
    }

    #[test]
    fn the_baseline_and_its_growth_pass() {
        let at_limit = Budget {
            rom: FULL_ROM_LIMIT,
            iwram: BASELINE.iwram + GROWTH.iwram,
            ewram: BASELINE.ewram + GROWTH.ewram,
        };
        let engine = ENGINE_TEXT_BASELINE + ENGINE_TEXT_GROWTH;
        for content in [Content::Stub, Content::Slice, Content::Full] {
            check(&BASELINE, &secs(ENGINE_TEXT_BASELINE), content).unwrap();
            check(&at_limit, &secs(engine), content).unwrap();
        }
    }

    #[test]
    fn the_rom_is_gated_only_on_a_full_import_and_engine_text_only_on_the_slice() {
        let big = Budget {
            rom: FULL_ROM_LIMIT + 1,
            ..BASELINE
        };
        check(&big, &secs(0), Content::Slice).unwrap();
        check(&big, &secs(0), Content::Stub).unwrap();
        assert!(check(&big, &secs(0), Content::Full).is_err());
        let engine = secs(ENGINE_TEXT_BASELINE + ENGINE_TEXT_GROWTH + 1);
        assert!(check(&BASELINE, &engine, Content::Slice).is_err());
        check(&BASELINE, &engine, Content::Full).unwrap();
        check(&BASELINE, &engine, Content::Stub).unwrap();
    }

    #[test]
    fn the_silice_rom_has_its_own_limit() {
        let at = |rom| Budget { rom, ..BASELINE };
        check(&at(SILICE_ROM_MEASURED), &secs(0), Content::Silice).unwrap();
        check(&at(SILICE_ROM_LIMIT), &secs(0), Content::Silice).unwrap();
        assert!(check(&at(SILICE_ROM_LIMIT + 1), &secs(0), Content::Silice).is_err());
        // Engine `.text` is gated on the slice only.
        let engine = secs(ENGINE_TEXT_BASELINE + ENGINE_TEXT_GROWTH + 1);
        check(&BASELINE, &engine, Content::Silice).unwrap();
    }

    #[test]
    fn ram_overruns_always_fail() {
        for b in [
            Budget {
                iwram: BASELINE.iwram + GROWTH.iwram + 1,
                ..BASELINE
            },
            Budget {
                ewram: BASELINE.ewram + GROWTH.ewram + 1,
                ..BASELINE
            },
        ] {
            for content in [
                Content::Stub,
                Content::Slice,
                Content::Full,
                Content::Silice,
            ] {
                assert!(check(&b, &secs(0), content).is_err(), "{b:?} {content:?}");
            }
        }
    }

    #[test]
    fn symbols_bucket_by_crate_path() {
        let d = demangle("_ZN7content7scripts14LittlerootTown4main17h0123456789abcdefE");
        assert_eq!(d, "content::scripts::LittlerootTown::main");
        assert_eq!(bucket(&d), "content::scripts");
        assert_eq!(
            bucket(&demangle("_ZN6engine5world5World4step17h0123456789abcdefE")),
            "engine"
        );
        let generic = demangle(
            "_ZN61_$LT$engine..script..Call$u20$as$u20$core..future..Future$GT$4poll17h0123456789abcdefE",
        );
        assert_eq!(
            generic,
            "<engine::script::Call as core::future::Future>::poll"
        );
        assert_eq!(bucket(&generic), "engine");
        assert_eq!(
            bucket("core::ptr::drop_in_place::<engine::world::World>"),
            "other(engine)"
        );
        let v0 = demangle("_RNvNtCsa5NKaRXKkCk_7content6battle4BLOB");
        assert_eq!(v0, "content::battle::BLOB");
        assert_eq!(bucket(&v0), "content::battle");
        assert_eq!(bucket("memcpy"), "other");
    }
}
