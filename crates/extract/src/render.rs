//! Map preview rendering: tilesets + layout blocks to an RGBA image, the way the game draws them.
//!
//! Rules follow the decomp (`src/fieldmap.c`, `src/field_camera.c`, `include/fieldmap.h`):
//! - VRAM holds 512 primary tiles then 512 secondary tiles (`NUM_TILES_IN_PRIMARY`,
//!   `NUM_TILES_TOTAL`); a tile index >= 512 reads `secondary[idx - 512]`.
//! - Palette slots 0-5 come from the primary tileset's palettes 0-5 and slots 6-12 from the
//!   secondary tileset's palettes 6-12 (`LoadTilesetPalette`, `NUM_PALS_IN_PRIMARY`,
//!   `NUM_PALS_TOTAL`). Slots 13-15 are not loaded from tilesets.
//! - `LoadTilesetPalette` overwrites slot 0 colour 0 with `RGB_BLACK`, so the backdrop (what shows
//!   through colour 0 on every layer) is black.
//! - A metatile is 8 tile entries: bottom layer TL, TR, BL, BR then top layer TL, TR, BL, BR
//!   (`DrawMetatile`). Metatile ids >= 512 read `secondary.metatiles[id - 512]`.
//! - The metatile's layer type is `attributes[id] >> 12` (`METATILE_ATTR_LAYER_MASK`). For
//!   `METATILE_LAYER_TYPE_NORMAL` the game fills BG3 under the metatile with tile entry 0x3014
//!   (primary tile 20, palette 3); the other types put the bottom layer on BG3 over the backdrop.
//!   Either way the bottom layer is drawn below the top layer.
//! - Palette slots 13-15 hold whatever else the game loaded there, so pixels using them are
//!   drawn transparent.

use crate::Rom;
use crate::gfx::{TILE_BYTES, rgb555_to_rgba};
pub use crate::maps::TILES_PER_TILESET;
use crate::maps::{MapLayout, TilesetRef};
use anyhow::{Context, Result, ensure};

/// Palettes in a tileset's `palettes` array.
pub const PALETTES_PER_TILESET: usize = 16;
/// Palette slots loaded from the primary tileset (`NUM_PALS_IN_PRIMARY`).
pub const NUM_PALS_IN_PRIMARY: usize = 6;
/// Palette slots loaded from both tilesets (`NUM_PALS_TOTAL`).
pub const NUM_PALS_TOTAL: usize = 13;

const METATILE_ID_MASK: u16 = 0x3FF;
/// `METATILE_LAYER_TYPE_NORMAL`: bottom layer on BG2, top on BG1, BG3 filled with [`NORMAL_UNDERLAY`].
const LAYER_TYPE_NORMAL: u16 = 0;
/// What `DrawMetatile` writes to BG3 for NORMAL metatiles: primary tile 20, palette 3.
const NORMAL_UNDERLAY: u16 = 0x3014;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TilesetData {
    /// 4bpp tile data, 32 bytes per tile, low nibble = left pixel.
    pub tiles: Vec<u8>,
    /// The tileset's 16 BGR555 palettes.
    pub palettes: Vec<[u16; 16]>,
    pub metatiles: Vec<[u16; 8]>,
    pub attributes: Vec<u16>,
}

/// Reads a tileset's graphics. Compressed tiles are LZ77-decompressed in full. Uncompressed tiles
/// have no stored length, so like `CopyTilesetToVram` we take 512 tiles (clamped to the ROM end).
pub fn load_tileset(rom: &Rom, t: &TilesetRef) -> Result<TilesetData> {
    let ctx = || format!("tileset at {:#x}", t.offset);
    let tiles = if t.compressed {
        rom.lz77_at(t.tiles).with_context(ctx)?
    } else {
        let avail = (rom.bytes().len() as u32)
            .checked_sub(t.tiles)
            .with_context(|| format!("{}: tiles at {:#x} past ROM end", ctx(), t.tiles))?;
        let len = (TILES_PER_TILESET * TILE_BYTES).min(avail as usize) / TILE_BYTES * TILE_BYTES;
        rom.slice(t.tiles, len).with_context(ctx)?.to_vec()
    };
    let pal_bytes = rom
        .slice(t.palettes, PALETTES_PER_TILESET * 32)
        .with_context(ctx)?;
    let palettes = pal_bytes
        .chunks_exact(32)
        .map(|p| std::array::from_fn(|i| u16::from_le_bytes([p[2 * i], p[2 * i + 1]])))
        .collect();
    let n = t.metatile_count as usize;
    let mt = rom.slice(t.metatiles, n * 16).with_context(ctx)?;
    let metatiles = mt
        .chunks_exact(16)
        .map(|m| std::array::from_fn(|i| u16::from_le_bytes([m[2 * i], m[2 * i + 1]])))
        .collect();
    let attr = rom.slice(t.metatile_attributes, n * 2).with_context(ctx)?;
    let attributes = attr
        .chunks_exact(2)
        .map(|a| u16::from_le_bytes([a[0], a[1]]))
        .collect();
    Ok(TilesetData {
        tiles,
        palettes,
        metatiles,
        attributes,
    })
}

/// RGBA, (width*16) x (height*16): metatile = 2x2 tiles bottom layer then 2x2 top layer, over
/// a 2x2 underlay of entry 0x3014 for layer type NORMAL (`attributes[id] >> 12 == 0`);
/// tile entry bits: 0-9 index (>=512 => secondary[idx-512]), 10 hflip, 11 vflip, 12-15 palette
/// (0-5 primary, 6-12 secondary, 13-15 drawn transparent); colour 0 is transparent over a black
/// backdrop (the game forces primary palette 0 colour 0 to black). Tiles past the loaded tile
/// data are transparent (VRAM is cleared on map load).
pub fn render_map(
    layout: &MapLayout,
    primary: &TilesetData,
    secondary: &TilesetData,
) -> Result<Vec<u8>> {
    let (w, h) = (layout.width as usize, layout.height as usize);
    ensure!(
        layout.blocks.len() == w * h,
        "layout at {:#x}: {} blocks for {w}x{h}",
        layout.offset,
        layout.blocks.len()
    );
    let stride = w * 16;
    let mut out = [0, 0, 0, 255].repeat(stride * h * 16);
    for (i, &block) in layout.blocks.iter().enumerate() {
        let id = (block & METATILE_ID_MASK) as usize;
        let (mt, attr) = metatile(id, primary, secondary)
            .with_context(|| format!("layout at {:#x}, block {i}", layout.offset))?;
        let (bx, by) = (i % w * 16, i / w * 16);
        let underlay = [NORMAL_UNDERLAY; 4];
        let under: &[u16] = if attr >> 12 == LAYER_TYPE_NORMAL {
            &underlay
        } else {
            &[]
        };
        for (k, &entry) in under.iter().chain(mt).enumerate() {
            let k = k % 4;
            let (tx, ty) = (bx + k % 2 * 8, by + k / 2 * 8);
            draw_tile(&mut out, stride, tx, ty, entry, primary, secondary).with_context(|| {
                format!("layout at {:#x}, block {i}, metatile {id}", layout.offset)
            })?;
        }
    }
    Ok(out)
}

fn metatile<'a>(
    id: usize,
    primary: &'a TilesetData,
    secondary: &'a TilesetData,
) -> Result<(&'a [u16; 8], u16)> {
    let (set, idx, name) = if id < TILES_PER_TILESET {
        (primary, id, "primary")
    } else {
        (secondary, id - TILES_PER_TILESET, "secondary")
    };
    let mt = set.metatiles.get(idx).with_context(|| {
        format!(
            "metatile {id}: {name} tileset has only {} metatiles",
            set.metatiles.len()
        )
    })?;
    let attr = set.attributes.get(idx).with_context(|| {
        format!(
            "metatile {id}: {name} tileset has only {} attributes",
            set.attributes.len()
        )
    })?;
    Ok((mt, *attr))
}

fn draw_tile(
    out: &mut [u8],
    stride: usize,
    x0: usize,
    y0: usize,
    entry: u16,
    primary: &TilesetData,
    secondary: &TilesetData,
) -> Result<()> {
    let idx = (entry & 0x3FF) as usize;
    let (hflip, vflip) = (entry & 0x400 != 0, entry & 0x800 != 0);
    let pal = (entry >> 12) as usize;
    let set = if idx < TILES_PER_TILESET {
        primary
    } else {
        secondary
    };
    let t = idx % TILES_PER_TILESET;
    // Tiles past the loaded data read VRAM the game cleared to zero on map load: transparent.
    let Some(tile) = set.tiles.get(t * TILE_BYTES..(t + 1) * TILE_BYTES) else {
        return Ok(());
    };
    if tile.iter().all(|&b| b == 0) {
        return Ok(());
    }
    let palette = match pal {
        0..NUM_PALS_IN_PRIMARY => &primary.palettes,
        NUM_PALS_IN_PRIMARY..NUM_PALS_TOTAL => &secondary.palettes,
        // Not loaded from tilesets: undefined contents, drawn transparent.
        _ => return Ok(()),
    }
    .get(pal)
    .with_context(|| format!("palette {pal} missing"))?;
    for y in 0..8 {
        for x in 0..8 {
            let (sx, sy) = (if hflip { 7 - x } else { x }, if vflip { 7 - y } else { y });
            let b = tile[sy * 4 + sx / 2];
            let c = if sx % 2 == 0 { b & 0xF } else { b >> 4 };
            if c == 0 {
                continue;
            }
            let p = ((y0 + y) * stride + x0 + x) * 4;
            out[p..p + 4].copy_from_slice(&rgba(palette[c as usize]));
        }
    }
    Ok(())
}

fn rgba(c: u16) -> [u8; 4] {
    rgb555_to_rgba(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile whose pixel (x, y) has colour `f(x, y)`.
    fn tile(f: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        (0..32)
            .map(|k| {
                let (x, y) = (k % 4 * 2, k / 4);
                f(x, y) | f(x + 1, y) << 4
            })
            .collect()
    }

    fn pals(base: u16) -> Vec<[u16; 16]> {
        // Palette p colour c = base + p * 16 + c (distinct 15-bit values).
        (0..16)
            .map(|p| std::array::from_fn(|c| base + (p * 16 + c) as u16))
            .collect()
    }

    fn layout(blocks: Vec<u16>, w: u16, h: u16) -> MapLayout {
        MapLayout {
            offset: 0,
            width: w,
            height: h,
            border: vec![0; 4],
            blocks,
            primary_tileset: 0,
            secondary_tileset: 0,
        }
    }

    fn px(img: &[u8], stride: usize, x: usize, y: usize) -> [u8; 4] {
        let p = (y * stride + x) * 4;
        img[p..p + 4].try_into().unwrap()
    }

    fn sets() -> (TilesetData, TilesetData) {
        // Primary tile 0: empty (colour 0). Tile 1: colour 1 only at (0, 0). Tile 2: solid 2.
        let mut ptiles = tile(|_, _| 0);
        ptiles.extend(tile(|x, y| (x == 0 && y == 0) as u8));
        ptiles.extend(tile(|_, _| 2));
        // Secondary tile 0 (index 512): colour 3 only at (0, 0).
        let stiles = tile(|x, y| 3 * (x == 0 && y == 0) as u8);
        let primary = TilesetData {
            tiles: ptiles,
            palettes: pals(0x100),
            metatiles: vec![
                // Metatile 0: bottom TL tile 1 unflipped pal 0, TR tile 1 hflip pal 5,
                // BL tile 1 vflip pal 1, BR tile 1 h+v flip pal 2; top all empty.
                [1, 0x5401, 0x1801, 0x2C01, 0, 0, 0, 0],
            ],
            attributes: vec![0],
        };
        let secondary = TilesetData {
            tiles: stiles,
            palettes: pals(0x4000),
            metatiles: vec![
                // Metatile 512: bottom solid tile 2 pal 3; top TL secondary tile 512 pal 7.
                [0x3002, 0x3002, 0x3002, 0x3002, 0x7200, 0, 0, 0],
            ],
            attributes: vec![0],
        };
        (primary, secondary)
    }

    fn col(c: u16) -> [u8; 4] {
        rgba(c)
    }

    #[test]
    fn flips_and_palette_banks() {
        let (p, s) = sets();
        let img = render_map(&layout(vec![0, 512 | 0x0400], 2, 1), &p, &s).unwrap();
        assert_eq!(img.len(), 32 * 16 * 4);
        let st = 32;
        let black = [0, 0, 0, 255];
        // TL: unflipped, pixel at (0,0), primary palette 0 colour 1.
        assert_eq!(px(&img, st, 0, 0), col(0x100 + 1));
        assert_eq!(px(&img, st, 1, 0), black);
        // TR: hflip -> pixel at (15, 0), palette 5 from primary.
        assert_eq!(px(&img, st, 15, 0), col(0x100 + 5 * 16 + 1));
        assert_eq!(px(&img, st, 8, 0), black);
        // BL: vflip -> (0, 15), palette 1.
        assert_eq!(px(&img, st, 0, 15), col(0x100 + 16 + 1));
        assert_eq!(px(&img, st, 0, 8), black);
        // BR: both flips -> (15, 15), palette 2.
        assert_eq!(px(&img, st, 15, 15), col(0x100 + 2 * 16 + 1));
        // Second block: metatile 512 (collision bits ignored). Bottom solid primary tile 2 with
        // palette 3 from primary; top secondary tile 512 with palette 7 from secondary.
        assert_eq!(px(&img, st, 16, 0), col(0x4000 + 7 * 16 + 3));
        assert_eq!(px(&img, st, 17, 0), col(0x100 + 3 * 16 + 2));
        assert_eq!(px(&img, st, 31, 15), col(0x100 + 3 * 16 + 2));
    }

    #[test]
    fn tiles_past_loaded_data_are_transparent() {
        let (p, mut s) = sets();
        // Bottom: secondary tile 800 (not loaded) with palette 14; top: nothing.
        s.metatiles[0] = [0xE320; 8];
        s.metatiles[0][4..].fill(0);
        let img = render_map(&layout(vec![512], 1, 1), &p, &s).unwrap();
        assert!(img.chunks_exact(4).all(|c| c == [0, 0, 0, 255]));
    }

    #[test]
    fn backdrop_is_black_even_if_palette_0_colour_0_is_not() {
        let (p, s) = sets();
        assert_ne!(p.palettes[0][0], 0);
        let img = render_map(&layout(vec![0], 1, 1), &p, &s).unwrap();
        assert_eq!(px(&img, 16, 4, 4), [0, 0, 0, 255]);
    }

    #[test]
    fn palette_slots_13_to_15_draw_transparent() {
        let (p, mut s) = sets();
        // Bottom: solid primary tile 2 with palettes 13, 14, 15, 15; top: nothing.
        s.metatiles[0] = [0xD002, 0xE002, 0xF002, 0xF002, 0, 0, 0, 0];
        let img = render_map(&layout(vec![512], 1, 1), &p, &s).unwrap();
        assert!(img.chunks_exact(4).all(|c| c == [0, 0, 0, 255]));
    }

    /// Primary tileset with 21 tiles where tile 20 is solid colour 4, plus metatile 1:
    /// bottom TL = tile 1 (one pixel at (0,0)), everything else empty.
    fn with_garbage_tile() -> (TilesetData, TilesetData) {
        let (mut p, s) = sets();
        for _ in 3..20 {
            p.tiles.extend(tile(|_, _| 0));
        }
        p.tiles.extend(tile(|_, _| 4));
        p.metatiles.push([1, 0, 0, 0, 0, 0, 0, 0]);
        p.attributes.push(0);
        (p, s)
    }

    #[test]
    fn normal_layer_type_draws_tile_20_palette_3_underneath() {
        let (mut p, s) = with_garbage_tile();
        let garbage = col(0x100 + 3 * 16 + 4);
        for (layer_type, under) in [(0u16, garbage), (1, [0, 0, 0, 255]), (2, [0, 0, 0, 255])] {
            p.attributes[1] = layer_type << 12 | 0x0012;
            let img = render_map(&layout(vec![1], 1, 1), &p, &s).unwrap();
            // The bottom layer still draws on top of the underlay.
            assert_eq!(px(&img, 16, 0, 0), col(0x100 + 1), "type {layer_type}");
            for (x, y) in [(1, 0), (8, 0), (0, 8), (15, 15)] {
                assert_eq!(px(&img, 16, x, y), under, "type {layer_type} ({x},{y})");
            }
        }
    }

    #[test]
    fn bad_references_are_errors() {
        let (p, mut s) = sets();
        // Metatile 1 does not exist in the primary set.
        assert!(render_map(&layout(vec![1], 1, 1), &p, &s).is_err());
        // Metatile 512 exists but has no attribute.
        s.attributes.clear();
        assert!(render_map(&layout(vec![512], 1, 1), &p, &s).is_err());
        // Block count mismatch.
        assert!(render_map(&layout(vec![0], 2, 1), &p, &s).is_err());
    }
}
