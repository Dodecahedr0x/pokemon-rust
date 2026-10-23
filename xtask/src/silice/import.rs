//! `cargo xtask silice import <contenu-jeu> [--out games/silice] [--force] [--report FILE]`: the
//! one-time bootstrap of `games/silice/` from the chapters (`chapitres/*.md`). Deterministic:
//! the same chapters and the same existing `ids/` give the same bytes. Nothing is guessed; what
//! the source leaves open goes to the report (default `target/silice/import-report.md`).

use super::md::{self, Chapter, RonEntry, Table};
use super::names::{self, Word};
use super::schema::*;
use super::{ids, read_ron, write_files};
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub struct Options {
    pub src: PathBuf,
    pub out: PathBuf,
    pub force: bool,
    /// Where the report goes; `None` writes it as `<out>/IMPORT-REPORT.md` with the data (and
    /// under the same `--force` rule).
    pub report: Option<PathBuf>,
    /// The authored maps whose flag, var and song names are allocated too; `None` is
    /// `<out>/maps`.
    pub maps: Option<PathBuf>,
}

/// The report's file name in the games dir.
pub const REPORT: &str = "IMPORT-REPORT.md";

/// The import report: named sections of lines, written in a fixed order.
#[derive(Default)]
pub struct Report {
    sections: BTreeMap<&'static str, Vec<String>>,
}

impl Report {
    fn add(&mut self, section: &'static str, line: String) {
        self.sections.entry(section).or_default().push(line);
    }

    fn render(&self, counts: &[(String, usize)]) -> String {
        let mut s = String::from("# Silice import report\n\n## Counts\n\n");
        for (k, v) in counts {
            let _ = writeln!(s, "- {k}: {v}");
        }
        for (name, lines) in &self.sections {
            let _ = write!(s, "\n## {name}\n\n");
            for l in lines {
                let _ = writeln!(s, "- {l}");
            }
        }
        s
    }
}

/// Everything the import reads from the chapters, before allocation.
#[derive(Default)]
pub struct Source {
    pub chapters: Vec<String>,
    /// Map constant -> first chapter defining it.
    pub maps: BTreeMap<String, String>,
    /// Text id -> definitions in chapter order: (chapter, block).
    pub texts: BTreeMap<String, Vec<(String, md::TextBlock)>>,
    pub trainers: Vec<Trainer>,
    pub encounters: EncountersFile,
    pub placements: Vec<ItemPlacement>,
    pub marts: Vec<Mart>,
    pub item_defs: BTreeMap<String, ItemDef>,
    pub refs: RefsFile,
}

pub fn run(o: &Options) -> Result<()> {
    let mut report = Report::default();
    let src = parse(&o.src, &mut report)?;
    let maps_dir = o.maps.clone().unwrap_or_else(|| o.out.join("maps"));
    let maps = super::maps::scan_dir(&maps_dir)?;
    let (mut files, counts) = build(&src, &o.out, &maps, &mut report)?;
    let rendered = report.render(&counts);
    let report_path = match &o.report {
        Some(p) => {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(p, &rendered).with_context(|| format!("writing {}", p.display()))?;
            p.clone()
        }
        None => {
            files.insert(REPORT.to_string(), rendered);
            o.out.join(REPORT)
        }
    };
    let changed = write_files(&o.out, &files, o.force)?;
    for (k, v) in &counts {
        println!("{k}: {v}");
    }
    println!(
        "{} files written, {} unchanged, under {}; report: {}",
        changed,
        files.len() - changed,
        o.out.display(),
        report_path.display()
    );
    Ok(())
}

fn chapter_files(src: &Path) -> Result<Vec<PathBuf>> {
    let dir = src.join("chapitres");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    v.sort();
    if v.is_empty() {
        bail!("no chapter in {}", dir.display());
    }
    Ok(v)
}

/// Prefixes of the names Silice defines itself.
const SILICE_PREFIXES: &[&str] = &[
    "FLAG_SIL_",
    "VAR_SIL_",
    "ITEM_SIL_",
    "TRAINER_SIL_",
    "MOVE_SIL_",
    "MUS_SIL_",
    "SE_SIL_",
    "MAP_",
];

static MAP_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"MAP_[A-Z0-9_]*[A-Z0-9]").unwrap());
static QTY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*[×x]\s*([0-9]+)\s*$").unwrap());
static POS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([0-9]+),\s*([0-9]+)\)").unwrap());
static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"ITEM_[A-Z0-9_]*[A-Z0-9]").unwrap());
static BASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)base[^:\n]{0,20}:\s*\**`?(ITEM_[A-Z0-9_]*[A-Z0-9])").unwrap()
});
static INLINE_MART: LazyLock<Regex> = LazyLock::new(|| {
    // `(`ITEM_X`, 200)` or `(ITEM_X, 200)`.
    Regex::new(r"`?(ITEM_[A-Z0-9_]*[A-Z0-9])`?\s*,\s*([0-9][0-9 ]*[0-9]|[0-9])\s*\)").unwrap()
});
static TEXT_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:msgbox|show_text_on_black|show_book|show_log)\(([^,)]*)").unwrap()
});
static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>|\[[^\]]*\]").unwrap());
static TEXT_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9<>\[\]…]+)+").unwrap());

/// Reads the chapters.
pub fn parse(src: &Path, report: &mut Report) -> Result<Source> {
    let chapters: Vec<Chapter> = chapter_files(src)?
        .iter()
        .map(|p| Chapter::read(p))
        .collect::<Result<_>>()?;
    let mut s = Source {
        chapters: chapters.iter().map(|c| c.stem.clone()).collect(),
        ..Source::default()
    };

    // Maps: the constants in the headings of §2 and §4 (literal ones first, for parents).
    let heading_lines = |c: &Chapter| -> Vec<usize> {
        c.sections()
            .into_iter()
            .filter(|(n, _)| *n == 2 || *n == 4)
            .flat_map(|(_, r)| r)
            .filter(|&i| c.lines[i].starts_with('#'))
            .collect()
    };
    let mut literal = BTreeSet::new();
    for c in &chapters {
        for i in heading_lines(c) {
            for m in MAP_TOKEN.find_iter(&c.lines[i]) {
                literal.insert(m.as_str().to_string());
            }
        }
    }
    for c in &chapters {
        for i in heading_lines(c) {
            for w in names::scan_line(&c.lines[i], &literal) {
                match w {
                    Word::Names(v) => {
                        for n in v.into_iter().filter(|n| n.starts_with("MAP_")) {
                            if !literal.contains(&n) {
                                report.add(
                                    "Map families expanded",
                                    format!("{}:{}: {n}", c.stem, i + 1),
                                );
                            }
                            s.maps.entry(n).or_insert_with(|| c.stem.clone());
                        }
                    }
                    Word::Dynamic(p) | Word::Wildcard(p) => report.add(
                        "Map headings not imported",
                        format!("{}:{}: `{p}`", c.stem, i + 1),
                    ),
                }
            }
        }
    }
    let map_set: BTreeSet<String> = s.maps.keys().cloned().collect();

    // Texts, trainers and encounters; which lines are data (not scanned for references).
    let mut skip: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    let mut scripts: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    for c in &chapters {
        let mut sk = vec![false; c.lines.len()];
        let mut sc = vec![false; c.lines.len()];
        let sections = c.sections();
        let section_of = |line: usize| {
            sections
                .iter()
                .find(|(_, r)| r.contains(&line))
                .map(|(n, _)| *n)
        };
        for f in c.fences() {
            let body = c.fence_body(&f);
            if f.lang == "ron" {
                match section_of(f.open) {
                    Some(5) => {
                        trainers(c, &body, &mut s, report)?;
                        sk[f.open..=f.close].fill(true);
                    }
                    Some(6) => {
                        encounters(c, &body, &map_set, &mut s, report)?;
                        sk[f.open..=f.close].fill(true);
                    }
                    n => report.add(
                        "RON blocks not imported",
                        format!(
                            "{}:{}: a `ron` block in §{}",
                            c.stem,
                            f.open + 1,
                            n.unwrap_or(0)
                        ),
                    ),
                }
                continue;
            }
            let mut warn = |line: usize, m: String| {
                report.add("Text warnings", format!("{}:{}: {m}", c.stem, line + 1))
            };
            if f.lang != "text" {
                continue;
            }
            match md::text_blocks(&body, &mut warn) {
                Some(blocks) => {
                    sk[f.open..=f.close].fill(true);
                    for b in blocks {
                        s.texts
                            .entry(b.id.clone())
                            .or_default()
                            .push((c.stem.clone(), b));
                    }
                }
                None => sc[f.open..=f.close].fill(true),
            }
        }
        skip.insert(c.stem.clone(), sk);
        scripts.insert(c.stem.clone(), sc);
    }

    // Items (§7 tables) and marts.
    for c in &chapters {
        for (n, r) in c.sections() {
            if n == 7 {
                items_section(c, r, &mut s, report);
            }
        }
    }

    // References everywhere else.
    let defined_texts: BTreeSet<&str> = s.texts.keys().map(String::as_str).collect();
    let mut dynamic: BTreeMap<(String, RefKind), BTreeSet<String>> = BTreeMap::new();
    let mut text_patterns: Vec<(String, String)> = Vec::new();
    for c in &chapters {
        let sk = &skip[&c.stem];
        let sc = &scripts[&c.stem];
        for (i, l) in c.lines.iter().enumerate() {
            if sk[i] {
                continue;
            }
            for w in names::scan_line(l, &map_set) {
                match w {
                    Word::Names(v) => {
                        for n in v {
                            s.refs.constants.entry(n).or_default().push(c.stem.clone());
                        }
                    }
                    Word::Dynamic(p) => {
                        dynamic
                            .entry((p, RefKind::Constant))
                            .or_default()
                            .insert(c.stem.clone());
                    }
                    Word::Wildcard(_) => {}
                }
            }
            let in_table = l.trim_start().starts_with('|');
            if sc[i] || in_table {
                for t in TEXT_TOKEN.find_iter(l) {
                    if defined_texts.contains(t.as_str()) {
                        s.refs
                            .texts
                            .entry(t.as_str().to_string())
                            .or_default()
                            .push(c.stem.clone());
                    }
                }
            }
            if sc[i] {
                for cap in TEXT_CALL.captures_iter(l) {
                    let arg = cap[1].trim();
                    if arg.is_empty() || arg.contains('(') {
                        continue;
                    }
                    // An elided id in a note (`msgbox(…, Sign)`, `msgbox(..._Repli)`).
                    if arg.starts_with('…') || arg.starts_with("...") {
                        report.add(
                            "Elided references",
                            format!("{}:{}: `{arg}`", c.stem, i + 1),
                        );
                        continue;
                    }
                    if arg.contains('|') {
                        let parts: Vec<&str> = arg.split('|').map(str::trim).collect();
                        let first = parts[0];
                        let stem = first.rsplit_once('_').map_or("", |(a, _)| a);
                        let ids: Vec<String> = parts
                            .iter()
                            .map(|p| {
                                if p.contains('_') || stem.is_empty() {
                                    p.to_string()
                                } else {
                                    format!("{stem}_{p}")
                                }
                            })
                            .collect();
                        dynamic
                            .entry((arg.to_string(), RefKind::Text))
                            .or_default()
                            .insert(c.stem.clone());
                        text_patterns.push((arg.to_string(), ids.join(" ")));
                        continue;
                    }
                    if names::pattern_regex(arg, false).is_some() {
                        dynamic
                            .entry((arg.to_string(), RefKind::Text))
                            .or_default()
                            .insert(c.stem.clone());
                    } else if arg.contains('_') && !arg.chars().all(|c| !c.is_ascii_lowercase()) {
                        s.refs
                            .texts
                            .entry(arg.to_string())
                            .or_default()
                            .push(c.stem.clone());
                    }
                }
                // A text pattern in another argument (`battle_trainer(X, Gym2_<P>_Defeat)`).
                for t in TEXT_TOKEN.find_iter(l) {
                    let t = t.as_str();
                    let outside = PLACEHOLDER.replace_all(t, "");
                    if !names::has_prefix(t)
                        && outside.chars().any(|c| c.is_ascii_lowercase())
                        && t.contains(['<', '['])
                        && names::pattern_regex(t, false).is_some()
                    {
                        dynamic
                            .entry((t.to_string(), RefKind::Text))
                            .or_default()
                            .insert(c.stem.clone());
                    }
                }
            }
        }
    }
    for v in s
        .refs
        .constants
        .values_mut()
        .chain(s.refs.texts.values_mut())
    {
        v.sort();
        v.dedup();
    }
    let enumerated: BTreeMap<String, String> = text_patterns.into_iter().collect();
    for ((pattern, kind), chapters) in dynamic {
        let expanded: Vec<String> = match (kind, enumerated.get(&pattern)) {
            (RefKind::Text, Some(list)) => list.split(' ').map(str::to_string).collect(),
            (RefKind::Text, None) => match names::pattern_regex(&pattern, false) {
                Some(re) => s.texts.keys().filter(|t| re.is_match(t)).cloned().collect(),
                None => Vec::new(),
            },
            // Only Silice's own names expand; a Gen3 family (`SPECIES_<NOM>`) stays unresolved.
            (RefKind::Constant, _) => match names::pattern_regex(&pattern, true) {
                Some(re) if SILICE_PREFIXES.iter().any(|p| pattern.starts_with(p)) => s
                    .refs
                    .constants
                    .keys()
                    .chain(s.trainers.iter().map(|t| &t.id))
                    .chain(s.maps.keys())
                    .filter(|t| re.is_match(t))
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
        };
        report.add(
            "Dynamic references",
            if expanded.is_empty() {
                format!(
                    "`{pattern}` ({}): unresolved",
                    chapters.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            } else {
                format!("`{pattern}` -> {}", expanded.join(", "))
            },
        );
        s.refs.dynamic.push(DynamicRef {
            pattern,
            kind,
            chapters: chapters.into_iter().collect(),
            expanded,
        });
    }
    item_definitions(&chapters, &mut s, report);
    Ok(s)
}

fn trainers(
    c: &Chapter,
    body: &[(usize, String)],
    s: &mut Source,
    report: &mut Report,
) -> Result<()> {
    for e in md::ron_entries(body) {
        if e.key.is_some() {
            bail!(
                "{}:{}: a keyed entry among the trainers",
                c.stem,
                e.line + 1
            );
        }
        let mut t: Trainer = ron::from_str(&e.value)
            .with_context(|| format!("{}:{}: trainer entry", c.stem, e.line + 1))?;
        t.notes = e.comments.clone();
        if !t.items.is_empty() {
            report.add("Trainer item bags", format!("{}: {:?}", t.id, t.items));
        }
        s.trainers.push(t);
    }
    Ok(())
}

fn encounters(
    c: &Chapter,
    body: &[(usize, String)],
    maps: &BTreeSet<String>,
    s: &mut Source,
    report: &mut Report,
) -> Result<()> {
    for RonEntry {
        key,
        value,
        comments,
        line,
    } in md::ron_entries(body)
    {
        let Some(key) = key else {
            bail!(
                "{}:{}: an encounter entry without its map",
                c.stem,
                line + 1
            );
        };
        let mut h: EncounterHeader = ron::from_str(&value)
            .with_context(|| format!("{}:{}: encounters of {key}", c.stem, line + 1))?;
        h.notes = comments;
        let name = names::map_name(&key, maps);
        let v = s.encounters.maps.entry(name.clone()).or_default();
        if !v.is_empty() {
            report.add(
                "Encounters",
                format!(
                    "{}:{}: {key} has a second table; kept as header {}",
                    c.stem,
                    line + 1,
                    v.len()
                ),
            );
        }
        if !maps.contains(&key) {
            report.add(
                "Encounters",
                format!(
                    "{}:{}: {key} is not a map heading (name {name})",
                    c.stem,
                    line + 1
                ),
            );
        }
        v.push(h);
    }
    Ok(())
}

fn is_constant(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn first_item(cell: &str) -> Option<String> {
    ITEM.find(cell).map(|m| m.as_str().to_string())
}

/// The number a cell starts with (`1 200`, `300 (note)`).
fn price(cell: &str) -> Option<u32> {
    static LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([0-9][0-9 ]*)").unwrap());
    let c = LEAD.captures(cell)?;
    c[1].chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .parse()
        .ok()
}

/// A mart label without markdown (`**`, backticks) or the trailing colon.
fn mart_label(raw: &str) -> String {
    clean(&raw.replace('`', ""))
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_string()
}

/// The parenthetical of a label that names a flag or var: when the shop is open.
fn mart_condition(label: &str) -> Option<String> {
    static PAREN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([^()]*)\)").unwrap());
    PAREN
        .captures_iter(label)
        .map(|c| c[1].trim().to_string())
        .find(|p| p.contains("FLAG_") || p.contains("VAR_"))
}

/// The label of a table (the closest heading or bold line above it) and the prose between the
/// label and the table.
fn label_above(c: &Chapter, line: usize, stop: usize) -> (String, Option<String>) {
    let mut prose: Vec<&str> = Vec::new();
    for i in (stop..line).rev() {
        let l = c.lines[i].trim();
        if l.starts_with('#') {
            prose.reverse();
            return (mart_label(l.trim_start_matches('#')), note_of(&prose));
        }
        if l.starts_with("**") {
            prose.reverse();
            return (mart_label(l), note_of(&prose));
        }
        if !l.is_empty() {
            prose.push(l);
        }
    }
    (String::new(), None)
}

fn note_of(lines: &[&str]) -> Option<String> {
    let n = clean(&lines.join(" ").replace('`', ""));
    let n = n.trim().trim_end_matches(':').trim().to_string();
    (!n.is_empty()).then_some(n)
}

/// A price that fits Gen3's `u16`.
fn price16(cell: &str) -> Option<u16> {
    price(cell).and_then(|p| u16::try_from(p).ok())
}

fn items_section(c: &Chapter, r: std::ops::Range<usize>, s: &mut Source, report: &mut Report) {
    for t in c.tables(r.clone()) {
        let item_col = t.col("ITEM_*");
        let price_col = t
            .col("Prix")
            .map(|k| (k, Currency::Money))
            .or_else(|| t.col("Jetons").map(|k| (k, Currency::Coins)));
        if let (Some(fc), Some(mc), Some(wc), Some(nc), Some(ic), Some(hc)) = (
            t.col("Flag"),
            t.col("Carte"),
            t.col("Où"),
            t.col("Objet (FR)"),
            item_col,
            t.col("Méthode"),
        ) {
            placements(c, &t, [fc, mc, wc, nc, ic, hc], t.col("Note"), s, report);
        } else if let (Some(ic), Some((pc, currency))) = (item_col, price_col) {
            let (label, note) = label_above(c, t.line, r.start);
            let note_col = t.col("Note").or_else(|| t.col("Pourquoi"));
            let cond_col = t.col("Condition");
            let text = |row: &Vec<String>, k: Option<usize>| {
                k.and_then(|k| row.get(k))
                    .map(|x| clean(&x.replace('`', "")))
                    .filter(|x| !x.is_empty())
            };
            let mut stock = Vec::new();
            for (ln, row) in &t.rows {
                match (
                    row.get(ic).and_then(|x| first_item(x)),
                    row.get(pc).and_then(|x| price16(x)),
                ) {
                    (Some(item), Some(p)) => stock.push(MartItem {
                        item,
                        price: p,
                        condition: text(row, cond_col),
                        note: text(row, note_col),
                    }),
                    _ => report.add(
                        "Mart rows not imported",
                        format!("{}:{}: {}", c.stem, ln + 1, row.join(" | ")),
                    ),
                }
            }
            s.marts.push(Mart {
                chapter: c.stem.clone(),
                map: MAP_TOKEN.find(&label).map(|m| m.as_str().to_string()),
                condition: mart_condition(&label),
                label,
                note,
                currency,
                stock,
            });
        } else if t.col("Boutique").is_some() || t.col("Inventaire (prix Gen3)").is_some() {
            for (ln, row) in &t.rows {
                report.add(
                    "Mart rows not imported",
                    format!(
                        "{}:{}: {} (stock in prose, enter by hand)",
                        c.stem,
                        ln + 1,
                        row.join(" | ")
                    ),
                );
            }
        } else if item_col.is_none() {
            report.add(
                "§7 tables not imported",
                format!("{}:{}: {}", c.stem, t.line + 1, t.header.join(" | ")),
            );
        }
    }
    // Shops written inline: a bold label followed by `(ITEM_X, price)` pairs. A bold paragraph
    // that names an item but gives no such pair is reported.
    let fenced = c.fenced_lines();
    let mut i = r.start;
    while i < r.end {
        let l = c.lines[i].trim_start();
        if !fenced[i] && l.starts_with("**") && !l.starts_with('|') {
            let mut para = String::new();
            let mut k = i;
            while k < r.end
                && !c.lines[k].trim().is_empty()
                && !c.lines[k].trim_start().starts_with('|')
            {
                para.push_str(c.lines[k].trim());
                para.push(' ');
                k += 1;
            }
            let caps: Vec<_> = INLINE_MART.captures_iter(&para).collect();
            let stock: Vec<MartItem> = caps
                .iter()
                .filter_map(|cap| {
                    Some(MartItem {
                        item: cap[1].to_string(),
                        price: price16(&cap[2])?,
                        condition: None,
                        note: None,
                    })
                })
                .collect();
            if !stock.is_empty() {
                let rest = para.trim().trim_start_matches("**");
                let label = mart_label(rest.split("**").next().unwrap_or(""));
                let tail = &para[caps.last().unwrap().get(0).unwrap().end()..];
                let tail = tail.trim_start_matches(['.', ',', ';', ' ']);
                s.marts.push(Mart {
                    chapter: c.stem.clone(),
                    map: MAP_TOKEN.find(&label).map(|m| m.as_str().to_string()),
                    condition: mart_condition(&label),
                    label,
                    note: note_of(&[tail]),
                    currency: Currency::Money,
                    stock,
                });
            } else if ITEM.is_match(&para) {
                report.add(
                    "Bold paragraphs naming items without a mart",
                    format!("{}:{}: {}", c.stem, i + 1, clean(&para)),
                );
            }
            i = k.max(i + 1);
        } else {
            i += 1;
        }
    }
}

fn placements(
    c: &Chapter,
    t: &Table,
    [fc, mc, wc, nc, ic, hc]: [usize; 6],
    note: Option<usize>,
    s: &mut Source,
    report: &mut Report,
) {
    for (ln, row) in &t.rows {
        let cell = |k: usize| {
            row.get(k)
                .map(String::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let flag = cell(fc).trim_matches('`').to_string();
        let map = cell(mc).trim_matches('`').to_string();
        if !map.starts_with("MAP_") {
            report.add(
                "Item placements",
                format!(
                    "{}:{}: map cell {map:?} is not a MAP_ constant",
                    c.stem,
                    ln + 1
                ),
            );
        }
        let place = cell(wc);
        let pos = {
            let all: Vec<_> = POS.captures_iter(&place).collect();
            (all.len() == 1).then(|| {
                (
                    all[0][1].parse().unwrap_or(0),
                    all[0][2].parse().unwrap_or(0),
                )
            })
        };
        let raw_name = cell(nc);
        let (name, quantity) = match QTY.captures(&raw_name) {
            Some(q) => (
                raw_name[..q.get(0).unwrap().start()].trim().to_string(),
                q[1].parse().unwrap_or(1),
            ),
            None => (raw_name.clone(), 1),
        };
        let items: Vec<String> = ITEM
            .find_iter(&cell(ic))
            .map(|m| m.as_str().to_string())
            .collect();
        // `A + B` with one id each: one placement per item.
        let parts: Vec<String> = name.split(" + ").map(|p| p.trim().to_string()).collect();
        let named: Vec<(String, Option<String>)> = if items.len() > 1 && parts.len() == items.len()
        {
            parts.into_iter().zip(items.into_iter().map(Some)).collect()
        } else {
            if items.len() > 1 {
                report.add(
                    "Item placements",
                    format!(
                        "{}:{}: {} items for the name {name:?}: one placement each, same name",
                        c.stem,
                        ln + 1,
                        items.len()
                    ),
                );
            }
            if items.is_empty() {
                vec![(name.clone(), None)]
            } else {
                items.into_iter().map(|i| (name.clone(), Some(i))).collect()
            }
        };
        let flag = if is_constant(&flag, "FLAG_") {
            Some(flag)
        } else {
            if flag.starts_with("FLAG_") {
                report.add(
                    "Item placements",
                    format!(
                        "{}:{}: flag cell {flag:?} is not one flag; left unset",
                        c.stem,
                        ln + 1
                    ),
                );
            }
            None
        };
        let method_text = cell(hc);
        let lower = method_text.to_lowercase();
        let method = if lower.starts_with("ball") {
            PlacementMethod::Ball
        } else if lower.starts_with("caché") {
            PlacementMethod::Hidden
        } else if lower.starts_with("donné") {
            PlacementMethod::Given
        } else if lower.starts_with("récompense") {
            PlacementMethod::Reward
        } else if lower.starts_with("boutique") {
            PlacementMethod::Shop
        } else {
            report.add(
                "Item placements",
                format!(
                    "{}:{}: method {method_text:?} is not one of the conventions' methods",
                    c.stem,
                    ln + 1
                ),
            );
            PlacementMethod::Other
        };
        let note = note.map(&cell).filter(|n| !n.is_empty());
        for (name, item) in named {
            s.placements.push(ItemPlacement {
                chapter: c.stem.clone(),
                flag: flag.clone(),
                map: map.clone(),
                place: place.clone(),
                pos,
                name,
                item,
                quantity,
                method,
                method_text: method_text.clone(),
                note: note.clone(),
            });
        }
    }
}

/// A name without its parenthetical notes, lowercase, for comparisons.
fn bare(s: &str) -> String {
    static PAREN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\([^)]*\)").unwrap());
    PAREN.replace_all(s, "").trim().to_lowercase()
}

fn clean(s: &str) -> String {
    s.replace("**", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `ITEM_SIL_*` definitions: names, pockets, prices, bases and effects found in the tables and
/// the "Objet inventé" paragraphs.
fn item_definitions(chapters: &[Chapter], s: &mut Source, report: &mut Report) {
    let mut ids: BTreeSet<String> = s
        .refs
        .constants
        .keys()
        .filter(|k| k.starts_with("ITEM_SIL_"))
        .cloned()
        .collect();
    for t in &s.trainers {
        for m in &t.party {
            if let Some(i) = m.item.as_ref().filter(|i| i.starts_with("ITEM_SIL_")) {
                ids.insert(i.clone());
            }
        }
    }
    ids.extend(
        s.placements
            .iter()
            .filter_map(|p| p.item.clone())
            .filter(|i| i.starts_with("ITEM_SIL_")),
    );
    ids.extend(
        s.marts
            .iter()
            .flat_map(|m| m.stock.iter().map(|x| x.item.clone()))
            .filter(|i| i.starts_with("ITEM_SIL_")),
    );
    let mut defs: BTreeMap<String, ItemDef> = ids
        .iter()
        .map(|id| {
            (
                id.clone(),
                ItemDef {
                    id: id.clone(),
                    name: None,
                    pocket: None,
                    price: None,
                    clone_of: None,
                    effect: None,
                    chapters: s.refs.constants.get(id).cloned().unwrap_or_default(),
                },
            )
        })
        .collect();
    let mut conflicts: BTreeSet<String> = BTreeSet::new();
    let mut set_name = |defs: &mut BTreeMap<String, ItemDef>, id: &str, name: &str, at: String| {
        let Some(d) = defs.get_mut(id) else { return };
        let name = clean(name);
        if name.is_empty() {
            return;
        }
        match &d.name {
            None => d.name = Some(name),
            Some(n) if bare(n) != bare(&name) => {
                conflicts.insert(format!("{id}: {n:?} kept, {name:?} at {at}"));
            }
            _ => {}
        }
    };
    let mark = |d: &mut ItemDef, text: &str| {
        if d.clone_of.is_none() {
            d.clone_of = BASE.captures(text).map(|c| c[1].to_string());
        }
        let lower = text.to_lowercase();
        if d.pocket.is_none()
            && (lower.contains("objet clé")
                || lower.contains("(clé")
                || lower.contains("objet clé"))
        {
            d.pocket = Some(extract::schema::ItemPocket::KeyItems);
        }
    };
    // Definition tables: `ITEM_*`, `Nom`, `Type`, `Effet`, `Objet Gen3 à cloner`.
    for c in chapters {
        for t in c.tables(0..c.lines.len()) {
            let (Some(ic), Some(nc)) = (t.col("ITEM_*"), t.col("Nom")) else {
                continue;
            };
            for (ln, row) in &t.rows {
                let items: Vec<String> = ITEM
                    .find_iter(&row[ic])
                    .map(|m| m.as_str().to_string())
                    .collect();
                let names: Vec<&str> = row
                    .get(nc)
                    .map(|n| n.split(", ").collect())
                    .unwrap_or_default();
                if items.len() != names.len() {
                    report.add(
                        "Item definitions",
                        format!(
                            "{}:{}: {} ids for {} names, row skipped: {}",
                            c.stem,
                            ln + 1,
                            items.len(),
                            names.len(),
                            row.join(" | ")
                        ),
                    );
                    continue;
                }
                for (id, name) in items.iter().zip(names) {
                    set_name(&mut defs, id, name, format!("{}:{}", c.stem, ln + 1));
                    let Some(d) = defs.get_mut(id) else { continue };
                    let ty = t
                        .col("Type")
                        .and_then(|k| row.get(k))
                        .map(String::as_str)
                        .unwrap_or("");
                    if d.pocket.is_none() && ty.to_lowercase().contains("clé") {
                        d.pocket = Some(extract::schema::ItemPocket::KeyItems);
                    }
                    if let Some(e) = t.col("Effet").and_then(|k| row.get(k)) {
                        d.effect.get_or_insert_with(|| clean(e));
                    }
                    if let Some(b) = t
                        .col("Objet Gen3 à cloner")
                        .and_then(|k| row.get(k))
                        .and_then(|x| first_item(x))
                    {
                        d.clone_of.get_or_insert(b);
                    }
                }
            }
        }
    }
    // Placements and marts: names, prices, "Objet inventé" notes.
    for p in &s.placements {
        let Some(id) = p.item.as_ref().filter(|i| defs.contains_key(*i)) else {
            continue;
        };
        set_name(&mut defs, id, &p.name, p.chapter.clone());
        if let Some(n) = &p.note
            && n.to_lowercase().contains("inventé")
        {
            let d = defs.get_mut(id).unwrap();
            mark(d, n);
            if d.effect.is_none() {
                let e = clean(n);
                let e = e
                    .split_once(':')
                    .map(|(_, r)| r.trim().to_string())
                    .unwrap_or(e);
                if !e.is_empty() && !e.starts_with("Objet inventé") {
                    d.effect = Some(e);
                }
            }
        }
    }
    for c in chapters {
        for t in c.tables(0..c.lines.len()) {
            let (Some(ic), Some(nc), Some(pc)) = (t.col("ITEM_*"), t.col("Objet"), t.col("Prix"))
            else {
                continue;
            };
            for (ln, row) in &t.rows {
                let Some(id) = row
                    .get(ic)
                    .and_then(|x| first_item(x))
                    .filter(|i| defs.contains_key(i))
                else {
                    continue;
                };
                set_name(&mut defs, &id, &row[nc], format!("{}:{}", c.stem, ln + 1));
                let d = defs.get_mut(&id).unwrap();
                if let Some(p) = row
                    .get(pc)
                    .and_then(|x| price(x))
                    .and_then(|p| u16::try_from(p).ok())
                {
                    match d.price {
                        None => d.price = Some(p),
                        Some(q) if q != p => report.add(
                            "Item definitions",
                            format!("{id}: price {q} kept, {p} at {}:{}", c.stem, ln + 1),
                        ),
                        _ => {}
                    }
                }
            }
        }
    }
    // "Objet inventé" paragraphs and list items naming one id.
    for c in chapters {
        let fenced = c.fenced_lines();
        let mut in_block = false;
        let mut i = 0;
        while i < c.lines.len() {
            let l = c.lines[i].trim();
            if fenced[i] || l.starts_with('|') {
                i += 1;
                continue;
            }
            if l.starts_with('#') || l.is_empty() && !in_block {
                in_block = false;
            }
            let lower = l.to_lowercase();
            if lower.starts_with("**objets inventés") {
                in_block = true;
                i += 1;
                continue;
            }
            let starts_def =
                lower.starts_with("**objet inventé") || (in_block && l.starts_with("- "));
            if !starts_def {
                if l.is_empty() {
                    in_block = in_block
                        && i + 1 < c.lines.len()
                        && c.lines[i + 1].trim_start().starts_with("- ");
                }
                i += 1;
                continue;
            }
            let mut text = l.to_string();
            let mut k = i + 1;
            while k < c.lines.len() {
                let n = &c.lines[k];
                if n.trim().is_empty()
                    || n.trim_start().starts_with("- ")
                    || n.trim_start().starts_with('|')
                    || n.starts_with('#')
                {
                    break;
                }
                text.push(' ');
                text.push_str(n.trim());
                k += 1;
            }
            let found: BTreeSet<String> = ITEM
                .find_iter(&text)
                .map(|m| m.as_str().to_string())
                .filter(|x| x.starts_with("ITEM_SIL_"))
                .collect();
            if found.len() == 1 {
                let id = found.into_iter().next().unwrap();
                if let Some(d) = defs.get_mut(&id) {
                    mark(d, &text);
                    if d.effect.is_none() {
                        d.effect = Some(clean(&text));
                    }
                }
                if let Some(name) = text
                    .strip_prefix("- **")
                    .and_then(|r| r.split_once("**"))
                    .map(|(n, _)| n.to_string())
                {
                    set_name(&mut defs, &id, &name, format!("{}:{}", c.stem, i + 1));
                }
            } else if !found.is_empty() {
                report.add(
                    "Item definitions",
                    format!(
                        "{}:{}: a definition paragraph naming {} items, not used",
                        c.stem,
                        i + 1,
                        found.len()
                    ),
                );
            }
            i = k;
        }
    }
    for x in conflicts {
        report.add("Item definitions", format!("name conflict: {x}"));
    }
    for d in defs.values() {
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
            report.add(
                "Items left open",
                format!("{}: no {}", d.id, missing.join(", no ")),
            );
        }
    }
    s.item_defs = defs;
}

/// Relative path -> file contents.
pub type Files = BTreeMap<String, String>;
/// `(label, count)` in report order.
pub type Counts = Vec<(String, usize)>;

/// Allocation, files and counts.
pub fn build(
    s: &Source,
    out: &Path,
    map_files: &[super::maps::MapRefs],
    report: &mut Report,
) -> Result<(Files, Counts)> {
    let mut files = BTreeMap::new();
    let maps: BTreeSet<String> = s.maps.keys().cloned().collect();

    // Texts.
    let mut entries: Vec<(&str, &str)> = Vec::new();
    let mut speakers = SpeakersFile::default();
    let mut dup = 0;
    for (id, defs) in &s.texts {
        let mut bodies: Vec<&str> = Vec::new();
        for (ch, b) in defs {
            if bodies.contains(&b.body.as_str()) {
                report.add(
                    "Duplicate text ids",
                    format!("{id}: same text again in {ch}:{}, kept once", b.line + 1),
                );
                continue;
            }
            if !bodies.is_empty() {
                dup += 1;
                report.add(
                    "Duplicate text ids",
                    format!(
                        "{id}: a different text in {ch}:{}; both are written (rule text-duplicate)",
                        b.line + 1
                    ),
                );
            }
            bodies.push(&b.body);
            entries.push((id, &b.body));
            speakers.texts.push(TextSource {
                id: id.clone(),
                chapter: ch.clone(),
                speaker: b.speaker.clone(),
                note: b.note.clone(),
            });
        }
    }
    files.insert("text/fr.ron".to_string(), text_ron(&entries)?);
    files.insert(
        "text/speakers.ron".to_string(),
        extract::export::to_ron(&speakers)?,
    );

    // Battle data.
    let tf = TrainersFile {
        trainers: s.trainers.clone(),
    };
    files.insert(
        "battle/trainers.ron".to_string(),
        extract::export::to_ron(&tf)?,
    );
    files.insert(
        "battle/encounters.ron".to_string(),
        extract::export::to_ron(&s.encounters)?,
    );

    // Items.
    let items = ItemsFile {
        items: s.item_defs.values().cloned().collect(),
        placements: s.placements.clone(),
        marts: s.marts.clone(),
    };
    files.insert("items.ron".to_string(), extract::export::to_ron(&items)?);

    // Ids.
    let constants = s.refs.constants.keys();
    let of = |p: &str| -> BTreeMap<String, Option<String>> {
        constants
            .clone()
            .filter(|k| k.starts_with(p))
            .map(|k| (k.clone(), None))
            .collect()
    };
    let mut want: BTreeMap<IdKind, BTreeMap<String, Option<String>>> = BTreeMap::new();
    want.insert(IdKind::Flags, of("FLAG_SIL_"));
    want.insert(IdKind::Vars, of("VAR_SIL_"));
    let mut songs = of("MUS_SIL_");
    songs.extend(of("SE_SIL_"));
    want.insert(IdKind::Songs, songs);
    // Names the authored maps add (hide flags, gate vars, songs).
    for m in map_files {
        for (field, v) in &m.fields {
            let kind = match field.as_str() {
                "flag" if v.starts_with("FLAG_SIL_") => IdKind::Flags,
                "var" if v.starts_with("VAR_SIL_") => IdKind::Vars,
                "music" if v.starts_with("MUS_SIL_") || v.starts_with("SE_SIL_") => IdKind::Songs,
                _ => continue,
            };
            let set = want.get_mut(&kind).unwrap();
            if !set.contains_key(v) {
                report.add(
                    "Names only the maps use (allocated)",
                    format!("maps/{}.ron: {v}", m.name),
                );
                set.insert(v.clone(), None);
            }
        }
    }
    // Gen3 story flags and vars the data reuses: the Silice block starts above them.
    let gen3_used: BTreeSet<&str> = s
        .refs
        .constants
        .keys()
        .map(String::as_str)
        .chain(
            map_files
                .iter()
                .flat_map(|m| m.fields.iter().map(|(_, v)| v.as_str())),
        )
        .collect();
    let floor = |kind: IdKind| -> Option<u32> {
        let (table, (lo, hi)) = match kind {
            IdKind::Flags => (&extract::names::FLAGS, kind.range()),
            IdKind::Vars => (&extract::names::VARS, kind.range()),
            _ => return None,
        };
        gen3_used
            .iter()
            .filter_map(|n| table.value(n))
            .filter(|v| (lo..=hi).contains(v))
            .max()
            .map(|v| v + 1)
    };
    let mut moves = of("MOVE_SIL_");
    for t in &s.trainers {
        for m in &t.party {
            for mv in m
                .moves
                .iter()
                .flatten()
                .filter(|m| m.starts_with("MOVE_SIL_"))
            {
                moves.insert(mv.clone(), None);
            }
        }
    }
    want.insert(IdKind::Moves, moves);
    want.insert(
        IdKind::Items,
        s.item_defs.keys().map(|k| (k.clone(), None)).collect(),
    );
    let form = |k: &str| {
        k.starts_with("SPECIES_")
            && (k.ends_with("_MEGA")
                || k.ends_with("_MEGA_X")
                || k.ends_with("_MEGA_Y")
                || k.ends_with("_PRIMAL"))
    };
    let mut species: BTreeMap<String, Option<String>> = s
        .refs
        .constants
        .keys()
        .filter(|k| form(k))
        .map(|k| (k.clone(), None))
        .collect();
    for t in &s.trainers {
        for m in t.party.iter().filter(|m| form(&m.species)) {
            species.insert(m.species.clone(), None);
        }
    }
    want.insert(IdKind::Species, species);
    let mut trainer_ids = BTreeMap::new();
    for t in &s.trainers {
        if trainer_ids.insert(t.id.clone(), None).is_some() {
            report.add(
                "Trainers",
                format!("{} is defined twice (rule trainer-duplicate)", t.id),
            );
        }
    }
    want.insert(IdKind::Trainers, trainer_ids);
    want.insert(
        IdKind::Maps,
        s.maps
            .keys()
            .map(|k| (k.clone(), Some(names::map_name(k, &maps))))
            .collect(),
    );
    let mut counts: Vec<(String, usize)> = vec![
        ("chapters".into(), s.chapters.len()),
        ("texts".into(), entries.len()),
        ("texts defined twice with different bodies".into(), dup),
        ("trainers".into(), s.trainers.len()),
        ("encounter maps".into(), s.encounters.maps.len()),
        (
            "encounter tables".into(),
            s.encounters
                .maps
                .values()
                .flatten()
                .map(|h| {
                    [
                        h.land.is_some(),
                        h.water.is_some(),
                        h.rock_smash.is_some(),
                        h.fishing.is_some(),
                    ]
                    .iter()
                    .filter(|&&b| b)
                    .count()
                })
                .sum(),
        ),
        ("ITEM_SIL_ definitions".into(), s.item_defs.len()),
        ("item placements".into(), s.placements.len()),
        ("marts".into(), s.marts.len()),
        ("referenced constants".into(), s.refs.constants.len()),
        ("referenced text ids".into(), s.refs.texts.len()),
        ("dynamic references".into(), s.refs.dynamic.len()),
        (
            "unresolved dynamic references".into(),
            s.refs
                .dynamic
                .iter()
                .filter(|d| d.expanded.is_empty())
                .count(),
        ),
    ];
    for (kind, names) in &want {
        let path = out.join("ids").join(format!("{}.ron", kind.file()));
        let existing: Option<IdsFile> = if path.exists() {
            Some(read_ron(&path)?)
        } else {
            None
        };
        let (file, stale) = ids::allocate(*kind, existing.as_ref(), names, floor(*kind))?;
        if file.base > kind.range().0 {
            report.add(
                "Id bases",
                format!(
                    "ids/{}.ron starts at {:#x}, above the Gen3 numbers the data reuses",
                    kind.file(),
                    file.base
                ),
            );
        }
        for n in stale {
            report.add(
                "Ids no longer in the source (kept)",
                format!("ids/{}.ron: {n}", kind.file()),
            );
        }
        for e in file
            .ids
            .iter()
            .filter(|e| e.removed && names.contains_key(&e.name))
        {
            report.add(
                "Ids no longer in the source (kept)",
                format!(
                    "ids/{}.ron: {} is a tombstone but the source uses it",
                    kind.file(),
                    e.name
                ),
            );
        }
        counts.push((format!("ids/{}", kind.file()), file.ids.len()));
        files.insert(
            format!("ids/{}.ron", kind.file()),
            extract::export::to_ron(&file)?,
        );
    }
    files.insert(
        "ids/refs.ron".to_string(),
        extract::export::to_ron(&s.refs)?,
    );
    Ok((files, counts))
}

/// `text/fr.ron`: a map in the `to_ron` layout, keeping repeated keys, each text a RON raw
/// string (`r"..."`, or `r#"..."#` when it holds a `"`), so the file shows the decomp syntax as
/// written (`\n`, `é`, `'`) without escapes. [`text_pairs`] reads it back exactly.
pub fn text_ron(entries: &[(&str, &str)]) -> Result<String> {
    if entries.is_empty() {
        return Ok("{}\n".to_string());
    }
    let mut s = String::from("{\n");
    for (k, v) in entries {
        let mut hashes = String::new();
        while v.contains(&format!("\"{hashes}")) {
            hashes.push('#');
        }
        let _ = writeln!(s, "    {}: r{hashes}\"{v}\"{hashes},", ron::to_string(k)?);
    }
    s.push_str("}\n");
    Ok(s)
}

/// Reads a `text/fr.ron` map as ordered pairs, keeping repeated keys (a plain map would keep
/// only one, hiding a duplicate text id).
pub fn text_pairs(s: &str) -> Result<Vec<(String, String)>> {
    struct Pairs;
    impl<'de> serde::de::Visitor<'de> for Pairs {
        type Value = Vec<(String, String)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map of text ids to strings")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut m: A,
        ) -> Result<Self::Value, A::Error> {
            let mut v = Vec::new();
            while let Some(e) = m.next_entry::<String, String>()? {
                v.push(e);
            }
            Ok(v)
        }
    }
    let mut d = ron::Deserializer::from_str(s)?;
    Ok(serde::Deserializer::deserialize_map(&mut d, Pairs)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_pairs_keep_duplicates() {
        let s = text_ron(&[("A", "1"), ("B", "2"), ("A", "3")]).unwrap();
        let p = text_pairs(&s).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p[2], ("A".to_string(), "3".to_string()));
    }

    #[test]
    fn text_ron_round_trips() {
        let e = [
            ("A_T1", "Caf\u{e9} \u{ab}x\u{bb}\\n'q'\\p{PLAYER}"),
            ("B", "say \"hi\" and \"#x"),
            ("C", "back\\slash"),
        ];
        let s = text_ron(&e).unwrap();
        assert!(
            s.contains("\"A_T1\": r\"Caf\u{e9} \u{ab}x\u{bb}\\n'q'\\p{PLAYER}\","),
            "{s}"
        );
        assert!(s.contains("\"B\": r##\"say"), "{s}");
        let back = text_pairs(&s).unwrap();
        let back: Vec<(&str, &str)> = back.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(back, e);
        assert_eq!(text_ron(&back).unwrap(), s);
        // A plain map reads it too.
        let m: BTreeMap<String, String> = ron::from_str(&s).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(text_ron(&[]).unwrap(), "{}\n");
    }
}
