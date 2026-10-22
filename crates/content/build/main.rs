//! Compiles `content/extracted/` (written by `cargo xtask extract`) into `'static` tables in
//! `$OUT_DIR/{consts,data,scripts,battle}.rs` (+ the `data.bin` and `battle.bin` blobs), and `content::BUILD_ID` into
//! `$OUT_DIR/build_id.rs` (`build_id.rs`). Without an extracted dir it builds a small built-in
//! world instead and sets `cfg(pkr_stub)`.
//!
//! `PKR_EXTRACTED_DIR` overrides the extracted dir (tests point it at a fixture, or at a
//! missing dir to force the stub).

mod battle;
mod battle_load;
mod blob_data;
mod build_id;
#[allow(dead_code)]
#[path = "../../engine/build_support.rs"]
mod build_support;
// `idents_with_prefix` and `without_line_comments` are for the script lowering (Task D2).
#[allow(dead_code)]
mod classify;
mod codegen;
#[allow(dead_code)]
mod input;
mod scripts;
mod stub;

use build_support::write_if_changed;
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pkr_stub)");
    println!("cargo::rustc-check-cfg=cfg(pkr_stub_battle)");
    println!("cargo::rustc-check-cfg=cfg(pkr_stale_extract)");
    println!("cargo::rerun-if-env-changed=PKR_EXTRACTED_DIR");
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.join("../..");
    let overridden = std::env::var_os("PKR_EXTRACTED_DIR");
    // A relative override is relative to the workspace root, like every other repo path.
    let dir = match &overridden {
        Some(d) => workspace.join(d),
        None => workspace.join("content/extracted"),
    };
    let dir = std::path::absolute(&dir).unwrap();
    let manifest = dir.join("manifest.ron");
    // An extract in an older format still builds (as the stub, with a warning), so that
    // `cargo xtask extract`, which links this crate, can run to replace it.
    let stale =
        manifest.exists() && input::format_of(&manifest) != Some(extract::schema::FORMAT_VERSION);
    // `BUILD_ID` hashes the content and extract sources: any edit under them reruns this script.
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=build");
    println!("cargo::rerun-if-changed=../extract/src");
    let real = manifest.exists() && !stale;
    let id = build_id::build_id(&manifest_dir, real.then_some(dir.as_path()))
        .unwrap_or_else(|e| panic!("hashing the content build inputs: {e}"));
    let world = if real {
        let dir = dir.canonicalize().unwrap();
        println!("cargo::rerun-if-changed={}", dir.display());
        println!("cargo::rustc-env=PKR_EXTRACTED={}", dir.display());
        input::load(&dir).unwrap_or_else(|e| panic!("reading {}: {e:#}", dir.display()))
    } else {
        if stale {
            println!(
                "cargo::warning={} is in an older format: building the stub world; run `cargo xtask extract`",
                dir.display()
            );
            println!("cargo::rerun-if-changed={}", manifest.display());
            // `content::STALE_EXTRACT`: the parity and screenshot tools refuse this build.
            println!("cargo::rustc-cfg=pkr_stale_extract");
        } else if overridden.is_some() {
            // An explicit dir without a manifest (`/nonexistent` for the stub): say so. Never
            // watch a missing path: cargo treats one as always dirty, which rebuilt content and
            // everything above it on every stub build. The script reruns when
            // `PKR_EXTRACTED_DIR` changes; when the dir exists, also when a file appears in it.
            println!(
                "cargo::warning=PKR_EXTRACTED_DIR={} has no manifest.ron: building the stub world",
                dir.display()
            );
            if dir.is_dir() {
                println!("cargo::rerun-if-changed={}", dir.display());
            }
        } else {
            // Fresh clone: quietly build the stub. `content/` is tracked (`content/.gitkeep`),
            // and `cargo xtask extract` creates `content/extracted/` in it, so watching it reruns
            // this script exactly when an extract appears.
            let parent = dir.parent().unwrap();
            println!("cargo::rerun-if-changed={}", parent.display());
        }
        println!("cargo::rustc-cfg=pkr_stub");
        println!("cargo::rustc-env=PKR_EXTRACTED=");
        stub::world()
    };
    if world.battle.stub {
        // No `battle/` (the stub, or a test fixture): the stub battle data.
        println!("cargo::rustc-cfg=pkr_stub_battle");
    }
    // The non-generated content sources, for `content::data::GLOBAL_DIGEST`.
    let mut sources = build_support::TreeHash::new();
    sources
        .tree(
            "src/",
            &manifest_dir.join("src"),
            &build_support::is_build_source,
            build_support::Content::Text,
        )
        .unwrap_or_else(|e| panic!("hashing the content sources: {e}"));
    let g = codegen::generate(&world, sources.finish())
        .unwrap_or_else(|e| panic!("generating content: {e:#}"));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let build_dir = manifest_dir.join("build");
    // Every support table, including ones Wave 1 adds (`movements.ron`).
    let support_dir = build_dir.join("support");
    let mut tables: Vec<_> = std::fs::read_dir(&support_dir)
        .unwrap_or_else(|e| panic!("{}: {e}", support_dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    tables.sort();
    println!("cargo::rerun-if-changed={}", support_dir.display());
    for t in &tables {
        println!("cargo::rerun-if-changed={}", t.display());
    }
    let support = scripts::support(&build_dir).unwrap_or_else(|e| panic!("{e:#}"));
    let sc = scripts::generate(real.then_some(dir.as_path()), &out, &world, &support)
        .unwrap_or_else(|e| panic!("generating scripts: {e:#}"));
    for (name, src) in [
        ("consts.rs", &g.consts),
        ("data.rs", &g.data),
        ("dispatch.rs", &sc.dispatch),
        ("scripts.rs", &sc.rs),
        ("report.txt", &format!("{}{}", sc.report, g.report)),
        ("battle.rs", &g.battle),
    ] {
        write_if_changed(&out.join(name), src.as_bytes()).unwrap();
    }
    write_if_changed(&out.join("battle.bin"), &g.battle_bin).unwrap();
    write_if_changed(&out.join("data.bin"), &g.data_bin).unwrap();
    write_if_changed(&out.join("build_id.rs"), format!("{id:#018x}\n").as_bytes()).unwrap();
}
