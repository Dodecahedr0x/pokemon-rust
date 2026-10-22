//! Writes `$OUT_DIR/engine_src_hash.rs`, the engine half of `checkpoint::BuildHash`.
//! Wave 0: the constant 0. Milestone 5 Task C replaces it with the FNV-1a-64 of the sources.

fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("engine_src_hash.rs"), "0\n").expect("write engine_src_hash.rs");
    println!("cargo:rerun-if-changed=build.rs");
}
