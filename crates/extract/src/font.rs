//! Latin normal font (`gFontNormalLatinGlyphs` / `gFontNormalLatinGlyphWidths` in the decomp's
//! `src/fonts.c`).
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
    let glyphs = hit
        .checked_sub(ANCHOR_CHAR * GLYPH_BYTES as u32)
        .with_context(|| format!("latin normal font anchor too close to ROM start ({hit:#x})"))?;
    let widths = glyphs + (GLYPH_COUNT * GLYPH_BYTES) as u32;
    let table = rom
        .slice(widths, GLYPH_COUNT)
        .with_context(|| format!("latin normal widths at {widths:#x}"))?;
    if let Some(i) = table.iter().position(|&w| w > MAX_WIDTH) {
        bail!(
            "latin normal widths at {widths:#x}: entry {i:#x} is {} (> {MAX_WIDTH})",
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
    let data = rom
        .slice(glyphs_off, GLYPH_COUNT * GLYPH_BYTES)
        .with_context(|| format!("latin normal glyphs at {glyphs_off:#x}"))?;
    Ok(Font {
        glyph_width_max: GLYPH_SIZE as u8,
        glyph_height: GLYPH_SIZE as u8,
        glyphs: data.chunks_exact(GLYPH_BYTES).map(decode_glyph).collect(),
        widths: rom
            .slice(widths_off, GLYPH_COUNT)
            .with_context(|| format!("latin normal widths at {widths_off:#x}"))?
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
    fn locate_rejects_missing_anchor() {
        let mut bytes = vec![0u8; 0x200];
        bytes[0xAC..0xB0].copy_from_slice(b"BPEE");
        let rom = Rom::from_bytes(bytes).unwrap();
        assert!(locate_latin_normal(&rom).is_err());
    }
}
