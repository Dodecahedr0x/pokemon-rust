//! Battle data and strings (`battle_data.rs`) against the decomp: the US ROM must give exactly
//! the decomp's values, the French ROM the same numbers with French names and strings.
use extract::battle_data::{self, BattleData, CODE_STRINGS, MENU_STRINGS};
use extract::export::{self, MAPS};
use extract::schema::{
    BattleStringsFile, EncountersFile, HealLocationsFile, Manifest, MovesFile, SpeciesFile,
    TrainersFile, TypesFile,
};
use extract::{Rom, fixtures, names, text};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

fn us() -> &'static (Rom, BattleData) {
    static D: OnceLock<(Rom, BattleData)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::us();
        let data = battle_data::extract(&rom, &MAPS).unwrap();
        (rom, data)
    })
}

fn fr() -> &'static (Rom, BattleData) {
    static D: OnceLock<(Rom, BattleData)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::fr();
        let data = battle_data::extract(&rom, &MAPS).unwrap();
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
            Some((
                n.trim().to_string(),
                v.trim().trim_end_matches(',').trim().to_string(),
            ))
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

#[test]
fn us_species_match_decomp() {
    let (_, d) = us();
    let info = src("src/data/pokemon/species_info.h");
    let learnsets = src("src/data/pokemon/level_up_learnsets.h");
    let pointers = designated(
        &src("src/data/pokemon/level_up_learnset_pointers.h"),
        "gLevelUpLearnsets",
    );
    assert_eq!(d.species.species.len(), 412);
    assert_eq!(d.species.species[0].id, "SPECIES_NONE");
    for id in [
        "SPECIES_TREECKO",
        "SPECIES_MUDKIP",
        "SPECIES_ZIGZAGOON",
        "SPECIES_POOCHYENA",
    ] {
        let s = d.species.species.iter().find(|s| s.id == id).unwrap();
        let f = fields(block(&info, &format!("[{id}] =")));
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
        // PERCENT_FEMALE(p) = min(254, p * 255 / 100), truncated to u8.
        let pct: f64 = f["genderRatio"]
            .strip_prefix("PERCENT_FEMALE(")
            .unwrap()
            .trim_end_matches(')')
            .parse()
            .unwrap();
        assert_eq!(
            s.gender_ratio,
            (pct * 255.0 / 100.0).min(254.0) as u8,
            "{id}"
        );
        assert_eq!(s.friendship as u32, num(&f["friendship"]), "{id}");
        assert_eq!(s.growth, f["growthRate"], "{id}");
        assert_eq!(s.abilities.to_vec(), list(&f["abilities"]), "{id}");
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
    }
    let mudkip = d
        .species
        .species
        .iter()
        .find(|s| s.id == "SPECIES_MUDKIP")
        .unwrap();
    assert_eq!(mudkip.base, [50, 70, 50, 40, 50, 50]);
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
}

#[test]
fn us_moves_match_decomp() {
    let (_, d) = us();
    let moves = src("src/data/battle_moves.h");
    for id in ["MOVE_TACKLE", "MOVE_EMBER", "MOVE_GROWL", "MOVE_STRUGGLE"] {
        let m = d.moves.moves.iter().find(|m| m.id == id).unwrap();
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

/// `(rate, [(species, min, max)])`.
type LandTable = (u8, Vec<(String, u8, u8)>);

#[test]
fn us_encounters_and_heal_locations_match_decomp() {
    let (_, d) = us();
    let json: serde_json::Value =
        serde_json::from_str(&src("src/data/wild_encounters.json")).unwrap();
    let headers = json["wild_encounter_groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["label"] == "gWildMonHeaders")
        .unwrap();
    let mut want: BTreeMap<String, LandTable> = BTreeMap::new();
    for e in headers["encounters"].as_array().unwrap() {
        let map = names::map_by_constant(e["map"].as_str().unwrap()).unwrap();
        if !MAPS.contains(&map.dir) || e["land_mons"].is_null() {
            continue;
        }
        let land = &e["land_mons"];
        let mons: Vec<(String, u8, u8)> = land["mons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m["species"].as_str().unwrap().to_string(),
                    m["min_level"].as_u64().unwrap() as u8,
                    m["max_level"].as_u64().unwrap() as u8,
                )
            })
            .collect();
        want.insert(
            map.dir.to_string(),
            (land["encounter_rate"].as_u64().unwrap() as u8, mons),
        );
    }
    let have: BTreeMap<String, LandTable> = d
        .encounters
        .maps
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                (
                    v.rate,
                    v.mons
                        .iter()
                        .map(|m| (m.species.clone(), m.min_level, m.max_level))
                        .collect(),
                ),
            )
        })
        .collect();
    assert_eq!(have, want);
    for map in ["Route101", "Route102", "Route103"] {
        assert_eq!(d.encounters.maps[map].mons.len(), 12, "{map}");
    }
    assert_eq!(d.encounters.maps["Route101"].rate, 20);

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
        369 + CODE_STRINGS.len() + MENU_STRINGS.len() + 2
    );
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
    assert_eq!(u.heal_locations, f.heal_locations);
    assert_eq!(
        u.strings.keys().collect::<Vec<_>>(),
        f.strings.keys().collect::<Vec<_>>()
    );
}

#[test]
fn fr_names_decode_and_reencode() {
    let (rom, d) = fr();
    let loc = battle_data::locate(rom).unwrap();
    let torchic = names::SPECIES.value("SPECIES_TORCHIC").unwrap();
    let s = &d.species.species[torchic as usize];
    assert_eq!(s.id, "SPECIES_TORCHIC");
    assert_eq!(s.name, "POUSSIFEU");
    let bytes = text::encode_lang(&s.name, rom.lang()).unwrap();
    assert_eq!(
        rom.slice(loc.species_names + 11 * torchic, bytes.len())
            .unwrap(),
        &bytes[..]
    );
    let tackle = d
        .moves
        .moves
        .iter()
        .find(|m| m.id == "MOVE_TACKLE")
        .unwrap();
    assert_eq!(tackle.name, "CHARGE");
    assert_eq!(d.types.names[10], "FEU");
    let may = d
        .trainers
        .trainers
        .iter()
        .find(|t| t.id == "TRAINER_MAY_ROUTE_103_MUDKIP")
        .unwrap();
    assert_eq!(may.name, "FLORA");
    // Every name re-encodes to its ROM bytes (the extraction checks each one); spot-check that
    // they differ from English.
    let (_, u) = us();
    assert_ne!(
        u.trainers.trainers[318].class_name,
        d.trainers.trainers[318].class_name
    );
}

#[test]
fn fr_wild_and_foe_tags_are_suffixes() {
    let (_, d) = fr();
    assert_eq!(d.strings["sText_WildPkmnPrefix"], " sauvage");
    assert_eq!(d.strings["sText_FoePkmnPrefix"], " ennemi");
    assert!(battle_data::tag_is_suffix(&d.strings["sText_WildPkmnPrefix"]).unwrap());
    assert!(battle_data::tag_is_suffix(&d.strings["sText_FoePkmnPrefix"]).unwrap());
    assert_eq!(
        d.strings["sText_WildPkmnAppeared"],
        "Un {B_OPPONENT_MON1_NAME} sauvage apparaît!\\p"
    );
    assert!(d.strings["STRINGID_PKMNGAINEDEXP"].starts_with("{B_BUFF1}"));
    assert_eq!(
        d.strings["gText_ConfirmStarterChoice"],
        "Prendre ce POKéMON?"
    );
    assert_eq!(
        d.strings["gText_BattleMenu"],
        "ATTAQUE{CLEAR_TO 56}SAC\\nPOKéMON{CLEAR_TO 56}FUITE"
    );
    for (k, v) in &d.strings {
        text::encode_lang(v, extract::text::Lang::Fr).unwrap_or_else(|e| panic!("{k}: {e:#}"));
    }
}

#[test]
fn export_writes_battle_files_that_round_trip() {
    let (_, d) = fr();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("battle-data-export");
    let _ = std::fs::remove_dir_all(&dir);
    let manifest: Manifest = export::export(&fixtures::fr(), &dir).unwrap();
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
    assert_eq!(
        export::read_ron::<EncountersFile>(&read("encounters.ron")).unwrap(),
        d.encounters
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
