//! Explorer v0 (Milestone 7): a headless, persistent, evolutionary explorer of the native game.
//!
//! It grows a tree of input-only nodes (macro-actions expanded to inputs) in `explore.db`,
//! scores them by novelty and story progress, and reports bugs with a reproducing trace. See
//! `docs/plans/2026-10-22-milestone-7-explorer.md`.
//!
//! Implemented in Wave 0: `model`, `config`, `runs`, `guard`, `observe` and the
//! `db` DDL. Wave 1: A `macros`/`path`/`expand`/`sim`, B `key`/`fitness`/`goals`,
//! C `db`/`cache`/`rebuild`, D `graph`/`select`/`mutate`/`runner`,
//! E `detect`/`softlock`/`findings`/`testing`, F `policy`/`balance`. Wave 2: G `wire`/`cli`.

// `cfg(pkr_silice)` (`build.rs`, from `content`'s `links` metadata) is the content's flavour.
const _: () = assert!(cfg!(pkr_silice) == matches!(content::FLAVOR, content::Flavor::Silice));

pub mod balance; // Task F
pub mod cache; // Task C
pub mod cli; // Task G (Wave 2)
pub mod config; // implemented (W0): Config + defaults + RON load
pub mod db; // Task C (+ schema.sql, implemented W0)
pub mod detect; // Task E
pub mod expand; // Task A
pub mod findings; // Task E
pub mod fitness; // Task B
pub mod goals; // Task B
pub mod graph; // Task D
pub mod guard; // implemented (W0): panic capture
pub mod incremental; // M8 Task N: incremental revalidation
pub mod key; // Task B
pub mod macros; // Task A
pub mod model; // implemented (W0): shared data types
pub mod mutate; // Task D
pub mod observe; // implemented (W0): StepObserver + NoObserver
pub mod path; // Task A
pub mod policy; // Task F
pub mod rebuild; // Task C
pub mod runner; // Task D
pub mod runs; // implemented (W0): run text codec; M8 K: the blob codec
pub mod select; // Task D
pub mod sim; // Task A
pub mod softlock; // Task E
#[cfg(feature = "testing")]
#[doc(hidden)]
pub mod testing; // Task E: fault-injection GameData wrappers
pub mod wire; // Task G (Wave 2): Evaluate/sim/db glue

pub use model::*;
