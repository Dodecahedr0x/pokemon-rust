//! Item, multichoice, std-string and map-section tables (`tables.rs`) against the decomp: the US
//! ROM must give exactly the decomp's values and strings, the French ROM the same counts and
//! numbers with strings that re-encode to their ROM bytes. `export_tables` writes them as
//! `tables/*.ron`.
use extract::export::Out;
use extract::export::tables::{FILES, export_tables};
use extract::schema::{
    FORMAT_MINOR, Hex, ItemPocket, ItemsFile, MINOR_FILES, MapSectionsFile, MultichoiceFile,
    StdStringsFile,
};
use extract::tables::{self, ITEM_SIZE, Tables};
use extract::{Rom, fixtures, names, text};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn us() -> &'static (Rom, Tables) {
    static D: OnceLock<(Rom, Tables)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::us();
        let t = tables::extract(&rom).unwrap();
        (rom, t)
    })
}

fn fr() -> &'static (Rom, Tables) {
    static D: OnceLock<(Rom, Tables)> = OnceLock::new();
    D.get_or_init(|| {
        let rom = fixtures::fr();
        let t = tables::extract(&rom).unwrap();
        (rom, t)
    })
}

fn src(rel: &str) -> String {
    let p = fixtures::decomp().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `#define <prefix>NAME N` lines of a header, in source order.
fn defines(src: &str, prefix: &str) -> Vec<(String, u32)> {
    src.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            (w.next()? == "#define").then_some(())?;
            let name = w.next()?;
            let v = w.next()?.parse().ok()?;
            name.starts_with(prefix).then(|| (name.to_string(), v))
        })
        .collect()
}

/// The text of a `_("…")` C string literal.
fn c_literal(s: &str) -> Option<&str> {
    s.split_once("_(\"")?.1.rsplit_once("\")").map(|(b, _)| b)
}

/// `NAME[] = _("…");` definitions of a C file and `Label::` + `.string "…$"` ones of an
/// assembly file.
fn string_defs(srcs: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for s in srcs {
        let mut label: Option<String> = None;
        for line in s.lines() {
            if let Some((head, _)) = line.split_once("[] = _(\"") {
                let name = head.split_whitespace().last().unwrap().to_string();
                out.insert(name, c_literal(line).unwrap().to_string());
            } else if let Some(l) = line.strip_suffix("::") {
                label = Some(l.to_string());
                out.insert(l.to_string(), String::new());
            } else if let Some(body) = line.trim().strip_prefix(".string \"")
                && let Some(l) = &label
            {
                let body = body.strip_suffix('"').unwrap().trim_end_matches('$');
                out.get_mut(l).unwrap().push_str(body);
            }
        }
    }
    out
}

struct DecompItem {
    id: String,
    name: String,
    price: u16,
    pocket: u8,
    importance: u8,
}

fn decomp_items() -> Vec<DecompItem> {
    let pockets: BTreeMap<String, u32> = defines(&src("include/constants/item.h"), "POCKET_")
        .into_iter()
        .collect();
    let mut out: Vec<DecompItem> = Vec::new();
    for line in src("src/data/items.h").lines() {
        let l = line.trim();
        if let Some(id) = l.strip_prefix("[ITEM_").and_then(|r| r.strip_suffix("] =")) {
            out.push(DecompItem {
                id: format!("ITEM_{id}"),
                name: String::new(),
                price: 0,
                pocket: 0,
                importance: 0,
            });
            continue;
        }
        let Some(it) = out.last_mut() else { continue };
        let field = |f: &str| l.strip_prefix(f).map(|v| v.trim_end_matches(','));
        if l.starts_with(".name = ") {
            it.name = c_literal(l).unwrap().to_string();
        } else if let Some(v) = field(".price = ") {
            it.price = v.parse().unwrap();
        } else if let Some(v) = field(".importance = ") {
            it.importance = v.parse().unwrap();
        } else if let Some(v) = field(".pocket = ") {
            it.pocket = pockets[v] as u8;
        }
    }
    out
}

/// `MULTI_*` (without `MULTI_B_PRESSED`) -> the decomp names of its strings, in order.
fn decomp_multichoice() -> Vec<(String, Vec<String>)> {
    let h = src("src/data/script_menu.h");
    let table = h.find("sMultichoiceLists[] =").unwrap();
    let mut lists: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current = None;
    for line in h[..table].lines() {
        if let Some(rest) = line.strip_prefix("static const struct MenuAction ") {
            let name = rest.split_once("[]").unwrap().0.to_string();
            lists.insert(name.clone(), Vec::new());
            current = Some(name);
        } else if let Some(t) = line
            .split("//")
            .next()
            .unwrap()
            .trim()
            .strip_prefix('{')
            .and_then(|r| r.strip_suffix("},"))
            && let Some(c) = &current
        {
            lists.get_mut(c).unwrap().push(t.to_string());
        } else if line.starts_with("};") {
            current = None;
        }
    }
    let body = &h[table..];
    let body = &body[..body.find("\n};").unwrap()];
    let entries: BTreeMap<String, String> = body
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().strip_prefix('[')?.split_once(']')?;
            let v = v.split_once("MULTICHOICE(")?.1.split_once(')')?.0;
            Some((k.to_string(), v.to_string()))
        })
        .collect();
    let ids = defines(&src("include/constants/script_menu.h"), "MULTI_");
    let mut ids: Vec<_> = ids
        .into_iter()
        .filter(|(n, _)| n != "MULTI_B_PRESSED")
        .collect();
    ids.sort_by_key(|&(_, v)| v);
    ids.iter()
        .enumerate()
        .map(|(i, (name, v))| {
            assert_eq!(*v as usize, i, "MULTI_* are dense");
            (name.clone(), lists[&entries[name]].clone())
        })
        .collect()
}

/// `STDSTRING_*` -> decomp name of its string.
fn decomp_std_strings() -> Vec<(String, String)> {
    let h = src("src/data/script_menu.h");
    let body = &h[h.find("gStdStrings[] =").unwrap()..];
    let body = &body[..body.find("\n};").unwrap()];
    let entries: BTreeMap<String, String> = body
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().strip_prefix('[')?.split_once(']')?;
            let v = v.trim().strip_prefix('=')?.trim().trim_end_matches(',');
            Some((k.to_string(), v.to_string()))
        })
        .collect();
    let mut ids = defines(&src("include/constants/script_menu.h"), "STDSTRING_");
    ids.sort_by_key(|&(_, v)| v);
    ids.iter()
        .enumerate()
        .map(|(i, (name, v))| {
            assert_eq!(*v as usize, i, "STDSTRING_* are dense");
            (name.clone(), entries[name].clone())
        })
        .collect()
}

/// `MAPSEC_*` -> name, from `region_map_sections.json` (the enum is generated from it).
fn decomp_map_sections() -> Vec<(String, String)> {
    let j: serde_json::Value =
        serde_json::from_str(&src("src/data/region_map/region_map_sections.json")).unwrap();
    j["map_sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let name = m["name"].as_str().unwrap_or("");
            (m["id"].as_str().unwrap().to_string(), name.to_string())
        })
        .collect()
}

/// A decomp string literal in the decoder's spelling: encoded, then decoded with the English
/// charmap (`{CLEAR_TO 0x48}` reads back as `{CLEAR_TO 72}`).
fn en(s: &str) -> String {
    let bytes = text::encode(s).unwrap_or_else(|e| panic!("{s:?}: {e:#}"));
    text::decode_lang(&bytes, text::Lang::En).unwrap().0
}

fn decomp_strings() -> BTreeMap<String, String> {
    string_defs(&[
        &src("src/strings.c"),
        &src("data/text/trick_house_mechadolls.inc"),
    ])
}

fn pocket(v: u8) -> Option<ItemPocket> {
    use ItemPocket::*;
    [
        None,
        Some(Items),
        Some(PokeBalls),
        Some(TmHm),
        Some(Berries),
        Some(KeyItems),
    ][v as usize]
}

#[test]
fn us_items_match_decomp() {
    let (_, t) = us();
    let items = &t.items.items;
    let want = decomp_items();
    assert_eq!(items.len(), want.len());
    assert_eq!(items.len() as u32, tables::item_count());
    for (i, (got, w)) in items.iter().zip(&want).enumerate() {
        assert_eq!(names::ITEMS.value(&got.id), Some(i as u32));
        // `ITEM_TM_<MOVE>` aliases are macro-made and not in `names`.
        assert!(
            names::ITEMS.value(&w.id).is_none_or(|v| v == i as u32),
            "{}",
            w.id
        );
        assert_eq!(got.name, en(&w.name), "{}", w.id);
        assert_eq!(
            (got.price, got.pocket, got.importance),
            (w.price, pocket(w.pocket), w.importance),
            "{}",
            w.id
        );
    }
}

#[test]
fn us_multichoice_matches_decomp() {
    let (_, t) = us();
    let lists = &t.multichoice.lists;
    let strings = decomp_strings();
    let want = decomp_multichoice();
    assert_eq!(lists.len(), want.len());
    assert_eq!(lists.len() as u32, tables::multichoice_count());
    for (id, names) in &want {
        let w: Vec<String> = names.iter().map(|n| en(&strings[n])).collect();
        assert_eq!(lists[id], w, "{id}");
    }
}

#[test]
fn us_std_strings_match_decomp() {
    let (_, t) = us();
    let got = &t.std_strings.strings;
    let strings = decomp_strings();
    let want = decomp_std_strings();
    assert_eq!(got.len(), want.len());
    assert_eq!(got.len() as u32, tables::std_string_count());
    for (id, name) in &want {
        assert_eq!(got[id], en(&strings[name]), "{id}");
    }
}

#[test]
fn us_map_sections_match_decomp() {
    let (_, t) = us();
    let got = &t.map_sections.sections;
    let want = decomp_map_sections();
    assert_eq!(got.len(), want.len());
    assert_eq!(got.len() as u32, tables::map_section_count());
    for (id, name) in &want {
        assert_eq!(got[id], en(name), "{id}");
    }
}

#[test]
fn fr_counts_and_numbers_match_us() {
    let (_, us) = us();
    let (_, fr) = fr();
    assert_eq!(fr.items.items.len(), us.items.items.len());
    for (f, u) in fr.items.items.iter().zip(&us.items.items) {
        assert_eq!(
            (&f.id, f.price, f.pocket, f.importance),
            (&u.id, u.price, u.pocket, u.importance)
        );
        assert!(!f.name.is_empty(), "{}", f.id);
    }
    let shape = |t: &Tables| {
        t.multichoice
            .lists
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(fr), shape(us));
    let keys = |m: &BTreeMap<String, String>| m.keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&fr.std_strings.strings), keys(&us.std_strings.strings));
    assert_eq!(
        keys(&fr.map_sections.sections),
        keys(&us.map_sections.sections)
    );
    // French text differs from English in every table (no FR literals: only inequality).
    let item_names = |t: &Tables| {
        t.items
            .items
            .iter()
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
    };
    assert_ne!(item_names(fr), item_names(us));
    assert_ne!(fr.multichoice, us.multichoice);
    assert_ne!(fr.std_strings, us.std_strings);
    assert_ne!(fr.map_sections, us.map_sections);
}

#[test]
fn strings_round_trip_on_both_roms() {
    for (rom, t) in [fr(), us()] {
        let loc = tables::locate(rom).unwrap();
        for (i, item) in t.items.items.iter().enumerate() {
            let s = text::round_trip_at(rom, loc.items + ITEM_SIZE * i as u32).unwrap();
            assert_eq!(s, item.name);
        }
        let all = tables::multichoice_strings(rom, &loc).unwrap();
        assert_eq!(all.len(), t.multichoice.lists.len());
        for (i, list) in all.iter().enumerate() {
            let id = names::MULTICHOICE.name(i as u32).unwrap();
            for (k, &off) in list.iter().enumerate() {
                assert_eq!(
                    text::round_trip_at(rom, off).unwrap(),
                    t.multichoice.lists[id][k]
                );
            }
        }
        for (v, id) in names::STD_STRINGS.iter() {
            let off = rom.req_ptr(loc.std_strings + 4 * v, "std").unwrap();
            assert_eq!(
                text::round_trip_at(rom, off).unwrap(),
                t.std_strings.strings[id]
            );
        }
        for i in 0..tables::map_section_count() {
            let id = names::MAP_SECTIONS.name(i).unwrap();
            let at = loc.map_sections + tables::MAPSEC_SIZE * i + 4;
            let off = rom.req_ptr(at, "mapsec").unwrap();
            assert_eq!(
                text::round_trip_at(rom, off).unwrap(),
                t.map_sections.sections[id]
            );
        }
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pkr-tables-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn read<T: serde::de::DeserializeOwned>(dir: &Path, rel: &str) -> T {
    let s = std::fs::read_to_string(dir.join(rel)).unwrap();
    ron::from_str(&s).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn export_writes_the_four_tables() {
    let (rom, t) = fr();
    let dir = temp_dir("fr");
    let mut out = Out::new(&dir);
    let mut provenance = BTreeMap::new();
    export_tables(rom, &mut out, &mut provenance).unwrap();

    let written: Vec<&str> = out.files().keys().map(String::as_str).collect();
    assert_eq!(written, {
        let mut f = FILES.to_vec();
        f.sort();
        f
    });
    let minor3: Vec<&str> = MINOR_FILES
        .iter()
        .filter(|&&(m, _)| m == 3)
        .map(|&(_, p)| p)
        .collect();
    assert_eq!(minor3, FILES);
    const { assert!(FORMAT_MINOR >= 3) };

    let items: ItemsFile = read(&dir, FILES[0]);
    let multichoice: MultichoiceFile = read(&dir, FILES[1]);
    let std_strings: StdStringsFile = read(&dir, FILES[2]);
    let map_sections: MapSectionsFile = read(&dir, FILES[3]);
    assert_eq!(items, t.items);
    assert_eq!(multichoice, t.multichoice);
    assert_eq!(std_strings, t.std_strings);
    assert_eq!(map_sections, t.map_sections);

    let loc = tables::locate(rom).unwrap();
    assert_eq!(provenance["tables/items"], Hex(loc.items));
    assert_eq!(provenance["tables/multichoice"], Hex(loc.multichoice));
    assert_eq!(provenance["tables/std_strings"], Hex(loc.std_strings));
    assert_eq!(provenance["tables/map_sections"], Hex(loc.map_sections));
    let _ = std::fs::remove_dir_all(&dir);
}
