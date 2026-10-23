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
            let c = if x < 2 || x > 13 {
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
