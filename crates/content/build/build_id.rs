//! `content::BUILD_ID`, the content half of `engine::checkpoint::BuildHash`: what this build
//! script compiles. FNV-1a-64 (`engine/build_support.rs`, never 0) over, in order:
//! - every non-test `.rs` of `crates/content/src`, `crates/content/build` and
//!   `crates/extract/src` (relative paths `src/…`, `build/…`, `extract/…`; `*_tests.rs`,
//!   `tests.rs`, `test_*.rs` and `tests/` left out). `extract` is a build dependency that parses
//!   and transforms the extracted data;
//! - the support tables `build/support/*.ron` (`support/…`, Milestone 8), which classify script
//!   commands and specials;
//! - with an extracted dir, every file in it (`extracted/…`) except the `previews/` renders, which
//!   the build never reads. That covers `manifest.ron` (format, ROM sha1, the sha1 of every other
//!   exported file) and also hand edits to the editable files, which leave the manifest as is;
//! - in stub mode, the literal `stub` instead.
//!
//! Hidden files and dirs (`.DS_Store`, …) and `*~` backups never count; symlinks are followed.
//! Paths are relative and mtimes are ignored, so the native and GBA builds of one checkout, and
//! two checkouts of one commit with one extract, agree.

use crate::build_support::{Content, TreeHash, is_build_source};
use std::path::Path;

pub fn build_id(content_dir: &Path, extracted: Option<&Path>) -> std::io::Result<u64> {
    let mut h = TreeHash::new();
    h.tree(
        "src/",
        &content_dir.join("src"),
        &is_build_source,
        Content::Text,
    )?;
    h.tree(
        "build/",
        &content_dir.join("build"),
        &is_build_source,
        Content::Text,
    )?;
    let support = content_dir.join("build/support");
    if support.is_dir() {
        h.tree(
            "support/",
            &support,
            &|rel: &str| rel.ends_with(".ron"),
            Content::Text,
        )?;
    }
    let extract = content_dir.join("../extract/src");
    h.tree("extract/", &extract, &is_build_source, Content::Text)?;
    match extracted {
        Some(dir) => h.tree(
            "extracted/",
            dir,
            &|rel: &str| !rel.starts_with("previews/"),
            Content::Raw,
        )?,
        None => h.write(b"stub"),
    }
    Ok(h.finish())
}
