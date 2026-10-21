#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

extern crate alloc;

pub mod data;
pub mod debug;
pub mod field;
pub mod flow;
pub mod grid;
pub mod hash;
pub mod input;
pub mod rng;
pub mod scene;
pub mod script;
pub mod text;
pub mod video;
pub mod world;

pub use data::GameData;
pub use input::Buttons;
pub use rng::Rng;
pub use scene::{Content, Scene, Tile};
pub use world::World;
