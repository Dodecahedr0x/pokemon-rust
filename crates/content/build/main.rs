//! Compiles `content/extracted/` (written by `cargo xtask extract`) into `'static` tables in
//! `$OUT_DIR/{consts,data,scripts,battle}.rs` (+ the `data.bin` and `battle.bin` blobs), and `content::BUILD_ID` into
//! `$OUT_DIR/build_id.rs` (`build_id.rs`). Without an extracted dir it builds a small built-in
//! world instead and sets `cfg(pkr_stub)`.
//!
//! **Flavours** (`content::FLAVOR`, SIL1 Wave 0):
//! - the stub (`cfg(pkr_stub)`): no usable extracted dir (`PKR_EXTRACTED_DIR=/nonexistent`, a
//!   fresh clone, or a stale extract), whatever `PKR_GAME` says;
//! - Emerald: `PKR_GAME` unset, empty or `emerald`, with an extract;
//! - Silice (`cfg(pkr_silice)`): `PKR_GAME=silice` with an extract, whose base assets
//!   (tilesets, sprites, font, window, battle data, tables) it compiles with the authored
//!   `games/silice` (`PKR_SILICE_DIR` overrides it, relative to the workspace) and the scripts of
//!   `src/silice/scripts` (`build/silice/`). Without a usable extract the build fails (the
//!   Silice maps name the extract's tilesets, which the stub does not have), unless
//!   `PKR_EXTRACTED_DIR` is set explicitly: then it builds the stub with a warning.
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
// `idents_with_prefix` and `without_line_comments` are used by `tests/build_classify.rs` only.
#[allow(dead_code)]
mod classify;
mod codegen;
#[allow(dead_code)]
mod input;
mod parity;
mod scripts;
mod silice;
mod stub;
mod ui;

use build_support::write_if_changed;
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pkr_stub)");
    println!("cargo::rustc-check-cfg=cfg(pkr_stub_battle)");
    println!("cargo::rustc-check-cfg=cfg(pkr_stale_extract)");
    println!("cargo::rustc-check-cfg=cfg(pkr_silice)");
    println!("cargo::rerun-if-env-changed=PKR_EXTRACTED_DIR");
    println!("cargo::rerun-if-env-changed=PKR_GAME");
    println!("cargo::rerun-if-env-changed=PKR_SILICE_DIR");
    println!(
        "cargo::rerun-if-env-changed={}",
        silice::registry::STRICT_ENV
    );
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.join("../..");
    let want_silice = match std::env::var("PKR_GAME").as_deref() {
        Err(_) | Ok("") | Ok("emerald") => false,
        Ok("silice") => true,
        Ok(other) => panic!("PKR_GAME={other:?}: expected `emerald` (the default) or `silice`"),
    };
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
    let games = match std::env::var_os("PKR_SILICE_DIR") {
        Some(d) => workspace.join(d),
        None => workspace.join("games/silice"),
    };
    let games = std::path::absolute(&games).unwrap();
    let silice = (want_silice && real).then_some(games.as_path());
    if want_silice && !real {
        // Never a silent stub: only an explicit `PKR_EXTRACTED_DIR` (a stub build or test that
        // happens to inherit `PKR_GAME=silice`) builds the stub, with a warning.
        let why = format!(
            "PKR_GAME=silice needs an extract for its base assets ({} has no usable \
             manifest.ron{}): run `cargo xtask extract`",
            dir.display(),
            if stale { ", only an older format" } else { "" }
        );
        if overridden.is_none() {
            panic!("{why}");
        }
        println!("cargo::warning={why}; PKR_EXTRACTED_DIR is set: building the stub world");
    }
    let id = build_id::build_id_for(&manifest_dir, real.then_some(dir.as_path()), silice)
        .unwrap_or_else(|e| panic!("hashing the content build inputs: {e}"));
    // The Silice flavour writes its own script files (`build/silice/dispatch.rs`).
    let mut silice_scripts = None;
    let world = if let Some(games) = silice {
        let dir = dir.canonicalize().unwrap();
        println!("cargo::rerun-if-changed={}", dir.display());
        println!("cargo::rerun-if-changed={}", games.display());
        println!("cargo::rustc-env=PKR_EXTRACTED={}", dir.display());
        println!("cargo::rustc-cfg=pkr_silice");
        let scripts = manifest_dir.join("src/silice/scripts");
        let s = silice::world(&dir, games, &scripts, silice::registry::strict())
            .unwrap_or_else(|e| panic!("building the Silice world: {e:#}"));
        let unwritten = s.unwritten.len();
        if unwritten > 0 {
            input::warn(&format!(
                "silice: {unwritten} script names the maps use are not written yet: dropped \
                 (the Wave 0 allowlist, `build/silice/registry.rs`; listed in content::REPORT; \
                 {}=1 fails on them)",
                silice::registry::STRICT_ENV
            ));
        }
        silice_scripts = Some(
            silice::dispatch::generate(&s.registry, &s.unwritten, &s.report)
                .unwrap_or_else(|e| panic!("generating the Silice scripts: {e:#}")),
        );
        s.world
    } else if real {
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
    // The flavour built, for the dependents' build scripts (`DEP_PKR_CONTENT_FLAVOR`: the shells
    // set their own `cfg(pkr_silice)` from it).
    let flavor = match (silice, real) {
        (Some(_), _) => "silice",
        (None, true) => "emerald",
        (None, false) => "stub",
    };
    println!("cargo::metadata=flavor={flavor}");
    // The non-generated sources, for `content::data::GLOBAL_DIGEST`.
    let sources = codegen::sources_digest_for(&manifest_dir, silice)
        .unwrap_or_else(|e| panic!("hashing the content sources: {e}"));
    let g =
        codegen::generate(&world, sources).unwrap_or_else(|e| panic!("generating content: {e:#}"));
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
    let sc = match silice_scripts {
        Some(sc) => sc,
        None => {
            scripts::generate(&out, &world).unwrap_or_else(|e| panic!("generating scripts: {e:#}"))
        }
    };
    let parity = if silice.is_some() {
        String::new()
    } else {
        parity::rows(&world).unwrap_or_else(|e| panic!("the parity rows: {e:#}"))
    };
    for (name, src) in [
        ("parity.tsv", &parity),
        ("consts.rs", &g.consts),
        ("data.rs", &g.data),
        ("dispatch.rs", &sc.dispatch),
        ("scripts.rs", &sc.rs),
        ("overrides.rs", &sc.overrides),
        ("report.txt", &format!("{}{}", sc.report, g.report)),
        ("battle.rs", &g.battle),
        ("ui.rs", &ui::generate(&world)),
    ] {
        write_if_changed(&out.join(name), src.as_bytes()).unwrap();
    }
    write_if_changed(&out.join("battle.bin"), &g.battle_bin).unwrap();
    write_if_changed(&out.join("data.bin"), &g.data_bin).unwrap();
    write_if_changed(&out.join("build_id.rs"), format!("{id:#018x}\n").as_bytes()).unwrap();
}
