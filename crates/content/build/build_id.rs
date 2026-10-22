//! `content::BUILD_ID`, the content half of `engine::checkpoint::BuildHash`: what this build
//! script compiles. FNV-1a-64 (`engine/build_support.rs`) over, in order:
//! - every non-test `.rs` of `crates/content/src` and `crates/content/build` (relative paths
//!   `src/…` and `build/…`; `*_tests.rs`, `tests.rs`, `test_*.rs` and `tests/` left out);
//! - with an extracted dir, every file in it (`extracted/…`) except the `previews/` renders, which
//!   the build never reads. That covers `manifest.ron` (format, ROM sha1, the sha1 of every other
//!   exported file) and also hand edits to the editable files, which leave the manifest as is;
//! - in stub mode, the literal `stub` instead.
//!
//! Paths are relative and mtimes are ignored, so the native and GBA builds of one checkout, and
//! two checkouts of one commit with one extract, agree.

use crate::build_support::{TreeHash, is_build_source};
use std::path::Path;

pub fn build_id(content_dir: &Path, extracted: Option<&Path>) -> std::io::Result<u64> {
    let mut h = TreeHash::new();
    h.tree("src/", &content_dir.join("src"), &is_build_source, true)?;
    h.tree("build/", &content_dir.join("build"), &is_build_source, true)?;
    match extracted {
        Some(dir) => h.tree(
            "extracted/",
            dir,
            &|rel: &str| !rel.starts_with("previews/"),
            false,
        )?,
        None => h.write(b"stub"),
    }
    Ok(h.finish())
}
