//! The names the authored maps (`games/silice/maps/*.ron`, the studio's `GameMap`) and, later,
//! the scene scripts (`crates/content/src/silice/scripts/*.rs`) use. Read textually, so `check`
//! needs no copy of the studio's schema: the maps are written by `extract::export::to_ron`, one
//! field per line.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

/// One map file's references.
#[derive(Clone, Debug, Default)]
pub struct MapRefs {
    /// The file stem (the map name).
    pub name: String,
    /// `(field, value)`: object and hidden-item `flag`, coord and frame-script `var`, header
    /// `music` and `weather`, warp `dest_map` and connection `map` (map names, not constants).
    pub fields: Vec<(String, String)>,
    /// Hidden items: `(item, flag)`.
    pub hidden: Vec<(String, String)>,
}

impl MapRefs {
    /// Values of the fields named `field`.
    pub fn values<'a>(&'a self, field: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.fields
            .iter()
            .filter(move |(f, _)| f == field)
            .map(|(_, v)| v.as_str())
    }
}

static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(flag|var|music|weather|dest_map|map): (?:Some\()?"([^"]+)""#).unwrap()
});
static HIDDEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"HiddenItem\(\s*item: "([^"]+)",\s*flag: "([^"]+)""#).unwrap());

pub fn scan_map(name: &str, text: &str) -> MapRefs {
    MapRefs {
        name: name.to_string(),
        fields: FIELD
            .captures_iter(text)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect(),
        hidden: HIDDEN
            .captures_iter(text)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect(),
    }
}

/// Every `*.ron` under `dir`, by name; empty when the directory does not exist.
pub fn scan_dir(dir: &Path) -> Result<Vec<MapRefs>> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut paths: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let text =
                std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            Ok(scan_map(&name, &text))
        })
        .collect()
}

/// Hook for the scene scripts once they are Rust (`crates/content/src/silice/scripts/*.rs`):
/// every constant-looking word, by file stem. Empty while the directory does not exist.
pub fn scan_scripts(dir: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    static WORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(?:FLAG|VAR|MAP|ITEM|TRAINER|SPECIES|MOVE|MUS|SE|WEATHER|HEAL_LOCATION)_[A-Z0-9_]*[A-Z0-9]\b")
            .unwrap()
    });
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    let mut paths: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    paths.sort();
    for p in paths {
        let text =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let mut words: Vec<String> = WORD
            .find_iter(&text)
            .map(|m| m.as_str().to_string())
            .collect();
        words.sort();
        words.dedup();
        out.insert(stem, words);
    }
    Ok(out)
}

/// The maps the scene scripts claim, by map file stem: each script file's `pub const MAPS:
/// &[&str] = &["A", …];` (the content build's per-map strictness, `build/silice/registry.rs`,
/// which also rejects any other form). A claimed map's scenes are written. Empty while the
/// directory does not exist.
pub fn scan_claims(dir: &Path) -> Result<BTreeSet<String>> {
    static MAPS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"pub\s+const\s+MAPS\s*:[^=]*=\s*&?\[([^\]]*)\]").unwrap());
    static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)""#).unwrap());
    let mut out = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for p in rd.flatten().map(|e| e.path()) {
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let text =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        for c in MAPS.captures_iter(&text) {
            out.extend(NAME.captures_iter(&c[1]).map(|n| n[1].to_string()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_the_claimed_maps() {
        let d = std::env::temp_dir().join(format!("pkr_scan_claims_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("ch00_a.rs"),
            "pub const MAPS: &[&str] = &[\n    \"A\",\n    \"B_1F\",\n];\npub async fn S(ctx: Ctx) {}\n",
        )
        .unwrap();
        std::fs::write(d.join("ch01_b.rs"), "pub const MAPS: &[&str] = &[\"C\"];\n").unwrap();
        std::fs::write(d.join("ch02_c.rs"), "pub async fn T(ctx: Ctx) {}\n").unwrap();
        std::fs::write(d.join("notes.txt"), "pub const MAPS: &[&str] = &[\"D\"];\n").unwrap();
        let c = scan_claims(&d).unwrap();
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(c.into_iter().collect::<Vec<_>>(), ["A", "B_1F", "C"]);
        assert!(scan_claims(Path::new("/nonexistent")).unwrap().is_empty());
    }

    #[test]
    fn scans_fields() {
        let t = "(\n    header: (\n        music: \"MUS_A\",\n        weather: \"WEATHER_NONE\",\n    ),\n    objects: [\n        (\n            flag: Some(\"FLAG_SIL_X\"),\n        ),\n        (\n            flag: None,\n        ),\n    ],\n    warps: [\n        (\n            dest_map: \"Town\",\n        ),\n    ],\n    coords: [\n        (\n            var: Some(\"VAR_SIL_Y\"),\n        ),\n    ],\n    bgs: [\n        (\n            kind: HiddenItem(\n                item: \"ITEM_A\",\n                flag: \"FLAG_SIL_H\",\n            ),\n        ),\n    ],\n    connections: [\n        (\n            map: \"Route\",\n        ),\n    ],\n)\n";
        let m = scan_map("M", t);
        let f: Vec<(&str, &str)> = m
            .fields
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        assert_eq!(
            f,
            vec![
                ("music", "MUS_A"),
                ("weather", "WEATHER_NONE"),
                ("flag", "FLAG_SIL_X"),
                ("dest_map", "Town"),
                ("var", "VAR_SIL_Y"),
                ("flag", "FLAG_SIL_H"),
                ("map", "Route"),
            ]
        );
        assert_eq!(
            m.hidden,
            vec![("ITEM_A".to_string(), "FLAG_SIL_H".to_string())]
        );
    }
}
