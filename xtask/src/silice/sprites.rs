//! `cargo xtask silice gfx sprites`: original overworld sprites (objects) from generated art.
//!
//! ```text
//! cargo xtask silice gfx sprites [<NAME>...] [--model codex|ark|nano]
//!                                 [--ref IMG]... [--prompt TEXT] [--prep-only] [--jobs N] [--games DIR]
//! ```
//!
//! `gfx/sprites.ron` ([`Spec`]) lists the sprites. Each is one generated picture of the character
//! seen from the front, the back and the left side (three figures in a row on magenta), kept
//! as `target/silice-gfx/raw/sprites/<NAME>.png`. The three figures are found by their
//! columns, scaled together (the same scale for the three) into 16x32 cells, bottom aligned,
//! and quantized to one 15-colour palette. Gen3's 9 frames (south, north, west standing, then
//! two walking frames each) are the standing poses plus synthesized steps: the body bobs one
//! pixel and one foot stays down, the other foot in the next frame.
//!
//! Output: `sprites/<NAME>.{ron,png}` in the extract's format, which the Silice content build
//! lays over the extract's (`content/build/silice/gfx.rs`): a name of Emerald's
//! (`MAGMA_MEMBER_M`) replaces that sprite and keeps its id; a new name (`SIL_ADRIEN`) is a
//! new graphics id (`gfx_id`, 0x100 and up) that maps name as `OBJ_EVENT_GFX_SIL_ADRIEN`.
//! A 4x review sheet goes to `target/silice-gfx/sprites/<NAME>.png`.

use super::gfx::{
    Align, Rgba, from555, kmeans_colours, nearest, opaque_mask, prep_box, read_rgba, rgb555,
    write_rgba,
};
use anyhow::{Context, Result, bail, ensure};
use extract::export::{read_ron, to_ron};
use extract::gfx::{IndexedImage, Palette};
use extract::schema::SpriteFile;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

const USAGE: &str = "usage: cargo xtask silice gfx sprites [<NAME>...] [--model codex|ark|nano] \
     [--ref IMG]... [--prompt TEXT] [--prep-only] [--jobs N] [--games DIR]";
const W: u32 = 16;
const H: u32 = 32;

/// `gfx/sprites.ron`.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The art direction every prompt starts with.
    #[serde(default)]
    pub style: String,
    pub sprites: Vec<Sprite>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    /// An Emerald graphics name without `OBJ_EVENT_GFX_` (replaced), or a new `SIL_` one.
    pub name: String,
    /// A new sprite's id (0x100 and up); Emerald's own for a replaced one (may be left out).
    #[serde(default)]
    pub gfx_id: Option<u16>,
    /// The character.
    pub prompt: String,
}

pub fn main(args: &[String]) -> Result<()> {
    let flags = super::image::parse_flags(args, USAGE, crate::root().join("games/silice"))?;
    let spec: Spec = read_ron(&flags.games.join("gfx/sprites.ron"))?;
    for n in &flags.rest {
        ensure!(spec.sprites.iter().any(|s| s.name == *n), "no sprite {n}");
    }
    let todo: Vec<&Sprite> = (spec.sprites.iter())
        .filter(|s| flags.rest.is_empty() || flags.rest.contains(&s.name))
        .collect();
    if flags.prompt.is_some() {
        ensure!(todo.len() == 1, "--prompt applies to one sprite; name it");
    }
    let raw_dir = crate::root().join("target/silice-gfx/raw/sprites");
    std::fs::create_dir_all(&raw_dir)?;
    if !flags.prep_only {
        let queue = std::sync::Mutex::new(todo.iter());
        let model = flags.model;
        let refs = &flags.refs;
        let subject = flags.prompt.as_deref();
        std::thread::scope(|sc| {
            for _ in 0..flags.jobs.max(1) {
                sc.spawn(|| {
                    loop {
                        let Some(s) = queue.lock().unwrap().next().copied() else {
                            break;
                        };
                        println!("sprite {}: asking {}", s.name, model.detail());
                        let raw = raw_dir.join(format!("{}.png", s.name));
                        let text = prompt(&spec, subject.unwrap_or(&s.prompt));
                        match super::image::generate(model, &text, refs) {
                            Ok(img) => {
                                if raw.exists() {
                                    let _ = std::fs::rename(
                                        &raw,
                                        raw_dir.join(format!("{}.prev.png", s.name)),
                                    );
                                }
                                if let Err(e) = std::fs::copy(&img, &raw) {
                                    println!("sprite {}: {e}", s.name);
                                }
                            }
                            Err(e) => println!("sprite {}: gen failed: {e:#}", s.name),
                        }
                    }
                });
            }
        });
    }
    for s in &todo {
        let raw = raw_dir.join(format!("{}.png", s.name));
        if !raw.exists() {
            println!("sprite {}: no raw picture", s.name);
            continue;
        }
        build(&flags.games, s, &raw).with_context(|| format!("sprite {}", s.name))?;
    }
    Ok(())
}

fn prompt(spec: &Spec, subject: &str) -> String {
    format!(
        "{} Character: {subject} Draw exactly three full-body figures of this same character side \
         by side in one row, evenly spaced with wide gaps, all the same size, standing still: \
         facing the viewer, facing away (seen from the back), facing left (side profile). Each \
         figure is a tiny Game Boy Advance overworld sprite of 16x32 pixels: big head (about a \
         third of the height), chibi proportions, each pixel visible as a large square. \
         Background: solid flat magenta #FF00FF, no shadow, no ground, no text.",
        spec.style
    )
}

/// Column groups of the opaque pixels: `(x0, x1)` runs, merged to `n`.
fn figures(mask: &[bool], w: u32, h: u32, n: usize) -> Result<Vec<(u32, u32)>> {
    let col: Vec<bool> = (0..w)
        .map(|x| (0..h).filter(|&y| mask[(y * w + x) as usize]).count() > 2)
        .collect();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut x = 0;
    while x < w {
        if col[x as usize] {
            let s = x;
            while x < w && col[x as usize] {
                x += 1;
            }
            runs.push((s, x));
        } else {
            x += 1;
        }
    }
    // Drop specks, then merge the runs closest together until `n` remain.
    runs.retain(|r| r.1 - r.0 > w / 100);
    while runs.len() > n {
        let i = (0..runs.len() - 1)
            .min_by_key(|&i| runs[i + 1].0 - runs[i].1)
            .unwrap();
        runs[i].1 = runs[i + 1].1;
        runs.remove(i + 1);
    }
    ensure!(
        runs.len() == n,
        "{} figures found, {n} wanted (regenerate)",
        runs.len()
    );
    Ok(runs)
}

fn build(games: &Path, s: &Sprite, raw_path: &Path) -> Result<()> {
    let key = Some([0xFF, 0x00, 0xFF]);
    let raw = read_rgba(raw_path)?;
    let mask = opaque_mask(&raw, key, 110);
    let groups = figures(&mask, raw.w, raw.h, 3)?;
    // A common box size: the tallest figure's rows, the widest figure's width at 1:2.
    let rows = |x0: u32, x1: u32| {
        let ys: Vec<u32> = (0..raw.h)
            .filter(|&y| (x0..x1).any(|x| mask[(y * raw.w + x) as usize]))
            .collect();
        (ys[0], ys[ys.len() - 1] + 1)
    };
    let extents: Vec<(u32, u32)> = groups.iter().map(|&(a, b)| rows(a, b)).collect();
    let bh = extents.iter().map(|e| e.1 - e.0).max().unwrap();
    let bw = groups.iter().map(|g| g.1 - g.0).max().unwrap().max(bh / 2);
    let mut poses = Vec::new();
    for (&(x0, x1), &(_, y1)) in groups.iter().zip(&extents) {
        let cx = (x0 + x1) / 2;
        let bx0 = cx.saturating_sub(bw / 2);
        let bx1 = (bx0 + bw).min(raw.w);
        let by0 = y1.saturating_sub(bh);
        let pose = prep_box(
            &raw,
            &mask,
            (bx0, by0, bx1, y1),
            W,
            H,
            key,
            false,
            Align::Bottom,
        )?;
        poses.push(pose);
    }
    // Front, back, left: the raw order is the prompt's.
    let (south, north, west) = (&poses[0], &poses[1], &poses[2]);
    let frames = [
        south.clone(),
        north.clone(),
        west.clone(),
        step(south, false),
        step(south, true),
        step(north, false),
        step(north, true),
        step_side(west, false),
        step_side(west, true),
    ];

    // One palette for every frame.
    let mut hist: HashMap<u16, u32> = HashMap::new();
    for f in &frames {
        for p in f.px.iter().filter(|p| p[3] != 0) {
            *hist.entry(rgb555(*p)).or_default() += 1;
        }
    }
    let pal = kmeans_colours(&hist, 15);
    let mut palette = [0u16; 16];
    for (i, &c) in pal.iter().enumerate() {
        palette[i + 1] = c;
    }
    let sheet_w = W * frames.len() as u32;
    let mut img = IndexedImage {
        width: sheet_w,
        height: H,
        pixels: vec![0; (sheet_w * H) as usize],
    };
    for (k, f) in frames.iter().enumerate() {
        for y in 0..H {
            for x in 0..W {
                let p = f.at(x, y);
                if p[3] != 0 {
                    img.pixels[(y * sheet_w + k as u32 * W + x) as usize] =
                        nearest(&pal, rgb555(p)) as u8 + 1;
                }
            }
        }
    }
    let emerald = extract::names::OBJ_EVENT_GFX.value(&format!("OBJ_EVENT_GFX_{}", s.name));
    let gfx_id = match (emerald, s.gfx_id) {
        (Some(e), None) => e as u16,
        (Some(e), Some(g)) => {
            ensure!(u32::from(g) == e, "Emerald's id is {e:#x}");
            g
        }
        (None, Some(g)) => {
            ensure!(g >= 0x100, "a new sprite's gfx_id is 0x100 or more");
            g
        }
        (None, None) => bail!("a new sprite needs a gfx_id (0x100 and up)"),
    };
    let file = SpriteFile {
        name: s.name.clone(),
        gfx_id,
        width: W as u16,
        height: H as u16,
        cell_width: W as u16,
        cell_height: H as u16,
        frame_sizes: vec![(W as u16, H as u16); frames.len()],
        palette_tag: 0x1100 + (gfx_id & 0xFF),
        palette,
    };
    let dir = games.join("sprites");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{}.ron", s.name)), to_ron(&file)?)?;
    extract::gfx::write_png_indexed(
        &dir.join(format!("{}.png", s.name)),
        &img,
        &Palette(palette),
    )?;

    // Review sheet, 4x, on a grey background.
    let z = 4;
    let mut big = Rgba {
        w: sheet_w * z,
        h: H * z,
        px: vec![[96, 104, 96, 255]; (sheet_w * H * z * z) as usize],
    };
    for y in 0..H * z {
        for x in 0..sheet_w * z {
            let v = img.pixels[((y / z) * sheet_w + x / z) as usize];
            if v != 0 {
                big.px[(y * sheet_w * z + x) as usize] = from555(palette[v as usize]);
            }
        }
    }
    let out = crate::root().join(format!("target/silice-gfx/sprites/{}.png", s.name));
    write_rgba(&out, &big)?;
    println!(
        "sprite {}: gfx {gfx_id:#x}, {} colours; review {}",
        s.name,
        pal.len(),
        out.display()
    );
    Ok(())
}

/// The opaque rows of `f`.
fn extent(f: &Rgba) -> (u32, u32) {
    let rows: Vec<u32> = (0..f.h)
        .filter(|&y| (0..f.w).any(|x| f.at(x, y)[3] != 0))
        .collect();
    (rows[0], rows[rows.len() - 1] + 1)
}

/// A south or north step: the body one pixel up, the planted foot (left half, or right half
/// when `right`) down as standing.
fn step(f: &Rgba, right: bool) -> Rgba {
    let (_, y1) = extent(f);
    let legs = y1.saturating_sub(4);
    let mut out = Rgba {
        w: f.w,
        h: f.h,
        px: vec![[0; 4]; f.px.len()],
    };
    for y in 0..f.h {
        for x in 0..f.w {
            let planted = y >= legs && ((x < f.w / 2) != right);
            let src = if planted || y + 1 >= f.h {
                f.at(x, y)
            } else {
                f.at(x, y + 1)
            };
            // The lifted foot's last row is left empty.
            if !planted && y + 1 == y1 && y >= legs {
                continue;
            }
            out.px[(y * f.w + x) as usize] = src;
        }
    }
    out
}

/// A west step: the body one pixel up, the feet apart (the front foot one pixel forward,
/// the back one back; swapped when `back`).
fn step_side(f: &Rgba, back: bool) -> Rgba {
    let (_, y1) = extent(f);
    let legs = y1.saturating_sub(4);
    let mut out = Rgba {
        w: f.w,
        h: f.h,
        px: vec![[0; 4]; f.px.len()],
    };
    for y in 0..f.h {
        for x in 0..f.w {
            let src = if y >= legs {
                let shift: i32 = if (x < f.w / 2) != back { 1 } else { -1 };
                let sx = x as i32 + shift;
                if sx < 0 || sx >= f.w as i32 {
                    continue;
                }
                f.at(sx as u32, y)
            } else if y + 1 < f.h {
                f.at(x, y + 1)
            } else {
                continue;
            };
            out.px[(y * f.w + x) as usize] = src;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_figures_are_found_by_their_columns() {
        let (w, h) = (90u32, 10u32);
        let mut mask = vec![false; (w * h) as usize];
        for &(a, b) in &[(5u32, 20u32), (22, 25), (40, 55), (70, 85)] {
            for y in 2..8 {
                for x in a..b {
                    mask[(y * w + x) as usize] = true;
                }
            }
        }
        // The two close runs (a figure and its detached hand) merge.
        assert_eq!(
            figures(&mask, w, h, 3).unwrap(),
            vec![(5, 25), (40, 55), (70, 85)]
        );
        assert!(figures(&mask, w, h, 5).is_err());
    }

    #[test]
    fn a_step_keeps_one_foot_down() {
        let mut f = Rgba {
            w: W,
            h: H,
            px: vec![[0; 4]; (W * H) as usize],
        };
        for y in 8..32 {
            for x in 4..12 {
                f.px[(y * W + x) as usize] = [200, 0, 0, 255];
            }
        }
        let s = step(&f, false);
        assert_ne!(s.at(5, 31)[3], 0, "the left foot stays down");
        assert_eq!(s.at(10, 31)[3], 0, "the right foot is lifted");
        assert_ne!(s.at(5, 7)[3], 0, "the body bobs up a pixel");
    }
}
