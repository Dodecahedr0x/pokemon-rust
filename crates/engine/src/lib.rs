#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

pub mod scene;
pub use scene::{Content, Scene, Tile};
pub mod debug;
