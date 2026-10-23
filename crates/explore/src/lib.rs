//! The explorer: an authored main path per flavour (the **spine**) plus **branches** off it
//! toward what the spine never touched, each recovering to the spine or aborted when it takes
//! too long for too little novelty. See `docs/plans/2026-10-23-explorer-spine.md`.
//!
//! The play layer is Milestone 7's: macro-actions expanded to inputs by the planner and the
//! policy (`macros`, `path`, `expand`, `policy`), the detectors (`detect`, `softlock`), findings
//! and repro (`findings`). `spine` and `branch` drive it; `db` stores runs (schema v4).

// `cfg(pkr_silice)` (`build.rs`, from `content`'s `links` metadata) is the content's flavour.
const _: () = assert!(cfg!(pkr_silice) == matches!(content::FLAVOR, content::Flavor::Silice));

pub mod balance;
pub mod branch;
pub mod cli;
pub mod config;
pub mod db;
pub mod detect;
pub mod expand;
pub mod findings;
pub mod goals;
pub mod guard;
pub mod key;
pub mod macros;
pub mod model;
pub mod observe;
pub mod oracle;
pub mod path;
pub mod policy;
pub mod runs;
pub mod semantic;
pub mod sim;
pub mod softlock;
pub mod spine;
pub mod store;
#[cfg(feature = "testing")]
#[doc(hidden)]
pub mod testing; // fault-injection GameData wrappers
pub mod workers; // deterministic worker pool

pub use model::*;
