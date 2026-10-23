//! Constant names in the chapters: scanning, family shorthands (`FLAG_A_1` / `_2`, `X_1…4`),
//! placeholders (`FLAG_SIL_FRAGMENT_<n>`) and map file names.

use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// The constant prefixes `refs.ron` records.
pub const PREFIXES: &[&str] = &[
    "FLAG_",
    "VAR_",
    "MAP_",
    "TRAINER_",
    "ITEM_",
    "SPECIES_",
    "MOVE_",
    "MUS_",
    "SE_",
    "WEATHER_",
    "HEAL_LOCATION_",
];

/// `TRAINER_` prefixes that are not trainers.
const NOT_TRAINERS: &[&str] = &["TRAINER_CLASS_", "TRAINER_PIC_", "TRAINER_TYPE_"];

/// What one written constant word is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word {
    /// Concrete names (one, or several for a range or a slash family).
    Names(Vec<String>),
    /// A documentation wildcard (`FLAG_SIL_ITEM_*`, `MUS_*`): not a reference.
    Wildcard(String),
    /// A placeholder pattern (`FLAG_SIL_FRAGMENT_<n>`, `MAP_ROUTE_S0x_ANTENNE`).
    Dynamic(String),
}

static WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Z][A-Z0-9]*_(?:[A-Za-z0-9_*…]|<[^>\s]*>|\[[^\]\s]*\])*(?:\.\.[A-Z]*[0-9]+)?")
        .unwrap()
});
static RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*_)([A-Z]*)([0-9]+)(?:…|\.\.)([A-Z]*)([0-9]+)$").unwrap());
/// A range after a constant: `` `X_01` … `_08` ``, `X_01 à _03`.
static RANGE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^`?\s*(?:…|\.\.\.|à)\s*`?(_[A-Z0-9][A-Z0-9_]*)`?").unwrap());
/// A shorthand after a constant: `` / `_X` ``, `, _X`, `(+ _X)`, possibly after a parenthetical.
static SLASH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^`?\s*(?:\([^)_]*\))?\s*(?:/|,|\(\+)\s*`?(_[A-Z0-9][A-Z0-9_]*)`?\)?").unwrap()
});

pub fn has_prefix(w: &str) -> bool {
    PREFIXES.iter().any(|p| w.starts_with(p)) && !NOT_TRAINERS.iter().any(|p| w.starts_with(p))
}

fn is_placeholder_segment(s: &str) -> bool {
    matches!(s, "N" | "NN" | "XX" | "XXX" | "")
}

/// Classifies one word that starts with a constant prefix.
pub fn classify(w: &str) -> Word {
    if w.contains('*') {
        return Word::Wildcard(w.to_string());
    }
    if let Some(c) = RANGE.captures(w)
        && c[2] == c[4]
    {
        let (a, b): (u32, u32) = (c[3].parse().unwrap(), c[5].parse().unwrap());
        let width = c[3].len();
        if a < b && b - a < 64 {
            return Word::Names(
                (a..=b)
                    .map(|n| format!("{}{}{:0width$}", &c[1], &c[2], n))
                    .collect(),
            );
        }
    }
    let last = w.rsplit('_').next().unwrap_or("");
    if w.contains(['<', '>', '[', '…', '.'])
        || w.chars().any(|c| c.is_ascii_lowercase())
        || is_placeholder_segment(last)
    {
        return Word::Dynamic(w.to_string());
    }
    Word::Names(vec![w.to_string()])
}

/// Every constant word of `line` with its classification, slash shorthands expanded
/// (`` `FLAG_A_JOINT` / `_MANO` `` gives `FLAG_A_JOINT` and `FLAG_A_MANO`). `maps` are the known
/// map constants, for the sibling rule of [`expand_suffix`].
pub fn scan_line(line: &str, maps: &BTreeSet<String>) -> Vec<Word> {
    let mut out = Vec::new();
    for m in WORD.find_iter(line) {
        let w = m.as_str();
        // A word glued to a longer identifier (`gMAP_X`) is not a constant.
        if line[..m.start()]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        // A macro call (`TRAINER_FLAG(id)`) is not a constant.
        if !has_prefix(w) || line[m.end()..].starts_with('(') {
            continue;
        }
        let word = classify(w);
        out.push(word.clone());
        if let Word::Names(names) = &word {
            let base = names.last().unwrap().clone();
            let mut rest = &line[m.end()..];
            if let Some(c) = RANGE_SUFFIX.captures(rest) {
                let suffix = c.get(1).unwrap().as_str();
                match classify(&format!("{base}…{}", &suffix[1..])) {
                    Word::Names(v) if v.len() > 1 => out.push(Word::Names(v)),
                    _ => out.push(Word::Dynamic(format!("{base} … {suffix}"))),
                }
                rest = &rest[c.get(0).unwrap().end()..];
            }
            while let Some(c) = SLASH.captures(rest) {
                let suffix = c.get(1).unwrap().as_str();
                out.push(Word::Names(vec![expand_suffix(&base, suffix, maps)]));
                rest = &rest[c.get(0).unwrap().end()..];
            }
        }
    }
    out
}

fn floor_like(s: &str) -> bool {
    let t = s.strip_prefix('B').unwrap_or(s);
    t.len() >= 2 && t.ends_with('F') && t[..t.len() - 1].chars().all(|c| c.is_ascii_digit())
}

/// (a floor word, a number with an optional letter prefix such as `2` or `P1`).
fn shape(s: &str) -> (bool, bool) {
    let numeric = s.chars().any(|c| c.is_ascii_digit())
        && s.trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .chars()
            .all(|c| c.is_ascii_digit());
    (floor_like(s), numeric)
}

/// Expands a `/ _SUFFIX` shorthand against `base`:
/// 1. a one-word suffix shaped like the base's last word (a number, a floor, `P1`) replaces it
///    (`X_1F` / `_2F`, `FLAG_A_1` / `_2`); a floor after a base without one is appended
///    (`MAP_A_TOUR, _2F` is `MAP_A_TOUR_2F`);
/// 2. a suffix whose first words repeat the base's last words is spliced there
///    (`MAP_A_TOUR` / `_TOUR_TOP` is `MAP_A_TOUR_TOP`);
/// 3. for a map with a known parent map, the suffix replaces what follows the parent
///    (`MAP_TOWN_POKEMON_CENTER` / `_MART` is `MAP_TOWN_MART`);
/// 4. otherwise the suffix replaces as many words as it has (`FLAG_A_JOINT` / `_MANO`).
pub fn expand_suffix(base: &str, suffix: &str, maps: &BTreeSet<String>) -> String {
    let b: Vec<&str> = base.split('_').collect();
    let s: Vec<&str> = suffix.trim_start_matches('_').split('_').collect();
    if s.len() == 1 && b.len() > 2 {
        let (bf, bn) = shape(b[b.len() - 1]);
        let (sf, sn) = shape(s[0]);
        if (bf && sf) || (bn && sn && !bf && !sf) {
            return format!("{}_{}", b[..b.len() - 1].join("_"), s[0]);
        }
        if sf && !bf {
            return format!("{base}_{}", s[0]);
        }
    }
    for k in (1..s.len().min(b.len() - 1) + 1).rev() {
        if b[b.len() - k..] == s[..k] {
            return format!("{}_{}", b[..b.len() - k].join("_"), s.join("_"));
        }
    }
    if base.starts_with("MAP_")
        && let Some(parent) = parent_map(base, maps)
    {
        return format!("{parent}_{}", s.join("_"));
    }
    let keep = b.len().saturating_sub(s.len()).max(2);
    format!("{}_{}", b[..keep].join("_"), s.join("_"))
}

/// The longest proper prefix of `constant` (at a word boundary) in `maps`.
pub fn parent_map<'a>(constant: &str, maps: &'a BTreeSet<String>) -> Option<&'a String> {
    maps.iter()
        .filter(|m| {
            constant.len() > m.len() + 1
                && constant.starts_with(m.as_str())
                && constant.as_bytes()[m.len()] == b'_'
        })
        .max_by_key(|m| m.len())
}

/// The map file name of a map constant (schema docs, "Map names").
pub fn map_name(constant: &str, maps: &BTreeSet<String>) -> String {
    match parent_map(constant, maps) {
        Some(p) => format!("{}_{}", map_name(p, maps), camel(&constant[p.len() + 1..])),
        None => camel(constant.strip_prefix("MAP_").unwrap_or(constant)),
    }
}

fn camel(words: &str) -> String {
    let mut out = String::new();
    for (i, w) in words.split('_').filter(|w| !w.is_empty()).enumerate() {
        if floor_like(w) {
            if i > 0 {
                out.push('_');
            }
            out.push_str(w);
            continue;
        }
        let mut cs = w.chars();
        if let Some(c) = cs.next() {
            out.push(c.to_ascii_uppercase());
            out.extend(cs.map(|c| c.to_ascii_lowercase()));
        }
    }
    out
}

/// A regex for a placeholder pattern: each `<..>`, `[..]`, `…` or placeholder word (`N`, `NN`,
/// `X`, `XX`), and in a constant (`constant`) each lowercase run, stands for one or more letters
/// or digits.
pub fn pattern_regex(p: &str, constant: bool) -> Option<Regex> {
    static PIECE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<[^>]*>|\[[^\]]*\]|…|\.\.\.|(?:^|_)(?:NN|N|XXX|XX|X)(?:_|$)").unwrap()
    });
    static PIECE_CONST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<[^>]*>|\[[^\]]*\]|…|\.\.\.|[a-z]+[a-z0-9]*|(?:^|_)(?:NN|N|XXX|XX|X)(?:_|$)")
            .unwrap()
    });
    let piece: &Regex = if constant { &PIECE_CONST } else { &PIECE };
    let mut re = String::from("^");
    let mut last = 0;
    for m in piece.find_iter(p) {
        re.push_str(&regex::escape(&p[last..m.start()]));
        let s = m.as_str();
        let lead = if s.starts_with('_') { "_" } else { "" };
        let trail = if s.len() > 1 && s.ends_with('_') {
            "_"
        } else {
            ""
        };
        re.push_str(lead);
        re.push_str("[A-Za-z0-9]+");
        re.push_str(trail);
        last = m.end();
    }
    if last == 0 {
        return None;
    }
    re.push_str(&regex::escape(&p[last..]));
    re.push('$');
    Regex::new(&re).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maps(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn words() {
        let m = maps(&["MAP_TOWN"]);
        let w = scan_line(
            "`FLAG_SIL_A_JOINT` / `_MANO`, `_FUSE` and MAP_TOWN_X_1F (1F), _2F",
            &m,
        );
        assert_eq!(
            scan_line("`FLAG_SIL_G_01` … `_03`, FLAG_SIL_H_A … _B", &m),
            vec![
                Word::Names(vec!["FLAG_SIL_G_01".into()]),
                Word::Names(vec![
                    "FLAG_SIL_G_01".into(),
                    "FLAG_SIL_G_02".into(),
                    "FLAG_SIL_G_03".into()
                ]),
                Word::Names(vec!["FLAG_SIL_H_A".into()]),
                Word::Dynamic("FLAG_SIL_H_A … _B".into()),
            ]
        );
        assert_eq!(
            scan_line("flag(TRAINER_FLAG(TRAINER_SIL_A))", &m),
            vec![Word::Names(vec!["TRAINER_SIL_A".into()])]
        );
        assert_eq!(
            classify("ITEM_SIL_STONE_X"),
            Word::Names(vec!["ITEM_SIL_STONE_X".into()])
        );
        assert_eq!(
            classify("FLAG_SIL_HIDE_<ESPÈCE>"),
            Word::Dynamic("FLAG_SIL_HIDE_<ESPÈCE>".into())
        );
        assert_eq!(
            w,
            vec![
                Word::Names(vec!["FLAG_SIL_A_JOINT".into()]),
                Word::Names(vec!["FLAG_SIL_A_MANO".into()]),
                Word::Names(vec!["FLAG_SIL_A_FUSE".into()]),
                Word::Names(vec!["MAP_TOWN_X_1F".into()]),
                Word::Names(vec!["MAP_TOWN_X_2F".into()]),
            ]
        );
        assert_eq!(
            classify("FLAG_SIL_W_1…3"),
            Word::Names(vec![
                "FLAG_SIL_W_1".into(),
                "FLAG_SIL_W_2".into(),
                "FLAG_SIL_W_3".into()
            ])
        );
        assert_eq!(
            classify("FLAG_SIL_G_P1…P2"),
            Word::Names(vec!["FLAG_SIL_G_P1".into(), "FLAG_SIL_G_P2".into()])
        );
        assert_eq!(classify("MUS_*"), Word::Wildcard("MUS_*".into()));
        for d in [
            "FLAG_SIL_F_<n>",
            "MAP_ROUTE_S0x_A",
            "VAR_SIL_P_N",
            "FLAG_SIL_F_",
        ] {
            assert_eq!(classify(d), Word::Dynamic(d.into()));
        }
        assert!(scan_line("gMAP_X TRAINER_CLASS_Y", &m).is_empty());
    }

    #[test]
    fn suffixes() {
        let m = maps(&["MAP_TOWN", "MAP_TOWN_TOUR"]);
        assert_eq!(
            expand_suffix("MAP_TOWN_TOUR", "_TOUR_TOP", &m),
            "MAP_TOWN_TOUR_TOP"
        );
        assert_eq!(
            expand_suffix("MAP_TOWN_POKEMON_CENTER", "_MART", &m),
            "MAP_TOWN_MART"
        );
        assert_eq!(
            expand_suffix("MAP_TOWN_HOUSE_1F", "_2F", &m),
            "MAP_TOWN_HOUSE_2F"
        );
        assert_eq!(expand_suffix("MAP_TOWN_A", "_A_2F", &m), "MAP_TOWN_A_2F");
        assert_eq!(
            expand_suffix("MAP_TOWN_TOUR_TOP", "_2F", &m),
            "MAP_TOWN_TOUR_TOP_2F"
        );
        assert_eq!(
            expand_suffix("FLAG_SIL_A_JOINT", "_MANO", &m),
            "FLAG_SIL_A_MANO"
        );
        assert_eq!(expand_suffix("FLAG_SIL_A_1", "_2", &m), "FLAG_SIL_A_2");
    }

    #[test]
    fn names() {
        let m = maps(&["MAP_TOWN", "MAP_ROUTE_S01", "MAP_TOWN_GYM"]);
        assert_eq!(
            map_name("MAP_TOWN_PLAYER_HOUSE_1F", &m),
            "Town_PlayerHouse_1F"
        );
        assert_eq!(map_name("MAP_ROUTE_S01", &m), "RouteS01");
        assert_eq!(map_name("MAP_TOWN_GYM_B1F", &m), "Town_Gym_B1F");
        assert_eq!(map_name("MAP_TOWER_1F", &m), "Tower_1F");
        assert_eq!(map_name("MAP_ZONE_404", &m), "Zone404");
    }

    #[test]
    fn patterns() {
        let r = pattern_regex("Gym2_<P>_Intro", false).unwrap();
        assert!(r.is_match("Gym2_Pawn_Intro") && !r.is_match("Gym2_A_B_Intro"));
        let r = pattern_regex("Chat_NN", false).unwrap();
        assert!(r.is_match("Chat_01"));
        let r = pattern_regex("A_T[choice]", false).unwrap();
        assert!(r.is_match("A_TMage"));
        assert!(pattern_regex("PLAIN_NAME_X1", false).is_none());
        assert!(
            pattern_regex("MAP_R_S0x_A", true)
                .unwrap()
                .is_match("MAP_R_S02_A")
        );
    }
}
