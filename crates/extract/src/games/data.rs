//! On-disk formats of the authored data under `games/<game>/` other than maps and layouts
//! ([`super::maps`]): what `cargo xtask silice import` writes and `cargo xtask silice check`
//! reads (SIL1 Task B), and what the content build loads (SIL1 Wave 0,
//! `crates/content/build/silice/schema.rs` re-exports this module). Every file is written with
//! `crate::export::to_ron`, the extract's RON style.
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
//!                            `trainers`, `items`, `species`, `moves`, `songs`, `maps`).
//!   ids/refs.ron             RefsFile: what the chapters' scenes, maps and tables reference
//!                            (import-time snapshot of the pseudo-scripts, until they are Rust).
//!   check-exceptions.ron     ExceptionsFile: known issues `check` lists instead of failing on.
//!   IMPORT-REPORT.md         The import's report: counts and every ambiguity (not loaded).
//! ```
//!
//! **Id allocation (the Wave 0 contract).** Each `ids/<kind>.ron` lists `(name, num)` pairs in
//! allocation order. The first import hands out numbers from the kind's `base` in name order. Later
//! imports never renumber: existing entries are kept as they are, new names are appended after the
//! highest number, and a name the source no longer uses stays (the import report lists it). A
//! removed name becomes a tombstone (`removed: true`) and keeps its number forever, so saves and
//! checkpoints stay valid. Bases and limits are in [`IdKind`].
//!
//! **Duplicate keys (loader contract).** RON maps may repeat a key, and serde keeps the last
//! value silently. The importer writes a text id defined twice in the source twice (in chapter
//! order) so that the duplicate is visible in the canonical data; `check` reports it (rule
//! `text-duplicate`). The game loader reads `text/fr.ron` entry by entry with [`parse_texts`],
//! which **rejects a repeated key** (it never deserializes into a map directly), until the author
//! resolves it.
//!
//! **Text strings.** `text/fr.ron` is written without RON's `\u{..}` escapes for non-ASCII:
//! `é` stays `é`; only `"` and `\` are escaped, so `\\n` in the file is the decomp's `\n`.
//!
//! **Map names.** A map constant `MAP_<WORDS>` names the map file `maps/<Name>.ron` by the Emerald
//! convention (`MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F` is `LittlerootTown_BrendansHouse_1F`): the
//! longest proper prefix that is itself a map is the parent, `<Parent>_<Rest>`; words are
//! capitalized and joined; a floor word (`1F`, `B1F`) is preceded by `_`. So `MAP_BRICO_BOURG_PLAYER_HOUSE_1F`
//! is `BricoBourg_PlayerHouse_1F` and `MAP_ROUTE_S01` is `RouteS01`. `ids/maps.ron` records the
//! name of each constant, so the rule is applied once, at import.

use crate::schema::{ItemPocket, LandEncounters, TrainerMon};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `text/fr.ron` (keys sorted; a duplicated key is written once per definition). The game
/// loader reads it as this map; `silice check` reads it as pairs to see duplicates.
pub type TextFile = crate::schema::TextFile;

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

/// `battle/forms.ron`: the battle-only forms (Mega Evolutions, Primal Reversions). Hand-written
/// (the chapters name the stones, not the forms' data); each form is numbered by
/// `ids/species.ron` and takes from its base species everything but its stats, types, ability
/// and weight (name, catch rate, yields, gender, growth, learnset, pics).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct FormsFile {
    /// The flag the player's mons need to Mega Evolve (`FLAG_SIL_*`); `None`: always.
    pub mega_flag: Option<String>,
    pub forms: Vec<FormDef>,
}

/// One battle-only form.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FormDef {
    /// `SPECIES_*_MEGA` / `_PRIMAL` (`ids/species.ron`).
    pub id: String,
    /// The Gen3 base species.
    pub from: String,
    /// The held item that triggers it (`ITEM_SIL_*`).
    pub item: String,
    pub kind: FormKindDef,
    /// HP, Atk, Def, Speed, SpAtk, SpDef.
    pub base: [u8; 6],
    /// `TYPE_*`.
    pub types: [String; 2],
    /// `ABILITY_*`.
    pub ability: String,
    /// Hectograms.
    pub weight: u16,
}

/// How a [`FormDef`] triggers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKindDef {
    /// Chosen in battle, once per side; the player needs the Mega flag (`game.ron`).
    Mega,
    /// On entering the field.
    Primal,
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
    /// Price from a mart table with an item-name column.
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
    /// Where, as written.
    pub place: String,
    /// The `(x,y)` the place names, when it names exactly one: a note from the source's ASCII
    /// plans, which are not to scale. The maps under `maps/` are authoritative for positions;
    /// `check` only matches the flag and item there (rule `placement-map`).
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
    /// When the shop is open, as written (a label parenthetical naming a flag or var).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// The prose around the stock (before a table, after an inline list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
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
    /// Money or coins (the coin case holds 9,999); Gen3 prices are `u16` too.
    pub price: u16,
    /// A `Condition` column, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// A `Note` (or `Pourquoi`) column, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `ids/<kind>.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IdsFile {
    /// The first number this kind hands out.
    pub base: u32,
    /// In allocation order.
    pub ids: Vec<IdEntry>,
}

/// `sound.ron` (Milestone 12): the game's own songs and invented sound effects, each mapped to
/// the Emerald song that plays for it until the author's tracks land.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SoundFile {
    /// `MUS_SIL_*` (`ids/songs.ron`) -> Emerald `MUS_*`.
    pub songs: std::collections::BTreeMap<String, String>,
    /// An invented `SE_*` name the scripts use -> Emerald `SE_*`.
    pub effects: std::collections::BTreeMap<String, String>,
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
    /// Music and sound effects: Gen3 numbers them in one table (`gSongTable`), so they share one
    /// number space and one file.
    Songs,
    Maps,
}

impl IdKind {
    pub const ALL: [IdKind; 8] = [
        IdKind::Flags,
        IdKind::Vars,
        IdKind::Trainers,
        IdKind::Items,
        IdKind::Species,
        IdKind::Moves,
        IdKind::Songs,
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
            IdKind::Songs => "songs",
            IdKind::Maps => "maps",
        }
    }

    /// The prefixes a Silice name of this kind has (empty for maps and species forms, which
    /// follow the Gen3 naming: maps live in Silice's own namespace, forms are `SPECIES_<BASE>_MEGA`).
    pub fn sil_prefixes(self) -> &'static [&'static str] {
        match self {
            IdKind::Flags => &["FLAG_SIL_"],
            IdKind::Vars => &["VAR_SIL_"],
            IdKind::Trainers => &["TRAINER_SIL_"],
            IdKind::Items => &["ITEM_SIL_"],
            IdKind::Moves => &["MOVE_SIL_"],
            IdKind::Songs => &["MUS_SIL_", "SE_SIL_"],
            IdKind::Species | IdKind::Maps => &[],
        }
    }

    /// The first number, and the last one allowed.
    ///
    /// - flags: 0x20 (after the temp flags 0x00-0x1F) to 0x4FF (trainer flags start at 0x500);
    /// - vars: 0x4020 (after the temp vars and `VAR_OBJ_GFX_ID_*`) to 0x40FF (the saved vars).
    ///   For flags and vars the first import raises the base above every Gen3 story flag or var
    ///   the Silice data reuses (`FLAG_RECEIVED_RUNNING_SHOES` 0x112), so a reused Gen3 number is
    ///   never inside the Silice block (rule `gen3-collision`);
    /// - trainers: 1 to 863, so `0x500 + id` stays in the trainer-flag range (0x500-0x85F);
    /// - items, species, moves and songs (music and sounds): after the last Gen3 number of the decomp table
    ///   (`crate::names`, `charmap::SOUNDS`), up to 0xFFFF;
    /// - maps: 0 to 0xFEFF (0x7F7F `MAP_DYNAMIC` is skipped; 0xFF00-0xFFFF are reserved for
    ///   `MAP_UNDEFINED` and the studio's previews).
    pub fn range(self) -> (u32, u32) {
        let after =
            |t: &crate::names::Table| t.entries.iter().map(|&(v, _)| v).max().unwrap_or(0) + 1;
        match self {
            IdKind::Flags => (0x20, 0x4FF),
            IdKind::Vars => (0x4020, 0x40FF),
            IdKind::Trainers => (1, 863),
            IdKind::Items => (after(&crate::names::ITEMS), 0xFFFF),
            IdKind::Species => (after(&crate::names::SPECIES), 0xFFFF),
            IdKind::Moves => (after(&crate::names::MOVES), 0xFFFF),
            IdKind::Songs => {
                let last = crate::text::charmap::SOUNDS
                    .iter()
                    .map(|&(v, _)| v as u32)
                    .max()
                    .unwrap_or(0);
                (last + 1, 0xFFFF)
            }
            IdKind::Maps => (0, 0xFEFF),
        }
    }

    /// Numbers inside [`Self::range`] that are never handed out: `MAP_DYNAMIC` for maps; for
    /// flags and vars, the numbers of [`SHARED_GEN3`] (the Gen3 flags and vars the shared code
    /// reads or writes in every flavour). A tombstone may hold one (an id moved off it).
    pub fn reserved(self, num: u32) -> bool {
        match self {
            IdKind::Maps => num == 0x7F7F,
            IdKind::Flags | IdKind::Vars => shared_gen3().any(|(k, _, v)| k == self && v == num),
            _ => false,
        }
    }
}

/// The Gen3 flags and vars that the code every flavour shares reads or writes: the content
/// crate root and its `SpecialIds`, the Gen3 prelude (`crates/content/src/prelude/`), the
/// Silice root (`crates/content/src/silice/{mod,prelude,new_game}.rs`, with `game.ron`) and
/// `samples.rs`. The Silice ids never take their numbers ([`IdKind::reserved`]): a Silice var
/// on `VAR_STARTER_MON` would be overwritten by `choose_starter`. The list is derived from those
/// sources: `crates/content/tests/sil1_w0_contracts.rs` scans them for every Gen3 `FLAG_*` and
/// `VAR_*` name and fails when this list differs.
pub const SHARED_GEN3: &[&str] = &[
    "FLAG_BADGE01_GET",
    "FLAG_BADGE02_GET",
    "FLAG_BADGE03_GET",
    "FLAG_BADGE04_GET",
    "FLAG_BADGE05_GET",
    "FLAG_BADGE06_GET",
    "FLAG_BADGE07_GET",
    "FLAG_BADGE08_GET",
    "FLAG_NURSE_UNION_ROOM_REMINDER",
    "FLAG_SHOWN_BOX_WAS_FULL_MESSAGE",
    "FLAG_SYS_B_DASH",
    "FLAG_SYS_CLOCK_SET",
    "FLAG_SYS_CTRL_OBJ_DELETE",
    "FLAG_SYS_ENC_DOWN_ITEM",
    "FLAG_SYS_ENC_UP_ITEM",
    "FLAG_SYS_GAME_CLEAR",
    "FLAG_SYS_NATIONAL_DEX",
    "FLAG_SYS_POKEDEX_GET",
    "FLAG_SYS_POKEMON_GET",
    "FLAG_SYS_RIBBON_GET",
    "FLAG_SYS_TOWER_GOLD",
    "FLAG_SYS_TOWER_SILVER",
    "FLAG_SYS_USE_STRENGTH",
    "VAR_ALTERING_CAVE_WILD_SET",
    "VAR_BIRCH_STATE",
    "VAR_DAYS",
    "VAR_FACING",
    "VAR_FRIENDSHIP_STEP_COUNTER",
    "VAR_ITEM_ID",
    "VAR_LAST_TALKED",
    "VAR_MON_BOX_ID",
    "VAR_MON_BOX_POS",
    "VAR_NATIONAL_DEX",
    "VAR_PC_BOX_TO_SEND_MON",
    "VAR_REPEL_STEP_COUNT",
    "VAR_RESULT",
    "VAR_STARTER_MON",
    "VAR_TEMP_3",
    "VAR_TEMP_CHALLENGE_STATUS",
];

/// [`SHARED_GEN3`] with each name's kind and Gen3 number (`crate::names`).
pub fn shared_gen3() -> impl Iterator<Item = (IdKind, &'static str, u32)> {
    SHARED_GEN3.iter().filter_map(|&n| {
        let (kind, table) = if n.starts_with("FLAG_") {
            (IdKind::Flags, &crate::names::FLAGS)
        } else {
            (IdKind::Vars, &crate::names::VARS)
        };
        Some((kind, n, table.value(n)?))
    })
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

/// Parses a `text/<lang>.ron` ([`TextFile`]) entry by entry, in file order, and fails on a
/// repeated key, naming it: the loader contract above. `silice check` reports duplicates
/// instead (rule `text-duplicate`).
pub fn parse_texts(text: &str) -> anyhow::Result<Vec<(String, String)>> {
    use serde::de::{Deserializer, MapAccess, Visitor};
    struct Unique;
    impl<'de> Visitor<'de> for Unique {
        type Value = Vec<(String, String)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map of text id to text")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut seen = std::collections::BTreeSet::new();
            let mut out = Vec::new();
            while let Some((k, v)) = map.next_entry::<String, String>()? {
                if !seen.insert(k.clone()) {
                    return Err(serde::de::Error::custom(format!(
                        "text id {k} is defined twice (repeated key)"
                    )));
                }
                out.push((k, v));
            }
            Ok(out)
        }
    }
    let mut de = ron::Deserializer::from_str(text)?;
    let out = (&mut de).deserialize_map(Unique)?;
    de.end()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_gen3_names_are_reserved() {
        assert_eq!(
            shared_gen3().count(),
            SHARED_GEN3.len(),
            "every name is Gen3"
        );
        // VAR_STARTER_MON, VAR_PC_BOX_TO_SEND_MON, VAR_ALTERING_CAVE_WILD_SET, VAR_DAYS,
        // VAR_BIRCH_STATE: inside the Silice var block.
        for v in [0x4023, 0x4036, 0x403E, 0x4040, 0x4049] {
            assert!(IdKind::Vars.reserved(v), "{v:#x}");
            assert!(!IdKind::Flags.reserved(v), "{v:#x}");
        }
        assert!(!IdKind::Vars.reserved(0x4020));
        assert!(IdKind::Flags.reserved(0x8C0), "FLAG_SYS_B_DASH");
        assert!(IdKind::Maps.reserved(0x7F7F));
        assert!(!IdKind::Items.reserved(0x4023));
    }

    #[test]
    fn parse_texts_keeps_file_order_and_rejects_a_repeated_key() {
        let ok = parse_texts(r#"{"B": r"x\n", "A": "y"}"#).unwrap();
        assert_eq!(ok, [("B".into(), r"x\n".into()), ("A".into(), "y".into())]);
        let e = parse_texts(r#"{"A": "x", "B": "y", "A": "z"}"#).unwrap_err();
        assert!(
            format!("{e:#}").contains("text id A is defined twice"),
            "{e:#}"
        );
        assert!(parse_texts(r#"{"A": "x"} trailing"#).is_err());
    }
}
