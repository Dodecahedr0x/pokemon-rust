//! What Silice scripts (`src/silice/scripts/*.rs`) use: `use crate::silice::prelude::*;`.
//!
//! The Gen3 prelude (`crate::prelude`: the engine's script types, every generated constant,
//! `msgbox`, `callstd`, `special`, `trainer_battle`, `give_item`…) plus Silice's own helpers.
//! Wave 0 has none; Task E adds `defeated`, `story`, `fallback(name)` (`Report::Degraded`) and
//! the others the plan lists.

pub use crate::prelude::*;
