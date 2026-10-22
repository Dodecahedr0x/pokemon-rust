use super::*;

static GLYPHS: [[u8; 64]; 4] = [[0; 64]; 4];
static FONT: Font = Font {
    glyphs: &GLYPHS,
    widths: &[3, 5, 6, 7],
};

#[test]
fn widths_follow_gen3() {
    // `GetStringWidth`: glyph widths, codes skipped, the longest line.
    assert_eq!(text_width(&FONT, &[1, 2, 0xFF, 3]), 11);
    assert_eq!(text_width(&FONT, &[1, 0xFC, 0x01, 2, 3, 0xFF]), 12);
    assert_eq!(text_width(&FONT, &[3, 3, 0xFE, 1, 0xFF]), 14);
    // `ConvertPixelWidthToTileWidth` and `ScriptMenu_AdjustLeftCoordFromWidth`.
    assert_eq!(tile_width(0), 2);
    assert_eq!(tile_width(7), 3);
    assert_eq!(tile_width(1000), MAX_WIDTH);
    assert_eq!(adjust_left(20, 8), 20);
    assert_eq!(adjust_left(21, 8), 20);
    assert_eq!(adjust_left(5, MAX_WIDTH), 0);
}

#[test]
fn layout_cuts_lists_to_the_screen_and_the_canvas() {
    static MANY: [&[u8]; 20] = [&[1, 0xFF]; 20];
    let m = Multichoice::new(&FONT, 0, &MANY, (0, 0), 0, true);
    // Rows 0..19 hold nine choices (18 rows) and the bottom frame row.
    assert_eq!(m.window().height(), 18);
    let m = Multichoice::new(&FONT, 0, &MANY, (0, 15), 0, true);
    assert_eq!(m.window().height(), 4);
    static WIDE: [&[u8]; 9] = [&[3; 200]; 9];
    let m = Multichoice::new(&FONT, 0, &WIDE, (0, 0), 0, true);
    assert_eq!(m.window().height(), 10);
    assert_eq!(m.window().width(), MAX_WIDTH);
    assert!(m.window().width() as usize * m.window().height() as usize <= CANVAS_TILES);
}
