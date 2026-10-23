//! The map editor (SIL1 Task C, `docs/plans/2026-10-23-silice-content.md`): authors original
//! maps for a games dir (`games/<game>/{maps,layouts}/<Name>.ron`), painting metatiles of the
//! extract's tilesets referenced by name and placing events.
//!
//! - [`model`]: the document, the edit operations (also the edit script format) and undo/redo,
//!   free of Dioxus and files.
//! - [`schema`]: the games dir files, in one deterministic RON style.
//! - [`names`]: the header constant tables (`MUS_*`, `WEATHER_*`, `MAP_TYPE_*`…).
//! - [`games`]: loading and saving a games dir, and its hygiene check.
//! - [`behavior`]: metatile behaviours and usual collisions (heuristics for authoring).
//! - [`catalog`]: what a map may reference (tilesets, maps, graphics, scripts).
//! - [`validate`]: errors and warnings against a catalog.
//! - [`render`]: pictures of a map and of a metatile palette, drawn by the map view's renderer.
//! - [`stamp`]: reusable cell grids (`stamps/<name>.ron`) and [`resources`], the ops' games
//!   dir environment.
//! - [`overlay`]: labels and overlays over pictures (a built-in bitmap font).
//! - [`batch`]: rebuilding maps from an edits dir in one process.
//! - [`reach`]: the walk graph (the engine's movement rules) and trainer sight;
//!   [`reach_lint`]: the reachability lint over a games dir (`lint-reach`).
//! - [`cli`]: the headless commands (`cargo xtask studio map …`).
//! - [`session`]: the Edit tab's tools over an editor, free of Dioxus.
//! - `ui` (feature `ui`): the studio's Edit tab.

pub mod batch;
pub mod behavior;
pub mod catalog;
pub mod cli;
pub mod games;
pub mod model;
pub mod names;
pub mod overlay;
pub mod reach;
mod reach_cli;
pub mod reach_lint;
pub mod render;
pub mod resources;
pub mod schema;
pub mod session;
pub mod stamp;
#[cfg(feature = "ui")]
pub mod ui;
pub mod validate;

pub use catalog::Catalog;
pub use games::GamesDir;
pub use model::{Brush, Doc, Editor, Event, EventKind, NewMap, Op};
pub use validate::{Issue, Severity, validate};

#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod tests;
