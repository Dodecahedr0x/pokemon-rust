//! Authored font glyphs (SIL1 Task G, R-11): `games/<game>/font/*.ron`, one original glyph per
//! file, drawn on the Gen3 Latin font's 16x16 cell. The content build merges them into the
//! extract's normal font (`crates/content/build/silice/font.rs`); `cargo xtask silice check`
//! reads them for the charmap and width rules (`font-glyph`, `text-charmap`, `text-width`).
//!
//! **Format.** A file is a [`GlyphFile`]:
//!
//! ```text
//! (
//!     char: '♥',          // the character the texts write
//!     width: 8,           // advance in pixels, shadow column included (1..=16)
//!     on_baseline: true,  // the lowest foreground row is the font's baseline
//!     rows: [             // 16 rows, top to bottom, each exactly `width` characters
//!         "........",
//!         ...
//!     ],
//! )
//! ```
//!
//! A pixel is `.` (background), `#` (foreground, the text color) or `s` (shadow): the 2bpp
//! values 0, 1 and 2 of the `.latfont` encoding (`crate::font`). Columns past `width` are
//! background. The last row must be blank: the printer draws 15 rows (`GLYPH_HEIGHT`).
//!
//! **Codes.** [`CODES`] gives each authored glyph its byte, append-only like the ids (a code is
//! never reused for another character). A character Gen3's charmap already has keeps its code
//! (the glyph then redraws it, at the extract's width). A new character takes a code of
//! [`RESERVED`]: bytes with no Western character in `charmap.txt` and a blank glyph in the FR
//! font, so neither the encoder nor the font used them. Texts reach the code through
//! [`encode_fr`], which writes each authored character as the decomp escape `{0xNN}` before
//! [`encode_lang`].
//!
//! **Metrics** ([`problems`]), checked against the extract's normal font: the cell is 16x16
//! (`.latfont`); the baseline is the lowest foreground row of `A` (0xBB); a glyph never inks
//! below it (no descenders), and an `on_baseline` glyph ends on it; a redrawn code keeps the
//! extract's width; a reserved code is blank in the extract.

use crate::text::charmap::CHARS;
use crate::text::{Lang, encode_lang};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::RangeInclusive;
use std::path::Path;

/// Glyph cell side in pixels (`.latfont`: 16x16, four 8x8 tiles).
pub const CELL: usize = 16;
/// Bytes of a `.latfont` glyph.
pub const LATFONT_BYTES: usize = 64;
/// Rows the text printer draws per glyph (`gCurGlyph.height` of `FONT_NORMAL`): the last row
/// of the cell must stay blank.
pub const DRAWN_ROWS: usize = 15;

/// Codes for authored characters Gen3 has no code for: no Western character in `charmap.txt`
/// (only kana share these bytes) and a blank glyph in the FR normal font (checked by
/// [`problems`]). 0x37 up, after the `Lv` (0x34), `=` (0x35) and `;` (0x36) glyphs.
pub const RESERVED: RangeInclusive<u8> = 0x37..=0x4F;

/// The authored glyphs and their byte codes. Append-only.
/// - `♥` (U+2665, R-11): a new character, the first reserved code.
/// - `·` (U+00B7, R-11): Gen3 has it at 0xAF; the authored glyph redraws that code.
pub const CODES: &[(char, u8)] = &[('\u{2665}', 0x37), ('\u{B7}', 0xAF)];

/// Character code of `A`, whose lowest foreground row is the baseline.
const BASELINE_REF: u8 = 0xBB;

/// The code of an authored glyph.
pub fn code(ch: char) -> Option<u8> {
    CODES.iter().find(|&&(c, _)| c == ch).map(|&(_, b)| b)
}

/// `text` with each authored character written as its byte, `{0xNN}`, which [`encode_lang`]
/// reads as a raw byte.
pub fn substitute(text: &str) -> Cow<'_, str> {
    if !text.chars().any(|c| code(c).is_some()) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match code(c) {
            Some(b) => out.push_str(&format!("{{0x{b:02X}}}")),
            None => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// [`encode_lang`] in French with the authored glyphs: what the Silice build encodes texts with.
pub fn encode_fr(text: &str) -> Result<Vec<u8>> {
    encode_lang(&substitute(text), Lang::Fr)
}

/// `games/<game>/font/<name>.ron`; see the module doc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GlyphFile {
    pub char: char,
    pub width: u8,
    pub on_baseline: bool,
    pub rows: Vec<String>,
}

/// A parsed authored glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// The file stem it was read from.
    pub name: String,
    pub ch: char,
    pub code: u8,
    pub width: u8,
    pub on_baseline: bool,
    /// `CELL * CELL` row-major 2bpp values (0 background, 1 foreground, 2 shadow).
    pub pixels: Vec<u8>,
}

impl Glyph {
    /// Parses one file's contents (`name` is the file stem, for messages).
    pub fn parse(name: &str, text: &str) -> Result<Glyph> {
        let f: GlyphFile = ron::from_str(text).with_context(|| format!("glyph {name}"))?;
        Glyph::from_file(name, &f)
    }

    pub fn from_file(name: &str, f: &GlyphFile) -> Result<Glyph> {
        let Some(code) = code(f.char) else {
            bail!(
                "glyph {name}: {:?} has no code in extract::games::font::CODES",
                f.char
            );
        };
        let w = usize::from(f.width);
        ensure!(
            (1..=CELL).contains(&w),
            "glyph {name}: width {w} (1..={CELL})"
        );
        ensure!(
            f.rows.len() == CELL,
            "glyph {name}: {} rows ({CELL})",
            f.rows.len()
        );
        let mut pixels = vec![0u8; CELL * CELL];
        for (y, row) in f.rows.iter().enumerate() {
            let n = row.chars().count();
            ensure!(n == w, "glyph {name}: row {y} is {n} pixels (width {w})");
            for (x, c) in row.chars().enumerate() {
                pixels[y * CELL + x] = match c {
                    '.' => 0,
                    '#' => 1,
                    's' => 2,
                    _ => bail!("glyph {name}: row {y}: {c:?} is not '.', '#' or 's'"),
                };
            }
        }
        ensure!(
            pixels[DRAWN_ROWS * CELL..].iter().all(|&p| p == 0),
            "glyph {name}: row {DRAWN_ROWS} must be blank (the printer draws {DRAWN_ROWS} rows)"
        );
        Ok(Glyph {
            name: name.to_string(),
            ch: f.char,
            code,
            width: f.width,
            on_baseline: f.on_baseline,
            pixels,
        })
    }

    /// The glyph in `.latfont` encoding (`crate::font`): tiles TL, TR, BL, BR of 8
    /// little-endian u16 rows, leftmost pixel in bits 15-14.
    pub fn latfont(&self) -> [u8; LATFONT_BYTES] {
        encode_latfont(&self.pixels)
    }

    /// The lowest row holding a foreground pixel.
    pub fn lowest_fg_row(&self) -> Option<usize> {
        lowest_fg_row(&self.pixels)
    }
}

/// Reads every `*.ron` of `dir` in name order; fails on a malformed file, a character without
/// a code or a character drawn twice. An absent `dir` gives no glyph.
pub fn load_dir(dir: &Path) -> Result<Vec<Glyph>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let mut out: Vec<Glyph> = Vec::new();
    for p in files {
        let name = p.file_stem().unwrap_or_default().to_string_lossy();
        let text = std::fs::read_to_string(&p).with_context(|| p.display().to_string())?;
        let g = Glyph::parse(&name, &text)?;
        if let Some(o) = out.iter().find(|o| o.ch == g.ch) {
            bail!("glyph {}: {:?} is also drawn by {}", g.name, g.ch, o.name);
        }
        out.push(g);
    }
    Ok(out)
}

/// 256 row-major 2bpp pixels as a `.latfont` glyph.
pub fn encode_latfont(pixels: &[u8]) -> [u8; LATFONT_BYTES] {
    let mut g = [0u8; LATFONT_BYTES];
    for tile in 0..4 {
        for y in 0..8 {
            let mut v = 0u16;
            for x in 0..8 {
                let (px, py) = ((tile & 1) * 8 + x, (tile >> 1) * 8 + y);
                v |= u16::from(pixels[py * CELL + px] & 3) << (14 - 2 * x);
            }
            let at = (tile * 8 + y) * 2;
            g[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    g
}

/// A `.latfont` glyph as 256 row-major 2bpp pixels.
pub fn decode_latfont(g: &[u8; LATFONT_BYTES]) -> Vec<u8> {
    let mut px = vec![0u8; CELL * CELL];
    for tile in 0..4 {
        for y in 0..8 {
            let at = (tile * 8 + y) * 2;
            let v = u16::from_le_bytes([g[at], g[at + 1]]);
            for x in 0..8 {
                let (cx, cy) = ((tile & 1) * 8 + x, (tile >> 1) * 8 + y);
                px[cy * CELL + cx] = ((v >> (14 - 2 * x)) & 3) as u8;
            }
        }
    }
    px
}

fn lowest_fg_row(pixels: &[u8]) -> Option<usize> {
    (0..CELL)
        .rev()
        .find(|&y| pixels[y * CELL..(y + 1) * CELL].contains(&1))
}

/// What breaks the extract font's metrics (see the module doc), one message per problem.
/// `font_px(code)` gives the extract's glyph as 256 pixels ([`decode_latfont`]), or `None`
/// when only the widths are known (the validator without the font png): the baseline and
/// blank-code checks are then skipped. `widths` are the extract's.
pub fn problems(
    glyphs: &[Glyph],
    font_px: &dyn Fn(u8) -> Option<Vec<u8>>,
    widths: &[u8],
) -> Vec<String> {
    let mut out = Vec::new();
    let baseline = font_px(BASELINE_REF).and_then(|a| lowest_fg_row(&a));
    if font_px(BASELINE_REF).is_some() && baseline.is_none() {
        out.push(format!(
            "the extract's glyph {BASELINE_REF:#04X} (A) has no foreground: no baseline"
        ));
    }
    for g in glyphs {
        let n = &g.name;
        let gen3 = CHARS.iter().find(|&&(c, _)| c == g.ch).map(|&(_, b)| b);
        match gen3 {
            Some(b) if b != g.code => out.push(format!(
                "glyph {n}: {:?} is {b:#04X} in the charmap but its code is {:#04X}",
                g.ch, g.code
            )),
            Some(_) => match widths.get(usize::from(g.code)) {
                Some(&w) if w != g.width => out.push(format!(
                    "glyph {n}: width {} but the extract draws {:#04X} {w} wide (a redrawn \
                     code keeps its width)",
                    g.width, g.code
                )),
                Some(_) => {}
                None => out.push(format!(
                    "glyph {n}: the extract has no glyph {:#04X}",
                    g.code
                )),
            },
            None if !RESERVED.contains(&g.code) => out.push(format!(
                "glyph {n}: code {:#04X} is outside the reserved {:#04X}..={:#04X}",
                g.code,
                RESERVED.start(),
                RESERVED.end()
            )),
            None => {
                if let Some(c) = crate::text::charmap::DECODE_CHARS[usize::from(g.code)] {
                    out.push(format!(
                        "glyph {n}: code {:#04X} is the charmap's {c:?}",
                        g.code
                    ));
                }
                if let Some(px) = font_px(g.code)
                    && px.iter().any(|&p| p != 0)
                {
                    out.push(format!(
                        "glyph {n}: the extract's glyph {:#04X} is not blank (the code is used)",
                        g.code
                    ));
                }
            }
        }
        if let Some(base) = baseline {
            match g.lowest_fg_row() {
                Some(r) if r > base => out.push(format!(
                    "glyph {n}: foreground on row {r}, below the baseline (row {base})"
                )),
                Some(r) if g.on_baseline && r != base => out.push(format!(
                    "glyph {n}: on_baseline but its lowest foreground row is {r} (baseline {base})"
                )),
                None => out.push(format!("glyph {n}: no foreground pixel")),
                _ => {}
            }
        }
    }
    for &(c, b) in CODES {
        if CODES.iter().filter(|&&(_, o)| o == b).count() > 1 {
            out.push(format!(
                "code {b:#04X} is given to {c:?} and another character"
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(u: u32) -> char {
        char::from_u32(u).unwrap()
    }

    fn file(rows: &[String], width: u8) -> GlyphFile {
        GlyphFile {
            char: ch(0x2665),
            width,
            on_baseline: true,
            rows: rows.to_vec(),
        }
    }

    fn blank(width: usize) -> Vec<String> {
        vec![".".repeat(width); CELL]
    }

    #[test]
    fn substitute_writes_codes() {
        let heart = ch(0x2665);
        assert_eq!(substitute("AB"), Cow::Borrowed("AB"));
        assert_eq!(substitute(&format!("A{heart}B")), format!("A{{0x37}}B"));
        let b = encode_fr(&format!("A{heart}")).unwrap();
        assert_eq!(b, vec![0xBB, 0x37, 0xFF]);
        let dot = ch(0xB7);
        assert_eq!(encode_fr(&dot.to_string()).unwrap(), vec![0xAF, 0xFF]);
        assert!(encode_lang(&heart.to_string(), Lang::Fr).is_err());
    }

    #[test]
    fn reserved_codes_have_no_western_character() {
        for b in RESERVED {
            assert!(
                crate::text::charmap::DECODE_CHARS[usize::from(b)].is_none(),
                "{b:#04X}"
            );
        }
        for &(c, b) in CODES {
            let gen3 = CHARS.iter().find(|&&(m, _)| m == c).map(|&(_, b)| b);
            assert!(
                gen3 == Some(b) || (gen3.is_none() && RESERVED.contains(&b)),
                "{c:?}"
            );
        }
    }

    #[test]
    fn latfont_round_trips() {
        let mut px = vec![0u8; CELL * CELL];
        px[0] = 1;
        px[7] = 2;
        px[8] = 1;
        px[15 * CELL + 9] = 2;
        let g = encode_latfont(&px);
        assert_eq!(&g[..2], &0x4002u16.to_le_bytes());
        assert_eq!(decode_latfont(&g), px);
    }

    #[test]
    fn parse_rejects_bad_files() {
        let mut rows = blank(3);
        rows[5] = "#s.".to_string();
        let ok = Glyph::from_file("t", &file(&rows, 3)).unwrap();
        assert_eq!(ok.code, 0x37);
        assert_eq!(ok.pixels[5 * CELL], 1);
        assert_eq!(ok.pixels[5 * CELL + 1], 2);
        assert_eq!(ok.lowest_fg_row(), Some(5));

        // Row length differs from the width.
        let mut bad = rows.clone();
        bad[2] = "....".to_string();
        assert!(Glyph::from_file("t", &file(&bad, 3)).is_err());
        // Unknown pixel.
        let mut bad = rows.clone();
        bad[2] = ".x.".to_string();
        assert!(Glyph::from_file("t", &file(&bad, 3)).is_err());
        // Ink on the undrawn last row.
        let mut bad = rows.clone();
        bad[15] = "#..".to_string();
        assert!(Glyph::from_file("t", &file(&bad, 3)).is_err());
        // 15 rows.
        assert!(Glyph::from_file("t", &file(&rows[..15], 3)).is_err());
        // No code.
        let mut f = file(&rows, 3);
        f.char = ch(0x2014);
        assert!(Glyph::from_file("t", &f).is_err());
    }

    #[test]
    fn metrics() {
        // A synthetic font: `A` with its lowest foreground on row 11, · 3 wide.
        let a = {
            let mut px = vec![0u8; CELL * CELL];
            px[11 * CELL] = 1;
            px
        };
        let font = move |c: u8| {
            Some(if c == BASELINE_REF {
                a.clone()
            } else {
                vec![0; 256]
            })
        };
        let mut widths = vec![3u8; 256];
        widths[0xAF] = 3;
        let glyph = |ch: char, w: u8, row: usize, on_baseline: bool| {
            let mut rows = blank(usize::from(w));
            rows[row] = format!("#{}", ".".repeat(usize::from(w) - 1));
            let mut f = file(&rows, w);
            f.char = ch;
            f.on_baseline = on_baseline;
            Glyph::from_file("t", &f).unwrap()
        };
        let heart = ch(0x2665);
        let dot = ch(0xB7);
        assert!(
            problems(
                &[glyph(heart, 8, 11, true), glyph(dot, 3, 8, false)],
                &font,
                &widths
            )
            .is_empty()
        );
        // Off the baseline, below it, a redrawn code at another width.
        assert_eq!(
            problems(&[glyph(heart, 8, 10, true)], &font, &widths).len(),
            1
        );
        assert_eq!(
            problems(&[glyph(heart, 8, 12, false)], &font, &widths).len(),
            1
        );
        assert_eq!(
            problems(&[glyph(dot, 4, 8, false)], &font, &widths).len(),
            1
        );
        // A reserved code the extract draws.
        let used = |c: u8| Some(vec![if c == 0x37 { 1 } else { 0 }; 256]);
        assert!(!problems(&[glyph(heart, 8, 11, true)], &used, &widths).is_empty());
        // Widths only: the pixel checks are skipped.
        assert!(problems(&[glyph(heart, 8, 3, true)], &|_| None, &widths).is_empty());
    }
}
