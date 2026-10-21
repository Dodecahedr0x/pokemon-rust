//! The dialogue window graphics against the decomp's `graphics/text_window/message_box.png`.
use extract::{fixtures, gfx, window};

#[test]
fn message_box_matches_decomp_png_in_both_roms() {
    let path = fixtures::decomp().join("graphics/text_window/message_box.png");
    // 4bpp tiles, 7 per row in the PNG, read row-major like gbagfx.
    let img = gfx::read_png_indexed(&path).unwrap();
    assert_eq!((img.width, img.height), (56, 16));
    let tiles = gfx::image_to_tiles_4bpp(&img).unwrap();
    assert_eq!(tiles.len(), window::TILE_COUNT * 32);
    // `message_box.gbapal` is the PNG's palette, 8-bit channels divided by 8.
    let reader = png::Decoder::new(std::fs::File::open(&path).unwrap())
        .read_info()
        .unwrap();
    let plte = reader.info().palette.as_ref().unwrap().to_vec();
    let palette: Vec<u16> = plte
        .chunks(3)
        .map(|c| (c[0] / 8) as u16 | ((c[1] / 8) as u16) << 5 | ((c[2] / 8) as u16) << 10)
        .collect();
    assert_eq!(palette.len(), 16);
    for (name, rom, at) in [
        ("US", fixtures::us(), 0xddd748),
        ("FR", fixtures::fr(), 0xddd8ec),
    ] {
        let mb = window::locate_message_box(&rom).unwrap();
        assert_eq!(mb.tiles_offset, at, "{name}");
        assert_eq!(mb.palette_offset, at - 32, "{name}");
        assert_eq!(mb.tiles, tiles, "{name}");
        assert_eq!(mb.palette.to_vec(), palette, "{name}");
        // The text colors: white background, dark gray text, light gray shadow.
        let p = mb.palette;
        assert_eq!(p[window::TEXT_BG as usize], 0x7FFF, "{name}");
        let rgb = |c: u16| [c & 0x1F, (c >> 5) & 0x1F, c >> 10];
        let (fg, sh) = (
            rgb(p[window::TEXT_FG as usize]),
            rgb(p[window::TEXT_SHADOW as usize]),
        );
        for k in 0..3 {
            assert!(
                fg[k] < 16 && fg[k] < sh[k] && sh[k] < 31,
                "{name}: {fg:?} {sh:?}"
            );
        }
    }
}
