//! Read-only data views: rows built from `content::battle` and `content::data`, with names
//! decoded by [`crate::text::decode`].
//!
//! See `docs/plans/2026-10-22-milestone-6-studio.md` (Task F). Rows are rebuilt on each call;
//! the UI builds them once and keeps them.

use crate::text::decode;
use content::battle;
use engine::battle::Str;
use engine::data::{MapId, MoveId, SpeciesId, TextId, TrainerId};
use engine::mon::{Ability, GrowthRate, MoveEffect, MoveFlags, MoveTarget, Type};
use std::collections::BTreeSet;

/// Gen3 land encounter slot rates in percent (`gWildMonHeaders` land slots 0..12).
pub const LAND_SLOT_PERCENT: [u8; 12] = [20, 20, 10, 10, 10, 10, 5, 5, 4, 4, 1, 1];

#[derive(Clone, Debug, PartialEq)]
pub struct SpeciesRow {
    pub id: SpeciesId,
    pub name: String,
    /// HP, Atk, Def, Speed, SpAtk, SpDef.
    pub base: [u8; 6],
    pub types: [Type; 2],
    /// The engine's view: abilities it does not implement are `Ability::Other`.
    pub abilities: [Ability; 2],
    /// The real names (`gAbilityNames` of the species' raw `ABILITY_*` ids).
    pub ability_names: [String; 2],
    pub catch_rate: u8,
    pub exp_yield: u8,
    pub growth: GrowthRate,
    pub gender_ratio: u8,
    /// Level, move, decoded move name.
    pub learnset: Vec<(u8, MoveId, String)>,
    pub has_gfx: bool,
    /// Trainers (of `TRAINER_IDS`) whose party has this species, sorted.
    pub trainers: Vec<TrainerId>,
    /// Maps whose land table has this species, sorted.
    pub maps: Vec<MapId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoveRow {
    pub id: MoveId,
    pub name: String,
    pub ty: Type,
    pub power: u8,
    pub accuracy: u8,
    pub pp: u8,
    pub effect: MoveEffect,
    pub effect_chance: u8,
    pub target: MoveTarget,
    pub priority: i8,
    pub flags: MoveFlags,
    /// Species whose level-up learnset has this move, sorted, without repeats.
    pub learners: Vec<SpeciesId>,
}

/// One trainer party member.
#[derive(Clone, Debug, PartialEq)]
pub struct PartyRow {
    pub species: SpeciesId,
    pub name: String,
    pub level: u8,
    /// 0..=255 (`TrainerMon::iv`).
    pub iv: u8,
    /// The custom moveset (empty slots left out), `None` for the level-up default.
    pub moves: Option<Vec<(MoveId, String)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainerRow {
    pub id: TrainerId,
    pub class: String,
    pub name: String,
    pub female: bool,
    pub double: bool,
    /// Gen3 `AI_SCRIPT_*` bits.
    pub ai: u32,
    pub party: Vec<PartyRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncounterRow {
    pub map: MapId,
    /// The decomp map identifier (`MapData::name`), not ROM text.
    pub map_name: &'static str,
    pub rate: u8,
    /// Per slot: species, decoded name, min level, max level, slot percent
    /// ([`LAND_SLOT_PERCENT`]).
    pub mons: Vec<(SpeciesId, String, u8, u8, u8)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRow {
    pub id: TextId,
    pub key: &'static str,
    pub text: String,
}

/// The decoded name of type `t` (`gTypeNames`).
pub fn type_name(t: Type) -> String {
    decode(battle::text(Str::TypeName(t)))
}

/// The decoded name of raw ability id `id` (`gAbilityNames[id]`); empty for an unknown id.
pub fn ability_name(id: u8) -> String {
    battle::ABILITY_NAMES
        .get(id as usize)
        .map_or_else(String::new, |n| decode(n))
}

fn species_name(id: SpeciesId) -> String {
    battle::TABLES
        .species
        .get(id.0 as usize)
        .map_or_else(String::new, |s| decode(s.name))
}

fn move_name(id: MoveId) -> String {
    battle::TABLES
        .moves
        .get(id.0 as usize)
        .map_or_else(String::new, |m| decode(m.name))
}

/// Every species of `TABLES` (index = `SpeciesId`), with trainer and map cross references.
pub fn species() -> Vec<SpeciesRow> {
    let n = battle::TABLES.species.len();
    let mut trainers = vec![BTreeSet::new(); n];
    for &t in &battle::TRAINER_IDS {
        for m in battle::trainer(t.0).map_or(&[][..], |d| d.party) {
            if let Some(s) = trainers.get_mut(m.species.0 as usize) {
                s.insert(t);
            }
        }
    }
    let mut maps = vec![BTreeSet::new(); n];
    for &map in &content::data::MAP_IDS {
        for m in battle::encounters(map.0).map_or(&[][..], |e| &e.mons[..]) {
            if let Some(s) = maps.get_mut(m.species.0 as usize) {
                s.insert(map);
            }
        }
    }
    battle::TABLES
        .species
        .iter()
        .zip(trainers.into_iter().zip(maps))
        .enumerate()
        .map(|(i, (s, (trainers, maps)))| SpeciesRow {
            id: SpeciesId(i as u16),
            name: decode(s.name),
            base: s.base,
            types: s.types,
            abilities: s.abilities,
            ability_names: battle::SPECIES_ABILITY_IDS[i].map(ability_name),
            catch_rate: s.catch_rate,
            exp_yield: s.exp_yield,
            growth: s.growth,
            gender_ratio: s.gender_ratio,
            learnset: s
                .learnset
                .iter()
                .map(|l| (l.level, l.mov, move_name(l.mov)))
                .collect(),
            has_gfx: s.gfx.is_some(),
            trainers: trainers.into_iter().collect(),
            maps: maps.into_iter().collect(),
        })
        .collect()
}

/// Every move of `TABLES` (index = `MoveId`), with its learners.
pub fn moves() -> Vec<MoveRow> {
    let mut learners = vec![BTreeSet::new(); battle::TABLES.moves.len()];
    for (i, s) in battle::TABLES.species.iter().enumerate() {
        for l in s.learnset {
            if let Some(v) = learners.get_mut(l.mov.0 as usize) {
                v.insert(SpeciesId(i as u16));
            }
        }
    }
    battle::TABLES
        .moves
        .iter()
        .zip(learners)
        .enumerate()
        .map(|(i, (m, learners))| MoveRow {
            id: MoveId(i as u16),
            name: decode(m.name),
            ty: m.ty,
            power: m.power,
            accuracy: m.accuracy,
            pp: m.pp,
            effect: m.effect,
            effect_chance: m.effect_chance,
            target: m.target,
            priority: m.priority,
            flags: m.flags,
            learners: learners.into_iter().collect(),
        })
        .collect()
}

/// The exported trainers (`TRAINER_IDS` order).
pub fn trainers() -> Vec<TrainerRow> {
    battle::TRAINER_IDS
        .iter()
        .filter_map(|&id| Some((id, battle::trainer(id.0)?)))
        .map(|(id, t)| TrainerRow {
            id,
            class: decode(t.class_name),
            name: decode(t.name),
            female: t.female,
            double: t.double,
            ai: t.ai,
            party: t
                .party
                .iter()
                .map(|m| PartyRow {
                    species: m.species,
                    name: species_name(m.species),
                    level: m.level,
                    iv: m.iv,
                    moves: m
                        .moves
                        .map(|ms| ms.iter().flatten().map(|&mv| (mv, move_name(mv))).collect()),
                })
                .collect(),
        })
        .collect()
}

/// The land tables of the exported maps (`MAP_IDS` order; maps without one are left out).
pub fn encounters() -> Vec<EncounterRow> {
    content::data::MAP_IDS
        .iter()
        .filter_map(|&map| Some((map, battle::encounters(map.0)?)))
        .map(|(map, e)| EncounterRow {
            map,
            map_name: content::data::map(map.0).map_or("", |m| m.name),
            rate: e.rate,
            mons: e
                .mons
                .iter()
                .zip(LAND_SLOT_PERCENT)
                .map(|(m, pct)| {
                    (
                        m.species,
                        species_name(m.species),
                        m.min_level,
                        m.max_level,
                        pct,
                    )
                })
                .collect(),
        })
        .collect()
}

/// Every text (`TEXTS` / `TEXT_NAMES`; index = `TextId`).
pub fn texts() -> Vec<TextRow> {
    content::data::TEXTS
        .iter()
        .zip(content::data::TEXT_NAMES)
        .enumerate()
        .map(|(i, (bytes, key))| TextRow {
            id: TextId(i as u16),
            key,
            text: decode(bytes),
        })
        .collect()
}

/// The rows whose `key` contains `q`, ignoring case (Unicode lower-casing, so accented names
/// match too). An empty or blank query keeps every row.
pub fn filter<'a, T>(rows: &'a [T], q: &str, key: impl Fn(&T) -> String) -> Vec<&'a T> {
    let q = q.trim().to_lowercase();
    rows.iter()
        .filter(|r| q.is_empty() || key(r).to_lowercase().contains(&q))
        .collect()
}

// ---- The Data tab's model (M6 Task H): rows, lists, links and text layout, free of the UI. ----

/// Every row of every Data sub-tab.
pub struct DataRows {
    pub species: Vec<SpeciesRow>,
    pub moves: Vec<MoveRow>,
    pub trainers: Vec<TrainerRow>,
    pub encounters: Vec<EncounterRow>,
    pub texts: Vec<TextRow>,
}

static ROWS: std::sync::OnceLock<DataRows> = std::sync::OnceLock::new();

/// The rows, built on first use (on the calling thread) and kept for the process.
pub fn rows() -> &'static DataRows {
    ROWS.get_or_init(DataRows::build)
}

/// The rows if they are built, without building them.
pub fn rows_if_ready() -> Option<&'static DataRows> {
    ROWS.get()
}

impl DataRows {
    pub fn build() -> DataRows {
        DataRows {
            species: species(),
            moves: moves(),
            trainers: trainers(),
            encounters: encounters(),
            texts: texts(),
        }
    }

    /// Species and move ids index their rows.
    pub fn species(&self, id: SpeciesId) -> Option<&SpeciesRow> {
        self.species.get(id.0 as usize)
    }

    pub fn mov(&self, id: MoveId) -> Option<&MoveRow> {
        self.moves.get(id.0 as usize)
    }

    pub fn trainer(&self, id: TrainerId) -> Option<&TrainerRow> {
        self.trainers.iter().find(|t| t.id == id)
    }

    pub fn encounter(&self, map: MapId) -> Option<&EncounterRow> {
        self.encounters.iter().find(|e| e.map == map)
    }

    pub fn text(&self, id: TextId) -> Option<&TextRow> {
        self.texts.get(id.0 as usize)
    }

    /// Whether the row `p` names exists.
    pub fn has(&self, p: Pick) -> bool {
        match p {
            Pick::Species(id) => self.species(id).is_some(),
            Pick::Move(id) => self.mov(id).is_some(),
            Pick::Trainer(id) => self.trainer(id).is_some(),
            Pick::Map(id) => self.encounter(id).is_some(),
            Pick::Text(id) => self.text(id).is_some(),
        }
    }

    /// The list of sub-tab `sub` filtered by `q` ([`filter`] on what the list shows plus, for
    /// texts, the text): what to pick and its label.
    pub fn list(&self, sub: Sub, q: &str) -> Vec<(Pick, String)> {
        match sub {
            Sub::Species => filter(&self.species, q, |r| r.name.clone())
                .into_iter()
                .map(|r| (Pick::Species(r.id), format!("#{:03} {}", r.id.0, r.name)))
                .collect(),
            Sub::Moves => filter(&self.moves, q, |r| r.name.clone())
                .into_iter()
                .map(|r| (Pick::Move(r.id), format!("#{:03} {}", r.id.0, r.name)))
                .collect(),
            Sub::Trainers => filter(&self.trainers, q, trainer_label)
                .into_iter()
                .map(|r| (Pick::Trainer(r.id), trainer_label(r)))
                .collect(),
            Sub::Encounters => filter(&self.encounters, q, |r| r.map_name.into())
                .into_iter()
                .map(|r| (Pick::Map(r.map), r.map_name.to_string()))
                .collect(),
            Sub::Texts => filter(&self.texts, q, |r| format!("{} {}", r.key, r.text))
                .into_iter()
                .map(|r| (Pick::Text(r.id), r.key.to_string()))
                .collect(),
        }
    }
}

/// "Class Name".
pub fn trainer_label(t: &TrainerRow) -> String {
    format!("{} {}", t.class, t.name)
}

/// The Data sub-tabs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sub {
    #[default]
    Species,
    Moves,
    Trainers,
    Encounters,
    Texts,
}

/// A row to show in the Data tab's detail panel (and the target of its links).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Species(SpeciesId),
    Move(MoveId),
    Trainer(TrainerId),
    /// An encounter table, by map.
    Map(MapId),
    Text(TextId),
}

impl Pick {
    /// The sub-tab that shows it.
    pub fn sub(self) -> Sub {
        match self {
            Pick::Species(_) => Sub::Species,
            Pick::Move(_) => Sub::Moves,
            Pick::Trainer(_) => Sub::Trainers,
            Pick::Map(_) => Sub::Encounters,
            Pick::Text(_) => Sub::Texts,
        }
    }
}

/// A piece of decoded text: escapes become breaks, placeholders tags.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Part {
    Text(String),
    /// `\n` (new line), `\l` (scroll a line) or `\p` (new box).
    Break(char),
    /// `{PLAYER}`, `{STRING 3}`, … without the braces.
    Tag(String),
}

/// Splits a [`decode`] string into [`Part`]s. Other backslashes and an unclosed `{` stay text.
pub fn text_parts(s: &str) -> Vec<Part> {
    fn flush(text: &mut String, parts: &mut Vec<Part>) {
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(text)));
        }
    }
    let (mut parts, mut text) = (Vec::new(), String::new());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let next = rest[c.len_utf8()..].chars().next();
        match (c, next) {
            ('\\', Some(e @ ('n' | 'l' | 'p'))) => {
                flush(&mut text, &mut parts);
                parts.push(Part::Break(e));
                rest = &rest[2..];
            }
            ('{', _) if rest.contains('}') => {
                flush(&mut text, &mut parts);
                let end = rest.find('}').unwrap_or(0);
                parts.push(Part::Tag(rest[1..end].to_string()));
                rest = &rest[end + 1..];
            }
            _ => {
                text.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    flush(&mut text, &mut parts);
    parts
}

/// The gender split of a Gen3 gender ratio (female when `personality & 0xFF < ratio`).
pub fn gender(ratio: u8) -> String {
    match ratio {
        255 => "genderless".into(),
        254 => "always female".into(),
        0 => "always male".into(),
        r => {
            let tenths = r as u32 * 1000 / 256;
            format!("{}.{}% female", tenths / 10, tenths % 10)
        }
    }
}

/// An ability's name, "(other)" when it has none.
pub fn ability_or_other(name: &str) -> &str {
    if name.is_empty() { "(other)" } else { name }
}

/// `0` as a dash (power and accuracy of status moves).
pub fn or_dash(v: u8) -> String {
    if v == 0 { "—".into() } else { v.to_string() }
}
