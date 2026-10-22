//! The abstract key (Task B). The fold is part of the db schema: changing it is a schema change.
#![allow(unused_variables)] // Wave 0 stub: remove when implemented.

use crate::config::KeyConfig;
use crate::model::{AbstractKey, KeyParts};
use engine::checkpoint::WorldSnapshot;

pub fn key_parts(snap: &WorldSnapshot, cfg: &KeyConfig) -> KeyParts {
    todo!("Task B")
}

/// Stable splitmix fold, platform-independent.
pub fn key(parts: &KeyParts) -> AbstractKey {
    todo!("Task B")
}

/// The key without the cell.
pub fn region(parts: &KeyParts) -> u64 {
    todo!("Task B")
}

/// Content-aware exclusions (temp, special, step-counter vars).
pub fn default_key_config() -> KeyConfig {
    todo!("Task B")
}
