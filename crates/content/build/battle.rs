//! Battle content (Milestone 4): `BattleTables`, trainers, encounters, heal locations, battle
//! strings and `BattleGfx`, read from `content/extracted/{battle,pokemon,trainers,starter,font}`
//! (`crate::battle_load::load`) or built in ([`crate::stub::battle`]), and generated as
//! `$OUT_DIR/battle.rs` plus one aligned blob, `$OUT_DIR/battle.bin`, that holds every tile,
//! palette, tilemap and glyph (`include_bytes!`, sliced by `content::blob::Blob`).
//!
//! The tables are complete (every species and move, so ids are Gen3 numbers); pics are only
//! built for the **slice set**: the starters, the land mons of the exported maps, the parties of
//! the trainers the exported scripts name, and the first battle's Zigzagoon.

use crate::classify::{self, StrSrc};
use crate::codegen::{bytes_lit, ident_ok};
use crate::input::{Glyph, Tile, World, warn};
use anyhow::{Context, Result, bail, ensure};
use extract::battle_data::{STAT_NAME_KEYS, stat_sharply_is_complete};
use extract::names::{self, Table};
use extract::schema::{
    BattleStringsFile, HealLocationsFile, LandEncounters, MonCoords, Move, MovesFile, Species,
    Trainer, TypesFile,
};
use extract::text::{Lang, encode_lang};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// `sStarterMon` (`starter_choose.c`).
pub const STARTERS: [&str; 3] = ["SPECIES_TREECKO", "SPECIES_TORCHIC", "SPECIES_MUDKIP"];
/// `CB2_StartFirstBattle` (`battle_setup.c`): Zigzagoon, level 2.
pub const FIRST_FOE: (&str, u8) = ("SPECIES_ZIGZAGOON", 2);
/// The highest level the player's starter is expected to reach in the slice: its level-up
/// moves up to here are slice moves.
pub const PLAYER_LEVEL_CAP: u8 = 15;
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
    /// The trainers the exported scripts name: only these get a `TrainerData`.
    pub referenced: BTreeSet<String>,
    /// Land tables by map name.
    pub encounters: BTreeMap<String, LandEncounters>,
    pub heal: HealLocationsFile,
    pub strings: BattleStringsFile,
    /// Pics of the slice species, by `SPECIES_*`.
    pub mon_gfx: BTreeMap<String, MonGfxIn>,
    /// Front pics of the referenced trainers, by `TRAINER_PIC_*`.
    pub trainer_pics: BTreeMap<String, PicIn>,
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

// ---- slice ----

/// (species, highest level) of every Pokémon the slice can field.
fn slice_uses(b: &BattleIn) -> Vec<(String, u8)> {
    let mut v: Vec<(String, u8)> = b
        .starters
        .iter()
        .map(|s| (s.clone(), PLAYER_LEVEL_CAP))
        .collect();
    v.push(b.first_foe.clone());
    for e in b.encounters.values() {
        v.extend(e.mons.iter().map(|m| (m.species.clone(), m.max_level)));
    }
    for t in b.trainers.iter().filter(|t| b.referenced.contains(&t.id)) {
        v.extend(t.party.iter().map(|m| (m.species.clone(), m.level)));
    }
    v
}

pub fn slice_species(b: &BattleIn) -> BTreeSet<String> {
    slice_uses(b).into_iter().map(|(s, _)| s).collect()
}

/// Level-up moves the slice mons know or learn, the custom moves of referenced trainers and
/// Struggle.
pub fn slice_moves(b: &BattleIn) -> BTreeSet<String> {
    let mut out = BTreeSet::from([b.struggle.clone()]);
    for (s, level) in slice_uses(b) {
        if let Some(sp) = b.species.iter().find(|x| x.id == s) {
            out.extend(
                sp.learnset
                    .iter()
                    .filter(|(l, _)| *l <= level)
                    .map(|(_, m)| m.clone()),
            );
        }
    }
    for t in b.trainers.iter().filter(|t| b.referenced.contains(&t.id)) {
        for m in &t.party {
            out.extend(m.moves.iter().flatten().cloned());
        }
    }
    out.remove("MOVE_NONE");
    out
}

// ---- generation ----

/// The blob: 4-aligned chunks, addressed by byte offset.
#[derive(Default)]
struct Blob(Vec<u8>);

impl Blob {
    fn align(&mut self) -> usize {
        while !self.0.len().is_multiple_of(4) {
            self.0.push(0);
        }
        self.0.len()
    }

    fn tiles(&mut self, t: &[Tile]) -> String {
        let at = self.align();
        for r in t.iter().flatten() {
            self.0.extend(r.to_le_bytes());
        }
        format!("BLOB.tiles({at}, {})", t.len())
    }

    fn u16s(&mut self, v: &[u16]) -> usize {
        let at = self.align();
        for x in v {
            self.0.extend(x.to_le_bytes());
        }
        at
    }

    fn pal(&mut self, p: &[u16; 16]) -> String {
        format!("BLOB.pal({})", self.u16s(p))
    }

    fn pals(&mut self, p: &[[u16; 16]]) -> String {
        format!("BLOB.pals({}, {})", self.u16s(p.as_flattened()), p.len())
    }

    fn tilemap(&mut self, v: &[u16]) -> String {
        format!("BLOB.u16s({}, {})", self.u16s(v), v.len())
    }

    fn glyphs(&mut self, g: &[Glyph]) -> String {
        let at = self.align();
        self.0.extend(g.iter().flatten());
        format!("BLOB.glyphs({at}, {})", g.len())
    }

    fn image(&mut self, g: &GfxIn) -> String {
        format!(
            "GfxImage {{ tiles: {}, palettes: {}, tilemap: {} }}",
            self.tiles(&g.tiles),
            self.pals(&g.palettes),
            self.tilemap(&g.tilemap)
        )
    }

    fn pic(&mut self, p: &PicIn) -> String {
        format!(
            "TrainerPic {{ tiles: {}, palette: {}, y: {} }}",
            self.tiles(&p.tiles),
            self.pal(&p.palette),
            p.y
        )
    }
}

pub struct Out {
    /// `battle.rs`.
    pub rs: String,
    /// `battle.bin`.
    pub bin: Vec<u8>,
    /// `SPECIES_*` / `MOVE_*` / `TRAINER_*` / `HEAL_LOCATION_*` constants: (name, type, value).
    pub consts: Vec<(String, &'static str, String)>,
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

/// Gen3 map number of a map name: an exported map, else the names table.
fn map_number(w: &World, name: &str) -> Option<u16> {
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
    })
}

/// Effects whose stat message the engine's one-`Str`-per-battler wording cannot say: the user's
/// own stats fall (`EFFECT_SUPERPOWER`, `EFFECT_OVERHEAT`) or the target's rise
/// (`EFFECT_SWAGGER`, `EFFECT_FLATTER`). See `classify::STRS` (`AttackerStatChanged`).
const UNWORDED_STAT_EFFECTS: [&str; 4] = [
    "EFFECT_SUPERPOWER",
    "EFFECT_OVERHEAT",
    "EFFECT_SWAGGER",
    "EFFECT_FLATTER",
];

/// The generator's state: the input, id numbers, the slice, and the output so far.
struct Gen<'a> {
    b: &'a BattleIn,
    w: &'a World,
    species_no: BTreeMap<&'a str, u16>,
    move_no: BTreeMap<&'a str, u16>,
    trainer_no: BTreeMap<&'a str, u16>,
    slice_sp: BTreeSet<String>,
    slice_mv: BTreeSet<String>,
    blob: Blob,
    /// `battle.rs`. The blob static is written last (its size is known then); statics may
    /// refer to it before.
    o: String,
    unsupported_effects: BTreeMap<String, String>,
    unsupported_abilities: BTreeSet<String>,
    trainer_ids: Vec<u16>,
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

    /// `SPECIES`: every species, with learnsets and the slice's pics.
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
                let v = classify::ability(a);
                if v == "Ability::Other" && self.slice_sp.contains(&s.id) {
                    self.unsupported_abilities.insert(a.clone());
                }
                v
            });
            writeln!(
                species,
                "    SpeciesInfo {{ name: {}, base: {:?}, types: [{}, {}], catch_rate: {}, exp_yield: {}, ev_yield: {:?}, gender_ratio: {}, friendship: {}, growth: {}, abilities: [{}, {}], learnset: &LS_{i}, weight: 0, gfx: {gfx} }}, // {}",
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
                s.id
            )?;
        }
        for s in &self.slice_sp {
            ensure!(
                self.species_no.contains_key(s.as_str()),
                "slice species {s} is not in species.ron"
            );
            ensure!(b.mon_gfx.contains_key(s), "slice species {s} has no pics");
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
            let slice = self.slice_mv.contains(&m.id);
            ensure!(
                !(slice && UNWORDED_STAT_EFFECTS.contains(&m.effect.as_str())),
                "slice move {} has {}: its stat message needs Rose/Fell-split `Str`s",
                m.id,
                m.effect
            );
            let effect = match classify::move_effect(&m.effect) {
                Some(e) => e,
                None => {
                    let n = names::MOVE_EFFECTS
                        .value(&m.effect)
                        .with_context(|| format!("{}: unknown effect {}", m.id, m.effect))?;
                    if slice {
                        self.unsupported_effects
                            .insert(m.id.clone(), m.effect.clone());
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
        for m in &self.slice_mv {
            ensure!(
                self.move_no.contains_key(m.as_str()),
                "slice move {m} is not in moves.ron"
            );
        }

        let mut chart = String::new();
        for (a, d, x) in &b.types.chart {
            ensure!(matches!(x, 0 | 5 | 20), "type chart multiplier {x}");
            write!(
                chart,
                "TypeMatchup {{ attack: {}, defend: {}, multiplier: {x} }},",
                ty(a)?,
                ty(d)?
            )?;
        }
        writeln!(
            self.o,
            "pub static CHART: [TypeMatchup; {}] = [{chart}];",
            b.types.chart.len()
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
            b.types.foresight_split,
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
            let pic = self.blob.pic(p);
            writeln!(self.o, "static TP_{k}: TrainerPic = {pic};")?;
            pic_names.insert(name.as_str(), k);
        }
        let mut trainer_arms = String::new();
        for t in b.trainers.iter().filter(|t| b.referenced.contains(&t.id)) {
            let id = get(&self.trainer_no, &t.id)?;
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
                write!(
                    party,
                    "TrainerMon {{ species: {}, level: {}, iv: {}, moves: {moves} }},",
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
            "/// The trainers the exported scripts name (`TRAINER_*`).\npub fn trainer(id: u16) -> Option<&'static TrainerData> {{\n    match id {{\n{trainer_arms}        _ => None,\n    }}\n}}"
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

        let mut enc_arms = String::new();
        for (map, e) in &b.encounters {
            let Some(id) = map_number(self.w, map) else {
                warn(&format!("encounters: unknown map {map}, skipped"));
                continue;
            };
            ensure!(
                e.mons.len() == 12,
                "encounters {map}: {} slots",
                e.mons.len()
            );
            let mut mons = String::new();
            for m in &e.mons {
                write!(
                    mons,
                    "WildMon {{ species: {}, min_level: {}, max_level: {} }},",
                    self.species_id(&m.species)?,
                    m.min_level,
                    m.max_level
                )?;
            }
            writeln!(
                self.o,
                "static ENC_{id:X}: LandEncounters = LandEncounters {{ rate: {}, mons: [{mons}] }}; // {map}",
                e.rate
            )?;
            writeln!(enc_arms, "        {id:#X} => Some(&ENC_{id:X}),")?;
        }
        writeln!(
            self.o,
            "/// The land table of map `id` (`MAP_*`).\npub fn encounters(id: u16) -> Option<&'static LandEncounters> {{\n    match id {{\n{enc_arms}        _ => None,\n    }}\n}}"
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

    /// `text()` (`GameData::battle_text`) and `ALL_STRS`.
    fn gen_strings(&mut self) -> Result<()> {
        let b = self.b;
        let mut arms = String::new();
        for (pat, src) in classify::STRS {
            match src {
                StrSrc::TypeNames => {
                    writeln!(arms, "        Str::{pat} => TYPE_NAMES[t as usize],")?
                }
                StrSrc::StatNames => {
                    writeln!(arms, "        Str::{pat} => STAT_NAMES[s as usize],")?
                }
                StrSrc::AbilityNames => writeln!(arms, "        Str::{pat} => ability_name(a),")?,
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
        let named = named_abilities();
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
            "/// `gAbilityNames` by `Ability` (`Ability::Other` has no name).\nfn ability_name(a: Ability) -> &'static [u8] {{\n    match a {{\n{ability_arms}        Ability::Other => b\"\\xFF\",\n    }}\n}}"
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
                StrSrc::StatNames => {
                    all.extend((0..8).map(|i| format!("Str::StatName(Stat::ALL[{i}])")))
                }
                StrSrc::AbilityNames => all.extend(
                    named_abilities()
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
            u.environments.iter().map(|e| self.blob.image(e)).collect()
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
            blob.image(&u.textbox),
            envs.join(", "),
            blob.image(&u.healthbox_player),
            blob.image(&u.healthbox_opponent),
            blob.image(&u.hpbar),
            blob.image(&u.expbar),
            blob.image(&u.status),
            blob.image(&u.misc),
            blob.image(&u.ball),
            blob.pic(&b.player_back),
            blob.image(&u.starter_bag),
            blob.image(&u.starter_ball),
            blob.image(&u.starter_circle)
        );
        writeln!(self.o, "{line}")?;
        Ok(())
    }

    /// The slice (`STARTERS`, `FIRST_BATTLE_FOE`, `SLICE_SPECIES`, `SLICE_MOVES`) and
    /// `SUMMARY`.
    fn gen_slice(&mut self) -> Result<()> {
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
        let ss: Vec<String> = self
            .slice_sp
            .iter()
            .map(|s| self.species_id(s))
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "pub static SLICE_SPECIES: [SpeciesId; {}] = [{}];",
            ss.len(),
            ss.join(", ")
        )?;
        let sm: Vec<String> = self
            .slice_mv
            .iter()
            .map(|m| self.move_id(m))
            .collect::<Result<_>>()?;
        writeln!(
            self.o,
            "pub static SLICE_MOVES: [MoveId; {}] = [{}];",
            sm.len(),
            sm.join(", ")
        )?;

        let summary = format!(
            "battle: {} species ({} with pics), {} moves ({} in the slice), {} trainers; slice moves with unsupported effects: {}; slice abilities left as Ability::Other: {}",
            b.species.len(),
            b.mon_gfx.len(),
            b.moves.moves.len(),
            self.slice_mv.len(),
            self.trainer_ids.len(),
            list(
                self.unsupported_effects
                    .iter()
                    .map(|(m, e)| format!("{m} ({e})"))
            ),
            list(self.unsupported_abilities.iter().cloned()),
        );
        if !self.unsupported_effects.is_empty() || !self.unsupported_abilities.is_empty() {
            warn(&summary);
        }
        writeln!(
            self.o,
            "/// What the build left unsupported.\npub const SUMMARY: &str = {summary:?};"
        )?;
        Ok(())
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
        for (n, i) in &self.trainer_no {
            consts.push((n.to_string(), "TrainerId", format!("TrainerId({i})")));
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
        trainer_no: numbered(
            b.trainers.iter().map(|t| t.id.as_str()),
            &names::TRAINERS,
            b.stub,
        )?,
        slice_sp: slice_species(b),
        slice_mv: slice_moves(b),
        blob: Blob::default(),
        o: String::from(
            "// @generated by content/build.rs\nuse engine::battle::{BattleGfx, GfxImage, Str};\nuse engine::data::*;\nuse engine::mon::*;\nuse crate::blob::Blob;\n\n",
        ),
        unsupported_effects: BTreeMap::new(),
        unsupported_abilities: BTreeSet::new(),
        trainer_ids: Vec::new(),
    };
    g.gen_species()?;
    g.gen_moves()?;
    g.gen_trainers()?;
    g.gen_strings()?;
    g.gen_gfx()?;
    g.gen_slice()?;
    g.blob.align();
    writeln!(
        g.o,
        "pub static BLOB: Blob<{}> = Blob(*include_bytes!(concat!(env!(\"OUT_DIR\"), \"/battle.bin\")));",
        g.blob.0.len()
    )?;
    let consts = g.consts();
    Ok(Out {
        rs: g.o,
        bin: g.blob.0,
        consts,
    })
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

/// Every `Ability` with a name (all but `Ability::Other`) and its `ABILITY_*` value, in
/// `ABILITY_*` order.
fn named_abilities() -> Vec<(&'static str, u32)> {
    names::ABILITIES
        .iter()
        .map(|(v, n)| (classify::ability(n), v))
        .filter(|(expr, _)| *expr != "Ability::Other")
        .collect()
}
