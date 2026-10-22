use super::*;
use alloc::boxed::Box;

/// Glyph 1: 3 px wide, fg at (0, 0); every other glyph is 0 px wide.
fn font() -> &'static Font {
    let mut glyphs = vec![[0u8; 64]; 0x200];
    glyphs[1][1] = 0x40; // row 0, pixel 0 = 2bpp 1 (fg)
    let mut widths = vec![0u8; 0x200];
    widths[1] = 3;
    Box::leak(Box::new(Font {
        glyphs: Box::leak(glyphs.into_boxed_slice()),
        widths: Box::leak(widths.into_boxed_slice()),
    }))
}

const C: Colors = Colors::new(2, 3, 1);

/// A per-pixel model of `draw_glyph` (Gen3 `CopyGlyphToWindow`) and `fill_rect`, for the
/// word-at-a-time versions.
fn model_glyph(px: &mut [[u8; 40]; 24], glyph: &[u8; 64], width: u8, x: i16, y: i16) {
    let map = [C.bg, C.fg, C.shadow, C.bg];
    for row in 0..GLYPH_HEIGHT {
        for col in 0..(width as i16).min(16) {
            let (cx, cy) = (x + col, y + row);
            if !(0..40).contains(&cx) || !(0..24).contains(&cy) {
                continue;
            }
            let (r, c) = (row as usize, col as usize);
            let off = ((r / 8) * 2 + c / 8) * 16 + (r % 8) * 2;
            let byte = if c % 8 < 4 {
                glyph[off + 1]
            } else {
                glyph[off]
            };
            let v = (byte >> (6 - 2 * (c % 4))) & 3;
            px[cy as usize][cx as usize] = map[v as usize];
        }
    }
}

#[test]
fn word_drawing_matches_the_pixel_model() {
    let mut seed = 0x1234_5678u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as u8
    };
    let glyphs: Vec<[u8; 64]> = (0..17).map(|_| core::array::from_fn(|_| next())).collect();
    let font = Box::leak(Box::new(Font {
        glyphs: Box::leak(glyphs.clone().into_boxed_slice()),
        widths: Box::leak((0..17u8).collect::<Vec<_>>().into_boxed_slice()),
    }));
    let pen = Pen::new(C);
    for id in 0..17u16 {
        for (x, y) in [
            (-1, 0),
            (0, 0),
            (3, 2),
            (7, -4),
            (13, 9),
            (30, 11),
            (36, 20),
        ] {
            let mut c = Canvas::new(Window::new(0, 0, 5, 3, 0), 0);
            c.fill(7);
            let mut model = [[7u8; 40]; 24];
            c.draw_glyph(font, id, x, y, &pen);
            if x >= 0 {
                model_glyph(&mut model, &glyphs[id as usize], id as u8, x, y);
            }
            c.fill_rect(x + 2, y - 1, id as i16, 5, 9);
            for py in (y - 1).max(0)..(y + 4).min(24) {
                for px in (x + 2).max(0)..(x + 2 + id as i16).min(40) {
                    model[py as usize][px as usize] = 9;
                }
            }
            for py in 0..24 {
                for px in 0..40 {
                    let got = c.pixel(px, py);
                    let want = model[py as usize][px as usize];
                    assert_eq!(got, want, "glyph {id} at ({x}, {y}), pixel ({px}, {py})");
                }
            }
        }
    }
}

#[test]
fn draw_text_clear_to_and_newlines() {
    let mut c = Canvas::new(Window::new(0, 1, 4, 4, 0), 0);
    c.fill(7);
    // "1{CLEAR_TO 16}1\n1"
    c.draw_text(font(), &[1, 0xFC, 0x13, 16, 1, 0xFE, 1, 0xFF], 0, 1, C);
    assert_eq!(c.pixel(0, 1), 2);
    assert_eq!(c.pixel(1, 1), 1, "glyph background");
    // CLEAR_TO clears x = 3..16 over a line height (rows 1..17) to the background.
    assert_eq!(c.pixel(3, 1), 1);
    assert_eq!(c.pixel(15, 16), 1);
    assert_eq!(c.pixel(3, 0), 7, "above the line");
    assert_eq!(c.pixel(16, 1), 2, "glyph at origin + 16");
    assert_eq!(c.pixel(19, 1), 7, "past the second glyph");
    assert_eq!(c.pixel(0, 17), 2, "second line at x = origin, y + 16");
}

#[test]
fn draw_text_skip_and_colors() {
    let mut c = Canvas::new(Window::new(0, 1, 4, 2, 0), 0);
    c.fill(7);
    // "{SKIP 8}{COLOR_HIGHLIGHT_SHADOW 13 14 15}1"
    c.draw_text(
        font(),
        &[0xFC, 0x12, 8, 0xFC, 4, 13, 14, 15, 1, 0xFF],
        2,
        0,
        C,
    );
    assert_eq!(c.pixel(2, 0), 7, "skipped, not cleared");
    assert_eq!(c.pixel(10, 0), 13);
    assert_eq!(c.pixel(11, 0), 14);
}

/// A redraw: the frame's tile uploads and whether pixel (0, 0) is glyph 1's foreground.
fn redraw(c: &mut Canvas, text: &[u8], colors: Colors) -> (usize, u8) {
    c.redraw(font(), text, 0, 0, colors);
    let mut f = Frame::default();
    c.flush(&mut f);
    (f.cmds.len(), c.pixel(0, 0))
}

#[test]
fn redraw_skips_the_same_text() {
    let mut c = Canvas::new(Window::new(0, 1, 2, 2, 0), 0);
    assert_eq!(redraw(&mut c, &[1, 0xFF], C), (4, 2));
    assert_eq!(
        redraw(&mut c, &[1, 0xFF], C),
        (0, 2),
        "nothing new to upload"
    );
    assert_eq!(redraw(&mut c, &[0xFF], C), (4, 1), "other text");
    assert_eq!(redraw(&mut c, &[0xFF], C), (0, 1));
}

#[test]
fn redraw_after_other_drawing() {
    let mut c = Canvas::new(Window::new(0, 1, 2, 2, 0), 0);
    redraw(&mut c, &[1, 0xFF], C);
    c.fill(7);
    assert_eq!(redraw(&mut c, &[1, 0xFF], C), (4, 2), "after fill");
    c.draw_glyph(font(), 1, 8, 8, &Pen::new(C));
    assert_eq!(redraw(&mut c, &[1, 0xFF], C), (4, 2), "after draw_glyph");
    assert_eq!(c.pixel(8, 8), 1, "the extra glyph is gone");
}

#[test]
fn redraw_when_only_the_colors_change() {
    let mut c = Canvas::new(Window::new(0, 1, 2, 2, 0), 0);
    redraw(&mut c, &[1, 0xFF], C);
    let other = Colors::new(5, 3, 1);
    assert_eq!(redraw(&mut c, &[1, 0xFF], other), (4, 5));
    assert_eq!(redraw(&mut c, &[1, 0xFF], other), (0, 5));
}

#[test]
fn printer_origin_and_speed() {
    let mut p = Printer::new(Window::new(0, 1, 4, 4, 0), 0, (1, 1), 2);
    p.start(vec![1, 1, 0xFF], C, true);
    p.update(font(), Buttons::NONE, Buttons::NONE);
    assert_eq!(p.canvas().pixel(1, 1), 2);
    assert!(p.is_printing());
    p.update(font(), Buttons::NONE, Buttons::NONE); // delay
    assert_eq!(p.canvas().pixel(4, 1), 1);
    p.update(font(), Buttons::NONE, Buttons::NONE);
    assert_eq!(p.canvas().pixel(4, 1), 2);
    assert!(!p.is_printing());
}
