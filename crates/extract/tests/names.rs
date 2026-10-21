//! Guards `src/names/generated.rs` against drift from the decomp: regenerates it from the decomp
//! sources and compares. Run with `PKR_BLESS=1` to rewrite the file instead.
use extract::{fixtures, names};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

fn read(d: &Path, rel: &str) -> String {
    std::fs::read_to_string(d.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Strips `//` and single-line `/* */` comments.
fn strip_comments(line: &str) -> String {
    let mut s = line.split("//").next().unwrap().to_string();
    while let (Some(a), Some(b)) = (s.find("/*"), s.find("*/")) {
        if b < a {
            break;
        }
        s.replace_range(a..b + 2, " ");
    }
    s
}

/// Object-like `#define NAME expr` lines of a header, in source order.
fn defines(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = strip_comments(line);
        let Some(rest) = line.trim_start().strip_prefix("#define") else {
            continue;
        };
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (name, expr) = rest.split_at(end);
        if name.is_empty() || expr.starts_with('(') {
            continue; // function-like macro
        }
        out.push((name.to_string(), expr.trim().to_string()));
    }
    out
}

/// Every object-like define of `include/constants/*.h`, first definition wins.
fn env(d: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let mut files: Vec<_> = std::fs::read_dir(d.join("include/constants"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "h"))
        .collect();
    files.sort();
    for f in files {
        for (n, e) in defines(&std::fs::read_to_string(f).unwrap()) {
            env.entry(n).or_insert(e);
        }
    }
    env
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(i64),
    Id(String),
    Op(String),
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() {
            let start = i;
            while i < c.len() && c[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let lit: String = c[start..i].iter().collect();
            let lit = lit.trim_end_matches(['u', 'U', 'l', 'L']);
            let v = match lit.strip_prefix("0x").or_else(|| lit.strip_prefix("0X")) {
                Some(h) => i64::from_str_radix(h, 16).ok()?,
                None => lit.parse().ok()?,
            };
            out.push(Tok::Num(v));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(c[start..i].iter().collect()));
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if two == "<<" || two == ">>" {
                out.push(Tok::Op(two));
                i += 2;
            } else if "+-*/|&()~".contains(ch) {
                out.push(Tok::Op(ch.to_string()));
                i += 1;
            } else {
                return None;
            }
        }
    }
    Some(out)
}

struct Eval<'a> {
    env: &'a HashMap<String, String>,
    depth: u32,
}

impl Eval<'_> {
    fn name(&mut self, n: &str) -> Option<i64> {
        let expr = self.env.get(n)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let v = self.expr(expr);
        self.depth -= 1;
        v
    }

    fn expr(&mut self, s: &str) -> Option<i64> {
        let toks = tokens(s)?;
        if toks.is_empty() {
            return None;
        }
        let mut pos = 0;
        let v = self.binary(&toks, &mut pos, 0)?;
        (pos == toks.len()).then_some(v)
    }

    fn prec(op: &str) -> Option<u8> {
        Some(match op {
            "|" => 1,
            "&" => 2,
            "<<" | ">>" => 3,
            "+" | "-" => 4,
            "*" | "/" => 5,
            _ => return None,
        })
    }

    fn binary(&mut self, t: &[Tok], pos: &mut usize, min: u8) -> Option<i64> {
        let mut lhs = self.unary(t, pos)?;
        while let Some(Tok::Op(op)) = t.get(*pos) {
            let Some(p) = Self::prec(op) else { break };
            if p < min {
                break;
            }
            let op = op.clone();
            *pos += 1;
            let rhs = self.binary(t, pos, p + 1)?;
            lhs = match op.as_str() {
                "|" => lhs | rhs,
                "&" => lhs & rhs,
                "<<" => lhs << rhs,
                ">>" => lhs >> rhs,
                "+" => lhs + rhs,
                "-" => lhs - rhs,
                "*" => lhs * rhs,
                "/" => lhs.checked_div(rhs)?,
                _ => unreachable!(),
            };
        }
        Some(lhs)
    }

    fn unary(&mut self, t: &[Tok], pos: &mut usize) -> Option<i64> {
        let tok = t.get(*pos)?.clone();
        *pos += 1;
        match tok {
            Tok::Num(v) => Some(v),
            Tok::Id(n) => self.name(&n),
            Tok::Op(o) if o == "-" => Some(-self.unary(t, pos)?),
            Tok::Op(o) if o == "~" => Some(!self.unary(t, pos)?),
            Tok::Op(o) if o == "(" => {
                let v = self.binary(t, pos, 0)?;
                (t.get(*pos) == Some(&Tok::Op(")".into()))).then_some(())?;
                *pos += 1;
                Some(v)
            }
            _ => None,
        }
    }
}

/// `(value, name)` sorted by value; within a value the primary name (the first in source order
/// that no other define of the table uses as an arithmetic base) comes first, then the others in
/// source order.
fn table_entries(defs: &[(String, i64, String)]) -> Vec<(i64, String)> {
    let is_base = |name: &str| {
        defs.iter().any(|(_, _, e)| {
            e.contains(|c: char| "+-*/<>|&".contains(c))
                && tokens(e)
                    .unwrap_or_default()
                    .contains(&Tok::Id(name.to_string()))
        })
    };
    let mut order: Vec<(i64, bool, usize, String)> = defs
        .iter()
        .enumerate()
        .map(|(i, (n, v, _))| (*v, is_base(n), i, n.clone()))
        .collect();
    order.sort();
    order.into_iter().map(|(v, _, _, n)| (v, n)).collect()
}

/// Defines of `file` whose name starts with `prefix`, evaluated.
fn prefixed(
    d: &Path,
    env: &HashMap<String, String>,
    file: &str,
    prefix: &str,
) -> Vec<(i64, String)> {
    let mut defs = Vec::new();
    for (n, e) in defines(&read(d, file)) {
        if !n.starts_with(prefix) {
            continue;
        }
        let mut ev = Eval { env, depth: 0 };
        if let Some(v) = ev.expr(&e) {
            defs.push((n, v, e));
        }
    }
    assert!(!defs.is_empty(), "{file}: no {prefix} defines");
    table_entries(&defs)
}

/// `NAME = value` assembler symbols of an `.inc` file with the given prefix.
fn asm_symbols(src: &str, prefix: &str) -> Vec<(i64, String)> {
    let mut defs = Vec::new();
    for line in src.lines() {
        let line = line.split('@').next().unwrap().trim();
        let Some((n, v)) = line.split_once('=') else {
            continue;
        };
        let n = n.trim();
        if n.starts_with(prefix) && !n.contains(' ') {
            let v = v.trim();
            defs.push((n.to_string(), v.parse().unwrap(), v.to_string()));
        }
    }
    table_entries(&defs)
}

fn metatile_behaviors(src: &str) -> Vec<(i64, String)> {
    let body = &src[src.find("enum {").unwrap() + 6..];
    let body = &body[..body.find("};").unwrap()];
    let mut out = Vec::new();
    let mut next = 0;
    for item in body.split(',') {
        let item = strip_comments(item);
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, v) = match item.split_once('=') {
            Some((n, v)) => (n.trim(), v.trim().parse().unwrap()),
            None => (item, next),
        };
        next = v + 1;
        if name.starts_with("MB_") {
            out.push((v, name.to_string()));
        }
    }
    out
}

fn specials(src: &str) -> Vec<(i64, String)> {
    src.lines()
        .filter_map(|l| l.trim().strip_prefix("def_special "))
        .enumerate()
        .map(|(i, n)| (i as i64, format!("SPECIAL_{}", n.trim())))
        .collect()
}

/// `gStdScripts` entries, named by their `@ STD_*`/`@ MSGBOX_*` comment.
fn std_scripts(src: &str) -> Vec<(i64, String)> {
    let body = &src[src.find("gStdScripts::").unwrap()..];
    let body = &body[..body.find("gStdScripts_End::").unwrap()];
    body.lines()
        .filter(|l| l.trim().starts_with(".4byte"))
        .enumerate()
        .map(|(i, l)| (i as i64, l.split('@').nth(1).unwrap().trim().to_string()))
        .collect()
}

/// `gTileset_<name>` headers in `headers.h` order, with their byte offset from
/// `gTileset_General`: each `struct Tileset` is 0x18 bytes, and the two `Tileset` pointers
/// defined among them (`gTilesetPointer_*`) take 4 bytes each.
fn tilesets(src: &str) -> Vec<(i64, String, u32)> {
    let mut out = Vec::new();
    let mut at = 0;
    for l in src.lines() {
        if let Some(n) = l.strip_prefix("const struct Tileset gTileset_") {
            out.push((
                out.len() as i64,
                n.trim_end_matches([' ', '=']).to_string(),
                at,
            ));
            at += 0x18;
        } else if l.starts_with("const struct Tileset *const ") {
            at += 4;
        } else {
            assert!(!l.starts_with("const ") && !l.starts_with("static "), "{l}");
        }
    }
    out
}

fn maps(d: &Path) -> Vec<(u8, u8, String, String)> {
    let groups: serde_json::Value =
        serde_json::from_str(&read(d, "data/maps/map_groups.json")).unwrap();
    let mut out = Vec::new();
    for (g, group) in groups["group_order"].as_array().unwrap().iter().enumerate() {
        for (n, dir) in groups[group.as_str().unwrap()]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let dir = dir.as_str().unwrap();
            let map: serde_json::Value =
                serde_json::from_str(&read(d, &format!("data/maps/{dir}/map.json"))).unwrap();
            out.push((
                g as u8,
                n as u8,
                dir.to_string(),
                map["id"].as_str().unwrap().to_string(),
            ));
        }
    }
    out
}

fn render_table(out: &mut String, name: &str, doc: &str, prefix: &str, entries: &[(i64, String)]) {
    writeln!(out, "/// {doc}").unwrap();
    writeln!(out, "pub static {name}: Table = Table {{").unwrap();
    writeln!(out, "    prefix: {prefix:?},").unwrap();
    writeln!(out, "    entries: &[").unwrap();
    for (v, n) in entries {
        assert!(*v >= 0 && *v <= u32::MAX as i64, "{n} = {v}");
        writeln!(out, "        ({v:#x}, {n:?}),").unwrap();
    }
    writeln!(out, "    ],").unwrap();
    writeln!(out, "}};\n").unwrap();
}

fn generate(d: &Path) -> String {
    let env = env(d);
    let mut out = String::from(
        "//! Generated from the pokeemerald decomp by `tests/names.rs`; do not edit by hand.\n\
         //! Regenerate with `PKR_BLESS=1 cargo test -p extract --test names`.\n\n\
         use super::{MapName, Table};\n\n",
    );
    out.push_str("/// `(group, num)` -> map directory and `MAP_*` constant, from `data/maps/map_groups.json` and each `map.json`.\n");
    out.push_str("pub static MAPS: &[MapName] = &[\n");
    for (g, n, dir, constant) in maps(d) {
        writeln!(
            out,
            "    MapName {{ group: {g}, num: {n}, dir: {dir:?}, constant: {constant:?} }},"
        )
        .unwrap();
    }
    out.push_str("];\n\n");
    let ev = "include/constants/event_object_movement.h";
    let tileset_list = tilesets(&read(d, "src/data/tilesets/headers.h"));
    let tables: Vec<(&str, &str, &str, Entries)> = vec![
        (
            "FLAGS",
            "`include/constants/flags.h`.",
            "FLAG_",
            prefixed(d, &env, "include/constants/flags.h", "FLAG_"),
        ),
        (
            "VARS",
            "`include/constants/vars.h`, special vars 0x8000+ included.",
            "VAR_",
            prefixed(d, &env, "include/constants/vars.h", "VAR_"),
        ),
        (
            "OBJ_EVENT_GFX",
            "`include/constants/event_objects.h`.",
            "OBJ_EVENT_GFX_",
            prefixed(
                d,
                &env,
                "include/constants/event_objects.h",
                "OBJ_EVENT_GFX_",
            ),
        ),
        (
            "MOVEMENT_TYPES",
            "`MOVEMENT_TYPE_*` from `include/constants/event_object_movement.h`.",
            "MOVEMENT_TYPE_",
            prefixed(d, &env, ev, "MOVEMENT_TYPE_"),
        ),
        (
            "MOVEMENT_ACTIONS",
            "`MOVEMENT_ACTION_*` (movement script bytes) from `include/constants/event_object_movement.h`.",
            "MOVEMENT_ACTION_",
            prefixed(d, &env, ev, "MOVEMENT_ACTION_"),
        ),
        (
            "TRAINER_TYPES",
            "`include/constants/trainer_types.h`.",
            "TRAINER_TYPE_",
            prefixed(
                d,
                &env,
                "include/constants/trainer_types.h",
                "TRAINER_TYPE_",
            ),
        ),
        (
            "ITEMS",
            "`include/constants/items.h`.",
            "ITEM_",
            prefixed(d, &env, "include/constants/items.h", "ITEM_"),
        ),
        (
            "METATILE_BEHAVIORS",
            "`MB_*` from the enum in `include/constants/metatile_behaviors.h`.",
            "MB_",
            metatile_behaviors(&read(d, "include/constants/metatile_behaviors.h")),
        ),
        (
            "SPECIALS",
            "`special` index -> `SPECIAL_<fn>`, from `data/specials.inc`.",
            "SPECIAL_",
            specials(&read(d, "data/specials.inc")),
        ),
        (
            "STD_SCRIPTS",
            "`callstd` index -> name (`STD_*` or `MSGBOX_*`), from `gStdScripts` in `data/event_scripts.s`.",
            "STD_",
            std_scripts(&read(d, "data/event_scripts.s")),
        ),
        (
            "MSGBOX_TYPES",
            "`MSGBOX_*` from `asm/macros/event.inc`.",
            "MSGBOX_",
            asm_symbols(&read(d, "asm/macros/event.inc"), "MSGBOX_"),
        ),
        (
            "TILESETS",
            "`Tileset` header index -> name (`gTileset_<name>`), in `src/data/tilesets/headers.h` order.",
            "",
            tileset_list
                .iter()
                .map(|(i, n, _)| (*i, n.clone()))
                .collect(),
        ),
    ];
    for (name, doc, prefix, entries) in &tables {
        render_table(&mut out, name, doc, prefix, entries);
    }
    out.push_str("/// Byte offset of each `TILESETS` header from `gTileset_General` (the headers are not quite\n/// contiguous: two `Tileset` pointers sit among them).\n");
    out.push_str("pub static TILESET_HEADER_OFFSETS: &[u32] = &[\n");
    for (_, n, at) in &tileset_list {
        writeln!(out, "    {at:#x}, // {n}").unwrap();
    }
    out.push_str("];\n");
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

type Entries = Vec<(i64, String)>;

const GENERATED: &str = "src/names/generated.rs";

#[test]
fn generated_names_match_decomp() {
    let d = fixtures::decomp();
    let want = generate(&d);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GENERATED);
    if std::env::var_os("PKR_BLESS").is_some() {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap();
    if let Some((i, (h, w))) = have
        .lines()
        .zip(want.lines())
        .enumerate()
        .find(|(_, (h, w))| h != w)
    {
        panic!(
            "{GENERATED} drifted at line {}: have {h:?}, decomp gives {w:?} (PKR_BLESS=1 to regenerate)",
            i + 1
        );
    }
    assert_eq!(
        have.lines().count(),
        want.lines().count(),
        "{GENERATED} length (PKR_BLESS=1 to regenerate)"
    );
}

#[test]
fn std_script_comments_agree_with_event_inc_symbols() {
    let d = fixtures::decomp();
    let inc = read(&d, "asm/macros/event.inc");
    let mut symbols = asm_symbols(&inc, "STD_");
    symbols.extend(asm_symbols(&inc, "MSGBOX_"));
    for (v, n) in symbols {
        assert_eq!(names::STD_SCRIPTS.name(v as u32), Some(n.as_str()), "{n}");
    }
}

#[test]
fn lookups_and_fallbacks() {
    assert_eq!(names::flag_name(0x1), "FLAG_TEMP_1");
    assert_eq!(names::FLAGS.value("FLAG_HIDDEN_ITEMS_START"), Some(0x1F4));
    assert_eq!(names::var_name(0x800D), "VAR_RESULT");
    assert_eq!(names::var_name(0x800C), "VAR_FACING");
    assert_eq!(names::var_name(0x8000), "VAR_0x8000");
    assert_eq!(names::var_name(0x4000), "VAR_TEMP_0");
    assert_eq!(names::var_name(0x4321), "VAR_0x4321");
    assert_eq!(names::flag_name(0x3FFF), "FLAG_0x3FFF");
    assert_eq!(names::gfx_name(0), "OBJ_EVENT_GFX_BRENDAN_NORMAL");
    assert_eq!(names::gfx_name(89), "OBJ_EVENT_GFX_MAY_NORMAL");
    assert_eq!(names::gfx_name(240), "OBJ_EVENT_GFX_VAR_0");
    assert_eq!(names::OBJ_EVENT_GFX.value("OBJ_EVENT_GFX_VARS"), Some(240));
    assert_eq!(
        names::movement_action_name(0xFE),
        "MOVEMENT_ACTION_STEP_END"
    );
    assert_eq!(names::behavior_name(2), "MB_TALL_GRASS");
    assert_eq!(names::special_name(0), "SPECIAL_HealPlayerParty");
    assert_eq!(names::std_name(2), "MSGBOX_NPC");
    assert_eq!(names::std_name(0), "STD_OBTAIN_ITEM");
    assert_eq!(names::MSGBOX_TYPES.value("MSGBOX_YESNO"), Some(5));
    assert_eq!(names::TILESETS.name(0), Some("General"));
    let m = names::map_by_dir("LittlerootTown").unwrap();
    assert_eq!((m.group, m.num, m.constant), (0, 9, "MAP_LITTLEROOT_TOWN"));
    assert_eq!(names::map(0, 16).unwrap().dir, "Route101");
    assert_eq!(names::map_by_constant("MAP_ROUTE101").unwrap().num, 16);
    assert!(names::map(0x7F, 0x7F).is_none());
    assert_eq!(names::MAPS.len(), 518);
}
