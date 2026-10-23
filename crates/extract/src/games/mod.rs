//! The authored games dirs (`games/<game>/`, SIL1): the one definition of their file formats,
//! shared by the studio's map editor (`studio::map_edit::schema` re-exports [`maps`]), the
//! importer and validator (`xtask/src/silice/schema.rs` re-exports [`data`]) and the content
//! build (`crates/content/build/silice/schema.rs` re-exports both). It lives here because
//! `extract` is the one crate all three already depend on (the content build as a
//! build-dependency), and these formats extend [`crate::schema`].
//!
//! The content's Emerald and stub builds never read it: `crates/content/build/build_id.rs`
//! leaves `extract/src/games/` out of their `BUILD_ID`, so editing it moves only the Silice
//! build's.

pub mod data;
pub mod font;
pub mod game;
pub mod maps;

pub use data::*;
pub use game::*;
pub use maps::*;
