//! The Emerald feature-parity ledger (`docs/plans/2026-10-23-emerald-parity.md`, Task L).
//!
//! - **The universe** ([`decomp`]): every feature the decomp (`PKR_DECOMP`) defines, per
//!   [`Category`], with where it is used (decomp sites and the maps whose scripts, layouts,
//!   trainers or wild mons use it).
//! - **The engine's answer** ([`Engine`]): the caller derives each entry's [`Status`] from the
//!   code that dispatches (the content build's support tables and classifiers, the battle
//!   tables); `xtask/src/ledger.rs` is the real one, the tests use synthetic answers.
//! - **Reach** ([`reach`]): the explorer db (the spine's and the branches' maps, the
//!   `unsupported`/`degraded` findings) and the sweep report.
//! - **Story** ([`story`]): the hand-kept story maps per act and the story-blocking features.
//! - **Owners** ([`owners`]): the milestone task that owns each gap (M9 tasks, M10, …).
//!
//! [`ledger::build`] joins them and ranks the gaps: story-blocking, then on a story map (earlier
//! acts first), then reached by the explorer, then by site count. [`ledger::Ledger::markdown`]
//! and RON are written by `cargo xtask ledger` to `target/parity/ledger.{md,ron}`. Nothing
//! here is ROM-derived: decomp names and counts only.

pub mod decomp;
pub mod ledger;
pub mod lowered;
pub mod owners;
pub mod reach;
pub mod story;

use serde::{Deserialize, Serialize};

/// What kind of feature an entry is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Category {
    /// Script commands (`asm/macros/event.inc` macros that emit a `SCR_OP_*`).
    Command,
    /// `SPECIAL_*` (`data/specials.inc`).
    Special,
    /// `MB_*` metatile behaviours (`include/constants/metatile_behaviors.h`).
    Behavior,
    /// `EFFECT_*` move effects (`include/constants/battle_move_effects.h`).
    MoveEffect,
    /// `ABILITY_*` (`include/constants/abilities.h`).
    Ability,
    /// `HOLD_EFFECT_*` (`include/constants/hold_effects.h`).
    HoldEffect,
    /// Item `fieldUseFunc`s (`src/data/items.h`).
    ItemFieldUse,
    /// Item `battleUseFunc`s (`src/data/items.h`).
    ItemBattleUse,
    /// `FIELD_MOVE_*` (`src/party_menu.c`).
    FieldMove,
    /// `WEATHER_*` (`include/constants/weather.h`).
    Weather,
    /// `BATTLE_TYPE_*` single flags (`include/constants/battle.h`).
    BattleType,
    /// `MAP_TYPE_*` (`include/constants/map_types.h`).
    MapType,
    /// The Gen3 screens (hand-kept list, [`owners::SCREENS`]).
    Screen,
    /// The engine's own `Report::Unsupported`/`Degraded` names (not a decomp universe: what
    /// the engine and the content say they simplify, plus what the explorer saw reported).
    EngineReport,
}

impl Category {
    pub const ALL: [Category; 14] = [
        Category::Command,
        Category::Special,
        Category::Behavior,
        Category::MoveEffect,
        Category::Ability,
        Category::HoldEffect,
        Category::ItemFieldUse,
        Category::ItemBattleUse,
        Category::FieldMove,
        Category::Weather,
        Category::BattleType,
        Category::MapType,
        Category::Screen,
        Category::EngineReport,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Command => "script commands",
            Category::Special => "specials",
            Category::Behavior => "metatile behaviours",
            Category::MoveEffect => "move effects",
            Category::Ability => "abilities",
            Category::HoldEffect => "hold effects",
            Category::ItemFieldUse => "item field uses",
            Category::ItemBattleUse => "item battle uses",
            Category::FieldMove => "field moves",
            Category::Weather => "weathers",
            Category::BattleType => "battle types",
            Category::MapType => "map types",
            Category::Screen => "screens",
            Category::EngineReport => "engine reports",
        }
    }
}

/// The engine's answer for one entry.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Status {
    /// Implemented as Gen3 does.
    Native,
    /// Runs in a simplified way (a fixed answer, a stand-in, a `Degraded` report).
    Degraded,
    /// Dropped with no effect (nothing emitted, a behaviour treated as plain ground).
    Ignored,
    /// Visibly unsupported (a `Report::Unsupported`, a stand-in that blocks).
    Unsupported,
    /// No engine counterpart at all.
    Missing,
    /// The ledger cannot derive it (see the ledger's issues).
    Unknown,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Native => "native",
            Status::Degraded => "degraded",
            Status::Ignored => "ignored",
            Status::Unsupported => "unsupported",
            Status::Missing => "missing",
            Status::Unknown => "unknown",
        }
    }

    pub fn is_gap(self) -> bool {
        self != Status::Native
    }
}

/// [`Engine::answer`]'s result: the status and why.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Answer {
    pub status: Option<Status>,
    pub note: String,
}

impl Answer {
    pub fn new(status: Status, note: impl Into<String>) -> Answer {
        Answer {
            status: Some(status),
            note: note.into(),
        }
    }
}

/// The engine's side: the caller answers each universe entry from the code that dispatches.
pub trait Engine {
    /// The status of `name` in `cat`. `status: None` is [`Status::Unknown`]; the note goes to
    /// the ledger's issues.
    fn answer(&self, cat: Category, name: &str, decomp: &decomp::Decomp) -> Answer;
}
