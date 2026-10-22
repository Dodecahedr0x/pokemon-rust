//! Gen3 constant names -> engine values, as Rust source snippets. Shared by the build script and
//! `tests/build_classify.rs`. Split by owner (Milestone 8): [`field`] (behaviors, movement types,
//! graphics; Task E) and [`battle`] (abilities, move effects, battle strings; Task G); this file
//! keeps the shared helpers.

mod battle;
mod field;

pub use battle::*;
pub use field::*;
use std::collections::BTreeSet;

/// Prefixes of constants whose unnamed numbers are written `<PREFIX>0x<HEX>`.
pub const HEX_PREFIXES: &[&str] = &[
    "FLAG_",
    "VAR_",
    "SPECIAL_",
    "ITEM_",
    "OBJ_EVENT_GFX_",
    "STD_",
];

/// Every `<PREFIX>0x<HEX>` identifier (`FLAG_0x1AB`) in `src`.
pub fn hex_names(src: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = BTreeSet::new();
    for p in HEX_PREFIXES {
        let pat = format!("{p}0x");
        let mut from = 0;
        while let Some(i) = src[from..].find(&pat).map(|i| i + from) {
            from = i + pat.len();
            if i > 0 && ident(b[i - 1]) {
                continue;
            }
            let end = from + b[from..].iter().take_while(|c| ident(**c)).count();
            if end > from && b[from..end].iter().all(u8::is_ascii_hexdigit) {
                out.insert(src[i..end].to_string());
            }
        }
    }
    out
}

/// Every whole identifier of `src` that starts with `prefix` (`TRAINER_MAY_ROUTE_103_MUDKIP`).
pub fn idents_with_prefix(src: &str, prefix: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = BTreeSet::new();
    let mut from = 0;
    while let Some(i) = src[from..].find(prefix).map(|i| i + from) {
        let end = i + b[i..].iter().take_while(|c| ident(**c)).count();
        from = end.max(i + 1);
        if i > 0 && ident(b[i - 1]) {
            continue;
        }
        out.insert(src[i..end].to_string());
    }
    out
}

/// `src` without its `//` line comments (generated scripts have no string literal holding
/// `//`), so names quoted in comments are not taken for references.
pub fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| l.split_once("//").map_or(l, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `TYPE_*` name -> `engine::mon::Type` expression.
pub fn type_variant(name: &str) -> Option<&'static str> {
    Some(match name {
        "TYPE_NORMAL" => "Type::Normal",
        "TYPE_FIGHTING" => "Type::Fighting",
        "TYPE_FLYING" => "Type::Flying",
        "TYPE_POISON" => "Type::Poison",
        "TYPE_GROUND" => "Type::Ground",
        "TYPE_ROCK" => "Type::Rock",
        "TYPE_BUG" => "Type::Bug",
        "TYPE_GHOST" => "Type::Ghost",
        "TYPE_STEEL" => "Type::Steel",
        "TYPE_MYSTERY" => "Type::Mystery",
        "TYPE_FIRE" => "Type::Fire",
        "TYPE_WATER" => "Type::Water",
        "TYPE_GRASS" => "Type::Grass",
        "TYPE_ELECTRIC" => "Type::Electric",
        "TYPE_PSYCHIC" => "Type::Psychic",
        "TYPE_ICE" => "Type::Ice",
        "TYPE_DRAGON" => "Type::Dragon",
        "TYPE_DARK" => "Type::Dark",
        _ => return None,
    })
}
