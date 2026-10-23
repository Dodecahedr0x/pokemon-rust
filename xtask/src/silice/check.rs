//! `cargo xtask silice check [--games DIR] [--extracted DIR] [--scripts DIR] [--allow RULE]...`:
//! the permanent validator of `games/silice/` (plan §4). Exit 1 on errors; warnings are listed.
//!
//! Inputs: the imported data (`text/`, `battle/`, `items.ron`, `ids/`), the authored maps
//! (`maps/*.ron`: object and hidden-item flags, coord and frame-script vars, music, weather,
//! warp and connection targets), and the scene scripts once they exist (`--scripts`, default
//! `crates/content/src/silice/scripts/`, empty until then).
//!
//! Name tables: the decomp names compiled into `extract::names` and `charmap::SOUNDS` always;
//! the extract (`content/extracted/`, or `PKR_EXTRACTED_DIR`) when it has a `manifest.ron`
//! (species, moves, items and heal locations of the ROM, and the FR font widths for the width
//! rules); every `#define` under `$PKR_DECOMP/include/constants/` when `PKR_DECOMP` is set (the
//! only source for `WEATHER_*`). A check whose source is missing is skipped with a note.
//!
//! Rules (`--allow RULE` or an entry of `check-exceptions.ron` turns a finding into a listed
//! exception):
//!
//! | Rule | Level | What |
//! |---|---|---|
//! | `id-alloc` | error | an ids file with a duplicate name or number, or a number outside its range; a number two song files share |
//! | `gen3-collision` | error | a Gen3 story flag or saved var the data uses whose number is inside the Silice block; a live Silice flag or var on the number of one the shared code uses (`extract::games::SHARED_GEN3`) |
//! | `id-removed` | error | a reference to a tombstone |
//! | `id-undefined` | error | a Silice name (`*_SIL_*`, a form) missing from its ids file |
//! | `id-unknown` | error | a Gen3 name (species, class, pic, AI) the decomp does not have |
//! | `sil-prefix` | error | an unknown constant of a kind Silice may invent, or a registry name, without `_SIL_` |
//! | `map-undefined` | error | a map neither in `ids/maps.ron` nor in `maps/` (constants, warp and connection targets, placement maps) |
//! | `placement-map` | error | a ball or hidden-item placement whose map exists but lacks its flag (or hides another item under it); a given item's flag once scene scripts name flags, in neither the map nor a script |
//! | `trainer-undefined` | error | a `TRAINER_SIL_*` no trainer defines |
//! | `trainer-duplicate` | error | a trainer id defined twice |
//! | `trainer-name` | error | a name over 10 bytes (`TRAINER_NAME_LENGTH`) |
//! | `trainer-class-name` | error | a class name over 12 bytes (`gTrainerClassNames[][13]`) |
//! | `trainer-items` | error | more than 4 items in the bag |
//! | `party-size` | error | a party of 0 or more than 6 |
//! | `party-moves` | error | a move list of 0 or more than 4 moves |
//! | `party-level` | error | a level outside 1..=100 |
//! | `encounter-slots` | error | a table without 12 (land), 5 (water, rock smash) or 10 (fishing) slots |
//! | `encounter-level` | error | a slot outside 1..=100 or with min over max |
//! | `fishing-unreachable` | warning | fishing slots of a rod no item table gives |
//! | `text-duplicate` | error | a text id defined twice |
//! | `text-missing` | error | a text id the scenes print but `text/fr.ron` lacks |
//! | `speakers-mismatch` | error | `text/speakers.ron` and `text/fr.ron` with different id sets |
//! | `dynamic-unresolved` | error | a placeholder reference the source does not enumerate |
//! | `text-charmap` | error | a character the FR charmap cannot encode |
//! | `text-width` | error | a line wider than 216 px (208 px for a defeat text); see [`line_widths`] |
//! | `text-lines` | error | a box (between `\p`) with more than 2 lines (a second `\n`) |
//! | `invented-move` | warning | a `MOVE_SIL_*` (D-00/D-02b say Gen3 moves only) |
//! | `item-incomplete` | warning | an `ITEM_SIL_*` without a name, pocket or base item |
//! | `item-name-length` | warning | an item name over 13 bytes (`ITEM_NAME_LENGTH` 14 with the terminator) |
//! | `item-name-clash` | warning | an item name equal to another item's |
//! | `exception-unused` | warning | an exception that matches nothing |

use super::import::text_pairs;
use super::maps::{self, MapRefs};
use super::names;
use super::read_ron;
use super::schema::*;
use anyhow::{Result, bail};
use extract::text::Lang;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct Options {
    pub games: PathBuf,
    pub extracted: PathBuf,
    pub decomp: Option<PathBuf>,
    pub scripts: PathBuf,
    pub allow: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub rule: &'static str,
    pub level: Level,
    pub subject: String,
    pub message: String,
}

/// The field message box width in pixels (`engine::text::MSG_WINDOW`, 27 tiles, text from x = 0).
pub const LINE_PX: u32 = 27 * 8;
/// The battle message box (26 tiles): trainer defeat texts are printed there.
pub const BATTLE_LINE_PX: u32 = 26 * 8;
/// `PLAYER_NAME_LENGTH` (`{PLAYER}`, `{RIVAL}`).
const PLAYER_LEN: u32 = 7;
/// The longest a `{STR_VAR_n}` buffer is filled with in Gen3 scripts: an item name
/// (`ITEM_NAME_LENGTH` 14 with the terminator). Pokémon (10) and move (12) names are shorter.
const STR_VAR_LEN: u32 = 13;

/// Everything a check reads.
pub struct Inputs {
    pub texts: Vec<(String, String)>,
    pub speakers: Option<SpeakersFile>,
    pub trainers: Option<TrainersFile>,
    pub encounters: Option<EncountersFile>,
    pub items: Option<ItemsFile>,
    pub ids: BTreeMap<IdKind, IdsFile>,
    /// Old per-kind song files (`ids/music.ron`, `ids/sounds.ron`), which must not exist.
    pub legacy_ids: BTreeMap<String, IdsFile>,
    pub refs: Option<RefsFile>,
    /// The authored maps under `maps/`.
    pub maps: Vec<MapRefs>,
    /// Scene scripts: file stem -> constants named.
    pub scripts: BTreeMap<String, Vec<String>>,
    pub exceptions: ExceptionsFile,
    /// The extract's FR font widths.
    pub widths: Option<Vec<u8>>,
    /// Gen3 names known from the extract and the decomp, on top of the compiled tables.
    pub extra_names: BTreeSet<String>,
    /// Gen3 item names (as [`name_key`]) from the extract.
    pub gen3_item_names: BTreeMap<String, String>,
    pub decomp: bool,
    pub notes: Vec<String>,
}

pub fn load(o: &Options) -> Result<Inputs> {
    let g = &o.games;
    let opt = |rel: &str| -> Option<PathBuf> {
        let p = g.join(rel);
        p.exists().then_some(p)
    };
    let mut notes = Vec::new();
    let texts = match opt("text/fr.ron") {
        Some(p) => text_pairs(&std::fs::read_to_string(&p)?)?,
        None => {
            notes.push("no text/fr.ron: text checks skipped".to_string());
            Vec::new()
        }
    };
    let speakers = opt("text/speakers.ron").map(|p| read_ron(&p)).transpose()?;
    let trainers = opt("battle/trainers.ron")
        .map(|p| read_ron(&p))
        .transpose()?;
    let encounters = opt("battle/encounters.ron")
        .map(|p| read_ron(&p))
        .transpose()?;
    let items = opt("items.ron").map(|p| read_ron(&p)).transpose()?;
    let refs = opt("ids/refs.ron").map(|p| read_ron(&p)).transpose()?;
    let mut ids = BTreeMap::new();
    for k in IdKind::ALL {
        if let Some(p) = opt(&format!("ids/{}.ron", k.file())) {
            ids.insert(k, read_ron::<IdsFile>(&p)?);
        }
    }
    let mut legacy_ids = BTreeMap::new();
    for f in ["music", "sounds"] {
        if let Some(p) = opt(&format!("ids/{f}.ron")) {
            legacy_ids.insert(f.to_string(), read_ron::<IdsFile>(&p)?);
        }
    }
    let maps = maps::scan_dir(&g.join("maps"))?;
    let scripts = maps::scan_scripts(&o.scripts)?;
    if scripts.is_empty() {
        notes.push(format!(
            "no scene scripts at {}: names checked from ids/refs.ron and maps/",
            o.scripts.display()
        ));
    }
    let exceptions = opt("check-exceptions.ron")
        .map(|p| read_ron(&p))
        .transpose()?
        .unwrap_or_default();

    let mut widths = None;
    let mut extra_names = BTreeSet::new();
    let mut gen3_item_names = BTreeMap::new();
    let x = &o.extracted;
    if x.join("manifest.ron").exists() {
        let f = x.join("font/latin_normal.ron");
        if f.exists() {
            widths = Some(read_ron::<extract::schema::FontFile>(&f)?.widths);
        } else {
            notes.push(format!("{} missing: text-width skipped", f.display()));
        }
        let sp = x.join("battle/species.ron");
        if sp.exists() {
            let s: extract::schema::SpeciesFile = read_ron(&sp)?;
            extra_names.extend(s.species.into_iter().map(|s| s.id));
        }
        let mv = x.join("battle/moves.ron");
        if mv.exists() {
            let m: extract::schema::MovesFile = read_ron(&mv)?;
            extra_names.extend(m.moves.into_iter().map(|m| m.id));
        }
        let it = x.join("tables/items.ron");
        if it.exists() {
            let f: extract::schema::ItemsFile = read_ron(&it)?;
            for i in f.items {
                extra_names.insert(i.id.clone());
                if i.id != "ITEM_NONE" && !i.name.starts_with('?') {
                    gen3_item_names.insert(name_key(&i.name), i.id);
                }
            }
        }
        let hl = x.join("battle/heal_locations.ron");
        if hl.exists() {
            let h: extract::schema::HealLocationsFile = read_ron(&hl)?;
            extra_names.extend(h.into_keys());
        }
    } else {
        notes.push(format!(
            "no extract at {} (no manifest.ron): text-width and item-name-clash skipped; names \
             checked against the compiled decomp tables only",
            x.display()
        ));
    }
    let decomp = match &o.decomp {
        Some(d) => {
            let dir = d.join("include/constants");
            match std::fs::read_dir(&dir) {
                Ok(rd) => {
                    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                    files.sort();
                    for f in files
                        .iter()
                        .filter(|f| f.extension().is_some_and(|e| e == "h"))
                    {
                        let text = std::fs::read_to_string(f)?;
                        for l in text.lines() {
                            let l = l.trim_start();
                            if let Some(r) = l.strip_prefix("#define ") {
                                if let Some(n) = r.split_whitespace().next() {
                                    extra_names.insert(n.to_string());
                                }
                            } else if let Some(n) = l.split([',', ' ', '=']).next()
                                && n.len() > 3
                                && n.chars().all(|c| {
                                    c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
                                })
                                && n.contains('_')
                            {
                                // enum members (`WEATHER_*` in some decomp versions).
                                extra_names.insert(n.to_string());
                            }
                        }
                    }
                    true
                }
                Err(_) => {
                    notes.push(format!(
                        "PKR_DECOMP: {} unreadable: decomp names skipped",
                        dir.display()
                    ));
                    false
                }
            }
        }
        None => {
            notes.push("PKR_DECOMP not set: WEATHER_* names not checked".to_string());
            false
        }
    };
    Ok(Inputs {
        texts,
        speakers,
        trainers,
        encounters,
        items,
        ids,
        legacy_ids,
        refs,
        maps,
        scripts,
        exceptions,
        widths,
        extra_names,
        gen3_item_names,
        decomp,
        notes,
    })
}

pub fn run(o: &Options) -> Result<()> {
    if !o.games.exists() {
        bail!("{} does not exist", o.games.display());
    }
    let inputs = load(o)?;
    let findings = check(&inputs);
    let (errors, warnings) = report(&inputs, &findings, &o.allow);
    if errors > 0 {
        bail!("silice check: {errors} error(s), {warnings} warning(s)");
    }
    println!("silice check: ok ({warnings} warning(s))");
    Ok(())
}

/// Simple glob: `*` matches any run of characters.
pub fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == s;
    }
    let mut rest = s;
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(p) else {
                return false;
            };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(p);
        } else {
            let Some(k) = rest.find(p) else { return false };
            rest = &rest[k + p.len()..];
        }
    }
    true
}

/// Prints the findings; returns the (errors, warnings) not excepted.
pub fn report(inputs: &Inputs, findings: &[Finding], allow: &[String]) -> (usize, usize) {
    for n in &inputs.notes {
        println!("note: {n}");
    }
    let ex = &inputs.exceptions.exceptions;
    let mut used = vec![false; ex.len()];
    let mut by_rule: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    let mut listed: Vec<(&Finding, Option<usize>)> = Vec::new();
    for f in findings {
        let hit = ex
            .iter()
            .position(|e| e.rule == f.rule && glob(&e.subject, &f.subject));
        if let Some(k) = hit {
            used[k] = true;
        }
        let excepted = hit.is_some() || allow.iter().any(|a| a == f.rule);
        let slot = if excepted {
            2
        } else if f.level == Level::Error {
            0
        } else {
            1
        };
        by_rule.entry(f.rule).or_default()[slot] += 1;
        listed.push((f, excepted.then_some(hit.unwrap_or(usize::MAX))));
    }
    let mut extra = Vec::new();
    for (k, e) in ex.iter().enumerate() {
        if !used[k] {
            extra.push(Finding {
                rule: "exception-unused",
                level: Level::Warning,
                subject: format!("{} {}", e.rule, e.subject),
                message: "matches no finding; remove it".to_string(),
            });
        }
    }
    for f in &extra {
        by_rule.entry(f.rule).or_default()[1] += 1;
        listed.push((f, None));
    }
    for (f, exc) in &listed {
        let tag = match (exc, f.level) {
            (Some(_), _) => "excepted",
            (None, Level::Error) => "error",
            (None, Level::Warning) => "warning",
        };
        println!("{tag}[{}] {}: {}", f.rule, f.subject, f.message);
    }
    println!("\n| rule | errors | warnings | excepted |\n|---|---|---|---|");
    let (mut e, mut w, mut x) = (0, 0, 0);
    for (rule, [a, b, c]) in &by_rule {
        println!("| {rule} | {a} | {b} | {c} |");
        e += a;
        w += b;
        x += c;
    }
    println!("| total | {e} | {w} | {x} |");
    for (k, ex) in ex.iter().enumerate() {
        if used[k] && ex.ruling_needed {
            println!("ruling needed [{}] {}: {}", ex.rule, ex.subject, ex.reason);
        }
    }
    (e, w)
}

fn sil_kind(name: &str) -> Option<IdKind> {
    if name.starts_with("MAP_") {
        return Some(IdKind::Maps);
    }
    if is_form(name) {
        return Some(IdKind::Species);
    }
    IdKind::ALL
        .into_iter()
        .find(|k| k.sil_prefixes().iter().any(|p| name.starts_with(p)))
}

fn is_form(name: &str) -> bool {
    name.starts_with("SPECIES_")
        && ["_MEGA", "_MEGA_X", "_MEGA_Y", "_PRIMAL"]
            .iter()
            .any(|s| name.ends_with(s))
}

/// Kinds Silice may add names to (with the SIL_ prefix).
const INVENTABLE: &[&str] = &[
    "FLAG_",
    "VAR_",
    "ITEM_",
    "MOVE_",
    "MUS_",
    "SE_",
    "WEATHER_",
    "HEAL_LOCATION_",
    "TRAINER_",
];

/// `songs.h` values outside the song table (`MUS_ROUTE118` 0x7FFF, `MUS_NONE` 0xFFFF).
const SPECIAL_SOUNDS: &[&str] = &["MUS_ROUTE118", "MUS_NONE"];

fn gen3_known(name: &str, i: &Inputs) -> Option<bool> {
    use extract::names as n;
    let table = |t: &n::Table| t.value(name).is_some();
    let known = i.extra_names.contains(name)
        || table(&n::SPECIES)
        || table(&n::MOVES)
        || table(&n::ITEMS)
        || table(&n::FLAGS)
        || table(&n::VARS)
        || table(&n::TRAINER_CLASSES)
        || table(&n::TRAINER_PICS)
        || table(&n::AI_FLAGS)
        || table(&n::HEAL_LOCATIONS)
        || table(&n::TRAINERS)
        || extract::text::charmap::SOUNDS
            .iter()
            .any(|&(_, s)| s == name)
        || SPECIAL_SOUNDS.contains(&name);
    if !known && name.starts_with("WEATHER_") && !i.decomp {
        return None;
    }
    Some(known)
}

/// The Gen3 number of a story flag (0x20..0x500: not temp, not trainer or system) or a saved
/// var (0x4020..0x4100: not temp, not special).
fn gen3_story_number(name: &str) -> Option<(IdKind, u32)> {
    if let Some(v) = extract::names::FLAGS.value(name)
        && (0x20..0x500).contains(&v)
        && !name.contains("_SIL_")
    {
        return Some((IdKind::Flags, v));
    }
    if let Some(v) = extract::names::VARS.value(name)
        && (0x4020..0x4100).contains(&v)
        && !name.contains("_SIL_")
    {
        return Some((IdKind::Vars, v));
    }
    None
}

/// Runs every rule.
pub fn check(i: &Inputs) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |rule: &'static str, level: Level, subject: &str, message: String| {
        out.push(Finding {
            rule,
            level,
            subject: subject.to_string(),
            message,
        })
    };
    use Level::{Error, Warning};

    // Registries.
    let mut live: BTreeMap<IdKind, BTreeSet<&str>> = BTreeMap::new();
    let mut dead: BTreeSet<&str> = BTreeSet::new();
    for (k, f) in &i.ids {
        for (subject, m) in super::ids::problems(*k, f) {
            push(
                "id-alloc",
                Error,
                &subject,
                format!("ids/{}.ron: {m}", k.file()),
            );
        }
        for e in &f.ids {
            if e.removed {
                dead.insert(&e.name);
                continue;
            }
            live.entry(*k).or_default().insert(&e.name);
            let p = k.sil_prefixes();
            let wrong_prefix = if !p.is_empty() {
                !p.iter().any(|p| e.name.starts_with(p))
            } else if *k == IdKind::Species {
                !is_form(&e.name)
            } else {
                !e.name.starts_with("MAP_")
            };
            if wrong_prefix {
                push(
                    "sil-prefix",
                    Error,
                    &e.name,
                    format!("in ids/{}.ron without its prefix", k.file()),
                );
            }
            if *k == IdKind::Species {
                let base = e.name.trim_end_matches("_X").trim_end_matches("_Y");
                let base = base.trim_end_matches("_MEGA").trim_end_matches("_PRIMAL");
                if gen3_known(base, i) != Some(true) {
                    push(
                        "id-unknown",
                        Error,
                        &e.name,
                        format!("the base species {base} is not Gen3"),
                    );
                }
            }
            if *k == IdKind::Moves {
                push(
                    "invented-move",
                    Warning,
                    &e.name,
                    "D-00/D-02b say Gen3 moves only".to_string(),
                );
            }
        }
    }
    // A moved id (tombstoned, then appended again) is live, not removed.
    dead.retain(|n| !live.values().any(|s| s.contains(n)));
    // Music and sounds share Gen3's song numbers: one file. Old split files are errors, and any
    // number two of the song files share is reported.
    {
        let mut song_files: Vec<(String, &IdsFile)> = i
            .legacy_ids
            .iter()
            .map(|(n, f)| (format!("ids/{n}.ron"), f))
            .collect();
        for (n, _) in &song_files {
            push(
                "id-alloc",
                Error,
                n,
                "music and sounds share one number space: use ids/songs.ron".to_string(),
            );
        }
        if let Some(f) = i.ids.get(&IdKind::Songs) {
            song_files.push(("ids/songs.ron".to_string(), f));
        }
        let mut by_num: BTreeMap<u32, (&str, &str)> = BTreeMap::new();
        for (file, f) in &song_files {
            for e in &f.ids {
                if let Some((of, on)) = by_num.get(&e.num)
                    && of != file
                {
                    push(
                        "id-alloc",
                        Error,
                        &e.name,
                        format!("number {} in {file} is also {on} in {of}", e.num),
                    );
                }
                by_num
                    .entry(e.num)
                    .or_insert((file.as_str(), e.name.as_str()));
            }
        }
    }
    let maps_by_constant: BTreeMap<&str, &str> = i
        .ids
        .get(&IdKind::Maps)
        .map(|f| {
            f.ids
                .iter()
                .filter(|e| !e.removed)
                .filter_map(|e| Some((e.name.as_str(), e.map.as_deref()?)))
                .collect()
        })
        .unwrap_or_default();
    let map_files: BTreeMap<&str, &MapRefs> = i.maps.iter().map(|m| (m.name.as_str(), m)).collect();
    let map_names: BTreeSet<&str> = maps_by_constant
        .values()
        .copied()
        .chain(map_files.keys().copied())
        .collect();
    let map_constants: BTreeSet<String> = maps_by_constant.keys().map(|s| s.to_string()).collect();
    let map_of = |constant: &str| -> String {
        maps_by_constant
            .get(constant)
            .map(|s| s.to_string())
            .unwrap_or_else(|| names::map_name(constant, &map_constants))
    };
    let trainer_ids: BTreeSet<&str> = i
        .trainers
        .iter()
        .flat_map(|t| t.trainers.iter().map(|t| t.id.as_str()))
        .collect();

    // Every name: (name, where).
    let mut names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut add = |n: &str, w: &str| {
        names
            .entry(n.to_string())
            .or_default()
            .insert(w.to_string());
    };
    if let Some(r) = &i.refs {
        for (n, ch) in &r.constants {
            for c in ch {
                add(n, c);
            }
        }
    }
    for m in &i.maps {
        let w = format!("maps/{}.ron", m.name);
        for (field, v) in &m.fields {
            match field.as_str() {
                "flag" | "var" | "music" | "weather" => add(v, &w),
                _ => {}
            }
        }
        for (item, _) in &m.hidden {
            add(item, &w);
        }
        for (field, target) in &m.fields {
            if (field == "dest_map" || field == "map")
                && target != "MAP_DYNAMIC"
                && !map_names.contains(target.as_str())
            {
                push(
                    "map-undefined",
                    Error,
                    target,
                    format!("the {field} of {w}: no map file or ids/maps.ron entry"),
                );
            }
        }
    }
    for (stem, words) in &i.scripts {
        for n in words {
            add(n, &format!("scripts/{stem}.rs"));
        }
    }
    if let Some(t) = &i.trainers {
        for tr in &t.trainers {
            let w = format!("trainers.ron {}", tr.id);
            add(&tr.class, &w);
            add(&tr.pic, &w);
            for a in &tr.ai {
                add(a, &w);
            }
            for m in &tr.party {
                add(&m.species, &w);
                if let Some(it) = &m.item {
                    add(it, &w);
                }
                for mv in m.moves.iter().flatten() {
                    add(mv, &w);
                }
            }
            for it in &tr.items {
                add(it, &w);
            }
        }
    }
    if let Some(e) = &i.encounters {
        for (map, hs) in &e.maps {
            let w = format!("encounters.ron {map}");
            for h in hs {
                for t in [&h.land, &h.water, &h.rock_smash, &h.fishing]
                    .into_iter()
                    .flatten()
                {
                    for m in &t.mons {
                        add(&m.species, &w);
                    }
                }
            }
        }
    }
    if let Some(it) = &i.items {
        for d in &it.items {
            if let Some(c) = &d.clone_of {
                add(c, &format!("items.ron {}", d.id));
            }
        }
        for p in &it.placements {
            let w = format!("items.ron placement in {}", p.chapter);
            if let Some(f) = &p.flag {
                add(f, &w);
            }
            if p.map.starts_with("MAP_") {
                add(&p.map, &w);
            } else {
                push(
                    "map-undefined",
                    Error,
                    &p.map,
                    format!("{w}: the map cell is not a MAP_ constant ({})", p.name),
                );
            }
            if let Some(x) = &p.item {
                add(x, &w);
            }
        }
        for m in &it.marts {
            let w = format!("items.ron mart {}", m.label);
            if let Some(map) = &m.map {
                add(map, &w);
            }
            for s in &m.stock {
                add(&s.item, &w);
            }
        }
    }
    // Gen3 story flags and saved vars inside the Silice block.
    for kind in [IdKind::Flags, IdKind::Vars] {
        let nums: Vec<u32> = i
            .ids
            .get(&kind)
            .map(|f| f.ids.iter().map(|e| e.num).collect())
            .unwrap_or_default();
        let (Some(lo), Some(hi)) = (nums.iter().min(), nums.iter().max()) else {
            continue;
        };
        for (n, wh) in &names {
            if let Some((k, v)) = gen3_story_number(n)
                && k == kind
                && (*lo..=*hi).contains(&v)
            {
                push(
                    "gen3-collision",
                    Error,
                    n,
                    format!(
                        "Gen3 number {v:#x} is inside the Silice block {lo:#x}..={hi:#x} of ids/{}.ron; used in {}",
                        kind.file(),
                        wh.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        }
        // The Gen3 flags and vars the shared code uses, whatever the data says: no live Silice
        // id may take their numbers (`IdKind::reserved`).
        let Some(f) = i.ids.get(&kind) else { continue };
        for (k, n, v) in extract::games::shared_gen3() {
            if k != kind {
                continue;
            }
            for e in f.ids.iter().filter(|e| !e.removed && e.num == v) {
                push(
                    "gen3-collision",
                    Error,
                    &e.name,
                    format!(
                        "number {v:#x} of ids/{}.ron is {n}, which the shared code uses \
                         (extract::games::SHARED_GEN3): `cargo xtask silice move {}`",
                        kind.file(),
                        e.name
                    ),
                );
            }
        }
    }
    for (n, wh) in &names {
        let w = wh.iter().cloned().collect::<Vec<_>>().join(", ");
        if dead.contains(n.as_str()) {
            push("id-removed", Error, n, format!("a tombstone, used in {w}"));
            continue;
        }
        if let Some(k) = sil_kind(n) {
            let ok = live.get(&k).is_some_and(|s| s.contains(n.as_str()));
            match k {
                IdKind::Maps
                    if !ok && !map_names.contains(names::map_name(n, &map_constants).as_str()) =>
                {
                    push(
                        "map-undefined",
                        Error,
                        n,
                        format!("no map defines it; used in {w}"),
                    );
                }
                IdKind::Maps => {}
                IdKind::Trainers => {
                    if !trainer_ids.contains(n.as_str()) {
                        push(
                            "trainer-undefined",
                            Error,
                            n,
                            format!("no trainer defines it; used in {w}"),
                        );
                    } else if !ok {
                        push(
                            "id-undefined",
                            Error,
                            n,
                            format!("missing from ids/trainers.ron; used in {w}"),
                        );
                    }
                }
                _ if !ok => push(
                    "id-undefined",
                    Error,
                    n,
                    format!("missing from ids/{}.ron; used in {w}", k.file()),
                ),
                _ => {}
            }
            continue;
        }
        match gen3_known(n, i) {
            Some(true) | None => {}
            Some(false) => {
                if INVENTABLE.iter().any(|p| n.starts_with(p))
                    && !n.starts_with("TRAINER_CLASS_")
                    && !n.starts_with("TRAINER_PIC_")
                {
                    push(
                        "sil-prefix",
                        Error,
                        n,
                        format!(
                            "not a Gen3 name; a Silice name takes the SIL_ prefix; used in {w}"
                        ),
                    );
                } else {
                    push(
                        "id-unknown",
                        Error,
                        n,
                        format!("not a Gen3 name; used in {w}"),
                    );
                }
            }
        }
    }

    // Item placements against the authored maps (the maps are authoritative for positions; the
    // flag must be there, and a hidden item with that flag must be the same item). Balls and
    // hidden items are map objects; a given item's flag is set by a scene script, so it is looked
    // for in the maps and the scripts, once scripts name flags.
    let script_names: BTreeSet<&str> = i.scripts.values().flatten().map(String::as_str).collect();
    // A placeholder script naming no flag (SIL1 Wave 0) does not count as the scene scripts.
    let scripts_name_flags = script_names.iter().any(|n| n.starts_with("FLAG_"));
    if let Some(it) = &i.items {
        for p in &it.placements {
            let Some(flag) = &p.flag else { continue };
            if !p.map.starts_with("MAP_") {
                continue;
            }
            let name = map_of(&p.map);
            let Some(m) = map_files.get(name.as_str()) else {
                continue;
            };
            let object = matches!(p.method, PlacementMethod::Ball | PlacementMethod::Hidden);
            if !object && (!scripts_name_flags || script_names.contains(flag.as_str())) {
                continue;
            }
            let hidden = m.hidden.iter().find(|(_, f)| f == flag);
            if !m.values("flag").any(|f| f == flag) && !script_names.contains(flag.as_str()) {
                push(
                    "placement-map",
                    Error,
                    flag,
                    format!(
                        "{} ({}) is placed in {} ({}) but maps/{name}.ron (and no scene \
                         script) has no such flag",
                        p.name,
                        p.item.as_deref().unwrap_or("no item"),
                        p.map,
                        p.chapter
                    ),
                );
            } else if let (Some((hi, _)), Some(item)) = (hidden, &p.item)
                && hi != item
            {
                push(
                    "placement-map",
                    Error,
                    flag,
                    format!(
                        "maps/{name}.ron hides {hi} under this flag, the placement says {item}"
                    ),
                );
            }
        }
    }

    // Texts.
    let mut seen = BTreeSet::new();
    let defined: BTreeSet<&str> = i.texts.iter().map(|(k, _)| k.as_str()).collect();
    for (k, _) in &i.texts {
        if !seen.insert(k.as_str()) {
            push(
                "text-duplicate",
                Error,
                k,
                "defined more than once in text/fr.ron".to_string(),
            );
        }
    }
    if let Some(sp) = &i.speakers {
        let spk: BTreeSet<&str> = sp.texts.iter().map(|t| t.id.as_str()).collect();
        for k in defined.difference(&spk) {
            push(
                "speakers-mismatch",
                Error,
                k,
                "in text/fr.ron but not in text/speakers.ron".to_string(),
            );
        }
        for k in spk.difference(&defined) {
            push(
                "speakers-mismatch",
                Error,
                k,
                "in text/speakers.ron but not in text/fr.ron".to_string(),
            );
        }
    }
    if let Some(r) = &i.refs {
        for (t, ch) in &r.texts {
            if !defined.contains(t.as_str()) {
                push(
                    "text-missing",
                    Error,
                    t,
                    format!("printed in {} but not defined", ch.join(", ")),
                );
            }
        }
        for d in &r.dynamic {
            if d.expanded.is_empty() {
                push(
                    "dynamic-unresolved",
                    Error,
                    &d.pattern,
                    format!(
                        "a placeholder the source does not enumerate ({})",
                        d.chapters.join(", ")
                    ),
                );
            } else if d.kind == RefKind::Text {
                for e in d.expanded.iter().filter(|e| !defined.contains(e.as_str())) {
                    push(
                        "text-missing",
                        Error,
                        e,
                        format!("from `{}` but not defined", d.pattern),
                    );
                }
            }
        }
    }
    let mut glyphs: BTreeMap<char, usize> = BTreeMap::new();
    let metrics = i.widths.as_ref().map(|w| Metrics::new(w));
    for (k, v) in &i.texts {
        let bad = bad_chars(v);
        if !bad.is_empty() {
            for c in &bad {
                *glyphs.entry(*c).or_default() += 1;
            }
            push(
                "text-charmap",
                Error,
                k,
                format!(
                    "not in the FR charmap: {}",
                    bad.iter()
                        .map(|c| format!("{c:?}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            );
        }
        // Measured even with unencodable characters: each counts as a widest capital.
        let Ok(bytes) = extract::text::encode_lang(&substitute(v, &bad), Lang::Fr) else {
            continue;
        };
        for (b, n) in box_lines(&bytes).into_iter().enumerate() {
            if n > 2 {
                push(
                    "text-lines",
                    Error,
                    k,
                    format!("box {} has {n} lines (max 2; use \\l to scroll)", b + 1),
                );
            }
        }
        if let Some(m) = &metrics {
            let max = if is_defeat_text(k) {
                BATTLE_LINE_PX
            } else {
                LINE_PX
            };
            for (n, px) in line_widths(&bytes, m).into_iter().enumerate() {
                if px > max {
                    push(
                        "text-width",
                        Error,
                        k,
                        format!("line {} is {px} px (max {max})", n + 1),
                    );
                }
            }
        }
    }
    if !glyphs.is_empty() {
        let list: Vec<String> = glyphs.iter().map(|(c, n)| format!("{c:?} x{n}")).collect();
        println!(
            "glyphs missing from the FR charmap (texts using them): {}",
            list.join(", ")
        );
    }

    // Trainers.
    if let Some(t) = &i.trainers {
        let mut ids = BTreeSet::new();
        for tr in &t.trainers {
            let s = tr.id.as_str();
            if !ids.insert(s) {
                push("trainer-duplicate", Error, s, "defined twice".to_string());
            }
            for (rule, field, text, max) in [
                ("trainer-name", "name", &tr.name, 10),
                ("trainer-class-name", "class name", &tr.class_name, 12),
            ] {
                match extract::text::encode_lang(text, Lang::Fr) {
                    Ok(b) if b.len() - 1 > max => push(
                        rule,
                        Error,
                        s,
                        format!("{field} {text:?} is {} bytes (max {max})", b.len() - 1),
                    ),
                    Ok(_) => {}
                    Err(_) => push(
                        "text-charmap",
                        Error,
                        s,
                        format!("{field} {text:?} is not in the FR charmap"),
                    ),
                }
            }
            if tr.party.is_empty() || tr.party.len() > 6 {
                push(
                    "party-size",
                    Error,
                    s,
                    format!("{} mons (1 to 6)", tr.party.len()),
                );
            }
            if tr.items.len() > 4 {
                push(
                    "trainer-items",
                    Error,
                    s,
                    format!("{} items (max 4)", tr.items.len()),
                );
            }
            for (n, m) in tr.party.iter().enumerate() {
                if let Some(mv) = &m.moves
                    && (mv.is_empty() || mv.len() > 4)
                {
                    push(
                        "party-moves",
                        Error,
                        s,
                        format!("mon {}: {} moves (1 to 4)", n + 1, mv.len()),
                    );
                }
                if !(1..=100).contains(&m.level) {
                    push(
                        "party-level",
                        Error,
                        s,
                        format!("mon {}: level {}", n + 1, m.level),
                    );
                }
            }
        }
    }

    // Encounters.
    if let Some(e) = &i.encounters {
        let rods: BTreeSet<&str> = i
            .items
            .iter()
            .flat_map(|it| {
                it.placements
                    .iter()
                    .filter_map(|p| p.item.as_deref())
                    .chain(
                        it.marts
                            .iter()
                            .flat_map(|m| m.stock.iter().map(|x| x.item.as_str())),
                    )
            })
            .collect();
        for (map, hs) in &e.maps {
            if !map_names.contains(map.as_str()) {
                push(
                    "map-undefined",
                    Error,
                    map,
                    "encounters for a map no ids/maps.ron entry or maps/ file names".to_string(),
                );
            }
            for (hn, h) in hs.iter().enumerate() {
                for (kind, t, n) in [
                    ("land", &h.land, 12),
                    ("water", &h.water, 5),
                    ("rock_smash", &h.rock_smash, 5),
                    ("fishing", &h.fishing, 10),
                ] {
                    let Some(t) = t else { continue };
                    if t.mons.len() != n {
                        push(
                            "encounter-slots",
                            Error,
                            map,
                            format!("header {hn} {kind}: {} slots (Gen3 has {n})", t.mons.len()),
                        );
                    }
                    for (k, m) in t.mons.iter().enumerate() {
                        if m.min_level == 0 || m.max_level > 100 || m.min_level > m.max_level {
                            push(
                                "encounter-level",
                                Error,
                                map,
                                format!(
                                    "header {hn} {kind} slot {k}: levels {}-{}",
                                    m.min_level, m.max_level
                                ),
                            );
                        }
                    }
                }
                if let Some(f) = &h.fishing {
                    for (rod, item, slots) in [
                        ("old rod", "ITEM_OLD_ROD", 0..2),
                        ("good rod", "ITEM_GOOD_ROD", 2..5),
                        ("super rod", "ITEM_SUPER_ROD", 5..10),
                    ] {
                        if slots.start < f.mons.len() && !rods.contains(item) {
                            push(
                                "fishing-unreachable",
                                Warning,
                                map,
                                format!(
                                    "{rod} slots {}-{}: no item table gives {item}",
                                    slots.start,
                                    slots.end - 1
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    // Items.
    if let Some(it) = &i.items {
        let mut by_name: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for d in &it.items {
            let mut missing = Vec::new();
            if d.name.is_none() {
                missing.push("name");
            }
            if d.pocket.is_none() {
                missing.push("pocket");
            }
            if d.clone_of.is_none() {
                missing.push("base item");
            }
            if !missing.is_empty() {
                push(
                    "item-incomplete",
                    Warning,
                    &d.id,
                    format!("no {}", missing.join(", no ")),
                );
            }
            if let Some(n) = &d.name {
                match extract::text::encode_lang(n, Lang::Fr) {
                    Ok(b) if b.len() - 1 > ITEM_NAME_MAX => push(
                        "item-name-length",
                        Warning,
                        &d.id,
                        format!("{n:?} is {} bytes (max {ITEM_NAME_MAX})", b.len() - 1),
                    ),
                    Ok(_) => {}
                    Err(_) => push(
                        "text-charmap",
                        Error,
                        &d.id,
                        format!("name {n:?} is not in the FR charmap"),
                    ),
                }
                let key = name_key(n);
                by_name.entry(key.clone()).or_default().push(&d.id);
                if let Some(g) = i.gen3_item_names.get(&key) {
                    push(
                        "item-name-clash",
                        Warning,
                        &d.id,
                        format!("{n:?} is also the name of {g}"),
                    );
                }
            }
        }
        for (n, v) in by_name.iter().filter(|(_, v)| v.len() > 1) {
            push(
                "item-name-clash",
                Warning,
                v[1],
                format!("{n:?} also names {}", v[0]),
            );
        }
        for d in it.items.iter().filter(|d| {
            !live
                .get(&IdKind::Items)
                .is_some_and(|s| s.contains(d.id.as_str()))
        }) {
            push(
                "id-undefined",
                Error,
                &d.id,
                "defined in items.ron but missing from ids/items.ron".to_string(),
            );
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `ITEM_NAME_LENGTH` (14) less the terminator.
const ITEM_NAME_MAX: usize = 13;

/// A trainer defeat text, shown in the battle message box: the id's last words say so
/// (`Gym_Sc10_Defeat`, `Casino_Gaston_Defaite`).
pub fn is_defeat_text(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("_defeat") || l.contains("_defaite")
}

/// An item name for comparisons: uppercase letters and digits only (the ROM squeezes
/// `POUDRE CLAIRE` into `POUDRECLAIRE`).
fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

/// Characters of `text` the FR charmap cannot encode (escapes and `{...}` codes skipped).
pub fn bad_chars(text: &str) -> Vec<char> {
    if extract::text::encode_lang(text, Lang::Fr).is_ok() {
        return Vec::new();
    }
    let mut out = BTreeSet::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '{' => {
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                }
            }
            c => {
                if extract::text::encode_lang(&c.to_string(), Lang::Fr).is_err() {
                    out.insert(c);
                }
            }
        }
    }
    if out.is_empty() {
        // An unknown `{CODE}` or escape.
        out.insert('{');
    }
    out.into_iter().collect()
}

/// `text` with each character of `bad` replaced by `W`, a nominal glyph as wide as the widest
/// capitals, so a line with a missing glyph is still measured.
fn substitute(text: &str, bad: &[char]) -> String {
    if bad.is_empty() {
        return text.to_string();
    }
    text.chars()
        .map(|c| if bad.contains(&c) { 'W' } else { c })
        .collect()
}

/// Glyph widths plus the worst-case widths of the placeholders.
pub struct Metrics<'a> {
    widths: &'a [u8],
    /// The widest capital A–Z.
    widest: u32,
}

impl<'a> Metrics<'a> {
    pub fn new(widths: &'a [u8]) -> Self {
        let widest = ('A'..='Z')
            .filter_map(|c| extract::text::encode_lang(&c.to_string(), Lang::Fr).ok())
            .map(|b| widths.get(b[0] as usize).copied().unwrap_or(0) as u32)
            .max()
            .unwrap_or(0);
        Metrics { widths, widest }
    }

    fn glyph(&self, g: usize) -> u32 {
        self.widths.get(g).copied().unwrap_or(0) as u32
    }

    /// `{PLAYER}` and `{RIVAL}`: 7 widest capitals; `{STR_VAR_n}`: 13; others nothing.
    fn placeholder(&self, code: u8) -> u32 {
        match code {
            0x01 | 0x06 => PLAYER_LEN * self.widest,
            0x02..=0x04 => STR_VAR_LEN * self.widest,
            _ => 0,
        }
    }
}

/// Pixel width of each line of encoded text (`\n`, `\l` and `\p` end a line), placeholders at
/// their worst case ([`Metrics::placeholder`]).
pub fn line_widths(b: &[u8], m: &Metrics) -> Vec<u32> {
    const EXT_ARGS: [usize; 0x19] = [
        0, 1, 1, 1, 3, 1, 1, 0, 1, 0, 0, 2, 1, 1, 1, 0, 2, 1, 1, 1, 1, 0, 0, 0, 0,
    ];
    let mut out = Vec::new();
    let mut cur = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            0xFF => break,
            0xFA | 0xFB | 0xFE => {
                out.push(cur);
                cur = 0;
                i += 1;
            }
            0xFD => {
                cur += m.placeholder(*b.get(i + 1).unwrap_or(&0));
                i += 2;
            }
            0xFC => {
                i += 2 + b
                    .get(i + 1)
                    .and_then(|&c| EXT_ARGS.get(c as usize))
                    .copied()
                    .unwrap_or(0)
            }
            0xF7 | 0xF8 => i += 2,
            0xF9 => {
                cur += m.glyph(0x100 | *b.get(i + 1).unwrap_or(&0) as usize);
                i += 2;
            }
            c => {
                cur += m.glyph(c as usize);
                i += 1;
            }
        }
    }
    out.push(cur);
    out
}

/// Lines per box: a box ends at `\p` (0xFB); `\n` (0xFE) starts the second line; `\l` (0xFA)
/// scrolls, so it keeps the line count at 2.
pub fn box_lines(b: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut lines = 1;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            0xFF => break,
            0xFB => {
                out.push(lines);
                lines = 1;
            }
            0xFE => lines += 1,
            0xFA => lines = lines.max(2),
            // Two-byte codes whose argument could look like a control byte.
            0xFD | 0xF7 | 0xF8 | 0xF9 => i += 1,
            _ => {}
        }
        i += 1;
    }
    out.push(lines);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob("*", "x"));
        assert!(glob("A_*", "A_B"));
        assert!(glob("*_B", "A_B"));
        assert!(glob("A*C*E", "ABCDE"));
        assert!(!glob("A*C", "AB"));
        assert!(glob("X", "X") && !glob("X", "Y"));
    }

    #[test]
    fn widths() {
        let w = vec![5u8; 512];
        let m = Metrics::new(&w);
        let b = extract::text::encode_lang("AB\\nC{PLAYER}\\pD{STR_VAR_1}", Lang::Fr).unwrap();
        assert_eq!(line_widths(&b, &m), vec![10, 5 + 35, 5 + 65]);
    }

    #[test]
    fn lines_per_box() {
        let enc = |s: &str| extract::text::encode_lang(s, Lang::Fr).unwrap();
        assert_eq!(box_lines(&enc("a\\nb\\lc\\ld\\pe")), vec![2, 1]);
        assert_eq!(box_lines(&enc("a\\nb\\nc")), vec![3]);
        assert_eq!(box_lines(&enc("a\\nb\\lc\\nd")), vec![3]);
    }

    #[test]
    fn charmap() {
        assert!(bad_chars("Plain text\\n{PLAYER}").is_empty());
        assert_eq!(
            bad_chars("a\u{2014}b\u{2665}"),
            vec!['\u{2014}', '\u{2665}']
        );
        assert_eq!(substitute("a\u{2014}b", &['\u{2014}']), "aWb");
    }

    #[test]
    fn story_numbers() {
        assert_eq!(
            gen3_story_number("FLAG_RECEIVED_RUNNING_SHOES"),
            Some((IdKind::Flags, 0x112))
        );
        assert_eq!(gen3_story_number("FLAG_SYS_POKEDEX_GET"), None);
        assert_eq!(gen3_story_number("VAR_RESULT"), None);
        assert_eq!(gen3_story_number("VAR_TEMP_1"), None);
    }
}
