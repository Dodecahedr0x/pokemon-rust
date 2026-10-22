//! Map and table data as byte blobs (owned by Milestone 8 Task E). Wave 0: nothing.

use crate::input::World;
use anyhow::Result;
use std::path::Path;

/// Writes the blobs into `out` (`$OUT_DIR`). Wave 0: writes nothing.
pub fn generate(w: &World, out: &Path) -> Result<()> {
    let _ = (w, out);
    Ok(())
}
