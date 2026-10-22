//! Everything the generator needs, in memory: read from `content/extracted/` ([`load`]) or
//! built in ([`crate::stub`]). [`write`] does the reverse of [`load`] (for test fixtures).

use anyhow::{Context, Result, bail, ensure};
use extract::export::{read_map, read_ron, read_tileset, to_ron};
use extract::gfx::{self, IndexedImage, Palette};
use extract::schema::{
    FORMAT_VERSION, FieldEffectSpriteFile, FontFile, ItemsFile, Manifest, MapFile, MapSectionsFile,
    MultichoiceFile, SpriteFile, StdStringsFile, TextFile, TilesetFile, WindowFile,
};
use extract::text::{Lang, encode_lang};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type Tile = [u32; 8];
/// `.latfont` glyph: 4 tiles (TL, TR, BL, BR) of 8 little-endian u16 rows, leftmost pixel in
/// bits 15-14.
pub type Glyph = [u8; 64];

pub struct TilesetIn {
    pub file: TilesetFile,
    pub tiles: Vec<Tile>,
}

pub struct SpriteIn {
    pub file: SpriteFile,
    /// Each frame's tiles in GBA 1D OBJ order.
    pub frames: Vec<Vec<Tile>>,
}

/// A field-effect pic (`sprites/field_effects/`): the emotes.
pub struct EmoteIn {
    /// `ExclamationMark`, `QuestionMark`, `HeartIcon`.
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub frames: Vec<Vec<Tile>>,
    pub palette: [u16; 16],
}

/// Task C's `tables/` (absent from older extracts: the tables are then empty).
#[derive(Default)]
pub struct Tables {
    pub items: ItemsFile,
    pub multichoice: MultichoiceFile,
    pub std_strings: StdStringsFile,
    pub map_sections: MapSectionsFile,
}

pub struct World {
    pub lang: Lang,
    /// In export order (`manifest.maps`).
    pub maps: Vec<MapFile>,
    pub tilesets: Vec<TilesetIn>,
    pub sprites: Vec<SpriteIn>,
    pub glyphs: Vec<Glyph>,
    pub widths: Vec<u8>,
    pub window: WindowFile,
    pub window_tiles: Vec<Tile>,
    /// Text name -> Gen3 bytes, 0xFF-terminated.
    pub texts: BTreeMap<String, Vec<u8>>,
    /// The script IR (`scripts/*.ron`), when the extract has one (Milestone 8 Task D2 lowers it).
    pub ir: Option<crate::scripts::Ir>,
    /// sha1 of the source ROM (`manifest.sha1`).
    pub sha1: String,
    /// Absolute extracted dir (`None` for the built-in stub).
    pub dir: Option<PathBuf>,
    /// Battle data; the stub's when the extracted dir has no `battle/` (test fixtures).
    pub battle: crate::battle::BattleIn,
    /// Items, multichoice lists, std strings, map sections (`tables/`, Task C).
    pub tables: Tables,
    /// The emote pics (`sprites/field_effects/`, Task A).
    pub emotes: Vec<EmoteIn>,
}

/// `read_ron` of a file added within the format (`FORMAT_MINOR`), or the default when an older
/// extract lacks it.
#[macro_export]
macro_rules! read_opt {
    ($path:expr) => {{
        let p = $path;
        if p.exists() {
            extract::export::read_ron(&p)?
        } else {
            Default::default()
        }
    }};
}

/// A build warning (shown by cargo).
pub fn warn(msg: &str) {
    println!("cargo::warning={msg}");
}

pub fn lang(code: &str) -> Result<Lang> {
    Ok(match code {
        "fr" => Lang::Fr,
        "en" => Lang::En,
        _ => anyhow::bail!("unknown language {code:?}"),
    })
}

pub fn tiles_of(img: &IndexedImage) -> Result<Vec<Tile>> {
    let bytes = gfx::image_to_tiles_4bpp(img)?;
    Ok(bytes
        .chunks_exact(32)
        .map(|t| {
            core::array::from_fn(|r| u32::from_le_bytes(t[r * 4..r * 4 + 4].try_into().unwrap()))
        })
        .collect())
}

fn image_of(tiles: &[Tile], tiles_per_row: u32) -> Result<IndexedImage> {
    let bytes: Vec<u8> = tiles
        .iter()
        .flatten()
        .flat_map(|r| r.to_le_bytes())
        .collect();
    gfx::tiles_4bpp_to_image(&bytes, tiles_per_row)
}

pub fn crop(img: &IndexedImage, x: u32, y: u32, w: u32, h: u32) -> Result<IndexedImage> {
    ensure!(
        x + w <= img.width && y + h <= img.height,
        "crop out of bounds"
    );
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for row in y..y + h {
        let s = (row * img.width + x) as usize;
        pixels.extend_from_slice(&img.pixels[s..s + w as usize]);
    }
    Ok(IndexedImage {
        width: w,
        height: h,
        pixels,
    })
}

const GLYPH: u32 = 16;

pub fn glyph_of(img: &IndexedImage, i: u32, per_row: u32) -> Glyph {
    let (ox, oy) = (i % per_row * GLYPH, i / per_row * GLYPH);
    let mut g = [0u8; 64];
    for tile in 0..4u32 {
        for y in 0..8u32 {
            let mut v = 0u16;
            for x in 0..8u32 {
                let (px, py) = (ox + (tile & 1) * 8 + x, oy + (tile >> 1) * 8 + y);
                let p = img.pixels[(py * img.width + px) as usize] & 3;
                v |= u16::from(p) << (14 - 2 * x);
            }
            let at = ((tile * 8 + y) * 2) as usize;
            g[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    g
}

fn glyph_pixels(g: &Glyph, img: &mut IndexedImage, i: u32, per_row: u32) {
    let (ox, oy) = (i % per_row * GLYPH, i / per_row * GLYPH);
    for tile in 0..4u32 {
        for y in 0..8u32 {
            let at = ((tile * 8 + y) * 2) as usize;
            let v = u16::from_le_bytes([g[at], g[at + 1]]);
            for x in 0..8u32 {
                let (px, py) = (ox + (tile & 1) * 8 + x, oy + (tile >> 1) * 8 + y);
                img.pixels[(py * img.width + px) as usize] = ((v >> (14 - 2 * x)) & 3) as u8;
            }
        }
    }
}

/// The `format` of a manifest, if it can be read.
pub fn format_of(manifest: &Path) -> Option<u32> {
    read_ron::<Manifest>(manifest).ok().map(|m| m.format)
}

/// Reads an extracted dir written by `cargo xtask extract` (plus Task B's scripts and texts
/// when present).
pub fn load(dir: &Path) -> Result<World> {
    let manifest: Manifest = read_ron(&dir.join("manifest.ron"))?;
    ensure!(
        manifest.format == FORMAT_VERSION,
        "{}: format v{}, expected v{FORMAT_VERSION}; run `cargo xtask extract` again",
        dir.display(),
        manifest.format
    );
    // Within a format, files are only added: an older minor lacks some (Milestone 8).
    if manifest.format_minor < extract::schema::FORMAT_MINOR {
        let missing =
            extract::schema::missing_since(manifest.format_minor, |p| dir.join(p).exists());
        bail!(
            "{}: format v{}.{}, expected v{FORMAT_VERSION}.{} (missing: {}); re-run `cargo xtask \
             extract`",
            dir.display(),
            manifest.format,
            manifest.format_minor,
            extract::schema::FORMAT_MINOR,
            if missing.is_empty() {
                "none of the listed files".to_string()
            } else {
                missing.join(", ")
            }
        );
    }
    let lang = lang(&manifest.language)?;
    let maps = manifest
        .maps
        .iter()
        .map(|m| read_map(dir, m))
        .collect::<Result<Vec<_>>>()?;
    let mut tilesets = Vec::new();
    for name in &manifest.tilesets {
        let file = read_tileset(dir, name)?;
        let img = gfx::read_png_indexed(&dir.join(format!("tilesets/{name}.png")))?;
        let mut tiles = tiles_of(&img).with_context(|| format!("tileset {name}"))?;
        ensure!(
            tiles.len() >= file.tile_count as usize,
            "tileset {name}: {} tiles in png, {} in ron",
            tiles.len(),
            file.tile_count
        );
        tiles.truncate(file.tile_count as usize);
        tilesets.push(TilesetIn { file, tiles });
    }
    let mut sprites = Vec::new();
    for name in &manifest.sprites {
        let file: SpriteFile = read_ron(&dir.join(format!("sprites/{name}.ron")))?;
        let img = gfx::read_png_indexed(&dir.join(format!("sprites/{name}.png")))?;
        let (cw, ch) = (u32::from(file.cell_width), u32::from(file.cell_height));
        // Every frame is cut at the cell size: the engine's `SpriteData` has one size. A frame
        // smaller than its cell (berry tree sprouts, 16x16 in 16x32 cells) sits top-left in its
        // cell, padded with index 0 (transparent) below.
        if file
            .frame_sizes
            .iter()
            .any(|&(w, h)| (u32::from(w), u32::from(h)) != (cw, ch))
        {
            warn(&format!(
                "sprite {name}: frames of mixed sizes {:?}, padded to {cw}x{ch}",
                file.frame_sizes
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
            ));
        }
        let frames = (0..file.frame_sizes.len() as u32)
            .map(|k| tiles_of(&crop(&img, k * cw, 0, cw, ch)?))
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("sprite {name}"))?;
        sprites.push(SpriteIn { file, frames });
    }
    let (glyphs, widths) = read_font(dir, "latin_normal")?;
    let window: WindowFile = read_ron(&dir.join("window/message_box.ron"))?;
    let img = gfx::read_png_indexed(&dir.join("window/message_box.png"))?;
    let mut window_tiles = tiles_of(&img)?;
    ensure!(
        window_tiles.len() >= usize::from(window.tile_count),
        "window: {} tiles in png, {} in ron",
        window_tiles.len(),
        window.tile_count
    );
    window_tiles.truncate(window.tile_count as usize);

    let text_path = dir.join(format!("text/{}.ron", lang.code()));
    let mut texts = BTreeMap::new();
    if text_path.exists() {
        let file: TextFile = read_ron(&text_path)?;
        for (k, v) in file {
            let bytes = encode_lang(&v, lang).with_context(|| format!("text {k}"))?;
            texts.insert(k, bytes);
        }
    }
    let ir = crate::scripts::read_ir(dir, &manifest)?;
    let tables = read_tables(dir)?;
    let emotes = read_emotes(dir)?;
    let battle = if dir.join("battle/species.ron").exists() {
        crate::battle_load::load(dir).context("battle data")?
    } else {
        crate::stub::battle()
    };
    Ok(World {
        lang,
        maps,
        tilesets,
        sprites,
        glyphs,
        widths,
        window,
        window_tiles,
        texts,
        ir,
        sha1: manifest.sha1.clone(),
        dir: Some(dir.to_path_buf()),
        battle,
        tables,
        emotes,
    })
}

/// Reads `tables/*.ron` (Task C), each optional: an extract without one has an empty table.
/// The generator encodes the strings (`codegen::tables`).
fn read_tables(dir: &Path) -> Result<Tables> {
    let t = dir.join("tables");
    let tables = Tables {
        items: read_opt!(t.join("items.ron")),
        multichoice: read_opt!(t.join("multichoice.ron")),
        std_strings: read_opt!(t.join("std_strings.ron")),
        map_sections: read_opt!(t.join("map_sections.ron")),
    };
    Ok(tables)
}

/// Reads `sprites/field_effects/*.{ron,png}` (absent from older extracts).
fn read_emotes(dir: &Path) -> Result<Vec<EmoteIn>> {
    let d = dir.join("sprites/field_effects");
    let mut out = Vec::new();
    if !d.is_dir() {
        return Ok(out);
    }
    let mut names: Vec<String> = std::fs::read_dir(&d)?
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "ron").then(|| p.file_stem()?.to_str().map(String::from))?
        })
        .collect();
    names.sort();
    for name in names {
        let f: FieldEffectSpriteFile = read_ron(&d.join(format!("{name}.ron")))?;
        let img = gfx::read_png_indexed(&d.join(format!("{name}.png")))?;
        let (w, h) = (u32::from(f.width), u32::from(f.height));
        let frames = (0..u32::from(f.frames))
            .map(|k| tiles_of(&crop(&img, k * w, 0, w, h)?))
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("field effect {name}"))?;
        out.push(EmoteIn {
            name: f.name,
            width: f.width,
            height: f.height,
            frames,
            palette: f.palette,
        });
    }
    Ok(out)
}

/// Reads `font/<name>.{ron,png}`: glyphs in ROM layout and widths.
pub fn read_font(dir: &Path, name: &str) -> Result<(Vec<Glyph>, Vec<u8>)> {
    let font: FontFile = read_ron(&dir.join(format!("font/{name}.ron")))?;
    let img = gfx::read_png_indexed(&dir.join(format!("font/{name}.png")))?;
    let per_row = u32::from(font.glyphs_per_row);
    ensure!(per_row > 0, "font {name}: glyphs_per_row is 0");
    let rows = u32::from(font.glyphs).div_ceil(per_row);
    ensure!(
        img.width >= per_row * GLYPH && img.height >= rows * GLYPH,
        "font {name}: png is {}x{}, {} glyphs need {}x{}",
        img.width,
        img.height,
        font.glyphs,
        per_row * GLYPH,
        rows * GLYPH
    );
    ensure!(
        font.widths.len() == usize::from(font.glyphs),
        "font {name}: {} widths for {} glyphs",
        font.widths.len(),
        font.glyphs
    );
    let glyphs = (0..u32::from(font.glyphs))
        .map(|i| glyph_of(&img, i, per_row))
        .collect();
    Ok((glyphs, font.widths))
}

/// Writes `w` as an extracted dir that [`load`] reads back (maps, tilesets, sprites, font,
/// window, manifest; not texts or scripts). Used to build test fixtures.
pub fn write(w: &World, dir: &Path) -> Result<()> {
    let put = |rel: &str, s: String| -> Result<()> {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, s).with_context(|| format!("writing {}", p.display()))
    };
    let pal = Palette([0; 16]);
    let png = |rel: &str, img: &IndexedImage| -> Result<()> {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap())?;
        gfx::write_png_indexed(&p, img, &pal)
    };
    for m in &w.maps {
        put(&format!("maps/{}.ron", m.name), to_ron(m)?)?;
    }
    for t in &w.tilesets {
        put(&format!("tilesets/{}.ron", t.file.name), to_ron(&t.file)?)?;
        png(
            &format!("tilesets/{}.png", t.file.name),
            &image_of(&t.tiles, 16)?,
        )?;
    }
    for s in &w.sprites {
        put(&format!("sprites/{}.ron", s.file.name), to_ron(&s.file)?)?;
        let (cw, ch) = (u32::from(s.file.cell_width), u32::from(s.file.cell_height));
        let mut sheet = IndexedImage {
            width: cw * s.frames.len() as u32,
            height: ch,
            pixels: vec![0; (cw * ch) as usize * s.frames.len()],
        };
        for (k, f) in s.frames.iter().enumerate() {
            let img = image_of(f, cw / 8)?;
            for y in 0..ch {
                let src = (y * img.width) as usize;
                let dst = (y * sheet.width + k as u32 * cw) as usize;
                sheet.pixels[dst..dst + cw as usize]
                    .copy_from_slice(&img.pixels[src..src + cw as usize]);
            }
        }
        png(&format!("sprites/{}.png", s.file.name), &sheet)?;
    }
    let per_row = 16u32;
    let rows = (w.glyphs.len() as u32).div_ceil(per_row);
    let mut font = IndexedImage {
        width: per_row * GLYPH,
        height: rows * GLYPH,
        pixels: vec![0; (per_row * GLYPH * rows * GLYPH) as usize],
    };
    for (i, g) in w.glyphs.iter().enumerate() {
        glyph_pixels(g, &mut font, i as u32, per_row);
    }
    png("font/latin_normal.png", &font)?;
    put(
        "font/latin_normal.ron",
        to_ron(&FontFile {
            glyphs: w.glyphs.len() as u16,
            glyphs_per_row: per_row as u16,
            widths: w.widths.clone(),
        })?,
    )?;
    png(
        "window/message_box.png",
        &image_of(&w.window_tiles, w.window_tiles.len() as u32)?,
    )?;
    put("window/message_box.ron", to_ron(&w.window)?)?;
    let manifest = Manifest {
        format: FORMAT_VERSION,
        format_minor: extract::schema::FORMAT_MINOR,
        game_code: "BPEF".into(),
        sha1: w.sha1.clone(),
        language: w.lang.code().into(),
        maps: w.maps.iter().map(|m| m.name.clone()).collect(),
        tilesets: w.tilesets.iter().map(|t| t.file.name.clone()).collect(),
        sprites: w.sprites.iter().map(|s| s.file.name.clone()).collect(),
        provenance: BTreeMap::new(),
        skipped_graphics: Default::default(),
        unknown_script_ops: BTreeMap::new(),
        files: BTreeMap::new(),
    };
    put("manifest.ron", to_ron(&manifest)?)
}
