//! Labels and overlays drawn over renders, for review: metatile ids and behaviour codes on a
//! palette, events and collision on a map. Text uses a built-in 3x5 bitmap font (digits and a
//! few capitals), not the game's.
//!
//! Colours (`render --events`): objects yellow (labelled with their local id), warps magenta
//! (`W<index>`), coord events cyan (`C<index>`), signs green (`S<index>`), hidden items orange
//! (`H<index>`). `render --collision`: solid cells are tinted red, each cell's behaviour code
//! is drawn bottom-right (`behavior`), and cells whose collision disagrees with the usual one
//! are outlined yellow and marked `!` (green and `+` when an allowance accepts them).
//! `render --reach`: reached cells tinted green, the start framed white; `--sight`: the
//! trainers' lines of sight tinted orange.

use super::behavior::{allowed, behavior, collision_mismatches};
use super::catalog::Catalog;
use super::model::Doc;
use super::render::Palette;
use crate::map_view::MapImage;
use engine::data::{block_collision, block_metatile};
use extract::schema::BgKind;
use std::collections::{BTreeMap, BTreeSet};

/// Pixels of a glyph (before scaling); glyphs are drawn one pixel apart.
pub const GLYPH_W: usize = 3;
pub const GLYPH_H: usize = 5;

/// RGB555 (red in the low bits, as the GBA).
pub const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

pub const BLACK: u16 = rgb(0, 0, 0);
pub const WHITE: u16 = rgb(31, 31, 31);
pub const YELLOW: u16 = rgb(31, 31, 0);
pub const MAGENTA: u16 = rgb(31, 0, 31);
pub const CYAN: u16 = rgb(0, 31, 31);
pub const GREEN: u16 = rgb(0, 31, 0);
pub const ORANGE: u16 = rgb(31, 16, 0);
pub const RED: u16 = rgb(31, 0, 0);

/// Rows of 3 bits, the leftmost pixel in bit 2.
fn glyph(c: char) -> Option<[u8; 5]> {
    Some(match c {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [3, 4, 4, 4, 3],
        'D' => [6, 5, 5, 5, 6],
        'G' => [3, 4, 5, 5, 3],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'L' => [4, 4, 4, 4, 7],
        'S' => [3, 4, 2, 1, 6],
        'W' => [5, 5, 5, 7, 5],
        '!' => [2, 2, 2, 0, 2],
        '+' => [0, 2, 7, 2, 0],
        _ => return None,
    })
}

/// The width in pixels of `text` at scale `s`.
pub fn text_width(text: &str, s: usize) -> usize {
    let n = text.chars().count();
    if n == 0 {
        0
    } else {
        (n * (GLYPH_W + 1) - 1) * s
    }
}

/// `img` scaled up `s` times (nearest neighbour).
pub fn scale(img: &MapImage, s: usize) -> MapImage {
    let s = s.max(1);
    let (w, h) = (img.width * s, img.height * s);
    let mut px = vec![0; w * h];
    for y in 0..h {
        for x in 0..w {
            px[y * w + x] = img.rgb555[(y / s) * img.width + x / s];
        }
    }
    MapImage {
        width: w,
        height: h,
        rgb555: px,
        ..*img
    }
}

fn put(img: &mut MapImage, x: usize, y: usize, c: u16) {
    if x < img.width && y < img.height {
        img.rgb555[y * img.width + x] = c;
    }
}

pub fn fill(img: &mut MapImage, x: usize, y: usize, w: usize, h: usize, c: u16) {
    for py in y..y + h {
        for px in x..x + w {
            put(img, px, py, c);
        }
    }
}

/// Blends `c` over a rectangle, half and half.
pub fn tint(img: &mut MapImage, x: usize, y: usize, w: usize, h: usize, c: u16) {
    let mix = |a: u16, b: u16, shift: u16| (((a >> shift) & 31) + ((b >> shift) & 31)) / 2;
    for py in y..(y + h).min(img.height) {
        for px in x..(x + w).min(img.width) {
            let o = img.rgb555[py * img.width + px];
            img.rgb555[py * img.width + px] = rgb(mix(o, c, 0), mix(o, c, 5), mix(o, c, 10));
        }
    }
}

/// A `t`-pixel frame inside the rectangle.
pub fn outline(img: &mut MapImage, x: usize, y: usize, w: usize, h: usize, t: usize, c: u16) {
    fill(img, x, y, w, t, c);
    fill(img, x, y + h - t, w, t, c);
    fill(img, x, y, t, h, c);
    fill(img, x + w - t, y, t, h, c);
}

/// `text` in `fg` on a black box, its top-left at (x, y), at scale `s`. Characters the font
/// lacks are blanks.
pub fn text(img: &mut MapImage, x: usize, y: usize, text: &str, s: usize, fg: u16) {
    let w = text_width(text, s);
    fill(img, x, y, w + 2 * s, (GLYPH_H + 2) * s, BLACK);
    for (i, ch) in text.chars().enumerate() {
        let Some(rows) = glyph(ch) else {
            continue;
        };
        let gx = x + s + i * (GLYPH_W + 1) * s;
        for (ry, row) in rows.iter().enumerate() {
            for rx in 0..GLYPH_W {
                if row >> (GLYPH_W - 1 - rx) & 1 == 1 {
                    fill(img, gx + rx * s, y + s + ry * s, s, s, fg);
                }
            }
        }
    }
}

/// The box [`text`] draws: (width, height).
pub fn text_box(t: &str, s: usize) -> (usize, usize) {
    (text_width(t, s) + 2 * s, (GLYPH_H + 2) * s)
}

/// Gutter colour of a labelled palette.
pub const GUTTER: u16 = rgb(4, 4, 6);

/// The labelled palette: each cell's art at `zoom`, under a gutter band holding its metatile
/// id (left) and, with `codes`, its behaviour code (right), in text at scale `s`. Labels never
/// cover the art. Columns are at least wide enough for a 4-digit id and a code, and are
/// separated by an `s`-pixel gutter.
pub fn label_palette(
    p: &Palette,
    c: &Catalog,
    primary: &str,
    secondary: &str,
    s: usize,
    zoom: usize,
    codes: bool,
) -> MapImage {
    let art = scale(&p.image, zoom);
    let (cell, cols) = (16 * zoom.max(1), p.cols as usize);
    let rows = p.metatiles.len().div_ceil(cols);
    let band = text_box("0", s).1;
    let label_w = text_box("0000", s).0 + s + text_box("D", s).0;
    let col_w = cell.max(label_w) + s;
    let row_h = band + cell + s;
    let (w, h) = (cols * col_w, rows * row_h);
    let mut img = MapImage {
        width: w,
        height: h,
        rgb555: vec![GUTTER; w * h],
        margin: 0,
        ..art
    };
    for (i, m) in p.metatiles.iter().enumerate() {
        let Some(m) = *m else {
            continue;
        };
        let (cx, cy) = (i % cols, i / cols);
        let (x, y) = (cx * col_w, cy * row_h);
        for py in 0..cell {
            let src = (cy * cell + py) * art.width + cx * cell;
            let dst = (y + band + py) * w + x;
            img.rgb555[dst..dst + cell].copy_from_slice(&art.rgb555[src..src + cell]);
        }
        text(&mut img, x, y, &m.to_string(), s, WHITE);
        if codes && let Some(code) = behavior(c, primary, secondary, m).and_then(|b| b.code()) {
            let t = code.to_string();
            let (bw, _) = text_box(&t, s);
            text(&mut img, x + col_w - s - bw, y, &t, s, YELLOW);
        }
    }
    img
}

/// The palette's behaviour legend: each code present, what it means, and its metatiles by
/// `MB_*` name.
pub fn palette_legend(p: &Palette, c: &Catalog, primary: &str, secondary: &str) -> String {
    let mut by_code = BTreeMap::<char, BTreeMap<String, Vec<u16>>>::new();
    for m in p.metatiles.iter().flatten() {
        if let Some(b) = behavior(c, primary, secondary, *m)
            && let Some(code) = b.code()
        {
            by_code
                .entry(code)
                .or_default()
                .entry(b.name)
                .or_default()
                .push(*m);
        }
    }
    let mut out = String::new();
    for (code, what) in super::behavior::LEGEND {
        let Some(names) = by_code.get(&code) else {
            continue;
        };
        out.push_str(&format!("{code} {what}\n"));
        for (name, ms) in names {
            let ms: Vec<String> = ms.iter().map(|m| m.to_string()).collect();
            out.push_str(&format!("    {name}: {}\n", ms.join(" ")));
        }
    }
    if out.is_empty() {
        out.push_str("no metatile has a behaviour code\n");
    }
    out
}

/// The top-left pixel of map cell (x, y) in a render with `img.margin` at scale `s`.
fn cell_px(img: &MapImage, x: i32, y: i32, s: usize) -> Option<(usize, usize)> {
    let m = img.margin as i32;
    let (cx, cy) = (x + m, y + m);
    (cx >= 0 && cy >= 0).then(|| (cx as usize * 16 * s, cy as usize * 16 * s))
}

/// Marks every event of `d` on its render (scaled `s` times): a coloured frame on its cell and
/// its label (stacked when several share a cell).
pub fn draw_events(img: &mut MapImage, d: &Doc, s: usize) {
    let m = &d.map;
    let mut marks: Vec<(i16, i16, u16, String)> = Vec::new();
    for o in &m.objects {
        marks.push((o.x, o.y, YELLOW, o.local_id.to_string()));
    }
    for (i, w) in m.warps.iter().enumerate() {
        marks.push((w.x, w.y, MAGENTA, format!("W{i}")));
    }
    for (i, c) in m.coords.iter().enumerate() {
        marks.push((c.x, c.y, CYAN, format!("C{i}")));
    }
    for (i, b) in m.bgs.iter().enumerate() {
        let (col, tag) = match b.kind {
            BgKind::HiddenItem { .. } => (ORANGE, "H"),
            _ => (GREEN, "S"),
        };
        marks.push((b.x, b.y, col, format!("{tag}{i}")));
    }
    let mut stacked = BTreeMap::<(i16, i16), usize>::new();
    let cell = 16 * s;
    for (x, y, col, label) in marks {
        let Some((px, py)) = cell_px(img, x as i32, y as i32, s) else {
            continue;
        };
        let k = stacked.entry((x, y)).or_default();
        if *k == 0 {
            outline(img, px, py, cell, cell, s, col);
        }
        let (_, bh) = text_box(&label, s);
        text(img, px + s, py + s + *k * bh, &label, s, col);
        *k += 1;
    }
}

/// Tints solid cells red, draws each cell's behaviour code and marks the cells whose
/// collision disagrees with the usual one (`behavior::collision_mismatches`).
///
/// A mismatch an allowance accepts (`Op::AllowCollision`, `Doc::allow`) is framed green and
/// marked `+` instead.
pub fn draw_collision(img: &mut MapImage, d: &Doc, c: &Catalog, s: usize) {
    let cell = 16 * s;
    for y in 0..d.height as i32 {
        for x in 0..d.width as i32 {
            let (Some(b), Some((px, py))) = (d.block(x, y), cell_px(img, x, y, s)) else {
                continue;
            };
            if block_collision(b) != 0 {
                tint(img, px, py, cell, cell, RED);
            }
            if let Some(code) =
                behavior(c, &d.primary, &d.secondary, block_metatile(b)).and_then(|b| b.code())
            {
                let t = code.to_string();
                let (bw, bh) = text_box(&t, s);
                text(img, px + cell - bw, py + cell - bh, &t, s, WHITE);
            }
        }
    }
    for (x, y, _, _) in collision_mismatches(d, c) {
        let Some((px, py)) = cell_px(img, x, y, s) else {
            continue;
        };
        let mt = d.block(x, y).map_or(0, block_metatile);
        let (mark, col) = match allowed(&d.allow, x, y, mt) {
            Some(_) => ("+", GREEN),
            None => ("!", YELLOW),
        };
        outline(img, px, py, cell, cell, s, col);
        let (_, bh) = text_box(mark, s);
        text(img, px + s, py + cell - bh - s, mark, s, col);
    }
}

/// `render --reach`: tints the reached `cells` green and frames the `start` cell white.
pub fn draw_reach(img: &mut MapImage, cells: &BTreeSet<(i32, i32)>, start: (i32, i32), s: usize) {
    let cell = 16 * s;
    for &(x, y) in cells {
        if let Some((px, py)) = cell_px(img, x, y, s) {
            tint(img, px, py, cell, cell, GREEN);
        }
    }
    if let Some((px, py)) = cell_px(img, start.0, start.1, s) {
        outline(img, px, py, cell, cell, 2 * s, WHITE);
    }
}

/// `render --sight`: tints the cells a trainer sees orange.
pub fn draw_sight(img: &mut MapImage, cells: &BTreeSet<(i32, i32)>, s: usize) {
    let cell = 16 * s;
    for &(x, y) in cells {
        if let Some((px, py)) = cell_px(img, x, y, s) {
            tint(img, px, py, cell, cell, ORANGE);
        }
    }
}
