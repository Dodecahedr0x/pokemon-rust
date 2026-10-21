#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

pub mod debug;
pub mod hash;
pub mod input;
pub mod rng;
pub mod scene;
pub mod world;

pub use input::Buttons;
pub use rng::Rng;
pub use scene::{Content, Scene, Tile};
pub use world::{FrameOut, World};
