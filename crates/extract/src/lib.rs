//! Reads a Pokémon Emerald ROM (BPEF, or BPEE for decomp-backed tests) into editable content files.
//! Tables are located through anchors and pointer back-tracing, never hard-coded per version.

pub mod export;
pub mod fixtures;
pub mod font;
pub mod gfx;
pub mod lz77;
pub mod maps;
pub mod names;
pub mod render;
pub mod rom;
pub mod schema;
pub mod scripts;
pub mod sprites;
pub mod text;

pub use rom::{GameCode, ROM_BASE, Rom};
