//! Locations of the ROMs and the decomp used by ROM-backed tests. Panics with setup help if missing.

use crate::Rom;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn path_from(var: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or(default)
}

fn open(path: PathBuf, help: &str) -> Rom {
    Rom::open(&path).unwrap_or_else(|e| panic!("{help} ({}): {e:#}", path.display()))
}

/// The user's French ROM (BPEF): `$PKR_ROM_FR` or `<repo>/rom/emeraude-fr.gba`.
pub fn fr() -> Rom {
    open(
        path_from("PKR_ROM_FR", root().join("rom/emeraude-fr.gba")),
        "unzip the French ROM to rom/emeraude-fr.gba or set PKR_ROM_FR",
    )
}

/// The US ROM (BPEE) matching the decomp: `$PKR_ROM_US` or `<repo>/../emerald.gba`.
pub fn us() -> Rom {
    open(
        path_from("PKR_ROM_US", root().join("../emerald.gba")),
        "set PKR_ROM_US to the US Emerald ROM",
    )
}

/// The pokeemerald decomp checkout: `$PKR_DECOMP` or `<repo>/../pokeemerald`.
pub fn decomp() -> PathBuf {
    let p = path_from("PKR_DECOMP", root().join("../pokeemerald"));
    assert!(
        p.join("charmap.txt").exists(),
        "set PKR_DECOMP to a pokeemerald checkout ({})",
        p.display()
    );
    p
}
