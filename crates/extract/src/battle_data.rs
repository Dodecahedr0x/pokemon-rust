//! Battle data and French battle strings (Milestone 4 Task A): `gSpeciesInfo` with
//! `gSpeciesNames` and `gLevelUpLearnsets`, `gBattleMoves` with `gMoveNames`,
//! `gTypeEffectiveness` with `gTypeNames`, `gTrainers` with their parties and
//! `gTrainerClassNames`, every table of `gWildMonHeaders`, `gEvolutionTable`,
//! `gTrainerMoneyTable`, `sNaturePowerMoves`, `sHealLocations`, `gBattleStringsTable` and the
//! code-referenced battle strings.
//!
//! **Locating.** Every table is found on both ROMs (BPEE and BPEF) from language-independent
//! bytes; the US decomp is the test oracle (`tests/battle_data.rs`):
//! - data anchors, Gen3 values written from the decomp sources: `SPECIES_NONE` (28 zero bytes)
//!   then Bulbasaur's first stats; `MOVE_NONE` then Pound; the first four type matchups;
//!   Bulbasaur's learnset (pointed to by `gLevelUpLearnsets[0]` and `[1]`); the Medium Fast
//!   experience table (`n³`); `sNaturePowerMoves`; the first two heal locations; and
//!   `gSpeciesNames[0]` (`??????????`).
//! - layout: `src/data.c` emits `gTrainerClassNames`, `gTrainers`, `gSpeciesNames` and
//!   `gMoveNames` back to back, and `battle_main.c` puts `gTypeNames` right after
//!   `gTypeEffectiveness`, then `gTrainerMoneyTable` (`{class, value}` pairs up to the `0xFF`
//!   class). `gWildMonHeaders` is the run of valid headers holding `MAP_ROUTE101`.
//! - `sSpeciesToNationalPokedexNum` is the referenced run of national numbers 1 to 251 (Gen1-2
//!   species are in national order), and `gPokedexEntries` the referenced table of 0x20-byte
//!   entries whose entries 0 to 3 weigh 0, 69, 130 and 1000 hectograms (none, Bulbasaur, Ivysaur,
//!   Venusaur, with heights 0, 7, 10 and 20): only the numbers, as the category names are
//!   localized.
//! - `gEvolutionTable` (`NUM_SPECIES` × `EVOS_PER_MON` × 8 bytes): `SPECIES_NONE`'s empty row
//!   then Bulbasaur's (`{EVO_LEVEL, 16, SPECIES_IVYSAUR}`) and Ivysaur's first entry.
//! - `gBattleStringsTable` is the only run of exactly `BATTLESTRINGS_COUNT -
//!   BATTLESTRINGS_TABLE_START` ROM pointers whose first string is `{B_TRAINER1_LOSE_TEXT}`.
//! - strings only code refers to: `battle_message.c` strings are packed in source order, so the
//!   ones after `sText_OutOfSafariBalls` (a table entry) are walked by name ([`CODE_STRINGS`],
//!   checked against `gStatNamesTable`, which follows them), and the menu strings are walked
//!   around `gText_MoveInterfaceDynamicColors`, whose bytes are language-independent
//!   ([`MENU_STRINGS`]). `gText_ConfirmStarterChoice` is the literal of `Task_AskConfirmStarter`
//!   (US code bytes with branch offsets masked), and `gText_BirchInTrouble` precedes it in
//!   `strings.c`.
//!
//! **Battle strings** are written in decomp `.string` syntax with the battle names of the 0xFD
//! placeholders (`{B_BUFF2}`, not the field `{PLAYER}` that shares FD 01).
//!
//! **The wild/foe tag.** BPEE prefixes the nickname (`"Wild "`, `"Foe "`); BPEF appends a
//! suffix: its `HANDLE_NICKNAME_STRING_CASE` (`BattleStringExpandPlaceholders`, 0x14e884 in
//! BPEF) copies the nickname, then `StringAppend`s `sText_WildPkmnPrefix` (wild battle) or
//! `sText_FoePkmnPrefix` (trainer battle), each a space then a word. Both strings are exported
//! under their decomp names; [`tag_is_suffix`] tells the two forms apart from the bytes (a
//! suffix starts with a space, a prefix ends with one).
//!
//! **Stat-change messages.** BPEE builds "sharply rose!" from two buffer strings,
//! `STRINGID_STATSHARPLY` (`"sharply "`) then `STRINGID_STATROSE` (`"rose!"`), and likewise
//! `STRINGID_STATHARSHLY` + `STRINGID_STATFELL`. BPEF's `ExpandBattleTextBuffPlaceholders`
//! (`B_BUFF_STRING` case, 0x14f3b8 in BPEF: `cmp r2, #0xD1; beq; cmp r2, #0xD3; bne; adds r5,
//! #3`) skips the next buffer entry after `STRINGID_STATSHARPLY` (0xD1) or
//! `STRINGID_STATHARSHLY` (0xD3): there the two strings are complete phrases ending in `!`, and
//! the following "rose!"/"fell!" entry is not printed. [`stat_sharply_is_complete`] tells the
//! two forms apart from the strings.
//!
//! **Stat names** are also exported in `STAT_*` order as `gStatNamesTable_0` ..
//! `gStatNamesTable_7` ([`STAT_NAME_KEYS`]).

use crate::export::Out;
use crate::schema::{
    BattleStringsFile, EncounterHeader, EncountersFile, EvolutionsFile, HealLocationsFile, Hex,
    LandEncounters, Move, MovesFile, Species, SpeciesFile, Trainer, TrainerMon, TrainerMoneyFile,
    TrainersFile, TypesFile, WildMon,
};
use crate::text::{self, charmap};
use crate::{ROM_BASE, Rom, names};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

const SPECIES_INFO_SIZE: u32 = 28;
const MOVE_SIZE: u32 = 12;
const TRAINER_SIZE: u32 = 40;
const WILD_HEADER_SIZE: u32 = 20;
/// `struct HealLocation` (s8, s8, u16, u16) padded to 8 by agbcc.
const HEAL_LOCATION_SIZE: u32 = 8;
const SPECIES_NAME_SIZE: u32 = 11;
const MOVE_NAME_SIZE: u32 = 13;
const TYPE_NAME_SIZE: u32 = 7;
const CLASS_NAME_SIZE: u32 = 13;
/// `ABILITY_NAME_LENGTH + 1`.
const ABILITY_NAME_SIZE: u32 = 13;
/// How far after `gTypeNames` [`locate_ability_names`] looks for the description pointers.
const ABILITY_SEARCH_SPAN: u32 = 0x2000;
/// `trainerName` spans 0x04..0x10 of `struct Trainer`.
const TRAINER_NAME_SIZE: usize = 12;
/// Slots of the `landMonsInfo`, `waterMonsInfo`, `rockSmashMonsInfo` and `fishingMonsInfo`
/// tables (`wild_encounters.json` `fields`; fishing: old rod 0-1, good rod 2-4, super rod 5-9).
const WILD_SLOTS: [u32; 4] = [12, 5, 5, 10];
/// `EVOS_PER_MON`, and `sizeof(struct Evolution)` (three u16 padded to 8 by agbcc).
const EVOS_PER_MON: u32 = 5;
const EVOLUTION_SIZE: u32 = 8;
/// `EVO_*` by value (`include/constants/pokemon.h`); 0 is an empty slot.
pub const EVO_METHODS: [&str; 16] = [
    "",
    "EVO_FRIENDSHIP",
    "EVO_FRIENDSHIP_DAY",
    "EVO_FRIENDSHIP_NIGHT",
    "EVO_LEVEL",
    "EVO_TRADE",
    "EVO_TRADE_ITEM",
    "EVO_ITEM",
    "EVO_LEVEL_ATK_GT_DEF",
    "EVO_LEVEL_ATK_EQ_DEF",
    "EVO_LEVEL_ATK_LT_DEF",
    "EVO_LEVEL_SILCOON",
    "EVO_LEVEL_CASCOON",
    "EVO_LEVEL_NINJASK",
    "EVO_LEVEL_SHEDINJA",
    "EVO_BEAUTY",
];
/// `sizeof(struct PokedexEntry)`, and the offset of its `height` (then `weight`).
const DEX_ENTRY_SIZE: u32 = 0x20;
const DEX_ENTRY_HEIGHT: u32 = 0x0C;
/// `NATIONAL_DEX_COUNT` (`NATIONAL_DEX_DEOXYS`): `gPokedexEntries` has entries 0 to this.
pub const NATIONAL_DEX_COUNT: u32 = 386;
/// Species 1 to this are numbered in national order (`NATIONAL_DEX_CELEBI`, Gen1-2).
const JOHTO_DEX_COUNT: u16 = 251;
/// (height, weight) of `gPokedexEntries[0..4]`.
const DEX_ANCHOR: [(u16, u16); 4] = [(0, 0), (7, 69), (10, 130), (20, 1000)];
/// The class that ends `gTrainerMoneyTable`; its value is the default of unlisted classes.
const TRAINER_MONEY_END: u8 = 0xFF;
/// `sizeof(struct TrainerMoney)`: two u8 padded to 4 by agbcc.
const TRAINER_MONEY_SIZE: u32 = 4;
/// Guard for a corrupt money table.
const MAX_TRAINER_MONEY: u32 = 256;
pub const MAX_LEVEL: usize = 100;
pub const GROWTH_RATES: usize = 6;
/// `LEVEL_UP_END`.
const LEARNSET_END: u16 = 0xFFFF;
/// `TYPE_FORESIGHT` and `TYPE_ENDTABLE` in `gTypeEffectiveness`.
const TYPE_FORESIGHT: u8 = 0xFE;
const TYPE_ENDTABLE: u8 = 0xFF;
/// `F_TRAINER_PARTY_CUSTOM_MOVESET`, `F_TRAINER_PARTY_HELD_ITEM`, `F_TRAINER_FEMALE`.
const PARTY_CUSTOM_MOVES: u8 = 1 << 0;
const PARTY_HELD_ITEM: u8 = 1 << 1;
const TRAINER_FEMALE: u8 = 1 << 7;
/// Guard for corrupt tables.
const MAX_LEARNSET: usize = 64;
const MAX_WILD_HEADERS: u32 = 1024;

/// `sNaturePowerMoves` by `BATTLE_ENVIRONMENT_*` (`battle_script_commands.c`).
const NATURE_POWER: [&str; 10] = [
    "MOVE_STUN_SPORE",
    "MOVE_RAZOR_LEAF",
    "MOVE_EARTHQUAKE",
    "MOVE_HYDRO_PUMP",
    "MOVE_SURF",
    "MOVE_BUBBLE_BEAM",
    "MOVE_ROCK_SLIDE",
    "MOVE_SHADOW_BALL",
    "MOVE_SWIFT",
    "MOVE_SWIFT",
];

/// The first two `sHealLocations` entries (`src/data/heal_locations.json`): map and cell.
const HEAL_ANCHOR: [(&str, u16, u16); 2] = [
    ("LittlerootTown_BrendansHouse_2F", 4, 2),
    ("LittlerootTown_MaysHouse_2F", 4, 2),
];

/// The map whose header anchors `gWildMonHeaders`.
const WILD_ANCHOR_MAP: &str = "Route101";

/// `battle_message.c` strings in source order right after `sText_OutOfSafariBalls`
/// (`STRINGID_OUTOFSAFARIBALLS`), up to the last entry of `gStatNamesTable`, which follows them.
/// `tests/battle_data.rs` checks this list against the decomp source.
pub const CODE_STRINGS: [&str; 53] = [
    "sText_OpponentMon1Appeared",
    "sText_WildPkmnAppeared",
    "sText_LegendaryPkmnAppeared",
    "sText_WildPkmnAppearedPause",
    "sText_TwoWildPkmnAppeared",
    "sText_Trainer1WantsToBattle",
    "sText_LinkTrainerWantsToBattle",
    "sText_TwoLinkTrainersWantToBattle",
    "sText_Trainer1SentOutPkmn",
    "sText_Trainer1SentOutTwoPkmn",
    "sText_Trainer1SentOutPkmn2",
    "sText_LinkTrainerSentOutPkmn",
    "sText_LinkTrainerSentOutTwoPkmn",
    "sText_TwoLinkTrainersSentOutPkmn",
    "sText_LinkTrainerSentOutPkmn2",
    "sText_LinkTrainerMultiSentOutPkmn",
    "sText_GoPkmn",
    "sText_GoTwoPkmn",
    "sText_GoPkmn2",
    "sText_DoItPkmn",
    "sText_GoForItPkmn",
    "sText_YourFoesWeakGetEmPkmn",
    "sText_LinkPartnerSentOutPkmnGoPkmn",
    "sText_PkmnThatsEnough",
    "sText_PkmnComeBack",
    "sText_PkmnOkComeBack",
    "sText_PkmnGoodComeBack",
    "sText_Trainer1WithdrewPkmn",
    "sText_LinkTrainer1WithdrewPkmn",
    "sText_LinkTrainer2WithdrewPkmn",
    "sText_WildPkmnPrefix",
    "sText_FoePkmnPrefix",
    "sText_EmptyString8",
    "sText_FoePkmnPrefix2",
    "sText_AllyPkmnPrefix",
    "sText_FoePkmnPrefix3",
    "sText_AllyPkmnPrefix2",
    "sText_FoePkmnPrefix4",
    "sText_AllyPkmnPrefix3",
    "sText_AttackerUsedX",
    "sText_ExclamationMark",
    "sText_ExclamationMark2",
    "sText_ExclamationMark3",
    "sText_ExclamationMark4",
    "sText_ExclamationMark5",
    "sText_HP2",
    "sText_Attack2",
    "sText_Defense2",
    "sText_Speed",
    "sText_SpAtk2",
    "sText_SpDef2",
    "sText_Accuracy",
    "sText_Evasiveness",
];

/// The `STRINGID_*` whose string [`CODE_STRINGS`] follows.
const CODE_STRINGS_AFTER: &str = "STRINGID_OUTOFSAFARIBALLS";
/// `gStatNamesTable` entries: the last [`STAT_NAMES`] walked strings, in `STAT_*` order.
pub const STAT_NAMES: usize = 8;

/// `battle_message.c` strings in source order around the anchor
/// `gText_MoveInterfaceDynamicColors` (index [`MENU_ANCHOR`]), whose bytes are only control codes.
pub const MENU_STRINGS: [&str; 13] = [
    "gText_WhatWillPkmnDo",
    "gText_WhatWillPkmnDo2",
    "gText_WhatWillWallyDo",
    "gText_LinkStandby",
    "gText_BattleMenu",
    "gText_SafariZoneMenu",
    "gText_MoveInterfacePP",
    "gText_MoveInterfaceType",
    "gText_MoveInterfacePpType",
    "gText_MoveInterfaceDynamicColors",
    "gText_WhichMoveToForget4",
    "gText_BattleYesNoChoice",
    "gText_BattleSwitchWhich",
];
const MENU_ANCHOR: usize = 9;
/// `gText_MoveInterfaceDynamicColors` without its terminator.
const MENU_ANCHOR_TEXT: &str =
    "{PALETTE 5}{COLOR_HIGHLIGHT_SHADOW DYNAMIC_COLOR4 DYNAMIC_COLOR5 DYNAMIC_COLOR6}";

/// `Task_AskConfirmStarter` (`starter_choose.c`) as compiled in BPEE, `None` for the bytes of
/// `bl` instructions (their offsets move with the code). Its literal pool holds
/// `gText_ConfirmStarterChoice` at [`CONFIRM_STARTER_LITERAL`].
const CONFIRM_STARTER_CODE: &str = "10 b5 83 b0 00 06 00 0e 15 49 84 00 24 18 e4 00 64 18 20 89 \
     ?? ?? ?? ?? 00 04 00 0c 00 21 ?? ?? ?? ?? 00 20 11 21 ?? ?? ?? ?? 0e 4a 01 20 00 90 00 20 \
     01 90 02 90 01 21 00 23 ?? ?? ?? ?? 00 20 ?? ?? ?? ?? 08 48 aa 21 89 00 0d 22 00 23 \
     ?? ?? ?? ?? 06 48 20 60 03 b0 10 bc 01 bc 00 47 00 00";
const CONFIRM_STARTER_LITERAL: u32 = 0x64;

/// File offsets of every table this module reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    pub species_info: u32,
    pub species_names: u32,
    pub learnsets: u32,
    pub moves: u32,
    pub move_names: u32,
    pub type_chart: u32,
    pub type_names: u32,
    pub ability_names: u32,
    pub trainers: u32,
    pub trainer_class_names: u32,
    pub experience_tables: u32,
    pub wild_headers: u32,
    pub evolutions: u32,
    pub trainer_money: u32,
    pub species_to_national: u32,
    pub pokedex_entries: u32,
    pub nature_power: u32,
    pub heal_locations: u32,
    pub battle_strings: u32,
    pub stat_names: u32,
    /// The [`CODE_STRINGS`], in order.
    pub code_strings: Vec<u32>,
    /// `gText_WhatWillPkmnDo`, the first of [`MENU_STRINGS`].
    pub menu_strings: u32,
    pub confirm_starter_choice: u32,
}

/// Table sizes, from the decomp constants in [`names`].
pub struct Counts {
    /// `NUM_SPECIES` (= `SPECIES_EGG`).
    pub species: u32,
    /// `MOVES_COUNT`.
    pub moves: u32,
    /// `NUMBER_OF_MON_TYPES`.
    pub types: u32,
    /// `ABILITIES_COUNT`.
    pub abilities: u32,
    /// `TRAINERS_COUNT`.
    pub trainers: u32,
    pub trainer_classes: u32,
    /// `NUM_HEAL_LOCATIONS - 1`.
    pub heal_locations: u32,
    /// `BATTLESTRINGS_TABLE_START`.
    pub strings_start: u32,
    /// `BATTLESTRINGS_COUNT`.
    pub strings: u32,
}

fn count(t: &names::Table) -> u32 {
    t.entries.last().map_or(0, |&(v, _)| v + 1)
}

pub fn counts() -> Counts {
    Counts {
        species: names::SPECIES.value("SPECIES_EGG").expect("SPECIES_EGG"),
        moves: count(&names::MOVES),
        types: count(&names::TYPES),
        abilities: count(&names::ABILITIES),
        trainers: count(&names::TRAINERS),
        trainer_classes: count(&names::TRAINER_CLASSES),
        heal_locations: count(&names::HEAL_LOCATIONS) - 1,
        strings_start: names::STRING_IDS
            .value("STRINGID_TRAINER1LOSETEXT")
            .expect("STRINGID_TRAINER1LOSETEXT"),
        strings: count(&names::STRING_IDS),
    }
}

fn value(t: &names::Table, name: &str) -> u32 {
    t.value(name)
        .unwrap_or_else(|| panic!("{name} is not in names"))
}

fn is_rom_ptr(rom: &Rom, v: u32) -> bool {
    v >= ROM_BASE && ((v - ROM_BASE) as usize) < rom.bytes().len()
}

/// The single offset of `needle` that some pointer refers to.
fn unique_referenced(rom: &Rom, needle: &[u8], what: &str) -> Result<u32> {
    let hits: Vec<u32> = rom
        .find_all(needle)
        .into_iter()
        .filter(|&o| !rom.pointers_to(o).is_empty())
        .collect();
    match hits[..] {
        [o] => Ok(o),
        _ => bail!("{what}: expected 1 referenced match, found {hits:#x?}"),
    }
}

fn ensure_referenced(rom: &Rom, off: u32, what: &str) -> Result<()> {
    ensure!(
        !rom.pointers_to(off).is_empty(),
        "{what} at {off:#x}: nothing points to it"
    );
    Ok(())
}

fn u16s_le(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Bytes with `None` wildcards, from `"10 b5 ?? …"`.
fn parse_pattern(s: &str) -> Vec<Option<u8>> {
    s.split_whitespace()
        .map(|t| (t != "??").then(|| u8::from_str_radix(t, 16).expect("hex byte")))
        .collect()
}

fn find_pattern(rom: &Rom, pat: &[Option<u8>]) -> Vec<u32> {
    let first = pat[0].expect("patterns start with a fixed byte");
    rom.bytes()
        .windows(pat.len())
        .enumerate()
        .filter(|(_, w)| w[0] == first && w.iter().zip(pat).all(|(b, p)| p.is_none_or(|p| p == *b)))
        .map(|(i, _)| i as u32)
        .collect()
}

pub fn locate(rom: &Rom) -> Result<Locations> {
    let n = counts();
    let ty = |name| value(&names::TYPES, name) as u8;

    let mut species = vec![0u8; SPECIES_INFO_SIZE as usize];
    species.extend([
        45,
        49,
        49,
        45,
        65,
        65,
        ty("TYPE_GRASS"),
        ty("TYPE_POISON"),
        45,
        64,
    ]);
    let species_info = unique_referenced(rom, &species, "gSpeciesInfo")?;

    let mut pound = vec![0u8; MOVE_SIZE as usize];
    pound.extend([0, 40, ty("TYPE_NORMAL"), 100, 35, 0, 0, 0, 0x33, 0, 0, 0]);
    let moves = unique_referenced(rom, &pound, "gBattleMoves")?;

    let chart_start = [
        ("TYPE_NORMAL", "TYPE_ROCK"),
        ("TYPE_NORMAL", "TYPE_STEEL"),
        ("TYPE_FIRE", "TYPE_FIRE"),
        ("TYPE_FIRE", "TYPE_WATER"),
    ]
    .iter()
    .flat_map(|&(a, d)| [ty(a), ty(d), 5])
    .collect::<Vec<u8>>();
    let type_chart = unique_referenced(rom, &chart_start, "gTypeEffectiveness")?;
    let chart_end = type_chart_end(rom, type_chart)?;
    let type_names = (chart_end + 3).next_multiple_of(4);
    ensure_referenced(rom, type_names, "gTypeNames")?;
    let ability_names = locate_ability_names(rom, type_names, &n)?;

    let mv = |name| value(&names::MOVES, name) as u16;
    let bulbasaur_moves = u16s_le(&[
        1 << 9 | mv("MOVE_TACKLE"),
        4 << 9 | mv("MOVE_GROWL"),
        7 << 9 | mv("MOVE_LEECH_SEED"),
        10 << 9 | mv("MOVE_VINE_WHIP"),
    ]);
    let bulbasaur = unique_referenced(rom, &bulbasaur_moves, "Bulbasaur's learnset")?;
    let refs = rom.pointers_to(bulbasaur);
    let learnsets = match refs.windows(2).find(|w| w[1] == w[0] + 4) {
        Some(w) => w[0],
        None => bail!("gLevelUpLearnsets: no two consecutive pointers to {bulbasaur:#x}"),
    };

    let medium_fast: Vec<u8> = (0..=MAX_LEVEL as u32)
        .flat_map(|l| (l * l * l).to_le_bytes())
        .collect();
    let experience_tables = unique_referenced(rom, &medium_fast, "gExperienceTables")?;

    let mut question = vec![text::encode("?").expect("?")[0]; 10];
    question.push(0xFF);
    let species_names = match rom.find_all(&question)[..] {
        [o] => o,
        ref hits => bail!("gSpeciesNames: expected 1 match, found {hits:#x?}"),
    };
    ensure_referenced(rom, species_names, "gSpeciesNames")?;
    let move_names = species_names + SPECIES_NAME_SIZE * n.species;
    ensure_referenced(rom, move_names, "gMoveNames")?;
    let trainers = species_names
        .checked_sub(TRAINER_SIZE * n.trainers)
        .context("gTrainers before ROM start")?;
    ensure_referenced(rom, trainers, "gTrainers")?;
    ensure!(
        rom.u8(trainers + 0x20)? == 0 && rom.ptr(trainers + 0x24)?.is_none(),
        "gTrainers at {trainers:#x}: TRAINER_NONE has a party"
    );
    let trainer_class_names = (trainers - CLASS_NAME_SIZE * n.trainer_classes) & !3;
    ensure_referenced(rom, trainer_class_names, "gTrainerClassNames")?;

    let wild_headers = locate_wild_headers(rom)?;
    let evolutions = locate_evolutions(rom, &n)?;
    let trainer_money = locate_trainer_money(rom, type_names, &n)?;
    let species_to_national = unique_referenced(
        rom,
        &u16s_le(&(1..=JOHTO_DEX_COUNT).collect::<Vec<_>>()),
        "sSpeciesToNationalPokedexNum",
    )?;
    let pokedex_entries = locate_pokedex_entries(rom)?;

    let nature_power =
        unique_referenced(rom, &u16s_le(&NATURE_POWER.map(mv)), "sNaturePowerMoves")?;

    let mut heal = Vec::new();
    for (map, x, y) in HEAL_ANCHOR {
        let m = names::map_by_dir(map).expect("heal anchor map");
        heal.extend([m.group, m.num]);
        heal.extend(x.to_le_bytes());
        heal.extend(y.to_le_bytes());
        heal.extend([0, 0]);
    }
    heal.truncate(heal.len() - 2);
    let heal_locations = unique_referenced(rom, &heal, "sHealLocations")?;

    let battle_strings = locate_battle_strings(rom, &n)?;
    let (code_strings, stat_names) = walk_code_strings(rom, battle_strings, &n)?;
    let menu_strings = locate_menu_strings(rom)?;
    let confirm_starter_choice = locate_confirm_starter(rom)?;

    Ok(Locations {
        species_info,
        species_names,
        learnsets,
        moves,
        move_names,
        type_chart,
        type_names,
        ability_names,
        trainers,
        trainer_class_names,
        experience_tables,
        wild_headers,
        evolutions,
        trainer_money,
        species_to_national,
        pokedex_entries,
        nature_power,
        heal_locations,
        battle_strings,
        stat_names,
        code_strings,
        menu_strings,
        confirm_starter_choice,
    })
}

/// `gAbilityNames`. `battle_main.c` includes `data/text/abilities.h` after `gTypeNames` and
/// `gTrainerMoneyTable`: the descriptions (`sNoneDescription` …), then `gAbilityNames`
/// (`ABILITIES_COUNT` × 13 bytes, only 2-byte aligned in BPEF), then
/// `gAbilityDescriptionPointers` at the next 4-byte boundary. The pointer table is the
/// referenced run of `ABILITIES_COUNT` increasing pointers into the descriptions, all after
/// `gTypeNames` and before the names; the names are the referenced table ending in the 3
/// padding bytes before it.
fn locate_ability_names(rom: &Rom, type_names: u32, n: &Counts) -> Result<u32> {
    let names_size = ABILITY_NAME_SIZE * n.abilities;
    let mut hits = Vec::new();
    for table in (type_names..type_names + ABILITY_SEARCH_SPAN).step_by(4) {
        let Some(latest) = table.checked_sub(names_size) else {
            continue;
        };
        if latest <= type_names + 3 {
            continue;
        }
        let mut prev = type_names;
        let ok = (0..n.abilities).all(|i| match rom.ptr(table + 4 * i) {
            Ok(Some(p)) if p > prev && p < latest - 3 => {
                prev = p;
                true
            }
            _ => false,
        });
        if !ok || rom.pointers_to(table).is_empty() {
            continue;
        }
        hits.extend((latest - 3..=latest).filter(|&o| !rom.pointers_to(o).is_empty()));
    }
    match hits[..] {
        [names] => Ok(names),
        _ => bail!("gAbilityNames: expected 1 referenced table after gTypeNames, found {hits:#x?}"),
    }
}

/// `gEvolutionTable`: the referenced table whose `SPECIES_NONE` row is empty, followed by
/// Bulbasaur's row (one `EVO_LEVEL` 16 to Ivysaur) and Ivysaur's first entry (32 to
/// Venusaur).
fn locate_evolutions(rom: &Rom, n: &Counts) -> Result<u32> {
    let sp = |name| value(&names::SPECIES, name) as u16;
    let level = EVO_METHODS.iter().position(|&m| m == "EVO_LEVEL").unwrap() as u16;
    let row = (EVOS_PER_MON * EVOLUTION_SIZE) as usize;
    let mut sig = vec![0u8; row];
    sig.extend(u16s_le(&[level, 16, sp("SPECIES_IVYSAUR")]));
    sig.resize(2 * row, 0);
    sig.extend(u16s_le(&[level, 32, sp("SPECIES_VENUSAUR")]));
    let table = unique_referenced(rom, &sig, "gEvolutionTable")?;
    rom.slice(table, row * n.species as usize)
        .context("gEvolutionTable")?;
    Ok(table)
}

/// `gPokedexEntries`: the referenced table whose first entries have the heights and weights of
/// [`DEX_ANCHOR`].
fn locate_pokedex_entries(rom: &Rom) -> Result<u32> {
    let (h, w) = DEX_ANCHOR[1];
    let mut hits = Vec::new();
    for at in rom.find_all(&u16s_le(&[h, w])) {
        let Some(start) = at.checked_sub(DEX_ENTRY_SIZE + DEX_ENTRY_HEIGHT) else {
            continue;
        };
        let ok = DEX_ANCHOR.iter().enumerate().all(|(i, &(h, w))| {
            rom.u16s(start + DEX_ENTRY_SIZE * i as u32 + DEX_ENTRY_HEIGHT, 2)
                .is_ok_and(|v| v == [h, w])
        });
        if ok && !rom.pointers_to(start).is_empty() {
            hits.push(start);
        }
    }
    match hits[..] {
        [t] => Ok(t),
        _ => bail!("gPokedexEntries: expected 1 referenced table, found {hits:#x?}"),
    }
}

/// `gTrainerMoneyTable`, at the first 4-byte boundary after `gTypeNames` (`battle_main.c`):
/// a referenced run of `{class, value}` entries of distinct known classes, ended by the `0xFF`
/// class.
fn locate_trainer_money(rom: &Rom, type_names: u32, n: &Counts) -> Result<u32> {
    let at = (type_names + TYPE_NAME_SIZE * n.types).next_multiple_of(4);
    ensure_referenced(rom, at, "gTrainerMoneyTable")?;
    read_money_pairs(rom, at, n).context("gTrainerMoneyTable after gTypeNames")?;
    Ok(at)
}

/// The `(class, value)` pairs of the money table at `at`, its `0xFF` end included.
fn read_money_pairs(rom: &Rom, at: u32, n: &Counts) -> Result<Vec<(u8, u8)>> {
    let mut pairs: Vec<(u8, u8)> = Vec::new();
    for i in 0..MAX_TRAINER_MONEY {
        let [class, v, pad0, pad1] = rom
            .slice(at + TRAINER_MONEY_SIZE * i, TRAINER_MONEY_SIZE as usize)?
            .try_into()
            .unwrap();
        ensure!(
            v > 0 && pad0 == 0 && pad1 == 0,
            "money table at {at:#x}: bad entry {i} (class {class}, value {v})"
        );
        if class == TRAINER_MONEY_END {
            pairs.push((class, v));
            ensure!(pairs.len() > 1, "money table at {at:#x}: empty");
            return Ok(pairs);
        }
        ensure!(
            u32::from(class) < n.trainer_classes && pairs.iter().all(|&(c, _)| c != class),
            "money table at {at:#x}: bad class {class}"
        );
        pairs.push((class, v));
    }
    bail!("money table at {at:#x}: no end")
}

/// Offset of the `TYPE_ENDTABLE` triple.
fn type_chart_end(rom: &Rom, start: u32) -> Result<u32> {
    let types = counts().types as u8;
    let mut at = start;
    loop {
        let [a, d, m] = rom.slice(at, 3)?.try_into().unwrap();
        if a == TYPE_ENDTABLE {
            return Ok(at);
        }
        ensure!(
            (a == TYPE_FORESIGHT && d == TYPE_FORESIGHT) || (a < types && d < types),
            "gTypeEffectiveness at {start:#x}: bad entry {a:#x} {d:#x} {m} at {at:#x}"
        );
        at += 3;
        ensure!(
            at - start < 3 * 1024,
            "gTypeEffectiveness at {start:#x}: no end"
        );
    }
}

/// A `WildPokemonHeader` that is not the terminator: a named map and four pointers, valid or
/// null, not all null.
fn wild_header_ok(rom: &Rom, at: u32) -> bool {
    let Ok(b) = rom.slice(at, WILD_HEADER_SIZE as usize) else {
        return false;
    };
    if names::map(b[0], b[1]).is_none() {
        return false;
    }
    let ptrs: Vec<u32> = (0..4)
        .map(|i| u32::from_le_bytes(b[4 + 4 * i..8 + 4 * i].try_into().unwrap()))
        .collect();
    ptrs.iter().all(|&p| p == 0 || is_rom_ptr(rom, p)) && ptrs.iter().any(|&p| p != 0)
}

fn locate_wild_headers(rom: &Rom) -> Result<u32> {
    let m = names::map_by_dir(WILD_ANCHOR_MAP).expect("wild anchor map");
    let key = [m.group, m.num];
    let hits: Vec<u32> = rom
        .find_all(&key)
        .into_iter()
        .filter(|&o| o % 4 == 0 && wild_header_ok(rom, o))
        .collect();
    let mut tables = Vec::new();
    for hit in hits {
        let mut start = hit;
        while start >= WILD_HEADER_SIZE && wild_header_ok(rom, start - WILD_HEADER_SIZE) {
            start -= WILD_HEADER_SIZE;
        }
        let mut end = hit;
        while wild_header_ok(rom, end) && end - start < MAX_WILD_HEADERS * WILD_HEADER_SIZE {
            end += WILD_HEADER_SIZE;
        }
        if rom.slice(end, 2)? == [0xFF, 0xFF] && !rom.pointers_to(start).is_empty() {
            tables.push(start);
        }
    }
    match tables[..] {
        [t] => Ok(t),
        _ => {
            bail!("gWildMonHeaders: expected 1 table holding {WILD_ANCHOR_MAP}, found {tables:#x?}")
        }
    }
}

fn locate_battle_strings(rom: &Rom, n: &Counts) -> Result<u32> {
    let len = n.strings - n.strings_start;
    let first = text::encode("{B_TRAINER1_LOSE_TEXT}")?;
    let words = rom.bytes().len() as u32 / 4;
    let is_ptr = |i: u32| is_rom_ptr(rom, rom.u32(i * 4).unwrap_or(0));
    let mut found = Vec::new();
    let mut i = 0;
    while i < words {
        if !is_ptr(i) {
            i += 1;
            continue;
        }
        let start = i;
        while i < words && is_ptr(i) {
            i += 1;
        }
        if i - start == len {
            let target = rom.u32(start * 4)? - ROM_BASE;
            if rom.slice(target, first.len()).ok() == Some(&first[..]) {
                found.push(start * 4);
            }
        }
    }
    match found[..] {
        [t] => Ok(t),
        _ => {
            bail!("gBattleStringsTable: expected 1 run of {len} string pointers, found {found:#x?}")
        }
    }
}

/// The offset of string `id` (`STRINGID_*`) in the table at `table`.
fn table_string(rom: &Rom, table: u32, n: &Counts, id: &str) -> Result<u32> {
    let v = value(&names::STRING_IDS, id);
    ensure!(v >= n.strings_start, "{id} is not in gBattleStringsTable");
    rom.req_ptr(table + 4 * (v - n.strings_start), id)
}

/// Bytes of the string at `off` (terminator included), stepping over control code arguments.
fn string_len(rom: &Rom, off: u32) -> Result<u32> {
    let bytes = rom.bytes().get(off as usize..).context("string offset")?;
    let (_, len) = text::decode(bytes).with_context(|| format!("string at {off:#x}"))?;
    Ok(len as u32)
}

/// Offsets of the [`CODE_STRINGS`] and of `gStatNamesTable`.
fn walk_code_strings(rom: &Rom, table: u32, n: &Counts) -> Result<(Vec<u32>, u32)> {
    let mut at = table_string(rom, table, n, CODE_STRINGS_AFTER)?;
    at += string_len(rom, at)?;
    let mut offsets = Vec::new();
    for name in CODE_STRINGS {
        offsets.push(at);
        at += string_len(rom, at).with_context(|| name.to_string())?;
    }
    let stat_names = at.next_multiple_of(4);
    let want = &offsets[offsets.len() - STAT_NAMES..];
    for (i, &w) in want.iter().enumerate() {
        let p = rom.req_ptr(stat_names + 4 * i as u32, "gStatNamesTable")?;
        ensure!(
            p == w,
            "gStatNamesTable at {stat_names:#x}: entry {i} is {p:#x}, the walk found {w:#x} \
             (battle_message.c strings out of order?)"
        );
    }
    Ok((offsets, stat_names))
}

fn locate_menu_strings(rom: &Rom) -> Result<u32> {
    let anchor = text::encode(MENU_ANCHOR_TEXT)?; // with its 0xFF
    let mut needle = anchor.clone();
    needle.extend_from_slice(&anchor[..anchor.len() - 1]);
    let hit = match rom.find_all(&needle)[..] {
        [o] => o,
        ref hits => bail!("gText_MoveInterfaceDynamicColors: expected 1 match, found {hits:#x?}"),
    };
    let mut start = hit;
    for _ in 0..MENU_ANCHOR {
        start = previous_string(rom, start).context("menu strings")?;
    }
    ensure_referenced(rom, start, "gText_WhatWillPkmnDo")?;
    // The string after the last one is `gText_BattleSwitchWhich2`: the anchor bytes again.
    let run = string_run(rom, start, MENU_STRINGS.len() + 1)?;
    let after = run[MENU_STRINGS.len()];
    ensure!(
        rom.slice(after, anchor.len())? == &anchor[..],
        "menu strings: the string after gText_BattleSwitchWhich at {after:#x} is not \
         gText_BattleSwitchWhich2"
    );
    Ok(start)
}

fn locate_confirm_starter(rom: &Rom) -> Result<u32> {
    let pat = parse_pattern(CONFIRM_STARTER_CODE);
    let code = match find_pattern(rom, &pat)[..] {
        [o] => o,
        ref hits => bail!("Task_AskConfirmStarter: expected 1 match, found {hits:#x?}"),
    };
    rom.req_ptr(code + CONFIRM_STARTER_LITERAL, "gText_ConfirmStarterChoice")
}

/// Offsets of `n` consecutive strings starting at `off`.
fn string_run(rom: &Rom, off: u32, n: usize) -> Result<Vec<u32>> {
    let mut out = Vec::with_capacity(n);
    let mut at = off;
    for _ in 0..n {
        out.push(at);
        at += string_len(rom, at)?;
    }
    Ok(out)
}

/// The string before the one at `off` (`off - 1` is its terminator).
fn previous_string(rom: &Rom, off: u32) -> Result<u32> {
    let before = rom.slice(0, off as usize - 1)?;
    let p = before
        .iter()
        .rposition(|&b| b == 0xFF)
        .context("no string before")? as u32
        + 1;
    ensure!(
        p + string_len(rom, p)? == off,
        "the string at {p:#x} does not end at {off:#x}"
    );
    Ok(p)
}

/// Field (`decode`) name -> battle name of the 0xFD placeholders both share (FD 00..0D).
fn battle_placeholders() -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for &(name, bytes) in charmap::CONSTANTS {
        if !(name.starts_with("B_") && bytes.len() == 2 && bytes[0] == 0xFD) {
            continue;
        }
        if let Some(&(_, field)) = charmap::SYMBOLS.iter().find(|(b, _)| *b == bytes)
            && field != name
        {
            out.push((format!("{{{field}}}"), name));
        }
    }
    out
}

/// Decodes the battle string at `off` (its 0xFD placeholders by their `B_*` names) and checks
/// that it re-encodes to the ROM bytes.
pub fn battle_string_at(rom: &Rom, off: u32) -> Result<String> {
    let field = text::round_trip_at(rom, off)?;
    let mut s = field;
    for (from, to) in battle_placeholders() {
        s = s.replace(&from, &format!("{{{to}}}"));
    }
    let bytes = text::encode_lang(&s, rom.lang())?;
    let want = rom.slice(off, bytes.len())?;
    ensure!(
        bytes == want,
        "battle string at {off:#x} ({s:?}) does not re-encode"
    );
    Ok(s)
}

/// Whether a wild/foe tag (`sText_WildPkmnPrefix`, `sText_FoePkmnPrefix`) goes after the
/// nickname: BPEF's tags start with a space and are appended, BPEE's `"Wild "` ends with one and
/// is prepended.
pub fn tag_is_suffix(tag: &str) -> Result<bool> {
    match (tag.starts_with(' '), tag.ends_with(' ')) {
        (true, false) => Ok(true),
        (false, true) => Ok(false),
        _ => bail!("cannot tell whether the tag {tag:?} is a prefix or a suffix"),
    }
}

/// Whether `STRINGID_STATSHARPLY` and `STRINGID_STATHARSHLY` are complete messages (BPEF: they
/// end in `!` and the "rose!"/"fell!" buffer entry after them is skipped) or adverbs that the
/// next entry completes (BPEE). See the module docs.
pub fn stat_sharply_is_complete(strings: &BattleStringsFile) -> Result<bool> {
    let ends = |id: &str| -> Result<bool> {
        let s = strings.get(id).with_context(|| format!("no {id}"))?;
        Ok(s.ends_with('!'))
    };
    let (sharply, harshly) = (ends("STRINGID_STATSHARPLY")?, ends("STRINGID_STATHARSHLY")?);
    ensure!(
        sharply == harshly,
        "STRINGID_STATSHARPLY and STRINGID_STATHARSHLY disagree on being complete"
    );
    Ok(sharply)
}

/// `strings.ron` keys of `gStatNamesTable`, index = `STAT_*`.
pub const STAT_NAME_KEYS: [&str; STAT_NAMES] = [
    "gStatNamesTable_0",
    "gStatNamesTable_1",
    "gStatNamesTable_2",
    "gStatNamesTable_3",
    "gStatNamesTable_4",
    "gStatNamesTable_5",
    "gStatNamesTable_6",
    "gStatNamesTable_7",
];

/// `gExperienceTables`: one row per `GROWTH_*`, exp for levels 0..=100.
pub fn read_experience_tables(rom: &Rom, off: u32) -> Result<Vec<Vec<u32>>> {
    (0..GROWTH_RATES)
        .map(|g| {
            (0..=MAX_LEVEL)
                .map(|l| rom.u32(off + 4 * (g * (MAX_LEVEL + 1) + l) as u32))
                .collect()
        })
        .collect()
}

/// Everything exported under `battle/`.
#[derive(Clone, Debug, PartialEq)]
pub struct BattleData {
    pub species: SpeciesFile,
    pub moves: MovesFile,
    pub types: TypesFile,
    pub trainers: TrainersFile,
    pub encounters: EncountersFile,
    pub evolutions: EvolutionsFile,
    pub trainer_money: TrainerMoneyFile,
    pub heal_locations: HealLocationsFile,
    pub strings: BattleStringsFile,
    /// `battle/<table>` -> offset, for the manifest.
    pub provenance: BTreeMap<String, Hex>,
}

fn fixed_name(rom: &Rom, off: u32, size: u32) -> Result<String> {
    let s = text::round_trip_at(rom, off)?;
    ensure!(
        string_len(rom, off)? <= size,
        "name at {off:#x} ({s:?}) overflows its {size} bytes"
    );
    Ok(s)
}

fn bits(t: &names::Table, v: u32) -> Vec<String> {
    (0..32)
        .filter(|b| v & (1 << b) != 0)
        .map(|b| t.name_or_hex(1 << b))
        .collect()
}

/// Reads every table of [`locate`]; encounters for the headers of `maps` (decomp directory
/// names) only.
pub fn extract(rom: &Rom, maps: &[&str]) -> Result<BattleData> {
    let loc = locate(rom)?;
    let n = counts();
    let mut provenance = BTreeMap::new();
    for (k, v) in [
        ("species_info", loc.species_info),
        ("species_names", loc.species_names),
        ("learnsets", loc.learnsets),
        ("moves", loc.moves),
        ("move_names", loc.move_names),
        ("type_chart", loc.type_chart),
        ("type_names", loc.type_names),
        ("ability_names", loc.ability_names),
        ("trainers", loc.trainers),
        ("trainer_class_names", loc.trainer_class_names),
        ("experience_tables", loc.experience_tables),
        ("wild_headers", loc.wild_headers),
        ("evolutions", loc.evolutions),
        ("trainer_money", loc.trainer_money),
        ("species_to_national", loc.species_to_national),
        ("pokedex_entries", loc.pokedex_entries),
        ("nature_power", loc.nature_power),
        ("heal_locations", loc.heal_locations),
        ("strings", loc.battle_strings),
        ("stat_names", loc.stat_names),
        ("menu_strings", loc.menu_strings),
        ("confirm_starter_choice", loc.confirm_starter_choice),
    ] {
        provenance.insert(format!("battle/{k}"), Hex(v));
    }
    Ok(BattleData {
        species: read_species(rom, &loc, &n)?,
        moves: read_moves(rom, &loc, &n)?,
        types: read_types(rom, &loc, &n)?,
        trainers: read_trainers(rom, &loc, &n)?,
        encounters: read_encounters(rom, &loc, maps)?,
        evolutions: read_evolutions(rom, &loc, &n)?,
        trainer_money: TrainerMoneyFile {
            classes: read_money_pairs(rom, loc.trainer_money, &n)?
                .into_iter()
                .map(|(c, v)| (names::TRAINER_CLASSES.name_or_hex(c.into()), v))
                .collect(),
        },
        heal_locations: read_heal_locations(rom, &loc, &n)?,
        strings: read_strings(rom, &loc, &n)?,
        provenance,
    })
}

impl BattleData {
    /// Writes `battle/*.ron`.
    pub fn write(&self, o: &mut Out) -> Result<()> {
        o.ron("battle/species.ron", &self.species)?;
        o.ron("battle/moves.ron", &self.moves)?;
        o.ron("battle/types.ron", &self.types)?;
        o.ron("battle/trainers.ron", &self.trainers)?;
        o.ron("battle/encounters.ron", &self.encounters)?;
        o.ron("battle/evolutions.ron", &self.evolutions)?;
        o.ron("battle/trainer_money.ron", &self.trainer_money)?;
        o.ron("battle/heal_locations.ron", &self.heal_locations)?;
        o.ron("battle/strings.ron", &self.strings)
    }
}

fn read_species(rom: &Rom, loc: &Locations, n: &Counts) -> Result<SpeciesFile> {
    let mut species = Vec::with_capacity(n.species as usize);
    for i in 0..n.species {
        let id = names::SPECIES.name_or_hex(i);
        let ctx = || id.clone();
        let at = loc.species_info + SPECIES_INFO_SIZE * i;
        let b = rom.slice(at, SPECIES_INFO_SIZE as usize)?;
        let ev = u16::from_le_bytes([b[0x0A], b[0x0B]]);
        let ty = |v: u8| -> Result<String> {
            ensure!(u32::from(v) < n.types, "{}: type {v}", ctx());
            Ok(names::TYPES.name_or_hex(v.into()))
        };
        let learnset_at = rom
            .req_ptr(loc.learnsets + 4 * i, "learnset")
            .with_context(ctx)?;
        let mut learnset = Vec::new();
        for k in 0.. {
            ensure!(k < MAX_LEARNSET, "{}: learnset has no end", ctx());
            let v = rom.u16(learnset_at + 2 * k as u32)?;
            if v == LEARNSET_END {
                break;
            }
            learnset.push((
                (v >> 9) as u8,
                names::MOVES.name_or_hex(u32::from(v & 0x1FF)),
            ));
        }
        ensure!(
            usize::from(b[0x13]) < GROWTH_RATES,
            "{}: growth {}",
            ctx(),
            b[0x13]
        );
        species.push(Species {
            name: fixed_name(
                rom,
                loc.species_names + SPECIES_NAME_SIZE * i,
                SPECIES_NAME_SIZE,
            )
            .with_context(ctx)?,
            id: id.clone(),
            base: b[0..6].try_into().unwrap(),
            types: [ty(b[6])?, ty(b[7])?],
            catch_rate: b[8],
            exp_yield: b[9],
            ev_yield: std::array::from_fn(|k| ((ev >> (2 * k)) & 3) as u8),
            gender_ratio: b[0x10],
            friendship: b[0x12],
            growth: names::GROWTH_RATES.name_or_hex(b[0x13].into()),
            abilities: [
                names::ABILITIES.name_or_hex(b[0x16].into()),
                names::ABILITIES.name_or_hex(b[0x17].into()),
            ],
            learnset,
            weight: species_weight(rom, loc, i).with_context(ctx)?,
        });
    }
    Ok(SpeciesFile { species })
}

/// `gPokedexEntries[SpeciesToNationalPokedexNum(species)].weight`, 0 without an entry
/// (`SPECIES_NONE`, and the old Unown placeholders, numbered past `NATIONAL_DEX_COUNT`).
fn species_weight(rom: &Rom, loc: &Locations, species: u32) -> Result<u16> {
    let Some(i) = species.checked_sub(1) else {
        return Ok(0);
    };
    let national = u32::from(rom.u16(loc.species_to_national + 2 * i)?);
    if national == 0 || national > NATIONAL_DEX_COUNT {
        return Ok(0);
    }
    rom.u16(loc.pokedex_entries + DEX_ENTRY_SIZE * national + DEX_ENTRY_HEIGHT + 2)
}

fn read_moves(rom: &Rom, loc: &Locations, n: &Counts) -> Result<MovesFile> {
    let mut moves = Vec::with_capacity(n.moves as usize);
    for i in 0..n.moves {
        let id = names::MOVES.name_or_hex(i);
        let b = rom.slice(loc.moves + MOVE_SIZE * i, MOVE_SIZE as usize)?;
        ensure!(u32::from(b[2]) < n.types, "{id}: type {}", b[2]);
        moves.push(Move {
            name: fixed_name(rom, loc.move_names + MOVE_NAME_SIZE * i, MOVE_NAME_SIZE)
                .with_context(|| id.clone())?,
            id,
            effect: names::MOVE_EFFECTS.name_or_hex(b[0].into()),
            power: b[1],
            ty: names::TYPES.name_or_hex(b[2].into()),
            accuracy: b[3],
            pp: b[4],
            chance: b[5],
            target: names::MOVE_TARGETS.name_or_hex(b[6].into()),
            priority: b[7] as i8,
            flags: bits(&names::MOVE_FLAGS, b[8].into()),
        });
    }
    let nature_power = rom
        .u16s(loc.nature_power, NATURE_POWER.len())?
        .into_iter()
        .map(|m| names::MOVES.name_or_hex(m.into()))
        .collect();
    Ok(MovesFile {
        moves,
        nature_power,
    })
}

fn read_types(rom: &Rom, loc: &Locations, n: &Counts) -> Result<TypesFile> {
    let names = (0..n.types)
        .map(|i| fixed_name(rom, loc.type_names + TYPE_NAME_SIZE * i, TYPE_NAME_SIZE))
        .collect::<Result<_>>()?;
    let end = type_chart_end(rom, loc.type_chart)?;
    let mut chart = Vec::new();
    let mut foresight_split = None;
    for at in (loc.type_chart..end).step_by(3) {
        let [a, d, m] = rom.slice(at, 3)?.try_into().unwrap();
        if a == TYPE_FORESIGHT {
            ensure!(foresight_split.is_none(), "two TYPE_FORESIGHT separators");
            foresight_split = Some(chart.len());
            continue;
        }
        chart.push((
            names::TYPES.name_or_hex(a.into()),
            names::TYPES.name_or_hex(d.into()),
            m,
        ));
    }
    let ability_names = (0..n.abilities)
        .map(|i| {
            fixed_name(
                rom,
                loc.ability_names + ABILITY_NAME_SIZE * i,
                ABILITY_NAME_SIZE,
            )
        })
        .collect::<Result<_>>()?;
    Ok(TypesFile {
        names,
        ability_names,
        foresight_split: foresight_split.context("gTypeEffectiveness has no TYPE_FORESIGHT")?,
        chart,
    })
}

fn read_trainers(rom: &Rom, loc: &Locations, n: &Counts) -> Result<TrainersFile> {
    let class_names: Vec<String> = (0..n.trainer_classes)
        .map(|i| {
            fixed_name(
                rom,
                loc.trainer_class_names + CLASS_NAME_SIZE * i,
                CLASS_NAME_SIZE,
            )
        })
        .collect::<Result<_>>()?;
    let mut trainers = Vec::with_capacity(n.trainers as usize);
    for i in 0..n.trainers {
        let id = names::TRAINERS.name_or_hex(i);
        let at = loc.trainers + TRAINER_SIZE * i;
        let b = rom.slice(at, TRAINER_SIZE as usize)?;
        let flags = b[0];
        ensure!(flags < 4, "{id}: party flags {flags:#x}");
        let class = b[1];
        let class_name = class_names
            .get(usize::from(class))
            .with_context(|| format!("{id}: class {class}"))?
            .clone();
        let name = text::round_trip_at(rom, at + 4).with_context(|| id.clone())?;
        ensure!(
            string_len(rom, at + 4)? as usize <= TRAINER_NAME_SIZE,
            "{id}: name overflows"
        );
        let size = usize::from(b[0x20]);
        let party_at = rom.ptr(at + 0x24)?;
        let mon_size: u32 = if flags & PARTY_CUSTOM_MOVES != 0 {
            16
        } else {
            8
        };
        let mut party = Vec::with_capacity(size);
        if size > 0 {
            let p = party_at.with_context(|| format!("{id}: null party"))?;
            for k in 0..size as u32 {
                let m = p + mon_size * k;
                let species = rom.u16(m + 4)?;
                ensure!(
                    u32::from(species) < n.species,
                    "{id}: party species {species}"
                );
                let item_at = m + 6;
                let moves_at = if flags & PARTY_HELD_ITEM != 0 {
                    m + 8
                } else {
                    m + 6
                };
                party.push(TrainerMon {
                    iv: rom.u16(m)?,
                    level: rom.u8(m + 2)?,
                    species: names::SPECIES.name_or_hex(species.into()),
                    item: (flags & PARTY_HELD_ITEM != 0)
                        .then(|| rom.u16(item_at).map(names::item_name))
                        .transpose()?,
                    moves: (flags & PARTY_CUSTOM_MOVES != 0)
                        .then(|| {
                            rom.u16s(moves_at, 4).map(|ms| {
                                ms.into_iter()
                                    .map(|v| names::MOVES.name_or_hex(v.into()))
                                    .collect()
                            })
                        })
                        .transpose()?,
                });
            }
        }
        trainers.push(Trainer {
            id,
            class: names::TRAINER_CLASSES.name_or_hex(class.into()),
            class_name,
            name,
            pic: names::TRAINER_PICS.name_or_hex(b[3].into()),
            female: b[2] & TRAINER_FEMALE != 0,
            ai: bits(&names::AI_FLAGS, rom.u32(at + 0x1C)?),
            double: b[0x18] != 0,
            party,
        });
    }
    Ok(TrainersFile { trainers })
}

/// One `WildPokemonInfo` (`{u8 encounterRate; const struct WildPokemon *wildPokemon;}`) of
/// `slots` mons, or `None` for a null pointer.
fn read_wild_table(rom: &Rom, info: Option<u32>, slots: u32) -> Result<Option<LandEncounters>> {
    let Some(info) = info else {
        return Ok(None);
    };
    let rate = rom.u8(info)?;
    let mons_at = rom.req_ptr(info + 4, "wild mons")?;
    let mut mons = Vec::with_capacity(slots as usize);
    for k in 0..slots {
        let w = mons_at + 4 * k;
        let species = rom.u16(w + 2)?;
        ensure!(
            u32::from(species) < counts().species,
            "wild mon at {w:#x}: species {species}"
        );
        mons.push(WildMon {
            min_level: rom.u8(w)?,
            max_level: rom.u8(w + 1)?,
            species: names::SPECIES.name_or_hex(species.into()),
        });
    }
    Ok(Some(LandEncounters { rate, mons }))
}

/// Every `gWildMonHeaders` entry of `maps`, in table order per map (Altering Cave has 9, one per
/// `VAR_ALTERING_CAVE_WILD_SET` value).
fn read_encounters(rom: &Rom, loc: &Locations, maps: &[&str]) -> Result<EncountersFile> {
    let mut out: BTreeMap<String, Vec<EncounterHeader>> = BTreeMap::new();
    let mut at = loc.wild_headers;
    while wild_header_ok(rom, at) {
        let m = names::map(rom.u8(at)?, rom.u8(at + 1)?).expect("checked");
        if maps.contains(&m.dir) {
            let table = |i: u32| -> Result<Option<LandEncounters>> {
                read_wild_table(rom, rom.ptr(at + 4 + 4 * i)?, WILD_SLOTS[i as usize])
                    .with_context(|| format!("{}: wild header at {at:#x}", m.dir))
            };
            out.entry(m.dir.to_string())
                .or_default()
                .push(EncounterHeader {
                    land: table(0)?,
                    water: table(1)?,
                    rock_smash: table(2)?,
                    fishing: table(3)?,
                });
        }
        at += WILD_HEADER_SIZE;
    }
    Ok(EncountersFile { maps: out })
}

/// `gEvolutionTable`: the non-empty slots of each species, which must come first in its row.
fn read_evolutions(rom: &Rom, loc: &Locations, n: &Counts) -> Result<EvolutionsFile> {
    let mut out = EvolutionsFile::default();
    for i in 0..n.species {
        let id = names::SPECIES.name_or_hex(i);
        let mut evos = Vec::new();
        let mut ended = false;
        for k in 0..EVOS_PER_MON {
            let at = loc.evolutions + (i * EVOS_PER_MON + k) * EVOLUTION_SIZE;
            let [method, param, target]: [u16; 3] = rom.u16s(at, 3)?.try_into().unwrap();
            if method == 0 {
                ensure!(
                    param == 0 && target == 0,
                    "{id}: empty evolution slot {k} at {at:#x} holds data"
                );
                ended = true;
                continue;
            }
            ensure!(!ended, "{id}: evolution slot {k} after an empty one");
            let name = EVO_METHODS
                .get(usize::from(method))
                .with_context(|| format!("{id}: evolution method {method} at {at:#x}"))?;
            ensure!(
                u32::from(target) < n.species && target != 0,
                "{id}: evolution target {target} at {at:#x}"
            );
            evos.push((
                name.to_string(),
                param,
                names::SPECIES.name_or_hex(target.into()),
            ));
        }
        if !evos.is_empty() {
            out.species.insert(id, evos);
        }
    }
    Ok(out)
}

fn read_heal_locations(rom: &Rom, loc: &Locations, n: &Counts) -> Result<HealLocationsFile> {
    let mut out = BTreeMap::new();
    for i in 0..n.heal_locations {
        let at = loc.heal_locations + HEAL_LOCATION_SIZE * i;
        let id = names::HEAL_LOCATIONS.name_or_hex(i + 1);
        let (group, num) = (rom.u8(at)?, rom.u8(at + 1)?);
        let map =
            names::map(group, num).with_context(|| format!("{id}: unknown map {group}.{num}"))?;
        out.insert(
            id,
            (map.dir.to_string(), rom.i16(at + 2)?, rom.i16(at + 4)?),
        );
    }
    Ok(out)
}

fn read_strings(rom: &Rom, loc: &Locations, n: &Counts) -> Result<BattleStringsFile> {
    let mut out = BTreeMap::new();
    for id in n.strings_start..n.strings {
        let name = names::STRING_IDS.name_or_hex(id);
        let off = rom.req_ptr(loc.battle_strings + 4 * (id - n.strings_start), &name)?;
        out.insert(
            name.clone(),
            battle_string_at(rom, off).with_context(|| name)?,
        );
    }
    let code = &loc.code_strings;
    for (name, &off) in CODE_STRINGS.iter().zip(code) {
        out.insert(
            name.to_string(),
            battle_string_at(rom, off).with_context(|| name.to_string())?,
        );
    }
    let menu = string_run(rom, loc.menu_strings, MENU_STRINGS.len())?;
    for (name, off) in MENU_STRINGS.iter().zip(menu) {
        out.insert(
            name.to_string(),
            battle_string_at(rom, off).with_context(|| name.to_string())?,
        );
    }
    let birch = previous_string(rom, loc.confirm_starter_choice)?;
    ensure_referenced(rom, birch, "gText_BirchInTrouble")?;
    for (name, off) in [
        ("gText_BirchInTrouble", birch),
        ("gText_ConfirmStarterChoice", loc.confirm_starter_choice),
    ] {
        out.insert(
            name.to_string(),
            text::round_trip_at(rom, off).with_context(|| name.to_string())?,
        );
    }
    for tag in ["sText_WildPkmnPrefix", "sText_FoePkmnPrefix"] {
        tag_is_suffix(&out[tag]).with_context(|| tag.to_string())?;
    }
    let stats = &CODE_STRINGS[CODE_STRINGS.len() - STAT_NAMES..];
    for (key, name) in STAT_NAME_KEYS.iter().zip(stats) {
        let s = out[*name].clone();
        out.insert(key.to_string(), s);
    }
    stat_sharply_is_complete(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_map_field_names_to_battle_names() {
        let map = battle_placeholders();
        assert!(map.contains(&("{PLAYER}".to_string(), "B_BUFF2")));
        assert!(map.contains(&("{GROUDON}".to_string(), "B_ATK_NAME_WITH_PREFIX_MON1")));
        assert_eq!(map.len(), 13, "FD 01..0D: {map:?}");
    }

    #[test]
    fn tags() {
        assert!(tag_is_suffix(" tag").unwrap());
        assert!(!tag_is_suffix("Wild ").unwrap());
        assert!(tag_is_suffix("Foe").is_err());
    }

    #[test]
    fn patterns_have_wildcards() {
        let p = parse_pattern("10 b5 ?? 00");
        assert_eq!(p, [Some(0x10), Some(0xB5), None, Some(0)]);
        assert_eq!(parse_pattern(CONFIRM_STARTER_CODE).len(), 0x60);
    }

    #[test]
    fn code_strings_end_with_the_stat_names() {
        assert_eq!(CODE_STRINGS[CODE_STRINGS.len() - STAT_NAMES], "sText_HP2");
        assert_eq!(
            MENU_STRINGS[MENU_ANCHOR],
            "gText_MoveInterfaceDynamicColors"
        );
    }
}
