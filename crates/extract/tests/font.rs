use extract::font::{self, Font};
use extract::{Rom, fixtures};

/// Reads the decomp's `latin_normal.png` (256 px wide, 2-bit indexed) as one palette index per pixel.
fn read_png_indices() -> (usize, usize, Vec<u8>) {
    let path = fixtures::decomp().join("graphics/fonts/latin_normal.png");
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Two);
    assert_eq!(info.color_type, png::ColorType::Indexed);
    let (w, h) = (info.width as usize, info.height as usize);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = &buf[y * info.line_size..][..info.line_size];
        for x in 0..w {
            px.push((row[x / 4] >> (6 - 2 * (x % 4))) & 3);
        }
    }
    (w, h, px)
}

/// The PNG's glyphs (16 per row of 16x16 cells), each as 256 row-major palette indices.
fn png_glyphs() -> Vec<Vec<u8>> {
    let (w, h, px) = read_png_indices();
    let mut glyphs = Vec::new();
    for gy in 0..h / 16 {
        for gx in 0..w / 16 {
            let mut g = Vec::with_capacity(256);
            for y in 0..16 {
                for x in 0..16 {
                    g.push(px[(gy * 16 + y) * w + gx * 16 + x]);
                }
            }
            glyphs.push(g);
        }
    }
    glyphs
}

/// gbagfx's `.latfont` encoding of one glyph: four 8x8 tiles (TL, TR, BL, BR), each row a
/// little-endian u16 with the leftmost pixel in bits 15-14.
fn encode_glyph(g: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    for tile in 0..4 {
        for row in 0..8 {
            let y = (tile >> 1) * 8 + row;
            let mut v: u16 = 0;
            for x in 0..8 {
                v |= (g[y * 16 + (tile & 1) * 8 + x] as u16) << (14 - 2 * x);
            }
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

// Character codes from charmap.txt.
const SPACE: usize = 0x00;
const E_ACUTE: usize = 0x1B;
const UPPER_A: usize = 0xBB;
const LOWER_A: usize = 0xD5;

/// Glyphs that BPEF redraws relative to `latin_normal.png` (US): `LV` (0x34), the unused
/// 0x57-0x59 and 0x64, the quotes 0xB1/0xB2 (French guillemets) and two symbols 0x105/0x108.
const FR_REDRAWN: [usize; 9] = [0x34, 0x57, 0x58, 0x59, 0x64, 0xB1, 0xB2, 0x105, 0x108];

/// Locates once, checks the raw bytes and the decoded font against `expected`, except the
/// glyphs listed in `redrawn`, which must differ.
fn check(rom: &Rom, expected: &[Vec<u8>], redrawn: &[usize]) -> ((u32, u32), Font) {
    let (glyphs_off, widths_off) = font::locate_latin_normal(rom).unwrap();
    assert_eq!(widths_off, glyphs_off + 512 * 64);
    let f = font::read_latin_normal_at(rom, glyphs_off, widths_off).unwrap();
    assert_eq!((f.glyph_width_max, f.glyph_height), (16, 16));
    assert_eq!(f.glyphs.len(), expected.len());
    for (i, (got, want)) in f.glyphs.iter().zip(expected).enumerate() {
        let raw = rom.slice(glyphs_off + i as u32 * 64, 64).unwrap();
        if redrawn.contains(&i) {
            assert_ne!(got, want, "glyph {i:#x} expected to differ");
        } else {
            assert_eq!(got, want, "glyph {i:#x} pixels");
            assert_eq!(raw, &encode_glyph(want)[..], "glyph {i:#x} bytes");
        }
    }
    assert_eq!(f.widths.len(), f.glyphs.len());
    for c in [SPACE, E_ACUTE, UPPER_A, LOWER_A] {
        assert!(
            (1..=16).contains(&f.widths[c]),
            "width of {c:#x} = {}",
            f.widths[c]
        );
    }
    ((glyphs_off, widths_off), f)
}

fn fonts_c_widths() -> Vec<u8> {
    let src = std::fs::read_to_string(fixtures::decomp().join("src/fonts.c")).unwrap();
    let body = &src[src.find("gFontNormalLatinGlyphWidths[] = {").unwrap()..];
    let body = &body[body.find('{').unwrap() + 1..body.find("};").unwrap()];
    body.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
fn us_latin_normal_matches_decomp() {
    let expected = png_glyphs();
    assert_eq!(expected.len(), 512);
    let (offsets, f) = check(&fixtures::us(), &expected, &[]);
    assert_eq!(offsets, (0x64c2e4, 0x6542e4));
    assert_eq!(f.widths, fonts_c_widths());
}

#[test]
fn fr_latin_normal_matches_decomp_except_redrawn_glyphs() {
    let (offsets, f) = check(&fixtures::fr(), &png_glyphs(), &FR_REDRAWN);
    assert_eq!(offsets, (0x650218, 0x658218));
    let us_widths = fonts_c_widths();
    let diff: Vec<usize> = (0..512).filter(|&i| f.widths[i] != us_widths[i]).collect();
    // `LV` is narrower in French (10 -> 8); the unused 0x64 gets a 12 px glyph (3 -> 12).
    assert_eq!(diff, vec![0x34, 0x64]);
    assert_eq!((f.widths[0x34], f.widths[0x64]), (8, 12));
}
