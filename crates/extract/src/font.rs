//! Latin fonts from the decomp's `src/fonts.c`: normal (`gFontNormalLatinGlyphs` /
//! `gFontNormalLatinGlyphWidths`) and small (`gFontSmallLatinGlyphs` / `gFontSmallLatinGlyphWidths`,
//! `FONT_SMALL`, used for the battle healthbox nickname, level and HP numbers).
//!
//! Glyphs are stored uncompressed in gbagfx's `.latfont` encoding: 512 glyphs of 64 bytes. Each
//! glyph is 16x16 pixels made of four 8x8 tiles in the order top-left, top-right, bottom-left,
//! bottom-right. A tile row is a little-endian u16 holding 8 pixels of 2 bits, the leftmost pixel
//! in bits 15-14. The widths table (one u8 per glyph) immediately follows the glyphs.

use crate::Rom;
use anyhow::{Context, Result, bail};

/// Number of glyphs in the font (`latin_normal.png` is 256x512 px: 32 rows of 16 glyphs).
pub const GLYPH_COUNT: usize = 512;
const GLYPH_SIZE: usize = 16;
const GLYPH_BYTES: usize = 64;
const MAX_WIDTH: u8 = 16;

/// Character code of `A` in `charmap.txt`, whose glyph is the anchor.
const ANCHOR_CHAR: u32 = 0xBB;
/// `.latfont` encoding of the `A` glyph from `latin_normal.png`. Unique in BPEE and BPEF.
const ANCHOR: [u8; GLYPH_BYTES] = [
    0xf0, 0xff, 0xf0, 0xff, 0xf0, 0xff, 0xb0, 0xd5, 0x60, 0x6a, 0x60, 0x6f, 0x60, 0x6f, 0x60, 0x6f,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x60, 0x55, 0x60, 0x6a, 0x60, 0x6f, 0x60, 0x6f, 0xa0, 0xaf, 0xf0, 0xff, 0xf0, 0xff, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// `.latfont` encoding of the `A` glyph from `latin_small.png`. `latin_small_narrow.png` draws the
/// same `A` (the narrow font differs in its widths), so it matches twice: see
/// [`locate_latin_small`].
const ANCHOR_SMALL: [u8; GLYPH_BYTES] = [
    0x00, 0x00, 0xc0, 0xff, 0xc0, 0xff, 0xc0, 0xff, 0xc0, 0xd6, 0x80, 0x69, 0x80, 0x6d, 0x80, 0x55,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x80, 0x69, 0x80, 0x6d, 0x80, 0x6d, 0x80, 0xae, 0xc0, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
/// Glyphs plus widths of one font: `fonts.c` places each widths table right after its glyphs.
const FONT_BYTES: u32 = (GLYPH_COUNT * GLYPH_BYTES + GLYPH_COUNT) as u32;

#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// Glyph cell width in pixels (16).
    pub glyph_width_max: u8,
    /// Glyph cell height in pixels (16).
    pub glyph_height: u8,
    /// One entry per character code: `glyph_width_max * glyph_height` row-major pixels, each a
    /// palette index 0-3 (gbagfx font palette: 0 background, 1 foreground, 2 shadow, 3 box).
    pub glyphs: Vec<Vec<u8>>,
    /// Advance width in pixels per character code, same length as `glyphs`.
    pub widths: Vec<u8>,
}

/// Returns the file offsets of the glyph data and of the widths table.
pub fn locate_latin_normal(rom: &Rom) -> Result<(u32, u32)> {
    let hits = rom.find_all(&ANCHOR);
    let [hit] = hits[..] else {
        bail!("latin normal font anchor: expected 1 match, found {hits:#x?}");
    };
    offsets_from_anchor(rom, hit, "latin normal")
}

/// Returns the file offsets of `gFontSmallLatinGlyphs` and `gFontSmallLatinGlyphWidths`.
///
/// The `A` anchor matches `gFontSmallNarrowLatinGlyphs` too, which `fonts.c` defines first, its
/// widths table immediately followed by the small font: the two hits must be exactly one font
/// apart, and the second is the small font.
pub fn locate_latin_small(rom: &Rom) -> Result<(u32, u32)> {
    let hits = rom.find_all(&ANCHOR_SMALL);
    let [narrow, small] = hits[..] else {
        bail!("latin small font anchor: expected 2 matches (narrow, small), found {hits:#x?}");
    };
    if small - narrow != FONT_BYTES {
        bail!(
            "latin small font anchor: matches {narrow:#x} and {small:#x} are not one font \
             ({FONT_BYTES:#x} bytes) apart"
        );
    }
    offsets_from_anchor(rom, small, "latin small")
}

/// Glyph and widths offsets of the font whose `A` glyph is at `hit`; checks the widths.
fn offsets_from_anchor(rom: &Rom, hit: u32, what: &str) -> Result<(u32, u32)> {
    let glyphs = hit
        .checked_sub(ANCHOR_CHAR * GLYPH_BYTES as u32)
        .with_context(|| format!("{what} font anchor too close to ROM start ({hit:#x})"))?;
    let widths = glyphs + (GLYPH_COUNT * GLYPH_BYTES) as u32;
    let table = rom
        .slice(widths, GLYPH_COUNT)
        .with_context(|| format!("{what} widths at {widths:#x}"))?;
    if let Some(i) = table.iter().position(|&w| w > MAX_WIDTH) {
        bail!(
            "{what} widths at {widths:#x}: entry {i:#x} is {} (> {MAX_WIDTH})",
            table[i]
        );
    }
    Ok((glyphs, widths))
}

/// Decodes a 64-byte `.latfont` glyph into 256 row-major palette indices.
fn decode_glyph(bytes: &[u8]) -> Vec<u8> {
    let mut px = vec![0; GLYPH_SIZE * GLYPH_SIZE];
    for (i, row) in bytes.chunks_exact(2).enumerate() {
        let (tile, y_in_tile) = (i / 8, i % 8);
        let v = u16::from_le_bytes([row[0], row[1]]);
        let y = (tile >> 1) * 8 + y_in_tile;
        for x in 0..8 {
            px[y * GLYPH_SIZE + (tile & 1) * 8 + x] = ((v >> (14 - 2 * x)) & 3) as u8;
        }
    }
    px
}

/// Reads the font from offsets returned by [`locate_latin_normal`] (avoids a second scan).
pub fn read_latin_normal_at(rom: &Rom, glyphs_off: u32, widths_off: u32) -> Result<Font> {
    read_latin_at(rom, glyphs_off, widths_off)
}

/// Reads any `.latfont` font (512 glyphs of 16x16) from its glyph and widths offsets, as
/// returned by [`locate_latin_normal`] or [`locate_latin_small`].
pub fn read_latin_at(rom: &Rom, glyphs_off: u32, widths_off: u32) -> Result<Font> {
    let data = rom
        .slice(glyphs_off, GLYPH_COUNT * GLYPH_BYTES)
        .with_context(|| format!("latin glyphs at {glyphs_off:#x}"))?;
    Ok(Font {
        glyph_width_max: GLYPH_SIZE as u8,
        glyph_height: GLYPH_SIZE as u8,
        glyphs: data.chunks_exact(GLYPH_BYTES).map(decode_glyph).collect(),
        widths: rom
            .slice(widths_off, GLYPH_COUNT)
            .with_context(|| format!("latin widths at {widths_off:#x}"))?
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_places_tiles_and_pixels() {
        let mut bytes = [0u8; GLYPH_BYTES];
        // Top-left tile, row 0: pixels 3,0,0,0,0,0,0,1 -> 0b11 << 14 | 0b01.
        bytes[..2].copy_from_slice(&0xC001u16.to_le_bytes());
        // Bottom-right tile (index 3), row 7: leftmost pixel 2.
        bytes[3 * 16 + 14..3 * 16 + 16].copy_from_slice(&0x8000u16.to_le_bytes());
        let px = decode_glyph(&bytes);
        assert_eq!(px[0], 3);
        assert_eq!(px[7], 1);
        assert_eq!(px[15 * 16 + 8], 2);
        assert_eq!(px.iter().filter(|&&p| p != 0).count(), 3);
    }

    #[test]
    fn small_font_is_the_second_of_two_anchors_one_font_apart() {
        let base = 0x100u32;
        let mut bytes = vec![0u8; (base + 2 * FONT_BYTES) as usize];
        bytes[0xAC..0xB0].copy_from_slice(b"BPEF");
        let a = (ANCHOR_CHAR * GLYPH_BYTES as u32) as usize;
        for font in 0..2 {
            let at = base as usize + font * FONT_BYTES as usize + a;
            bytes[at..at + GLYPH_BYTES].copy_from_slice(&ANCHOR_SMALL);
        }
        let rom = Rom::from_bytes(bytes.clone()).unwrap();
        let glyphs = base + FONT_BYTES;
        let widths = glyphs + (GLYPH_COUNT * GLYPH_BYTES) as u32;
        assert_eq!(locate_latin_small(&rom).unwrap(), (glyphs, widths));
        // A single match (no narrow font before it) is an error.
        let at = base as usize + a;
        bytes[at..at + GLYPH_BYTES].fill(0);
        assert!(locate_latin_small(&Rom::from_bytes(bytes).unwrap()).is_err());
    }

    #[test]
    fn locate_rejects_missing_anchor() {
        let mut bytes = vec![0u8; 0x200];
        bytes[0xAC..0xB0].copy_from_slice(b"BPEE");
        let rom = Rom::from_bytes(bytes).unwrap();
        assert!(locate_latin_normal(&rom).is_err());
    }
}
