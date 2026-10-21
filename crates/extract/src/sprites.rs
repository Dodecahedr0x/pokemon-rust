//! Object event (overworld sprite) graphics: `gObjectEventGraphicsInfoPointers`, each info's
//! `SpriteFrameImage` pic table, and the `sObjectEventSpritePalettes` table.
//!
//! Layouts from the decomp: `struct ObjectEventGraphicsInfo` (include/global.fieldmap.h, 0x24 bytes),
//! `struct SpriteFrameImage { const void *data; u16 size; }` (include/sprite.h, 8 bytes) and
//! `struct SpritePalette { const u16 *data; u16 tag; }` (8 bytes).

use crate::{ROM_BASE, Rom};
use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Start of `gObjectEventGraphicsInfo_BrendanNormal` (id 0): tileTag TAG_NONE, paletteTag BRENDAN,
/// reflectionPaletteTag BRIDGE_REFLECTION, size 512, 16×32, shadow M, tracks FOOT, padding.
/// Built from source constants; unique in BPEE and BPEF.
const BRENDAN_NORMAL_INFO: [u8; 16] = [
    0xFF, 0xFF, 0x00, 0x11, 0x02, 0x11, 0x00, 0x02, 0x10, 0x00, 0x20, 0x00, 0x10, 0x01, 0x00, 0x00,
];
/// `sObjectEventSpritePalettes` starts with the tags NPC_1..NPC_4 (0x1103..0x1106).
const FIRST_PALETTE_TAGS: [u16; 4] = [0x1103, 0x1104, 0x1105, 0x1106];
const PAL_TAG_NONE: u16 = 0x11FF;
const INFO_IMAGES: u32 = 0x1C;
const MAX_IDS: u32 = 256;
const MAX_FRAMES: u32 = 256;
const TILE_BYTES: usize = 32;
/// A 64×64 4bpp frame.
const MAX_FRAME_BYTES: usize = 64 * 64 / 2;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ObjectGraphics {
    pub id: u8,
    /// File offset of the `ObjectEventGraphicsInfo`.
    pub offset: u32,
    pub width: u16,
    pub height: u16,
    pub palette_tag: u16,
    /// Raw 4bpp per frame (8×8 tiles row-major), normally `width * height / 2` bytes each.
    /// Berry trees share pic tables that mix 16×16 dirt/sprout frames and 16×32 tree frames.
    pub frames: Vec<Vec<u8>>,
    /// BGR555.
    pub palette: [u16; 16],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ObjectGraphicsTable {
    /// File offset of `gObjectEventGraphicsInfoPointers`.
    pub offset: u32,
    pub count: u16,
    /// File offset of `sObjectEventSpritePalettes`.
    pub palettes_offset: u32,
}

struct Info {
    palette_tag: u16,
    width: u16,
    height: u16,
    images: u32,
}

/// Reads and sanity-checks an `ObjectEventGraphicsInfo`.
fn read_info(rom: &Rom, off: u32) -> Result<Info> {
    let palette_tag = rom.u16(off + 2)?;
    let width = rom.i16(off + 8)?;
    let height = rom.i16(off + 0x0A)?;
    ensure!(
        palette_tag & 0xFF00 == 0x1100,
        "graphics info at {off:#x}: bad palette tag {palette_tag:#x}"
    );
    let dim_ok = |d: i16| d > 0 && d <= 128 && d % 8 == 0;
    ensure!(
        dim_ok(width) && dim_ok(height),
        "graphics info at {off:#x}: bad size {width}x{height}"
    );
    rom.ptr(off + 0x10)?
        .ok_or_else(|| anyhow!("graphics info at {off:#x}: null oam"))?;
    let images = rom
        .ptr(off + INFO_IMAGES)?
        .ok_or_else(|| anyhow!("graphics info at {off:#x}: null images"))?;
    Ok(Info {
        palette_tag,
        width: width as u16,
        height: height as u16,
        images,
    })
}

/// The info pointed to by table entry `i`, if it is a valid graphics info.
fn entry(rom: &Rom, table: u32, i: u32) -> Option<(u32, Info)> {
    let off = rom.ptr(table + 4 * i).ok()??;
    read_info(rom, off).ok().map(|info| (off, info))
}

/// All values of 4-aligned ROM words that point into `[start, end)` (file offsets).
fn pointer_targets_in(rom: &Rom, start: u32, end: u32) -> HashSet<u32> {
    rom.bytes()
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()).wrapping_sub(ROM_BASE))
        .filter(|t| (start..end).contains(t))
        .collect()
}

fn locate_palettes(rom: &Rom) -> Result<u32> {
    let b = rom.bytes();
    let n = FIRST_PALETTE_TAGS.len() * 8;
    let hits: Vec<u32> = (0..b.len().saturating_sub(n))
        .step_by(4)
        .filter(|&o| {
            FIRST_PALETTE_TAGS.iter().enumerate().all(|(i, &tag)| {
                let e = o + i * 8;
                b[e + 3] == 0x08
                    && u16::from_le_bytes([b[e + 4], b[e + 5]]) == tag
                    && b[e + 6] == 0
                    && b[e + 7] == 0
            })
        })
        .map(|o| o as u32)
        .collect();
    match hits[..] {
        [off] => Ok(off),
        _ => bail!("object event palette table: expected 1 match, found {hits:x?}"),
    }
}

/// Finds `gObjectEventGraphicsInfoPointers` and `sObjectEventSpritePalettes`.
pub fn locate_object_graphics(rom: &Rom) -> Result<ObjectGraphicsTable> {
    let infos = rom.find_all(&BRENDAN_NORMAL_INFO);
    let [info] = infos[..] else {
        bail!("Brendan graphics info signature: expected 1 match, found {infos:x?}");
    };
    read_info(rom, info).context("Brendan graphics info")?;
    let tables: Vec<u32> = rom
        .pointers_to(info)
        .into_iter()
        .filter(|&t| !rom.pointers_to(t).is_empty())
        .collect();
    let [offset] = tables[..] else {
        bail!("graphics info pointer table for info {info:#x}: expected 1, found {tables:x?}");
    };
    // The table ends at the first invalid entry or where another table (Mauville old man) begins.
    let starts = pointer_targets_in(rom, offset + 4, offset + 4 * MAX_IDS);
    let count = (1..MAX_IDS)
        .find(|&i| starts.contains(&(offset + 4 * i)) || entry(rom, offset, i).is_none())
        .unwrap_or(MAX_IDS);
    Ok(ObjectGraphicsTable {
        offset,
        count: count as u16,
        palettes_offset: locate_palettes(rom)?,
    })
}

impl ObjectGraphicsTable {
    pub fn get(&self, rom: &Rom, id: u8) -> Result<ObjectGraphics> {
        ensure!(
            u16::from(id) < self.count,
            "object graphics id {id} out of range (count {})",
            self.count
        );
        let (offset, info) = entry(rom, self.offset, id.into()).ok_or_else(|| {
            anyhow!(
                "object graphics id {id}: invalid entry at {:#x}",
                self.offset + 4 * u32::from(id)
            )
        })?;
        // Pic tables are packed back to back; another info's `images` marks where this one ends.
        let pic_starts: HashSet<u32> = (0..MAX_IDS)
            .map_while(|i| entry(rom, self.offset, i))
            .map(|(_, i)| i.images)
            .collect();
        // The table ends at a non-frame entry (e.g. a berry tree palette slot table) or there.
        let mut frames = Vec::new();
        for k in 0..MAX_FRAMES {
            let e = info.images + 8 * k;
            if k > 0 && pic_starts.contains(&e) {
                break;
            }
            let (Ok(Some(data)), Ok(len)) = (rom.ptr(e), rom.u16(e + 4)) else {
                break;
            };
            let len = usize::from(len);
            if len == 0 || !len.is_multiple_of(TILE_BYTES) || len > MAX_FRAME_BYTES {
                break;
            }
            let Ok(bytes) = rom.slice(data, len) else {
                break;
            };
            frames.push(bytes.to_vec());
        }
        ensure!(
            !frames.is_empty(),
            "object graphics id {id}: no frames at pic table {:#x}",
            info.images
        );
        Ok(ObjectGraphics {
            id,
            offset,
            width: info.width,
            height: info.height,
            palette_tag: info.palette_tag,
            frames,
            palette: self.palette(rom, info.palette_tag)?,
        })
    }

    fn palette(&self, rom: &Rom, tag: u16) -> Result<[u16; 16]> {
        for i in 0.. {
            let e = self.palettes_offset + 8 * i;
            let t = rom.u16(e + 4)?;
            let Some(data) = rom.ptr(e)? else { break };
            if t == PAL_TAG_NONE {
                break;
            }
            if t == tag {
                let mut pal = [0u16; 16];
                for (c, p) in pal.iter_mut().enumerate() {
                    *p = rom.u16(data + 2 * c as u32)?;
                }
                return Ok(pal);
            }
        }
        bail!(
            "palette tag {tag:#x} not in palette table at {:#x}",
            self.palettes_offset
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_header_layout() {
        // tileTag, paletteTag, reflectionPaletteTag, size, width, height.
        let words: Vec<u16> = BRENDAN_NORMAL_INFO[..12]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(words, [0xFFFF, 0x1100, 0x1102, 512, 16, 32]);
    }
}
