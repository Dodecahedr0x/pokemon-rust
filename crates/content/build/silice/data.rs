//! Silice's battle and item data (SIL1 Task A), merged into the generator's `World` over the
//! extract's base tables (species, moves, types, pics, Gen3 items):
//!
//! - **Trainers** ([`trainers`]): `battle/trainers.ron`, numbered by `ids/trainers.ron`, replace
//!   Emerald's: the list is indexed by number, `TRAINER_NONE` stays at 0, a number without a
//!   trainer (a tombstone) is an empty entry (no `trainer()`, no constant). Parties, levels, IVs,
//!   moves, held items (`TrainerMon.item`; Silice turns `Rules::trainer_held_items` on), AI flags,
//!   classes, pics by extract name and names. Names over Gen3's limits (10 bytes, class 12) fail
//!   unless `check-exceptions.ron` lists the trainer (`trainer-name`, `trainer-class-name`): then
//!   they are cut, with a report line. The AI item bags are `data::trainer_items`.
//! - **Encounters** ([`encounters`]): `battle/encounters.ron`, by Silice map file name (numbered
//!   by `ids/maps.ron`, built or not), replace Emerald's: `wild_encounters` and the water, rock
//!   smash and fishing tables.
//! - **Items** ([`items`]): the `ITEM_SIL_*` of `items.ron`, at their `ids/items.ron` numbers
//!   after the Gen3 items: name, price, pocket (the base item's when the source gives none) and
//!   the base item's hold effect (`clone_of`). Incomplete definitions fail unless excepted
//!   (`item-incomplete`): a missing name becomes the id's words, a missing pocket `Items`, a
//!   missing price 0, each reported; a long name (`item-name-length`) is cut. The marts
//!   (`data::MARTS`) list their map and stock. Placements are map events (the maps have them).
//! - **Forms** ([`forms`]): `battle/forms.ron`, the Mega and Primal forms appended to the
//!   species at their `ids/species.ron` numbers, and `data::FORM_CHANGES`.
//!
//! A Silice id is passed to the generator as `<PREFIX>0x<HEX>` (`ITEM_0x17A`), which
//! `extract::names` reads as a number; Gen3 names pass through.

use super::ids::Ids;
use super::load::Exceptions;
use super::schema::{self as games, Currency, IdKind};
use super::text::Encoder;
use crate::input::World;
use anyhow::{Context, Result, ensure};
use extract::names;
use extract::schema::{EncounterHeader, ItemPocket, LandEncounters, Trainer};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

/// `TRAINER_NAME_LENGTH`.
pub const TRAINER_NAME_MAX: usize = 10;
/// `gTrainerClassNames[][13]` without the terminator.
pub const CLASS_NAME_MAX: usize = 12;
/// `ITEM_NAME_LENGTH` 14 without the terminator.
pub const ITEM_NAME_MAX: usize = 13;

/// A constant for `content::consts`: (name, type, value).
pub type Const = (String, &'static str, String);

/// An item name for the generator: a live Silice item as its number (`ITEM_0x17A`), a Gen3
/// name as it is.
pub fn item_ref(ids: &Ids, name: &str) -> Result<String> {
    if let Some(n) = ids.live(IdKind::Items, name)? {
        return Ok(format!("ITEM_0x{n:X}"));
    }
    ensure!(
        names::ITEMS.value(name).is_some(),
        "unknown item {name} (neither ids/items.ron nor Gen3)"
    );
    Ok(name.to_string())
}

/// Replaces `w`'s trainers by `battle/trainers.ron`'s; returns their constants and
/// `data::trainer_items`.
pub fn trainers(
    games: &Path,
    ids: &Ids,
    exc: &Exceptions,
    enc: &Encoder,
    w: &mut World,
    report: &mut Vec<String>,
) -> Result<(Vec<Const>, String)> {
    let file: games::TrainersFile = crate::read_opt!(games.join("battle/trainers.ron"));
    let species: BTreeSet<&str> = w.battle.species.iter().map(|s| s.id.as_str()).collect();
    let moves: BTreeSet<&str> = w.battle.moves.moves.iter().map(|m| m.id.as_str()).collect();
    let none = (w.battle.trainers.first())
        .filter(|t| t.id == "TRAINER_NONE")
        .cloned()
        .context("the extract's trainers do not start with TRAINER_NONE")?;
    let mut by_num: BTreeMap<u32, Trainer> = BTreeMap::new();
    let mut consts = Vec::new();
    let mut bags = 0;
    let mut bag_arms = String::new();
    for t in &file.trainers {
        let at = t.id.as_str();
        let num = ids
            .live(IdKind::Trainers, at)?
            .with_context(|| format!("{at} is not in ids/trainers.ron"))?;
        ensure!(!by_num.contains_key(&num), "{at} is defined twice");
        ensure!(
            (1..=6).contains(&t.party.len()),
            "{at}: {} mons (1 to 6)",
            t.party.len()
        );
        let mut party = Vec::new();
        for m in &t.party {
            ensure!(
                species.contains(m.species.as_str()),
                "{at}: unknown species {}",
                m.species
            );
            ensure!(
                (1..=100).contains(&m.level),
                "{at}: level {} (1 to 100)",
                m.level
            );
            ensure!(m.iv <= 255, "{at}: iv {} (0 to 255)", m.iv);
            // A custom move list has 1 to 4 moves, padded with `MOVE_NONE` (Gen3's 4 slots).
            let custom = match &m.moves {
                Some(ms) => {
                    ensure!(
                        (1..=4).contains(&ms.len()),
                        "{at}: {} moves (1 to 4)",
                        ms.len()
                    );
                    for mv in ms {
                        ensure!(moves.contains(mv.as_str()), "{at}: unknown move {mv}");
                    }
                    let mut ms = ms.clone();
                    ms.resize(4, "MOVE_NONE".into());
                    Some(ms)
                }
                None => None,
            };
            let item = match m.item.as_deref() {
                None | Some("ITEM_NONE") => None,
                Some(i) => Some(item_ref(ids, i).with_context(|| at.to_string())?),
            };
            party.push(extract::schema::TrainerMon {
                item,
                moves: custom,
                ..m.clone()
            });
        }
        for f in &t.ai {
            ensure!(
                names::AI_FLAGS.value(f).is_some(),
                "{at}: unknown AI flag {f}"
            );
        }
        ensure!(
            names::TRAINER_CLASSES.value(&t.class).is_some(),
            "{at}: unknown class {}",
            t.class
        );
        ensure!(
            t.items.len() <= 4,
            "{at}: {} bag items (at most 4)",
            t.items.len()
        );
        let mut bag = Vec::new();
        for i in &t.items {
            let r = item_ref(ids, i).with_context(|| at.to_string())?;
            let v = names::ITEMS
                .value(&r)
                .with_context(|| format!("{at}: item {r}"))?;
            bag.push(format!("ItemId({v})"));
        }
        if !bag.is_empty() {
            bags += 1;
            writeln!(bag_arms, "        {num} => &[{}], // {at}", bag.join(", "))?;
        }
        // The generator encodes the names again (`battle.rs`): pass the text it can encode.
        let name = enc.encode_name(&t.name, TRAINER_NAME_MAX, "trainer-name", at, exc, report)?;
        let class_name = enc.encode_name(
            &t.class_name,
            CLASS_NAME_MAX,
            "trainer-class-name",
            at,
            exc,
            report,
        )?;
        by_num.insert(
            num,
            Trainer {
                id: t.id.clone(),
                class: t.class.clone(),
                class_name: decoded(&class_name, w)?,
                name: decoded(&name, w)?,
                pic: t.pic.clone(),
                female: t.female,
                ai: t.ai.clone(),
                double: t.double,
                party,
            },
        );
        consts.push((t.id.clone(), "TrainerId", format!("TrainerId({num})")));
    }
    report.push(format!("trainers: {bags} AI item bags"));
    let bags_rs = format!(
        "/// Each trainer's AI item bag (`battle/trainers.ron`'s `items`, `GameData::trainer_items`).\npub fn trainer_items(trainer: u16) -> &'static [ItemId] {{\n    match trainer {{\n{bag_arms}        _ => &[],\n    }}\n}}\n"
    );
    // The pics, by extract name: every one must exist; only these are compiled.
    let dir = w.dir.clone().context("the Silice world needs an extract")?;
    let pics_path = dir.join("trainers/pics.ron");
    let pics: extract::schema::TrainerPicsFile = crate::read_opt!(pics_path);
    let used: BTreeSet<&str> = by_num.values().map(|t| t.pic.as_str()).collect();
    let mut kept = BTreeMap::new();
    for p in &used {
        let pic = match w.battle.trainer_pics.remove(*p) {
            Some(pic) => pic,
            None => crate::battle_load::trainer_pic(&dir, &pics, p)?
                .with_context(|| format!("trainer pic {p} is not in the extract"))?,
        };
        kept.insert(p.to_string(), pic);
    }
    w.battle.trainer_pics = kept;
    let max = by_num.keys().max().copied().unwrap_or(0);
    let mut list = vec![none.clone()];
    for n in 1..=max {
        list.push(match by_num.remove(&n) {
            Some(t) => t,
            None => {
                report.push(format!("trainer number {n}: no trainer (left out)"));
                Trainer {
                    id: format!("TRAINER_SIL_NONE_{n}"),
                    party: Vec::new(),
                    ..none.clone()
                }
            }
        });
    }
    w.battle.trainers = list;
    Ok((consts, bags_rs))
}

/// Gen3 bytes back to `.string` text that encodes to the same bytes (the generator encodes
/// trainer and item names itself, with the extract's codec).
fn decoded(b: &[u8], w: &World) -> Result<String> {
    let s = extract::text::decode_lang(b, w.lang)?.0;
    ensure!(
        extract::text::encode_lang(&s, w.lang).ok().as_deref() == Some(b),
        "{s:?} does not round-trip through the charmap"
    );
    Ok(s)
}

/// Replaces `w`'s encounters by `battle/encounters.ron`'s: every map of `ids/maps.ron`.
pub fn encounters(games: &Path, ids: &Ids, w: &mut World) -> Result<usize> {
    let file: games::EncountersFile = crate::read_opt!(games.join("battle/encounters.ron"));
    let species: BTreeSet<&str> = w.battle.species.iter().map(|s| s.id.as_str()).collect();
    let mut out = BTreeMap::new();
    for (map, headers) in &file.maps {
        let e = ids
            .map_file(map)
            .with_context(|| format!("encounters: map {map} is not in ids/maps.ron"))?;
        ensure!(!e.removed, "encounters: map {map} is removed");
        let mut hs = Vec::new();
        for (k, h) in headers.iter().enumerate() {
            let at = format!("encounters {map} #{k}");
            let table = |t: &Option<LandEncounters>, what: &str, slots: usize| -> Result<()> {
                let Some(t) = t else { return Ok(()) };
                ensure!(
                    t.mons.len() == slots,
                    "{at}: {} {what} slots ({slots})",
                    t.mons.len()
                );
                for m in &t.mons {
                    ensure!(
                        species.contains(m.species.as_str()),
                        "{at}: unknown species {}",
                        m.species
                    );
                    ensure!(
                        (1..=100).contains(&m.min_level)
                            && (1..=100).contains(&m.max_level)
                            && m.min_level <= m.max_level,
                        "{at}: levels {}-{}",
                        m.min_level,
                        m.max_level
                    );
                }
                Ok(())
            };
            table(&h.land, "land", 12)?;
            table(&h.water, "water", 5)?;
            table(&h.rock_smash, "rock smash", 5)?;
            table(&h.fishing, "fishing", 10)?;
            hs.push(EncounterHeader {
                land: h.land.clone(),
                water: h.water.clone(),
                rock_smash: h.rock_smash.clone(),
                fishing: h.fishing.clone(),
            });
        }
        out.insert(map.clone(), hs);
    }
    let n = out.len();
    w.battle.encounters = out;
    Ok(n)
}

/// What [`items`] adds besides `w`'s item table.
pub struct ItemsOut {
    pub consts: Vec<Const>,
    /// (number, base `ITEM_*`): the hold effects.
    pub bases: Vec<(u16, String)>,
    /// `data::MARTS` and its types.
    pub data_rs: String,
    pub defined: usize,
    pub marts: usize,
}

/// The Silice items appended to `w`'s item table, and the marts.
pub fn items(
    games: &Path,
    ids: &Ids,
    exc: &Exceptions,
    enc: &Encoder,
    w: &mut World,
    report: &mut Vec<String>,
) -> Result<ItemsOut> {
    let file: games::ItemsFile = crate::read_opt!(games.join("items.ron"));
    let mut consts = Vec::new();
    let mut bases = Vec::new();
    let mut seen = BTreeSet::new();
    let mut defs = Vec::new();
    for d in &file.items {
        let at = d.id.as_str();
        let num = ids
            .live(IdKind::Items, at)?
            .with_context(|| format!("{at} is not in ids/items.ron"))?;
        ensure!(seen.insert(num), "{at} is defined twice");
        let num = u16::try_from(num)?;
        let base = match &d.clone_of {
            Some(b) => {
                let v = (names::ITEMS.value(b))
                    .with_context(|| format!("{at}: base item {b} is not a Gen3 item"))?;
                Some((b.clone(), v as usize))
            }
            None => None,
        };
        let mut missing = Vec::new();
        let name = match &d.name {
            Some(n) => n.clone(),
            None => {
                missing.push("name");
                let words = at.trim_start_matches("ITEM_SIL_").replace('_', " ");
                words.chars().take(ITEM_NAME_MAX).collect::<String>()
            }
        };
        let base_pocket = base
            .as_ref()
            .and_then(|(_, v)| w.tables.items.items.get(*v))
            .and_then(|i| i.pocket);
        let pocket = match d.pocket.or(base_pocket) {
            Some(p) => p,
            None => {
                missing.push("pocket");
                ItemPocket::Items
            }
        };
        if !missing.is_empty() {
            ensure!(
                exc.find("item-incomplete", at).is_some(),
                "{at}: no {} (give it, or list {at} under `item-incomplete` in \
                 check-exceptions.ron)",
                missing.join(", no ")
            );
            report.push(format!(
                "placeholder: {at}: no {} -> {} (item-incomplete exception)",
                missing.join(", no "),
                [
                    missing.contains(&"name").then(|| format!("name {name:?}")),
                    missing
                        .contains(&"pocket")
                        .then(|| "pocket Items".to_string()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
            ));
        }
        let bytes = enc.encode_name(&name, ITEM_NAME_MAX, "item-name-length", at, exc, report)?;
        if let Some((b, _)) = &base {
            bases.push((num, b.clone()));
        }
        defs.push((
            num,
            extract::schema::Item {
                id: d.id.clone(),
                name: decoded(&bytes, w)?,
                pocket: Some(pocket),
                price: d.price.unwrap_or(0),
                importance: u8::from(pocket == ItemPocket::KeyItems),
            },
        ));
        consts.push((d.id.clone(), "u16", format!("{num:#X}")));
    }
    let table = &mut w.tables.items.items;
    for (num, item) in defs {
        let i = usize::from(num);
        ensure!(
            i >= table.len() || table[i].pocket.is_none() && table[i].id.starts_with("ITEM_SIL_"),
            "{} = {num} is a Gen3 item's number",
            item.id
        );
        while table.len() <= i {
            // A number without an item (a tombstone, or Gen3's end): no item.
            let n = table.len();
            table.push(extract::schema::Item {
                id: format!("ITEM_SIL_NONE_{n}"),
                name: "?".into(),
                pocket: None,
                price: 0,
                importance: 0,
            });
        }
        table[i] = item;
    }
    let (data_rs, marts) = marts(&file.marts, ids)?;
    Ok(ItemsOut {
        defined: consts.len(),
        consts,
        bases,
        data_rs,
        marts,
    })
}

/// `data::MARTS`: every mart's map (`None` when the source names none), currency and stock.
fn marts(marts: &[games::Mart], ids: &Ids) -> Result<(String, usize)> {
    let mut o = String::from(
        "/// A Silice mart (`games/silice/items.ron`): where it is, what it takes, its stock.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub struct Mart {\n    \
         /// The map the source names, if any.\n    pub map: Option<MapId>,\n    \
         /// Casino coins instead of money.\n    pub coins: bool,\n    \
         /// (`ITEM_*`, price).\n    pub stock: &'static [(u16, u16)],\n}\n",
    );
    let mut list = String::new();
    for m in marts {
        let map = match &m.map {
            Some(c) => {
                let e = (ids.files.get(&IdKind::Maps).into_iter())
                    .flat_map(|f| &f.ids)
                    .find(|e| &e.name == c && !e.removed)
                    .with_context(|| {
                        format!("mart {:?}: map {c} is not in ids/maps.ron", m.label)
                    })?;
                format!("Some(MapId({:#X}))", e.num)
            }
            None => "None".into(),
        };
        let mut stock = String::new();
        for s in &m.stock {
            let r = item_ref(ids, &s.item).with_context(|| format!("mart {:?}", m.label))?;
            let v = names::ITEMS.value(&r).unwrap();
            write!(stock, "({v:#X}, {}),", s.price)?;
        }
        writeln!(
            list,
            "    Mart {{ map: {map}, coins: {}, stock: &[{stock}] }},",
            m.currency == Currency::Coins
        )?;
    }
    writeln!(
        o,
        "/// Every mart of `items.ron`, in chapter order.\npub static MARTS: [Mart; {}] = [\n{list}];",
        marts.len()
    )?;
    Ok((o, marts.len()))
}

/// What [`forms`] adds besides `w`'s species.
pub struct FormsOut {
    /// `SPECIES_*_MEGA` / `_PRIMAL` -> `SpeciesId`.
    pub consts: Vec<Const>,
    /// `data::FORM_CHANGES`.
    pub data_rs: String,
    pub report: String,
}

/// The battle-only forms of `battle/forms.ron`, appended to `w`'s species at their
/// `ids/species.ron` numbers (a number without a form is a copy of `SPECIES_NONE`), and
/// `data::FORM_CHANGES` (`GameData::form_changes`). A form takes its base species' name,
/// yields, growth, learnset and pics (no form pics yet), and its own stats, types, ability and
/// weight. A form `ids/species.ron` names without data fails, and so does a stone that is not
/// an `ITEM_SIL_*` item.
pub fn forms(games: &Path, ids: &Ids, w: &mut World) -> Result<FormsOut> {
    let file: games::FormsFile = crate::read_opt!(games.join("battle/forms.ron"));
    let enable = match &file.mega_flag {
        Some(f) => Some(
            ids.live(IdKind::Flags, f)?
                .with_context(|| format!("forms.ron: mega_flag {f} is not in ids/flags.ron"))?,
        ),
        None => None,
    };
    let named: Vec<(String, u32)> = (ids.files.get(&IdKind::Species).into_iter())
        .flat_map(|f| &f.ids)
        .filter(|e| !e.removed)
        .map(|e| (e.name.clone(), e.num))
        .collect();
    for (n, _) in &named {
        ensure!(
            file.forms.iter().any(|f| &f.id == n),
            "{n} (ids/species.ron) has no data in battle/forms.ron"
        );
    }
    let species = &mut w.battle.species;
    let mut consts = Vec::new();
    let mut changes = String::new();
    let mut seen = BTreeSet::new();
    for f in &file.forms {
        let at = f.id.as_str();
        let num = ids
            .live(IdKind::Species, at)?
            .with_context(|| format!("{at} is not in ids/species.ron"))? as usize;
        ensure!(
            seen.insert(f.item.clone()),
            "{at}: {} triggers two forms",
            f.item
        );
        let from_no = (species.iter().position(|s| s.id == f.from))
            .with_context(|| format!("{at}: base species {} is not a Gen3 species", f.from))?;
        let item = (ids.live(IdKind::Items, &f.item)?)
            .with_context(|| format!("{at}: {} is not in ids/items.ron", f.item))?;
        let base = species[from_no].clone();
        while species.len() <= num {
            let n = species.len();
            species.push(extract::schema::Species {
                id: format!("SPECIES_0x{n:X}"),
                ..species[0].clone()
            });
        }
        let id = format!("SPECIES_0x{num:X}");
        if let Some(g) = w.battle.mon_gfx.get(&f.from).cloned() {
            w.battle.mon_gfx.insert(id.clone(), g);
        }
        species[num] = extract::schema::Species {
            id,
            base: f.base,
            types: f.types.clone(),
            abilities: [f.ability.clone(), "ABILITY_NONE".into()],
            weight: f.weight,
            ..base
        };
        consts.push((f.id.clone(), "SpeciesId", format!("SpeciesId({num})")));
        let kind = match f.kind {
            games::FormKindDef::Mega => "Mega",
            games::FormKindDef::Primal => "Primal",
        };
        let flag = match (f.kind, enable) {
            (games::FormKindDef::Mega, Some(n)) => format!("Some(FlagId({n:#X}))"),
            _ => "None".into(),
        };
        writeln!(
            changes,
            "    FormChange {{ base: SpeciesId({from_no}), item: ItemId({item}), form: SpeciesId({num}), kind: FormKind::{kind}, enable_flag: {flag} }}, // {at}"
        )?;
    }
    let data_rs = format!(
        "/// Every battle-only form of `battle/forms.ron` (`GameData::form_changes`).\npub static FORM_CHANGES: [FormChange; {}] = [\n{changes}];\n",
        file.forms.len()
    );
    Ok(FormsOut {
        report: format!(
            "form changes: {} ({} Mega, {} Primal), pics from the base species",
            file.forms.len(),
            (file.forms.iter())
                .filter(|f| f.kind == games::FormKindDef::Mega)
                .count(),
            (file.forms.iter())
                .filter(|f| f.kind == games::FormKindDef::Primal)
                .count()
        ),
        consts,
        data_rs,
    })
}
