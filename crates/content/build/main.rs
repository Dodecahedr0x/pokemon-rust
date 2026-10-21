//! Compiles `content/extracted/` (written by `cargo xtask extract`) into `'static` tables in
//! `$OUT_DIR/{consts,data,scripts}.rs`. Without an extracted dir it builds a small built-in
//! world instead and sets `cfg(pkr_stub)`.
//!
//! `PKR_EXTRACTED_DIR` overrides the extracted dir (tests point it at a fixture, or at a
//! missing dir to force the stub).

mod classify;
mod codegen;
#[allow(dead_code)]
mod input;
mod stub;

use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pkr_stub)");
    println!("cargo::rerun-if-env-changed=PKR_EXTRACTED_DIR");
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let overridden = std::env::var_os("PKR_EXTRACTED_DIR").is_some();
    let dir = match std::env::var_os("PKR_EXTRACTED_DIR") {
        Some(d) => PathBuf::from(d),
        None => manifest_dir.join("../../content/extracted"),
    };
    let dir = std::path::absolute(&dir).unwrap();
    let manifest = dir.join("manifest.ron");
    let world = if manifest.exists() {
        let dir = dir.canonicalize().unwrap();
        println!("cargo::rerun-if-changed={}", dir.display());
        println!("cargo::rustc-env=PKR_EXTRACTED={}", dir.display());
        input::load(&dir).unwrap_or_else(|e| panic!("reading {}: {e:#}", dir.display()))
    } else {
        // Watch for an extract appearing: `content/` when it exists (`cargo xtask extract`
        // creates it). Otherwise watch the missing manifest, which cargo treats as always dirty,
        // so the stub is rebuilt on every build until `content/` exists.
        let parent = dir.parent().filter(|p| !overridden && p.is_dir());
        match parent {
            Some(p) => println!("cargo::rerun-if-changed={}", p.display()),
            None => println!("cargo::rerun-if-changed={}", manifest.display()),
        }
        println!("cargo::rustc-cfg=pkr_stub");
        println!("cargo::rustc-env=PKR_EXTRACTED=");
        stub::world()
    };
    let g = codegen::generate(&world).unwrap_or_else(|e| panic!("generating content: {e:#}"));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for (name, src) in [
        ("consts.rs", &g.consts),
        ("data.rs", &g.data),
        ("scripts.rs", &g.scripts),
    ] {
        let path = out.join(name);
        // Keep the mtime when nothing changed.
        if std::fs::read_to_string(&path).ok().as_ref() != Some(src) {
            std::fs::write(&path, src).unwrap();
        }
    }
}
