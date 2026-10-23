//! Silice's texts (SIL1 Task A): `games/silice/text/fr.ron` read entry by entry (a repeated key
//! fails: the loader contract of Task B, `extract::games::parse_texts`) and encoded with the
//! extract's charmap for the extract's language ([`Encoder`]).
//!
//! **Encoding.** The extract's codec (`extract::text::encode_lang`) with its language overlay
//! (French: `«`/`»` on the bytes English draws as curly quotes), plus the glyph codes of
//! [`glyph_codes`], the hook for Silice's own glyphs (SIL1 Task G: `♥`, `·`…): each such
//! character is written as its byte (`{0xNN}` to the codec).
//!
//! **Unencodable characters** fail the build, listing every text and character, unless
//! `check-exceptions.ron` lists the text (rule `text-charmap`, its id as the subject): then each
//! such character becomes [`PLACEHOLDER`] and `content::REPORT` gets a line. Trainer and item
//! names follow the same rule, with the trainer or item id as the subject.
//!
//! **Ids.** Each text id becomes a `TextId` constant of `content::consts` (the shape of
//! Emerald's `gText_*` and map texts), numbered with the kept Gen3 texts in name order.

use super::load::Exceptions;
use crate::input::World;
use anyhow::{Context, Result, bail, ensure};
use extract::text::{Lang, encode_lang};
use std::collections::BTreeMap;
use std::path::Path;

/// `(id, text)` in file order; an error on a repeated id.
pub fn read(path: &Path) -> Result<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    super::schema::parse_texts(&text).with_context(|| path.display().to_string())
}

/// What an excepted unencodable character becomes.
pub const PLACEHOLDER: char = '?';

/// **The text-encoder hook (SIL1 Task G).** Silice's own glyphs: each `(character, byte)` is
/// encoded as that byte (written `{0xNN}`, then the FR codec: what `font::encode` /
/// `extract::games::font::encode_fr` do). Task G's `font.rs` merges its glyphs into `w`'s font
/// in `world` (`font::merge`, after `load_assets`); when it lands, this body becomes the pairs
/// of its table, e.g. `Ok(super::font::GLYPHS.iter().map(|g| (g.ch, g.code)).collect())`
/// (`font::authored_glyph` per character). Every text, trainer, class and item name goes
/// through [`Encoder`], so nothing else changes. Until then: none (plain `encode_lang`).
pub fn glyph_codes(_games: &Path, _w: &mut World) -> Result<Vec<(char, u8)>> {
    Ok(Vec::new())
}

/// The Silice text encoder: the extract's codec for `lang`, plus `glyphs` ([`glyph_codes`]).
pub struct Encoder {
    lang: Lang,
    glyphs: Vec<(char, u8)>,
}

impl Encoder {
    pub fn new(lang: Lang, glyphs: Vec<(char, u8)>) -> Encoder {
        Encoder { lang, glyphs }
    }

    /// `s` (decomp `.string` syntax) as Gen3 bytes, 0xFF-terminated.
    pub fn encode(&self, s: &str) -> Result<Vec<u8>> {
        let mut t = String::with_capacity(s.len());
        for c in s.chars() {
            match self.glyphs.iter().find(|g| g.0 == c) {
                Some((_, b)) => t.push_str(&format!("{{0x{b:02X}}}")),
                None => t.push(c),
            }
        }
        encode_lang(&t, self.lang)
    }

    /// Whether the character `c` alone can be encoded.
    fn char_ok(&self, c: char) -> bool {
        self.glyphs.iter().any(|g| g.0 == c) || encode_lang(&c.to_string(), self.lang).is_ok()
    }

    /// The characters of `s` outside `{…}` codes and `\` escapes that cannot be encoded, in
    /// order, each once.
    pub fn bad_chars(&self, s: &str) -> Vec<char> {
        let mut out = Vec::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '{' => {
                    for d in chars.by_ref() {
                        if d == '}' {
                            break;
                        }
                    }
                }
                c if !self.char_ok(c) && !out.contains(&c) => out.push(c),
                _ => {}
            }
        }
        out
    }

    /// `s` with every bad character ([`Self::bad_chars`]) replaced by [`PLACEHOLDER`].
    pub fn substitute(&self, s: &str, bad: &[char]) -> String {
        s.chars()
            .map(|c| if bad.contains(&c) { PLACEHOLDER } else { c })
            .collect()
    }

    /// `s` encoded; an unencodable character becomes [`PLACEHOLDER`] when `exc` lists `subject`
    /// under `text-charmap` (a `report` line), else it fails. Returns the text actually encoded.
    pub fn encode_excepted(
        &self,
        s: &str,
        subject: &str,
        exc: &Exceptions,
        report: &mut Vec<String>,
    ) -> Result<(String, Vec<u8>)> {
        let bad = self.bad_chars(s);
        if bad.is_empty() {
            let b = self.encode(s).with_context(|| subject.to_string())?;
            return Ok((s.to_string(), b));
        }
        ensure!(
            exc.find("text-charmap", subject).is_some(),
            "{subject}: {:?} cannot be encoded (add the glyphs, or list {subject} under \
             `text-charmap` in check-exceptions.ron)",
            String::from_iter(&bad)
        );
        let t = self.substitute(s, &bad);
        report.push(format!(
            "placeholder: {subject}: {:?} -> {PLACEHOLDER:?} (text-charmap exception)",
            String::from_iter(&bad)
        ));
        let b = self.encode(&t).with_context(|| subject.to_string())?;
        Ok((t, b))
    }

    /// `s` encoded within `max` bytes (the terminator not counted): a longer one is cut, at a
    /// character, and trimmed, when `exc` lists `subject` under `rule` (a `report` line), else
    /// it fails. Unencodable characters as [`Self::encode_excepted`].
    pub fn encode_name(
        &self,
        s: &str,
        max: usize,
        rule: &str,
        subject: &str,
        exc: &Exceptions,
        report: &mut Vec<String>,
    ) -> Result<Vec<u8>> {
        let (mut t, mut b) = self.encode_excepted(s, subject, exc, report)?;
        if b.len() - 1 <= max {
            return Ok(b);
        }
        ensure!(
            exc.find(rule, subject).is_some(),
            "{subject}: {s:?} is {} bytes, over {max} (shorten it, or list {subject} under \
             `{rule}` in check-exceptions.ron)",
            b.len() - 1
        );
        while b.len() - 1 > max {
            t.pop();
            t.truncate(t.trim_end().len());
            b = self.encode(&t).with_context(|| subject.to_string())?;
        }
        report.push(format!(
            "truncated: {subject}: {s:?} -> {t:?} ({max} bytes; {rule} exception)"
        ));
        Ok(b)
    }
}

/// `text/fr.ron` encoded, by id; `report` gets the placeholders. Every unencodable text that
/// no exception covers is listed in one error.
pub fn load(
    games: &Path,
    enc: &Encoder,
    exc: &Exceptions,
    report: &mut Vec<String>,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let path = games.join("text/fr.ron");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let mut out = BTreeMap::new();
    let mut failed = Vec::new();
    for (id, text) in read(&path)? {
        ensure!(
            crate::codegen::ident_ok(&id),
            "text id {id:?} is not an identifier"
        );
        match enc.encode_excepted(&text, &id, exc, report) {
            Ok((_, b)) => {
                out.insert(id, b);
            }
            Err(e) => failed.push(format!("{e:#}")),
        }
    }
    if !failed.is_empty() {
        bail!(
            "{} texts of text/fr.ron cannot be encoded:\n  {}",
            failed.len(),
            failed.join("\n  ")
        );
    }
    Ok(out)
}
