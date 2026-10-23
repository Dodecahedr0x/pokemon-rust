//! Body sharing (Milestone 8 Task L): functions whose bodies differ only in their operands
//! (texts, movements, constants, script ids, small literals) become one shared `async fn` per
//! *shape* in the [`SHAPES`] module, and each such script is a plain function returning the
//! shape's future:
//!
//! ```text
//! pub fn Route102_EventScript_Boy(ctx: Ctx) -> impl core::future::Future<Output = Flow> {
//!     crate::scripts::Shapes::shape12(ctx, Route102_Text_CatchWholeBunchOfPokemon)
//! }
//! ```
//!
//! The shared body is the function's own body with every operand replaced by a parameter, so the
//! requests it issues (and therefore every hash) are the same. On the full import about two thirds
//! of the functions share a shape (an NPC's `msgbox`, a single `trainerbattle`, an item ball): the
//! type checker sees one async body per shape instead of one per script, which is what keeps the
//! content build under its memory and time gates. Digests are taken from the unshared sources.
//!
//! Operands, by token:
//! - a text name: `TextId`;
//! - `&<movement>` (bare or absolute): `&'static [Step]`;
//! - `crate::data::script_ids::<Name>`: `ScriptId`;
//! - a constant whose prefix has one type in `content::consts` ([`CONST_TYPES`]);
//! - an integer literal in a known argument position ([`literal_type`]).
//!
//! Comments are dropped from shared bodies; references to items of the function's own module
//! are made absolute so the body compiles in [`SHAPES`]. A function with an operand in a
//! pattern position (a match arm's pattern: the rest of its line holds `=>`) is not shared: a
//! parameter there would be a binding that matches anything.

use super::*;

/// The module of the shared bodies.
pub(super) const SHAPES: &str = "Shapes";

/// Constant prefixes whose every constant has one type (`content::consts`, the prelude).
const CONST_TYPES: [(&str, &str); 11] = [
    ("FLAG_", "FlagId"),
    ("VAR_", "VarId"),
    ("TRAINER_", "TrainerId"),
    ("MAP_", "MapId"),
    ("SPECIES_", "SpeciesId"),
    ("MOVE_", "MoveId"),
    ("ITEM_", "u16"),
    ("SPECIAL_", "u16"),
    ("HEAL_LOCATION_", "u8"),
    ("STD_", "u8"),
    ("MOVEMENT_TYPE_", "MovementType"),
];

/// Calls whose `n`th argument has a known integer type when it is a literal.
const CALL_ARGS: [(&str, usize, &str); 16] = [
    ("ctx.set_var", 1, "u16"),
    ("ctx.delay", 0, "u16"),
    ("ctx.random", 0, "u16"),
    ("wrapping_add", 0, "u16"),
    ("wrapping_sub", 0, "u16"),
    ("ctx.move_obj", 0, "u8"),
    ("ctx.wait_move", 0, "u8"),
    ("ctx.turn", 0, "u8"),
    ("ctx.add_object", 0, "u8"),
    ("ctx.remove_object", 0, "u8"),
    ("ctx.show_object", 0, "u8"),
    ("ctx.hide_object", 0, "u8"),
    ("ctx.set_object_pos", 0, "u8"),
    ("ctx.set_object_template_pos", 0, "u8"),
    ("ctx.set_movement_type", 0, "u8"),
    ("ctx.trainer_approach", 0, "u8"),
];

/// The type of an integer literal written right after `before`, if its position has one.
fn literal_type(before: &str) -> Option<&'static str> {
    if before.ends_with("local_id: ") || before.ends_with("kind: ") {
        return Some("u8");
    }
    if before.ends_with(".cmp(&") {
        return Some("u16");
    }
    let open = before.rfind('(')?;
    let args = &before[open + 1..];
    if args.contains([')', '{', '}', '"', '&']) || !(args.is_empty() || args.ends_with(", ")) {
        return None;
    }
    let n = args.matches(',').count();
    let callee = before[..open]
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .next()?;
    CALL_ARGS
        .iter()
        .find(|&&(f, i, _)| (callee == f || callee.ends_with(&format!(".{f}"))) && i == n)
        .map(|&(_, _, t)| t)
}

/// A function's body with its operands as parameters `p0..`.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Shape {
    /// Parameter types, in order.
    params: Vec<&'static str>,
    body: String,
}

/// What the module of a function can name without a path.
pub(super) struct Names<'n> {
    /// Text names.
    pub texts: BTreeSet<&'n str>,
    /// Movement name -> its module.
    pub movements: BTreeMap<&'n str, &'n str>,
    /// Function and tail-call loop names -> their module.
    pub items: BTreeMap<String, &'n str>,
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `body` (a function's statements) normalised to a [`Shape`], with the operand of each
/// parameter; `None` when it has no operand.
fn shape_of(module: &str, body: &str, names: &Names) -> Option<(Shape, Vec<String>)> {
    let mut out = String::new();
    let mut params: Vec<&'static str> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c == '"' {
            // A string literal, copied as is.
            let start = i;
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
            out.push_str(&body[start..i]);
            continue;
        }
        if c == '/' && b.get(i + 1) == Some(&b'/') {
            // A comment, dropped with the spaces before it.
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            let trimmed = out.trim_end_matches(' ').len();
            out.truncate(trimmed);
            if out.ends_with('\n') || out.is_empty() {
                // The line held only the comment.
                i += 1;
            }
            continue;
        }
        if !is_ident_char(c) {
            out.push(c);
            i += 1;
            continue;
        }
        // A token: identifiers joined by `::`.
        let start = i;
        loop {
            while i < b.len() && is_ident_char(b[i] as char) {
                i += 1;
            }
            if b.get(i) == Some(&b':')
                && b.get(i + 1) == Some(&b':')
                && b.get(i + 2).is_some_and(|&x| is_ident_char(x as char))
            {
                i += 2;
                continue;
            }
            break;
        }
        let tok = &body[start..i];
        let last = tok.rsplit("::").next().unwrap_or(tok);
        let prev = body[..start].chars().next_back();
        let param = if prev == Some('.') {
            // A field or method.
            None
        } else if c.is_ascii_digit() {
            literal_type(&out)
        } else if tok == last && names.texts.contains(tok) {
            Some("TextId")
        } else if out.ends_with('&')
            && names
                .movements
                .get(last)
                .is_some_and(|m| tok == last || tok == format!("{SCRIPTS}::{m}::{last}"))
        {
            out.pop();
            Some("&'static [Step]")
        } else if tok.starts_with(&format!("{DATA}::script_ids::")) {
            Some("ScriptId")
        } else if tok == last {
            CONST_TYPES
                .iter()
                .find(|(p, _)| tok.starts_with(p))
                .map(|&(_, t)| t)
        } else {
            None
        };
        match param {
            // In a pattern, `p0` would bind instead of compare: keep the function unshared.
            Some(_) if body[i..].lines().next().is_some_and(|l| l.contains("=>")) => return None,
            Some(ty) => {
                let arg = if ty.starts_with('&') {
                    format!("&{tok}")
                } else {
                    tok.to_string()
                };
                // One parameter per distinct operand.
                let k = match args.iter().position(|a| *a == arg) {
                    Some(k) if params[k] == ty => k,
                    _ => {
                        params.push(ty);
                        args.push(arg);
                        params.len() - 1
                    }
                };
                write!(out, "p{k}").ok()?;
            }
            None => match names.items.get(tok) {
                // An item of the function's own module, named bare.
                Some(m) if *m == module => write!(out, "{SCRIPTS}::{m}::{tok}").ok()?,
                _ if tok == last && names.movements.get(tok) == Some(&module) => {
                    write!(out, "{SCRIPTS}::{module}::{tok}").ok()?
                }
                _ => out.push_str(tok),
            },
        }
    }
    if params.is_empty() {
        return None;
    }
    Some((Shape { params, body: out }, args))
}

/// Splits `pub async fn <name>(ctx: Ctx) -> Flow {\n<body>}\n` into its body.
fn plain_body<'s>(name: &str, src: &'s str) -> Option<&'s str> {
    src.strip_prefix(&format!("pub async fn {name}(ctx: Ctx) -> Flow {{\n"))?
        .strip_suffix("}\n")
}

/// Replaces the functions of `sources` (by entry) that share a shape with at least one other by
/// a call to the shape, and returns the [`SHAPES`] module (empty when nothing is shared).
pub(super) fn share(
    low: &Low,
    sources: &mut BTreeMap<usize, String>,
    names: &Names,
) -> Result<String> {
    let mut shaped: Vec<(usize, Shape, Vec<String>)> = Vec::new();
    for (&f, src) in sources.iter() {
        let l = &low.labs[f];
        if low.overrides.contains_key(l.name) || low.group_of.contains_key(&f) {
            continue;
        }
        let Some(body) = plain_body(l.name, src) else {
            continue;
        };
        if let Some((shape, args)) = shape_of(l.module, body, names) {
            shaped.push((f, shape, args));
        }
    }
    let mut count: BTreeMap<&Shape, usize> = BTreeMap::new();
    for (_, s, _) in &shaped {
        *count.entry(s).or_default() += 1;
    }
    // Shapes numbered by their first function (label order).
    let mut index: BTreeMap<&Shape, usize> = BTreeMap::new();
    let mut module = String::new();
    for (_, s, _) in &shaped {
        if count[s] < 2 || index.contains_key(s) {
            continue;
        }
        let n = index.len();
        index.insert(s, n);
        let params: Vec<String> = s
            .params
            .iter()
            .enumerate()
            .map(|(k, t)| format!(", p{k}: {t}"))
            .collect();
        write!(
            module,
            "\n/// Shared by {} scripts.\npub async fn shape{n}(ctx: Ctx{}) -> Flow {{\n{}}}\n",
            count[s],
            params.concat(),
            s.body
        )?;
    }
    let mut replaced = Vec::new();
    for (f, s, args) in &shaped {
        let Some(&n) = index.get(s) else {
            continue;
        };
        let name = low.labs[*f].name;
        replaced.push((
            *f,
            format!(
                "pub fn {name}(ctx: Ctx) -> impl core::future::Future<Output = Flow> {{\n    {SCRIPTS}::{SHAPES}::shape{n}(ctx, {})\n}}\n",
                args.join(", ")
            ),
        ));
    }
    for (f, src) in replaced {
        sources.insert(f, src);
    }
    if !module.is_empty() {
        module.insert_str(0, &format!("use {DATA}::Flow;\n"));
    }
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Names<'static> {
        Names {
            texts: BTreeSet::from(["T_Text"]),
            movements: BTreeMap::new(),
            items: BTreeMap::new(),
        }
    }

    /// Operands become parameters in expressions, but a function with one in a match arm's
    /// pattern (alone, in an or-pattern or a tuple) is not shared.
    #[test]
    fn operands_in_patterns_are_not_shared() {
        let n = names();
        let expr = "    ctx.message(T_Text).await;\n    ctx.set_flag(FLAG_A, true).await;\n";
        let (shape, args) = shape_of("M", expr, &n).expect("shared");
        assert_eq!(shape.params, ["TextId", "FlagId"]);
        assert_eq!(args, ["T_Text", "FLAG_A"]);
        for arm in [
            "        MAP_A => return Flow::End,\n",
            "        MAP_A | MAP_B => return Flow::End,\n",
            "        (MAP_A, _) => return Flow::End,\n",
        ] {
            let body = format!(
                "    ctx.message(T_Text).await;\n    match (ctx.current_map().await, 0) {{\n{arm}        _ => {{}}\n    }}\n"
            );
            assert!(shape_of("M", &body, &n).is_none(), "{arm}");
        }
        // After the arrow, an operand is an expression again.
        let body = "    match ctx.var(VAR_A).await {\n        0 => ctx.message(T_Text).await,\n        _ => {}\n    }\n";
        let (shape, _) = shape_of("M", body, &n).expect("shared");
        assert_eq!(shape.params, ["VarId", "TextId"]);
    }
}
