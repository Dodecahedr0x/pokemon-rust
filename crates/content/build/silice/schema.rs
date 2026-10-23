//! The serde types of every `games/silice/*.ron` file the build loads (SIL1 Wave 0). They are
//! defined once in `extract::games` and shared with the studio's map editor (maps and layouts)
//! and `cargo xtask silice` (everything else); see that module for each file and its contract:
//! - `maps/<Name>.ron`: [`GameMap`] (the layout by name, names for header constants, warp and
//!   connection targets by map name);
//! - `layouts/<Name>.ron`: [`Layout`] (the extract's own);
//! - `ids/<kind>.ron`: [`IdsFile`], append-only with tombstones ([`IdKind`] has the ranges);
//! - `text/fr.ron`: [`TextFile`], read with [`parse_texts`] (a repeated key fails);
//! - `text/speakers.ron`, `battle/{trainers,encounters}.ron`, `items.ron`, `ids/refs.ron`,
//!   `check-exceptions.ron`: [`SpeakersFile`], [`TrainersFile`], [`EncountersFile`],
//!   [`ItemsFile`], [`RefsFile`], [`ExceptionsFile`].
//!
//! `extract` is already a build dependency of `content`, so sharing costs no new crate, and the
//! Emerald build leaves `extract/src/games/` out of its `BUILD_ID`.

#[allow(unused_imports)]
pub use extract::games::*;
#[allow(unused_imports)]
pub use extract::schema::Layout;
