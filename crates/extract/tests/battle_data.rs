//! Battle data and strings (`battle_data.rs`) against the decomp: the US ROM must give exactly
//! the decomp's values, the French ROM the same numbers with French names and strings.
use extract::battle_data::{
    self, BattleData, CODE_STRINGS, FISHING_ROD_SLOTS, MENU_STRINGS, STAT_NAME_KEYS,
    TRAINER_MONEY_DEFAULT_CLASS,
};
use extract::export::{self, ExportOptions, SLICE_MAPS};
use extract::schema::{
    BattleStringsFile, EncounterHeader, EncountersFile, EvolutionsFile, HealLocationsFile,
    LandEncounters, Manifest, MovesFile, SpeciesFile, TrainerMoneyFile, TrainersFile, TypesFile,
};
use extract::{Rom, fixtures, names, text};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

/// Every map, in `gMapGroups` order: encounters are read for all of them.
fn all_maps() -> Vec<&'static str> {
    ExportOptions::all(false).maps
}

fn us() -> &'static (Rom, BattleData) {
    static D: OnceLock<(Rom, BattleData)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::us();
        let data = battle_data::extract(&rom, &all_maps()).unwrap();
        (rom, data)
    })
}

fn fr() -> &'static (Rom, BattleData) {
    static D: OnceLock<(Rom, BattleData)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::fr();
        let data = battle_data::extract(&rom, &all_maps()).unwrap();
        (rom, data)
    })
}

fn src(rel: &str) -> String {
    let p = fixtures::decomp().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `NAME[] = _("…");` string definitions of a C file, in source order.
fn c_strings(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let Some((head, rest)) = line.split_once("[] = _(\"") else {
            continue;
        };
        let name = head.split_whitespace().last().unwrap().to_string();
        let body = rest.rsplit_once("\");").unwrap().0;
        out.push((name, body.to_string()));
    }
    out
}

/// `Label::` + `.string "…"` lines of an assembly text file, the `$` terminator dropped.
fn inc_strings(src: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in src.lines() {
        if let Some(label) = line.strip_suffix("::") {
            out.push((label.to_string(), String::new()));
        } else if let Some(s) = line.trim().strip_prefix(".string \"") {
            let s = s.strip_suffix('"').unwrap().trim_end_matches('$');
            out.last_mut().unwrap().1.push_str(s);
        }
    }
    out
}

/// `[KEY] = value,` lines of the array whose definition line contains `array`.
fn designated(src: &str, array: &str) -> Vec<(String, String)> {
    let start = src.find(array).unwrap_or_else(|| panic!("{array}"));
    let body = &src[start..];
    let body = &body[..body.find("\n};").unwrap()];
    body.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix('[')?;
            let (key, value) = rest.split_once(']')?;
            let value = value.trim().strip_prefix('=')?.trim().trim_end_matches(',');
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// The `{ … }` initializer following `[key] =` (or `key[] = `).
fn block<'a>(src: &'a str, key: &str) -> &'a str {
    let at = src.find(key).unwrap_or_else(|| panic!("{key}"));
    let open = at + src[at..].find('{').unwrap();
    let mut depth = 0;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open + 1..open + i];
                }
            }
            _ => {}
        }
    }
    panic!("unclosed {key}")
}

/// `.name = value` fields of an initializer (one per line).
fn fields(block: &str) -> HashMap<String, String> {
    block
        .lines()
        .filter_map(|l| {
            let (n, v) = l.trim().strip_prefix('.')?.split_once('=')?;
            // Macro bodies end their lines with `\`.
            let v = v.trim().trim_end_matches('\\').trim().trim_end_matches(',');
            Some((n.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// `{ A, B }` -> `["A", "B"]`.
fn list(v: &str) -> Vec<String> {
    v.trim_matches(|c| c == '{' || c == '}' || c == ' ')
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `A | B` -> `["A", "B"]`, `0` -> `[]`.
fn flags(v: &str) -> Vec<String> {
    v.split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| s != "0")
        .collect()
}

/// Both strings encode to the same bytes (the decomp spells `'` where we decode `’`, …).
fn same_text(ours: &str, decomp: &str, what: &str) {
    assert_eq!(
        text::encode(ours).unwrap(),
        text::encode(decomp).unwrap(),
        "{what}: {ours:?} vs decomp {decomp:?}"
    );
}

/// `_("…")` -> `…`.
fn c_text(v: &str) -> &str {
    v.strip_prefix("_(\"").unwrap().strip_suffix("\")").unwrap()
}

fn num(v: &str) -> u32 {
    match v {
        "TRUE" => 1,
        "FALSE" => 0,
        "STANDARD_FRIENDSHIP" => 70,
        _ => v.parse().unwrap_or_else(|_| panic!("{v}")),
    }
}

/// The `[KEY] =` entries of a designated initializer whose entries span several lines, with
/// what follows the `=` on the key's line.
fn entries<'a>(src: &'a str, prefix: &str) -> Vec<(&'a str, &'a str)> {
    src.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("    [")?;
            let (key, value) = rest.split_once("] =")?;
            key.starts_with(prefix).then_some((key, value.trim()))
        })
        .collect()
}

/// `genderRatio` values: `PERCENT_FEMALE(p)` = `min(254, p * 255 / 100)` in integer math (the
/// percentages have at most one decimal).
fn gender_ratio(v: &str) -> u8 {
    match v {
        "MON_MALE" => 0,
        "MON_FEMALE" => 254,
        "MON_GENDERLESS" => 255,
        _ => {
            let p = v
                .strip_prefix("PERCENT_FEMALE(")
                .unwrap()
                .trim_end_matches(')');
            let tenths: u32 = match p.split_once('.') {
                Some((i, f)) => {
                    assert_eq!(f.len(), 1, "{v}");
                    i.parse::<u32>().unwrap() * 10 + f.parse::<u32>().unwrap()
                }
                None => p.parse::<u32>().unwrap() * 10,
            };
            (tenths * 255 / 1000).min(254) as u8
        }
    }
}

#[test]
fn gender_ratio_golden() {
    assert_eq!(gender_ratio("PERCENT_FEMALE(12.5)"), 31);
    assert_eq!(gender_ratio("PERCENT_FEMALE(50)"), 127);
    assert_eq!(gender_ratio("PERCENT_FEMALE(100)"), 254);
}

#[test]
fn us_species_match_decomp() {
    let (_, d) = us();
    let info = src("src/data/pokemon/species_info.h");
    let learnsets = src("src/data/pokemon/level_up_learnsets.h");
    let pointers = designated(
        &src("src/data/pokemon/level_up_learnset_pointers.h"),
        "gLevelUpLearnsets",
    );
    let all = entries(&info, "SPECIES_");
    assert_eq!(all.len(), 412);
    assert_eq!(d.species.species.len(), all.len());
    for (i, ((id, value), s)) in all.iter().zip(&d.species.species).enumerate() {
        let id = *id;
        assert_eq!(s.id, id);
        assert_eq!(names::SPECIES.value(id), Some(i as u32), "{id}");
        let ptr = &pointers.iter().find(|(k, _)| k == id).unwrap().1;
        let want: Vec<(u8, String)> = block(&learnsets, &format!("{ptr}[] ="))
            .lines()
            .filter_map(|l| l.trim().strip_prefix("LEVEL_UP_MOVE("))
            .map(|l| {
                let (lv, m) = l.trim_end_matches("),").split_once(',').unwrap();
                (lv.trim().parse().unwrap(), m.trim().to_string())
            })
            .collect();
        assert_eq!(s.learnset, want, "{id}");
        let f = match *value {
            "{0}," => {
                assert_eq!(s.base, [0; 6], "{id}");
                assert_eq!(s.catch_rate, 0, "{id}");
                continue;
            }
            "OLD_UNOWN_SPECIES_INFO," => fields(block(&info, "#define OLD_UNOWN_SPECIES_INFO")),
            "" => fields(block(&info, &format!("[{id}] ="))),
            v => panic!("{id} = {v}"),
        };
        let base = [
            "baseHP",
            "baseAttack",
            "baseDefense",
            "baseSpeed",
            "baseSpAttack",
            "baseSpDefense",
        ]
        .map(|k| num(&f[k]) as u8);
        assert_eq!(s.base, base, "{id}");
        assert_eq!(s.types.to_vec(), list(&f["types"]), "{id}");
        assert_eq!(s.catch_rate as u32, num(&f["catchRate"]), "{id}");
        assert_eq!(s.exp_yield as u32, num(&f["expYield"]), "{id}");
        let ev = [
            "evYield_HP",
            "evYield_Attack",
            "evYield_Defense",
            "evYield_Speed",
            "evYield_SpAttack",
            "evYield_SpDefense",
        ]
        .map(|k| num(&f[k]) as u8);
        assert_eq!(s.ev_yield, ev, "{id}");
        assert_eq!(s.gender_ratio, gender_ratio(&f["genderRatio"]), "{id}");
        assert_eq!(s.friendship as u32, num(&f["friendship"]), "{id}");
        assert_eq!(s.growth, f["growthRate"], "{id}");
        assert_eq!(s.abilities.to_vec(), list(&f["abilities"]), "{id}");
    }
    let by_id = |id: &str| &d.species.species[names::SPECIES.value(id).unwrap() as usize];
    let mudkip = by_id("SPECIES_MUDKIP");
    assert_eq!(mudkip.base, [50, 70, 50, 40, 50, 50]);
    assert_eq!(mudkip.gender_ratio, 31);
    assert_eq!(by_id("SPECIES_TREECKO").gender_ratio, 31);
    assert_eq!(mudkip.learnset[0], (1, "MOVE_TACKLE".to_string()));
}

#[test]
fn us_names_match_decomp() {
    let (_, d) = us();
    let species = designated(&src("src/data/text/species_names.h"), "gSpeciesNames");
    for (id, name) in &species {
        let s = d.species.species.iter().find(|s| &s.id == id).unwrap();
        same_text(&s.name, c_text(name), id);
    }
    let moves = designated(&src("src/data/text/move_names.h"), "gMoveNames");
    assert_eq!(moves.len(), d.moves.moves.len());
    for ((id, name), m) in moves.iter().zip(&d.moves.moves) {
        assert_eq!(id, &m.id);
        same_text(&m.name, c_text(name), id);
    }
    let types = designated(&src("src/battle_main.c"), "gTypeNames[");
    assert_eq!(types.len(), d.types.names.len());
    for ((id, name), ours) in types.iter().zip(&d.types.names) {
        same_text(ours, c_text(name), id);
    }
    let abilities = designated(&src("src/data/text/abilities.h"), "gAbilityNames[");
    assert_eq!(abilities.len(), 78);
    assert_eq!(abilities.len(), d.types.ability_names.len());
    for ((id, name), ours) in abilities.iter().zip(&d.types.ability_names) {
        assert_eq!(
            names::ABILITIES.value(id),
            Some(abilities.iter().position(|(i, _)| i == id).unwrap() as u32)
        );
        same_text(ours, c_text(name), id);
    }
}

#[test]
fn us_moves_match_decomp() {
    let (_, d) = us();
    let moves = src("src/data/battle_moves.h");
    let all = entries(&moves, "MOVE_");
    assert_eq!(all.len(), 355);
    assert_eq!(d.moves.moves.len(), all.len());
    for ((id, _), m) in all.iter().zip(&d.moves.moves) {
        let id = *id;
        assert_eq!(m.id, id);
        let f = fields(block(&moves, &format!("[{id}] =")));
        assert_eq!(m.effect, f["effect"], "{id}");
        assert_eq!(m.power as u32, num(&f["power"]), "{id}");
        assert_eq!(m.ty, f["type"], "{id}");
        assert_eq!(m.accuracy as u32, num(&f["accuracy"]), "{id}");
        assert_eq!(m.pp as u32, num(&f["pp"]), "{id}");
        assert_eq!(m.chance as u32, num(&f["secondaryEffectChance"]), "{id}");
        assert_eq!(m.target, f["target"], "{id}");
        assert_eq!(
            m.priority as i32,
            f["priority"].parse::<i32>().unwrap(),
            "{id}"
        );
        assert_eq!(m.flags, flags(&f["flags"]), "{id}");
    }
    let struggle = d
        .moves
        .moves
        .iter()
        .find(|m| m.id == "MOVE_STRUGGLE")
        .unwrap();
    assert_eq!(
        (struggle.effect.as_str(), struggle.power),
        ("EFFECT_RECOIL", 50)
    );
    let np = designated(&src("src/battle_script_commands.c"), "sNaturePowerMoves[]");
    let want: Vec<String> = np.into_iter().map(|(_, v)| v).collect();
    assert_eq!(d.moves.nature_power, want);
}

#[test]
fn us_type_chart_matches_decomp() {
    let (_, d) = us();
    let main = src("src/battle_main.c");
    let body = block(&main, "gTypeEffectiveness[");
    let mut chart = Vec::new();
    let mut split = None;
    for l in body.lines() {
        let parts: Vec<&str> = l.split(',').map(str::trim).collect();
        if parts.len() < 3 || !parts[0].starts_with("TYPE_") {
            continue;
        }
        let mul = match parts[2] {
            "TYPE_MUL_NO_EFFECT" => 0,
            "TYPE_MUL_NOT_EFFECTIVE" => 5,
            "TYPE_MUL_SUPER_EFFECTIVE" => 20,
            m => panic!("{m}"),
        };
        match parts[0] {
            "TYPE_FORESIGHT" => split = Some(chart.len()),
            "TYPE_ENDTABLE" => break,
            a => chart.push((a.to_string(), parts[1].to_string(), mul)),
        }
    }
    assert_eq!(chart.len(), 110);
    assert_eq!(d.types.chart, chart);
    assert_eq!(Some(d.types.foresight_split), split);
    assert_eq!(d.types.foresight_split, 108);
}

/// `sParty_*` -> its members as `(iv, level, species, item, moves)`.
type Party = Vec<(u16, u8, String, Option<String>, Option<Vec<String>>)>;

fn parties() -> HashMap<String, Party> {
    let s = src("src/data/trainer_parties.h");
    let mut out = HashMap::new();
    for chunk in s.split("static const struct ").skip(1) {
        let name = chunk
            .split_whitespace()
            .nth(1)
            .unwrap()
            .trim_end_matches("[]")
            .to_string();
        let body = block(chunk, &name);
        let mut party = Vec::new();
        for mon in body.split(".iv").skip(1) {
            let f = fields(&format!(".iv{mon}"));
            party.push((
                num(&f["iv"]) as u16,
                num(&f["lvl"]) as u8,
                f["species"].clone(),
                f.get("heldItem").cloned(),
                f.get("moves").map(|m| list(m)),
            ));
        }
        out.insert(name, party);
    }
    out
}

#[test]
fn us_trainers_match_decomp() {
    let (_, d) = us();
    let all = src("src/data/trainers.h");
    let parties = parties();
    assert_eq!(d.trainers.trainers.len(), 855);
    let classes = designated(
        &src("src/data/text/trainer_class_names.h"),
        "gTrainerClassNames",
    );
    for t in &d.trainers.trainers {
        let f = fields(block(&all, &format!("[{}] =", t.id)));
        let id = &t.id;
        assert_eq!(t.class, f["trainerClass"], "{id}");
        let class_name = &classes.iter().find(|(k, _)| k == &t.class).unwrap().1;
        same_text(&t.class_name, c_text(class_name), id);
        assert_eq!(t.pic, f["trainerPic"], "{id}");
        assert_eq!(
            t.female,
            f["encounterMusic_gender"].contains("F_TRAINER_FEMALE"),
            "{id}"
        );
        same_text(&t.name, c_text(&f["trainerName"]), id);
        assert_eq!(t.double as u32, num(&f["doubleBattle"]), "{id}");
        assert_eq!(t.ai, flags(&f["aiFlags"]), "{id}");
        let party_name = f["party"]
            .split_once('(')
            .map(|(_, p)| p.trim_end_matches(')').to_string());
        let want = party_name.map(|p| parties[&p].clone()).unwrap_or_default();
        let have: Party = t
            .party
            .iter()
            .map(|m| {
                (
                    m.iv,
                    m.level,
                    m.species.clone(),
                    m.item.clone(),
                    m.moves.clone(),
                )
            })
            .collect();
        assert_eq!(have, want, "{id}");
    }
    let may = d
        .trainers
        .trainers
        .iter()
        .find(|t| t.id == "TRAINER_MAY_ROUTE_103_MUDKIP")
        .unwrap();
    assert_eq!(may.class, "TRAINER_CLASS_RIVAL");
    assert_eq!(may.pic, "TRAINER_PIC_MAY");
    assert!(may.female && !may.double);
    assert_eq!(
        may.ai,
        [
            "AI_SCRIPT_CHECK_BAD_MOVE",
            "AI_SCRIPT_TRY_TO_FAINT",
            "AI_SCRIPT_CHECK_VIABILITY"
        ]
    );
    assert_eq!(may.party.len(), 1);
    assert_eq!(
        (may.party[0].species.as_str(), may.party[0].level),
        ("SPECIES_TREECKO", 5)
    );
    let calvin = d
        .trainers
        .trainers
        .iter()
        .find(|t| t.id == "TRAINER_CALVIN_1")
        .unwrap();
    assert_eq!(calvin.name, "CALVIN");
    assert_eq!(calvin.class, "TRAINER_CLASS_YOUNGSTER");
    assert_eq!(
        (calvin.party[0].species.as_str(), calvin.party[0].level),
        ("SPECIES_POOCHYENA", 5)
    );
}

/// One decomp `*_mons` table as an exported table, `None` when absent.
fn decomp_table(v: &serde_json::Value) -> Option<LandEncounters> {
    if v.is_null() {
        return None;
    }
    let mons = v["mons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| extract::schema::WildMon {
            min_level: m["min_level"].as_u64().unwrap() as u8,
            max_level: m["max_level"].as_u64().unwrap() as u8,
            species: m["species"].as_str().unwrap().to_string(),
        })
        .collect();
    Some(LandEncounters {
        rate: v["encounter_rate"].as_u64().unwrap() as u8,
        mons,
    })
}

/// `gWildMonHeaders` of `src/data/wild_encounters.json`, by map, in table order.
fn decomp_encounters() -> EncountersFile {
    let json: serde_json::Value =
        serde_json::from_str(&src("src/data/wild_encounters.json")).unwrap();
    let headers = json["wild_encounter_groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["label"] == "gWildMonHeaders")
        .unwrap();
    let mut want = EncountersFile::default();
    for e in headers["encounters"].as_array().unwrap() {
        let map = names::map_by_constant(e["map"].as_str().unwrap()).unwrap();
        want.maps
            .entry(map.dir.to_string())
            .or_default()
            .push(EncounterHeader {
                land: decomp_table(&e["land_mons"]),
                water: decomp_table(&e["water_mons"]),
                rock_smash: decomp_table(&e["rock_smash_mons"]),
                fishing: decomp_table(&e["fishing_mons"]),
            });
    }
    want
}

#[test]
fn us_encounters_match_decomp() {
    let (_, d) = us();
    let want = decomp_encounters();
    assert_eq!(d.encounters.maps.len(), want.maps.len());
    for (map, headers) in &want.maps {
        assert_eq!(d.encounters.maps.get(map), Some(headers), "{map}");
    }
    let land = |map: &str| d.encounters.maps[map][0].land.as_ref().unwrap();
    for map in ["Route101", "Route102", "Route103"] {
        assert_eq!(land(map).mons.len(), 12, "{map}");
    }
    assert_eq!(land("Route101").rate, 20);
    // Slot counts: land 12, water 5, rock smash 5, fishing 10 (`wild_encounters.json` fields).
    for (map, headers) in &d.encounters.maps {
        for h in headers {
            let n = |t: &Option<LandEncounters>| t.as_ref().map(|t| t.mons.len());
            assert!(n(&h.land).is_none_or(|n| n == 12), "{map}");
            assert!(n(&h.water).is_none_or(|n| n == 5), "{map}");
            assert!(n(&h.rock_smash).is_none_or(|n| n == 5), "{map}");
            assert!(n(&h.fishing).is_none_or(|n| n == 10), "{map}");
        }
    }
}

/// The rod groups of the `fishing_mons` field of `wild_encounters.json`.
#[test]
fn fishing_rod_slots_match_decomp() {
    let json: serde_json::Value =
        serde_json::from_str(&src("src/data/wild_encounters.json")).unwrap();
    let fields = json["wild_encounter_groups"][0]["fields"]
        .as_array()
        .unwrap();
    let fishing = fields.iter().find(|f| f["type"] == "fishing_mons").unwrap();
    let groups = &fishing["groups"];
    for (rod, slots) in ["old_rod", "good_rod", "super_rod"]
        .into_iter()
        .zip(FISHING_ROD_SLOTS)
    {
        let want: Vec<u64> = groups[rod]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let got: Vec<u64> = slots.map(|i| i as u64).collect();
        assert_eq!(got, want, "{rod}");
    }
}

#[test]
fn altering_cave_has_nine_wild_sets() {
    for (_, d) in [us(), fr()] {
        let sets = &d.encounters.maps["AlteringCave"];
        assert_eq!(sets.len(), 9);
        assert!(sets.iter().all(|h| h.land.is_some()));
        // Every other map has one header.
        let multi: Vec<&String> = d
            .encounters
            .maps
            .iter()
            .filter(|(_, h)| h.len() != 1)
            .map(|(m, _)| m)
            .collect();
        assert_eq!(multi, ["AlteringCave"]);
    }
}

#[test]
fn encounters_are_limited_to_the_exported_maps() {
    let rom = fixtures::us();
    let d = battle_data::extract(&rom, &["Route101", "LittlerootTown", "AlteringCave"]).unwrap();
    assert_eq!(
        d.encounters.maps.keys().collect::<Vec<_>>(),
        ["AlteringCave", "Route101"]
    );
}

/// `gEvolutionTable` from `src/data/pokemon/evolution.h`.
fn decomp_evolutions() -> EvolutionsFile {
    let text = src("src/data/pokemon/evolution.h");
    let body = text.split_once("] =\n{").unwrap().1;
    let mut want = EvolutionsFile::default();
    for chunk in body.split("[SPECIES_").skip(1) {
        let (species, rest) = chunk.split_once(']').unwrap();
        let mut evos = Vec::new();
        for e in rest
            .split('{')
            .filter(|e| e.trim_start().starts_with("EVO_"))
        {
            let e = e.split('}').next().unwrap();
            let f: Vec<&str> = e.split(',').map(str::trim).collect();
            let param = match f[1].parse::<u16>() {
                Ok(v) => v,
                Err(_) => names::ITEMS.value(f[1]).unwrap() as u16,
            };
            evos.push((f[0].to_string(), param, f[2].to_string()));
        }
        want.species.insert(format!("SPECIES_{species}"), evos);
    }
    want
}

#[test]
fn us_evolutions_match_decomp() {
    let (_, d) = us();
    let want = decomp_evolutions();
    assert!(want.species.len() > 150, "{}", want.species.len());
    assert_eq!(d.evolutions, want);
    assert_eq!(
        d.evolutions.species["SPECIES_TREECKO"],
        [("EVO_LEVEL".to_string(), 16, "SPECIES_GROVYLE".to_string())]
    );
}

#[test]
fn evo_methods_match_decomp() {
    let text = src("include/constants/pokemon.h");
    let want: Vec<(String, u16)> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.strip_prefix("#define ")?.split_whitespace();
            let (name, v) = (w.next()?, w.next()?);
            (name.starts_with("EVO_") && !name.starts_with("EVO_MODE"))
                .then(|| (name.to_string(), v.parse().unwrap()))
        })
        .collect();
    let have: Vec<(String, u16)> = battle_data::EVO_METHODS
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, n)| (n.to_string(), i as u16))
        .collect();
    assert_eq!(have, want);
}

#[test]
fn us_trainer_money_matches_decomp() {
    let (_, d) = us();
    let text = src("src/battle_main.c");
    let body = text
        .split_once("gTrainerMoneyTable[] =")
        .unwrap()
        .1
        .split_once("};")
        .unwrap()
        .0;
    let mut want = TrainerMoneyFile::default();
    for e in body.split('{').skip(2) {
        let e = e.split('}').next().unwrap();
        let (class, value) = e.split_once(',').unwrap();
        let class = match class.trim() {
            "0xFF" => names::TRAINER_CLASSES.name_or_hex(0xFF),
            c => c.to_string(),
        };
        want.classes.insert(class, value.trim().parse().unwrap());
    }
    assert!(want.classes.len() > 40, "{}", want.classes.len());
    assert_eq!(d.trainer_money, want);
    assert_eq!(d.trainer_money.classes[TRAINER_MONEY_DEFAULT_CLASS], 5);
}

#[test]
fn us_heal_locations_match_decomp() {
    let (_, d) = us();
    let json: serde_json::Value =
        serde_json::from_str(&src("src/data/heal_locations.json")).unwrap();
    let want: HealLocationsFile = json["heal_locations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            let map = names::map_by_constant(h["map"].as_str().unwrap()).unwrap();
            (
                h["id"].as_str().unwrap().to_string(),
                (
                    map.dir.to_string(),
                    h["x"].as_i64().unwrap() as i16,
                    h["y"].as_i64().unwrap() as i16,
                ),
            )
        })
        .collect();
    assert_eq!(d.heal_locations, want);
}

#[test]
fn us_experience_tables_match_the_decomp_formulas() {
    let (rom, _) = us();
    let loc = battle_data::locate(rom).unwrap();
    let t = battle_data::read_experience_tables(rom, loc.experience_tables).unwrap();
    // include/constants/experience_tables.h (gExperienceTables in src/pokemon.c), n = level.
    let sq = |n: i64| n * n;
    let cube = |n: i64| n * n * n;
    let erratic = |n: i64| {
        if n <= 50 {
            (100 - n) * cube(n) / 50
        } else if n <= 68 {
            (150 - n) * cube(n) / 100
        } else if n <= 98 {
            ((1911 - 10 * n) / 3) * cube(n) / 500
        } else {
            (160 - n) * cube(n) / 100
        }
    };
    let fluctuating = |n: i64| {
        if n <= 15 {
            ((n + 1) / 3 + 24) * cube(n) / 50
        } else if n <= 36 {
            (n + 14) * cube(n) / 50
        } else {
            (n / 2 + 32) * cube(n) / 50
        }
    };
    let medium_slow = |n: i64| 6 * cube(n) / 5 - 15 * sq(n) + 100 * n - 140;
    let formulas: [&dyn Fn(i64) -> i64; 6] = [
        &cube,
        &erratic,
        &fluctuating,
        &medium_slow,
        &|n| 4 * cube(n) / 5,
        &|n| 5 * cube(n) / 4,
    ];
    for (g, f) in formulas.iter().enumerate() {
        for level in 0..=100 {
            // Level 0 is 0 and level 1 is 1 in every table (MediumSlow(1) would be -54).
            let want = match level {
                0 => 0,
                1 => 1,
                n => f(n),
            };
            assert_eq!(
                t[g][level as usize] as i64, want,
                "growth {g} level {level}"
            );
        }
    }
}

/// `STRINGID_*` -> `sText_*` from `gBattleStringsTable`.
fn table_names() -> Vec<(String, String)> {
    designated(&src("src/battle_message.c"), "gBattleStringsTable[")
        .into_iter()
        .map(|(k, v)| (k.split_whitespace().next().unwrap().to_string(), v))
        .collect()
}

#[test]
fn us_strings_match_decomp() {
    let (_, d) = us();
    let defs: HashMap<String, String> = c_strings(&src("src/battle_message.c"))
        .into_iter()
        .chain(c_strings(&src("src/strings.c")))
        .chain(inc_strings(&src("data/text/pc_transfer.inc")))
        .collect();
    let table = table_names();
    assert_eq!(table.len(), 369);
    for (id, def) in &table {
        same_text(&d.strings[id], &defs[def], id);
    }
    for name in CODE_STRINGS
        .iter()
        .chain(&MENU_STRINGS)
        .chain(&["gText_BirchInTrouble", "gText_ConfirmStarterChoice"])
    {
        same_text(&d.strings[*name], &defs[*name], name);
    }
    assert_eq!(
        d.strings.len(),
        369 + CODE_STRINGS.len() + MENU_STRINGS.len() + 2 + STAT_NAME_KEYS.len()
    );
    // gStatNamesTable in STAT_* order.
    let stats = designated(&src("src/battle_message.c"), "gStatNamesTable[");
    assert_eq!(stats.len(), STAT_NAME_KEYS.len());
    for (key, (stat, def)) in STAT_NAME_KEYS.iter().zip(&stats) {
        same_text(&d.strings[*key], &defs[def], stat);
    }
    assert_eq!(
        d.strings["STRINGID_PKMNGAINEDEXP"],
        "{B_BUFF1} gained{B_BUFF2}\\n{B_BUFF3} EXP. Points!\\p"
    );
    assert_eq!(d.strings["sText_WildPkmnPrefix"], "Wild ");
    assert!(!battle_data::tag_is_suffix(&d.strings["sText_WildPkmnPrefix"]).unwrap());
    assert!(!battle_data::tag_is_suffix(&d.strings["sText_FoePkmnPrefix"]).unwrap());
}

/// The walked string lists are the decomp's source order.
#[test]
fn walked_strings_follow_the_decomp_source_order() {
    let names: Vec<String> = c_strings(&src("src/battle_message.c"))
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let after = names
        .iter()
        .position(|n| n == "sText_OutOfSafariBalls")
        .unwrap();
    assert_eq!(
        names[after + 1..after + 1 + CODE_STRINGS.len()],
        CODE_STRINGS
    );
    let first = names
        .iter()
        .position(|n| n == "gText_WhatWillPkmnDo")
        .unwrap();
    assert_eq!(names[first..first + MENU_STRINGS.len()], MENU_STRINGS);
    let table = table_names();
    assert!(
        table
            .iter()
            .any(|(k, v)| k == "STRINGID_OUTOFSAFARIBALLS" && v == "sText_OutOfSafariBalls")
    );
    let strings = c_strings(&src("src/strings.c"));
    let at = strings
        .iter()
        .position(|(n, _)| n == "gText_ConfirmStarterChoice")
        .unwrap();
    assert_eq!(strings[at - 1].0, "gText_BirchInTrouble");
}

#[test]
fn fr_numbers_match_us() {
    let (_, u) = us();
    let (_, f) = fr();
    assert_eq!(u.species.species.len(), f.species.species.len());
    for (a, b) in u.species.species.iter().zip(&f.species.species) {
        let mut b = b.clone();
        b.name = a.name.clone();
        assert_eq!(a, &b);
    }
    assert_eq!(u.moves.nature_power, f.moves.nature_power);
    for (a, b) in u.moves.moves.iter().zip(&f.moves.moves) {
        let mut b = b.clone();
        b.name = a.name.clone();
        assert_eq!(a, &b);
    }
    assert_eq!(u.types.chart, f.types.chart);
    assert_eq!(u.types.foresight_split, f.types.foresight_split);
    for (a, b) in u.trainers.trainers.iter().zip(&f.trainers.trainers) {
        let mut b = b.clone();
        b.name = a.name.clone();
        b.class_name = a.class_name.clone();
        assert_eq!(a, &b);
    }
    assert_eq!(u.encounters, f.encounters);
    assert_eq!(u.evolutions, f.evolutions);
    assert_eq!(u.trainer_money, f.trainer_money);
    assert_eq!(u.heal_locations, f.heal_locations);
    assert_eq!(
        u.strings.keys().collect::<Vec<_>>(),
        f.strings.keys().collect::<Vec<_>>()
    );
}

// The French tests assert structure, not French text: we keep ROM text out of the repository.

#[test]
fn fr_names_decode_and_reencode() {
    let (rom, d) = fr();
    let (_, u) = us();
    let loc = battle_data::locate(rom).unwrap();
    let torchic = names::SPECIES.value("SPECIES_TORCHIC").unwrap();
    let s = &d.species.species[torchic as usize];
    assert_eq!(s.id, "SPECIES_TORCHIC");
    assert!(!s.name.is_empty());
    assert_ne!(s.name, u.species.species[torchic as usize].name);
    let bytes = text::encode_lang(&s.name, rom.lang()).unwrap();
    assert_eq!(
        rom.slice(loc.species_names + 11 * torchic, bytes.len())
            .unwrap(),
        &bytes[..]
    );
    // Every name re-encodes to its ROM bytes (the extraction checks each one); spot-check that
    // they differ from English.
    let tackle = names::MOVES.value("MOVE_TACKLE").unwrap() as usize;
    assert_eq!(d.moves.moves[tackle].id, "MOVE_TACKLE");
    assert!(!d.moves.moves[tackle].name.is_empty());
    assert_ne!(d.moves.moves[tackle].name, u.moves.moves[tackle].name);
    let fire = names::TYPES.value("TYPE_FIRE").unwrap() as usize;
    assert!(!d.types.names[fire].is_empty());
    assert_ne!(d.types.names[fire], u.types.names[fire]);
    let trainer = |data: &'static BattleData, id: &str| {
        let i = names::TRAINERS.value(id).unwrap() as usize;
        let t = &data.trainers.trainers[i];
        assert_eq!(t.id, id);
        t
    };
    let may = "TRAINER_MAY_ROUTE_103_MUDKIP";
    assert!(!trainer(d, may).name.is_empty());
    assert_ne!(trainer(d, may).name, trainer(u, may).name);
    let calvin = "TRAINER_CALVIN_1";
    assert_ne!(trainer(d, calvin).class_name, trainer(u, calvin).class_name);
    // gAbilityNames: one name per ABILITY_*, the translated ones differ from English.
    assert_eq!(d.types.ability_names.len(), u.types.ability_names.len());
    for id in [
        "ABILITY_INTIMIDATE",
        "ABILITY_STATIC",
        "ABILITY_SYNCHRONIZE",
        "ABILITY_TRACE",
    ] {
        let i = names::ABILITIES.value(id).unwrap() as usize;
        assert!(!d.types.ability_names[i].is_empty(), "{id}");
        assert_ne!(d.types.ability_names[i], u.types.ability_names[i], "{id}");
    }
}

#[test]
fn fr_wild_and_foe_tags_are_suffixes() {
    let (_, d) = fr();
    let (_, u) = us();
    for tag in ["sText_WildPkmnPrefix", "sText_FoePkmnPrefix"] {
        let s = &d.strings[tag];
        assert!(
            s.starts_with(' ') && !s.ends_with(' ') && s.len() > 1,
            "{tag}"
        );
        assert!(battle_data::tag_is_suffix(s).unwrap(), "{tag}");
    }
    let appeared = &d.strings["sText_WildPkmnAppeared"];
    assert!(appeared.contains("{B_OPPONENT_MON1_NAME}") && appeared.ends_with("\\p"));
    assert_ne!(appeared, &u.strings["sText_WildPkmnAppeared"]);
    assert!(d.strings["STRINGID_PKMNGAINEDEXP"].starts_with("{B_BUFF1}"));
    let confirm = "gText_ConfirmStarterChoice";
    assert!(!d.strings[confirm].is_empty());
    assert_ne!(d.strings[confirm], u.strings[confirm]);
    // The battle menu keeps the US control structure: two columns, two rows.
    let menu = &d.strings["gText_BattleMenu"];
    assert_ne!(menu, &u.strings["gText_BattleMenu"]);
    assert_eq!(menu.matches("{CLEAR_TO 56}").count(), 2);
    assert_eq!(menu.matches("\\n").count(), 1);
    for (k, v) in &d.strings {
        text::encode_lang(v, extract::text::Lang::Fr).unwrap_or_else(|e| panic!("{k}: {e:#}"));
    }
}

/// `gText_BattleYesNoChoice`: the US control-code prefix, then two words on two lines.
#[test]
fn fr_yes_no_choice_is_two_lines() {
    let key = "gText_BattleYesNoChoice";
    let us_text = &us().1.strings[key];
    let prefix = &us_text[..=us_text.rfind('}').unwrap()];
    let fr_text = &fr().1.strings[key];
    assert_ne!(fr_text, us_text);
    let rest = fr_text.strip_prefix(prefix).unwrap();
    let words: Vec<&str> = rest.split("\\n").collect();
    assert_eq!(words.len(), 2, "{key}");
    for w in words {
        assert!(!w.is_empty() && !w.contains('{'), "{key}");
    }
}

/// BPEF prints `STRINGID_STATSHARPLY`/`STRINGID_STATHARSHLY` as whole messages and skips the
/// buffer entry after them (`ExpandBattleTextBuffPlaceholders`, see the module docs); BPEE
/// composes "sharply " + "rose!".
#[test]
fn fr_stat_sharply_messages_are_complete() {
    let (us_rom, u) = us();
    let (fr_rom, f) = fr();
    assert!(battle_data::stat_sharply_is_complete(&f.strings).unwrap());
    assert!(!battle_data::stat_sharply_is_complete(&u.strings).unwrap());
    // `cmp r2, #STATSHARPLY; beq; cmp r2, #STATHARSHLY; bne; adds r5, #3`.
    let id = |n: &str| names::STRING_IDS.value(n).unwrap() as u8;
    let skip = [
        id("STRINGID_STATSHARPLY"),
        0x2a,
        0x01,
        0xd0,
        id("STRINGID_STATHARSHLY"),
        0x2a,
        0x00,
        0xd1,
        0x03,
        0x35,
    ];
    assert_eq!(fr_rom.find_all(&skip).len(), 1);
    assert!(us_rom.find_all(&skip).is_empty());
}

/// [`STAT_NAME_KEYS`] are `gStatNamesTable` in `STAT_*` order, in both languages.
#[test]
fn stat_name_keys_follow_the_stat_names_table() {
    let stats = designated(&src("src/battle_message.c"), "gStatNamesTable[");
    for d in [&us().1, &fr().1] {
        for (key, (stat, def)) in STAT_NAME_KEYS.iter().zip(&stats) {
            assert_eq!(d.strings[*key], d.strings[def], "{stat}");
            assert!(!d.strings[*key].is_empty(), "{stat}");
        }
    }
}

#[test]
fn export_writes_battle_files_that_round_trip() {
    let (_, d) = fr();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("battle-data-export");
    let _ = std::fs::remove_dir_all(&dir);
    let manifest: Manifest =
        export::export(&fixtures::fr(), &dir, &ExportOptions::slice(false)).unwrap();
    let read = |f: &str| dir.join("battle").join(f);
    assert_eq!(
        export::read_ron::<SpeciesFile>(&read("species.ron")).unwrap(),
        d.species
    );
    assert_eq!(
        export::read_ron::<MovesFile>(&read("moves.ron")).unwrap(),
        d.moves
    );
    assert_eq!(
        export::read_ron::<TypesFile>(&read("types.ron")).unwrap(),
        d.types
    );
    assert_eq!(
        export::read_ron::<TrainersFile>(&read("trainers.ron")).unwrap(),
        d.trainers
    );
    // The slice export holds the slice maps' headers only.
    let mut slice = d.encounters.clone();
    slice.maps.retain(|m, _| SLICE_MAPS.contains(&m.as_str()));
    assert_eq!(
        export::read_ron::<EncountersFile>(&read("encounters.ron")).unwrap(),
        slice
    );
    assert_eq!(
        export::read_ron::<EvolutionsFile>(&read("evolutions.ron")).unwrap(),
        d.evolutions
    );
    assert_eq!(
        export::read_ron::<TrainerMoneyFile>(&read("trainer_money.ron")).unwrap(),
        d.trainer_money
    );
    assert_eq!(
        export::read_ron::<HealLocationsFile>(&read("heal_locations.ron")).unwrap(),
        d.heal_locations
    );
    assert_eq!(
        export::read_ron::<BattleStringsFile>(&read("strings.ron")).unwrap(),
        d.strings
    );
    for f in [
        "species",
        "moves",
        "types",
        "trainers",
        "encounters",
        "evolutions",
        "trainer_money",
        "heal_locations",
        "strings",
    ] {
        assert!(
            manifest.files.contains_key(&format!("battle/{f}.ron")),
            "{f}"
        );
    }
    for (k, v) in &d.provenance {
        assert_eq!(manifest.provenance.get(k), Some(v), "{k}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Each species' `gPokedexEntries[SpeciesToNationalPokedexNum(species)].weight` from the decomp
/// (`SPECIES_TO_NATIONAL` in `src/pokemon.c`, `src/data/pokemon/pokedex_entries.h`); 0 for
/// species without an entry (`SPECIES_NONE`, the old Unown placeholders).
fn decomp_weights() -> BTreeMap<String, u16> {
    let entries = src("src/data/pokemon/pokedex_entries.h");
    let mut by_dex = HashMap::new();
    for chunk in entries.split("[NATIONAL_DEX_").skip(1) {
        let (dex, rest) = chunk.split_once(']').unwrap();
        let w = rest.split_once(".weight = ").unwrap().1;
        let w: u16 = w.split(',').next().unwrap().trim().parse().unwrap();
        by_dex.insert(dex.to_string(), w);
    }
    let pokemon = src("src/pokemon.c");
    let mut out = BTreeMap::new();
    for chunk in pokemon.split("    SPECIES_TO_NATIONAL(").skip(1) {
        let name = chunk.split(')').next().unwrap();
        out.insert(
            format!("SPECIES_{name}"),
            by_dex.get(name).copied().unwrap_or(0),
        );
    }
    out
}

#[test]
fn us_weights_match_decomp() {
    let (_, d) = us();
    let want = decomp_weights();
    assert!(want.len() > 400, "{}", want.len());
    assert_eq!(d.species.species[0].weight, 0);
    for s in &d.species.species[1..] {
        assert_eq!(Some(&s.weight), want.get(&s.id), "{}", s.id);
    }
    let w = |id: &str| {
        d.species
            .species
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .weight
    };
    assert_eq!(w("SPECIES_BULBASAUR"), 69);
    assert_eq!(w("SPECIES_ZIGZAGOON"), 175);
}

/// Wave C: `battle/dex.ron` is the decomp's `HOENN_DEX_*` order (`include/constants/pokedex.h`,
/// each `HOENN_DEX_<X>` is `SPECIES_<X>`), and `battle/rematches.ron` its `gRematchTable`
/// (`REMATCH(t1, …, t5, map)` lines of `src/battle_setup.c`), on both ROMs.
#[test]
fn hoenn_dex_and_rematches_match_decomp() {
    let h = src("include/constants/pokedex.h");
    let want_dex: Vec<String> = h
        .lines()
        .map(str::trim)
        .skip_while(|l| *l != "HOENN_DEX_TREECKO,")
        .take(battle_data::HOENN_DEX_COUNT as usize)
        .map(|l| format!("SPECIES_{}", &l["HOENN_DEX_".len()..l.len() - 1]))
        .collect();
    assert_eq!(want_dex.len(), battle_data::HOENN_DEX_COUNT as usize);
    let c = src("src/battle_setup.c");
    let want_rematches: Vec<(Vec<String>, String)> = c
        .lines()
        .filter_map(|l| l.trim().split_once("= REMATCH(").map(|(_, r)| r))
        .map(|r| {
            let args: Vec<String> = r
                .trim_end_matches("),")
                .split(',')
                .map(|a| a.trim().to_string())
                .collect();
            let map = names::map_by_constant(&args[5]).unwrap().dir.to_string();
            (args[..5].to_vec(), map)
        })
        .collect();
    assert_eq!(
        want_rematches.len(),
        battle_data::REMATCH_TABLE_ENTRIES as usize
    );
    for (_, data) in [us(), fr()] {
        assert_eq!(data.dex.hoenn, want_dex);
        let got: Vec<(Vec<String>, String)> = (data.rematches.rematches.iter())
            .map(|r| (r.trainers.clone(), r.map.clone()))
            .collect();
        assert_eq!(got, want_rematches);
    }
}
