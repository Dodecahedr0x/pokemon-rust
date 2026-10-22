//! Writes `$OUT_DIR/engine_src_hash.rs`, the engine half of `checkpoint::BuildHash`: the
//! FNV-1a-64 of every non-test `src/**/*.rs`, in relative-path order (see `build_support.rs`).
//! Test sources (`tests.rs`, `test_*.rs`, `*_tests.rs`, `tests/`) are left out, and every test
//! module of the engine lives in such a file (`no_hashed_engine_source_holds_an_inline_test_module`
//! enforces it), so editing a test does not change the build identity. Everything else in a
//! hashed file is hashed byte for byte: comments and doc comments, and the few `#[cfg(test)]`
//! items that are not modules (the `mod tests;` lines, test-only helpers such as
//! `TextBox::printed`), so editing those does change it.

#[allow(dead_code)] // the content build script uses the rest
#[path = "build_support.rs"]
mod build_support;

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let hash = build_support::engine_src_hash(&manifest.join("src")).expect("hashing engine src");
    build_support::write_if_changed(
        &out.join("engine_src_hash.rs"),
        format!("{hash:#018x}\n").as_bytes(),
    )
    .expect("write engine_src_hash.rs");
    // A directory is scanned recursively: any file added, removed or edited under `src` reruns.
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build_support.rs");
}
