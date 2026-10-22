//! Event scripts to async Rust: [`lower`] ([`mod@lower`], Milestone 8 Task D1), run by the
//! content build (`content/build/scripts.rs`, Task D2) on the script IR ([`ScriptFile`]) that
//! the export writes, with the committed support tables ([`Support`]) and the hand-written
//! overrides. The extract writes no Rust: the generated code exists only in the content
//! build's `$OUT_DIR`. The output contract is in [`mod@lower`].

use crate::names;
use crate::schema::{ScriptFile, ScriptIndex};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};

pub mod lower;
pub use lower::NATIVE_COMMANDS;

/// What [`lower`] produced, for the build report and tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    /// Generated functions (roots and label functions).
    pub fns: usize,
    /// Labels, including those that stay blocks of a function.
    pub labels: usize,
    pub texts: usize,
    pub movements: usize,
    /// Movement actions with no `Step` and no `movements.ron` entry, by `MOVEMENT_ACTION_*`
    /// name.
    pub unknown_movement_actions: BTreeMap<String, usize>,
    /// `SPECIAL_*` and `STD_*` names emitted.
    pub specials: BTreeSet<String>,
    pub stds: BTreeSet<String>,
    /// `call` sites into a function that can `end` (Gen3 stops the whole script: the lowering
    /// returns `Flow::End` through the caller).
    pub end_in_callee: usize,
    /// `waitmovement 0` with no `applymovement` earlier in the same block. Per block, not per
    /// path: an `applymovement` in a caller or in an earlier block of the function still counts
    /// the site as unresolved (it runs as `ctx.wait_move(0)`, the engine's last mover, either
    /// way).
    pub unresolved_waitmovement: usize,
    /// Tail-call groups (functions that tail-call each other), each lowered to one shared
    /// `loop { match pc }`.
    pub tail_call_groups: usize,
    /// Command and special sites by [`Class`] name (`Native`, `Ignore`, `Default`,
    /// `Unsupported`).
    pub by_class: BTreeMap<String, usize>,
    /// `Report::Unsupported` sites by command or special name.
    pub reports: BTreeMap<String, usize>,
    /// Percentage of decomp names (`NameOrigin::Decomp`) per kind (`functions`, `texts`,
    /// `movements`), rounded down.
    pub named_decomp_pct: BTreeMap<String, u32>,
}

impl Stats {
    /// One line for the build output: counts, sites by class, the decomp-named share, the
    /// reported sites by frequency (ties by name, at most 8), `end` in callees and unresolved
    /// `waitmovement 0`.
    pub fn summary(&self) -> String {
        let mut reports: Vec<(&String, &usize)> = self.reports.iter().collect();
        reports.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let total: usize = self.reports.values().sum();
        let top: Vec<String> = reports
            .iter()
            .take(8)
            .map(|(k, n)| format!("{k} {n}"))
            .collect();
        let pairs = |m: &dyn Fn() -> Vec<String>| {
            let v = m();
            if v.is_empty() {
                "none".to_string()
            } else {
                v.join(", ")
            }
        };
        format!(
            "scripts: {} fns, {} texts, {} movements; sites {}; decomp names {}; {} unsupported \
             sites ({}); end in callee {}; unresolved waitmovement {}",
            self.fns,
            self.texts,
            self.movements,
            pairs(&|| self
                .by_class
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect()),
            pairs(&|| self
                .named_decomp_pct
                .iter()
                .map(|(k, n)| format!("{k} {n}%"))
                .collect()),
            total,
            pairs(&|| top.clone()),
            self.end_in_callee,
            self.unresolved_waitmovement
        )
    }
}

/// How a script command or special runs (`content/build/support/{commands,specials}.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Class {
    /// Lowered to engine requests or prelude helpers.
    Native,
    /// Has no effect on the engine's state (audio, cosmetic windows): nothing is emitted.
    Ignore,
    /// Writes this value where the command or special writes its result (`VAR_RESULT`), with no
    /// report: the default is the documented behaviour.
    Default(u16),
    /// `ctx.report(Report::Unsupported(name))`, logged; the script continues.
    Unsupported,
}

/// The committed support tables: every command (`scripts::OPS` macro name) and special
/// (`SPECIAL_*`) with its [`Class`], and the movement actions with no engine `Step`. A command or
/// special missing from them fails [`lower`].
///
/// Each table is its own file under `content/build/support/`, a top-level RON map:
/// `commands.ron` (`{ "op": Class }`), `specials.ron` (`{ "SPECIAL_*": Class }`) and
/// `movements.ron` (`{ "MOVEMENT_ACTION_*": MoveFallback }`); [`Support::parse`] reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Support {
    #[serde(default)]
    pub commands: BTreeMap<String, Class>,
    #[serde(default)]
    pub specials: BTreeMap<String, Class>,
    /// `MOVEMENT_ACTION_*` with no `Step` -> what to emit instead (Task D1 addition).
    #[serde(default)]
    pub movements: BTreeMap<String, MoveFallback>,
    /// Whether the prelude has Task H's item helpers (`add_item`, `remove_item`,
    /// `check_item_space`, `check_item_type`, and `give_item`/`find_item`/`check_item` that write
    /// `VAR_RESULT` themselves). With them the item commands and item std scripts lower to bare
    /// helper calls; without them (the Wave 0 prelude) to `ctx.set_var(VAR_RESULT, …)` around
    /// the Wave 0 helpers and requests. Not a table: the content build sets it from the prelude
    /// source (Task D2 addition, removed once H has merged).
    #[serde(skip)]
    pub item_helpers: bool,
}

impl Support {
    /// Reads the sources of `commands.ron` and, if given, `specials.ron` and `movements.ron`.
    pub fn parse(
        commands: &str,
        specials: Option<&str>,
        movements: Option<&str>,
    ) -> Result<Support> {
        Ok(Support {
            commands: ron::from_str(commands).context("commands.ron")?,
            specials: match specials {
                Some(s) => ron::from_str(s).context("specials.ron")?,
                None => BTreeMap::new(),
            },
            movements: match movements {
                Some(s) => ron::from_str(s).context("movements.ron")?,
                None => BTreeMap::new(),
            },
            item_helpers: false,
        })
    }
}

/// A movement action the engine has no `Step` for (`movements.ron`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MoveFallback {
    /// The nearest actions that have a `Step`, in order (a diagonal walk is two straight ones).
    As(Vec<String>),
    /// `Step::Delay(n)`: about the Gen3 action's frame count.
    Delay(u8),
    /// Nothing: an effect the engine does not draw.
    Skip,
}

/// What [`lower`] produces for the content build's `$OUT_DIR`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lowered {
    /// Module -> Rust source for `$OUT_DIR/scripts/<Module>.rs`.
    pub modules: BTreeMap<String, String>,
    /// The static thunk table and `SCRIPT_NAMES` (no giant `match`).
    pub dispatch: String,
    pub stats: Stats,
    /// Keyed by script **name**, never by `ScriptId` (ids shift when scripts are added). Each
    /// digest covers the fn's lowered source, the texts and movements it references, its static
    /// call-graph closure (callees and std scripts) and its override source.
    pub script_digests: BTreeMap<String, u64>,
    /// (script, `TRAINER_*`) of every function that starts with `trainerbattle` (Gen3
    /// `GetTrainerFlagFromScriptPointer`), sorted, for `GameData::trainer_flag_of`.
    pub trainer_scripts: Vec<(String, String)>,
}

/// IR -> Rust (Milestone 8 Task D1). No ROM access. `overrides`: script name -> override source
/// text (digested). The output contract is in [`mod@lower`].
pub fn lower(
    files: &[ScriptFile],
    index: &ScriptIndex,
    support: &Support,
    overrides: &BTreeMap<String, String>,
) -> Result<Lowered> {
    lower::lower(files, index, support, overrides)
}

/// The primary name of `v` in `table`, if it is a valid Rust identifier (the prelude defines a
/// constant for each such name).
pub fn named(table: &names::Table, v: u32) -> Option<&'static str> {
    table.name(v).filter(|n| is_ident(n))
}

pub fn is_ident(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
