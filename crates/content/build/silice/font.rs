//! The Silice font (SIL1 Task G, R-11): the extract's normal font with the authored glyphs of
//! `games/silice/font/*.ron` merged in. Format, codes and metric rules are in
//! `extract::games::font`; this module is the build's side.
//!
//! **Hook for the text encoder (Task A).** Encode every Silice text with [`encode`] (that is,
//! `extract::games::font::encode_fr`): it writes each authored character as its byte
//! (`{0xNN}`) and then runs `encode_lang(.., Lang::Fr)`. A per-character encoder asks
//! [`authored_glyph`] first and falls back on the charmap. [`GLYPHS`] lists the codes.
//!
//! **Merge** ([`merge`], called by [`super::world`]): each authored glyph replaces the font's
//! glyph and width at its code, after [`extract::games::font::problems`] checked it against the
//! extract's metrics (cell, baseline from `A`, the width of a redrawn code, a blank reserved
//! code). Only the normal font: the small font (`FONT_SMALL`) prints the battle healthbox's
//! nickname, level and HP, which never hold these characters. A games dir without `font/`
//! (a test fixture) merges nothing; its texts still encode the codes, drawn blank.

use anyhow::{Result, bail};
use extract::games::font as authored;
use std::path::Path;

pub use authored::Glyph;

/// `(char, code)` of every authored glyph (`extract::games::font::CODES`).
#[allow(dead_code)] // Task A's encoder hook, unused until its encoder lands.
pub const GLYPHS: &[(char, u8)] = authored::CODES;

/// The byte code of an authored glyph, `None` for any other character.
#[allow(dead_code)] // Task A's encoder hook, unused until its encoder lands.
pub fn authored_glyph(ch: char) -> Option<u8> {
    authored::code(ch)
}

/// `text` (decomp `.string` syntax) encoded in French with the authored glyphs, 0xFF-terminated.
#[allow(dead_code)] // Task A's encoder hook, unused until its encoder lands.
pub fn encode(text: &str) -> Result<Vec<u8>> {
    authored::encode_fr(text)
}

/// The authored glyphs of the games dir `games` (`font/*.ron`, in name order).
pub fn load(games: &Path) -> Result<Vec<Glyph>> {
    authored::load_dir(&games.join("font"))
}

/// Writes `authored` into the font (`.latfont` glyphs and widths, by code) after checking each
/// against the font's metrics. When any is merged, every code of [`GLYPHS`] must have its glyph.
pub fn merge(glyphs: &mut [[u8; 64]], widths: &mut [u8], authored: &[Glyph]) -> Result<()> {
    if authored.is_empty() {
        return Ok(());
    }
    let font_px = |c: u8| glyphs.get(usize::from(c)).map(authored::decode_latfont);
    let mut problems = authored::problems(authored, &font_px, widths);
    for &(c, b) in GLYPHS {
        if !authored.iter().any(|g| g.ch == c) {
            problems.push(format!("no glyph file draws {c:?} (code {b:#04X})"));
        }
    }
    if !problems.is_empty() {
        bail!("authored glyphs:\n  {}", problems.join("\n  "));
    }
    for g in authored {
        let i = usize::from(g.code);
        glyphs[i] = g.latfont();
        widths[i] = g.width;
    }
    Ok(())
}
