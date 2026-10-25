//! The decoration catalog (`gDecorations`) from the French Emerald ROM, when one is
//! available. Shop lists are resolved from the same ROM in `scripts.rs`. Neither is
//! written into the extract: a missing ROM leaves the catalog empty and the mart
//! commands reported.

use crate::blob_data::BlobOut;
use anyhow::Result;
use extract::decorations::Decor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn bytes() -> Option<&'static [u8]> {
    static ROM: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    ROM.get_or_init(load).as_deref()
}

pub fn catalog() -> &'static [Decor] {
    static C: OnceLock<Vec<Decor>> = OnceLock::new();
    C.get_or_init(|| {
        bytes()
            .and_then(extract::decorations::table)
            .unwrap_or_default()
    })
    .as_slice()
}

fn load() -> Option<Vec<u8>> {
    println!("cargo::rerun-if-env-changed=PKR_ROM_FR");
    println!("cargo::rerun-if-env-changed=PKR_ROM");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
    let workspace = manifest.join("../..");
    let mut paths = Vec::new();
    for key in ["PKR_ROM_FR", "PKR_ROM"] {
        if let Some(p) = std::env::var_os(key) {
            paths.push(PathBuf::from(p));
        }
    }
    paths.push(workspace.join("rom/emeraude-fr.gba"));
    for p in paths {
        let p = if p.is_absolute() {
            p
        } else {
            workspace.join(p)
        };
        if let Some(bytes) = read_rom(&p) {
            println!("cargo::rerun-if-changed={}", p.display());
            return Some(bytes);
        }
    }
    None
}

fn read_rom(p: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(p).ok()?;
    let emerald = bytes.len() > 0xB0 && matches!(&bytes[0xAC..0xB0], b"BPEF" | b"BPEE");
    if emerald || extract::decorations::table(&bytes).is_some() {
        Some(bytes)
    } else {
        None
    }
}

/// `DECORATIONS`, indexed by decoration id. Empty when no ROM was found (the stub).
pub fn emit(blob: &mut BlobOut, o: &mut String) -> Result<()> {
    let cat = catalog();
    if cat.is_empty() {
        o.push_str("pub static DECORATIONS: [Option<DecorationInfo>; 0] = [];\n");
        return Ok(());
    }
    let mut items = String::new();
    for d in cat {
        if d.id == 0 {
            items.push_str("None,");
            continue;
        }
        use std::fmt::Write as _;
        write!(
            items,
            "Some(DecorationInfo {{ name: {}, price: {}, category: {}, description: {} }}),",
            blob.bytes(&d.name),
            d.price,
            d.category,
            blob.bytes(&d.description)
        )?;
    }
    use std::fmt::Write as _;
    writeln!(
        o,
        "/// `gDecorations` by id (`None`: `DECOR_NONE`).\npub static DECORATIONS: [Option<DecorationInfo>; {}] = [{items}];",
        cat.len()
    )?;
    Ok(())
}
