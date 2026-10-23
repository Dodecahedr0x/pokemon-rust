// The build script of every shell that links `content` (game-native, game-gba, explore, studio,
// xtask; SIL1 Task D), `include!`d by their `build.rs`: sets `cfg(pkr_silice)` when `content`
// built the Silice flavour, read from its `links` metadata (`cargo::metadata=flavor=…` in
// `content/build/main.rs`, here `DEP_PKR_CONTENT_FLAVOR`). Each shell checks the cfg against
// `content::FLAVOR` at compile time.
//
// Staleness: a `rerun-if-env-changed` cannot watch a `DEP_*` variable (cargo sets it for this
// script only). This script reruns whenever `content`'s build script reruns (on a `PKR_GAME`
// change, among others): cargo fingerprints a build-script run with the runs of its `links`
// dependencies. `rerun-if-changed=build.rs` keeps it from rerunning on every other file change.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pkr_silice)");
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("DEP_PKR_CONTENT_FLAVOR").as_deref() == Ok("silice") {
        println!("cargo::rustc-cfg=pkr_silice");
    }
}
