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
