//! `games/silice/text/fr.ron` (Task A encodes and merges it). The loader contract of SIL1 Task
//! B: the file is read entry by entry and a repeated key fails, naming it
//! (`extract::games::parse_texts`). The committed import still repeats two ids, so Wave 0 does
//! not load the texts into the build yet.

use anyhow::{Context, Result};
use std::path::Path;

/// `(id, text)` in file order; an error on a repeated id.
pub fn read(path: &Path) -> Result<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    super::schema::parse_texts(&text).with_context(|| path.display().to_string())
}
