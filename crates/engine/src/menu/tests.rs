use super::*;

/// The cancel answer the tests give (Gen3 `MULTI_B_PRESSED`).
const B_PRESSED: u16 = 127;

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
    let m = Multichoice::new(&FONT, 0, &MANY, (0, 0), 0, Some(B_PRESSED));
    // The top frame row on row 0, nine choices (rows 1..=18), the bottom frame row on row 19.
    assert_eq!(m.window().top(), 1);
    assert_eq!(m.window().height(), 18);
    let m = Multichoice::new(&FONT, 0, &MANY, (0, 15), 0, Some(B_PRESSED));
    assert_eq!(m.window().height(), 2);
    // A top past the screen still shows one choice and its frame.
    let m = Multichoice::new(&FONT, 0, &MANY, (0, 200), 0, Some(B_PRESSED));
    assert_eq!((m.window().top(), m.window().height()), (17, 2));
    static WIDE: [&[u8]; 9] = [&[3; 200]; 9];
    let m = Multichoice::new(&FONT, 0, &WIDE, (0, 0), 0, Some(B_PRESSED));
    assert_eq!(m.window().height(), 10);
    assert_eq!(m.window().width(), MAX_WIDTH);
    assert!(m.window().width() as usize * m.window().height() as usize <= CANVAS_TILES);
}

#[test]
fn window_sits_inside_its_frame_rect_as_in_gen3() {
    // Gen3 `CreateWindowFromRect(left, top, ..)` puts the window at `(left + 1, top + 1)`:
    // `multichoicedefault 20, 8, MULTI_YESNO, ..` (Battle Arena, Battle Factory) opens at 21, 9.
    static YES_NO: [&[u8]; 2] = [&[1, 2, 0xFF], &[1, 0xFF]];
    let m = Multichoice::new(&FONT, 0, &YES_NO, (20, 8), 1, None);
    let w = m.window();
    assert_eq!((w.left(), w.top(), w.width(), w.height()), (21, 9, 3, 4));
    assert_eq!(m.cursor(), 1);
    // `ScriptMenu_AdjustLeftCoordFromWidth` first, then the + 1.
    static WIDE: [&[u8]; 1] = [&[3; 200]];
    let m = Multichoice::new(&FONT, 0, &WIDE, (5, 0), 0, Some(B_PRESSED));
    assert_eq!((m.window().left(), m.window().top()), (1, 1));
}

#[test]
fn the_top_frame_row_is_drawn_on_screen_row_0() {
    use crate::video::Cmd;
    static ONE: [&[u8]; 1] = [&[1, 0xFF]];
    let mut m = Multichoice::new(&FONT, 0, &ONE, (0, 0), 0, Some(B_PRESSED));
    let mut f = Frame::default();
    let pen = Pen::new(Colors::new(2, 3, 1));
    m.draw_next(&FONT, &pen, GLYPHS_PER_FRAME, &mut f);
    let starts: Vec<u16> = (f.cmds.iter())
        .filter_map(|c| match c {
            Cmd::MapEntries { start, .. } => Some(*start),
            _ => None,
        })
        .collect();
    let rows: Vec<u16> = starts.iter().map(|s| s / MAP_SIZE as u16).collect();
    assert_eq!(rows, [0, 1, 2, 3]);
}

#[test]
fn a_long_choice_is_drawn_over_several_frames() {
    use crate::video::Cmd;
    // 30 glyphs (and a control code, which costs nothing): three frames of at most 12.
    static LONG: [&[u8]; 1] = [&{
        let mut t = [1u8; 32];
        t[5] = 0xFC;
        t[6] = 0x01;
        t[31] = 0xFF;
        t
    }];
    let mut m = Multichoice::new(&FONT, 0, &LONG, (0, 0), 0, Some(B_PRESSED));
    let pen = Pen::new(Colors::new(2, 3, 1));
    let maps = |f: &Frame| {
        (f.cmds.iter())
            .filter(|c| matches!(c, Cmd::MapEntries { .. }))
            .count()
    };
    let mut frames = Vec::new();
    while !m.is_drawn() {
        let mut f = Frame::default();
        m.draw_next(&FONT, &pen, GLYPHS_PER_FRAME, &mut f);
        frames.push(maps(&f));
        assert!(frames.len() <= 3);
    }
    assert_eq!(GLYPHS_PER_FRAME, 12);
    // The map rows (frame, choice, frame) come with the last glyphs only.
    assert_eq!(frames, [0, 0, 4]);
}
