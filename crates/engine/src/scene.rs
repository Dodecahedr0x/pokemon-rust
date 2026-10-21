/// 8x8 4bpp tile: one u32 per row, pixel x stored in nibble x (low nibble first), GBA layout.
pub type Tile = [u32; 8];

/// Map width/height in tiles (one 256x256 px GBA screenblock).
pub const MAP_TILES: usize = 32;

/// A single-layer scene. M1 ignores palette banks and flip bits.
pub struct Scene {
    /// RGB555; index 0 is also the backdrop.
    pub palette: [u16; 16],
    pub tiles: &'static [Tile],
    /// GBA text-BG map entries, row-major, 32x32.
    pub map: &'static [u16; MAP_TILES * MAP_TILES],
}

/// The only contract between the engine and the game data.
pub trait Content {
    fn scene(&self) -> &Scene;
}

/// Palette index of pixel (`x`, `y`); both must be < 8.
pub const fn tile_pixel(tile: &Tile, x: usize, y: usize) -> u8 {
    ((tile[y] >> (x * 4)) & 0xF) as u8
}

pub const fn map_tile_index(entry: u16) -> usize {
    (entry & 0x03FF) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_pixel_reads_low_nibble_first() {
        let t: Tile = [0x0000_0021, 0, 0, 0, 0, 0, 0, 0x7000_0000];
        assert_eq!(tile_pixel(&t, 0, 0), 1);
        assert_eq!(tile_pixel(&t, 1, 0), 2);
        assert_eq!(tile_pixel(&t, 2, 0), 0);
        assert_eq!(tile_pixel(&t, 7, 7), 7);
    }

    #[test]
    fn map_entry_tile_index_ignores_flags() {
        assert_eq!(map_tile_index(0xF3FF), 0x3FF);
    }
}
