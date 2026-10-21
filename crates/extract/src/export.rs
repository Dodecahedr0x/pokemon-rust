//! Writes everything the other modules read from the ROM as editable files (see
//! docs/plans/2026-10-21-milestone-2-extract.md, Task H):
//!
//! ```text
//! manifest.ron                 game code, sha1, every ROM offset used, sha1 of every other file
//! maps/<Map>/map.ron           MapFile: header, layout, events, connections, script refs
//! tilesets/<hex>/tiles.png     indexed, 16 tiles per row, palette 0
//! tilesets/<hex>/{palettes,metatiles,attributes}.ron
//! sprites/<id>.png + .ron      frames side by side with their own palette; size, frames, tag
//! font/latin_normal.png + .ron 2bpp glyph sheet (gbagfx layout and palette); widths
//! text/fr.ron                  "<Map>.<hex offset>" -> string, for every text the maps' scripts show
//! previews/<Map>.png           map render
//! ```
//!
//! Output is deterministic: maps are sorted (`BTreeMap`/`BTreeSet`) and the RON style is fixed.

use crate::gfx::{self, IndexedImage, Palette};
use crate::maps::{self, LITTLEROOT, MapHeader, MapId, OLDALE, ROUTE101, TilesetRef};
use crate::render::{self, TilesetData};
use crate::sprites::{self, ObjectGraphics, ObjectGraphicsTable};
use crate::{GameCode, Rom, font, scripts};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The exported maps and their directory names (decomp `MAP_*` names in CamelCase).
pub const MAPS: [(MapId, &str); 3] = [
    (LITTLEROOT, "LittlerootTown"),
    (OLDALE, "OldaleTown"),
    (ROUTE101, "Route101"),
];

/// `OBJ_EVENT_GFX_BRENDAN_NORMAL`.
pub const BRENDAN_NORMAL: u8 = 0;
/// `OBJ_EVENT_GFX_MAY_NORMAL`.
pub const MAY_NORMAL: u8 = 89;

/// gbagfx's font palette (`gFontPalette` in `tools/gbagfx/font.c`).
const FONT_PALETTE: [[u8; 3]; 4] = [
    [0x90, 0xC8, 0xFF],
    [0x38, 0x38, 0x38],
    [0xD8, 0xD8, 0xD8],
    [0xFF, 0xFF, 0xFF],
];
/// Glyphs per row of `latin_normal.png` (256 px wide like the decomp's).
const FONT_GLYPHS_PER_ROW: usize = 16;
const TILESET_TILES_PER_ROW: u32 = 16;

/// `maps/<Map>/map.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapFile {
    pub name: String,
    pub id: MapId,
    pub header: MapHeader,
    pub scripts: MapScripts,
}

/// What the map's scripts reach (offsets are ROM file offsets).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapScripts {
    /// Entry points: object, coord and bg event scripts, then map scripts.
    pub roots: Vec<u32>,
    /// Keys into `text/fr.ron`, `"<Map>.<hex offset>"`.
    pub texts: Vec<String>,
    pub movements: Vec<u32>,
    pub braille: Vec<u32>,
    /// Instructions the walker could not decode: offset -> opcode.
    pub unknown: BTreeMap<u32, u8>,
}

/// `sprites/<id>.ron`, also listed in the manifest.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpriteFile {
    /// File offset of the `ObjectEventGraphicsInfo`.
    pub offset: u32,
    pub width: u16,
    pub height: u16,
    pub frames: usize,
    pub palette_tag: u16,
    /// BGR555, also the PNG's palette.
    pub palette: [u16; 16],
}

/// `font/latin_normal.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FontFile {
    pub glyph_width_max: u8,
    pub glyph_height: u8,
    pub glyphs_per_row: usize,
    /// Advance width per character code.
    pub widths: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapEntry {
    pub id: MapId,
    pub header: u32,
    pub layout: u32,
    pub events_objects: usize,
    pub map_scripts: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FontEntry {
    pub glyphs: u32,
    pub widths: u32,
}

/// `manifest.ron`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    /// "BPEF" or "BPEE".
    pub game_code: String,
    /// sha1 of the whole ROM.
    pub sha1: String,
    /// `gMapGroups`.
    pub map_table: u32,
    pub maps: BTreeMap<String, MapEntry>,
    /// Keyed by the struct offset in lower hex (the directory name under `tilesets/`).
    pub tilesets: BTreeMap<String, TilesetRef>,
    pub object_graphics: ObjectGraphicsTable,
    pub sprites: BTreeMap<u8, SpriteFile>,
    /// Object event graphics ids used by the maps that are not table entries (e.g. the
    /// `OBJ_EVENT_GFX_VAR_*` ids resolved at run time), not exported.
    pub skipped_graphics_ids: BTreeSet<u8>,
    pub font: FontEntry,
    pub texts: usize,
    /// Per map, instructions the script walker could not decode (offset -> opcode).
    pub unknown_script_ops: BTreeMap<String, BTreeMap<u32, u8>>,
    /// Every other written file (relative, `/`-separated) -> sha1.
    pub files: BTreeMap<String, String>,
}

/// The fixed RON style of every exported file, with a trailing newline.
pub fn to_ron<T: Serialize>(value: &T) -> Result<String> {
    let config = ron::ser::PrettyConfig::new()
        .new_line("\n".to_string())
        .indentor("    ".to_string())
        .struct_names(false);
    let mut s = ron::ser::to_string_pretty(value, config)?;
    s.push('\n');
    Ok(s)
}

/// Creates files under `root` and records their sha1.
struct Out {
    root: PathBuf,
    files: BTreeMap<String, String>,
}

impl Out {
    fn path(&self, rel: &str) -> Result<PathBuf> {
        let p = self.root.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        Ok(p)
    }

    fn record(&mut self, rel: &str) -> Result<()> {
        let p = self.root.join(rel);
        let bytes = std::fs::read(&p).with_context(|| format!("reading back {}", p.display()))?;
        self.files.insert(
            rel.to_string(),
            sha1_smol::Sha1::from(&bytes).digest().to_string(),
        );
        Ok(())
    }

    fn bytes(&mut self, rel: &str, data: &[u8]) -> Result<()> {
        let p = self.path(rel)?;
        std::fs::write(&p, data).with_context(|| format!("writing {}", p.display()))?;
        self.record(rel)
    }

    fn ron<T: Serialize>(&mut self, rel: &str, value: &T) -> Result<()> {
        self.bytes(rel, to_ron(value)?.as_bytes())
    }

    fn png_indexed(&mut self, rel: &str, img: &IndexedImage, pal: &Palette) -> Result<()> {
        gfx::write_png_indexed(&self.path(rel)?, img, pal)?;
        self.record(rel)
    }

    fn png_rgba(&mut self, rel: &str, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
        gfx::write_png_rgba(&self.path(rel)?, w, h, rgba)?;
        self.record(rel)
    }
}

/// Exports the three maps, their tilesets, sprites, font and text from `rom` into `out`, which is
/// created if needed (existing files are overwritten, others left alone).
pub fn export(rom: &Rom, out: &Path) -> Result<Manifest> {
    let mut o = Out {
        root: out.to_path_buf(),
        files: BTreeMap::new(),
    };
    let table = maps::locate_map_table(rom)?;

    let mut map_entries = BTreeMap::new();
    let mut tileset_refs: BTreeMap<u32, TilesetRef> = BTreeMap::new();
    let mut tileset_data: BTreeMap<u32, TilesetData> = BTreeMap::new();
    let mut texts = BTreeMap::new();
    let mut unknown_ops = BTreeMap::new();
    let mut graphics_ids = BTreeSet::from([BRENDAN_NORMAL, MAY_NORMAL]);

    for (id, name) in MAPS {
        let header = table.header(rom, id)?;
        let layout = &header.layout;
        for ts in [layout.primary_tileset, layout.secondary_tileset] {
            if let std::collections::btree_map::Entry::Vacant(e) = tileset_refs.entry(ts) {
                let r = maps::read_tileset(rom, ts, &table)
                    .with_context(|| format!("{name}: tileset at {ts:#x}"))?;
                tileset_data.insert(ts, render::load_tileset(rom, &r)?);
                e.insert(r);
            }
        }
        let rgba = render::render_map(
            layout,
            &tileset_data[&layout.primary_tileset],
            &tileset_data[&layout.secondary_tileset],
        )?;
        o.png_rgba(
            &format!("previews/{name}.png"),
            u32::from(layout.width) * 16,
            u32::from(layout.height) * 16,
            &rgba,
        )?;

        let roots = scripts::map_script_roots(rom, &header)
            .with_context(|| format!("{name}: script roots"))?;
        let scan = scripts::walk(rom, &roots).with_context(|| format!("{name}: script walk"))?;
        let mut keys = Vec::new();
        for &t in &scan.texts {
            let key = format!("{name}.{t:x}");
            texts.insert(key.clone(), rom.string_at(t).context(name)?);
            keys.push(key);
        }
        if !scan.unknown.is_empty() {
            unknown_ops.insert(name.to_string(), scan.unknown.clone());
        }
        graphics_ids.extend(header.events.objects.iter().map(|ob| ob.graphics_id));

        map_entries.insert(
            name.to_string(),
            MapEntry {
                id,
                header: header.offset,
                layout: layout.offset,
                events_objects: header.events.objects.len(),
                map_scripts: header.map_scripts,
            },
        );
        let file = MapFile {
            name: name.to_string(),
            id,
            scripts: MapScripts {
                roots,
                texts: keys,
                movements: scan.movements.into_iter().collect(),
                braille: scan.braille.into_iter().collect(),
                unknown: scan.unknown,
            },
            header,
        };
        o.ron(&format!("maps/{name}/map.ron"), &file)?;
    }

    for (off, data) in &tileset_data {
        let dir = format!("tilesets/{off:x}");
        let img = gfx::tiles_4bpp_to_image(&data.tiles, TILESET_TILES_PER_ROW)
            .with_context(|| format!("tileset at {off:#x}"))?;
        let pal0 = data
            .palettes
            .first()
            .with_context(|| format!("tileset at {off:#x} has no palettes"))?;
        o.png_indexed(&format!("{dir}/tiles.png"), &img, &Palette(*pal0))?;
        o.ron(&format!("{dir}/palettes.ron"), &data.palettes)?;
        o.ron(&format!("{dir}/metatiles.ron"), &data.metatiles)?;
        o.ron(&format!("{dir}/attributes.ron"), &data.attributes)?;
    }

    let gfx_table = sprites::locate_object_graphics(rom)?;
    let mut sprites_out = BTreeMap::new();
    let mut skipped = BTreeSet::new();
    for id in graphics_ids {
        if u16::from(id) >= gfx_table.count {
            skipped.insert(id);
            continue;
        }
        let g = gfx_table.get(rom, id)?;
        let img = sprite_sheet(&g)?;
        o.png_indexed(&format!("sprites/{id}.png"), &img, &Palette(g.palette))?;
        let file = SpriteFile {
            offset: g.offset,
            width: g.width,
            height: g.height,
            frames: g.frames.len(),
            palette_tag: g.palette_tag,
            palette: g.palette,
        };
        o.ron(&format!("sprites/{id}.ron"), &file)?;
        sprites_out.insert(id, file);
    }

    let (glyphs_off, widths_off) = font::locate_latin_normal(rom)?;
    let f = font::read_latin_normal_at(rom, glyphs_off, widths_off)?;
    o.bytes("font/latin_normal.png", &font_png(&f)?)?;
    o.ron(
        "font/latin_normal.ron",
        &FontFile {
            glyph_width_max: f.glyph_width_max,
            glyph_height: f.glyph_height,
            glyphs_per_row: FONT_GLYPHS_PER_ROW,
            widths: f.widths.clone(),
        },
    )?;

    o.ron("text/fr.ron", &texts)?;

    let manifest = Manifest {
        game_code: match rom.code() {
            GameCode::Bpee => "BPEE",
            GameCode::Bpef => "BPEF",
        }
        .to_string(),
        sha1: rom.sha1_hex(),
        map_table: table.offset,
        maps: map_entries,
        tilesets: tileset_refs
            .into_iter()
            .map(|(off, r)| (format!("{off:x}"), r))
            .collect(),
        object_graphics: gfx_table,
        sprites: sprites_out,
        skipped_graphics_ids: skipped,
        font: FontEntry {
            glyphs: glyphs_off,
            widths: widths_off,
        },
        texts: texts.len(),
        unknown_script_ops: unknown_ops,
        files: o.files.clone(),
    };
    let path = o.path("manifest.ron")?;
    std::fs::write(&path, to_ron(&manifest)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(manifest)
}

/// Frames side by side, each `width x height` with 8x8 tiles row-major.
fn sprite_sheet(g: &ObjectGraphics) -> Result<IndexedImage> {
    let (w, h) = (u32::from(g.width), u32::from(g.height));
    ensure!(
        w > 0 && h > 0 && w.is_multiple_of(8) && h.is_multiple_of(8),
        "object graphics id {} at {:#x}: bad size {w}x{h}",
        g.id,
        g.offset
    );
    let frame_bytes = (w * h / 2) as usize;
    let n = g.frames.len() as u32;
    let sheet_w = w * n;
    let mut pixels = vec![0u8; (sheet_w * h) as usize];
    for (k, frame) in g.frames.iter().enumerate() {
        ensure!(
            frame.len() == frame_bytes,
            "object graphics id {} at {:#x}: frame {k} is {} bytes, expected {frame_bytes}",
            g.id,
            g.offset,
            frame.len()
        );
        let img = gfx::tiles_4bpp_to_image(frame, w / 8)?;
        for y in 0..h {
            let src = (y * w) as usize;
            let dst = (y * sheet_w + k as u32 * w) as usize;
            pixels[dst..dst + w as usize].copy_from_slice(&img.pixels[src..src + w as usize]);
        }
    }
    Ok(IndexedImage {
        width: sheet_w,
        height: h,
        pixels,
    })
}

/// 2-bit indexed PNG in `latin_normal.png`'s layout: 16 glyphs per row, gbagfx palette.
fn font_png(f: &font::Font) -> Result<Vec<u8>> {
    let (gw, gh) = (usize::from(f.glyph_width_max), usize::from(f.glyph_height));
    let rows = f.glyphs.len().div_ceil(FONT_GLYPHS_PER_ROW);
    let (w, h) = (FONT_GLYPHS_PER_ROW * gw, rows * gh);
    let stride = w.div_ceil(4);
    let mut data = vec![0u8; stride * h];
    for (i, glyph) in f.glyphs.iter().enumerate() {
        ensure!(glyph.len() == gw * gh, "glyph {i}: {} pixels", glyph.len());
        let (ox, oy) = (i % FONT_GLYPHS_PER_ROW * gw, i / FONT_GLYPHS_PER_ROW * gh);
        for (p, &px) in glyph.iter().enumerate() {
            ensure!(px < 4, "glyph {i}: pixel value {px}");
            let (x, y) = (ox + p % gw, oy + p / gw);
            data[y * stride + x / 4] |= px << (6 - 2 * (x % 4));
        }
    }
    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, w as u32, h as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Two);
    enc.set_palette(FONT_PALETTE.concat());
    enc.write_header()?.write_image_data(&data)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graphics(w: u16, h: u16, frames: Vec<Vec<u8>>) -> ObjectGraphics {
        ObjectGraphics {
            id: 1,
            offset: 0x100,
            width: w,
            height: h,
            palette_tag: 0,
            frames,
            palette: [0; 16],
        }
    }

    #[test]
    fn sprite_sheet_places_frames_side_by_side() {
        // Two 8x16 frames: frame 0 is all 1s, frame 1 has 2s in its bottom tile only.
        let f0 = vec![0x11; 64];
        let mut f1 = vec![0; 64];
        f1[32..].fill(0x22);
        let img = sprite_sheet(&graphics(8, 16, vec![f0, f1])).unwrap();
        assert_eq!((img.width, img.height), (16, 16));
        assert_eq!(img.pixels[0], 1);
        assert_eq!(img.pixels[8], 0);
        assert_eq!(img.pixels[8 * 16 + 8], 2);
        assert_eq!(img.pixels[15 * 16 + 7], 1);
    }

    #[test]
    fn sprite_sheet_rejects_odd_frame_size() {
        let err = sprite_sheet(&graphics(8, 16, vec![vec![0; 32]])).unwrap_err();
        assert!(format!("{err}").contains("0x100"), "{err}");
    }

    #[test]
    fn font_png_packs_2bpp_msb_first() {
        let mut glyphs = vec![vec![0u8; 256]; 17];
        glyphs[0][1] = 3; // x=1, y=0
        glyphs[16][0] = 2; // second row, x=0, y=16
        let f = font::Font {
            glyph_width_max: 16,
            glyph_height: 16,
            glyphs,
            widths: vec![0; 17],
        };
        let png = font_png(&f).unwrap();
        let mut reader = png::Decoder::new(png.as_slice()).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (256, 32));
        assert_eq!(info.bit_depth, png::BitDepth::Two);
        assert_eq!(buf[0], 0b0011_0000);
        assert_eq!(buf[16 * info.line_size], 0b1000_0000);
    }

    #[test]
    fn to_ron_is_stable_and_round_trips() {
        let v: BTreeMap<String, Vec<u16>> =
            BTreeMap::from([("b".into(), vec![1, 2]), ("a".into(), vec![])]);
        let s = to_ron(&v).unwrap();
        assert!(s.ends_with('\n') && !s.contains('\r'));
        assert!(s.find("\"a\"") < s.find("\"b\""));
        let back: BTreeMap<String, Vec<u16>> = ron::from_str(&s).unwrap();
        assert_eq!(back, v);
    }
}
