//! Reads a Pokémon Emerald ROM (BPEF, or BPEE for decomp-backed tests) into editable content files.
//! Tables are located through anchors and pointer back-tracing, never hard-coded per version.

pub mod battle_data;
pub mod battle_gfx;
pub mod codegen;
pub mod export;
pub mod fixtures;
pub mod font;
pub mod games;
pub mod gfx;
pub mod inc;
pub mod labels;
pub mod lz77;
pub mod maps;
pub mod movement;
pub mod names;
pub mod render;
pub mod rom;
pub mod roots;
pub mod schema;
pub mod scripts;
pub mod sprites;
pub mod tables;
pub mod text;
pub mod window;

pub use rom::{GameCode, ROM_BASE, Rom};
