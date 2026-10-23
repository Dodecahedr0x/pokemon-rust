//! SIL1 Task G: the authored glyphs (R-11).
//! - In any flavour, on the build's own code (`build/silice/font.rs`): the committed
//!   `games/silice/font/*.ron` load, draw every reserved code, and merge into a font (glyph and
//!   width at the code); a merge that breaks the metrics or misses a glyph fails.
//! - In the Silice build: the generated font holds the authored glyphs, and a text holding them
//!   renders with the authored pixels (the engine's canvas, natively).

#[allow(dead_code)]
#[path = "../build/silice/font.rs"]
mod font;

use std::path::{Path, PathBuf};

fn games() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../games/silice")
}

fn authored() -> Vec<font::Glyph> {
    font::load(&games()).unwrap()
}

fn heart() -> char {
    char::from_u32(0x2665).unwrap()
}

fn middle_dot() -> char {
    char::from_u32(0xB7).unwrap()
}

/// A blank font of 512 glyphs, 3 px wide, with an `A` whose foreground ends on row 11 (the FR
/// font's baseline) and 0xAF 3 px wide.
fn synthetic_font() -> (Vec<[u8; 64]>, Vec<u8>) {
    let mut glyphs = vec![[0u8; 64]; 512];
    let mut a = vec![0u8; 256];
    a[11 * 16] = 1;
    glyphs[0xBB] = extract::games::font::encode_latfont(&a);
    (glyphs, vec![3u8; 512])
}

#[test]
fn committed_glyphs_cover_every_code() {
    let g = authored();
    let mut drawn: Vec<(char, u8)> = g.iter().map(|g| (g.ch, g.code)).collect();
    drawn.sort();
    let mut codes = font::GLYPHS.to_vec();
    codes.sort();
    assert_eq!(drawn, codes);
    assert_eq!(font::authored_glyph(heart()), Some(0x37));
    assert_eq!(font::authored_glyph(middle_dot()), Some(0xAF));
    assert_eq!(font::authored_glyph('A'), None);
    for g in &g {
        assert_eq!(
            font::encode(&format!("A{}", g.ch)).unwrap(),
            vec![0xBB, g.code, 0xFF]
        );
    }
}

#[test]
fn merge_writes_glyphs_and_widths() {
    let (mut glyphs, mut widths) = synthetic_font();
    let g = authored();
    font::merge(&mut glyphs, &mut widths, &g).unwrap();
    for g in &g {
        let i = usize::from(g.code);
        assert_eq!(widths[i], g.width, "{}", g.name);
        assert_eq!(
            extract::games::font::decode_latfont(&glyphs[i]),
            g.pixels,
            "{}",
            g.name
        );
    }
    // Nothing else moved.
    let (blank, w) = synthetic_font();
    for i in (0..512).filter(|&i| !g.iter().any(|g| usize::from(g.code) == i)) {
        assert_eq!((glyphs[i], widths[i]), (blank[i], w[i]), "{i:#x}");
    }
}

#[test]
fn merge_rejects_broken_metrics() {
    let g = authored();
    // A missing glyph.
    let (mut glyphs, mut widths) = synthetic_font();
    let one: Vec<_> = g.iter().filter(|g| g.ch == heart()).cloned().collect();
    let e = font::merge(&mut glyphs, &mut widths, &one).unwrap_err();
    assert!(format!("{e:#}").contains("no glyph file"), "{e:#}");
    // A redrawn code at another width than the font's.
    let (mut glyphs, mut widths) = synthetic_font();
    widths[0xAF] = 4;
    assert!(font::merge(&mut glyphs, &mut widths, &g).is_err());
    // A reserved code the font already draws.
    let (mut glyphs, mut widths) = synthetic_font();
    glyphs[0x37][0] = 0x40;
    assert!(font::merge(&mut glyphs, &mut widths, &g).is_err());
    // Another baseline.
    let (mut glyphs, mut widths) = synthetic_font();
    let mut a = vec![0u8; 256];
    a[10 * 16] = 1;
    glyphs[0xBB] = extract::games::font::encode_latfont(&a);
    assert!(font::merge(&mut glyphs, &mut widths, &g).is_err());
    // No glyph files: nothing to merge.
    let (mut glyphs, mut widths) = synthetic_font();
    font::merge(&mut glyphs, &mut widths, &[]).unwrap();
    assert_eq!((glyphs, widths), synthetic_font());
}

#[cfg(pkr_silice)]
mod silice {
    use super::*;
    use engine::text::{Canvas, Colors, MSG_WINDOW};

    #[test]
    fn the_font_holds_the_authored_widths() {
        let f = &content::data::FONT;
        for g in authored() {
            assert_eq!(f.widths[usize::from(g.code)], g.width, "{}", g.name);
        }
    }

    /// Renders `A`, each authored glyph, then `A`, and compares every pixel of each glyph's
    /// advance with its RON drawing (foreground, shadow, background colors).
    #[test]
    fn a_text_renders_the_authored_pixels() {
        let f = &content::data::FONT;
        let g = authored();
        let text: String = core::iter::once('A')
            .chain(g.iter().map(|g| g.ch))
            .chain(core::iter::once('A'))
            .collect();
        let bytes = font::encode(&text).unwrap();
        let colors = Colors::new(2, 3, 1);
        let mut c = Canvas::new(MSG_WINDOW, 0);
        c.redraw(f, &bytes, 0, 0, colors);
        let mut x = i16::from(f.widths[0xBB]);
        for g in &g {
            for y in 0..16usize {
                for dx in 0..usize::from(g.width) {
                    let want = match g.pixels[y * 16 + dx] {
                        1 => colors.fg,
                        2 => colors.shadow,
                        _ => colors.bg,
                    };
                    let got = c.pixel(x + dx as i16, y as i16);
                    assert_eq!(got, want, "{} at ({dx}, {y})", g.name);
                }
            }
            x += i16::from(g.width);
        }
        // The next glyph starts right after the advance: the `A`'s first column has ink.
        assert!((0..16).any(|y| c.pixel(x, y) == colors.fg));
    }
}
