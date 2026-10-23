//! `cargo xtask silice gfx`: original tilesets from generated art (Q3: no ROM graphics in the
//! games dir).
//!
//! ```text
//! cargo xtask silice gfx prep <raw.png> <out.png> --size WxH [--key RRGGBB|none] [--tol N]
//!                             [--stretch] [--align bottom|center|top]
//! cargo xtask silice gfx gen <Tileset> [<feature>...] [--prep-only] [--jobs N] [--games DIR]
//! cargo xtask silice gfx build [<Tileset>...] [--games DIR] [--extracted DIR]
//! ```
//!
//! **gen** asks Codex's image tool (`codex exec`, `image_gen`) for each named feature's
//! picture (all features with a `prompt` when none is named; `--jobs` at a time, default 4),
//! keeps it as `target/silice-gfx/raw/<Tileset>/<feature>.png` (the previous one as
//! `<feature>.prev.png`; raw pictures are never committed), then preps it into the feature's art
//! `gfx/<Tileset>/<feature>.png` at its cells' size, as `prep` does with the feature's `key`
//! and `align`. `mirror` features are their source's art flipped. `--prep-only` re-preps the
//! kept raw pictures without calling Codex. Review the art, re-`gen` what is wrong (tune the
//! prompt), then `build`.
//!
//! **prep** turns a generated picture (an image model's 1024-ish px "pixel art" of one object
//! on a flat chroma-key background) into art at its real pixel size: the key colour (default
//! magenta `FF00FF`) becomes transparent, the opaque part is cropped, then scaled into `WxH`
//! (aspect kept, placed by `--align`, default bottom; `--stretch` fills `WxH` exactly, for
//! ground textures with `--key none`). Each output pixel is the most frequent RGB555 colour of
//! its source block, so the art stays crisp; a block less than half opaque is transparent.
//!
//! **build** reads `gfx/<Tileset>.ron` ([`Spec`]) and its art `gfx/<Tileset>/<feature>.png`
//! (RGBA, `16 * w` x `16 * h` px for a feature of `w x h` metatiles) and writes:
//!
//! - `tilesets/<Tileset>.{ron,png}`: a secondary tileset in the extract's format (the content
//!   build lays it over the extract's, `content/build/silice/gfx.rs`). Its 7 palettes (BG
//!   palettes 6..13) come from clustering the art's 8x8 tiles by colour, then quantizing each
//!   cluster to 15 colours (index 0 is transparent). Tiles are deduplicated, flips included.
//! - Each art cell is one metatile, in spec order (feature by feature, row by row), so a
//!   feature's ids stay put while the features before it keep their sizes: scripts name them
//!   (`set_metatile`), and the stamps list them. The feature's pixels are on the top layer, over
//!   the bottom layer of the primary tileset's `ground` metatile (transparent pixels show it;
//!   no `ground`: the art is the bottom layer). Cells are `Covered` (both layers under
//!   the objects), but the rows listed in `over` are `Normal`: their art is drawn above the
//!   objects (the player walks behind a roof edge, under an arch).
//! - `stamps/<Tileset>_<feature>.ron`: the feature's blocks with the collision of its `cells`
//!   (the `Stamp` op of `studio map apply` paints it).
//! - `target/silice-gfx/<Tileset>.png`: every feature as the game draws it (3x), for review.
//!
//! The primary's tile entries are referenced by index (never copied into the games dir).

use anyhow::{Context, Result, bail, ensure};
use extract::export::{read_ron, to_ron};
use extract::gfx::{IndexedImage, Palette};
use extract::schema::{Row, TilesetFile, to_rows};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: cargo xtask silice gfx prep <raw.png> <out.png> --size WxH \
     [--key RRGGBB|none] [--tol N] [--stretch] [--align bottom|center|top]\n       \
     cargo xtask silice gfx gen <Tileset> [<feature>...] [--prep-only] [--jobs N] [--games DIR]\n       \
     cargo xtask silice gfx build [<Tileset>...] [--games DIR] [--extracted DIR]";

/// First BG palette of a secondary tileset, and how many it has.
const PAL_FIRST: usize = 6;
const PAL_COUNT: usize = 7;
const MAX_TILES: usize = 512;
const MAX_METATILES: usize = 512;
const LAYER_NORMAL: u16 = 0;
const LAYER_COVERED: u16 = 1;

/// `gfx/<Tileset>.ron`.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The tileset's name (`Sil…`), as layouts name it.
    pub name: String,
    /// The primary tileset it pairs with (`General` outdoors, `Building` indoors).
    pub primary: String,
    /// The primary metatile under transparent pixels, unless a feature says otherwise.
    #[serde(default)]
    pub ground: Option<u16>,
    /// The art direction every `gen` prompt starts with.
    #[serde(default)]
    pub style: String,
    pub features: Vec<Feature>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    /// `gfx/<Tileset>/<name>.png`; the stamp is `<Tileset>_<name>`.
    pub name: String,
    /// One string per metatile row, one char per cell: `.` walkable, `#` solid, `D` door
    /// (`MB_ANIMATED_DOOR`, solid as Emerald's), or a `legend` key.
    pub cells: Vec<String>,
    /// Rows drawn above the objects.
    #[serde(default)]
    pub over: Vec<usize>,
    /// The primary metatile under this feature (`None`: the spec's).
    #[serde(default)]
    pub ground: Option<u16>,
    /// Extra cell kinds.
    #[serde(default)]
    pub legend: BTreeMap<char, Cell>,
    /// What `gen` asks the image model for (after the spec's `style`).
    #[serde(default)]
    pub prompt: Option<String>,
    /// `gen`'s prep: the chroma key (`RRGGBB`, default magenta; `none`: an opaque texture,
    /// stretched to the cells).
    #[serde(default)]
    pub key: Option<String>,
    /// `gen`'s prep: `bottom` (default), `center` or `top`.
    #[serde(default)]
    pub align: Option<String>,
    /// The art of another feature (same size), mirrored left to right, instead of a prompt.
    #[serde(default)]
    pub mirror: Option<String>,
}

impl Feature {
    fn size(&self) -> (u32, u32) {
        (
            self.cells[0].chars().count() as u32,
            self.cells.len() as u32,
        )
    }

    fn key(&self) -> Result<Option<[u8; 3]>> {
        Ok(match self.key.as_deref() {
            None => Some([0xFF, 0x00, 0xFF]),
            Some("none") => None,
            Some(s) => {
                let v = u32::from_str_radix(s.trim_start_matches('#'), 16)?;
                Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
            }
        })
    }

    fn align(&self) -> Result<Align> {
        Ok(match self.align.as_deref() {
            None | Some("bottom") => Align::Bottom,
            Some("center") => Align::Center,
            Some("top") => Align::Top,
            Some(a) => bail!("feature {}: align {a:?}", self.name),
        })
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// An `MB_*` name.
    pub behavior: String,
    pub collision: u8,
}

/// `stamps/<name>.ron` (the studio's `map_edit::stamp::Stamp`).
#[derive(Serialize)]
struct StampFile {
    primary: String,
    secondary: String,
    width: u16,
    height: u16,
    elevation: bool,
    blocks: Vec<Row>,
}

pub fn main(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("prep") => prep_cmd(&args[1..]),
        Some("gen") => gen_cmd(&args[1..]),
        Some("build") => {
            let mut games = crate::root().join("games/silice");
            let mut extracted = crate::root().join("content/extracted");
            let mut names = Vec::new();
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--games" => games = PathBuf::from(it.next().context(USAGE)?),
                    "--extracted" => extracted = PathBuf::from(it.next().context(USAGE)?),
                    s if !s.starts_with('-') => names.push(s.to_string()),
                    _ => bail!(USAGE),
                }
            }
            if names.is_empty() {
                names = specs(&games)?;
            }
            for n in &names {
                build(&games, &extracted, n).with_context(|| format!("gfx/{n}.ron"))?;
            }
            Ok(())
        }
        _ => bail!(USAGE),
    }
}

/// The `gfx/*.ron` stems.
fn specs(games: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let d = games.join("gfx");
    if d.is_dir() {
        for e in std::fs::read_dir(&d)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "ron") {
                out.push(p.file_stem().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    Ok(out)
}

// ---- RGBA images ----

#[derive(Clone)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[u8; 4]>,
}

impl Rgba {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        self.px[(y * self.w + x) as usize]
    }
}

pub fn read_rgba(path: &Path) -> Result<Rgba> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut dec = png::Decoder::new(file);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let px = match info.color_type {
        png::ColorType::Rgba => buf
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect(),
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().map(|&g| [g, g, g, 255]).collect(),
        t => bail!("{}: unsupported colour type {t:?}", path.display()),
    };
    let mut px: Vec<[u8; 4]> = px;
    px.truncate((w * h) as usize);
    Ok(Rgba { w, h, px })
}

pub fn write_rgba(path: &Path, img: &Rgba) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let bytes: Vec<u8> = img.px.iter().flatten().copied().collect();
    extract::gfx::write_png_rgba(path, img.w, img.h, &bytes)
}

// ---- gen ----

fn gen_cmd(args: &[String]) -> Result<()> {
    let mut games = crate::root().join("games/silice");
    let mut prep_only = false;
    let mut jobs = 4usize;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--games" => games = PathBuf::from(it.next().context(USAGE)?),
            "--prep-only" => prep_only = true,
            "--jobs" => jobs = it.next().context(USAGE)?.parse()?,
            s if !s.starts_with('-') => pos.push(s.to_string()),
            _ => bail!(USAGE),
        }
    }
    let name = pos.first().context(USAGE)?.clone();
    let spec: Spec = read_ron(&games.join(format!("gfx/{name}.ron")))?;
    let picked: Vec<&Feature> = if pos.len() > 1 {
        pos[1..]
            .iter()
            .map(|n| {
                spec.features
                    .iter()
                    .find(|f| f.name == *n)
                    .with_context(|| format!("no feature {n}"))
            })
            .collect::<Result<_>>()?
    } else {
        spec.features.iter().collect()
    };
    let raw_dir = crate::root().join(format!("target/silice-gfx/raw/{name}"));
    std::fs::create_dir_all(&raw_dir)?;
    if !prep_only {
        let todo: Vec<(&Feature, String)> = picked
            .iter()
            .filter(|f| f.mirror.is_none())
            .filter_map(|f| f.prompt.as_ref().map(|p| (*f, full_prompt(&spec, f, p))))
            .collect();
        let queue = std::sync::Mutex::new(todo.into_iter());
        let failed = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..jobs.max(1) {
                s.spawn(|| {
                    loop {
                        let Some((f, prompt)) = queue.lock().unwrap().next() else {
                            break;
                        };
                        let raw = raw_dir.join(format!("{}.png", f.name));
                        println!("gen {name}/{}: asking codex", f.name);
                        match codex_image(&prompt) {
                            Ok(img) => {
                                if raw.exists() {
                                    let _ = std::fs::rename(
                                        &raw,
                                        raw_dir.join(format!("{}.prev.png", f.name)),
                                    );
                                }
                                match std::fs::copy(&img, &raw) {
                                    Ok(_) => println!("gen {name}/{}: {}", f.name, raw.display()),
                                    Err(e) => {
                                        failed.lock().unwrap().push(format!("{}: {e}", f.name))
                                    }
                                }
                            }
                            Err(e) => failed.lock().unwrap().push(format!("{}: {e:#}", f.name)),
                        }
                    }
                });
            }
        });
        let failed = failed.into_inner().unwrap();
        if !failed.is_empty() {
            println!("gen failures:\n  {}", failed.join("\n  "));
        }
    }
    // Prep (sources first, then mirrors).
    let art_dir = games.join(format!("gfx/{name}"));
    std::fs::create_dir_all(&art_dir)?;
    let mut missing = Vec::new();
    for f in picked.iter().filter(|f| f.mirror.is_none()) {
        let raw = raw_dir.join(format!("{}.png", f.name));
        if !raw.exists() {
            missing.push(f.name.clone());
            continue;
        }
        let (w, h) = f.size();
        let key = f.key()?;
        let art = prep(
            &read_rgba(&raw)?,
            16 * w,
            16 * h,
            key,
            110,
            key.is_none(),
            f.align()?,
        )
        .with_context(|| format!("prep {}", f.name))?;
        write_rgba(&art_dir.join(format!("{}.png", f.name)), &art)?;
    }
    for f in spec.features.iter().filter(|f| f.mirror.is_some()) {
        let src = f.mirror.as_ref().unwrap();
        if !(picked.iter().any(|p| p.name == f.name || p.name == *src)) {
            continue;
        }
        let a = read_rgba(&art_dir.join(format!("{src}.png")))?;
        let mut m = a.clone();
        for y in 0..a.h {
            for x in 0..a.w {
                m.px[(y * a.w + x) as usize] = a.at(a.w - 1 - x, y);
            }
        }
        write_rgba(&art_dir.join(format!("{}.png", f.name)), &m)?;
    }
    if !missing.is_empty() {
        println!(
            "no raw picture (no prompt, or gen failed): {}",
            missing.join(", ")
        );
    }
    println!("art in {}", art_dir.display());
    Ok(())
}

/// The full prompt of `f`: the style, the subject, the size and background rules.
fn full_prompt(spec: &Spec, f: &Feature, subject: &str) -> String {
    let (w, h) = f.size();
    let bg = match f.key.as_deref() {
        Some("none") => format!(
            "A seamless tileable texture filling the whole square canvas edge to edge, seen from \
             straight above, no border, no single object, no vignette. The whole texture is only \
             {}x{} pixels (it repeats on that grid in game); each pixel must be visible as a large \
             square, so it has very few, chunky details.",
            16 * w,
            16 * h
        ),
        k => {
            let key = match k {
                None | Some("FF00FF") => "magenta #FF00FF".to_string(),
                Some("00FF00") => "pure green #00FF00".to_string(),
                Some(k) => format!("#{k}"),
            };
            format!(
                "One single object, centered, in the game's 3/4 top-down view. The whole sprite is \
                 exactly {}x{} pixels ({w}x{h} tiles of 16x16) and its footprint fills that {w}:{h} \
                 (width:height) canvas; each pixel must be visible as a large square. Background \
                 outside the object: solid flat {key}, a colour the object never uses; no shadow, no \
                 ground, no border, no text.",
                16 * w,
                16 * h
            )
        }
    };
    format!("{} Subject: {subject} {bg}", spec.style)
}

/// One `codex exec` image generation; the picture's path under `~/.codex/generated_images`.
fn codex_image(prompt: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "pkr-gfx-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir)?;
    let out = std::process::Command::new("codex")
        .args(["exec", "--skip-git-repo-check", "-s", "read-only"])
        .arg(format!(
            "Use your built-in image_gen tool exactly once to generate the image described below, \
             then stop. Do not post-process, resize or save any file. Image: {prompt}"
        ))
        .current_dir(&dir)
        .output()
        .context("running codex")?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let session = log
        .lines()
        .find_map(|l| l.strip_prefix("session id: "))
        .map(str::trim)
        .context("no session id in codex's output")?;
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex")
        });
    let gen_dir = home.join("generated_images").join(session);
    let mut imgs: Vec<PathBuf> = std::fs::read_dir(&gen_dir)
        .with_context(|| format!("codex made no image ({})", gen_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    imgs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    imgs.pop()
        .with_context(|| format!("codex made no image ({})", gen_dir.display()))
}

// ---- prep ----

fn prep_cmd(args: &[String]) -> Result<()> {
    let mut pos = Vec::new();
    let mut size = None;
    let mut key = Some([0xFF, 0x00, 0xFF]);
    let mut tol = 110u32;
    let mut stretch = false;
    let mut align = Align::Bottom;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => {
                let s = it.next().context(USAGE)?;
                let (w, h) = s.split_once('x').context("--size WxH")?;
                size = Some((w.parse::<u32>()?, h.parse::<u32>()?));
            }
            "--key" => {
                let s = it.next().context(USAGE)?;
                key = match s.as_str() {
                    "none" => None,
                    s => {
                        let v = u32::from_str_radix(s.trim_start_matches('#'), 16)?;
                        Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
                    }
                }
            }
            "--tol" => tol = it.next().context(USAGE)?.parse()?,
            "--stretch" => stretch = true,
            "--align" => {
                align = match it.next().context(USAGE)?.as_str() {
                    "bottom" => Align::Bottom,
                    "center" => Align::Center,
                    "top" => Align::Top,
                    _ => bail!(USAGE),
                }
            }
            s if !s.starts_with('-') => pos.push(PathBuf::from(s)),
            _ => bail!(USAGE),
        }
    }
    ensure!(pos.len() == 2, USAGE);
    let (w, h) = size.context("--size WxH is required")?;
    let raw = read_rgba(&pos[0])?;
    let out = prep(&raw, w, h, key, tol, stretch, align)?;
    write_rgba(&pos[1], &out)?;
    let opaque = out.px.iter().filter(|p| p[3] != 0).count();
    println!("{}: {w}x{h}, {opaque} opaque px", pos[1].display());
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Align {
    Bottom,
    Center,
    Top,
}

/// Whether `c` is the key colour (or one of the key-tinted fringe colours image models leave
/// around an object).
fn keyed(c: [u8; 4], key: Option<[u8; 3]>, tol: u32) -> bool {
    if c[3] < 128 {
        return true;
    }
    let Some(k) = key else { return false };
    let d: u32 = (0..3)
        .map(|i| (c[i] as i32 - k[i] as i32).unsigned_abs().pow(2))
        .sum();
    if d <= tol * tol {
        return true;
    }
    // Magenta fringes: red and blue well above green.
    k == [0xFF, 0x00, 0xFF] && magenta(c)
}

fn magenta(c: [u8; 4]) -> bool {
    let (r, g, b) = (c[0] as u32, c[1] as u32, c[2] as u32);
    r > 110 && b > 110 && g * 10 < r.min(b) * 7
}

fn luma(c: u16) -> u32 {
    let (r, g, b) = ((c & 31) as u32, (c >> 5 & 31) as u32, (c >> 10 & 31) as u32);
    r * 3 + g * 6 + b
}

fn rgb555(c: [u8; 4]) -> u16 {
    (c[0] as u16 >> 3) | (c[1] as u16 >> 3) << 5 | (c[2] as u16 >> 3) << 10
}

fn from555(c: u16) -> [u8; 4] {
    let e = |v: u16| ((v & 31) << 3 | (v & 31) >> 2) as u8;
    [e(c), e(c >> 5), e(c >> 10), 255]
}

pub fn prep(
    raw: &Rgba,
    w: u32,
    h: u32,
    key: Option<[u8; 3]>,
    tol: u32,
    stretch: bool,
    align: Align,
) -> Result<Rgba> {
    let mask: Vec<bool> = raw.px.iter().map(|&c| !keyed(c, key, tol)).collect();
    // The opaque bounding box.
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..raw.h {
        for x in 0..raw.w {
            if mask[(y * raw.w + x) as usize] {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    ensure!(x0 < x1, "no opaque pixel (wrong --key?)");
    if key.is_none() {
        (x0, y0, x1, y1) = (0, 0, raw.w, raw.h);
    }
    let (bw, bh) = ((x1 - x0) as f64, (y1 - y0) as f64);
    let (tw, th) = if stretch {
        (w, h)
    } else {
        let s = (w as f64 / bw).min(h as f64 / bh);
        (
            ((bw * s).round() as u32).clamp(1, w),
            ((bh * s).round() as u32).clamp(1, h),
        )
    };
    let ox = (w - tw) / 2;
    let oy = match align {
        Align::Bottom => h - th,
        Align::Center => (h - th) / 2,
        Align::Top => 0,
    };
    let mut out = Rgba {
        w,
        h,
        px: vec![[0; 4]; (w * h) as usize],
    };
    for ty in 0..th {
        for tx in 0..tw {
            let sx0 = x0 + (tx as f64 * bw / tw as f64) as u32;
            let sx1 = (x0 + ((tx + 1) as f64 * bw / tw as f64) as u32)
                .max(sx0 + 1)
                .min(x1);
            let sy0 = y0 + (ty as f64 * bh / th as f64) as u32;
            let sy1 = (y0 + ((ty + 1) as f64 * bh / th as f64) as u32)
                .max(sy0 + 1)
                .min(y1);
            // The block's inner part: generated pixel art blurs at block edges.
            let (ix0, ix1) = inner(sx0, sx1);
            let (iy0, iy1) = inner(sy0, sy1);
            let mut counts: HashMap<u16, u32> = HashMap::new();
            let (mut opaque, mut total) = (0, 0);
            for y in iy0..iy1 {
                for x in ix0..ix1 {
                    total += 1;
                    if mask[(y * raw.w + x) as usize] {
                        opaque += 1;
                        *counts.entry(rgb555(raw.at(x, y))).or_default() += 1;
                    }
                }
            }
            if opaque * 2 < total {
                continue;
            }
            let mut best = counts
                .iter()
                .max_by_key(|&(c, n)| (*n, *c))
                .map(|(c, _)| *c)
                .unwrap();
            // Outlines: a dark colour covering a third of the block wins (thin lines survive
            // the downscale).
            let dark = counts
                .iter()
                .filter(|&(c, n)| luma(*c) < 60 && n * 3 >= opaque)
                .max_by_key(|&(c, n)| (*n, *c));
            if let Some((&c, _)) = dark {
                best = c;
            }
            if key.is_some() && magenta(from555(best)) {
                continue;
            }
            out.px[((oy + ty) * w + ox + tx) as usize] = from555(best);
        }
    }
    Ok(out)
}

/// The middle half of `a..b` (at least one pixel).
fn inner(a: u32, b: u32) -> (u32, u32) {
    let n = b - a;
    if n < 4 {
        return (a, b);
    }
    (a + n / 4, b - n / 4)
}

// ---- build ----

/// An 8x8 tile of colours (`None`: transparent).
type ArtTile = [Option<u16>; 64];

fn default_cell(c: char) -> Option<Cell> {
    let (behavior, collision) = match c {
        '.' => ("MB_NORMAL", 0),
        '#' => ("MB_NORMAL", 1),
        'D' => ("MB_ANIMATED_DOOR", 1),
        _ => return None,
    };
    Some(Cell {
        behavior: behavior.into(),
        collision,
    })
}

fn behavior_value(name: &str) -> Result<u16> {
    extract::names::METATILE_BEHAVIORS
        .value(name)
        .map(|v| v as u16)
        .with_context(|| format!("unknown behaviour {name}"))
}

/// One metatile cell of a feature before quantization.
struct CellArt {
    feature: usize,
    cx: usize,
    cy: usize,
    /// TL, TR, BL, BR.
    tiles: [ArtTile; 4],
    blank: bool,
}

pub fn build(games: &Path, extracted: &Path, name: &str) -> Result<()> {
    let spec: Spec = read_ron(&games.join(format!("gfx/{name}.ron")))?;
    ensure!(spec.name == name, "the spec names {:?}", spec.name);
    let primary_dir = if games
        .join(format!("tilesets/{}.ron", spec.primary))
        .exists()
    {
        games.to_path_buf()
    } else {
        extracted.to_path_buf()
    };
    let primary: TilesetFile =
        read_ron(&primary_dir.join(format!("tilesets/{}.ron", spec.primary)))?;
    ensure!(
        !primary.secondary,
        "{} is a secondary tileset",
        spec.primary
    );

    // Cut every feature into cells and tiles.
    let mut cells = Vec::new();
    for (fi, f) in spec.features.iter().enumerate() {
        let h = f.cells.len();
        ensure!(h > 0, "feature {}: no cells", f.name);
        let w = f.cells[0].chars().count();
        ensure!(
            f.cells.iter().all(|r| r.chars().count() == w),
            "feature {}: rows of different lengths",
            f.name
        );
        for c in f.cells.iter().flat_map(|r| r.chars()) {
            ensure!(
                f.legend.contains_key(&c) || default_cell(c).is_some(),
                "feature {}: cell {c:?} is not in its legend",
                f.name
            );
        }
        let path = games.join(format!("gfx/{name}/{}.png", f.name));
        let art = read_rgba(&path)?;
        ensure!(
            art.w == 16 * w as u32 && art.h == 16 * h as u32,
            "{}: {}x{} px, the cells want {}x{}",
            path.display(),
            art.w,
            art.h,
            16 * w,
            16 * h
        );
        for cy in 0..h {
            for cx in 0..w {
                let tiles: [ArtTile; 4] = core::array::from_fn(|t| {
                    core::array::from_fn(|i| {
                        let x = (cx * 16 + t % 2 * 8 + i % 8) as u32;
                        let y = (cy * 16 + t / 2 * 8 + i / 8) as u32;
                        let c = art.at(x, y);
                        (c[3] >= 128).then(|| rgb555(c))
                    })
                });
                let blank = tiles.iter().flatten().all(Option::is_none);
                cells.push(CellArt {
                    feature: fi,
                    cx,
                    cy,
                    tiles,
                    blank,
                });
            }
        }
    }

    // Palettes: cluster the tiles, quantize each cluster.
    let art_tiles: Vec<&ArtTile> = cells
        .iter()
        .filter(|c| !c.blank)
        .flat_map(|c| c.tiles.iter())
        .collect();
    let (palettes, assign) = quantize(&art_tiles, PAL_COUNT, 15);

    // Tiles (deduplicated with flips), metatiles and attributes.
    let mut tiles: Vec<[u8; 64]> = vec![[0; 64]];
    let mut tile_ids: HashMap<[u8; 64], u16> = HashMap::from([([0; 64], 0)]);
    let mut metatiles: Vec<[u16; 8]> = Vec::new();
    let mut attributes: Vec<u16> = Vec::new();
    let mut blocks: Vec<Vec<u16>> = spec.features.iter().map(|_| Vec::new()).collect();
    let mut k = 0;
    for c in &cells {
        let f = &spec.features[c.feature];
        let ch = f.cells[c.cy].chars().nth(c.cx).unwrap();
        let kind = f
            .legend
            .get(&ch)
            .cloned()
            .or_else(|| default_cell(ch))
            .unwrap();
        let ground = f.ground.or(spec.ground);
        let collision = kind.collision as u16;
        let mut entries = [0u16; 8];
        let top_start = match ground {
            Some(g) => {
                let gm = primary
                    .metatiles
                    .get(g as usize)
                    .with_context(|| format!("ground metatile {g} is not in {}", spec.primary))?;
                entries[..4].copy_from_slice(&gm[..4]);
                4
            }
            None => 0,
        };
        for t in 0..4 {
            let pal = if c.blank { 0 } else { assign[k] };
            k += usize::from(!c.blank);
            let indexed: [u8; 64] = core::array::from_fn(|i| match c.tiles[t][i] {
                None => 0,
                Some(col) => nearest(&palettes[pal], col) as u8 + 1,
            });
            let (id, hf, vf) = tile_id(&mut tiles, &mut tile_ids, indexed);
            entries[top_start + t] = (512 + id)
                | (hf as u16) << 10
                | (vf as u16) << 11
                | ((PAL_FIRST + pal) as u16) << 12;
        }
        let layer = if f.over.contains(&c.cy) {
            LAYER_NORMAL
        } else {
            LAYER_COVERED
        };
        let attr = behavior_value(&kind.behavior)? | layer << 12;
        metatiles.push(entries);
        attributes.push(attr);
        blocks[c.feature].push((512 + metatiles.len() as u16 - 1) | collision << 10);
    }
    ensure!(
        tiles.len() <= MAX_TILES,
        "{} tiles (max {MAX_TILES})",
        tiles.len()
    );
    ensure!(
        metatiles.len() <= MAX_METATILES,
        "{} metatiles (max {MAX_METATILES})",
        metatiles.len()
    );

    // tilesets/<name>.{ron,png}
    let mut pals = vec![[0u16; 16]; 16];
    for (i, p) in palettes.iter().enumerate() {
        for (j, &c) in p.iter().enumerate() {
            pals[PAL_FIRST + i][j + 1] = c;
        }
    }
    let file = TilesetFile {
        name: name.to_string(),
        secondary: true,
        tile_count: tiles.len() as u16,
        palettes: pals.clone(),
        metatiles: metatiles.clone(),
        attributes: attributes.clone(),
    };
    let ts_dir = games.join("tilesets");
    std::fs::create_dir_all(&ts_dir)?;
    std::fs::write(ts_dir.join(format!("{name}.ron")), to_ron(&file)?)?;
    let rows = tiles.len().div_ceil(16);
    let mut img = IndexedImage {
        width: 128,
        height: rows as u32 * 8,
        pixels: vec![0; 128 * rows * 8],
    };
    for (i, t) in tiles.iter().enumerate() {
        for (p, &v) in t.iter().enumerate() {
            let (x, y) = (i % 16 * 8 + p % 8, i / 16 * 8 + p / 8);
            img.pixels[y * 128 + x] = v;
        }
    }
    extract::gfx::write_png_indexed(
        &ts_dir.join(format!("{name}.png")),
        &img,
        &Palette(pals[PAL_FIRST]),
    )?;

    // stamps/<name>_<feature>.ron
    let st_dir = games.join("stamps");
    std::fs::create_dir_all(&st_dir)?;
    for (f, b) in spec.features.iter().zip(&blocks) {
        let w = f.cells[0].chars().count();
        let stamp = StampFile {
            primary: spec.primary.clone(),
            secondary: name.to_string(),
            width: w as u16,
            height: f.cells.len() as u16,
            elevation: false,
            blocks: to_rows(b, w),
        };
        std::fs::write(
            st_dir.join(format!("{name}_{}.ron", f.name)),
            to_ron(&stamp)?,
        )?;
    }

    // The review picture.
    let primary_tiles = read_tiles(&primary_dir, &spec.primary)?;
    let preview = draw_features(&spec, &blocks, &primary, &primary_tiles, &file, &tiles)?;
    let out = crate::root().join(format!("target/silice-gfx/{name}.png"));
    write_rgba(&out, &preview)?;
    println!(
        "{name}: {} features, {} tiles, {} metatiles, {} palettes; preview {}",
        spec.features.len(),
        tiles.len(),
        metatiles.len(),
        palettes.len(),
        out.display()
    );
    Ok(())
}

/// The id of `t` among `tiles` (added if new), with the flips that give it.
fn tile_id(
    tiles: &mut Vec<[u8; 64]>,
    ids: &mut HashMap<[u8; 64], u16>,
    t: [u8; 64],
) -> (u16, bool, bool) {
    for (hf, vf) in [(false, false), (true, false), (false, true), (true, true)] {
        let f: [u8; 64] = core::array::from_fn(|i| {
            let (x, y) = (i % 8, i / 8);
            let (x, y) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
            t[y * 8 + x]
        });
        if let Some(&id) = ids.get(&f) {
            return (id, hf, vf);
        }
    }
    tiles.push(t);
    let id = (tiles.len() - 1) as u16;
    ids.insert(t, id);
    (id, false, false)
}

// ---- quantization ----

fn lab(c: u16) -> [f32; 3] {
    // Perceptual-ish weights on RGB555.
    let (r, g, b) = ((c & 31) as f32, (c >> 5 & 31) as f32, (c >> 10 & 31) as f32);
    [r * 0.30 * 3.0, g * 0.59 * 3.0, b * 0.11 * 3.0 + b * 0.5]
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

fn nearest(pal: &[u16], c: u16) -> usize {
    let l = lab(c);
    (0..pal.len())
        .min_by(|&a, &b| dist(lab(pal[a]), l).total_cmp(&dist(lab(pal[b]), l)))
        .unwrap()
}

/// `k` colours for weighted `colours` (k-means, seeded by the most frequent distinct ones).
fn kmeans_colours(colours: &HashMap<u16, u32>, k: usize) -> Vec<u16> {
    let mut list: Vec<(u16, u32)> = colours.iter().map(|(&c, &n)| (c, n)).collect();
    list.sort_by_key(|&(c, n)| (std::cmp::Reverse(n), c));
    if list.len() <= k {
        return list.into_iter().map(|(c, _)| c).collect();
    }
    // Farthest-point seeding, weighted by frequency.
    let mut centers: Vec<[f32; 3]> = vec![lab(list[0].0)];
    while centers.len() < k {
        let next = list
            .iter()
            .max_by(|a, b| {
                let da = centers
                    .iter()
                    .map(|&c| dist(c, lab(a.0)))
                    .fold(f32::MAX, f32::min)
                    * (a.1 as f32).sqrt();
                let db = centers
                    .iter()
                    .map(|&c| dist(c, lab(b.0)))
                    .fold(f32::MAX, f32::min)
                    * (b.1 as f32).sqrt();
                da.total_cmp(&db)
            })
            .unwrap();
        centers.push(lab(next.0));
    }
    for _ in 0..12 {
        let mut sum = vec![([0f32; 3], 0f32); k];
        for &(c, n) in &list {
            let l = lab(c);
            let j = (0..k)
                .min_by(|&a, &b| dist(centers[a], l).total_cmp(&dist(centers[b], l)))
                .unwrap();
            for (s, v) in sum[j].0.iter_mut().zip(l) {
                *s += v * n as f32;
            }
            sum[j].1 += n as f32;
        }
        for j in 0..k {
            if sum[j].1 > 0.0 {
                centers[j] = core::array::from_fn(|i| sum[j].0[i] / sum[j].1);
            }
        }
    }
    // Each center becomes its closest real colour (crisp, no muddy averages).
    let mut out: Vec<u16> = centers
        .iter()
        .map(|&c| {
            list.iter()
                .min_by(|a, b| dist(lab(a.0), c).total_cmp(&dist(lab(b.0), c)))
                .unwrap()
                .0
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn tile_error(t: &ArtTile, pal: &[u16]) -> f32 {
    t.iter()
        .flatten()
        .map(|&c| dist(lab(c), lab(pal[nearest(pal, c)])))
        .sum()
}

/// Up to `npal` palettes of `per` colours, and each tile's palette.
fn quantize(tiles: &[&ArtTile], npal: usize, per: usize) -> (Vec<Vec<u16>>, Vec<usize>) {
    if tiles.is_empty() {
        return (vec![vec![0]], Vec::new());
    }
    // Seed: k-means on the tiles' mean colours.
    let mean = |t: &ArtTile| {
        let v: Vec<[f32; 3]> = t.iter().flatten().map(|&c| lab(c)).collect();
        if v.is_empty() {
            return [0.0; 3];
        }
        core::array::from_fn(|i| v.iter().map(|x| x[i]).sum::<f32>() / v.len() as f32)
    };
    let means: Vec<[f32; 3]> = tiles.iter().map(|t| mean(t)).collect();
    let k = npal.min(tiles.len());
    let mut centers: Vec<[f32; 3]> = vec![means[0]];
    while centers.len() < k {
        let next = (0..means.len())
            .max_by(|&a, &b| {
                let da = centers
                    .iter()
                    .map(|&c| dist(c, means[a]))
                    .fold(f32::MAX, f32::min);
                let db = centers
                    .iter()
                    .map(|&c| dist(c, means[b]))
                    .fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .unwrap();
        centers.push(means[next]);
    }
    let mut assign: Vec<usize> = means
        .iter()
        .map(|m| {
            (0..k)
                .min_by(|&a, &b| dist(centers[a], *m).total_cmp(&dist(centers[b], *m)))
                .unwrap()
        })
        .collect();
    let mut palettes = Vec::new();
    for _ in 0..6 {
        palettes = (0..k)
            .map(|j| {
                let mut h: HashMap<u16, u32> = HashMap::new();
                for (t, _) in tiles.iter().zip(&assign).filter(|(_, a)| **a == j) {
                    for &c in t.iter().flatten() {
                        *h.entry(c).or_default() += 1;
                    }
                }
                let p = kmeans_colours(&h, per);
                if p.is_empty() { vec![0] } else { p }
            })
            .collect();
        let next: Vec<usize> = tiles
            .iter()
            .map(|t| {
                (0..k)
                    .min_by(|&a, &b| {
                        tile_error(t, &palettes[a]).total_cmp(&tile_error(t, &palettes[b]))
                    })
                    .unwrap()
            })
            .collect();
        if next == assign {
            break;
        }
        assign = next;
    }
    (palettes, assign)
}

// ---- preview ----

fn read_tiles(dir: &Path, name: &str) -> Result<Vec<[u8; 64]>> {
    let img = extract::gfx::read_png_indexed(&dir.join(format!("tilesets/{name}.png")))?;
    let per_row = img.width as usize / 8;
    let n = per_row * img.height as usize / 8;
    Ok((0..n)
        .map(|i| {
            core::array::from_fn(|p| {
                let (x, y) = (i % per_row * 8 + p % 8, i / per_row * 8 + p / 8);
                img.pixels[y * img.width as usize + x]
            })
        })
        .collect())
}

fn draw_features(
    spec: &Spec,
    blocks: &[Vec<u16>],
    primary: &TilesetFile,
    primary_tiles: &[[u8; 64]],
    secondary: &TilesetFile,
    secondary_tiles: &[[u8; 64]],
) -> Result<Rgba> {
    const PAD: u32 = 8;
    let width = spec
        .features
        .iter()
        .map(|f| f.cells[0].chars().count() as u32 * 16 + PAD)
        .sum::<u32>()
        + PAD;
    let height = spec
        .features
        .iter()
        .map(|f| f.cells.len() as u32 * 16)
        .max()
        .unwrap_or(0)
        + 2 * PAD;
    let mut img = Rgba {
        w: width,
        h: height,
        px: vec![[40, 40, 48, 255]; (width * height) as usize],
    };
    let mut ox = PAD;
    for (f, b) in spec.features.iter().zip(blocks) {
        let w = f.cells[0].chars().count();
        for (i, &block) in b.iter().enumerate() {
            let m = block & 0x3FF;
            let (ts, idx) = if m >= 512 {
                (secondary, m - 512)
            } else {
                (primary, m)
            };
            let entries = ts.metatiles[idx as usize];
            let (cx, cy) = ((i % w) as u32 * 16 + ox, (i / w) as u32 * 16 + PAD);
            for (e, &entry) in entries.iter().enumerate() {
                let t = (entry & 0x3FF) as usize;
                let (hf, vf) = (entry & 0x400 != 0, entry & 0x800 != 0);
                let pal = (entry >> 12) as usize;
                let (tiles, pals) = if t >= 512 {
                    (secondary_tiles, &secondary.palettes)
                } else {
                    (primary_tiles, &primary.palettes)
                };
                let Some(tile) = tiles.get(t % 512) else {
                    continue;
                };
                let q = e % 4;
                for p in 0..64 {
                    let (x, y) = (p % 8, p / 8);
                    let (sx, sy) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
                    let v = tile[sy * 8 + sx];
                    if v == 0 {
                        continue;
                    }
                    let c = from555(pals[pal][v as usize]);
                    let (px, py) = (cx + (q % 2 * 8 + x) as u32, cy + (q / 2 * 8 + y) as u32);
                    img.px[(py * width + px) as usize] = c;
                }
            }
        }
        ox += w as u32 * 16 + PAD;
    }
    // 3x.
    let mut big = Rgba {
        w: width * 3,
        h: height * 3,
        px: vec![[0; 4]; (width * height * 9) as usize],
    };
    for y in 0..height * 3 {
        for x in 0..width * 3 {
            big.px[(y * width * 3 + x) as usize] = img.at(x / 3, y / 3);
        }
    }
    Ok(big)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prep_keys_crops_and_scales_blocks() {
        // A 2x1 object drawn 10x on a magenta 40x20 canvas.
        let (w, h) = (40, 20);
        let mut px = vec![[255, 0, 255, 255]; w * h];
        for y in 5..15 {
            for x in 10..30 {
                px[y * w + x] = if x < 20 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
            }
        }
        let raw = Rgba {
            w: w as u32,
            h: h as u32,
            px,
        };
        let out = prep(&raw, 4, 4, Some([255, 0, 255]), 110, false, Align::Bottom).unwrap();
        // Scaled to 4x2, bottom-aligned.
        assert_eq!(out.at(0, 0)[3], 0);
        assert_eq!(out.at(0, 2), from555(rgb555([255, 0, 0, 255])));
        assert_eq!(out.at(3, 3), from555(rgb555([0, 0, 255, 255])));
        assert_eq!(out.at(0, 1)[3], 0);
    }

    #[test]
    fn quantize_keeps_few_colours_exact() {
        let a: ArtTile = core::array::from_fn(|i| Some(if i % 2 == 0 { 0x001F } else { 0x7C00 }));
        let b: ArtTile = core::array::from_fn(|i| if i < 32 { Some(0x03E0) } else { None });
        let (pals, assign) = quantize(&[&a, &b], 7, 15);
        for (t, &p) in [&a, &b].iter().zip(&assign) {
            assert_eq!(tile_error(t, &pals[p]), 0.0);
        }
    }

    #[test]
    fn tiles_dedupe_with_flips() {
        let mut tiles = vec![[0u8; 64]];
        let mut ids = HashMap::from([([0u8; 64], 0u16)]);
        let t: [u8; 64] = core::array::from_fn(|i| (i % 8) as u8);
        let flipped: [u8; 64] = core::array::from_fn(|i| (7 - i % 8) as u8);
        assert_eq!(tile_id(&mut tiles, &mut ids, t), (1, false, false));
        assert_eq!(tile_id(&mut tiles, &mut ids, flipped), (1, true, false));
        assert_eq!(tiles.len(), 2);
    }
}
