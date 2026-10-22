//! Fault-injection `GameData` wrappers for tests (Task E). Each delegates like `Probe`.

use engine::data::{MapId, ScriptId};
use std::cell::Cell;

/// `script()` returns a panicking script for `script`.
pub struct PanicOnScript<G> {
    pub inner: G,
    pub script: ScriptId,
}

/// A leaked `MapData` copy of `map` with the cells around `at` blocked.
pub struct WallAround<G> {
    pub inner: G,
    pub map: MapId,
    pub at: (i16, i16),
}

/// `encounters()` depends on a call counter (round-trip break).
pub struct Drifting<G> {
    pub inner: G,
    pub calls: Cell<u32>,
}

/// A different build id, same behaviour.
pub struct Rebuilt<G> {
    pub inner: G,
    pub build_id: u64,
}
