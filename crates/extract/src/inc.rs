//! Decomp source reader for script names (Milestone 8 Task B). Not an assembler: it reads the
//! labels of the pokeemerald script sources in source order, what each label's body references,
//! and the `map.json` event labels, which is all the label transfer ([`crate::labels`]) needs.
//!
//! **Sources.** `data/event_scripts.s` is read as the root, following its `.include` lines into
//! `data/maps/*/scripts.inc`, `data/scripts/*.inc` and `data/text/*.inc` in the assembler's
//! order (so "source order" is the order of the assembled script section), with the inline labels
//! of `event_scripts.s` itself (`EventScript_WhiteOut`, …) where they appear. Files of those
//! directories that `event_scripts.s` does not include are read afterwards, sorted by path.
//! `#ifdef`/`#ifndef` take the vanilla branch (`BUGFIX`, `UBFIX` and friends undefined).
//!
//! **Labels.** A label (`Name::` or `Name:`) owns the statements up to the next label, an
//! `.include`, or the end of its file. Its kind comes from its first statement: `.string` is a
//! text, `.braille`/`brailleformat` braille, a `asm/macros/movement.inc` action a movement,
//! `map_script`/`map_script_2`/`.byte`/`.2byte`/`.4byte`/`.align`/`pokemartlistend` data, and
//! anything else a script. An empty body is an alias of the next label and takes its kind.
//! A script or movement whose body does not end (no terminator command, no `step_end`) falls
//! through to the next label of its file when that label is of the same kind: [`Label::next`].
//! (A script that runs into a table, such as `TrainerHill_1F_EventScript_DummyOnWarp`, which
//! relies on the table's first byte being `end`, has no successor.)
//!
//! **References.** [`Decomp::refs`] lists the label operands of a body in textual order, up to
//! the first terminator, each with the operand kind its macro implies. The knowledge is the small
//! table of reference-bearing macros in [`stmt_refs`] (from `asm/macros/{event,map}.inc`); every
//! other line contributes nothing. The `v*` commands' operands are RAM-relative and never count.
//!
//! **Modules.** `data/maps/<Map>/scripts.inc` is module `<Map>`, `data/scripts/<stem>.inc` is
//! `Common_<stem>`, `data/text/<stem>.inc` is `Text_<stem>` (`cable_club` exists in both
//! directories) and the inline labels of `event_scripts.s` are `Common_event_scripts`.

use crate::scripts;
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The root source, relative to the decomp.
pub const ROOT_SOURCE: &str = "data/event_scripts.s";
/// Directories whose `.inc` files hold script, text and movement labels.
const SOURCE_DIRS: [&str; 2] = ["data/scripts", "data/text"];

/// What a label's body holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LabelKind {
    Script,
    Text,
    Braille,
    Movement,
    /// Map script tables, mart lists and other tables.
    Data,
}

/// The kind of a label operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Script,
    Text,
    Movement,
    Braille,
}

/// One statement: a macro (or directive) and its comma-separated operands, as written.
/// `.string` and `.braille` keep their whole quoted operand as one argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stmt {
    /// 1-based source line.
    pub line: usize,
    pub mac: String,
    pub args: Vec<String>,
}

/// One source file, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// Relative to the decomp root, `/`-separated.
    pub path: String,
    pub module: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    /// Index into [`Decomp::files`].
    pub file: usize,
    /// 1-based source line of the label.
    pub line: usize,
    pub kind: LabelKind,
    pub body: Vec<Stmt>,
    /// The label this one falls through to (index into [`Decomp::labels`]), for scripts and
    /// movements whose body does not end.
    pub next: Option<usize>,
}

/// One label operand of a body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub kind: RefKind,
    /// The operand as written: a label, a C symbol (`gText_…`), a RAM symbol (`gStringVar4`)
    /// or a number (`0`, `NULL`, `FALSE`).
    pub target: String,
}

impl Ref {
    /// Whether the operand names a symbol (label or C symbol) rather than a number or `NULL`.
    pub fn is_symbol(&self) -> bool {
        is_symbol(&self.target)
    }
}

/// The script labels of one map's `map.json` events, by index in the ROM's event lists
/// (`None` where the event has no script: weather coords, hidden items, `0x0`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapJson {
    pub name: String,
    pub shared_scripts_map: Option<String>,
    pub shared_events_map: Option<String>,
    pub objects: Vec<Option<String>>,
    pub coords: Vec<Option<String>>,
    pub bgs: Vec<Option<String>>,
}

/// A map's `map_script` table as labels, mirroring [`crate::roots::MapScriptTable`] (first
/// entry of each type wins).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapScriptLabels {
    pub on_load: Option<String>,
    pub on_transition: Option<String>,
    pub on_resume: Option<String>,
    pub on_return_to_field: Option<String>,
    pub on_dive_warp: Option<String>,
    pub on_frame: Vec<String>,
    pub on_warp_into: Vec<String>,
}

/// Every label of the decomp's script sources and every `map.json`.
#[derive(Clone, Debug)]
pub struct Decomp {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
    /// Source order.
    pub labels: Vec<Label>,
    /// Map directory name -> its `map.json` labels.
    pub maps: BTreeMap<String, MapJson>,
    /// Label name -> index of its first definition.
    by_name: BTreeMap<String, usize>,
    /// `asm/macros/movement.inc`: macro name -> `MOVEMENT_ACTION_*` value.
    movement_macros: BTreeMap<String, u8>,
    /// The battle facility helper macros ([`FACILITY_MACROS`]).
    facility_macros: BTreeMap<String, MacroDef>,
}

impl Decomp {
    /// Reads the decomp at `root`.
    pub fn load(root: &Path) -> Result<Decomp> {
        let movement_macros = read_movement_macros(root)?;
        let mut d = Decomp {
            root: root.to_path_buf(),
            files: Vec::new(),
            labels: Vec::new(),
            maps: BTreeMap::new(),
            by_name: BTreeMap::new(),
            movement_macros,
            facility_macros: read_facility_macros(root)?,
        };
        let mut seen = BTreeSet::new();
        d.read_source(ROOT_SOURCE, &mut seen)?;
        let mut rest = Vec::new();
        for entry in read_dir_sorted(&root.join("data/maps"))? {
            let rel = format!("data/maps/{entry}/scripts.inc");
            if root.join(&rel).exists() {
                rest.push(rel);
            }
        }
        for dir in SOURCE_DIRS {
            for entry in read_dir_sorted(&root.join(dir))? {
                if entry.ends_with(".inc") {
                    rest.push(format!("{dir}/{entry}"));
                }
            }
        }
        rest.sort();
        for rel in rest {
            if !seen.contains(&rel) {
                d.read_source(&rel, &mut seen)?;
            }
        }
        d.resolve_kinds();
        for (i, l) in d.labels.iter().enumerate() {
            d.by_name.entry(l.name.clone()).or_insert(i);
        }
        for entry in read_dir_sorted(&root.join("data/maps"))? {
            let path = root.join("data/maps").join(&entry).join("map.json");
            if path.exists() {
                let m = read_map_json(&path).with_context(|| path.display().to_string())?;
                d.maps.insert(entry, m);
            }
        }
        Ok(d)
    }

    /// The label named `name` (its first definition).
    pub fn label(&self, name: &str) -> Option<&Label> {
        self.by_name.get(name).map(|&i| &self.labels[i])
    }

    /// The index of the label named `name` in [`Decomp::labels`].
    pub fn label_index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    /// The module of a label (see the module doc).
    pub fn module_of(&self, l: &Label) -> &str {
        &self.files[l.file].module
    }

    /// The label operands of `l`'s body in textual order, up to its first terminator.
    pub fn refs(&self, l: &Label) -> Vec<Ref> {
        let mut out = Vec::new();
        for s in &l.body {
            out.extend(stmt_refs(s));
            if is_terminator(s) {
                break;
            }
        }
        out
    }

    /// Whether `l`'s body ends (a script terminator, a movement `step_end`, a text `$`), so
    /// that it does not fall through to the next label.
    pub fn terminates(&self, l: &Label) -> bool {
        match l.kind {
            LabelKind::Script => l.body.iter().any(is_terminator),
            LabelKind::Movement => l.body.iter().any(|s| s.mac == "step_end"),
            LabelKind::Text | LabelKind::Braille => l
                .body
                .iter()
                .any(|s| s.args.first().is_some_and(|a| a.ends_with("$\""))),
            LabelKind::Data => true,
        }
    }

    /// The script commands (`script_cmd_table.inc` names) `l`'s body assembles to, up to its
    /// first terminator. `compare` stands for either `compare_var_to_value` or
    /// `compare_var_to_var` (the macro picks by the operand's value). `None` if the body uses a
    /// macro this reader does not expand.
    pub fn ops(&self, l: &Label) -> Option<Vec<&'static str>> {
        match self.ops_prefix(l) {
            (ops, true) => Some(ops),
            (_, false) => None,
        }
    }

    /// [`Decomp::ops`] up to the first macro this reader does not expand, and whether the body
    /// was expanded completely.
    pub fn ops_prefix(&self, l: &Label) -> (Vec<&'static str>, bool) {
        let mut out = Vec::new();
        for s in &l.body {
            let Some(ops) = self.expand_stmt(s) else {
                return (out, false);
            };
            out.extend(ops);
            if is_terminator(s) {
                break;
            }
        }
        (out, true)
    }

    /// The script commands one statement assembles to: [`expand`] for the `event.inc` macros,
    /// then the battle facility helpers. `None` for an unknown macro.
    pub fn expand_stmt(&self, s: &Stmt) -> Option<Vec<&'static str>> {
        expand(s).or_else(|| self.facility_macros.get(&s.mac)?.expand(&s.args))
    }

    /// The `MOVEMENT_ACTION_*` bytes of a movement body, without `step_end`; `None` if a line is
    /// not a movement macro.
    pub fn movement_actions(&self, l: &Label) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for s in &l.body {
            if s.mac == "step_end" {
                break;
            }
            out.push(*self.movement_macros.get(&s.mac)?);
        }
        Some(out)
    }

    /// The `.string` text of a text label, lines joined, without its `$` terminator; `None` if
    /// the body has no `$`.
    pub fn text(&self, l: &Label) -> Option<String> {
        let mut out = String::new();
        for s in &l.body {
            if s.mac != ".string" {
                continue;
            }
            let a = s.args.first()?;
            let inner = a.strip_prefix('"')?.strip_suffix('"')?;
            out.push_str(inner);
            if let Some(done) = out.strip_suffix('$') {
                return Some(done.to_string());
            }
        }
        None
    }

    /// The `map_script` table of map `map` (its scripts map: `shared_scripts_map` if set).
    pub fn map_scripts(&self, map: &str) -> MapScriptLabels {
        let mut t = MapScriptLabels::default();
        let Some(table) = self.label(&format!("{}_MapScripts", self.scripts_map(map))) else {
            return t;
        };
        let mut seen = BTreeSet::new();
        for s in &table.body {
            if s.mac != "map_script" || s.args.len() != 2 || !seen.insert(s.args[0].clone()) {
                continue;
            }
            let target = s.args[1].clone();
            let list = || {
                self.label(&target)
                    .map(|l| {
                        l.body
                            .iter()
                            .filter(|s| s.mac == "map_script_2" && s.args.len() == 3)
                            .map(|s| s.args[2].clone())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            match s.args[0].as_str() {
                "MAP_SCRIPT_ON_LOAD" => t.on_load = Some(target),
                "MAP_SCRIPT_ON_TRANSITION" => t.on_transition = Some(target),
                "MAP_SCRIPT_ON_RESUME" => t.on_resume = Some(target),
                "MAP_SCRIPT_ON_RETURN_TO_FIELD" => t.on_return_to_field = Some(target),
                "MAP_SCRIPT_ON_DIVE_WARP" => t.on_dive_warp = Some(target),
                "MAP_SCRIPT_ON_FRAME_TABLE" => t.on_frame = list(),
                "MAP_SCRIPT_ON_WARP_INTO_MAP_TABLE" => t.on_warp_into = list(),
                _ => {}
            }
        }
        t
    }

    /// The map whose `<Map>_MapScripts` applies to `map` (`shared_scripts_map`, else itself).
    pub fn scripts_map<'a>(&'a self, map: &'a str) -> &'a str {
        self.maps
            .get(map)
            .and_then(|m| m.shared_scripts_map.as_deref())
            .unwrap_or(map)
    }

    /// The `map.json` whose events apply to `map` (`shared_events_map`, else its own).
    pub fn events_map(&self, map: &str) -> Option<&MapJson> {
        let m = self.maps.get(map)?;
        match &m.shared_events_map {
            Some(shared) => self.maps.get(shared),
            None => Some(m),
        }
    }

    fn read_source(&mut self, rel: &str, seen: &mut BTreeSet<String>) -> Result<()> {
        ensure!(seen.insert(rel.to_string()), "{rel} is included twice");
        let path = self.root.join(rel);
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file = self.files.len();
        self.files.push(SourceFile {
            path: rel.to_string(),
            module: module_name(rel)?,
        });
        // Index of the label whose body is being read, if any.
        let mut cur: Option<usize> = None;
        let mut cond: Vec<bool> = Vec::new();
        let mut in_comment = false;
        for (i, raw) in src.lines().enumerate() {
            let line_no = i + 1;
            let line = strip_comments(raw, &mut in_comment);
            let line = line.trim();
            if let Some(directive) = line.strip_prefix('#') {
                let mut words = directive.split_whitespace();
                match words.next() {
                    Some("ifdef") => cond.push(false),
                    Some(d @ ("if" | "elif")) => {
                        bail!("{rel}:{line_no}: #{d} is not supported (only #ifdef/#ifndef)")
                    }
                    Some("ifndef") => cond.push(true),
                    Some("else") => {
                        let c = cond.last_mut().context("#else without #if")?;
                        *c = !*c;
                    }
                    Some("endif") => {
                        cond.pop().context("#endif without #if")?;
                    }
                    _ => {}
                }
                continue;
            }
            if cond.iter().any(|c| !c) || line.is_empty() {
                continue;
            }
            if let Some(name) = label_def(line) {
                let idx = self.labels.len();
                if let Some(prev) = cur {
                    // Provisional: kept only if the previous body does not end (resolve_kinds).
                    self.labels[prev].next = Some(idx);
                }
                self.labels.push(Label {
                    name: name.to_string(),
                    file,
                    line: line_no,
                    kind: LabelKind::Script,
                    body: Vec::new(),
                    next: None,
                });
                cur = Some(idx);
                continue;
            }
            let (mac, rest) = match line.split_once(char::is_whitespace) {
                Some((m, r)) => (m, r.trim()),
                None => (line, ""),
            };
            if mac == ".include" {
                cur = None;
                let target = rest.trim_matches('"');
                if is_source(target) {
                    self.read_source(target, seen)
                        .with_context(|| format!("included from {rel}:{line_no}"))?;
                }
                continue;
            }
            let Some(l) = cur else { continue };
            let args = if mac == ".string" || mac == ".braille" {
                vec![rest.to_string()]
            } else if rest.is_empty() {
                Vec::new()
            } else {
                split_args(rest)
            };
            self.labels[l].body.push(Stmt {
                line: line_no,
                mac: mac.to_string(),
                args,
            });
        }
        ensure!(cond.is_empty(), "{rel}: unterminated #if");
        Ok(())
    }

    /// Kinds from the bodies (aliases take their target's), then fallthrough links kept only for
    /// scripts and movements whose bodies do not end.
    fn resolve_kinds(&mut self) {
        for l in &mut self.labels {
            if let Some(first) = l.body.first() {
                l.kind = self_kind(first, &self.movement_macros);
            }
        }
        for i in (0..self.labels.len()).rev() {
            if self.labels[i].body.is_empty()
                && let Some(n) = self.labels[i].next
            {
                self.labels[i].kind = self.labels[n].kind;
            }
        }
        for i in 0..self.labels.len() {
            let l = &self.labels[i];
            let same_kind = l.next.is_some_and(|n| self.labels[n].kind == l.kind);
            let keep = l.body.is_empty()
                || (matches!(l.kind, LabelKind::Script | LabelKind::Movement)
                    && same_kind
                    && !self.terminates(l));
            if !keep {
                self.labels[i].next = None;
            }
        }
    }
}

fn self_kind(first: &Stmt, movement: &BTreeMap<String, u8>) -> LabelKind {
    match first.mac.as_str() {
        ".string" => LabelKind::Text,
        ".braille" | "brailleformat" => LabelKind::Braille,
        "map_script" | "map_script_2" | ".byte" | ".2byte" | ".4byte" | ".align"
        | "pokemartlistend" => LabelKind::Data,
        m if movement.contains_key(m) => LabelKind::Movement,
        _ => LabelKind::Script,
    }
}

/// `data/maps/*/scripts.inc` and the `.inc` files of [`SOURCE_DIRS`].
fn is_source(rel: &str) -> bool {
    (rel.starts_with("data/maps/") && rel.ends_with("/scripts.inc"))
        || SOURCE_DIRS
            .iter()
            .any(|d| rel.starts_with(d) && rel.ends_with(".inc"))
}

fn module_name(rel: &str) -> Result<String> {
    if rel == ROOT_SOURCE {
        return Ok("Common_event_scripts".to_string());
    }
    if let Some(map) = rel
        .strip_prefix("data/maps/")
        .and_then(|r| r.strip_suffix("/scripts.inc"))
    {
        return Ok(map.to_string());
    }
    if let Some(stem) = rel
        .strip_prefix("data/scripts/")
        .and_then(|r| r.strip_suffix(".inc"))
    {
        return Ok(format!("Common_{stem}"));
    }
    if let Some(stem) = rel
        .strip_prefix("data/text/")
        .and_then(|r| r.strip_suffix(".inc"))
    {
        return Ok(format!("Text_{stem}"));
    }
    bail!("{rel} is not a script source")
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        out.push(e?.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    Ok(out)
}

/// `Name` of a `Name:`/`Name::` line.
fn label_def(line: &str) -> Option<&str> {
    let name = line.strip_suffix("::").or_else(|| line.strip_suffix(':'))?;
    is_ident(name).then_some(name)
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An identifier other than the numeric aliases `NULL`, `FALSE` and `TRUE`.
fn is_symbol(s: &str) -> bool {
    is_ident(s) && !matches!(s, "NULL" | "FALSE" | "TRUE")
}

/// `line` without `@` comments (outside strings) and `/* … */` comments.
fn strip_comments(line: &str, in_comment: &mut bool) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if *in_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                *in_comment = false;
            }
            continue;
        }
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '@' => break,
            '/' if chars.peek() == Some(&'/') => break,
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                *in_comment = true;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Macro operands, as `as` splits them: at commas, and at spaces between plain words (the
/// sources have one `frontier_set A B` with no comma); expressions like `(X + 1)` stay whole.
fn split_args(rest: &str) -> Vec<String> {
    let mut out = Vec::new();
    for piece in rest.split(',').map(str::trim) {
        let words: Vec<&str> = piece.split_whitespace().collect();
        if words.len() > 1
            && words
                .iter()
                .all(|w| is_ident(w) || w.parse::<i64>().is_ok())
        {
            out.extend(words.iter().map(|w| w.to_string()));
        } else {
            out.push(piece.to_string());
        }
    }
    out
}

/// Script commands after which execution never reaches the next statement.
fn is_terminator(s: &Stmt) -> bool {
    scripts::opcode(&s.mac).is_some_and(scripts::is_terminator_op)
}

/// `TRAINER_BATTLE_*` (`include/constants/battle_setup.h`) by name or number.
fn trainer_battle_type(s: &str) -> Option<u8> {
    const TYPES: [&str; 13] = [
        "TRAINER_BATTLE_SINGLE",
        "TRAINER_BATTLE_CONTINUE_SCRIPT_NO_MUSIC",
        "TRAINER_BATTLE_CONTINUE_SCRIPT",
        "TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT",
        "TRAINER_BATTLE_DOUBLE",
        "TRAINER_BATTLE_REMATCH",
        "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE",
        "TRAINER_BATTLE_REMATCH_DOUBLE",
        "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE_NO_MUSIC",
        "TRAINER_BATTLE_PYRAMID",
        "TRAINER_BATTLE_SET_TRAINER_A",
        "TRAINER_BATTLE_SET_TRAINER_B",
        "TRAINER_BATTLE_HILL",
    ];
    TYPES
        .iter()
        .position(|t| *t == s)
        .map(|p| p as u8)
        .or_else(|| s.parse().ok())
}

/// The label operands of one statement, in operand order (the table of reference-bearing
/// macros of `asm/macros/{event,map}.inc`).
pub fn stmt_refs(s: &Stmt) -> Vec<Ref> {
    use RefKind::*;
    let a = |i: usize| s.args.get(i).cloned();
    let mut out: Vec<(RefKind, Option<String>)> = Vec::new();
    let cc = |prefix: &str| {
        ["lt", "eq", "gt", "le", "ge", "ne"]
            .iter()
            .any(|c| s.mac.strip_prefix(prefix) == Some(c))
    };
    match s.mac.as_str() {
        "call" | "goto" => out.push((Script, a(0))),
        "goto_if" | "call_if" | "case" => out.push((Script, a(1))),
        "goto_if_set"
        | "goto_if_unset"
        | "call_if_set"
        | "call_if_unset"
        | "goto_if_defeated"
        | "goto_if_not_defeated"
        | "call_if_defeated"
        | "call_if_not_defeated" => out.push((Script, a(1))),
        _ if cc("goto_if_") || cc("call_if_") => out.push((Script, s.args.last().cloned())),
        "map_script" => out.push((Script, a(1))),
        "map_script_2" => out.push((Script, a(2))),
        "msgbox" | "message" | "messageautoscroll" | "messageinstant" | "pokenavcall" => {
            out.push((Text, a(0)))
        }
        "bufferstring" => out.push((Text, a(1))),
        "loadword" if a(0).as_deref() == Some("0") => out.push((Text, a(1))),
        "braillemessage" | "braillemsgbox" => out.push((Braille, a(0))),
        "applymovement" => out.push((Movement, a(1))),
        "trainerbattle_single" | "trainerbattle_double" => {
            let texts = if s.mac == "trainerbattle_single" {
                2
            } else {
                3
            };
            for i in 1..=texts {
                out.push((Text, a(i)));
            }
            if let Some(script) = a(texts + 1).filter(|e| e != "FALSE") {
                out.push((Script, Some(script)));
            }
        }
        "trainerbattle_rematch" => out.extend([(Text, a(1)), (Text, a(2))]),
        "trainerbattle_rematch_double" => out.extend([(Text, a(1)), (Text, a(2)), (Text, a(3))]),
        "trainerbattle_no_intro" => out.push((Text, a(1))),
        "trainerbattle" => {
            let kinds: &[RefKind] = match a(0).as_deref().and_then(trainer_battle_type) {
                Some(0 | 5 | 9..=12) => &[Text, Text],
                Some(1 | 2) => &[Text, Text, Script],
                Some(3) => &[Text],
                Some(4 | 7) => &[Text, Text, Text],
                Some(6 | 8) => &[Text, Text, Text, Script],
                _ => &[],
            };
            for (i, &k) in kinds.iter().enumerate() {
                out.push((k, a(3 + i)));
            }
        }
        _ => {}
    }
    out.into_iter()
        .map(|(kind, t)| Ref {
            kind,
            target: t.unwrap_or_default(),
        })
        .collect()
}

/// The script commands one statement assembles to (see [`Decomp::ops`]). `None` for macros
/// this reader does not know; data directives give nothing.
pub fn expand(s: &Stmt) -> Option<Vec<&'static str>> {
    let n = s.args.len();
    let cc = |prefix: &str| {
        ["lt", "eq", "gt", "le", "ge", "ne"]
            .iter()
            .any(|c| s.mac.strip_prefix(prefix) == Some(c))
    };
    let at = |base: &'static str, with: &'static str, extra: usize| {
        if n > extra { with } else { base }
    };
    Some(match s.mac.as_str() {
        m if m.starts_with('.') => vec![],
        "msgbox" => vec!["loadword", "callstd"],
        "giveitem" | "finditem" => vec!["setorcopyvar", "setorcopyvar", "callstd"],
        "givedecoration" => vec!["setorcopyvar", "callstd"],
        "register_matchcall" => vec!["setvar", "special", "setorcopyvar", "callstd"],
        "dofieldeffectsparkle" => vec![
            "setfieldeffectargument",
            "setfieldeffectargument",
            "setfieldeffectargument",
            "dofieldeffect",
        ],
        "braillemsgbox" => vec!["braillemessage", "waitbuttonpress", "closebraillemessage"],
        "seteventmon" => vec!["setvar", "setvar", "setvar", "special"],
        "switch" => vec!["copyvar"],
        "case" => vec!["compare", "goto_if"],
        "compare" => vec!["compare"],
        "goto_if_set" | "goto_if_unset" => vec!["checkflag", "goto_if"],
        "call_if_set" | "call_if_unset" => vec!["checkflag", "call_if"],
        "vgoto_if_set" | "vgoto_if_unset" => vec!["checkflag", "vgoto_if"],
        "goto_if_defeated" | "goto_if_not_defeated" => vec!["checktrainerflag", "goto_if"],
        "call_if_defeated" | "call_if_not_defeated" => vec!["checktrainerflag", "call_if"],
        "vgoto_if_eq" | "vgoto_if_ne" if n == 3 => vec!["compare", "vgoto_if"],
        "vgoto_if_eq" | "vgoto_if_ne" => vec!["vgoto_if"],
        _ if cc("goto_if_") && n == 3 => vec!["compare", "goto_if"],
        _ if cc("goto_if_") => vec!["goto_if"],
        _ if cc("call_if_") && n == 3 => vec!["compare", "call_if"],
        _ if cc("call_if_") => vec!["call_if"],
        m if m.starts_with("trainerbattle") => vec!["trainerbattle"],
        "applymovement" => vec![at("applymovement", "applymovementat", 2)],
        "waitmovement" => vec![at("waitmovement", "waitmovementat", 1)],
        "removeobject" => vec![at("removeobject", "removeobjectat", 1)],
        "addobject" => vec![at("addobject", "addobjectat", 1)],
        "showplayer" => vec!["showobjectat"],
        "hideplayer" => vec!["hideobjectat"],
        "map_script" | "map_script_2" | "pokemartlistend" | "brailleformat" => vec![],
        m => vec![scripts::op_name(scripts::opcode(m)?)?],
    })
}

/// `asm/macros/movement.inc`: `create_movement_action <macro>, MOVEMENT_ACTION_*`.
fn read_movement_macros(root: &Path) -> Result<BTreeMap<String, u8>> {
    let path = root.join("asm/macros/movement.inc");
    let src =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("create_movement_action ") else {
            continue;
        };
        let Some((mac, action)) = rest.split_once(',') else {
            continue;
        };
        let value = crate::names::MOVEMENT_ACTIONS
            .value(action.trim())
            .with_context(|| format!("unknown movement action {action}"))?;
        out.insert(mac.trim().to_string(), value as u8);
    }
    ensure!(
        out.contains_key("step_end"),
        "{}: no step_end",
        path.display()
    );
    Ok(out)
}

/// The fields of a `map.json` the transfer needs.
fn read_map_json(path: &Path) -> Result<MapJson> {
    #[derive(serde::Deserialize)]
    struct Raw {
        name: String,
        shared_scripts_map: Option<String>,
        shared_events_map: Option<String>,
        object_events: Option<Vec<Event>>,
        coord_events: Option<Vec<Event>>,
        bg_events: Option<Vec<Event>>,
    }
    #[derive(serde::Deserialize)]
    struct Event {
        script: Option<String>,
    }
    let raw: Raw = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let scripts = |events: Option<Vec<Event>>| -> Vec<Option<String>> {
        events
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.script.filter(|s| is_symbol(s)))
            .collect()
    };
    Ok(MapJson {
        name: raw.name,
        shared_scripts_map: raw.shared_scripts_map,
        shared_events_map: raw.shared_events_map,
        objects: scripts(raw.object_events),
        coords: scripts(raw.coord_events),
        bgs: scripts(raw.bg_events),
    })
}

/// The battle facility helper macro sources: fixed `setvar`/`copyvar`/`special` sequences.
pub const FACILITY_MACROS: [&str; 3] = [
    "asm/macros/battle_frontier",
    "asm/macros/trainer_hill.inc",
    "asm/macros/battle_tent.inc",
];

/// A helper macro: its parameters (with defaults) and body lines. Expanded by substituting the
/// arguments and evaluating its `.if`/`.elseif`/`.else` conditions, which only compare a
/// parameter with a number or a name (`==`, `!=`) or test `>= VARS_START`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MacroDef {
    params: Vec<(String, Option<String>)>,
    body: Vec<String>,
}

impl MacroDef {
    fn expand(&self, args: &[String]) -> Option<Vec<&'static str>> {
        let mut vals = BTreeMap::new();
        for (i, (p, default)) in self.params.iter().enumerate() {
            let v = args
                .get(i)
                .cloned()
                .or_else(|| default.clone())
                .unwrap_or_default();
            vals.insert(p.as_str(), v);
        }
        let subst = |t: &str| -> String {
            match t.strip_prefix('\\') {
                Some(p) => vals.get(p).cloned().unwrap_or_default(),
                None => t.to_string(),
            }
        };
        // Per open `.if`: (this branch is live, some branch was taken).
        let mut stack: Vec<(bool, bool)> = Vec::new();
        let mut out = Vec::new();
        for line in &self.body {
            let mut words = line.split_whitespace();
            let first = words.next()?;
            let rest: Vec<&str> = words.collect();
            let live = stack.iter().all(|(l, _)| *l);
            match first {
                ".if" => {
                    let c = live && eval(&rest, &subst)?;
                    stack.push((c, c));
                }
                ".elseif" => {
                    let (_, taken) = stack.pop()?;
                    let outer = stack.iter().all(|(l, _)| *l);
                    let c = outer && !taken && eval(&rest, &subst)?;
                    stack.push((c, taken || c));
                }
                ".else" => {
                    let (_, taken) = stack.pop()?;
                    let outer = stack.iter().all(|(l, _)| *l);
                    stack.push((outer && !taken, true));
                }
                ".endif" => {
                    stack.pop()?;
                }
                _ if !live => {}
                op => out.push(scripts::op_name(scripts::opcode(op)?)?),
            }
        }
        stack.is_empty().then_some(out)
    }
}

/// A `.if` condition `\a OP b`, OP one of `==`, `!=`, or `>= VARS_START`.
fn eval(words: &[&str], subst: &dyn Fn(&str) -> String) -> Option<bool> {
    let [a, op, b] = words else { return None };
    let (a, b) = (subst(a), subst(b));
    let num = |s: &str| match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => s.parse().ok(),
    };
    match *op {
        ">=" if b == "VARS_START" => Some(match num(&a) {
            Some(n) => n >= 0x4000,
            None => a.starts_with("VAR_"),
        }),
        "==" | "!=" => {
            let eq = match (num(&a), num(&b)) {
                (Some(x), Some(y)) => x == y,
                _ => a == b,
            };
            Some(eq == (*op == "=="))
        }
        _ => None,
    }
}

/// Every `.macro` of [`FACILITY_MACROS`].
fn read_facility_macros(root: &Path) -> Result<BTreeMap<String, MacroDef>> {
    let mut files = Vec::new();
    for rel in FACILITY_MACROS {
        let p = root.join(rel);
        if p.is_dir() {
            for e in read_dir_sorted(&p)? {
                files.push(p.join(e));
            }
        } else {
            files.push(p);
        }
    }
    let mut out = BTreeMap::new();
    for path in files {
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut cur: Option<(String, MacroDef)> = None;
        let mut in_comment = false;
        for raw in src.lines() {
            let line = strip_comments(raw, &mut in_comment);
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(".macro ") {
                let rest = rest.trim();
                let (name, params) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                let params = params
                    .split(',')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(|p| match p.split_once('=') {
                        Some((n, d)) => (n.trim().to_string(), Some(d.trim().to_string())),
                        None => (p.trim_end_matches(":req").to_string(), None),
                    })
                    .collect();
                let def = MacroDef {
                    params,
                    body: Vec::new(),
                };
                cur = Some((name.to_string(), def));
            } else if line == ".endm" {
                let (name, def) = cur.take().context(".endm without .macro")?;
                out.insert(name, def);
            } else if let Some((_, def)) = cur.as_mut()
                && !line.is_empty()
            {
                def.body.push(line.replace(',', " "));
            }
        }
    }
    ensure!(!out.is_empty(), "no battle facility macros found");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stmt(mac: &str, args: &[&str]) -> Stmt {
        Stmt {
            line: 1,
            mac: mac.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    fn targets(s: &Stmt) -> Vec<(RefKind, String)> {
        stmt_refs(s)
            .into_iter()
            .map(|r| (r.kind, r.target))
            .collect()
    }

    #[test]
    fn reference_table_covers_the_operand_forms() {
        use RefKind::*;
        let t = |k, s: &str| (k, s.to_string());
        assert_eq!(
            targets(&stmt("goto_if_eq", &["VAR_X", "1", "L"])),
            [t(Script, "L")]
        );
        assert_eq!(targets(&stmt("goto_if_eq", &["L"])), [t(Script, "L")]);
        assert_eq!(targets(&stmt("case", &["2", "L"])), [t(Script, "L")]);
        assert_eq!(
            targets(&stmt("msgbox", &["T", "MSGBOX_NPC"])),
            [t(Text, "T")]
        );
        assert_eq!(targets(&stmt("loadword", &["1", "T"])), []);
        assert_eq!(
            targets(&stmt(
                "trainerbattle_single",
                &["TR", "A", "B", "S", "NO_MUSIC"]
            )),
            [t(Text, "A"), t(Text, "B"), t(Script, "S")]
        );
        assert_eq!(
            targets(&stmt("trainerbattle_single", &["TR", "A", "B"])),
            [t(Text, "A"), t(Text, "B")]
        );
        assert_eq!(
            targets(&stmt(
                "trainerbattle",
                &["TRAINER_BATTLE_CONTINUE_SCRIPT", "TR", "1", "A", "B", "S"]
            )),
            [t(Text, "A"), t(Text, "B"), t(Script, "S")]
        );
        assert_eq!(
            targets(&stmt("map_script_2", &["VAR_X", "0", "S"])),
            [t(Script, "S")]
        );
        assert_eq!(
            targets(&stmt("applymovement", &["1", "M"])),
            [t(Movement, "M")]
        );
        assert_eq!(targets(&stmt("vgoto_if_eq", &["VAR_X", "1", "L"])), []);
        assert_eq!(targets(&stmt("setvar", &["VAR_X", "1"])), []);
    }

    #[test]
    fn expansion_follows_the_macros() {
        assert_eq!(
            expand(&stmt("msgbox", &["T"])).unwrap(),
            ["loadword", "callstd"]
        );
        assert_eq!(
            expand(&stmt("goto_if_set", &["F", "L"])).unwrap(),
            ["checkflag", "goto_if"]
        );
        assert_eq!(
            expand(&stmt("applymovement", &["1", "M", "MAP_X"])).unwrap(),
            ["applymovementat"]
        );
        assert_eq!(expand(&stmt("end", &[])).unwrap(), ["end"]);
        assert_eq!(expand(&stmt("frontier_set", &["A"])), None);
    }

    #[test]
    fn comments_and_labels() {
        let mut c = false;
        assert_eq!(strip_comments("\tend @ done", &mut c), "\tend ");
        assert_eq!(
            strip_comments(".string \"a@b\" @ x", &mut c),
            ".string \"a@b\" "
        );
        assert_eq!(strip_comments("a /* b", &mut c), "a ");
        assert!(c);
        assert_eq!(strip_comments("c */ d", &mut c), " d");
        assert_eq!(label_def("Foo_Bar::"), Some("Foo_Bar"));
        assert_eq!(label_def("Foo:"), Some("Foo"));
        assert_eq!(label_def(".string \"a:\""), None);
        assert_eq!(strip_comments("end // x", &mut c), "end ");
        assert_eq!(
            strip_comments(".string \"a//b\"", &mut c),
            ".string \"a//b\""
        );
    }

    #[test]
    fn facility_macros_expand_their_conditions() {
        let def = MacroDef {
            params: vec![
                ("data".to_string(), None),
                ("val".to_string(), Some("0xFFFF".to_string())),
            ],
            body: [
                "setvar VAR_0x8004 FRONTIER_UTIL_FUNC_SET_DATA",
                "setvar VAR_0x8005 \\data",
                ".if \\val == 0xFFFF",
                ".elseif \\val >= VARS_START",
                "copyvar VAR_0x8006 \\val",
                ".else",
                "setvar VAR_0x8006 \\val",
                ".endif",
                "special CallFrontierUtilFunc",
            ]
            .map(String::from)
            .to_vec(),
        };
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            def.expand(&args(&["D"])).unwrap(),
            ["setvar", "setvar", "special"]
        );
        assert_eq!(
            def.expand(&args(&["D", "VAR_RESULT"])).unwrap(),
            ["setvar", "setvar", "copyvar", "special"]
        );
        assert_eq!(
            def.expand(&args(&["D", "3"])).unwrap(),
            ["setvar", "setvar", "setvar", "special"]
        );
    }
}
