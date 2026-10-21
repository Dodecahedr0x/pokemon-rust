//! Dialogue window graphics: `gMessageBox_Gfx` (`graphics/text_window/message_box.png`, 14 4bpp
//! tiles loaded by `LoadMessageBoxGfx`) and `gMessageBox_Pal`, the palette the field dialogue
//! window uses (`GetOverworldTextboxPalettePtr`), which also holds the text colors.
//!
//! In `src/graphics.c` the palette is defined right before the tiles, so it is the 32 bytes in
//! front of them.

use crate::Rom;
use crate::gfx::TILE_BYTES;
use anyhow::{Context, Result, bail, ensure};

/// `LoadMessageBoxGfx` loads 0x1C0 bytes.
pub const TILE_COUNT: usize = 14;
/// Palette indices of `TEXT_COLOR_DARK_GRAY`, `TEXT_COLOR_LIGHT_GRAY` and `TEXT_COLOR_WHITE`:
/// field dialogue prints dark gray on white with a light gray shadow (`AddTextPrinterForMessage`).
pub const TEXT_FG: u8 = 2;
pub const TEXT_SHADOW: u8 = 3;
pub const TEXT_BG: u8 = 1;

/// Tiles 1 and 2 of `message_box.png` in 4bpp (tile 0 is blank). Unique in BPEE and BPEF.
const ANCHOR_TILE: usize = 1;
const ANCHOR: [u8; 64] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xdd,
    0x00, 0x00, 0xdd, 0xdd, 0x00, 0xd0, 0xdd, 0xee, 0x00, 0xdd, 0xee, 0xee, 0x00, 0xdd, 0xee, 0xee,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageBox {
    /// File offset of `gMessageBox_Gfx`.
    pub tiles_offset: u32,
    /// File offset of `gMessageBox_Pal`.
    pub palette_offset: u32,
    /// [`TILE_COUNT`] 4bpp tiles.
    pub tiles: Vec<u8>,
    pub palette: [u16; 16],
}

/// Finds the message box tiles by their anchor, then checks that code points at both the tiles
/// and the palette in front of them.
pub fn locate_message_box(rom: &Rom) -> Result<MessageBox> {
    let hits = rom.find_all(&ANCHOR);
    let [hit] = hits[..] else {
        bail!("message box anchor: expected 1 match, found {hits:#x?}");
    };
    let tiles_offset = hit
        .checked_sub((ANCHOR_TILE * TILE_BYTES) as u32)
        .context("message box anchor too close to ROM start")?;
    let palette_offset = tiles_offset
        .checked_sub(32)
        .context("message box palette before ROM start")?;
    for (what, off) in [("tiles", tiles_offset), ("palette", palette_offset)] {
        ensure!(
            !rom.pointers_to(off).is_empty(),
            "message box {what} at {off:#x}: nothing points to it"
        );
    }
    let tiles = rom.slice(tiles_offset, TILE_COUNT * TILE_BYTES)?.to_vec();
    let palette = rom.u16s(palette_offset, 16)?.try_into().expect("16 colors");
    Ok(MessageBox {
        tiles_offset,
        palette_offset,
        tiles,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROM_BASE;

    fn rom(with_palette_pointer: bool) -> Rom {
        let mut b = vec![0u8; 0x400];
        b[0xAC..0xB0].copy_from_slice(b"BPEE");
        // palette at 0x100, tiles at 0x120.
        b[0x102..0x104].copy_from_slice(&0x7FFFu16.to_le_bytes());
        let at = 0x120 + ANCHOR_TILE * TILE_BYTES;
        b[at..at + ANCHOR.len()].copy_from_slice(&ANCHOR);
        b[0x300..0x304].copy_from_slice(&(ROM_BASE + 0x120).to_le_bytes());
        if with_palette_pointer {
            b[0x304..0x308].copy_from_slice(&(ROM_BASE + 0x100).to_le_bytes());
        }
        Rom::from_bytes(b).unwrap()
    }

    #[test]
    fn locates_tiles_and_palette() {
        let m = locate_message_box(&rom(true)).unwrap();
        assert_eq!((m.tiles_offset, m.palette_offset), (0x120, 0x100));
        assert_eq!(m.tiles.len(), 0x1C0);
        assert_eq!(m.palette[1], 0x7FFF);
        let err = locate_message_box(&rom(false)).unwrap_err();
        assert!(format!("{err}").contains("palette at 0x100"), "{err}");
    }
}
