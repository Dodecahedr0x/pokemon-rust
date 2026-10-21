#![cfg_attr(not(test), no_std)]
//! Game-agnostic engine core. Knows nothing about Hoenn.

pub mod hash;
pub mod input;
pub mod rng;

pub use input::Buttons;
pub use rng::Rng;
