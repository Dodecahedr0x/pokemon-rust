//! Hand-written script overrides (Milestone 8): a script the lowering cannot express as Gen3
//! does, replaced by hand. The build digests each override's source into the script's
//! `SCRIPT_DIGESTS` entry.
//!
//! Convention: one file per script, `<Name>.rs`, holding `pub async fn <Name>(ctx: Ctx)` (the
//! decomp script name), using `crate::prelude::*`; texts and movements are referred to by their
//! generated ids, never by ROM text or data. Each file is listed in [`OVERRIDES`] by name.

/// The overridden scripts (decomp names). Wave 0: none.
pub static OVERRIDES: &[&str] = &[];
