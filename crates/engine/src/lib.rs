#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

extern crate alloc;

pub mod battle;
pub mod checkpoint;
pub mod data;
pub mod debug;
pub mod field;
pub mod flow;
pub mod grid;
pub mod hash;
pub mod input;
pub mod mon;
pub mod objects;
pub mod rng;
pub mod script;
pub mod starter;
pub mod text;
pub mod trainer;
pub mod video;
pub mod wild;
pub mod world;

pub use data::GameData;
pub use input::Buttons;
pub use rng::Rng;
pub use video::Tile;
pub use world::World;
