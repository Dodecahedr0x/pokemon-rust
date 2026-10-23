//! The feature universe, read from a pret/pokeemerald checkout: the constants of every
//! [`Category`], and where each one is used.
//!
//! - **Scripts.** Every label of `data/maps/*/scripts.inc` (module: the map), `data/scripts/*.inc`
//!   (module `Common_<stem>`) and `data/event_scripts.s` (`Common_event_scripts`). A line whose
//!   first word is an `event.inc` macro is a site of the commands it expands to (helper macros
//!   such as `msgbox` expand recursively); `special X`/`specialvar V, X` is also a site of
//!   `SPECIAL_X`. A map uses what its own labels and the labels its `map.json` events name reach,
//!   through every label any of them names (`call`, `goto`, map scripts, …) and the std scripts
//!   (`callstd`/`msgbox` constants, through `gStdScripts`).
//! - **Layouts.** A map uses the behaviours of its layout's cells (`map.bin`, the tilesets'
//!   `metatile_attributes.bin`, Emerald's low byte).
//! - **Battles.** A map uses the move effects, abilities and hold effects of the trainers its
//!   scripts battle (`trainerbattle … TRAINER_*`, `src/data/trainer_parties.h`, default moves from
//!   the level-up learnsets) and of its land wild mons (`src/data/wild_encounters.json`, at their
//!   maximum level).
//! - **Items.** A map uses the items its scripts and hidden items name (`ITEM_*`).

use crate::Category;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One map of `data/maps/map_groups.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MapInfo {
    /// The folder name (`Route101`).
    pub name: String,
    /// `MAP_ROUTE101`.
    pub constant: String,
    /// `group << 8 | num`, the engine's `MapId`.
    pub id: u16,
    pub map_type: String,
    pub weather: String,
    pub requires_flash: bool,
    /// A `dive` or `emerge` connection.
    pub dive: bool,
    /// Behaviour name -> cells.
    pub behaviors: BTreeMap<String, usize>,
    /// The object events' `OBJ_EVENT_GFX_*`.
    pub gfx: BTreeSet<String>,
    /// `ITEM_*` the reached scripts and the hidden items name.
    pub items: BTreeSet<String>,
    /// `TRAINER_*` the reached scripts battle.
    pub trainers: BTreeSet<String>,
    /// Command or special name -> sites in the reached scripts.
    pub script_sites: BTreeMap<String, usize>,
    /// Move effects, abilities and hold effects of the trainers and wild mons.
    pub battle: BTreeSet<(Category, String)>,
}

/// An item of `src/data/items.h`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub hold_effect: Option<String>,
    pub field_use: Option<String>,
    pub battle_use: Option<String>,
}

/// One trainer party member.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartyMon {
    pub species: String,
    pub level: u8,
    pub item: Option<String>,
    /// Empty: the default moves.
    pub moves: Vec<String>,
}

/// The universe and its uses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Decomp {
    pub maps: Vec<MapInfo>,
    /// Every entry name per category (in decomp order).
    pub universe: BTreeMap<Category, Vec<String>>,
    /// Command or special name -> sites in every script (each label once).
    pub script_sites: BTreeMap<String, usize>,
    pub items: BTreeMap<String, Item>,
    /// `MOVE_*` -> `EFFECT_*`.
    pub moves: BTreeMap<String, String>,
    /// `SPECIES_*` -> its abilities (without `ABILITY_NONE`).
    pub species_abilities: BTreeMap<String, Vec<String>>,
    /// `SPECIES_*` -> the level-up learnset.
    pub learnsets: BTreeMap<String, Vec<(u8, String)>>,
    /// `TRAINER_*` -> its party.
    pub trainers: BTreeMap<String, Vec<PartyMon>>,
    /// Map constant -> (species, max level) of its wild encounters.
    pub wild: BTreeMap<String, Vec<(String, u8)>>,
    /// The values of the `#define`d constants of the universe (`MAP_TYPE_INDOOR` -> 8).
    pub values: BTreeMap<String, i64>,
    /// What could not be read (missing files, unknown names), for the ledger's issues.
    pub issues: Vec<String>,
}

fn read(root: &Path, rel: &str) -> Result<String> {
    let p = root.join(rel);
    std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whole identifiers of `s`.
fn idents(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !is_ident(c)).filter(|w| !w.is_empty())
}

/// `src` without `//` and `/* */` comments.
fn strip_c_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while !rest.is_empty() {
        let line = rest.find("//");
        let block = rest.find("/*");
        match (line, block) {
            (Some(l), b) if b.is_none_or(|b| l < b) => {
                out.push_str(&rest[..l]);
                rest = rest[l..].find('\n').map_or("", |e| &rest[l + e..]);
            }
            (_, Some(b)) => {
                out.push_str(&rest[..b]);
                rest = rest[b..].find("*/").map_or("", |e| &rest[b + e + 2..]);
            }
            _ => {
                out.push_str(rest);
                rest = "";
            }
        }
    }
    out
}

/// A C integer: decimal, hex, or `(1 << n)`.
fn c_int(v: &str) -> Option<i64> {
    let v = v
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    if let Some((a, b)) = v.split_once("<<") {
        return Some(c_int(a)? << c_int(b)?);
    }
    if let Some(h) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
        return i64::from_str_radix(h, 16).ok();
    }
    v.parse().ok()
}

/// `#define <prefix>NAME <integer>` names of a header, in file order.
pub fn defines(src: &str, prefix: &str) -> Vec<(String, i64)> {
    strip_c_comments(src)
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("#define")?.trim_start();
            let (name, value) = rest.split_once(char::is_whitespace)?;
            let v = c_int(value)?;
            name.starts_with(prefix).then(|| (name.to_string(), v))
        })
        .collect()
}

/// The `prefix*` members of the first C `enum` that has one, with their values.
pub fn enum_members(src: &str, prefix: &str) -> Vec<(String, i64)> {
    let src = strip_c_comments(src);
    let mut out = Vec::new();
    let mut next = 0i64;
    let mut inside = false;
    for l in src.lines() {
        let t = l.trim();
        if !inside {
            if t.starts_with("enum") {
                inside = true;
                next = 0;
                out.clear();
            }
            continue;
        }
        if t.starts_with('}') {
            if out.is_empty() {
                inside = false;
                continue;
            }
            break;
        }
        for item in t.split(',') {
            let item = item.trim();
            let (name, value) = match item.split_once('=') {
                Some((n, v)) => (n.trim(), c_int(v)),
                None => (item, None),
            };
            if name.is_empty() || !name.chars().all(is_ident) {
                continue;
            }
            let v = value.unwrap_or(next);
            next = v + 1;
            if name.starts_with(prefix) {
                out.push((name.to_string(), v));
            }
        }
    }
    out
}

/// `[KEY] = { .field = value, … }` blocks of a C table whose keys start with `prefix`: key ->
/// field -> value (trimmed, without the trailing comma).
pub fn designated(src: &str, prefix: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let src = strip_c_comments(src);
    let mut out: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for l in src.lines() {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix('[')
            && let Some((key, after)) = rest.split_once(']')
            && key.starts_with(prefix)
            && after.trim_start().starts_with('=')
        {
            out.push((key.to_string(), BTreeMap::new()));
            continue;
        }
        if let Some(rest) = t.strip_prefix('.')
            && let Some((f, v)) = rest.split_once('=')
            && let Some((_, fields)) = out.last_mut()
        {
            let v = v.trim().trim_end_matches(',').trim();
            fields
                .entry(f.trim().to_string())
                .or_insert_with(|| v.to_string());
        }
    }
    out
}

/// A `event.inc` macro: whether it emits a `SCR_OP_*` and the macros its body calls.
#[derive(Clone, Debug, Default)]
struct Macro {
    command: bool,
    calls: Vec<String>,
}

fn parse_macros(src: &str) -> BTreeMap<String, Macro> {
    let mut out: BTreeMap<String, Macro> = BTreeMap::new();
    let mut cur: Option<(String, Macro)> = None;
    let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
    for l in src.lines() {
        let t = l.split('@').next().unwrap_or("").trim();
        if let Some(rest) = t.strip_prefix(".macro") {
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            cur = Some((name, Macro::default()));
            bodies.push((String::new(), Vec::new()));
            continue;
        }
        if t.starts_with(".endm") {
            if let Some((n, m)) = cur.take() {
                bodies.last_mut().unwrap().0 = n.clone();
                out.insert(n, m);
            }
            continue;
        }
        if let Some((_, m)) = cur.as_mut() {
            if t.contains("SCR_OP_") {
                m.command = true;
            }
            if let Some(w) = t.split(|c: char| c.is_whitespace() || c == ',').next()
                && !w.is_empty()
                && !w.starts_with('.')
            {
                bodies.last_mut().unwrap().1.push(w.to_string());
            }
        }
    }
    for (n, words) in bodies {
        let calls: Vec<String> = words
            .into_iter()
            .filter(|w| out.contains_key(w) && *w != n)
            .collect();
        if let Some(m) = out.get_mut(&n) {
            m.calls = calls;
        }
    }
    out
}

/// The commands macro `name` runs (itself for a command).
fn expand(macros: &BTreeMap<String, Macro>, name: &str, depth: u8, out: &mut Vec<String>) {
    let Some(m) = macros.get(name) else { return };
    if m.command {
        out.push(name.to_string());
        return;
    }
    if depth > 8 {
        return;
    }
    for c in &m.calls {
        expand(macros, c, depth + 1, out);
    }
}

/// One script label's uses.
#[derive(Clone, Debug, Default)]
struct Label {
    module: String,
    sites: BTreeMap<String, usize>,
    words: BTreeSet<String>,
    /// `TRAINER_*` of its `trainerbattle*` lines (not `settrainerflag` and the like).
    trainers: BTreeSet<String>,
}

/// `.inc`/`.s` script source -> labels (in `module`).
fn scan_scripts(
    src: &str,
    module: &str,
    macros: &BTreeMap<String, Macro>,
    labels: &mut BTreeMap<String, Label>,
) {
    let mut cur: Option<String> = None;
    for l in src.lines() {
        let line = l.split('@').next().unwrap_or("");
        let mut body = line;
        if !line.starts_with(|c: char| c.is_whitespace())
            && let Some((name, rest)) = line.split_once(':')
            && !name.is_empty()
            && name.chars().all(is_ident)
        {
            cur = Some(name.to_string());
            labels.entry(name.to_string()).or_insert_with(|| Label {
                module: module.to_string(),
                ..Label::default()
            });
            body = rest.trim_start_matches(':');
        }
        let Some(label) = cur.as_ref() else { continue };
        let t = body.trim();
        if t.is_empty() || t.starts_with('.') {
            // Data (`.4byte Label`, `.string`): still references.
            if t.starts_with(".4byte") || t.starts_with(".2byte") {
                let e = labels.get_mut(label).unwrap();
                e.words.extend(idents(t).skip(1).map(str::to_string));
            }
            continue;
        }
        let (word, args) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
        let e = labels.get_mut(label).unwrap();
        e.words.extend(idents(args).map(str::to_string));
        if word.starts_with("trainerbattle") {
            e.trainers.extend(
                idents(args)
                    .filter(|w| w.starts_with("TRAINER_"))
                    .map(str::to_string),
            );
        }
        let mut cmds = Vec::new();
        expand(macros, word, 0, &mut cmds);
        for c in cmds {
            *e.sites.entry(c).or_default() += 1;
        }
        let special = match word {
            "special" => args.split(',').next(),
            "specialvar" => args.split(',').nth(1),
            _ => None,
        };
        if let Some(s) = special.map(str::trim).filter(|s| !s.is_empty()) {
            let s = if s.starts_with("SPECIAL_") {
                s.to_string()
            } else {
                format!("SPECIAL_{s}")
            };
            *e.sites.entry(s).or_default() += 1;
        }
    }
}

/// `gStdScripts` order: std constant (`STD_*`, `MSGBOX_*`, from the `@` comments) -> label.
fn std_scripts(src: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    for l in src.lines() {
        let t = l.trim();
        if t.starts_with("gStdScripts::") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        let Some(rest) = t.strip_prefix(".4byte") else {
            break;
        };
        let (label, constant) = rest.split_once('@').unwrap_or((rest, ""));
        let (label, constant) = (label.trim(), constant.trim());
        if !constant.is_empty() {
            out.insert(constant.to_string(), label.to_string());
        }
    }
    out
}

/// Files of `dir` with extension `ext`, sorted.
fn files(dir: &Path, ext: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// The default moves of `species` at `level` (Gen3 `GiveMonInitialMoveset`: the last four
/// learnset moves at or below the level, no duplicates).
pub fn default_moves(learnset: &[(u8, String)], level: u8) -> Vec<String> {
    let mut moves: Vec<String> = Vec::new();
    for (l, m) in learnset {
        if *l > level {
            break;
        }
        if moves.contains(m) {
            continue;
        }
        if moves.len() == 4 {
            moves.remove(0);
        }
        moves.push(m.clone());
    }
    moves
}

impl Decomp {
    /// Reads the decomp at `root`. A missing optional file is an issue, not an error.
    pub fn load(root: &Path) -> Result<Decomp> {
        let mut d = Decomp::default();
        let opt = |rel: &str, d: &mut Decomp| -> String {
            read(root, rel).unwrap_or_else(|e| {
                d.issues.push(format!("decomp: {e:#}"));
                String::new()
            })
        };

        // Constants.
        let event = opt("asm/macros/event.inc", &mut d);
        let macros = parse_macros(&event);
        let mut commands: Vec<String> = macros
            .iter()
            .filter(|(_, m)| m.command)
            .map(|(n, _)| n.clone())
            .collect();
        // Handlers without a macro of their name (`ScrCmd_addobjectat`, emitted by
        // `addobject` with a position): commands by their handler's name.
        for l in opt("data/script_cmd_table.inc", &mut d).lines() {
            let w: Vec<&str> = idents(l).collect();
            if let ["script_cmd_table_entry", _, handler, ..] = w[..]
                && let Some(name) = handler.strip_prefix("ScrCmd_")
                && !commands.iter().any(|c| c == name)
            {
                commands.push(name.to_string());
            }
        }
        d.universe.insert(Category::Command, commands);
        let specials: Vec<String> = opt("data/specials.inc", &mut d)
            .lines()
            .filter_map(|l| l.trim().strip_prefix("def_special"))
            .map(|n| format!("SPECIAL_{}", n.split('@').next().unwrap_or("").trim()))
            .collect();
        d.universe.insert(Category::Special, specials);
        let mb = enum_members(
            &opt("include/constants/metatile_behaviors.h", &mut d),
            "MB_",
        );
        let behavior_names: BTreeMap<i64, String> =
            mb.iter().map(|(n, v)| (*v, n.clone())).collect();
        d.universe
            .insert(Category::Behavior, mb.into_iter().map(|(n, _)| n).collect());
        for (cat, rel, prefix) in [
            (
                Category::MoveEffect,
                "include/constants/battle_move_effects.h",
                "EFFECT_",
            ),
            (
                Category::Ability,
                "include/constants/abilities.h",
                "ABILITY_",
            ),
            (
                Category::HoldEffect,
                "include/constants/hold_effects.h",
                "HOLD_EFFECT_",
            ),
            (Category::Weather, "include/constants/weather.h", "WEATHER_"),
            (
                Category::BattleType,
                "include/constants/battle.h",
                "BATTLE_TYPE_",
            ),
            (
                Category::MapType,
                "include/constants/map_types.h",
                "MAP_TYPE_",
            ),
        ] {
            let v = defines(&opt(rel, &mut d), prefix);
            d.values.extend(v.iter().cloned());
            d.universe
                .insert(cat, v.into_iter().map(|(n, _)| n).collect());
        }
        let party_menu = opt("src/party_menu.c", &mut d);
        let field_moves: Vec<String> = enum_members(&party_menu, "FIELD_MOVE_")
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        d.universe.insert(Category::FieldMove, field_moves);

        // Items, moves, species.
        for (key, f) in designated(&opt("src/data/items.h", &mut d), "ITEM_") {
            let some = |k: &str| {
                f.get(k)
                    .filter(|v| !v.is_empty() && *v != "NULL" && *v != "0")
                    .cloned()
            };
            d.items.insert(
                key,
                Item {
                    hold_effect: some("holdEffect").filter(|h| h != "HOLD_EFFECT_NONE"),
                    field_use: some("fieldUseFunc"),
                    battle_use: some("battleUseFunc"),
                },
            );
        }
        let uses = |f: fn(&Item) -> &Option<String>, d: &Decomp| -> Vec<String> {
            let s: BTreeSet<String> = d.items.values().filter_map(|i| f(i).clone()).collect();
            s.into_iter().collect()
        };
        let fu = uses(|i| &i.field_use, &d);
        let bu = uses(|i| &i.battle_use, &d);
        d.universe.insert(Category::ItemFieldUse, fu);
        d.universe.insert(Category::ItemBattleUse, bu);
        for (key, f) in designated(&opt("src/data/battle_moves.h", &mut d), "MOVE_") {
            if let Some(e) = f.get("effect") {
                d.moves.insert(key, e.clone());
            }
        }
        for (key, f) in designated(&opt("src/data/pokemon/species_info.h", &mut d), "SPECIES_") {
            if let Some(a) = f.get("abilities") {
                let v: Vec<String> = idents(a)
                    .filter(|w| w.starts_with("ABILITY_") && *w != "ABILITY_NONE")
                    .map(str::to_string)
                    .collect();
                d.species_abilities.insert(key, v);
            }
        }
        let mut sets: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
        let mut cur: Option<String> = None;
        for l in opt("src/data/pokemon/level_up_learnsets.h", &mut d).lines() {
            if l.contains("LevelUpLearnset[]") {
                cur = idents(l)
                    .find(|w| w.ends_with("LevelUpLearnset"))
                    .map(str::to_string);
                continue;
            }
            if let (Some(c), Some(rest)) = (&cur, l.trim().strip_prefix("LEVEL_UP_MOVE("))
                && let Some((lvl, mv)) = rest.split_once(',')
                && let Ok(lvl) = lvl.trim().parse::<u8>()
            {
                let mv = mv
                    .trim()
                    .trim_end_matches("),")
                    .trim_end_matches(')')
                    .trim();
                sets.entry(c.clone())
                    .or_default()
                    .push((lvl, mv.to_string()));
            }
        }
        for l in opt("src/data/pokemon/level_up_learnset_pointers.h", &mut d).lines() {
            let w: Vec<&str> = idents(l).collect();
            if let [sp, set] = w[..]
                && sp.starts_with("SPECIES_")
                && let Some(s) = sets.get(set)
            {
                d.learnsets.insert(sp.to_string(), s.clone());
            }
        }

        // Trainers.
        let parties = parse_parties(&opt("src/data/trainer_parties.h", &mut d));
        for (key, f) in designated(&opt("src/data/trainers.h", &mut d), "TRAINER_") {
            if let Some(p) = f
                .get("party")
                .and_then(|p| idents(p).find(|w| w.starts_with("sParty_")))
                && let Some(mons) = parties.get(p)
            {
                d.trainers.insert(key, mons.clone());
            }
        }

        // Wild encounters.
        let wild = opt("src/data/wild_encounters.json", &mut d);
        if !wild.is_empty() {
            let v: serde_json::Value =
                serde_json::from_str(&wild).context("src/data/wild_encounters.json")?;
            for g in v["wild_encounter_groups"].as_array().into_iter().flatten() {
                if g["for_maps"].as_bool() != Some(true) {
                    continue;
                }
                for e in g["encounters"].as_array().into_iter().flatten() {
                    let Some(map) = e["map"].as_str() else {
                        continue;
                    };
                    let mons = d.wild.entry(map.to_string()).or_default();
                    // Land only: the water, Rock Smash and fishing tables need field moves or
                    // rods, so they would put their mons on maps long before the story meets them.
                    for kind in ["land_mons"] {
                        for m in e[kind]["mons"].as_array().into_iter().flatten() {
                            if let (Some(s), Some(l)) =
                                (m["species"].as_str(), m["max_level"].as_u64())
                            {
                                let k = (s.to_string(), l.min(100) as u8);
                                if !mons.contains(&k) {
                                    mons.push(k);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Scripts.
        let mut labels: BTreeMap<String, Label> = BTreeMap::new();
        let maps_dir = root.join("data/maps");
        let groups = opt("data/maps/map_groups.json", &mut d);
        let mut map_order: Vec<(String, u16)> = Vec::new();
        if !groups.is_empty() {
            let v: serde_json::Value =
                serde_json::from_str(&groups).context("data/maps/map_groups.json")?;
            for (g, gname) in v["group_order"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let Some(gname) = gname.as_str() else {
                    continue;
                };
                for (n, m) in v[gname].as_array().into_iter().flatten().enumerate() {
                    if let Some(m) = m.as_str() {
                        map_order.push((m.to_string(), ((g as u16) << 8) | n as u16));
                    }
                }
            }
        }
        for (m, _) in &map_order {
            if let Ok(src) = std::fs::read_to_string(maps_dir.join(m).join("scripts.inc")) {
                scan_scripts(&src, m, &macros, &mut labels);
            }
        }
        for f in files(&root.join("data/scripts"), "inc") {
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("x");
            if let Ok(src) = std::fs::read_to_string(&f) {
                scan_scripts(&src, &format!("Common_{stem}"), &macros, &mut labels);
            }
        }
        let event_scripts = opt("data/event_scripts.s", &mut d);
        scan_scripts(&event_scripts, "Common_event_scripts", &macros, &mut labels);
        let stds = std_scripts(&event_scripts);
        for l in labels.values() {
            for (k, n) in &l.sites {
                *d.script_sites.entry(k.clone()).or_default() += n;
            }
        }
        // Label -> the labels it names.
        let edges: BTreeMap<&str, Vec<&str>> = labels
            .iter()
            .map(|(n, l)| {
                let v = l
                    .words
                    .iter()
                    .filter_map(|w| {
                        if labels.contains_key(w) {
                            Some(w.as_str())
                        } else {
                            stds.get(w).map(String::as_str)
                        }
                    })
                    .collect();
                (n.as_str(), v)
            })
            .collect();

        // Layouts and tilesets.
        let tilesets = tileset_attributes(root, &mut d.issues);
        let layouts = opt("data/layouts/layouts.json", &mut d);
        let layouts: serde_json::Value = if layouts.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&layouts).context("data/layouts/layouts.json")?
        };
        let layout_of = |id: &str| {
            layouts["layouts"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|l| l["id"].as_str() == Some(id))
        };

        // Maps.
        for (name, id) in map_order {
            let json = match read(root, &format!("data/maps/{name}/map.json")) {
                Ok(s) => s,
                Err(e) => {
                    d.issues.push(format!("decomp: {e:#}"));
                    continue;
                }
            };
            let j: serde_json::Value =
                serde_json::from_str(&json).with_context(|| format!("{name}/map.json"))?;
            let s = |k: &str| j[k].as_str().unwrap_or("").to_string();
            let mut m = MapInfo {
                name: name.clone(),
                constant: s("id"),
                id,
                map_type: s("map_type"),
                weather: s("weather"),
                requires_flash: j["requires_flash"].as_bool().unwrap_or(false),
                ..MapInfo::default()
            };
            m.dive = j["connections"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| matches!(c["direction"].as_str(), Some("dive") | Some("emerge")));
            let mut roots: Vec<&str> = labels
                .iter()
                .filter(|(_, l)| l.module == name)
                .map(|(n, _)| n.as_str())
                .collect();
            for k in ["object_events", "coord_events", "bg_events"] {
                for e in j[k].as_array().into_iter().flatten() {
                    if let Some(g) = e["graphics_id"].as_str() {
                        m.gfx.insert(g.to_string());
                    }
                    if let Some(sc) = e["script"].as_str()
                        && let Some((k, _)) = labels.get_key_value(sc)
                    {
                        roots.push(k.as_str());
                    }
                    if let Some(item) = e["item"].as_str() {
                        m.items.insert(item.to_string());
                    }
                }
            }
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let mut stack = roots;
            while let Some(l) = stack.pop() {
                if !seen.insert(l) {
                    continue;
                }
                stack.extend(edges.get(l).into_iter().flatten().copied());
            }
            for l in &seen {
                let lab = &labels[*l];
                for (k, n) in &lab.sites {
                    *m.script_sites.entry(k.clone()).or_default() += n;
                }
                for w in &lab.words {
                    if w.starts_with("ITEM_") && w != "ITEM_NONE" {
                        m.items.insert(w.clone());
                    }
                }
                m.trainers.extend(
                    lab.trainers
                        .iter()
                        .filter(|t| d.trainers.contains_key(*t))
                        .cloned(),
                );
            }
            if let Some(l) = layout_of(&s("layout")) {
                m.behaviors =
                    layout_behaviors(root, l, &tilesets, &behavior_names).unwrap_or_else(|e| {
                        d.issues.push(format!("decomp: {name}: {e:#}"));
                        BTreeMap::new()
                    });
            }
            d.maps.push(m);
        }
        let battle: Vec<BTreeSet<(Category, String)>> =
            d.maps.iter().map(|m| d.battle_uses(m)).collect();
        for (m, b) in d.maps.iter_mut().zip(battle) {
            m.battle = b;
        }
        Ok(d)
    }

    /// The move effects, abilities and hold effects of `m`'s trainers and wild mons.
    fn battle_uses(&self, m: &MapInfo) -> BTreeSet<(Category, String)> {
        let mut out = BTreeSet::new();
        let mut mon = |species: &str, level: u8, moves: &[String], item: Option<&String>| {
            let moves = if moves.is_empty() {
                self.learnsets
                    .get(species)
                    .map(|l| default_moves(l, level))
                    .unwrap_or_default()
            } else {
                moves.to_vec()
            };
            for mv in moves {
                if let Some(e) = self.moves.get(&mv) {
                    out.insert((Category::MoveEffect, e.clone()));
                }
            }
            for a in self.species_abilities.get(species).into_iter().flatten() {
                out.insert((Category::Ability, a.clone()));
            }
            if let Some(h) = item
                .and_then(|i| self.items.get(i))
                .and_then(|i| i.hold_effect.clone())
            {
                out.insert((Category::HoldEffect, h));
            }
        };
        for t in &m.trainers {
            for p in self.trainers.get(t).into_iter().flatten() {
                mon(&p.species, p.level, &p.moves, p.item.as_ref());
            }
        }
        for (s, l) in self.wild.get(&m.constant).into_iter().flatten() {
            mon(s, *l, &[], None);
        }
        out
    }

    /// The names of `cat`.
    pub fn names(&self, cat: Category) -> &[String] {
        self.universe.get(&cat).map_or(&[], Vec::as_slice)
    }

    pub fn map_by_id(&self, id: u16) -> Option<&MapInfo> {
        self.maps.iter().find(|m| m.id == id)
    }
}

/// `trainer_parties.h`: party symbol -> members.
fn parse_parties(src: &str) -> BTreeMap<String, Vec<PartyMon>> {
    let src = strip_c_comments(src);
    let mut out: BTreeMap<String, Vec<PartyMon>> = BTreeMap::new();
    let mut party: Option<String> = None;
    let mut mon: Option<PartyMon> = None;
    for l in src.lines() {
        let t = l.trim();
        if t.contains("sParty_") && t.contains('=') {
            party = idents(t)
                .find(|w| w.starts_with("sParty_"))
                .map(str::to_string);
            if let Some(p) = &party {
                out.entry(p.clone()).or_default();
            }
            continue;
        }
        let Some(p) = &party else { continue };
        if t == "{" {
            mon = Some(PartyMon::default());
            continue;
        }
        if (t == "}," || t == "}") && mon.is_some() {
            let m = mon.take().unwrap();
            if !m.species.is_empty() {
                out.get_mut(p).unwrap().push(m);
            }
            continue;
        }
        if let Some(m) = mon.as_mut()
            && let Some(rest) = t.strip_prefix('.')
            && let Some((f, v)) = rest.split_once('=')
        {
            let v = v.trim().trim_end_matches(',').trim();
            match f.trim() {
                "lvl" => m.level = v.parse().unwrap_or(0),
                "species" => m.species = v.to_string(),
                "heldItem" if v != "ITEM_NONE" => m.item = Some(v.to_string()),
                "moves" => {
                    m.moves = idents(v)
                        .filter(|w| w.starts_with("MOVE_") && *w != "MOVE_NONE")
                        .map(str::to_string)
                        .collect()
                }
                _ => {}
            }
        }
    }
    out
}

/// Tileset symbol (`gTileset_General`) -> its metatile attributes.
fn tileset_attributes(root: &Path, issues: &mut Vec<String>) -> BTreeMap<String, Vec<u16>> {
    let mut out = BTreeMap::new();
    let (Ok(headers), Ok(metatiles)) = (
        read(root, "src/data/tilesets/headers.h"),
        read(root, "src/data/tilesets/metatiles.h"),
    ) else {
        issues.push(
            "decomp: no src/data/tilesets/{headers,metatiles}.h: no behaviours per map".into(),
        );
        return out;
    };
    // gMetatileAttributes_X -> path.
    let mut paths = BTreeMap::new();
    for l in metatiles.lines() {
        if let Some(sym) = idents(l).find(|w| w.starts_with("gMetatileAttributes_"))
            && let Some(p) = l.split('"').nth(1)
        {
            paths.insert(sym.to_string(), p.to_string());
        }
    }
    let mut cur: Option<String> = None;
    for l in headers.lines() {
        if let Some(sym) = idents(l).find(|w| w.starts_with("gTileset_"))
            && l.contains("struct Tileset")
        {
            cur = Some(sym.to_string());
        }
        if let (Some(c), true) = (&cur, l.contains(".metatileAttributes"))
            && let Some(sym) = idents(l).find(|w| w.starts_with("gMetatileAttributes_"))
            && let Some(p) = paths.get(sym)
        {
            match std::fs::read(root.join(p)) {
                Ok(b) => {
                    let v = b
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    out.insert(c.clone(), v);
                }
                Err(e) => issues.push(format!("decomp: {p}: {e}")),
            }
        }
    }
    out
}

/// Gen3 `NUM_METATILES_IN_PRIMARY`.
const PRIMARY: usize = 512;

fn layout_behaviors(
    root: &Path,
    l: &serde_json::Value,
    tilesets: &BTreeMap<String, Vec<u16>>,
    names: &BTreeMap<i64, String>,
) -> Result<BTreeMap<String, usize>> {
    let path = l["blockdata_filepath"].as_str().context("no blockdata")?;
    let b = std::fs::read(root.join(path)).with_context(|| path.to_string())?;
    let ts = |k: &str| l[k].as_str().and_then(|t| tilesets.get(t));
    let (p, s) = (ts("primary_tileset"), ts("secondary_tileset"));
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for c in b.chunks_exact(2) {
        let id = usize::from(u16::from_le_bytes([c[0], c[1]]) & 0x3FF);
        let attr = if id < PRIMARY {
            p.and_then(|v| v.get(id))
        } else {
            s.and_then(|v| v.get(id - PRIMARY))
        };
        let Some(a) = attr else { continue };
        let raw = i64::from(a & 0xFF);
        let name = names
            .get(&raw)
            .cloned()
            .unwrap_or_else(|| format!("MB_0x{raw:02X}"));
        *out.entry(name).or_default() += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_parsers() {
        assert_eq!(c_int("(1 << 3)"), Some(8));
        assert_eq!(c_int("0x1F"), Some(31));
        assert_eq!(
            defines(
                "#define A_X 1 // c\n#define A_Y (A_X | 2)\n#define B_Z 3",
                "A_"
            ),
            [("A_X".to_string(), 1)]
        );
        assert_eq!(
            enum_members("enum {\n MB_A,\n MB_B = 5, // x\n MB_C,\n};", "MB_"),
            [("MB_A".into(), 0), ("MB_B".into(), 5), ("MB_C".into(), 6)]
        );
        let d = designated("    [ITEM_A] =\n    {\n        .x = 1,\n    },", "ITEM_");
        assert_eq!(d[0].1["x"], "1");
    }

    #[test]
    fn macros_expand_to_commands() {
        let m = parse_macros(
            ".macro loadword a\n.byte SCR_OP_LOADWORD\n.endm\n.macro callstd f\n.byte SCR_OP_CALLSTD\n.endm\n.macro msgbox t, ty\nloadword 0, \\t\ncallstd \\ty\n.endm\n",
        );
        let mut v = Vec::new();
        expand(&m, "msgbox", 0, &mut v);
        assert_eq!(v, ["loadword", "callstd"]);
    }

    #[test]
    fn default_moves_keep_the_last_four() {
        let l: Vec<(u8, String)> = (1..=6).map(|i| (i, format!("MOVE_{i}"))).collect();
        assert_eq!(
            default_moves(&l, 5),
            ["MOVE_2", "MOVE_3", "MOVE_4", "MOVE_5"]
        );
    }
}
