//! `content::BUILD_ID`, the content half of `engine::checkpoint::BuildHash`: what this build
//! script compiles. FNV-1a-64 (`engine/build_support.rs`, never 0) over, in order:
//! - every non-test `.rs` of `crates/content/src`, `crates/content/build` and
//!   `crates/extract/src` that the active flavour compiles (relative paths `src/…`, `build/…`,
//!   `extract/…`; `*_tests.rs`, `tests.rs`, `test_*.rs` and `tests/` left out). `extract` is a
//!   build dependency that parses and transforms the extracted data. The flavours split the
//!   sources (`codegen::{src_source, build_source, extract_source}`, SIL1 Wave 0): the stub and
//!   Emerald leave out `src/silice/`, `build/silice/` and `extract/src/games/`; Silice leaves out
//!   `src/overrides/` and `src/new_game.rs`;
//! - the support tables `build/support/*.ron` (`support/…`, Milestone 8), which classify script
//!   commands and specials;
//! - Emerald: every file of the extracted dir (`extracted/…`) except the `previews/` renders,
//!   which the build never reads. That covers `manifest.ron` (format, ROM sha1, the sha1 of every
//!   other exported file) and also hand edits to the editable files, which leave the manifest as
//!   is;
//! - Silice: the extracted dir without the Emerald game it does not compile (`maps/`, the script
//!   IR `scripts/`, `previews/`), then the games dir's build inputs (`games/…`,
//!   `codegen::games_source`);
//! - the stub: the literal `stub` instead.
//!
//! Hidden files and dirs (`.DS_Store`, …) and `*~` backups never count; symlinks are followed.
//! Paths are relative and mtimes are ignored, so the native and GBA builds of one checkout, and
//! two checkouts of one commit with one extract, agree.

use crate::build_support::{Content, TreeHash};
use crate::codegen::{build_source, extract_source, games_source, src_source};
use std::path::Path;

/// The stub's (`extracted` `None`) or Emerald's `BUILD_ID`.
#[allow(dead_code)] // used by the tests
pub fn build_id(content_dir: &Path, extracted: Option<&Path>) -> std::io::Result<u64> {
    build_id_for(content_dir, extracted, None)
}

/// [`build_id`] of any flavour: `silice_games` is the Silice games dir (`games/silice`), with
/// the extracted dir its base assets come from.
pub fn build_id_for(
    content_dir: &Path,
    extracted: Option<&Path>,
    silice_games: Option<&Path>,
) -> std::io::Result<u64> {
    let silice = silice_games.is_some();
    let mut h = TreeHash::new();
    h.tree(
        "src/",
        &content_dir.join("src"),
        &|rel: &str| src_source(silice, rel),
        Content::Text,
    )?;
    h.tree(
        "build/",
        &content_dir.join("build"),
        &|rel: &str| build_source(silice, rel),
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
    h.tree(
        "extract/",
        &extract,
        &|rel: &str| extract_source(silice, rel),
        Content::Text,
    )?;
    match extracted {
        Some(dir) => h.tree(
            "extracted/",
            dir,
            &|rel: &str| {
                !rel.starts_with("previews/")
                    && !(silice && (rel.starts_with("maps/") || rel.starts_with("scripts/")))
            },
            Content::Raw,
        )?,
        None => h.write(b"stub"),
    }
    if let Some(games) = silice_games {
        h.tree("games/", games, &games_source, Content::Text)?;
    }
    Ok(h.finish())
}
