//! Battle content (Milestone 4): `BattleTables`, trainers, encounters, heal locations, battle
//! strings and `BattleGfx`, read from `content/extracted/{battle,pokemon,trainers,starter,font}`
//! (`crate::battle_load::load`) or built in ([`crate::stub::battle`]), and generated as
//! `$OUT_DIR/battle.rs` plus one aligned blob, `$OUT_DIR/battle.bin`, that holds every tile,
//! palette, tilemap and glyph (`include_bytes!`, sliced by `content::blob::Blob`).
//!
//! The tables are complete (every species, move and trainer, so ids are Gen3 numbers). Pics are
//! built for every species and trainer pic the extract has; a species without pics battles
//! without them (`gfx: None`), and a trainer whose pic is missing is left out (`trainer()` is
//! `None`). Nothing here depends on which maps were exported (Milestone 8 Task E).

use crate::blob_data::BlobOut;
use crate::classify::{self, StrSrc};
use crate::codegen::{bytes_lit, ident_ok};
use crate::input::{Glyph, Tile, World, warn};
use anyhow::{Context, Result, bail, ensure};
use extract::battle_data::{STAT_NAME_KEYS, TRAINER_MONEY_DEFAULT_CLASS, stat_sharply_is_complete};
use extract::names::{self, Table};
use extract::schema::{
    BattleStringsFile, EncounterHeader, EvolutionsFile, HealLocationsFile, MonCoords, Move,
    MovesFile, Species, Trainer, TrainerMoneyFile, TypesFile,
};
use extract::text::{Lang, encode_lang};
use std::collections::BTreeMap;
use std::fmt::Write;

/// `sStarterMon` (`starter_choose.c`).
pub const STARTERS: [&str; 3] = ["SPECIES_TREECKO", "SPECIES_TORCHIC", "SPECIES_MUDKIP"];
/// `CB2_StartFirstBattle` (`battle_setup.c`): Zigzagoon, level 2.
pub const FIRST_FOE: (&str, u8) = ("SPECIES_ZIGZAGOON", 2);
/// The player's back pic (`TRAINER_BACK_PIC_BRENDAN`: the player is male).
pub const PLAYER_BACK: &str = "TRAINER_BACK_PIC_BRENDAN";

/// A graphic of `BattleGfx` (`engine::battle::GfxImage`).
#[derive(Clone, Default)]
pub struct GfxIn {
    pub tiles: Vec<Tile>,
    pub palettes: Vec<[u16; 16]>,
    pub tilemap: Vec<u16>,
}

/// A trainer pic (`engine::mon::TrainerPic`); `y` is the Gen3 `.size`.
#[derive(Clone)]
pub struct PicIn {
    pub tiles: Vec<Tile>,
    pub palette: [u16; 16],
    pub y: u8,
}

/// A species' battle pics (`engine::mon::MonGfx`).
#[derive(Clone)]
pub struct MonGfxIn {
    pub front: Vec<Tile>,
    pub back: Vec<Tile>,
    pub palette: [u16; 16],
    pub shiny: [u16; 16],
    pub coords: MonCoords,
}

/// The interface graphics, by `BattleGfx` field.
#[derive(Clone, Default)]
pub struct UiGfx {
    pub textbox: GfxIn,
    /// By `Environment as usize`; empty (no environment) in the stub.
    pub environments: Vec<GfxIn>,
    pub healthbox_player: GfxIn,
    pub healthbox_opponent: GfxIn,
    pub hpbar: GfxIn,
    pub expbar: GfxIn,
    pub status: GfxIn,
    pub misc: GfxIn,
    pub ball: GfxIn,
    pub starter_bag: GfxIn,
    pub starter_ball: GfxIn,
    pub starter_circle: GfxIn,
}

pub struct BattleIn {
    /// Every species, index = `SPECIES_*` number.
    pub species: Vec<Species>,
    /// Every move, index = `MOVE_*` number.
    pub moves: MovesFile,
    pub types: TypesFile,
    /// Every trainer, index = `TRAINER_*` number.
    pub trainers: Vec<Trainer>,
    /// Every wild header of each map, by map name, in `gWildMonHeaders` order.
    pub encounters: BTreeMap<String, Vec<EncounterHeader>>,
    pub heal: HealLocationsFile,
    pub strings: BattleStringsFile,
    /// The species pics the extract has, by `SPECIES_*`.
    pub mon_gfx: BTreeMap<String, MonGfxIn>,
    /// The trainer front pics the extract has, by `TRAINER_PIC_*`.
    pub trainer_pics: BTreeMap<String, PicIn>,
    /// `gEvolutionTable` (empty before format 5.1).
    pub evolutions: EvolutionsFile,
    /// `gTrainerMoneyTable` by class (empty before format 5.1).
    pub trainer_money: TrainerMoneyFile,
    pub player_back: PicIn,
    pub ui: UiGfx,
    pub small_glyphs: Vec<Glyph>,
    pub small_widths: Vec<u8>,
    pub starters: [String; 3],
    pub first_foe: (String, u8),
    /// Built in (`crate::stub::battle`): sets `cfg(pkr_stub_battle)`.
    pub stub: bool,
    /// `MOVE_*` of Struggle.
    pub struggle: String,
}

// ---- generation ----

/// A `GfxImage` expression, its data in the blob.
fn image(blob: &mut BlobOut, g: &GfxIn) -> String {
    format!(
        "GfxImage {{ tiles: {}, palettes: {}, tilemap: {} }}",
        blob.tiles(&g.tiles),
        blob.pals(&g.palettes),
        blob.u16s(&g.tilemap)
    )
}

/// A `TrainerPic` expression, its data in the blob.
fn pic(blob: &mut BlobOut, p: &PicIn) -> String {
    format!(
        "TrainerPic {{ tiles: {}, palette: {}, y: {} }}",
        blob.tiles(&p.tiles),
        blob.pal(&p.palette),
        p.y
    )
}

pub struct Out {
    /// `battle.rs`.
    pub rs: String,
    /// `battle.bin`.
    pub bin: Vec<u8>,
    /// `SPECIES_*` / `MOVE_*` / `TRAINER_*` / `HEAL_LOCATION_*` constants: (name, type, value).
    pub consts: Vec<(String, &'static str, String)>,
    /// What the build degraded (for `content::REPORT`), one item per line.
    pub report: String,
}

/// The number of an id: its position in the exported table, which must agree with the names
/// table. Only the stub has names the table lacks (`SPECIES_STUB_A`, …).
fn numbered<'a>(
    ids: impl Iterator<Item = &'a str>,
    table: &Table,
    stub: bool,
) -> Result<BTreeMap<&'a str, u16>> {
    let mut out = BTreeMap::new();
    for (i, id) in ids.enumerate() {
        ensure!(ident_ok(id), "{id:?} is not an identifier");
        ensure!(
            stub || table.value(id).is_some(),
            "{id} is not in the {}* names table",
            table.prefix
        );
        if let Some(v) = table.value(id) {
            ensure!(
                v as usize == i,
                "{id} is number {i}, the names table says {v}"
            );
        }
        out.insert(id, i as u16);
    }
    Ok(out)
}

fn get(m: &BTreeMap<&str, u16>, name: &str) -> Result<u16> {
    m.get(name)
        .copied()
        .with_context(|| format!("unknown {name}"))
}

/// Gen3 map number of a map name: an exported map, else the names table (an authored game's
/// own numbers instead: `AuthoredIn::map_numbers`).
fn map_number(w: &World, name: &str) -> Option<u16> {
    if let Some(a) = &w.authored {
        return a.map_numbers.get(name).copied();
    }
    if let Some(m) = w.maps.iter().find(|m| m.name == name) {
        return Some(u16::from(m.group) << 8 | u16::from(m.num));
    }
    names::map_by_dir(name).map(|m| u16::from(m.group) << 8 | u16::from(m.num))
}

fn ty(name: &str) -> Result<&'static str> {
    classify::type_variant(name).with_context(|| format!("unknown type {name}"))
}

/// The bytes of one `StrSrc` (`TypeNames` / `StatNames` excluded).
fn str_text(b: &BattleIn, src: StrSrc) -> Result<String> {
    let s = |k: &str| {
        b.strings
            .get(k)
            .cloned()
            .with_context(|| format!("battle/strings.ron has no {k}"))
    };
    Ok(match src {
        StrSrc::Key(k) => s(k)?,
        StrSrc::UsedMove => {
            let used = s("sText_AttackerUsedX")?;
            ensure!(
                used.contains("{B_BUFF2}"),
                "sText_AttackerUsedX has no {{B_BUFF2}}"
            );
            let end = s("sText_ExclamationMark5")?;
            for k in ["", "2", "3", "4"] {
                if s(&format!("sText_ExclamationMark{k}"))? != end {
                    warn(&format!(
                        "sText_ExclamationMark{k} differs from sText_ExclamationMark5: moves of sGrammarMoveUsedTable end with ExclamationMark5 too"
                    ));
                }
            }
            used.replace("{B_BUFF2}", "{B_CURRENT_MOVE}") + &end
        }
        StrSrc::Sharply { adverb, verb } => {
            if stat_sharply_is_complete(&b.strings)? {
                s(adverb)?
            } else {
                s(adverb)? + &s(verb)?
            }
        }
        StrSrc::Buff1 { key, code } => {
            let t = s(key)?;
            ensure!(t.contains(code), "{key} has no {code}");
            t.replace(code, "{B_BUFF1}")
        }
        StrSrc::TypeNames | StrSrc::StatNames | StrSrc::AbilityNames => bail!("indexed source"),
        StrSrc::Missing => bail!("no Gen3 string"),
    })
}

/// The generator's state: the input, id numbers, and the output so far.
struct Gen<'a> {
    b: &'a BattleIn,
    w: &'a World,
    species_no: BTreeMap<&'a str, u16>,
    move_no: BTreeMap<&'a str, u16>,
    trainer_no: BTreeMap<&'a str, u16>,
    blob: BlobOut,
    /// `battle.rs`. The blob static is written last (its size is known then); statics may
    /// refer to it before.
    o: String,
    /// `EFFECT_*` the engine does not implement -> the moves that have it.
    unsupported_effects: BTreeMap<String, Vec<String>>,
    /// `ABILITY_*` folded into `Ability::Other` -> how many species have it.
    unsupported_abilities: BTreeMap<String, usize>,
    trainer_ids: Vec<u16>,
    /// Trainers left out: no front pic, or an empty party.
    skipped_trainers: Vec<String>,
    /// The flavour's battle switches (`AuthoredIn::rules`, else Emerald's).
    rules: classify::Rules,
}

impl Gen<'_> {
    fn enc(&self, s: &str, what: &str) -> Result<Vec<u8>> {
        encode_lang(s, self.w.lang).with_context(|| what.to_string())
    }

    fn species_id(&self, n: &str) -> Result<String> {
        Ok(format!("SpeciesId({})", get(&self.species_no, n)?))
    }

    fn move_id(&self, n: &str) -> Result<String> {
        Ok(format!("MoveId({})", get(&self.move_no, n)?))
    }

    /// `SPECIES`: every species, with learnsets and the pics the extract has.
    fn gen_species(&mut self) -> Result<()> {
        let b = self.b;
        let mut species = String::new();
        let mut ability_ids = String::new();
        for (i, s) in b.species.iter().enumerate() {
            let ids = s.abilities.each_ref().map(|a| {
                names::ABILITIES
                    .value(a)
                    .filter(|&v| v < b.types.ability_names.len() as u32)
                    .with_context(|| format!("{}: unknown ability {a}", s.id))
            });
            let [a0, a1] = ids;
            write!(ability_ids, "[{}, {}],", a0?, a1?)?;
            let mut ls = String::new();
            for (level, m) in &s.learnset {
                write!(
                    ls,
                    "LevelUpMove {{ level: {level}, mov: {} }},",
                    self.move_id(m)?
                )?;
            }
            writeln!(
                self.o,
                "static LS_{i}: [LevelUpMove; {}] = [{ls}];",
                s.learnset.len()
            )?;
            let gfx = match b.mon_gfx.get(&s.id) {
                Some(g) => {
                    let line = format!(
                        "static G_{i}: MonGfx = MonGfx {{ front: {}, back: {}, palette: {}, shiny: {}, front_y: {}, back_y: {}, elevation: {} }};",
                        self.blob.tiles(&g.front),
                        self.blob.tiles(&g.back),
                        self.blob.pal(&g.palette),
                        self.blob.pal(&g.shiny),
                        g.coords.front_y,
                        g.coords.back_y,
                        g.coords.elevation
                    );
                    writeln!(self.o, "{line}")?;
                    format!("Some(&G_{i})")
                }
                None => "None".into(),
            };
            let abilities = s.abilities.each_ref().map(|a| {
                let v = classify::ability_with(a, self.rules);
                if v == "Ability::Other" && a != "ABILITY_NONE" {
                    *self.unsupported_abilities.entry(a.clone()).or_default() += 1;
                }
                v
            });
            writeln!(
                species,
                "    SpeciesInfo {{ name: {}, base: {:?}, types: [{}, {}], catch_rate: {}, exp_yield: {}, ev_yield: {:?}, gender_ratio: {}, friendship: {}, growth: {}, abilities: [{}, {}], learnset: &LS_{i}, weight: {}, gfx: {gfx} }}, // {}",
                bytes_lit(&self.enc(&s.name, &s.id)?),
                s.base,
                ty(&s.types[0])?,
                ty(&s.types[1])?,
                s.catch_rate,
                s.exp_yield,
                s.ev_yield,
                s.gender_ratio,
                s.friendship,
                classify::growth(&s.growth)
                    .with_context(|| format!("{}: growth {}", s.id, s.growth))?,
                abilities[0],
                abilities[1],
                s.weight,
                s.id
            )?;
        }
        for s in b.starters.iter().chain([&b.first_foe.0]) {
            ensure!(
                self.species_no.contains_key(s.as_str()),
                "species {s} is not in species.ron"
            );
        }
        writeln!(
            self.o,
            "pub static SPECIES: [SpeciesInfo; {}] = [\n{species}];",
            b.species.len()
        )?;
        writeln!(
            self.o,
            "/// Each species' raw `ABILITY_*` ids (`gSpeciesInfo[].abilities`), indexed by\n/// `SpeciesId`; [`ABILITY_NAMES`] names them. `SpeciesInfo::abilities` folds the ones the\n/// engine does not implement into `Ability::Other`.\npub static SPECIES_ABILITY_IDS: [[u8; 2]; {}] = [{ability_ids}];",
            b.species.len()
        )?;
        Ok(())
    }

    /// `MOVES`, `CHART` and `TABLES`.
    fn gen_moves(&mut self) -> Result<()> {
        let b = self.b;
        let mut moves = String::new();
        for m in &b.moves.moves {
            let effect = match classify::move_effect_for(&m.effect, &m.id) {
                Some(e) => e,
                None => {
                    let n = names::MOVE_EFFECTS
                        .value(&m.effect)
                        .with_context(|| format!("{}: unknown effect {}", m.id, m.effect))?;
                    if m.id != "MOVE_NONE" {
                        self.unsupported_effects
                            .entry(m.effect.clone())
                            .or_default()
                            .push(m.id.clone());
                    }
                    format!("MoveEffect::Unsupported({n})")
                }
            };
            writeln!(moves, "    {},", move_info(m, &effect, self.w.lang)?)?;
        }
        writeln!(
            self.o,
            "pub static MOVES: [MoveInfo; {}] = [\n{moves}];",
            b.moves.moves.len()
        )?;

        // Fairy (`Type::Fairy`, Silice): its Gen6 matchups go before Foresight's entries, only
        // when the content uses the type and its chart does not list it (never for Emerald).
        const FAIRY: &str = "TYPE_FAIRY";
        let uses_fairy = b.species.iter().any(|s| s.types.iter().any(|t| t == FAIRY))
            || b.moves.moves.iter().any(|m| m.ty == FAIRY);
        let charted = b
            .types
            .chart
            .iter()
            .any(|(a, d, _)| a == FAIRY || d == FAIRY);
        let fairy = uses_fairy && !charted;
        let mut foresight_split = b.types.foresight_split;
        let mut chart = String::new();
        let mut len = b.types.chart.len();
        for (i, (a, d, x)) in b.types.chart.iter().enumerate() {
            ensure!(matches!(x, 0 | 5 | 20), "type chart multiplier {x}");
            if fairy && i == b.types.foresight_split {
                for k in 0..engine_fairy_len() {
                    write!(chart, "FAIRY_MATCHUPS[{k}],")?;
                }
            }
            write!(
                chart,
                "TypeMatchup {{ attack: {}, defend: {}, multiplier: {x} }},",
                ty(a)?,
                ty(d)?
            )?;
        }
        if fairy {
            if b.types.foresight_split >= b.types.chart.len() {
                for k in 0..engine_fairy_len() {
                    write!(chart, "FAIRY_MATCHUPS[{k}],")?;
                }
            }
            foresight_split += engine_fairy_len();
            len += engine_fairy_len();
        }
        writeln!(
            self.o,
            "pub static CHART: [TypeMatchup; {len}] = [{chart}];"
        )?;
        ensure!(
            b.moves.nature_power.len() == 10,
            "nature_power has {} moves, expected 10",
            b.moves.nature_power.len()
        );
        let np: Vec<String> = b
            .moves
            .nature_power
            .iter()
            .map(|m| self.move_id(m))
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "pub static TABLES: BattleTables = BattleTables {{ species: &SPECIES, moves: &MOVES, type_chart: &CHART, foresight_split: {}, struggle: {}, nature_power: [{}] }};",
            foresight_split,
            self.move_id(&b.struggle)?,
            np.join(", ")
        )?;
        Ok(())
    }

    /// `trainer()` and `TRAINER_IDS`, `encounters()`, `heal_location()`.
    fn gen_trainers(&mut self) -> Result<()> {
        let b = self.b;
        let mut pic_names = BTreeMap::new();
        for (k, (name, p)) in b.trainer_pics.iter().enumerate() {
            let pic = pic(&mut self.blob, p);
            writeln!(self.o, "static TP_{k}: TrainerPic = {pic};")?;
            pic_names.insert(name.as_str(), k);
        }
        let mut trainer_arms = String::new();
        for t in &b.trainers {
            let id = get(&self.trainer_no, &t.id)?;
            if t.party.is_empty() || !pic_names.contains_key(t.pic.as_str()) {
                if id != 0 {
                    self.skipped_trainers.push(t.id.clone());
                }
                continue;
            }
            let mut party = String::new();
            for m in &t.party {
                ensure!(m.iv <= 255, "{}: iv {}", t.id, m.iv);
                let moves = match &m.moves {
                    Some(v) => {
                        ensure!(v.len() == 4, "{}: {} custom moves", t.id, v.len());
                        let ms: Vec<String> = v
                            .iter()
                            .map(|n| {
                                Ok(match get(&self.move_no, n)? {
                                    0 => "None".to_string(),
                                    x => format!("Some(MoveId({x}))"),
                                })
                            })
                            .collect::<Result<_>>()?;
                        format!("Some([{}])", ms.join(", "))
                    }
                    None => "None".into(),
                };
                let item = match &m.item {
                    Some(name) if self.rules.trainer_held_items && name != "ITEM_NONE" => {
                        let v = names::ITEMS
                            .value(name)
                            .with_context(|| format!("{}: unknown item {name}", t.id))?;
                        format!("Some(ItemId({v}))")
                    }
                    _ => "None".into(),
                };
                write!(
                    party,
                    "TrainerMon {{ species: {}, level: {}, iv: {}, moves: {moves}, item: {item} }},",
                    self.species_id(&m.species)?,
                    m.level,
                    m.iv
                )?;
            }
            let mut ai = 0u32;
            for f in &t.ai {
                ai |= names::AI_FLAGS
                    .value(f)
                    .with_context(|| format!("{}: unknown AI flag {f}", t.id))?;
            }
            let pic = pic_names
                .get(t.pic.as_str())
                .with_context(|| format!("{}: no pic {}", t.id, t.pic))?;
            writeln!(
                self.o,
                "static TR_{id}: TrainerData = TrainerData {{ class_name: {}, name: {}, pic: &TP_{pic}, female: {}, ai: {ai:#X}, double: {}, party: &[{party}] }}; // {}",
                bytes_lit(&self.enc(&t.class_name, &t.id)?),
                bytes_lit(&self.enc(&t.name, &t.id)?),
                t.female,
                t.double,
                t.id
            )?;
            writeln!(trainer_arms, "        {id} => Some(&TR_{id}),")?;
            self.trainer_ids.push(id);
        }
        writeln!(
            self.o,
            "/// Every trainer with a party and a pic (`TRAINER_*`).\npub fn trainer(id: u16) -> Option<&'static TrainerData> {{\n    match id {{\n{trainer_arms}        _ => None,\n    }}\n}}"
        )?;
        writeln!(
            self.o,
            "pub static TRAINER_IDS: [TrainerId; {}] = [{}];",
            self.trainer_ids.len(),
            self.trainer_ids
                .iter()
                .map(|i| format!("TrainerId({i})"))
                .collect::<Vec<_>>()
                .join(", ")
        )?;

        self.gen_encounters()?;
        self.gen_evolutions_and_money()?;
        let mut hold_arms = String::new();
        for (v, name) in names::ITEMS.iter() {
            if let Some(e) = classify::hold_effect(name) {
                writeln!(hold_arms, "        {v} => Some({e}), // {name}")?;
            }
        }
        // An authored game's items take their base item's effect.
        for (v, base) in self.w.authored.iter().flat_map(|a| &a.item_bases) {
            if let Some(e) = classify::hold_effect(base) {
                writeln!(hold_arms, "        {v} => Some({e}), // as {base}")?;
            }
        }
        writeln!(
            self.o,
            "/// The modelled `holdEffect` of item `id` (`ITEM_*`).\npub fn hold_effect(id: u16) -> Option<HoldEffect> {{\n    match id {{\n{hold_arms}        _ => None,\n    }}\n}}"
        )?;
        let mut heal_arms = String::new();
        for (name, (map, x, y)) in &b.heal {
            let id = names::HEAL_LOCATIONS
                .value(name)
                .with_context(|| format!("unknown {name}"))?;
            let Some(m) = map_number(self.w, map) else {
                warn(&format!("heal location {name}: unknown map {map}, skipped"));
                continue;
            };
            writeln!(
                heal_arms,
                "        {id} => Some((MapId({m:#X}), {x}, {y})), // {name}"
            )?;
        }
        writeln!(
            self.o,
            "/// Gen3 heal location `id` (`HEAL_LOCATION_*`): map and player cell.\npub fn heal_location(id: u8) -> Option<(MapId, i16, i16)> {{\n    match id {{\n{heal_arms}        _ => None,\n    }}\n}}"
        )?;
        Ok(())
    }

    /// `evolutions()` (`gEvolutionTable`) and `TRAINER_MONEY` (each built trainer's
    /// `gTrainerMoneyTable` value; empty when the extract predates format 5.1).
    fn gen_evolutions_and_money(&mut self) -> Result<()> {
        let b = self.b;
        let mut arms = String::new();
        for (species, evos) in &b.evolutions.species {
            let from = get(&self.species_no, species)?;
            let mut v = String::new();
            for (method, param, target) in evos {
                write!(
                    v,
                    "Evolution {{ method: {}, param: {param}, target: {} }},",
                    evo_method(method),
                    self.species_id(target)?
                )?;
            }
            writeln!(arms, "        {from} => &[{v}], // {species}")?;
        }
        writeln!(
            self.o,
            "/// `gEvolutionTable[species]`.\npub fn evolutions(species: u16) -> &'static [Evolution] {{\n    match species {{\n{arms}        _ => &[],\n    }}\n}}"
        )?;
        let money = &b.trainer_money.classes;
        let mut table = Vec::new();
        if !money.is_empty() {
            // `GetTrainerMoneyToGive`: the class's entry, else the `0xFF` terminator's value.
            let default = money.get(TRAINER_MONEY_DEFAULT_CLASS).copied();
            for t in &b.trainers {
                let m = money.get(&t.class).copied().or(default).with_context(|| {
                    format!("{}: no money for {} and no default", t.id, t.class)
                })?;
                table.push(m.to_string());
            }
        }
        writeln!(
            self.o,
            "/// `gTrainerMoneyTable` value of each trainer's class, by `TRAINER_*` (empty: not extracted).\npub static TRAINER_MONEY: [u8; {}] = [{}];",
            table.len(),
            table.join(",")
        )?;
        Ok(())
    }

    /// `encounters()` and `wild_encounters()` (land), and `water()`, `rock_smash()`, `fishing()`
    /// (compiled for Milestone 10), from every wild header of every map.
    fn gen_encounters(&mut self) -> Result<()> {
        let b = self.b;
        let mut land_arms = String::new();
        let mut first_arms = String::new();
        let mut out = Vec::new();
        let mut other: [(&str, String); 3] = [
            ("water", String::new()),
            ("rock_smash", String::new()),
            ("fishing", String::new()),
        ];
        for (map, headers) in &b.encounters {
            let Some(id) = map_number(self.w, map) else {
                warn(&format!("encounters: unknown map {map}, skipped"));
                continue;
            };
            for (set, h) in headers.iter().enumerate() {
                let mons = |v: &[extract::schema::WildMon]| -> Result<String> {
                    let mut s = String::new();
                    for m in v {
                        write!(
                            s,
                            "WildMon {{ species: {}, min_level: {}, max_level: {} }},",
                            self.species_id(&m.species)?,
                            m.min_level,
                            m.max_level
                        )?;
                    }
                    Ok(s)
                };
                if let Some(e) = &h.land {
                    ensure!(
                        e.mons.len() == 12,
                        "encounters {map} #{set}: {} land slots",
                        e.mons.len()
                    );
                    let line = format!(
                        "static ENC_{id:X}_{set}: LandEncounters = LandEncounters {{ rate: {}, mons: [{}] }}; // {map}",
                        e.rate,
                        mons(&e.mons)?
                    );
                    out.push(line);
                    writeln!(
                        land_arms,
                        "        ({id:#X}, {set}) => Some(&ENC_{id:X}_{set}),"
                    )?;
                    if set == 0 {
                        writeln!(first_arms, "        {id:#X} => Some(&ENC_{id:X}_0),")?;
                    }
                }
                for ((_, arms), table) in
                    other.iter_mut().zip([&h.water, &h.rock_smash, &h.fishing])
                {
                    if let Some(e) = table {
                        writeln!(
                            arms,
                            "        ({id:#X}, {set}) => Some(({}, &[{}])), // {map}",
                            e.rate,
                            mons(&e.mons)?
                        )?;
                    }
                }
            }
        }
        for line in out {
            writeln!(self.o, "{line}")?;
        }
        writeln!(
            self.o,
            "/// The land table of map `id`'s first wild header (`MAP_*`).\npub fn encounters(id: u16) -> Option<&'static LandEncounters> {{\n    match id {{\n{first_arms}        _ => None,\n    }}\n}}"
        )?;
        writeln!(
            self.o,
            "/// The land table of wild header `set` of map `id` (Gen3 `GetCurrentMapWildMonHeaderId`: `set` is\n/// `VAR_ALTERING_CAVE_WILD_SET`, and a set the map does not have reads header 0).\npub fn wild_encounters(id: u16, set: u16) -> Option<&'static LandEncounters> {{\n    let set = if set < wild_sets(id) {{ set }} else {{ 0 }};\n    match (id, set) {{\n{land_arms}        _ => None,\n    }}\n}}"
        )?;
        let mut sets = String::new();
        for (map, headers) in &b.encounters {
            if let Some(id) = map_number(self.w, map)
                && headers.len() > 1
            {
                writeln!(sets, "        {id:#X} => {},", headers.len())?;
            }
        }
        writeln!(
            self.o,
            "/// How many wild headers map `id` has.\npub fn wild_sets(id: u16) -> u16 {{\n    match id {{\n{sets}        _ => 1,\n    }}\n}}"
        )?;
        for (name, arms) in other {
            writeln!(
                self.o,
                "/// The {name} table (rate, slots) of wild header `set` of map `id` (Milestone 10).\npub fn {name}(id: u16, set: u16) -> Option<(u8, &'static [WildMon])> {{\n    let set = if set < wild_sets(id) {{ set }} else {{ 0 }};\n    match (id, set) {{\n{arms}        _ => None,\n    }}\n}}"
            )?;
        }
        Ok(())
    }

    /// `text()` (`GameData::battle_text`) and `ALL_STRS`.
    fn gen_strings(&mut self) -> Result<()> {
        let b = self.b;
        let mut arms = String::new();
        for (pat, src) in classify::STRS {
            match src {
                StrSrc::TypeNames => {
                    // `Type::Fairy` (19th) has no Gen3 name.
                    writeln!(
                        arms,
                        "        Str::{pat} => TYPE_NAMES.get(t as usize).copied().unwrap_or(b\"\\xFF\"),"
                    )?
                }
                StrSrc::StatNames => {
                    writeln!(arms, "        Str::{pat} => STAT_NAMES[s as usize],")?
                }
                StrSrc::AbilityNames => writeln!(arms, "        Str::{pat} => ability_name(a),")?,
                StrSrc::Missing => writeln!(arms, "        Str::{pat} => b\"\\xFF\",")?,
                _ => {
                    let t = str_text(b, *src).with_context(|| format!("Str::{pat}"))?;
                    writeln!(
                        arms,
                        "        Str::{pat} => {},",
                        bytes_lit(&self.enc(&t, pat)?)
                    )?
                }
            }
        }
        ensure!(
            b.types.names.len() == 18,
            "{} type names, expected 18",
            b.types.names.len()
        );
        let tn: Vec<String> = b
            .types
            .names
            .iter()
            .map(|n| Ok(bytes_lit(&self.enc(n, "type name")?)))
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "static TYPE_NAMES: [&[u8]; 18] = [{}];",
            tn.join(", ")
        )?;
        let sn: Vec<String> = STAT_NAME_KEYS
            .iter()
            .map(|k| {
                let s = b.strings.get(*k).with_context(|| format!("no {k}"))?;
                Ok(bytes_lit(&self.enc(s, k)?))
            })
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "static STAT_NAMES: [&[u8]; 8] = [{}];",
            sn.join(", ")
        )?;
        let named = named_abilities(self.rules);
        ensure!(
            b.types.ability_names.len() == names::ABILITIES.iter().count(),
            "{} ability names, expected one per ABILITY_*",
            b.types.ability_names.len()
        );
        let an: Vec<String> = b
            .types
            .ability_names
            .iter()
            .map(|n| Ok(bytes_lit(&self.enc(n, "ability name")?)))
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "/// `gAbilityNames`, indexed by raw `ABILITY_*` id (Gen3-encoded, 0xFF-terminated).\npub static ABILITY_NAMES: [&[u8]; {}] = [{}];",
            an.len(),
            an.join(", ")
        )?;
        let mut ability_arms = String::new();
        for (expr, id) in &named {
            let n = &b.types.ability_names[*id as usize];
            writeln!(
                ability_arms,
                "        {expr} => {},",
                bytes_lit(&self.enc(n, "ability name")?)
            )?;
        }
        writeln!(
            self.o,
            "/// `gAbilityNames` by `Ability` (`Ability::Other` and the abilities the content does not\n/// map have no name).\n#[allow(unreachable_patterns)]\nfn ability_name(a: Ability) -> &'static [u8] {{\n    match a {{\n{ability_arms}        _ => b\"\\xFF\",\n    }}\n}}"
        )?;
        writeln!(
            self.o,
            "/// `GameData::battle_text`.\npub fn text(s: Str) -> &'static [u8] {{\n    match s {{\n{arms}    }}\n}}"
        )?;
        let mut all = Vec::new();
        for (pat, src) in classify::STRS {
            match src {
                StrSrc::TypeNames => {
                    all.extend((0..18).map(|i| format!("Str::TypeName(Type::ALL[{i}])")))
                }
                StrSrc::Missing => {}
                StrSrc::StatNames => {
                    all.extend((0..8).map(|i| format!("Str::StatName(Stat::ALL[{i}])")))
                }
                StrSrc::AbilityNames => all.extend(
                    named_abilities(self.rules)
                        .iter()
                        .map(|(expr, _)| format!("Str::AbilityName({expr})")),
                ),
                _ => all.push(format!("Str::{pat}")),
            }
        }
        writeln!(
            self.o,
            "/// Every `Str` (the indexed ones for every index).\npub static ALL_STRS: [Str; {}] = [{}];",
            all.len(),
            all.join(", ")
        )?;
        Ok(())
    }

    /// `SMALL_FONT` and `GFX`.
    fn gen_gfx(&mut self) -> Result<()> {
        let b = self.b;
        let u = &b.ui;
        let envs: Vec<String> = if u.environments.is_empty() {
            vec!["GfxImage::EMPTY".into(); 10]
        } else {
            ensure!(
                u.environments.len() == 10,
                "{} environments",
                u.environments.len()
            );
            u.environments
                .iter()
                .map(|e| image(&mut self.blob, e))
                .collect()
        };
        ensure!(
            b.small_glyphs.len() == b.small_widths.len(),
            "small font glyphs/widths length"
        );
        let glyphs = self.blob.glyphs(&b.small_glyphs);
        writeln!(
            self.o,
            "static SMALL_WIDTHS: [u8; {}] = *{};\npub static SMALL_FONT: Font = Font {{ glyphs: {glyphs}, widths: &SMALL_WIDTHS }};",
            b.small_widths.len(),
            bytes_lit(&b.small_widths),
        )?;
        let blob = &mut self.blob;
        let line = format!(
            "pub static GFX: BattleGfx = BattleGfx {{ textbox: {}, environments: [{}], healthbox_player: {}, healthbox_opponent: {}, hpbar: {}, expbar: {}, status: {}, misc: {}, ball: {}, small_font: &SMALL_FONT, player_back: {}, starter_bag: {}, starter_ball: {}, starter_circle: {} }};",
            image(blob, &u.textbox),
            envs.join(", "),
            image(blob, &u.healthbox_player),
            image(blob, &u.healthbox_opponent),
            image(blob, &u.hpbar),
            image(blob, &u.expbar),
            image(blob, &u.status),
            image(blob, &u.misc),
            image(blob, &u.ball),
            pic(blob, &b.player_back),
            image(blob, &u.starter_bag),
            image(blob, &u.starter_ball),
            image(blob, &u.starter_circle)
        );
        writeln!(self.o, "{line}")?;
        Ok(())
    }

    /// `STARTERS`, `FIRST_BATTLE_FOE`, `SUMMARY` and the report.
    fn gen_summary(&mut self) -> Result<String> {
        let b = self.b;
        writeln!(
            self.o,
            "/// `sStarterMon`.\npub static STARTERS: [SpeciesId; 3] = [{}, {}, {}];\n/// `CB2_StartFirstBattle`: species and level.\npub const FIRST_BATTLE_FOE: (SpeciesId, u8) = ({}, {});",
            self.species_id(&b.starters[0])?,
            self.species_id(&b.starters[1])?,
            self.species_id(&b.starters[2])?,
            self.species_id(&b.first_foe.0)?,
            b.first_foe.1
        )?;
        let no_pics: Vec<&str> = b
            .species
            .iter()
            .filter(|s| s.id != "SPECIES_NONE" && !b.mon_gfx.contains_key(&s.id))
            .map(|s| s.id.as_str())
            .collect();
        let unsupported_moves: usize = self.unsupported_effects.values().map(Vec::len).sum();
        let summary = format!(
            "battle: {} species ({} with pics), {} moves ({} with unsupported effects), {} trainers ({} left out), {} abilities left as Ability::Other",
            b.species.len(),
            b.mon_gfx.len(),
            b.moves.moves.len(),
            unsupported_moves,
            self.trainer_ids.len(),
            self.skipped_trainers.len(),
            self.unsupported_abilities.len(),
        );
        writeln!(
            self.o,
            "/// What the build left unsupported (counts; `content::REPORT` lists them).\npub const SUMMARY: &str = {summary:?};"
        )?;
        let mut r = format!("{summary}\n");
        for (e, moves) in &self.unsupported_effects {
            writeln!(
                r,
                "unsupported effect {e} (MoveEffect::Unsupported): {}",
                moves.join(", ")
            )?;
        }
        for (a, n) in &self.unsupported_abilities {
            writeln!(r, "ability {a} -> Ability::Other ({n} species)")?;
        }
        writeln!(
            r,
            "species without pics (gfx: None): {}",
            list(no_pics.iter().map(|s| s.to_string()))
        )?;
        writeln!(
            r,
            "trainers left out (no pic or no party; trainer() is None): {}",
            list(self.skipped_trainers.iter().cloned())
        )?;
        Ok(r)
    }

    /// `SPECIES_*` / `MOVE_*` / `TRAINER_*` / `HEAL_LOCATION_*`.
    fn consts(&self) -> Vec<(String, &'static str, String)> {
        let mut consts = Vec::new();
        for (n, i) in &self.species_no {
            consts.push((n.to_string(), "SpeciesId", format!("SpeciesId({i})")));
        }
        for (n, i) in &self.move_no {
            consts.push((n.to_string(), "MoveId", format!("MoveId({i})")));
        }
        // An authored game's trainers are named by its ids (`AuthoredIn::consts`, live ids
        // only); of the list, only the Gen3 names (`TRAINER_NONE`).
        let authored = self.w.authored.is_some();
        for (n, i) in &self.trainer_no {
            if !authored || names::TRAINERS.value(n).is_some() {
                consts.push((n.to_string(), "TrainerId", format!("TrainerId({i})")));
            }
        }
        for (v, n) in names::HEAL_LOCATIONS.iter() {
            consts.push((n.to_string(), "u8", format!("{v}")));
        }
        consts
    }
}

pub fn generate(b: &BattleIn, w: &World) -> Result<Out> {
    let mut g = Gen {
        b,
        w,
        species_no: numbered(
            b.species.iter().map(|s| s.id.as_str()),
            &names::SPECIES,
            b.stub,
        )?,
        move_no: numbered(
            b.moves.moves.iter().map(|m| m.id.as_str()),
            &names::MOVES,
            b.stub,
        )?,
        // An authored game numbers its own trainers (`TRAINER_SIL_*`, by index).
        trainer_no: numbered(
            b.trainers.iter().map(|t| t.id.as_str()),
            &names::TRAINERS,
            b.stub || w.authored.is_some(),
        )?,
        blob: BlobOut::new("BLOB"),
        o: String::from(
            "// @generated by content/build.rs\nuse engine::battle::{BattleGfx, GfxImage, Str};\nuse engine::data::*;\nuse engine::mon::*;\n\n",
        ),
        unsupported_effects: BTreeMap::new(),
        unsupported_abilities: BTreeMap::new(),
        trainer_ids: Vec::new(),
        skipped_trainers: Vec::new(),
        rules: w
            .authored
            .as_ref()
            .map_or(classify::Rules::EMERALD, |a| a.rules),
    };
    g.gen_species()?;
    g.gen_moves()?;
    g.gen_trainers()?;
    g.gen_strings()?;
    g.gen_gfx()?;
    let report = g.gen_summary()?;
    let decl = g.blob.static_decl("battle.bin");
    g.o.push_str(&decl);
    let consts = g.consts();
    Ok(Out {
        rs: g.o,
        bin: g.blob.bytes,
        consts,
        report,
    })
}

/// `engine::mon::FAIRY_MATCHUPS.len()` (the build script does not link the engine).
pub(crate) const fn engine_fairy_len() -> usize {
    12
}

/// `EVO_*` -> `EvoMethod` expression (Gen3 `EVO_*` folded; the exact kind waits for Milestone 9).
fn evo_method(name: &str) -> &'static str {
    match name {
        "EVO_LEVEL"
        | "EVO_LEVEL_ATK_GT_DEF"
        | "EVO_LEVEL_ATK_EQ_DEF"
        | "EVO_LEVEL_ATK_LT_DEF"
        | "EVO_LEVEL_SILCOON"
        | "EVO_LEVEL_CASCOON"
        | "EVO_LEVEL_NINJASK" => "EvoMethod::Level",
        "EVO_FRIENDSHIP" | "EVO_FRIENDSHIP_DAY" | "EVO_FRIENDSHIP_NIGHT" => "EvoMethod::Friendship",
        "EVO_ITEM" => "EvoMethod::Item",
        "EVO_TRADE" | "EVO_TRADE_ITEM" => "EvoMethod::Trade",
        _ => "EvoMethod::Other",
    }
}

fn list(it: impl Iterator<Item = String>) -> String {
    let v: Vec<String> = it.collect();
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

fn move_info(m: &Move, effect: &str, lang: Lang) -> Result<String> {
    let mut flags = 0u8;
    for f in &m.flags {
        flags |= classify::move_flag(f).with_context(|| format!("{}: unknown flag {f}", m.id))?;
    }
    flags |= classify::extra_move_flags(&m.id);
    Ok(format!(
        "MoveInfo {{ name: {}, effect: {effect}, power: {}, ty: {}, accuracy: {}, pp: {}, effect_chance: {}, target: {}, priority: {}, flags: MoveFlags({flags:#X}) }}",
        bytes_lit(&encode_lang(&m.name, lang).with_context(|| m.id.clone())?),
        m.power,
        ty(&m.ty)?,
        m.accuracy,
        m.pp,
        m.chance,
        classify::move_target(&m.target)
            .with_context(|| format!("{}: unknown target {}", m.id, m.target))?,
        m.priority
    ))
}

/// Every `Ability` with a name under `rules` (all but `Ability::Other`) and its `ABILITY_*`
/// value, in `ABILITY_*` order.
fn named_abilities(rules: classify::Rules) -> Vec<(&'static str, u32)> {
    names::ABILITIES
        .iter()
        .map(|(v, n)| (classify::ability_with(n, rules), v))
        .filter(|(expr, _)| *expr != "Ability::Other")
        .collect()
}
