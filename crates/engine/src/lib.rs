#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

extern crate alloc;

pub mod audio;
pub mod bag;
pub mod battle;
pub mod berry;
pub mod checkpoint;
pub mod data;
pub mod debug;
pub mod field;
pub mod flow;
pub mod fx;
pub mod gates;
pub mod grid;
pub mod hash;
pub mod input;
pub mod item_use;
pub mod menu;
pub mod mon;
pub mod objects;
pub mod options;
pub mod pc;
pub mod pokedex;
pub mod rng;
pub mod script;
pub mod sky;
pub mod starter;
pub mod str_vars;
pub mod text;
pub mod trainer;
pub mod ui;
pub mod video;
pub mod weather;
pub mod wild;
pub mod world;

pub use data::GameData;
pub use input::{Buttons, InputMap};
pub use rng::Rng;
pub use video::Tile;
pub use world::World;
