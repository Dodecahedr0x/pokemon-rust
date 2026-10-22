//! Helpers shared by the game-native integration tests.

use std::path::PathBuf;

/// A fresh empty directory of its own (`pkr-<tag>-XXXXXX` in the system temp dir). A passing
/// test removes it; a failing one leaves it to look at.
pub fn temp_root(tag: &str) -> PathBuf {
    tempfile::Builder::new()
        .prefix(&format!("pkr-{tag}-"))
        .tempdir()
        .expect("a temp dir")
        .keep()
}
