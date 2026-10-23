//! The Milestone 6 studio (`docs/plans/2026-10-22-milestone-6-studio.md`): a headless core of
//! pure controllers, plus the Dioxus desktop UI behind the `ui` feature.

// `cfg(pkr_silice)` (`build.rs`, from `content`'s `links` metadata) is the content's flavour.
const _: () = assert!(cfg!(pkr_silice) == matches!(content::FLAVOR, content::Flavor::Silice));

pub mod build; // Task D
pub mod data_view; // Task F
pub mod explorer; // M7 Task S1
pub mod frames; // Wave 0 spike
pub mod keys; // Task G
pub mod map_edit; // SIL1 Task C (the Silice plan)
pub mod map_picker; // M8 Task S
pub mod map_view; // Task E
pub mod native; // Task C
pub mod pane; // Wave 0 spike (the trait below is the contract)
pub mod paths; // Wave 0 spike
pub mod play_view; // Task G
pub mod proc; // M7 Task S1: process groups (build, explorer)
pub mod rom; // Task D
pub mod sprite; // Task E
pub mod text; // Task F
pub mod tree; // Task B
#[cfg(feature = "ui")]
pub mod ui; // Wave 0 skeleton; play/tree (Task G), map/data (Task H)
