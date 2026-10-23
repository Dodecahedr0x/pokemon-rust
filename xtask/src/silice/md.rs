//! The chapter markdown of `contenu-jeu/chapitres/*.md` (conventions §1): numbered `## N.`
//! sections, fenced blocks (`text` dialogue and pseudo-scripts, `ron` data), and pipe tables.

use anyhow::{Context, Result};
use std::ops::Range;
use std::path::Path;

/// One chapter file, as lines.
pub struct Chapter {
    /// File stem (`00-prologue`).
    pub stem: String,
    pub lines: Vec<String>,
}

impl Chapter {
    pub fn read(path: &Path) -> Result<Chapter> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Chapter::parse(
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
            &text,
        ))
    }

    pub fn parse(stem: &str, text: &str) -> Chapter {
        Chapter {
            stem: stem.to_string(),
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    /// `## N.` sections: number and line range (the heading line included).
    pub fn sections(&self) -> Vec<(u32, Range<usize>)> {
        let mut starts = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            if let Some(rest) = l.strip_prefix("## ")
                && let Some((num, _)) = rest.split_once('.')
                && let Ok(n) = num.trim().parse::<u32>()
            {
                starts.push((n, i));
            }
        }
        let mut out = Vec::new();
        for (k, &(n, s)) in starts.iter().enumerate() {
            let e = starts.get(k + 1).map_or(self.lines.len(), |&(_, e)| e);
            out.push((n, s..e));
        }
        out
    }

    /// Every fenced block, in order.
    pub fn fences(&self) -> Vec<Fence> {
        let mut out = Vec::new();
        let mut open: Option<(usize, String, usize)> = None;
        for (i, l) in self.lines.iter().enumerate() {
            let t = l.trim_start();
            let Some(after) = t.strip_prefix("```") else {
                continue;
            };
            match open.take() {
                None => open = Some((i, after.trim().to_string(), l.len() - t.len())),
                Some((s, lang, indent)) => {
                    if after.trim().is_empty() {
                        out.push(Fence {
                            lang,
                            open: s,
                            close: i,
                            indent,
                        });
                    } else {
                        // A new opening fence before a close: keep the newer one open.
                        open = Some((i, after.trim().to_string(), l.len() - t.len()));
                    }
                }
            }
        }
        out
    }

    /// Lines inside a fence, the fence's indentation removed.
    pub fn fence_body(&self, f: &Fence) -> Vec<(usize, String)> {
        (f.open + 1..f.close)
            .map(|i| {
                let l = &self.lines[i];
                let strip = l
                    .char_indices()
                    .take_while(|&(k, c)| k < f.indent && c == ' ')
                    .count();
                (i, l[strip..].trim_end().to_string())
            })
            .collect()
    }

    /// Whether a line is inside any fenced block (fence lines included).
    pub fn fenced_lines(&self) -> Vec<bool> {
        let mut v = vec![false; self.lines.len()];
        for f in self.fences() {
            for x in &mut v[f.open..=f.close] {
                *x = true;
            }
        }
        v
    }

    /// Pipe tables outside fences: header cells, and rows with their line numbers.
    pub fn tables(&self, range: Range<usize>) -> Vec<Table> {
        let fenced = self.fenced_lines();
        let mut out = Vec::new();
        let mut i = range.start;
        while i < range.end {
            let is_row = |k: usize| {
                k < range.end && !fenced[k] && self.lines[k].trim_start().starts_with('|')
            };
            if is_row(i) && is_row(i + 1) && is_separator(&self.lines[i + 1]) {
                let header = cells(&self.lines[i]);
                let mut rows = Vec::new();
                let mut k = i + 2;
                while is_row(k) {
                    rows.push((k, cells(&self.lines[k])));
                    k += 1;
                }
                out.push(Table {
                    line: i,
                    header,
                    rows,
                });
                i = k;
            } else {
                i += 1;
            }
        }
        out
    }
}

/// A fenced block: `lang` is the info string (`text`, `ron`, or empty).
pub struct Fence {
    pub lang: String,
    pub open: usize,
    pub close: usize,
    pub indent: usize,
}

pub struct Table {
    /// Line of the header row.
    pub line: usize,
    pub header: Vec<String>,
    pub rows: Vec<(usize, Vec<String>)>,
}

impl Table {
    /// The column whose header, without backticks, equals `name` (ASCII case-insensitive).
    pub fn col(&self, name: &str) -> Option<usize> {
        self.header
            .iter()
            .position(|h| h.replace('`', "").trim().eq_ignore_ascii_case(name))
    }
}

fn is_separator(l: &str) -> bool {
    let t = l.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn cells(l: &str) -> Vec<String> {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// One `[Id]` text of a `text` fence.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub id: String,
    pub speaker: Option<String>,
    pub note: Option<String>,
    /// `.string` syntax: the body lines joined.
    pub body: String,
    /// Line of the `[Id]` header (0-based).
    pub line: usize,
}

/// Whether `l` is an `[Id] speaker (note)` header: an identifier in brackets, then optional
/// text, and no line-break escape at the end (a body line such as `[LOG_01] — DOSSIER\n` is
/// text).
pub fn header(l: &str) -> Option<(String, Option<String>, Option<String>)> {
    let rest = l.strip_prefix('[')?;
    let (id, after) = rest.split_once(']')?;
    let mut chars = id.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    if !(after.is_empty() || after.starts_with(' ')) || ends_with_escape(l) {
        return None;
    }
    let after = after.trim();
    let (speaker, note) = match after.find('(') {
        Some(p) if after.ends_with(')') => {
            let s = after[..p].trim();
            let n = after[p + 1..after.len() - 1].trim();
            (
                (!s.is_empty()).then(|| s.to_string()),
                (!n.is_empty()).then(|| n.to_string()),
            )
        }
        _ => ((!after.is_empty()).then(|| after.to_string()), None),
    };
    Some((id.to_string(), speaker, note))
}

pub fn ends_with_escape(l: &str) -> bool {
    let t = l.trim_end();
    t.ends_with("\\n") || t.ends_with("\\p") || t.ends_with("\\l")
}

/// Parses a `text` fence body into texts. Returns `None` for a pseudo-script (no header on the
/// first non-blank line). Warnings go to `warn`.
pub fn text_blocks(
    body: &[(usize, String)],
    warn: &mut dyn FnMut(usize, String),
) -> Option<Vec<TextBlock>> {
    let first = body.iter().find(|(_, l)| !l.trim().is_empty())?;
    header(first.1.trim_start())?;
    let mut out: Vec<TextBlock> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut prev_blank = true;
    let finish =
        |out: &mut Vec<TextBlock>, lines: &mut Vec<String>, warn: &mut dyn FnMut(usize, String)| {
            if let Some(t) = out.last_mut() {
                let mut s = String::new();
                for (k, l) in lines.iter().enumerate() {
                    if k > 0 && !ends_with_escape(&lines[k - 1]) {
                        warn(
                            t.line,
                            format!(
                                "{}: a line without \\n, \\l or \\p joined with a space",
                                t.id
                            ),
                        );
                        s.push(' ');
                    }
                    s.push_str(l);
                }
                t.body = s;
            }
            lines.clear();
        };
    for (i, l) in body {
        let t = l.trim_start();
        if prev_blank && let Some((id, speaker, note)) = header(t) {
            finish(&mut out, &mut lines, warn);
            out.push(TextBlock {
                id,
                speaker,
                note,
                body: String::new(),
                line: *i,
            });
            prev_blank = false;
            continue;
        }
        if l.trim().is_empty() {
            prev_blank = true;
            continue;
        }
        if prev_blank && !lines.is_empty() {
            warn(*i, "a blank line inside a text".to_string());
        }
        prev_blank = false;
        lines.push(l.trim_end().to_string());
    }
    finish(&mut out, &mut lines, warn);
    for t in &out {
        if t.body.is_empty() {
            warn(t.line, format!("{}: empty text", t.id));
        }
    }
    Some(out)
}

/// One top-level entry of a `ron` fence: `( ... )` or `"KEY": ( ... )`.
#[derive(Debug, Clone, PartialEq)]
pub struct RonEntry {
    pub key: Option<String>,
    /// The value, comments removed.
    pub value: String,
    /// `//` comments inside the entry, the ones on their own lines just before it, and the one
    /// after it on its closing line, in order, trimmed.
    pub comments: Vec<String>,
    /// First line of the entry (0-based, file lines).
    pub line: usize,
}

/// Splits a `ron` fence into entries: a bare list of `(...)` (optionally inside `[ ]`) or of
/// `"KEY": (...)` (optionally inside `{ }`).
pub fn ron_entries(body: &[(usize, String)]) -> Vec<RonEntry> {
    let mut out: Vec<RonEntry> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut depth = 0usize; // depth inside the current entry
    let mut wrapper = 0usize; // `[` / `{` around the entries
    let mut cur: Option<RonEntry> = None;
    let mut closed_line: Option<usize> = None;
    let mut entry_indent = 0usize;
    let indent = |l: &str| l.len() - l.trim_start().len();
    for (ln, l) in body {
        let chars: Vec<char> = l.chars().collect();
        let mut k = 0;
        let mut in_str = false;
        while k < chars.len() {
            let c = chars[k];
            if in_str {
                if let Some(e) = cur.as_mut() {
                    e.value.push(c);
                }
                if c == '\\' && k + 1 < chars.len() {
                    if let Some(e) = cur.as_mut() {
                        e.value.push(chars[k + 1]);
                    }
                    k += 2;
                    continue;
                }
                if c == '"' {
                    in_str = false;
                    if depth == 0
                        && let Some(e) = cur.as_mut()
                        && e.key.is_none()
                    {
                        // The closing quote of a key.
                        let v = std::mem::take(&mut e.value);
                        e.key = Some(v.trim_matches('"').to_string());
                    }
                }
                k += 1;
                continue;
            }
            if c == '/' && chars.get(k + 1) == Some(&'/') {
                let text: String = chars[k + 2..].iter().collect();
                let text = text.trim().to_string();
                if !text.is_empty() {
                    match (&mut cur, closed_line) {
                        (Some(e), _) => e.comments.push(text),
                        // On the closing line, or right after it (no blank line) and indented
                        // deeper than the entry: the previous entry's.
                        (None, Some(cl))
                            if cl == *ln || (cl + 1 == *ln && indent(l) > entry_indent) =>
                        {
                            if let Some(e) = out.last_mut() {
                                e.comments.push(text);
                            }
                            closed_line = Some(*ln);
                        }
                        _ => pending.push(text),
                    }
                }
                break;
            }
            match c {
                '"' => {
                    in_str = true;
                    if cur.is_none() {
                        cur = Some(RonEntry {
                            key: None,
                            value: String::new(),
                            comments: std::mem::take(&mut pending),
                            line: *ln,
                        });
                        entry_indent = indent(l);
                    }
                    if let Some(e) = cur.as_mut() {
                        e.value.push(c);
                    }
                }
                '[' | '{' if cur.is_none() => wrapper += 1,
                ']' | '}' if cur.is_none() => wrapper = wrapper.saturating_sub(1),
                '(' | '[' | '{' => {
                    if cur.is_none() {
                        cur = Some(RonEntry {
                            key: None,
                            value: String::new(),
                            comments: std::mem::take(&mut pending),
                            line: *ln,
                        });
                        entry_indent = indent(l);
                    }
                    depth += 1;
                    cur.as_mut().unwrap().value.push(c);
                }
                ')' | ']' | '}' => {
                    if let Some(e) = cur.as_mut() {
                        e.value.push(c);
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            out.push(cur.take().unwrap());
                            closed_line = Some(*ln);
                        }
                    }
                }
                ':' | ',' if depth == 0 => {}
                _ => {
                    if let Some(e) = cur.as_mut() {
                        e.value.push(c);
                    }
                }
            }
            k += 1;
        }
        if let Some(e) = cur.as_mut() {
            e.value.push('\n');
        }
    }
    let _ = wrapper;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> Vec<(usize, String)> {
        s.lines()
            .enumerate()
            .map(|(i, l)| (i, l.to_string()))
            .collect()
    }

    #[test]
    fn headers() {
        assert_eq!(
            header("[A_B1] SPEAKER (a note (1,2))"),
            Some((
                "A_B1".into(),
                Some("SPEAKER".into()),
                Some("a note (1,2)".into())
            ))
        );
        assert_eq!(
            header("[A_B] (only note)").unwrap().2,
            Some("only note".into())
        );
        assert_eq!(header("[A_B]").unwrap().1, None);
        assert_eq!(header("[LOG_1] - TITLE\\n"), None);
        assert_eq!(header("[SUBJECT: X]"), None);
        assert_eq!(header("[1A]"), None);
    }

    #[test]
    fn grouped_texts() {
        let b = body("[A_T1] X\nOne\\n\ntwo\n\n[A_T2]\nThree\\p\nfour\nfive");
        let mut w = Vec::new();
        let t = text_blocks(&b, &mut |l, m| w.push((l, m))).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].body, "Three\\pfour five");
        assert_eq!(t[0].body, "One\\ntwo");
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(text_blocks(&body("lock\nmsgbox(A_T1)"), &mut |_, _| ()).is_none());
    }

    #[test]
    fn ron_split() {
        let b = body(
            "// head\n[\n    (a: 1, b: \"x // y\"), // tail\n    // lead\n    (a: (2), // in\n     b: \"z\"),\n]",
        );
        let e = ron_entries(&b);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].comments, vec!["head", "tail"]);
        assert_eq!(e[1].comments, vec!["lead", "in"]);
        // A deeper-indented comment right after an entry belongs to it.
        let t = ron_entries(&body("(a: 1),\n    // after a\n(b: 2),"));
        assert_eq!(t[0].comments, vec!["after a"]);
        assert!(t[1].comments.is_empty());
        assert!(e[0].value.contains("\"x // y\""));
        let k = ron_entries(&body(
            "\"MAP_A\": ( land: None ), // c\n\"MAP_B\": (\n x: 1,\n),",
        ));
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].key.as_deref(), Some("MAP_A"));
        assert_eq!(k[0].comments, vec!["c"]);
        assert_eq!(k[1].key.as_deref(), Some("MAP_B"));
        assert!(k[1].value.trim().starts_with('('));
    }
}
