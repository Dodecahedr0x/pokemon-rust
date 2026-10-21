use extract::gfx::{self, Palette};
use extract::{Rom, fixtures};

/// `general` primary tileset struct (`struct Tileset`, include/global.fieldmap.h).
const GENERAL_US: u32 = 0x3df704;
const GENERAL_FR: u32 = 0x3e45f0;

/// Tiles of `general` whose art is localized in the French ROM: the gym sign
/// ("GYM" -> "ARN", tiles 222/223/238/239) and the Poké Mart sign ("MART" -> "POKé SHOP",
/// tiles 327-330, 343, 358). Everything else is byte-identical to the decomp.
const FR_LOCALIZED_TILES: &[usize] = &[222, 223, 238, 239, 327, 328, 329, 330, 343, 358];

fn both() -> [(&'static str, Rom, u32, &'static [usize]); 2] {
    [
        ("US", fixtures::us(), GENERAL_US, &[]),
        ("FR", fixtures::fr(), GENERAL_FR, FR_LOCALIZED_TILES),
    ]
}

fn tile_of(i: usize, width: u32) -> usize {
    let (x, y) = (i % width as usize, i / width as usize);
    y / 8 * (width as usize / 8) + x / 8
}

#[test]
fn general_tileset_tiles_match_decomp_png() {
    let dir = fixtures::decomp().join("data/tilesets/primary/general");
    let expected = gfx::read_png_indexed(&dir.join("tiles.png")).unwrap();
    assert_eq!((expected.width, expected.height), (128, 256));
    for (name, rom, ts, localized) in both() {
        assert_eq!(rom.u8(ts).unwrap(), 1, "{name}: general is compressed");
        let tiles_off = rom.ptr(ts + 4).unwrap().expect("tiles ptr");
        let raw = rom.lz77_at(tiles_off).unwrap();
        assert_eq!(raw.len(), 512 * 32, "{name}: 512 tiles");
        let img = gfx::tiles_4bpp_to_image(&raw, 16).unwrap();
        assert_eq!(img.width, expected.width, "{name}");
        // Same height today; compare over the common part and require any tail to be index 0.
        let n = (img.height.min(expected.height) * img.width) as usize;
        let mut diff_tiles: Vec<usize> = (0..n)
            .filter(|&i| img.pixels[i] != expected.pixels[i])
            .map(|i| tile_of(i, img.width))
            .collect();
        diff_tiles.sort_unstable();
        diff_tiles.dedup();
        assert_eq!(
            diff_tiles, localized,
            "{name}: tiles differing from tiles.png"
        );
        assert!(img.pixels[n..].iter().all(|&p| p == 0), "{name}: rom tail");
        assert!(
            expected.pixels[n..].iter().all(|&p| p == 0),
            "{name}: png tail"
        );
    }
}

#[test]
fn decomp_png_reencodes_to_us_rom_bytes() {
    let png = fixtures::decomp().join("data/tilesets/primary/general/tiles.png");
    let img = gfx::read_png_indexed(&png).unwrap();
    let us = fixtures::us();
    let raw = us
        .lz77_at(us.ptr(GENERAL_US + 4).unwrap().unwrap())
        .unwrap();
    assert_eq!(gfx::image_to_tiles_4bpp(&img).unwrap(), raw);
}

#[test]
fn general_palette_0_matches_decomp_pal() {
    let pal_path = fixtures::decomp().join("data/tilesets/primary/general/palettes/00.pal");
    let expected = gfx::read_jasc_pal(&pal_path).unwrap();
    for (name, rom, ts, _) in both() {
        let pals = rom.ptr(ts + 8).unwrap().expect("palettes ptr");
        let pal = Palette::from_bytes(rom.slice(pals, 32).unwrap()).unwrap();
        assert_eq!(pal, expected, "{name}");
    }
}

#[test]
fn lz77_at_bad_offset_is_err() {
    let us = fixtures::us();
    let len = us.bytes().len() as u32;
    let err = us.lz77_at(len - 2).unwrap_err();
    assert!(
        format!("{err:#}").contains(&format!("{:#x}", len - 2)),
        "{err:#}"
    );
}
