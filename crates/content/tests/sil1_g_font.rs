//! SIL1 Task G: the authored glyphs (R-11).
//! - In any flavour, on the build's own code (`build/silice/font.rs`): the committed
//!   `games/silice/font/*.ron` load, draw every character of `GLYPHS`, and merge into a font
//!   (glyph and width at the glyph index); a merge that breaks the metrics or misses a glyph
//!   fails.
//! - Originality, whenever an extract is present: no authored glyph repeats the foreground of
//!   an extracted glyph (see [`authored_glyphs_are_not_rom_glyphs`]).
//! - In the Silice build: the generated font holds the authored glyphs, and a text holding them
//!   renders with the authored pixels (the engine's canvas, natively).

#[allow(dead_code)]
#[path = "../build/silice/font.rs"]
mod font;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn games() -> PathBuf {
    workspace().join("games/silice")
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

/// A blank font of 512 glyphs, 6 px wide, with an `A` whose foreground ends on row 11 (the FR
/// font's baseline), 0xAF 3 px and 0x1E7 8 px wide (the FR widths).
fn synthetic_font() -> (Vec<[u8; 64]>, Vec<u8>) {
    let mut glyphs = vec![[0u8; 64]; 512];
    let mut a = vec![0u8; 256];
    a[11 * 16] = 1;
    glyphs[0xBB] = extract::games::font::encode_latfont(&a);
    let mut widths = vec![6u8; 512];
    widths[0xAF] = 3;
    widths[0x1E7] = 8;
    (glyphs, widths)
}

#[test]
fn committed_glyphs_cover_every_code() {
    let g = authored();
    let mut drawn: Vec<(char, &[u8])> = g.iter().map(|g| (g.ch, g.bytes)).collect();
    drawn.sort();
    let mut codes = font::GLYPHS.to_vec();
    codes.sort();
    assert_eq!(drawn, codes);
    assert_eq!(font::authored_glyph(heart()), Some(&[0xF9, 0xE7][..]));
    assert_eq!(font::authored_glyph(middle_dot()), Some(&[0xAF][..]));
    assert_eq!(font::authored_glyph('A'), None);
    for g in &g {
        let mut want = vec![0xBB];
        want.extend_from_slice(g.bytes);
        want.push(0xFF);
        assert_eq!(font::encode(&format!("A{}", g.ch)).unwrap(), want);
    }
    assert_eq!(
        g.iter().map(|g| g.index).collect::<BTreeSet<_>>(),
        BTreeSet::from([0xAF, 0x1E7])
    );
}

#[test]
fn merge_writes_glyphs_and_widths() {
    let (mut glyphs, mut widths) = synthetic_font();
    let g = authored();
    font::merge(&mut glyphs, &mut widths, &g).unwrap();
    for g in &g {
        let i = usize::from(g.index);
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
    for i in (0..512).filter(|&i| !g.iter().any(|g| usize::from(g.index) == i)) {
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
    // A redrawn glyph at another width than the font's.
    for i in [0xAF, 0x1E7] {
        let (mut glyphs, mut widths) = synthetic_font();
        widths[i] = 5;
        assert!(font::merge(&mut glyphs, &mut widths, &g).is_err(), "{i:#x}");
    }
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

/// The foreground pixels of a 16x16 glyph, moved to the top-left of their bounding box.
fn fg_mask(pixels: &[u8]) -> BTreeSet<(usize, usize)> {
    let pts: Vec<(usize, usize)> = (0..256)
        .filter(|&i| pixels[i] == 1)
        .map(|i| (i % 16, i / 16))
        .collect();
    let x0 = pts.iter().map(|p| p.0).min().unwrap_or(0);
    let y0 = pts.iter().map(|p| p.1).min().unwrap_or(0);
    pts.into_iter().map(|(x, y)| (x - x0, y - y0)).collect()
}

/// `mask` with runs of equal rows, then of equal columns, collapsed to one: a glyph made by
/// repeating a row or a column of another collapses to the same shape.
fn collapsed(mask: &BTreeSet<(usize, usize)>) -> Vec<Vec<bool>> {
    let w = mask.iter().map(|p| p.0 + 1).max().unwrap_or(0);
    let h = mask.iter().map(|p| p.1 + 1).max().unwrap_or(0);
    let mut rows: Vec<Vec<bool>> = (0..h)
        .map(|y| (0..w).map(|x| mask.contains(&(x, y))).collect())
        .collect();
    rows.dedup();
    let mut cols: Vec<Vec<bool>> = (0..w)
        .map(|x| rows.iter().map(|r| r[x]).collect())
        .collect();
    cols.dedup();
    cols
}

/// Glyphs must be original drawings (never ROM pixels): each authored glyph's foreground,
/// normalized to its top-left, differs from every extracted glyph's (normal and small fonts),
/// also after collapsing repeated rows and columns, so a ROM glyph stretched by a duplicated
/// row does not pass. Glyphs of 4 foreground pixels or fewer (the 2x2 middle dot) are exempt:
/// at that size every font draws the same few shapes, so a match says nothing about copying.
/// Skipped without an extract.
#[test]
fn authored_glyphs_are_not_rom_glyphs() {
    let dir = match std::env::var_os("PKR_EXTRACTED_DIR") {
        Some(d) => PathBuf::from(d),
        None => workspace().join("games/emerald"),
    };
    let fonts: Vec<PathBuf> = ["latin_normal", "latin_small"]
        .iter()
        .map(|n| dir.join(format!("font/{n}")))
        .filter(|p| p.with_extension("png").exists())
        .collect();
    if fonts.is_empty() {
        eprintln!("no extracted font under {}: skipped", dir.display());
        return;
    }
    let authored = authored();
    let mut checked = 0;
    for f in &fonts {
        let meta: extract::schema::FontFile =
            extract::export::read_ron(&f.with_extension("ron")).unwrap();
        let img = extract::gfx::read_png_indexed(&f.with_extension("png")).unwrap();
        let per_row = usize::from(meta.glyphs_per_row);
        for i in 0..usize::from(meta.glyphs) {
            let (ox, oy) = (i % per_row * 16, i / per_row * 16);
            let px: Vec<u8> = (0..256)
                .map(|k| img.pixels[(oy + k / 16) * img.width as usize + ox + k % 16] & 3)
                .collect();
            let rom = fg_mask(&px);
            for g in authored
                .iter()
                .filter(|g| g.pixels.iter().filter(|&&p| p == 1).count() > 4)
            {
                let mine = fg_mask(&g.pixels);
                assert!(
                    mine != rom && collapsed(&mine) != collapsed(&rom),
                    "{}: its foreground repeats glyph {i:#x} of {}",
                    g.name,
                    f.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn mask_helpers() {
    let a: BTreeSet<_> = [(3, 4), (4, 4), (3, 5)].into();
    assert_eq!(
        fg_mask(&{
            let mut p = vec![0u8; 256];
            for &(x, y) in &a {
                p[y * 16 + x] = 1;
            }
            p
        }),
        [(0, 0), (1, 0), (0, 1)].into()
    );
    // A duplicated row collapses back.
    let one: BTreeSet<_> = [(0, 0), (1, 0), (0, 1)].into();
    let two: BTreeSet<_> = [(0, 0), (1, 0), (0, 1), (0, 2)].into();
    assert_eq!(collapsed(&one), collapsed(&two));
}

#[cfg(pkr_silice)]
mod silice {
    use super::*;
    use engine::text::{Canvas, Colors, MSG_WINDOW};

    #[test]
    fn the_font_holds_the_authored_glyphs() {
        let f = &content::data::FONT;
        for g in authored() {
            let i = usize::from(g.index);
            assert_eq!(f.widths[i], g.width, "{}", g.name);
            assert_eq!(f.glyphs[i], g.latfont(), "{}", g.name);
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
