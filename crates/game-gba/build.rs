//! Sets `cfg(pkr_silice)` when `content` built the Silice flavour (its `links` metadata,
//! `DEP_PKR_CONTENT_FLAVOR`; SIL1 Task D), so this crate can leave out the Emerald-only code
//! (Hoenn constants, `Game::after_starter`) on a Silice build.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pkr_silice)");
    println!("cargo::rerun-if-env-changed=DEP_PKR_CONTENT_FLAVOR");
    if std::env::var("DEP_PKR_CONTENT_FLAVOR").as_deref() == Ok("silice") {
        println!("cargo::rustc-cfg=pkr_silice");
    }
}
