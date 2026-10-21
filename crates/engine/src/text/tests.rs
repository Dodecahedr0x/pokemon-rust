use super::*;
use crate::data::{Behavior, Dir, GfxId, MapData, MapId, NewGame, ScriptId, SpriteData, TextId};
use crate::script::{Ctx, Script};
use alloc::boxed::Box;

const FG: u8 = 2;
const SHADOW: u8 = 3;
const BG: u8 = 1;
const A: Buttons = Buttons::A;
const NONE: Buttons = Buttons::NONE;

/// A `.latfont` glyph with the given (x, y, 2bpp value) pixels.
fn glyph(pixels: &[(usize, usize, u16)]) -> [u8; 64] {
    let mut g = [0u8; 64];
    for &(x, y, v) in pixels {
        let off = ((y / 8) * 2 + x / 8) * 16 + (y % 8) * 2;
        let w = u16::from_le_bytes([g[off], g[off + 1]]) | (v << (14 - 2 * (x % 8)));
        g[off..off + 2].copy_from_slice(&w.to_le_bytes());
    }
    g
}

/// Synthetic font: 1 = 3 px wide (fg, shadow, 3 on row 0), 2 = 10 px wide (fg at (0,0) and
/// (9,9)), 3 = 16 px wide (fg at (15,14) and (15,15)), 0xEF = cursor, 0x105 = escaped glyph.
fn font() -> &'static Font {
    let mut glyphs = vec![[0u8; 64]; 0x200];
    let mut widths = vec![6u8; 0x200];
    glyphs[1] = glyph(&[(0, 0, 1), (1, 0, 2), (2, 0, 3)]);
    widths[1] = 3;
    glyphs[2] = glyph(&[(0, 0, 1), (9, 9, 1)]);
    widths[2] = 10;
    glyphs[3] = glyph(&[(15, 14, 1), (15, 15, 1)]);
    widths[3] = 16;
    glyphs[0xEF] = glyph(&[(0, 0, 1)]);
    glyphs[0x105] = glyph(&[(0, 0, 1)]);
    widths[0x105] = 4;
    Box::leak(Box::new(Font {
        glyphs: Box::leak(glyphs.into_boxed_slice()),
        widths: Box::leak(widths.into_boxed_slice()),
    }))
}

fn window() -> &'static WindowGfx {
    let tiles: Vec<Tile> = (0..14u32).map(|i| [i; 8]).collect();
    Box::leak(Box::new(WindowGfx {
        tiles: Box::leak(tiles.into_boxed_slice()),
        palette: Box::leak(Box::new(core::array::from_fn(|i| i as u16))),
        text_fg: FG,
        text_shadow: SHADOW,
        text_bg: BG,
    }))
}

struct Game {
    font: &'static Font,
    window: &'static WindowGfx,
}

fn game() -> Game {
    Game {
        font: font(),
        window: window(),
    }
}

impl GameData for Game {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!()
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!()
    }
    fn font(&self) -> &'static Font {
        self.font
    }
    fn window(&self) -> &'static WindowGfx {
        self.window
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => &[1, 0xFF],
            UiText::No => &[2, 0xFF],
            UiText::RivalName => &[5, 0xFF],
        }
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[4, 4, 0xFF],
        }
    }
}

fn px(c: &Canvas, x: usize, y: usize) -> u8 {
    let t = c.tiles[(y / 8) * c.win.width as usize + x / 8];
    ((t[y % 8] >> ((x % 8) * 4)) & 0xF) as u8
}

fn leak(b: &[u8]) -> &'static [u8] {
    Box::leak(b.to_vec().into_boxed_slice())
}

/// A text box printing `text`.
fn printing(g: &Game, text: &[u8]) -> TextBox {
    let mut tb = TextBox::new();
    tb.print(g, leak(text), &mut Frame::default());
    tb
}

fn tick(tb: &mut TextBox, g: &Game, pressed: Buttons) -> Frame {
    let mut f = Frame::default();
    tb.update(g, pressed, pressed, &mut f);
    f
}

/// Updates until the printer leaves `Printing` (at most 1000 frames); returns the frame count.
fn run_page(tb: &mut TextBox, g: &Game) -> usize {
    let mut n = 0;
    while tb.state == State::Printing {
        tick(tb, g, NONE);
        n += 1;
        assert!(n < 1000);
    }
    n
}

#[test]
fn glyph_renders_into_tile_bytes() {
    let g = game();
    let mut tb = printing(&g, &[1, 0xFF]);
    let f = tick(&mut tb, &g, NONE);
    // Row y = 1 of tile 0: fg, shadow, bg (2bpp 3), then bg up to the canvas edge.
    let tile = f
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::BgTile {
                char_block: 2,
                index,
                tile,
            } if *index == MSG_WINDOW.base => Some(*tile),
            _ => None,
        })
        .expect("tile 0 uploaded");
    assert_eq!(tile[0], 0x1111_1111);
    assert_eq!(tile[1], 0x1111_1132);
    assert_eq!(tile[2], 0x1111_1111);
    // Only the changed tiles are uploaded: rows 1..16 span tile rows 0 and 1.
    let uploads: Vec<u16> = f
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::BgTile { index, .. } => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(uploads, [MSG_WINDOW.base, MSG_WINDOW.base + 27]);
}

#[test]
fn wide_glyph_uses_all_four_tiles() {
    let g = game();
    let mut tb = printing(&g, &[2, 3, 0xFF]);
    run_page(&mut tb, &g);
    assert_eq!(px(&tb.msg, 0, 1), FG);
    assert_eq!(px(&tb.msg, 9, 10), FG);
    // Glyph 3 at x = 10: row 14 is drawn, row 15 is not (15 rows per glyph).
    assert_eq!(px(&tb.msg, 25, 15), FG);
    assert_eq!(px(&tb.msg, 25, 16), BG);
}

#[test]
fn variable_width_layout() {
    let g = game();
    let mut tb = printing(&g, &[1, 1, 2, 1, 0xFF]);
    run_page(&mut tb, &g);
    for x in [0, 3, 6, 16] {
        assert_eq!(px(&tb.msg, x, 1), FG, "glyph at x = {x}");
    }
    assert_eq!(px(&tb.msg, 17, 1), SHADOW);
    assert_eq!(tb.x, 19);
}

#[test]
fn glyphs_clip_at_the_window_edge() {
    let g = game();
    let mut text = vec![3; 14]; // 14 * 16 = 224 > 216
    text.push(0xFF);
    let mut tb = printing(&g, &text);
    run_page(&mut tb, &g);
    assert_eq!(px(&tb.msg, 13 * 16 - 1, 15), FG);
    assert_eq!(tb.state, State::Idle);
}

#[test]
fn newline_moves_to_second_line() {
    let g = game();
    let mut tb = printing(&g, &[1, 1, 0xFE, 1, 0xFF]);
    run_page(&mut tb, &g);
    assert_eq!(px(&tb.msg, 0, 17), FG);
    assert_eq!(px(&tb.msg, 3, 17), BG);
}

#[test]
fn prompt_clear_waits_then_clears() {
    let g = game();
    let mut tb = printing(&g, &[1, 0xFB, 2, 0xFF]);
    run_page(&mut tb, &g);
    assert_eq!(tb.state, State::WaitClear);
    assert!(tb.is_printing());
    for _ in 0..10 {
        tick(&mut tb, &g, Buttons::UP);
    }
    assert_eq!(tb.state, State::WaitClear);
    assert_eq!(px(&tb.msg, 1, 1), SHADOW);
    tick(&mut tb, &g, Buttons::B);
    assert_eq!(px(&tb.msg, 1, 1), BG);
    assert_eq!(tb.state, State::Printing);
    tick(&mut tb, &g, NONE);
    // Glyph 2 at the origin: fg at (0, 1), nothing where glyph 1's shadow was.
    assert_eq!(px(&tb.msg, 0, 1), FG);
    assert_eq!(px(&tb.msg, 1, 1), BG);
    assert!(!tb.is_printing());
}

#[test]
fn prompt_scroll_waits_then_scrolls_one_line() {
    let g = game();
    let mut tb = printing(&g, &[2, 0xFE, 1, 0xFA, 2, 0xFF]);
    run_page(&mut tb, &g);
    assert_eq!(tb.state, State::WaitScroll);
    assert_eq!(px(&tb.msg, 0, 17), FG);
    tick(&mut tb, &g, NONE);
    assert_eq!(tb.state, State::WaitScroll);
    tick(&mut tb, &g, A);
    // 16 px at 2 px per frame.
    for i in 0..8 {
        assert_eq!(tb.state, State::Scrolling(16 - 2 * i));
        tick(&mut tb, &g, NONE);
    }
    assert_eq!(tb.state, State::Scrolling(0));
    // The second line moved to the first; the old first line is gone.
    assert_eq!(px(&tb.msg, 0, 1), FG);
    assert_eq!(px(&tb.msg, 1, 1), SHADOW);
    assert_eq!(px(&tb.msg, 9, 10), BG);
    assert_eq!(px(&tb.msg, 0, 17), BG);
    tick(&mut tb, &g, NONE);
    tick(&mut tb, &g, NONE);
    // The next glyph prints at the start of the second line.
    assert_eq!(px(&tb.msg, 0, 17), FG);
    assert_eq!(px(&tb.msg, 9, 26), FG);
    assert!(!tb.is_printing());
}

#[test]
fn placeholders_expand() {
    let g = game();
    // {PLAYER} {RIVAL} {STR_VAR_1} 1
    let out = expand(&g, &[0xFD, 0x01, 0xFD, 0x06, 0xFD, 0x02, 1, 0xFF, 9]);
    assert_eq!(out, [4, 4, 5, 1, 0xFF]);
    // Unterminated input still ends with 0xFF.
    assert_eq!(expand(&g, &[1]), [1, 0xFF]);
}

#[test]
fn code_arguments_are_never_text() {
    let g = game();
    // FC arguments and placeholder args that look like FD / FF / FE stay verbatim.
    let text = [
        0xFC, 0x0B, 0xFD, 0x01, 0xFC, 0x10, 0xFF, 0xFE, 0xF9, 0xFF, 1, 0xFF,
    ];
    assert_eq!(expand(&g, &text), text);
    let mut tb = printing(&g, &[0xFC, 0x0B, 0xFF, 0xFE, 0xF8, 0xFF, 1, 0xFF]);
    run_page(&mut tb, &g);
    assert_eq!(px(&tb.msg, 0, 1), FG);
    assert_eq!(tb.y, 1);
    assert_eq!(tb.x, 3);
}

#[test]
fn ext_codes() {
    let g = game();
    // {COLOR 5} 1 {COLOR_HIGHLIGHT_SHADOW 6 7 8} 1 {ESCAPE 5} {MIN_LETTER_SPACING 0} {JPN}
    let mut tb = printing(
        &g,
        &[
            0xFC, 0x01, 5, 1, 0xFC, 0x04, 6, 7, 8, 1, 0xFC, 0x0C, 0x05, 0xFC, 0x14, 0, 0xFC, 0x15,
            0xFF,
        ],
    );
    run_page(&mut tb, &g);
    assert_eq!(px(&tb.msg, 0, 1), 5);
    assert_eq!(px(&tb.msg, 1, 1), SHADOW);
    assert_eq!(px(&tb.msg, 3, 1), 6);
    assert_eq!(px(&tb.msg, 4, 1), 8);
    assert_eq!(px(&tb.msg, 5, 1), 7);
    assert_eq!(px(&tb.msg, 3, 2), 7, "highlight is the new background");
    // Escaped glyph 0x105 (4 px wide) at x = 6.
    assert_eq!(px(&tb.msg, 6, 1), 6);
    assert_eq!(tb.x, 10);
    assert!(!tb.is_printing());
}

#[test]
fn pause_codes() {
    let g = game();
    let mut tb = printing(&g, &[0xFC, 0x08, 3, 1, 0xFC, 0x09, 1, 0xFF]);
    tick(&mut tb, &g, NONE);
    assert_eq!(tb.state, State::Pause(3));
    let mut n = 0;
    while px(&tb.msg, 0, 1) != FG {
        tick(&mut tb, &g, NONE);
        n += 1;
    }
    // 3 frames counting down, 1 to resume, 1 to print.
    assert_eq!(n, 5);
    run_page(&mut tb, &g);
    assert_eq!(tb.state, State::WaitPress);
    tick(&mut tb, &g, A);
    tick(&mut tb, &g, NONE);
    assert_eq!(px(&tb.msg, 3, 1), FG);
    assert!(!tb.is_printing());
}

#[test]
fn four_frames_per_character() {
    let g = game();
    let mut tb = printing(&g, &[1, 1, 1, 0xFF]);
    let mut drawn = Vec::new();
    for frame in 0..20 {
        tick(&mut tb, &g, NONE);
        let x = tb.x;
        if drawn.last() != Some(&x) {
            drawn.push(x);
            assert_eq!(frame % 4, 0, "glyph on frame {frame}");
        }
        if !tb.is_printing() {
            // Complete as soon as the last character is drawn.
            assert_eq!(frame, 8);
            break;
        }
    }
    assert_eq!(drawn, [3, 6, 9]);
    assert!(!tb.is_printing());
}

#[test]
fn a_finishes_the_page() {
    let g = game();
    let mut tb = printing(&g, &[1, 1, 0xFE, 1, 0xFB, 1, 0xFF]);
    tick(&mut tb, &g, NONE);
    assert_eq!(tb.x, 3);
    tick(&mut tb, &g, A);
    // Whole page printed, stopped at the \p wait (the same press does not dismiss it).
    assert_eq!(tb.state, State::WaitClear);
    assert_eq!(px(&tb.msg, 3, 1), FG);
    assert_eq!(px(&tb.msg, 0, 17), FG);
    tick(&mut tb, &g, NONE);
    assert_eq!(tb.state, State::WaitClear);
    tick(&mut tb, &g, A);
    tick(&mut tb, &g, A);
    assert!(!tb.is_printing());
}

#[test]
fn frame_tile_layout() {
    let g = game();
    let mut tb = TextBox::new();
    let mut f = Frame::default();
    tb.open(&g, &mut f);
    assert!(tb.is_open());
    assert!(f.cmds.contains(&Cmd::BgPalette {
        bank: 15,
        colors: *g.window.palette
    }));
    assert!(f.cmds.contains(&Cmd::BgTiles {
        char_block: 2,
        first: FRAME_BASE,
        tiles: g.window.tiles
    }));
    assert!(f.cmds.contains(&Cmd::Bg {
        layer: 0,
        control: BgControl {
            enabled: true,
            priority: 0,
            char_block: 2,
            screen_block: 31
        }
    }));
    let (start, e) = f
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::MapEntries {
                screen_block: 31,
                start,
                entries,
            } => Some((*start, entries.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(start, 14 * 32);
    assert_eq!(e.len(), 6 * 32);
    let at = |x: usize, y: usize| e[(y - 14) * 32 + x];
    let fr = |t: u16| 0xF000 | (FRAME_BASE + t);
    let vf = |t: u16| fr(t) | 0x0800;
    // Top row.
    assert_eq!(at(0, 14), fr(1));
    assert_eq!(at(1, 14), fr(3));
    for x in 2..28 {
        assert_eq!(at(x, 14), fr(4));
    }
    assert_eq!(at(28, 14), fr(5));
    assert_eq!(at(29, 14), fr(6));
    assert_eq!(at(30, 14), 0);
    assert_eq!(at(31, 14), 0);
    // Sides and the canvas.
    for y in 15..19 {
        assert_eq!(at(0, y), fr(7));
        assert_eq!(at(1, y), fr(9));
        assert_eq!(at(29, y), fr(10));
        for x in 0..27 {
            assert_eq!(
                at(2 + x, y),
                0xF000 | (32 + (y as u16 - 15) * 27 + x as u16)
            );
        }
    }
    // Bottom row: the top row flipped vertically.
    assert_eq!(at(0, 19), vf(1));
    assert_eq!(at(1, 19), vf(3));
    assert_eq!(at(2, 19), vf(4));
    assert_eq!(at(27, 19), vf(4));
    assert_eq!(at(28, 19), vf(5));
    assert_eq!(at(29, 19), vf(6));
    // The empty canvas is uploaded.
    let uploads = f
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::BgTile { tile, .. } if *tile == [0x1111_1111; 8]))
        .count();
    assert_eq!(uploads, 108);
}

#[test]
fn yes_no_frame_position() {
    let e = YES_NO_WINDOW.frame_entries();
    assert_eq!(YES_NO_WINDOW.frame_start(), 8 * 32);
    let at = |x: usize, y: usize| e[(y - 8) * 32 + x];
    assert_eq!(at(18, 8), 0);
    assert_eq!(at(19, 8), 0xF000 | (FRAME_BASE + 1));
    assert_eq!(at(26, 8), 0xF000 | (FRAME_BASE + 6));
    assert_eq!(at(21, 9), 0xF000 | 160);
    assert_eq!(at(25, 12), 0xF000 | (160 + 19));
    assert_eq!(at(26, 13), 0xF800 | (FRAME_BASE + 6));
    assert_eq!(at(27, 13), 0);
}

#[test]
fn yes_no_cursor() {
    let g = game();
    let mut tb = TextBox::new();
    let mut f = Frame::default();
    tb.start_yes_no(&g, &mut f);
    assert!(tb.is_yes_no_active());
    assert_eq!(tb.yes_no_result(), None);
    // Labels at x = 8, cursor at x = 0 on the first line.
    assert_eq!(px(&tb.yes_no, 8, 1), FG);
    assert_eq!(px(&tb.yes_no, 8, 17), FG);
    assert_eq!(px(&tb.yes_no, 0, 1), FG);
    assert_eq!(px(&tb.yes_no, 0, 17), BG);
    tick(&mut tb, &g, Buttons::DOWN);
    assert_eq!(tb.yes_no_cursor, Some(1));
    assert_eq!(px(&tb.yes_no, 0, 1), BG);
    assert_eq!(px(&tb.yes_no, 0, 17), FG);
    // No wrap-around.
    tick(&mut tb, &g, Buttons::DOWN);
    assert_eq!(tb.yes_no_cursor, Some(1));
    tick(&mut tb, &g, Buttons::UP);
    tick(&mut tb, &g, Buttons::UP);
    assert_eq!(tb.yes_no_cursor, Some(0));
    let f = tick(&mut tb, &g, A);
    assert_eq!(tb.yes_no_result(), Some(true));
    assert!(!tb.is_yes_no_active());
    assert!(f.cmds.contains(&Cmd::MapEntries {
        screen_block: 31,
        start: 8 * 32,
        entries: vec![0; 6 * 32]
    }));

    tb.start_yes_no(&g, &mut Frame::default());
    tick(&mut tb, &g, Buttons::DOWN);
    tick(&mut tb, &g, A);
    assert_eq!(tb.yes_no_result(), Some(false));

    // B means no, wherever the cursor is.
    tb.start_yes_no(&g, &mut Frame::default());
    tick(&mut tb, &g, Buttons::B);
    assert_eq!(tb.yes_no_result(), Some(false));
}

#[test]
fn yes_no_keeps_the_message() {
    let g = game();
    let mut tb = printing(&g, &[1, 0xFF]);
    run_page(&mut tb, &g);
    tb.start_yes_no(&g, &mut Frame::default());
    tick(&mut tb, &g, A);
    assert!(tb.is_open());
    assert_eq!(px(&tb.msg, 0, 1), FG);
}

#[test]
fn close_disables_bg0() {
    let g = game();
    let mut tb = printing(&g, &[1, 0xFB, 1, 0xFF]);
    run_page(&mut tb, &g);
    let mut f = Frame::default();
    tb.close(&mut f);
    assert!(!tb.is_open());
    assert!(!tb.is_printing());
    assert!(f.cmds.contains(&Cmd::MapFill {
        screen_block: 31,
        entry: 0
    }));
    assert!(f.cmds.iter().any(|c| matches!(
        c,
        Cmd::Bg {
            layer: 0,
            control: BgControl { enabled: false, .. }
        }
    )));
    // Reopening reloads the graphics.
    let mut f = Frame::default();
    tb.open(&g, &mut f);
    assert!(f.cmds.iter().any(|c| matches!(c, Cmd::BgPalette { .. })));
}

#[test]
fn hash_tracks_state() {
    let g = game();
    let h = |tb: &TextBox| {
        let mut h = Fnv64::new();
        tb.hash(&mut h);
        h.finish()
    };
    let mut a = printing(&g, &[1, 1, 0xFF]);
    let mut b = printing(&g, &[1, 1, 0xFF]);
    assert_eq!(h(&a), h(&b));
    tick(&mut a, &g, NONE);
    assert_ne!(h(&a), h(&b));
    tick(&mut b, &g, NONE);
    assert_eq!(h(&a), h(&b));
    assert_ne!(h(&TextBox::new()), h(&a));
}
