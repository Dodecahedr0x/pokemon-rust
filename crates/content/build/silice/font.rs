//! The Silice font (SIL1 Task G, R-11): the extract's normal font with the authored glyphs of
//! `games/silice/font/*.ron` merged in. Format, codes and metric rules are in
//! `extract::games::font`; this module is the build's side.
//!
//! **Hook for the text encoder (Task A).**
//! - Encode every Silice text with [`encode`] (that is, `extract::games::font::encode_fr`): it
//!   writes each authored character as its bytes (`{0xNN ...}`), runs
//!   `encode_lang(.., Lang::Fr)`, and rejects Japanese characters.
//! - [`GLYPHS`] is `&[(char, &[u8])]`: each authored character and its **byte sequence**, one
//!   or two bytes. `♥` is `[0xF9, 0xE7]` (Gen3's `{EMOJI_HEART}`, 2 bytes in a name or text),
//!   `·` is `[0xAF]`. A per-character encoder asks [`authored_glyph`] first, appends the whole
//!   sequence, and falls back on the charmap.
//! - Width measurement must run on the merged font ([`merge`] right after `load_assets`).
//!
//! **Merge** ([`merge`], called by [`super::world`]): each authored glyph replaces the font's
//! glyph and width at the glyph it draws (`extract::games::font::glyph_index`: 0xAF, 0x1E7),
//! after [`extract::games::font::problems`] checked it against the extract's metrics (cell,
//! baseline from `A`, a redrawn glyph's width, a blank reserved glyph). Only the normal font:
//! the small font (`FONT_SMALL`) prints the battle healthbox's nickname, level and HP, which
//! never hold these characters. A games dir without `font/` (a test fixture) merges nothing;
//! its texts still encode, drawn with the extract's glyphs.

use anyhow::{Result, bail};
use extract::games::font as authored;
use std::path::Path;

pub use authored::Glyph;

/// `(char, bytes)` of every authored character (`extract::games::font::CODES`).
#[allow(dead_code)] // Task A's encoder hook, unused until its encoder lands.
pub const GLYPHS: &[(char, &[u8])] = authored::CODES;

/// The byte sequence of an authored character (1 or 2 bytes), `None` for any other character.
#[allow(dead_code)] // Task A's encoder hook, unused until its encoder lands.
pub fn authored_glyph(ch: char) -> Option<&'static [u8]> {
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

/// Writes `authored` into the font (`.latfont` glyphs and widths, by glyph index) after
/// checking each against the font's metrics. When any is merged, every character of [`GLYPHS`]
/// must have its glyph.
pub fn merge(glyphs: &mut [[u8; 64]], widths: &mut [u8], authored: &[Glyph]) -> Result<()> {
    if authored.is_empty() {
        return Ok(());
    }
    let font_px = |i: u16| glyphs.get(usize::from(i)).map(authored::decode_latfont);
    let mut problems = authored::problems(authored, &font_px, widths);
    for &(c, b) in authored::CODES {
        if !authored.iter().any(|g| g.ch == c) {
            problems.push(format!("no glyph file draws {c:?} ({b:02X?})"));
        }
    }
    if !problems.is_empty() {
        bail!("authored glyphs:\n  {}", problems.join("\n  "));
    }
    for g in authored {
        let i = usize::from(g.index);
        glyphs[i] = g.latfont();
        widths[i] = g.width;
    }
    Ok(())
}
