//! Derived art: features drawn from other features' art instead of a prompt (a switched-off
//! jammer, glowing floor slabs, a wall screen, an object set on its floor). `gen` runs a
//! tileset's derivation after prep, so the derived art follows its sources; the derived
//! features have no prompt (`gen` leaves their art to this module).

use super::gfx::{Rgba, read_rgba, write_rgba};
use anyhow::Result;
use std::path::Path;

/// Runs the derivation of tileset `name` (art dir `gfx`), if it has one.
pub fn run(name: &str, gfx: &Path) -> Result<bool> {
    match name {
        "SilGrisEchec" => gris_echec(&gfx.join(name))?,
        "SilGrisEchecGym" => gris_echec_gym(&gfx.join(name))?,
        "SilReverseCity" => reverse_city(&gfx.join(name))?,
        "SilMangaSquareGym" => manga_square_gym(&gfx.join(name))?,
        "SilNebulaTourHall" => nebula_tour_hall(&gfx.join(name))?,
        "SilOwlys" => owlys(&gfx.join(name))?,
        "SilOwlysGym" => owlys_gym(&gfx.join(name))?,
        "SilEvalonCitadelle" => evalon_citadelle(&gfx.join(name))?,
        "SilRouteS06" => route_s06(&gfx.join(name))?,
        "SilRouteS07" => route_s07(&gfx.join(name))?,
        "SilRoutesS2S3" => routes_s2s3(&gfx.join(name))?,
        _ => return Ok(false),
    }
    println!("derived art for {name}");
    Ok(true)
}

fn load(dir: &Path, f: &str) -> Result<Rgba> {
    read_rgba(&dir.join(format!("{f}.png")))
}

fn save(dir: &Path, f: &str, img: &Rgba) -> Result<()> {
    write_rgba(&dir.join(format!("{f}.png")), img)
}

fn get(img: &Rgba, x: u32, y: u32) -> [u8; 4] {
    img.px[(y * img.w + x) as usize]
}

fn set(img: &mut Rgba, x: u32, y: u32, rgb: [u8; 3]) {
    img.px[(y * img.w + x) as usize] = [rgb[0], rgb[1], rgb[2], 255];
}

fn crop(img: &Rgba, x0: u32, y0: u32, w: u32, h: u32) -> Rgba {
    Rgba {
        w,
        h,
        px: (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| get(img, x0 + x, y0 + y))
            .collect(),
    }
}

/// `base` tiled to `top`'s size, `top`'s opaque pixels over it.
fn over(base: &Rgba, top: &Rgba) -> Rgba {
    let mut out = tile(base, top.w, top.h);
    for (o, p) in out.px.iter_mut().zip(&top.px) {
        if p[3] >= 128 {
            *o = [p[0], p[1], p[2], 255];
        }
    }
    out
}

fn tile(img: &Rgba, w: u32, h: u32) -> Rgba {
    Rgba {
        w,
        h,
        px: (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| get(img, x % img.w, y % img.h))
            .collect(),
    }
}

fn lum(c: [u8; 4]) -> u32 {
    (c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32) / 10
}

/// SilGrisEchec: the roof jammer J1 (on, off), the bin on a white square, the Tour's door
/// crackling with static.
fn gris_echec(t: &Path) -> Result<()> {
    let roof = crop(&load(t, "annexe")?, 48, 16, 16, 16);
    let jam = load(t, "brouilleur")?;
    save(t, "j1", &over(&roof, &jam))?;
    // Switched off: the saturated pixels go dull grey.
    let mut off = jam.clone();
    for p in off.px.iter_mut().filter(|p| p[3] >= 128) {
        let (hi, lo) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
        if hi - lo > 60 {
            let v = (lum(*p) / 2 + 30) as u8;
            *p = [v, v, v, 255];
        }
    }
    save(t, "j1_hs", &over(&roof, &off))?;
    save(
        t,
        "poubelle_case",
        &over(&load(t, "case_blanche")?, &load(t, "poubelle")?),
    )?;
    // The door leaf (not its frame line at the edges) becomes grey snow.
    let mut door = crop(&load(t, "tour")?, 16, 64, 32, 16);
    let mut seed = 2u32;
    for y in 1..16 {
        for x in 3..29 {
            if get(&door, x, y)[3] >= 128 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let v = [70, 110, 150, 190, 230][(seed >> 16) as usize % 5];
                set(&mut door, x, y, [v, v, v]);
            }
        }
    }
    save(t, "tour_porte_brouillee", &door)
}

/// SilGrisEchecGym: the lit maze floor, the binary slabs 0 and 1, the wall screen, the objects
/// set on their floor.
fn gris_echec_gym(g: &Path) -> Result<()> {
    let sol = load(g, "sol")?;
    let mut lit = sol.clone();
    for y in 0..16 {
        for x in 0..16 {
            let p = get(&lit, x, y);
            let c = if x == 0 || x == 15 || y == 0 || y == 15 {
                [120, 248, 248]
            } else if x == 1 || x == 14 || y == 1 || y == 14 {
                [56, 168, 200]
            } else {
                [
                    (p[0] / 2).saturating_add(20),
                    (p[1] / 2).saturating_add(70),
                    (p[2] / 2).saturating_add(90),
                ]
            };
            set(&mut lit, x, y, c);
        }
    }
    save(g, "sol_allume", &lit)?;
    const ZERO: [&str; 8] = [
        "..####..", ".##..##.", ".##..##.", ".##..##.", ".##..##.", ".##..##.", ".##..##.",
        "..####..",
    ];
    const ONE: [&str; 8] = [
        "...##...", "..###...", ".####...", "...##...", "...##...", "...##...", "...##...",
        ".######.",
    ];
    for (name, glyph, col, dim) in [
        ("dalle_0", ZERO, [96, 240, 128], [24, 96, 48]),
        ("dalle_1", ONE, [248, 168, 64], [112, 64, 24]),
    ] {
        let mut d = sol.clone();
        for y in 2..14 {
            for x in 2..14 {
                let edge = x == 2 || x == 13 || y == 2 || y == 13;
                set(&mut d, x, y, if edge { dim } else { [24, 28, 36] });
            }
        }
        for (gy, row) in glyph.iter().enumerate() {
            for (gx, ch) in row.chars().enumerate() {
                if ch == '#' {
                    set(&mut d, 4 + gx as u32, 4 + gy as u32, col);
                }
            }
        }
        save(g, name, &d)?;
    }
    let mut ecran = load(g, "mur")?;
    for y in 18..29 {
        for x in 2..14 {
            let edge = x == 2 || x == 13 || y == 18 || y == 28;
            set(
                &mut ecran,
                x,
                y,
                if edge { [40, 48, 56] } else { [8, 40, 24] },
            );
        }
    }
    for x in 4..11 {
        set(&mut ecran, x, 21, [96, 240, 128]);
    }
    for x in 4..8 {
        set(&mut ecran, x, 24, [96, 240, 128]);
    }
    set(&mut ecran, 4, 21, [200, 255, 210]);
    save(g, "mur_ecran", &ecran)?;
    save(g, "statue", &over(&sol, &load(g, "statue_roi")?))?;
    save(g, "terminal_sol", &over(&sol, &load(g, "terminal")?))?;
    save(g, "pad_sol", &over(&sol, &load(g, "pad")?))?;
    save(
        g,
        "bloc_mousse_sol",
        &over(&load(g, "mousse_sol")?, &load(g, "bloc_mousse")?),
    )
}

fn flip_v(img: &Rgba) -> Rgba {
    Rgba {
        w: img.w,
        h: img.h,
        px: (0..img.h)
            .flat_map(|y| (0..img.w).map(move |x| (x, y)))
            .map(|(x, y)| get(img, x, img.h - 1 - y))
            .collect(),
    }
}

/// SilReverseCity: the house upside down (on its roof; hanging from the ceiling, its door row
/// lost in the rock), the open Porte Eta-Nord, the quay's water edges (paving and a dark lip over
/// the red water).
fn reverse_city(r: &Path) -> Result<()> {
    let flipped = flip_v(&load(r, "maison")?);
    save(r, "maison_renversee", &flipped)?;
    save(r, "maison_suspendue", &crop(&flipped, 0, 16, 48, 48))?;
    // The shutter's leaf (inside its frame) becomes the dark open passage.
    let mut open = crop(&load(r, "porte_nord")?, 32, 0, 16, 16);
    for y in 3..16 {
        for x in 2..14 {
            set(
                &mut open,
                x,
                y,
                if y < 5 { [40, 32, 56] } else { [12, 12, 20] },
            );
        }
    }
    save(r, "porte_nord_ouverte", &open)?;
    let (eau, pave) = (load(r, "eau")?, load(r, "pave")?);
    let lip = [20, 20, 28];
    let (mut quai, mut droite, mut mur) = (eau.clone(), eau.clone(), eau.clone());
    let px = |x, y| {
        let p = get(&pave, x, y);
        [p[0], p[1], p[2]]
    };
    for i in 0..16 {
        for j in 0..4 {
            set(&mut quai, i, j, px(i, j));
            set(&mut droite, 12 + j, i, px(12 + j, i));
        }
        set(&mut quai, i, 4, lip);
        set(&mut droite, 11, i, lip);
        for j in 12..16 {
            set(&mut mur, i, j, px(i, j));
        }
        set(&mut mur, i, 10, lip);
        set(&mut mur, i, 11, [56, 56, 72]);
    }
    save(r, "eau_quai", &quai)?;
    save(r, "eau_droite", &droite)?;
    save(r, "eau_mur", &mur)
}

/// SilMangaSquareGym: the opened shoji door (`shoji_ouverte`, METATILE_SHOJI_OUVERT): each panel
/// of `shoji_porte` squeezed to half its width against its side, the stepping stone between.
fn manga_square_gym(t: &Path) -> Result<()> {
    let door = load(t, "shoji_porte")?;
    let mut open = load(t, "dalle")?;
    for y in 0..16 {
        for x in 0..4 {
            let l = get(&door, 2 * x, y);
            let r = get(&door, 8 + 2 * x, y);
            set(&mut open, x, y, [l[0], l[1], l[2]]);
            set(&mut open, 12 + x, y, [r[0], r[1], r[2]]);
        }
    }
    save(t, "shoji_ouverte", &open)
}

/// SilNebulaTourHall: the closed laser sas, three red beams over the corridor floor between black
/// emitter studs: `laser_h` beams left to right, `laser_v` the same turned a quarter.
fn nebula_tour_hall(t: &Path) -> Result<()> {
    let floor = load(t, "couloir")?;
    let mut h = floor.clone();
    for y in [4, 8, 12] {
        for x in 0..16 {
            let c = if !(2..=13).contains(&x) {
                [24, 24, 28]
            } else {
                [248, 56, 56]
            };
            set(&mut h, x, y, c);
            if (2..14).contains(&x) {
                set(&mut h, x, y - 1, [160, 24, 32]);
            }
        }
    }
    save(t, "laser_h", &h)?;
    let mut v = floor.clone();
    for y in 0..16 {
        for x in 0..16 {
            let p = get(&h, y, x);
            set(&mut v, x, y, [p[0], p[1], p[2]]);
        }
    }
    save(t, "laser_v", &v)
}

/// SilOwlys: a gargoyle perched on the left rim (the Ruelle des Gargouilles); the flagstones of
/// the gate's arrows.
fn owlys(t: &Path) -> Result<()> {
    save(t, "seuil", &load(t, "pave")?)?;
    save(
        t,
        "bord_gargouille",
        &over(&load(t, "bord_g")?, &load(t, "gargouille")?),
    )
}

/// SilOwlysGym: the trapdoor (a slab with a discreet crack, the source's fallback), the
/// pillar's top row with the owl's red eyes put out.
fn owlys_gym(g: &Path) -> Result<()> {
    let mut trappe = load(g, "dalle")?;
    const CRACK: [(u32, u32); 14] = [
        (3, 4),
        (4, 5),
        (5, 5),
        (6, 6),
        (7, 7),
        (8, 7),
        (9, 8),
        (10, 9),
        (11, 9),
        (12, 10),
        (7, 8),
        (6, 9),
        (6, 10),
        (5, 11),
    ];
    for (x, y) in CRACK {
        set(&mut trappe, x, y, [16, 16, 22]);
    }
    save(g, "trappe", &trappe)?;
    let pil = load(g, "pilier")?;
    let mut off = crop(&pil, 0, 0, pil.w, 16);
    // The saturated red eyes go dark grey.
    for p in off.px.iter_mut().filter(|p| p[3] >= 128) {
        if p[0] as u32 > p[1] as u32 + 60 && p[0] as u32 > p[2] as u32 + 60 {
            *p = [40, 42, 52, 255];
        }
    }
    save(g, "pilier_eteint", &off)
}

/// SilEvalonCitadelle: the flagstones in the wall's shadow, and the engraved slabs (sword,
/// shield, skull, rune, the golems' pressure plate), all drawn on `dalles`.
fn evalon_citadelle(c: &Path) -> Result<()> {
    let dalles = load(c, "dalles")?;
    let mut ombre = dalles.clone();
    for y in 0..8 {
        for x in 0..16 {
            let p = get(&ombre, x, y);
            // The light kept, in percent: darkest at the wall's foot.
            let k = 40 + y * 7;
            let f = |v: u8| (v as u32 * k / 100) as u8;
            set(&mut ombre, x, y, [f(p[0]), f(p[1]), f(p[2])]);
        }
    }
    save(c, "dalles_ombre", &ombre)?;
    const EPEE: [&str; 10] = [
        "....##....",
        "....##....",
        "....##....",
        "....##....",
        "....##....",
        "..######..",
        "....##....",
        "....##....",
        "...####...",
        "....##....",
    ];
    const BOUCLIER: [&str; 10] = [
        ".########.",
        ".#......#.",
        ".#..##..#.",
        ".#..##..#.",
        ".#.####.#.",
        ".#..##..#.",
        "..#.##.#..",
        "..#....#..",
        "...#..#...",
        "....##....",
    ];
    const CRANE: [&str; 10] = [
        "..######..",
        ".########.",
        "##########",
        "##oo##oo##",
        "##oo##oo##",
        "##########",
        ".####o###.",
        "..######..",
        "..#o##o#..",
        "..######..",
    ];
    const RUNE: [&str; 10] = [
        "....##....",
        "...#..#...",
        "..#....#..",
        ".#..##..#.",
        "#..#..#..#",
        "#..#..#..#",
        ".#..##..#.",
        "..#....#..",
        "...#..#...",
        "....##....",
    ];
    // (feature, glyph, the slab's fill, the glyph's colour); `o` is a hole (the skull's eyes).
    for (name, glyph, fill, col) in [
        ("dalle_epee", EPEE, [80, 96, 124], [224, 232, 240]),
        ("dalle_bouclier", BOUCLIER, [112, 96, 64], [240, 200, 72]),
        ("dalle_crane", CRANE, [104, 36, 36], [236, 228, 204]),
        ("rune", RUNE, [40, 44, 56], [96, 232, 248]),
    ] {
        let mut d = dalles.clone();
        for y in 1..15 {
            for x in 1..15 {
                let edge = x == 1 || x == 14 || y == 1 || y == 14;
                set(&mut d, x, y, if edge { [48, 48, 52] } else { fill });
            }
        }
        for (gy, row) in glyph.iter().enumerate() {
            for (gx, ch) in row.chars().enumerate() {
                let (x, y) = (3 + gx as u32, 3 + gy as u32);
                match ch {
                    '#' => set(&mut d, x, y, col),
                    'o' => set(&mut d, x, y, [24, 16, 16]),
                    _ => {}
                }
            }
        }
        save(c, name, &d)?;
    }
    // A raised steel plate with four rivets.
    let mut plate = dalles;
    for y in 2..14 {
        for x in 2..14 {
            let rgb = if x == 2 || x == 13 || y == 13 {
                [56, 60, 68]
            } else if y == 2 || x == 3 || y == 3 {
                [200, 208, 216]
            } else if (x == 5 || x == 10) && (y == 5 || y == 10) {
                [56, 60, 68]
            } else {
                [140, 148, 160]
            };
            set(&mut plate, x, y, rgb);
        }
    }
    save(c, "plaque_pression", &plate)
}

/// `top`'s opaque pixels pasted on `dst` at (`x0`, `y0`).
fn paste(dst: &mut Rgba, top: &Rgba, x0: u32, y0: u32) {
    for y in 0..top.h {
        for x in 0..top.w {
            let p = get(top, x, y);
            if p[3] >= 128 {
                set(dst, x0 + x, y0 + y, [p[0], p[1], p[2]]);
            }
        }
    }
}

/// A transparent `w x h` picture.
fn blank(w: u32, h: u32) -> Rgba {
    Rgba {
        w,
        h,
        px: vec![[0, 0, 0, 0]; (w * h) as usize],
    }
}

/// Draws `rows` at (`x0`, `y0`): each character found in `ink` is painted its colour, the
/// others are left alone.
fn glyph(img: &mut Rgba, x0: u32, y0: u32, rows: &[&str], ink: &[(u8, [u8; 3])]) {
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.bytes().enumerate() {
            if let Some((_, rgb)) = ink.iter().find(|(c, _)| *c == ch) {
                set(img, x0 + x as u32, y0 + y as u32, *rgb);
            }
        }
    }
}

/// SilRouteS06: the torii at the head of a staircase (`torii`, 4 cells wide over 2-wide stairs;
/// `torii_grand`, 6 over 4): the prompted gate over the terrace wall's top two rows (lip and
/// face) under its posts, the mossy steps between them. The Monks' shortcut's mouth
/// (`passage`): a dark arch cut in the granite, a red paper charm over it.
fn route_s06(t: &Path) -> Result<()> {
    let (wall, steps) = (load(t, "falaise")?, load(t, "marches")?);
    for (name, art) in [("torii", "torii_art"), ("torii_grand", "torii_grand_art")] {
        let gate = load(t, art)?;
        let last = gate.w - 16;
        let mut bg = tile(&steps, gate.w, gate.h);
        paste(&mut bg, &crop(&wall, 0, 0, 16, gate.h), 0, 0);
        paste(&mut bg, &crop(&wall, 16, 0, 16, gate.h), last, 0);
        save(t, name, &over(&bg, &gate))?;
    }
    let mut mouth = crop(&load(t, "roche")?, 0, 0, 16, 16);
    const ARCH: [&str; 16] = [
        "................",
        "................",
        "......oooo......",
        "....oo####oo....",
        "...o###rr###o...",
        "..o####rr####o..",
        "..o####rr####o..",
        "..o#####r####o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
        "..o##########o..",
    ];
    glyph(
        &mut mouth,
        0,
        0,
        &ARCH,
        &[
            (b'o', [88, 56, 64]),
            (b'#', [16, 10, 14]),
            (b'r', [216, 48, 40]),
        ],
    );
    save(t, "passage", &mouth)
}

/// The logic arrow pointing north, 16x16: `#` the dark outline, `y` the yellow fill.
const FLECHE_N: [&str; 16] = [
    "................",
    ".......##.......",
    "......#yy#......",
    ".....#yyyy#.....",
    "....#yyyyyy#....",
    "...#yyyyyyyy#...",
    "..#yyyyyyyyyy#..",
    "..####yyyy####..",
    ".....#yyyy#.....",
    ".....#yyyy#.....",
    ".....#yyyy#.....",
    ".....#yyyy#.....",
    ".....#yyyy#.....",
    ".....######.....",
    "................",
    "................",
];

/// SilRouteS07: the drawn features ([`route_s07_drawn`], the void first); the void's dawn-pink
/// (S7-A, by Manga-Square), violet (S7-D) and negative (by Reverse-City's gate) variants; the
/// logic arrows (yellow on the concrete deck: N, S, E, W); the reversal slab (a violet diamond
/// on the glass walkway).
fn route_s07(t: &Path) -> Result<()> {
    route_s07_drawn(t)?;
    let vide = load(t, "vide")?;
    // Each variant maps the fog's brightness onto a ramp from its night colour to its digits'.
    let (lo, hi) = (lum(get(&vide, 0, 0)), 205);
    let ramp = |dark: [u8; 3], light: [u8; 3]| {
        let mut out = vide.clone();
        for p in out.px.iter_mut() {
            let k = (lum(*p).clamp(lo, hi) - lo) * 256 / (hi - lo);
            let mix = |a: u8, b: u8| ((a as u32 * (256 - k) + b as u32 * k) / 256) as u8;
            *p = [
                mix(dark[0], light[0]),
                mix(dark[1], light[1]),
                mix(dark[2], light[2]),
                255,
            ];
        }
        out
    };
    // Dawn: a dusky rose night, pink digits. Violet: dark purple, lavender digits. Negative (by
    // Reverse-City): cyan's complement, crimson digits on near-black.
    save(t, "vide_aube", &ramp([84, 40, 76], [252, 168, 212]))?;
    save(t, "vide_violet", &ramp([36, 14, 60], [208, 152, 252]))?;
    save(t, "vide_negatif", &ramp([22, 12, 16], [236, 72, 84]))?;
    let deck = load(t, "beton")?;
    let at = |x: u32, y: u32| FLECHE_N[y as usize].as_bytes()[x as usize];
    for (name, turn) in [
        ("fleche_n", 0),
        ("fleche_s", 1),
        ("fleche_e", 2),
        ("fleche_o", 3),
    ] {
        let mut d = deck.clone();
        for y in 0..16 {
            for x in 0..16 {
                let g = match turn {
                    0 => at(x, y),
                    1 => at(x, 15 - y),
                    2 => at(y, 15 - x),
                    _ => at(y, x),
                };
                match g {
                    b'#' => set(&mut d, x, y, [40, 32, 16]),
                    b'y' => set(&mut d, x, y, [248, 208, 48]),
                    _ => {}
                }
            }
        }
        save(t, name, &d)?;
    }
    let mut slab = load(t, "verre")?;
    for y in 0..16i32 {
        for x in 0..16i32 {
            let rgb = match (2 * x - 15).abs() + (2 * y - 15).abs() {
                0..=5 => [208, 168, 248],
                6..=19 => [144, 80, 208],
                20..=23 => [48, 16, 64],
                _ => continue,
            };
            set(&mut slab, x as u32, y as u32, rgb);
        }
    }
    save(t, "retournement", &slab)
}

/// SilRouteS07's drawn features (no prompt): the glass walkway, the crystal grass, the screens,
/// the Cocket barrier, the broken pylon, the Secteur Eta gate.
fn route_s07_drawn(t: &Path) -> Result<()> {
    // The data fog: an indigo night with four streams of falling blocky 0s and 1s (the lowest
    // digit of each stream bright, the trail dimmer), a few violet specks; it wraps both ways.
    let mut vide = blank(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            set(&mut vide, x, y, [16, 12, 40]);
        }
    }
    for (x, y) in [(6, 2), (14, 29), (22, 13), (30, 21), (12, 9)] {
        set(&mut vide, x, y, [64, 40, 120]);
    }
    const ZERO: [&str; 5] = ["###", "#.#", "#.#", "#.#", "###"];
    const ONE: [&str; 5] = [".#.", "##.", ".#.", ".#.", "###"];
    for (x0, y0, digits) in [
        (1, 3, "1010"),
        (9, 18, "011"),
        (17, 8, "1101"),
        (25, 24, "100"),
    ] {
        let n = digits.len();
        for (i, d) in digits.bytes().enumerate() {
            let rgb = match n - 1 - i {
                0 => [120, 240, 248],
                1 => [40, 112, 128],
                _ => [30, 60, 88],
            };
            let g = if d == b'0' { ZERO } else { ONE };
            for (gy, row) in g.iter().enumerate() {
                for (gx, ch) in row.bytes().enumerate() {
                    if ch == b'#' {
                        let y = (y0 + 6 * i as u32 + gy as u32) % 32;
                        set(&mut vide, x0 + gx as u32, y, rgb);
                    }
                }
            }
        }
    }
    save(t, "vide", &vide)?;

    // The glass walkway: a pale cyan pane, its steel frame along the top and left edges, two
    // reflection streaks, the abyss showing through in the bottom right corner.
    let mut verre = blank(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            let rgb = if x == 0 || y == 0 {
                [112, 120, 136]
            } else if x + y >= 26 {
                [56, 64, 120]
            } else if (x + y == 9 && (3..=6).contains(&x)) || (x + y == 13 && (6..=9).contains(&x))
            {
                [240, 252, 252]
            } else if x == 1 || y == 1 {
                [200, 236, 240]
            } else {
                [160, 216, 228]
            };
            set(&mut verre, x, y, rgb);
        }
    }
    save(t, "verre", &verre)?;

    // Crystal grass: two rows of two translucent blue tufts on the dark steel.
    let mut herbe = blank(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            set(&mut herbe, x, y, [64, 72, 92]);
        }
    }
    const TUFT: [&str; 6] = [
        "...o...", "..obo..", ".obhbo.", ".obbbo.", "obbobbo", "obo.obo",
    ];
    let ink = [
        (b'o', [32, 48, 112]),
        (b'b', [144, 200, 248]),
        (b'h', [232, 248, 255]),
    ];
    for (x, y) in [(0, 1), (8, 1), (0, 9), (8, 9)] {
        glyph(&mut herbe, x, y, &TUFT, &ink);
    }
    save(t, "herbe_cristal", &herbe)?;

    // An information screen on a pole.
    let mut ecran = blank(16, 16);
    const ECRAN: [&str; 16] = [
        "................",
        ".oooooooooooooo.",
        ".occcccccccccco.",
        ".ocddddddddddco.",
        ".ocdwwwwwddddco.",
        ".ocdddddddyddco.",
        ".ocdwwwddyyydco.",
        ".ocdddddyyyyyco.",
        ".ocddddddddddco.",
        ".occcccccccccco.",
        ".oooooooooooooo.",
        "......oggo......",
        "......oggo......",
        ".....obbbbo.....",
        "....obbbbbbo....",
        "....oooooooo....",
    ];
    glyph(
        &mut ecran,
        0,
        0,
        &ECRAN,
        &[
            (b'o', [24, 24, 32]),
            (b'c', [96, 224, 232]),
            (b'd', [24, 32, 56]),
            (b'w', [232, 240, 248]),
            (b'y', [248, 208, 48]),
            (b'g', [136, 144, 160]),
            (b'b', [176, 172, 164]),
        ],
    );
    save(t, "ecran", &ecran)?;

    // Team Cocket's barrier: a striped plank on orange trestles, a red lamp on top.
    let mut barriere = blank(16, 16);
    const BARRIERE: [&str; 16] = [
        "................",
        "................",
        ".......oo.......",
        "......orro......",
        "......orro......",
        ".oooooooooooooo.",
        ".oyyykkkyyykkko.",
        ".oyykkkyyykkkyo.",
        ".oykkkyyykkkyyo.",
        ".oooooooooooooo.",
        "...ap......pa...",
        "...ap......pa...",
        "..ap........pa..",
        "..ap........pa..",
        ".ap..........pa.",
        ".oo..........oo.",
    ];
    glyph(
        &mut barriere,
        0,
        0,
        &BARRIERE,
        &[
            (b'o', [32, 24, 16]),
            (b'r', [248, 48, 48]),
            (b'y', [248, 208, 48]),
            (b'k', [32, 32, 32]),
            (b'a', [240, 128, 32]),
            (b'p', [176, 80, 24]),
        ],
    );
    save(t, "barriere", &barriere)?;

    // A broken lattice pylon, three cells tall: two rails braced by crosses, a cyan signal
    // light on top, two snapped cables, its foot bent apart.
    let mut pylone = blank(16, 48);
    let (rail, edge, brace) = ([144, 152, 168], [56, 64, 80], [112, 120, 136]);
    for y in 4..48u32 {
        let bend = y.saturating_sub(41);
        let (l, r) = (5 - bend / 2, 10 + bend / 3);
        set(&mut pylone, l - 1, y, edge);
        set(&mut pylone, l, y, rail);
        set(&mut pylone, r, y, rail);
        set(&mut pylone, r + 1, y, edge);
    }
    for s in (4..40u32).step_by(8) {
        for x in 5..=10 {
            set(&mut pylone, x, s, edge);
        }
        for i in 1..8 {
            set(&mut pylone, 6 + i / 2, s + i, brace);
            set(&mut pylone, 9 - i / 2, s + i, brace);
        }
    }
    for x in 4..=11 {
        set(&mut pylone, x, 3, edge);
    }
    for y in 0..3 {
        for x in 6..=9 {
            set(
                &mut pylone,
                x,
                y,
                if y == 0 {
                    [200, 248, 252]
                } else {
                    [96, 224, 232]
                },
            );
        }
    }
    for i in 0..13 {
        set(&mut pylone, 12 + i / 5, 8 + i, [40, 40, 48]);
        set(&mut pylone, 3 - i / 5, 14 + i, [40, 40, 48]);
    }
    for (x, y) in [(5, 38), (6, 39), (5, 40), (10, 37), (9, 38)] {
        set(&mut pylone, x, y, [24, 24, 28]);
    }
    save(t, "pylone", &pylone)?;

    // The Secteur Eta gate, in Reverse-City's negative colours: a black steel frame edged in
    // mint, a cobalt band with the crimson Cocket emblem, the violet-lit passage in the middle
    // of the bottom row.
    let mut porte = blank(48, 32);
    for y in 2..32u32 {
        for x in 1..47u32 {
            let opening = (16..=31).contains(&x) && y >= 12;
            let rgb = if x == 1 || x == 46 || y == 2 {
                [120, 232, 184]
            } else if (4..=9).contains(&y) && (3..=44).contains(&x) {
                let d = (2 * x as i32 - 47).abs() + (2 * y as i32 - 13).abs();
                if d <= 6 {
                    [200, 32, 48]
                } else if y == 4 {
                    [96, 128, 240]
                } else {
                    [48, 80, 208]
                }
            } else if (y == 11 && (15..=32).contains(&x)) || ((x == 15 || x == 32) && y >= 11) {
                [120, 232, 184]
            } else if opening {
                if y <= 13 { [72, 48, 120] } else { [36, 20, 60] }
            } else if y >= 11 && (x == 9 || x == 38) {
                [88, 176, 144]
            } else if y >= 11 {
                [52, 52, 64]
            } else {
                [24, 24, 30]
            };
            set(&mut porte, x, y, rgb);
        }
    }
    save(t, "porte_eta", &porte)
}

/// SilRoutesS2S3: the containment cable's pieces, all cut from `cable_ref` so they join: a
/// straight segment that tiles (the conduit's cross-section, repeated, with two ribs), the 16 rows
/// above the conduit's first straight row (out of the ground), the 16 rows from its last one (back
/// into the ground), the segment down the cliff, and the segment sinking into the still sea (a
/// waterline, its dark reflection, then the sea under it). The server rack on the transformer's
/// steel, a tyre rut and the wall cables (over General's gravel and walls). The depot's portals are
/// cut into the `containers` cells they stand in.
fn routes_s2s3(t: &Path) -> Result<()> {
    // The generated conduit is a third of its picture: doubled across (its middle 16 columns),
    // it is two cells wide as the source wants.
    let src = load(t, "cable_ref")?;
    let r = Rgba {
        w: 32,
        h: src.h,
        px: (0..src.h)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .map(|(x, y)| get(&src, src.w / 2 - 8 + x / 2, y))
            .collect(),
    };
    let width = |y: u32| (0..r.w).filter(|&x| get(&r, x, y)[3] >= 128).count();
    // The conduit's width: the most common row width in the middle half.
    let mut counts = [0u32; 65];
    for y in r.h / 4..r.h * 3 / 4 {
        counts[width(y).min(64)] += 1;
    }
    let w = (1..65).max_by_key(|&i| counts[i]).unwrap_or(16);
    let near = |y: u32| width(y).abs_diff(w) <= 2;
    let top = (0..r.h).find(|&y| near(y)).unwrap_or(16);
    let bot = (0..r.h).rev().find(|&y| near(y)).unwrap_or(r.h - 17);
    save(
        t,
        "cable_sortie",
        &crop(&r, 0, top.saturating_sub(15), 32, 16),
    )?;
    save(t, "cable_entree", &crop(&r, 0, bot.min(r.h - 16), 32, 16))?;
    let row = (top + bot) / 2;
    let mut seg = Rgba {
        w: 32,
        h: 16,
        px: vec![[0; 4]; 32 * 16],
    };
    let opaque = |x: u32| get(&r, x, row)[3] >= 128;
    for y in 0..16 {
        for x in (0..32).filter(|&x| opaque(x)) {
            let p = get(&r, x, row);
            // The outline columns stay; the ribs darken the body.
            let edge = x == 0 || x == 31 || !opaque(x - 1) || !opaque(x + 1);
            let k = if (y == 3 || y == 11) && !edge {
                55
            } else {
                100
            };
            let f = |v: u8| (v as u32 * k / 100) as u8;
            set(&mut seg, x, y, [f(p[0]), f(p[1]), f(p[2])]);
        }
    }
    save(t, "cable", &seg)?;
    save(t, "cable_falaise", &seg)?;
    let body: Vec<u32> = (0..32).filter(|&x| opaque(x)).collect();
    let (l, rt) = (body[0], body[body.len() - 1]);
    let mut mer = seg.clone();
    for p in mer.px[9 * 32..].iter_mut() {
        *p = [0; 4];
    }
    for x in l.saturating_sub(2)..=(rt + 2).min(31) {
        set(&mut mer, x, 9, [168, 192, 216]);
    }
    for x in (l + 2)..=rt.saturating_sub(2) {
        set(&mut mer, x, 11, [36, 52, 84]);
        set(&mut mer, x, 13, [36, 52, 84]);
    }
    save(t, "cable_mer", &mer)?;
    // The server block: a black rack (slots, green and blue lights) on the transformer's steel.
    let tr = load(t, "transfo")?;
    let mut srv = crop(&tr, tr.w / 2 - 8, tr.h / 2 - 8, 16, 16);
    for y in 0..16 {
        for x in 0..16 {
            let rgb = if x == 0 || x == 15 || y == 15 {
                [12, 12, 16]
            } else if y == 0 {
                [70, 74, 84]
            } else if y % 3 == 1 && (2..14).contains(&x) {
                match x {
                    3 => [96, 240, 128],
                    5 => [96, 176, 248],
                    _ => [58, 62, 72],
                }
            } else {
                [28, 30, 36]
            };
            set(&mut srv, x, y, rgb);
        }
    }
    save(t, "serveur", &srv)?;
    // A tyre rut down the gravel: a grey band with a ladder tread (transparent around it).
    let mut rut = Rgba {
        w: 16,
        h: 16,
        px: vec![[0; 4]; 256],
    };
    for y in 0..16 {
        for x in 3..13 {
            let tread = (5..11).contains(&x) && y % 4 < 2;
            let rgb = match (x, tread) {
                (3 | 12, _) => [168, 172, 176],
                (_, true) => [96, 98, 104],
                _ => [136, 140, 146],
            };
            set(&mut rut, x, y, rgb);
        }
    }
    save(t, "traces", &rut)?;
    // Two Cocket cables down a wall (the cable's black, a red band), a steel clamp on each.
    let mut wall = Rgba {
        w: 16,
        h: 16,
        px: vec![[0; 4]; 256],
    };
    for y in 0..16 {
        for (x, rgb) in [
            (5, [44, 46, 52]),
            (6, [20, 20, 24]),
            (10, [44, 46, 52]),
            (11, [20, 20, 24]),
        ] {
            let band = y == 12 || (y == 4 && x > 8);
            set(&mut wall, x, y, if band { [200, 52, 48] } else { rgb });
        }
    }
    for (x, y) in [(4, 8), (7, 8), (9, 1), (12, 1)] {
        set(&mut wall, x, y, [168, 168, 176]);
        set(&mut wall, x, y + 1, [72, 72, 80]);
    }
    save(t, "paroi_cables", &wall)?;
    save(t, "paroi_cables_d", &wall)?;
    // The steel wire fence: the security cables with their yellow (and the tag's red) gone grey.
    let mut fence = load(t, "cable_secu")?;
    for p in fence.px.iter_mut().filter(|p| p[3] >= 128) {
        let (hi, lo) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
        if hi - lo > 60 {
            let v = (lum(*p) * 3 / 4 + 40).min(255) as u8;
            *p = [v, v, v.saturating_add(8), 255];
        }
    }
    save(t, "grillage", &fence)?;
    // The wall's containers edge to edge: each column's transparent pixels take the nearest
    // opaque pixel's colour below (above for the bottom rows).
    for (from, to) in [
        ("containers", "containers_mur"),
        ("containers_b", "containers_mur_b"),
    ] {
        let mut m = load(t, from)?;
        let src = m.clone();
        for x in 0..m.w {
            for y in 0..m.h {
                if get(&src, x, y)[3] >= 128 {
                    continue;
                }
                let below = (y..m.h).find(|&j| get(&src, x, j)[3] >= 128);
                let above = (0..y).rev().find(|&j| get(&src, x, j)[3] >= 128);
                if let Some(j) = below.or(above) {
                    let p = get(&src, x, j);
                    set(&mut m, x, y, [p[0], p[1], p[2]]);
                }
            }
        }
        // Columns with no opaque pixel take their row's nearest one.
        let cols = m.clone();
        for y in 0..m.h {
            for x in 0..m.w {
                if get(&cols, x, y)[3] >= 128 {
                    continue;
                }
                let near = (0..m.w)
                    .filter(|&i| get(&cols, i, y)[3] >= 128)
                    .min_by_key(|&i| i.abs_diff(x));
                if let Some(i) = near {
                    let p = get(&cols, i, y);
                    set(&mut m, x, y, [p[0], p[1], p[2]]);
                }
            }
        }
        save(t, to, &m)?;
    }
    let c = load(t, "containers_mur")?;
    // The south portal: an open doorway in the container's side.
    let mut sud = crop(&c, 16, 16, 16, 16);
    for y in 4..16 {
        for x in 3..13 {
            let frame = x == 3 || x == 12 || y == 4;
            set(
                &mut sud,
                x,
                y,
                if frame { [40, 40, 48] } else { [14, 14, 22] },
            );
        }
    }
    save(t, "portail_sud", &sud)?;
    // The north portal: a hatch striped yellow and black in the roof, a chevron pointing down.
    let mut nord = crop(&c, 16, 0, 16, 16);
    for y in 3..13 {
        for x in 3..13 {
            let frame = x == 3 || x == 12 || y == 3 || y == 12;
            let rgb = if !frame {
                [20, 20, 28]
            } else if (x + y) / 2 % 2 == 0 {
                [240, 200, 40]
            } else {
                [24, 24, 24]
            };
            set(&mut nord, x, y, rgb);
        }
    }
    for i in 0..3 {
        set(&mut nord, 5 + i, 6 + i, [240, 200, 40]);
        set(&mut nord, 10 - i, 6 + i, [240, 200, 40]);
    }
    save(t, "portail_nord", &nord)
}
