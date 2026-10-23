//! On-disk formats of the authored Silice data under `games/silice/` that `cargo xtask silice
//! import` writes and `cargo xtask silice check` reads (SIL1 Task B). The module is
//! self-contained (serde plus `extract::schema`), so SIL1 Wave 0 can move it to
//! `crates/content/build/silice/schema.rs` as is. Every file is written with
//! `extract::export::to_ron`, the extract's RON style.
//!
//! ```text
//! games/silice/
//!   text/fr.ron              TextFile: text id -> FR text in decomp `.string` syntax (`\n`, `\l`,
//!                            `\p`, `{PLAYER}`), keys sorted. The extract's `TextFile`; see
//!                            "Duplicate keys" below.
//!   text/speakers.ron        SpeakersFile: per text id, the chapter it came from and the
//!                            speaker header of its block (documentation only: Gen3 shows no
//!                            speaker name, the text carries it when it matters).
//!   battle/trainers.ron      TrainersFile: the extract's `TrainersFile` plus `items` (the AI's
//!                            item bag) and `notes` per trainer. An extract reader loads it (the
//!                            extra fields are ignored).
//!   battle/encounters.ron    EncountersFile: the extract's v5 `EncountersFile` (map name -> Vec
//!                            of headers) plus `notes` per header.
//!   items.ron                ItemsFile: the `ITEM_SIL_*` definitions, the item placements of the
//!                            chapters' §7 tables, and the marts.
//!   ids/<kind>.ron           IdsFile: append-only number allocation per kind (`flags`, `vars`,
//!                            `trainers`, `items`, `species`, `moves`, `music`, `sounds`, `maps`).
//!   ids/refs.ron             RefsFile: what the chapters' scenes, maps and tables reference
//!                            (import-time snapshot of the pseudo-scripts, until they are Rust).
//!   check-exceptions.ron     ExceptionsFile: known issues `check` lists instead of failing on.
//! ```
//!
//! **Id allocation (the Wave 0 contract).** Each `ids/<kind>.ron` lists `(name, num)` pairs in
//! allocation order. The first import hands out numbers from the kind's `base` in name order. Later
//! imports never renumber: existing entries are kept as they are, new names are appended after the
//! highest number, and a name the source no longer uses stays (the import report lists it). A
//! removed name becomes a tombstone (`removed: true`) and keeps its number forever, so saves and
//! checkpoints stay valid. Bases and limits are in [`IdKind`].
//!
//! **Duplicate keys.** RON maps may repeat a key, and serde keeps the last value. The importer
//! writes a text id defined twice in the source twice (in chapter order) so that the duplicate is
//! visible in the canonical data; `check` reports it (rule `text-duplicate`) and the loader must
//! refuse it until it is resolved.
//!
//! **Map names.** A map constant `MAP_<WORDS>` names the map file `maps/<Name>.ron` by the Emerald
//! convention (`MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F` is `LittlerootTown_BrendansHouse_1F`): the
//! longest proper prefix that is itself a map is the parent, `<Parent>_<Rest>`; words are
//! capitalized and joined; a floor word (`1F`, `B1F`) is preceded by `_`. So `MAP_BRICO_BOURG_PLAYER_HOUSE_1F`
//! is `BricoBourg_PlayerHouse_1F` and `MAP_ROUTE_S01` is `RouteS01`. `ids/maps.ron` records the
//! name of each constant, so the rule is applied once, at import.

use extract::schema::{ItemPocket, LandEncounters, TrainerMon};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `text/fr.ron` (keys sorted; a duplicated key is written once per definition). The game
/// loader reads it as this map; `silice check` reads it as pairs to see duplicates.
#[allow(dead_code)]
pub type TextFile = extract::schema::TextFile;

/// `text/speakers.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SpeakersFile {
    /// Sorted by id, then chapter.
    pub texts: Vec<TextSource>,
}

/// Where one text came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TextSource {
    pub id: String,
    /// The chapter file stem (`00-prologue`).
    pub chapter: String,
    /// The speaker header (`MATHIEU`, `PANNEAU`, `NARRATION`), when the block has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    /// A parenthetical note after the speaker (`(près du portail sud)`), without parentheses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `battle/trainers.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct TrainersFile {
    pub trainers: Vec<Trainer>,
}

/// The extract's `Trainer` plus the item bag and the author's notes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Trainer {
    /// `TRAINER_SIL_*`.
    pub id: String,
    /// `TRAINER_CLASS_*` (an Emerald class as a placeholder).
    pub class: String,
    /// Displayed class name, decomp `.string` syntax, at most 12 bytes encoded.
    pub class_name: String,
    /// At most 10 bytes encoded (`TRAINER_NAME_LENGTH`).
    pub name: String,
    /// `TRAINER_PIC_*` (an Emerald pic as a placeholder until original art exists).
    pub pic: String,
    pub female: bool,
    /// `AI_SCRIPT_*`.
    pub ai: Vec<String>,
    pub double: bool,
    /// 1 to 6 mons; `item` is the held item (`Some("ITEM_NONE")` for none, as the extract writes
    /// it for held-item layouts), `moves` 4 `MOVE_*` or `None` for the level-up moves.
    pub party: Vec<TrainerMon>,
    /// The AI's item bag (`gTrainers[].items`, at most 4 `ITEM_*`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    /// The `//` comments of the source entry, in order (placeholders, replaced moves).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// `battle/encounters.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EncountersFile {
    /// By map name (see "Map names"), each map's headers (Silice maps have one).
    pub maps: BTreeMap<String, Vec<EncounterHeader>>,
}

/// The extract's `EncounterHeader` plus notes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EncounterHeader {
    /// 12 slots.
    pub land: Option<LandEncounters>,
    /// 5 slots.
    pub water: Option<LandEncounters>,
    /// 5 slots.
    pub rock_smash: Option<LandEncounters>,
    /// 10 slots: old rod 0-1, good rod 2-4, super rod 5-9.
    pub fishing: Option<LandEncounters>,
    /// The `//` comments of the source entry, in order (FR species names, percentages).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// `items.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ItemsFile {
    /// `ITEM_SIL_*` definitions, sorted by id. Their numbers are in `ids/items.ron`.
    pub items: Vec<ItemDef>,
    /// The chapters' §7 item tables, in chapter order.
    pub placements: Vec<ItemPlacement>,
    /// Shops whose stock is given as `ITEM_*` with a price, in chapter order.
    pub marts: Vec<Mart>,
}

/// One invented item. Fields the source does not give are `None` (the import report lists them).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemDef {
    /// `ITEM_SIL_*`.
    pub id: String,
    /// FR name as written in the source (`Mallette`), not yet in the Gen3 bag style.
    pub name: Option<String>,
    /// Bag pocket: `KeyItems` when the source calls it a key item, otherwise unset.
    pub pocket: Option<ItemPocket>,
    /// Price from a mart table.
    pub price: Option<u16>,
    /// The Gen3 item it is built on (`Base : ITEM_*`, `Objet Gen3 à cloner`).
    pub clone_of: Option<String>,
    /// The effect as the source describes it.
    pub effect: Option<String>,
    /// Chapters that use the item.
    pub chapters: Vec<String>,
}

/// One row of a §7 item table.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemPlacement {
    pub chapter: String,
    /// The flag set when it is taken (`None` for "—").
    pub flag: Option<String>,
    /// `MAP_*`.
    pub map: String,
    /// Where, as written; `pos` is its `(x,y)` when it gives exactly one.
    pub place: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<(u16, u16)>,
    /// FR name as written, without the quantity.
    pub name: String,
    /// `ITEM_*` (`None` when the row gives none, e.g. running shoes as a flag).
    pub item: Option<String>,
    pub quantity: u8,
    pub method: PlacementMethod,
    /// The method cell as written (`Ball (coffre)`).
    pub method_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// How an item is obtained (conventions §6: Ball, Caché, Donné, Récompense, Boutique).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementMethod {
    Ball,
    Hidden,
    Given,
    Reward,
    Shop,
    Other,
}

/// A shop.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Mart {
    pub chapter: String,
    /// The label the source gives (heading or bold line).
    pub label: String,
    /// The `MAP_*` the label names, if any.
    pub map: Option<String>,
    pub currency: Currency,
    pub stock: Vec<MartItem>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    Money,
    /// Casino coins.
    Coins,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MartItem {
    pub item: String,
    pub price: u32,
}

/// `ids/<kind>.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IdsFile {
    /// The first number this kind hands out.
    pub base: u32,
    /// In allocation order.
    pub ids: Vec<IdEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IdEntry {
    pub name: String,
    pub num: u32,
    /// For maps: the map file name (see "Map names").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    /// A tombstone: the name is gone but its number is never reused.
    #[serde(default, skip_serializing_if = "is_false")]
    pub removed: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The id kinds Silice allocates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdKind {
    Flags,
    Vars,
    Trainers,
    Items,
    Species,
    Moves,
    Music,
    Sounds,
    Maps,
}

impl IdKind {
    pub const ALL: [IdKind; 9] = [
        IdKind::Flags,
        IdKind::Vars,
        IdKind::Trainers,
        IdKind::Items,
        IdKind::Species,
        IdKind::Moves,
        IdKind::Music,
        IdKind::Sounds,
        IdKind::Maps,
    ];

    /// The file stem under `ids/`.
    pub fn file(self) -> &'static str {
        match self {
            IdKind::Flags => "flags",
            IdKind::Vars => "vars",
            IdKind::Trainers => "trainers",
            IdKind::Items => "items",
            IdKind::Species => "species",
            IdKind::Moves => "moves",
            IdKind::Music => "music",
            IdKind::Sounds => "sounds",
            IdKind::Maps => "maps",
        }
    }

    /// The prefix every Silice name of this kind has (`None` for maps and species forms, which
    /// follow the Gen3 naming: maps live in Silice's own namespace, forms are `SPECIES_<BASE>_MEGA`).
    pub fn sil_prefix(self) -> Option<&'static str> {
        match self {
            IdKind::Flags => Some("FLAG_SIL_"),
            IdKind::Vars => Some("VAR_SIL_"),
            IdKind::Trainers => Some("TRAINER_SIL_"),
            IdKind::Items => Some("ITEM_SIL_"),
            IdKind::Moves => Some("MOVE_SIL_"),
            IdKind::Music => Some("MUS_SIL_"),
            IdKind::Sounds => Some("SE_SIL_"),
            IdKind::Species | IdKind::Maps => None,
        }
    }

    /// The first number, and the last one allowed.
    ///
    /// - flags: 0x20 (after the temp flags 0x00-0x1F) to 0x4FF (trainer flags start at 0x500);
    /// - vars: 0x4020 (after the temp vars and `VAR_OBJ_GFX_ID_*`) to 0x40FF (the saved vars);
    /// - trainers: 1 to 863, so `0x500 + id` stays in the trainer-flag range (0x500-0x85F);
    /// - items, species, moves, music and sounds: after the last Gen3 number of the decomp table
    ///   (`extract::names`, `charmap::SOUNDS`), up to 0xFFFF;
    /// - maps: 0 to 0xFEFF (0x7F7F `MAP_DYNAMIC` is skipped; 0xFF00-0xFFFF are reserved for
    ///   `MAP_UNDEFINED` and the studio's previews).
    pub fn range(self) -> (u32, u32) {
        let after =
            |t: &extract::names::Table| t.entries.iter().map(|&(v, _)| v).max().unwrap_or(0) + 1;
        match self {
            IdKind::Flags => (0x20, 0x4FF),
            IdKind::Vars => (0x4020, 0x40FF),
            IdKind::Trainers => (1, 863),
            IdKind::Items => (after(&extract::names::ITEMS), 0xFFFF),
            IdKind::Species => (after(&extract::names::SPECIES), 0xFFFF),
            IdKind::Moves => (after(&extract::names::MOVES), 0xFFFF),
            IdKind::Music | IdKind::Sounds => {
                let last = extract::text::charmap::SOUNDS
                    .iter()
                    .map(|&(v, _)| v as u32)
                    .max()
                    .unwrap_or(0);
                (last + 1, 0xFFFF)
            }
            IdKind::Maps => (0, 0xFEFF),
        }
    }

    /// Numbers inside [`Self::range`] that are never handed out.
    pub fn reserved(self, num: u32) -> bool {
        self == IdKind::Maps && num == 0x7F7F
    }
}

/// `ids/refs.ron`: what the chapters reference outside the imported RON blocks. It is the
/// import-time snapshot of the pseudo-scripts (§3), map sections (§2) and tables (§4, §7-§9); the
/// scenes become Rust later, and `check` reads this file until then.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct RefsFile {
    /// Every constant named (`FLAG_`, `VAR_`, `MAP_`, `TRAINER_`, `ITEM_`, `SPECIES_`, `MOVE_`,
    /// `MUS_`, `SE_`, `WEATHER_`, `HEAL_LOCATION_`), families expanded -> the chapters naming it.
    pub constants: BTreeMap<String, Vec<String>>,
    /// Text ids the scenes print (`msgbox`, `show_text_on_black`, `show_book`), expanded -> the
    /// chapters.
    pub texts: BTreeMap<String, Vec<String>>,
    /// References written with a placeholder (`Gym2_<P>_Intro`, `FLAG_SIL_FRAGMENT_<n>`,
    /// `Bar_Sc02_T1 | T2 | T3`), sorted by pattern.
    pub dynamic: Vec<DynamicRef>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DynamicRef {
    /// As written.
    pub pattern: String,
    pub kind: RefKind,
    pub chapters: Vec<String>,
    /// The concrete ids it stands for, when the source enumerates them (an explicit list or
    /// range, or the defined ids matching the placeholder); empty when unresolved.
    pub expanded: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Text,
    Constant,
}

/// `check-exceptions.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ExceptionsFile {
    pub exceptions: Vec<Exception>,
}

/// A known issue: findings of `rule` whose subject matches `subject` (`*` matches any run of
/// characters) are listed, not counted as errors or warnings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Exception {
    pub rule: String,
    pub subject: String,
    pub reason: String,
    /// The author has to decide (the reason says what).
    #[serde(default)]
    pub ruling_needed: bool,
}
