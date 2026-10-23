use super::test_game::*;
use super::*;
use crate::battle::BattleMon;

const NONE: Buttons = Buttons::NONE;

fn input(pressed: Buttons) -> Input {
    Input {
        held: pressed,
        pressed,
    }
}

fn tick(w: &mut BattleWindows, g: &Game, pressed: Buttons) -> Frame {
    let mut f = Frame::default();
    w.update(g, &input(pressed), &mut f);
    f
}

fn loaded(g: &Game) -> BattleWindows {
    let mut w = BattleWindows::new();
    w.load(g, &mut Frame::default());
    w
}

fn player() -> BattleMon {
    mon(
        "TORCHY",
        TORCH,
        [Some(TACKLE), Some(GROWL), Some(EMBER), None],
        [35, 0, 6, 0],
    )
}

fn names_of<'a>(p: &'a BattleMon, o: &'a BattleMon) -> Names<'a> {
    Names {
        player: &[0xFF],
        trainer_class: &[0xFF],
        trainer_name: &[0xFF],
        wild: true,
        mons: [p, o],
        parties: [party(&[]), party(&[])],
    }
}

/// The TEXT BG rows 14..20 the last view redraw in `f` wrote.
fn text_rows(f: &Frame) -> Vec<u16> {
    f.cmds
        .iter()
        .rev()
        .find_map(|c| match c {
            Cmd::MapEntries {
                screen_block: 24,
                start,
                entries,
            } => {
                assert_eq!(*start, 14 * 32);
                Some(entries.clone())
            }
            _ => None,
        })
        .expect("no TEXT rows")
}

fn at(e: &[u16], col: usize, row: usize) -> u16 {
    e[(row - 14) * 32 + col]
}

/// The synthetic tilemap entry at (col, row) of the 64-row textbox map (see `test_game`).
fn map_at(col: usize, row: usize) -> u16 {
    ((row as u16) << 5 | col as u16) & 0x3FF | if row >= 34 { 0x1000 } else { 0 }
}

/// Asserts `w`'s canvas entries in `e` (tiles from `w.base()`, palette `pal`).
fn assert_window(e: &[u16], w: Window, pal: u16) {
    for y in 0..w.height() as usize {
        for x in 0..w.width() as usize {
            let tile = w.base() + (y * w.width() as usize + x) as u16;
            let (col, row) = (w.left() as usize + x, w.top() as usize + y);
            assert_eq!(at(e, col, row), tile | pal << 12, "{w:?} at ({col}, {row})");
        }
    }
}

#[test]
fn windows_fit_their_char_blocks_without_overlap() {
    let mut text = vec![
        MSG_WINDOW,
        ACTION_PROMPT_WINDOW,
        ACTION_MENU_WINDOW,
        PP_WINDOW,
        PP_REMAINING_WINDOW,
        MOVE_TYPE_WINDOW,
        LEARN_WINDOW,
    ];
    text.extend(MOVE_NAME_WINDOWS);
    text.sort_by_key(|w| w.base());
    let mut next = TEXTBOX_TILES as u16;
    for w in &text {
        assert!(w.base() >= next, "{w:?} overlaps");
        next = w.base() + w.width() as u16 * w.height() as u16;
        assert!(w.top() >= 15 && w.top() + w.height() <= 19, "{w:?} rows");
        assert!(w.left() + w.width() <= 30, "{w:?} columns");
    }
    assert!(next <= 512, "TEXT tiles past char block 0");
    let yn = YES_NO_WINDOW;
    assert!(yn.base() >= TEXTBOX_TILES as u16);
    assert!(
        yn.base() + (yn.width() * yn.height()) as u16 <= 256,
        "BG1 tiles into ANIM's"
    );
}

#[test]
fn load_uploads_gfx_and_draws_the_message_view() {
    let g = game();
    let mut w = BattleWindows::new();
    let mut f = Frame::default();
    w.load(&g, &mut f);
    let tb = &g.gfx.textbox;
    for (bank, colors) in [
        (0, tb.palettes[0]),
        (1, tb.palettes[1]),
        (5, tb.palettes[2]),
    ] {
        assert!(
            f.cmds.contains(&Cmd::BgPalette { bank, colors }),
            "bank {bank}"
        );
    }
    for (layer, char_block, screen_block, n) in [(0, 0, 24, 0x90), (1, 1, 28, 0x2C)] {
        assert!(f.cmds.contains(&Cmd::BgTiles {
            char_block,
            first: 0,
            tiles: &tb.tiles[..n]
        }));
        assert!(f.cmds.contains(&Cmd::MapFill {
            screen_block,
            entry: 0
        }));
        assert!(f.cmds.contains(&Cmd::Bg {
            layer,
            control: BgControl {
                enabled: true,
                priority: 0,
                char_block,
                screen_block
            }
        }));
    }
    let e = text_rows(&f);
    assert_eq!(e.len(), 6 * 32);
    // The frame from tilemap rows 14..20, the message canvas over rows 15..19, palette 0.
    for col in 0..32 {
        assert_eq!(at(&e, col, 14), map_at(col, 14));
        assert_eq!(at(&e, col, 19), map_at(col, 19));
    }
    assert_eq!(at(&e, 1, 15), map_at(1, 15));
    assert_eq!(at(&e, 28, 18), map_at(28, 18));
    assert_window(&e, MSG_WINDOW, 0);
    // The empty canvas (bg 15) is uploaded to char block 0.
    let n = f
        .cmds
        .iter()
        .filter(
            |c| matches!(c, Cmd::BgTile { char_block: 0, tile, .. } if *tile == [0xFFFF_FFFF; 8]),
        )
        .count();
    assert_eq!(n, 26 * 4);
}

#[test]
fn empty_gfx_loads_nothing_but_still_works() {
    let g = Game {
        gfx: &crate::battle::EMPTY_GFX,
        ..game()
    };
    let mut w = BattleWindows::new();
    let mut f = Frame::default();
    w.load(&g, &mut f);
    assert!(!f.cmds.iter().any(|c| matches!(c, Cmd::BgPalette { .. })));
    let e = text_rows(&f);
    assert_eq!(at(&e, 0, 14), 0);
    assert_window(&e, MSG_WINDOW, 0);
    w.print(&g, enc("A"), MsgWait::Frames(0), &mut Frame::default());
    tick(&mut w, &g, NONE);
    assert!(!w.is_printing());
}

#[test]
fn message_prints_then_waits_frames() {
    let g = game();
    let mut w = loaded(&g);
    w.print(&g, enc("AB"), MsgWait::Frames(3), &mut Frame::default());
    assert!(w.is_printing());
    // 'A' on update 1, 'B' on update 5 (4 frames per glyph), then 3 frames.
    for i in 1..=7 {
        tick(&mut w, &g, NONE);
        assert!(w.is_printing(), "update {i}");
    }
    tick(&mut w, &g, NONE);
    assert!(!w.is_printing());
    // Message colors: fg 1, shadow 6, bg 15 at (0, 1) of B_WIN_MSG.
    assert_eq!(w.msg.canvas().pixel(0, 1), 1);
    assert_eq!(w.msg.canvas().pixel(1, 1), 15);
}

#[test]
fn message_button_wait_needs_a_new_press() {
    let g = game();
    let mut w = loaded(&g);
    w.print(&g, enc("A"), MsgWait::Button, &mut Frame::default());
    tick(&mut w, &g, NONE);
    for _ in 0..10 {
        tick(&mut w, &g, NONE);
        assert!(w.is_printing());
    }
    tick(&mut w, &g, Buttons::B);
    assert!(!w.is_printing());
}

#[test]
fn page_prompt_waits_and_clears_as_the_field_printer() {
    let g = game();
    let mut w = loaded(&g);
    // "A\pB": the shared printer stops at \p until A, clears, then prints B.
    let mut text = t("A");
    text.push(0xFB);
    text.extend(t("B"));
    text.push(0xFF);
    w.print(&g, text, MsgWait::Frames(0), &mut Frame::default());
    tick(&mut w, &g, NONE);
    for _ in 0..20 {
        tick(&mut w, &g, NONE);
    }
    assert!(w.is_printing());
    assert_eq!(w.msg.canvas().pixel(0, 1), 1);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.msg.canvas().pixel(0, 1), 15, "cleared");
    tick(&mut w, &g, NONE);
    assert_eq!(w.msg.canvas().pixel(0, 1), 1, "B drawn");
    assert!(!w.is_printing());
}

#[test]
fn action_view_rows_and_cursor() {
    let g = game();
    let mut w = loaded(&g);
    let (p, o) = (player(), player());
    let mut f = Frame::default();
    w.action_menu(&g, &names_of(&p, &o), &mut f);
    assert!(w.is_menu_active());
    let e = text_rows(&f);
    // Frame from tilemap rows 34..40.
    assert_eq!(at(&e, 0, 14), map_at(0, 34));
    assert_eq!(at(&e, 15, 16), map_at(15, 36));
    assert_eq!(at(&e, 31, 19), map_at(31, 39));
    assert_window(&e, ACTION_PROMPT_WINDOW, 0);
    // The menu canvas, palette 5, except the cursor cells (cols 16 and 23).
    for row in 15..19 {
        for col in 17..29 {
            if col != 23 {
                let x = (col - 17) as u16;
                let y = (row - 15) as u16;
                assert_eq!(at(&e, col, row), 0x5000 | (0x130 + y * 12 + x));
            }
        }
    }
    assert_eq!(at(&e, 16, 15), 1);
    assert_eq!(at(&e, 16, 16), 2);
    for (col, row) in [(23, 15), (23, 16), (16, 17), (16, 18), (23, 17), (23, 18)] {
        assert_eq!(at(&e, col, row), 0x1016);
    }
    // The prompt is "What will\nTORCHY do?" in message colors at x 1, y 1.
    assert_eq!(w.prompt.pixel(1, 1), 1);
    assert_eq!(w.prompt.pixel(1, 17), 1);
    assert_eq!(w.prompt.pixel(0, 1), 15);
    // The menu text: FIGHT at x 0, BAG at x 56 (CLEAR_TO), menu colors.
    assert_eq!(w.action.pixel(0, 1), 13);
    assert_eq!(w.action.pixel(56, 1), 13);
    assert_eq!(w.action.pixel(55, 1), 14);
    assert_eq!(w.action.pixel(56, 17), 13);
}

#[test]
fn action_cursor_moves_as_handle_input_choose_action() {
    let g = game();
    let mut w = loaded(&g);
    let (p, o) = (player(), player());
    w.action_menu(&g, &names_of(&p, &o), &mut Frame::default());
    // Left / up at the top left: nothing, and no redraw.
    for b in [Buttons::LEFT, Buttons::UP, Buttons::B, NONE] {
        let f = tick(&mut w, &g, b);
        assert_eq!(w.action_cursor, 0);
        assert!(f.cmds.is_empty(), "{b:?}");
    }
    let steps = [
        (Buttons::RIGHT, 1),
        (Buttons::RIGHT, 1),
        (Buttons::DOWN, 3),
        (Buttons::DOWN, 3),
        (Buttons::LEFT, 2),
        (Buttons::UP, 0),
        (Buttons::DOWN, 2),
    ];
    for (b, want) in steps {
        let before = w.action_cursor;
        let f = tick(&mut w, &g, b);
        assert_eq!(w.action_cursor, want, "{b:?}");
        if before == want {
            assert!(f.cmds.is_empty());
            continue;
        }
        let e = text_rows(&f);
        let (col, row) = (7 * (want as usize & 1) + 16, 15 + (want as usize & 2));
        assert_eq!(at(&e, col, row), 1);
        assert_eq!(at(&e, col, row + 1), 2);
    }
    assert_eq!(w.action_result(), None);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.action_result(), Some(Action::Pokemon));
    assert!(!w.is_menu_active());
    // The cursor is kept for the next turn.
    let mut f = Frame::default();
    w.action_menu(&g, &names_of(&p, &o), &mut f);
    assert_eq!(w.action_result(), None);
    assert_eq!(at(&text_rows(&f), 16, 17), 1);
    tick(&mut w, &g, Buttons::RIGHT);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.action_result(), Some(Action::Run));
}

#[test]
fn move_view_rows_pp_and_type() {
    let g = game();
    let mut w = loaded(&g);
    let p = player();
    let mut f = Frame::default();
    w.move_menu(&g, &p, &mut f);
    let e = text_rows(&f);
    assert_eq!(at(&e, 0, 14), map_at(0, 54));
    assert_eq!(at(&e, 20, 15), map_at(20, 55));
    assert_eq!(at(&e, 31, 19), map_at(31, 59));
    for win in MOVE_NAME_WINDOWS {
        assert_window(&e, win, 5);
    }
    assert_window(&e, PP_WINDOW, 5);
    assert_window(&e, PP_REMAINING_WINDOW, 5);
    assert_window(&e, MOVE_TYPE_WINDOW, 5);
    assert_eq!(at(&e, 1, 15), 1);
    assert_eq!(at(&e, 1, 16), 2);
    for (col, row) in [(10, 15), (10, 16), (1, 17), (1, 18), (10, 17), (10, 18)] {
        assert_eq!(at(&e, col, row), 0x1016);
    }
    // Names in menu colors; "PP " and "35/35" in the PP colors (fg 12, bg 14).
    assert_eq!(w.names[0].pixel(0, 1), 13);
    assert_eq!(w.names[3].pixel(0, 1), 13, "MOVE_NONE's name");
    assert_eq!(w.pp_label.pixel(0, 1), 12);
    assert_eq!(w.pp.pixel(2, 1), 12);
    assert_eq!(w.pp.pixel(1, 1), 14);
    assert_eq!(w.move_type.pixel(0, 1), 13);
    // PP palette: state 3 (full) -> gPPTextPalette[6], [7] into colors 12 and 11.
    let mut pal = g.gfx.textbox.palettes[2];
    pal[12] = 0x706;
    pal[11] = 0x707;
    assert!(f.cmds.contains(&Cmd::BgPalette {
        bank: 5,
        colors: pal
    }));
}

/// The text a canvas row would hold: which glyph columns start at y = 1 (fg pixels).
fn ink(c: &Canvas) -> Vec<i16> {
    let w = c.window().width() as i16 * 8;
    (0..w)
        .filter(|&x| c.pixel(x, 1) == 13 || c.pixel(x, 1) == 12)
        .collect()
}

#[test]
fn start_toggles_mega_evolution_when_available() {
    let g = game();
    let mut w = loaded(&g);
    let p = player();
    let hash = |w: &BattleWindows| {
        let mut h = crate::hash::Fnv64::new();
        w.hash(&mut h);
        h.finish()
    };
    // Unavailable: START does nothing, nothing extra is hashed.
    w.move_menu(&g, &p, &mut Frame::default());
    let plain = (ink(&w.move_type), hash(&w));
    tick(&mut w, &g, Buttons::START);
    assert!(!w.mega_armed());
    assert_eq!((ink(&w.move_type), hash(&w)), plain);
    // Available: START arms it (the type window says so), again disarms it.
    w.move_menu_mega(&g, &p, true, &mut Frame::default());
    assert_ne!(hash(&w), plain.1);
    tick(&mut w, &g, Buttons::START);
    assert!(w.mega_armed());
    assert_ne!(ink(&w.move_type), plain.0);
    tick(&mut w, &g, Buttons::START);
    assert!(!w.mega_armed());
    assert_eq!(ink(&w.move_type), plain.0);
    tick(&mut w, &g, Buttons::START);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.move_result(), Some(MoveChoice::Move(0)));
    assert!(w.mega_armed());
    // The next menu without a Mega forgets it.
    w.move_menu(&g, &p, &mut Frame::default());
    assert!(!w.mega_armed());
}

#[test]
fn move_cursor_respects_the_move_count_and_redraws_pp_and_type() {
    let g = game();
    let mut w = loaded(&g);
    let p = player(); // 3 moves: slot 3 is empty
    w.move_menu(&g, &p, &mut Frame::default());
    let type_normal = ink(&w.move_type);
    let f = tick(&mut w, &g, Buttons::RIGHT);
    assert_eq!(w.move_cursor, 1);
    // GROWL: 0/40 -> state 2 (empty): gPPTextPalette[4], [5].
    let mut pal = g.gfx.textbox.palettes[2];
    pal[12] = 0x704;
    pal[11] = 0x705;
    assert!(f.cmds.contains(&Cmd::BgPalette {
        bank: 5,
        colors: pal
    }));
    // Down from slot 1 would reach slot 3, which is empty.
    tick(&mut w, &g, Buttons::DOWN);
    assert_eq!(w.move_cursor, 1);
    tick(&mut w, &g, Buttons::LEFT);
    tick(&mut w, &g, Buttons::DOWN);
    assert_eq!(w.move_cursor, 2);
    assert_ne!(ink(&w.move_type), type_normal, "EMBER is FIRE");
    tick(&mut w, &g, Buttons::RIGHT);
    assert_eq!(w.move_cursor, 2);
    tick(&mut w, &g, Buttons::UP);
    assert_eq!(w.move_cursor, 0);
    // A move without PP can still be chosen (the core says NoPPLeft).
    tick(&mut w, &g, Buttons::RIGHT);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.move_result(), Some(MoveChoice::Move(1)));
    assert!(!w.is_menu_active());
    w.move_menu(&g, &p, &mut Frame::default());
    assert_eq!(w.move_result(), None);
    assert_eq!(w.move_cursor, 1, "kept between turns");
    tick(&mut w, &g, Buttons::B);
    assert_eq!(w.move_result(), Some(MoveChoice::Back));
    // A mon whose slot under the cursor is empty resets it.
    let one = mon("X", TORCH, [Some(TACKLE), None, None, None], [1, 0, 0, 0]);
    w.move_menu(&g, &one, &mut Frame::default());
    assert_eq!(w.move_cursor, 0);
    for b in [Buttons::RIGHT, Buttons::DOWN] {
        tick(&mut w, &g, b);
        assert_eq!(w.move_cursor, 0);
    }
}

#[test]
fn pp_color_thresholds() {
    // GetCurrentPpToMaxPpState, by hand from battle_message.c.
    let cases = [
        // (cur, max, state)
        (35, 35, 3),
        (0, 0, 3),
        (2, 2, 3),
        (1, 2, 1),
        (0, 2, 2),
        (1, 1, 3),
        (0, 1, 2),
        (5, 5, 3),
        (3, 5, 3),
        (2, 5, 0),
        (1, 5, 1),
        (0, 5, 2),
        (7, 7, 3),
        (3, 7, 3),
        (2, 7, 0),
        (20, 20, 3),
        (11, 20, 3),
        (10, 20, 0),
        (6, 20, 0),
        (5, 20, 1),
        (1, 20, 1),
        (0, 20, 2),
        (8, 8, 3),
        (5, 8, 3),
        (4, 8, 0),
        (3, 8, 0),
        (2, 8, 1),
        (0, 8, 2),
    ];
    for (cur, max, want) in cases {
        assert_eq!(pp_state(cur, max), want, "{cur}/{max}");
    }
}

#[test]
fn pp_numbers_are_right_aligned() {
    let mut v = Vec::new();
    push_2_digits(&mut v, 5);
    push_2_digits(&mut v, 35);
    push_2_digits(&mut v, 0);
    assert_eq!(v, [0x77, 0xA6, 0xA4, 0xA6, 0x77, 0xA1]);
}

#[test]
fn learn_move_view_has_a_fifth_entry() {
    let g = game();
    let mut w = loaded(&g);
    let p = mon(
        "TORCHY",
        TORCH,
        [Some(TACKLE), Some(GROWL), Some(EMBER), Some(TACKLE)],
        [1, 1, 1, 1],
    );
    let mut f = Frame::default();
    w.learn_move_menu(&g, &p, EMBER, &mut f);
    let e = text_rows(&f);
    assert_window(&e, LEARN_WINDOW, 5);
    assert_window(&e, MOVE_TYPE_WINDOW, 5);
    assert_eq!(at(&e, 21, 15), 0x1016);
    assert_eq!(at(&e, 21, 16), 0x1016);
    assert!(
        !f.cmds.iter().any(|c| matches!(c, Cmd::BgPalette { .. })),
        "no PP"
    );
    assert_eq!(w.learn.pixel(0, 1), 13);
    // Right of slot 0 is slot 1, right of slot 1 the new move; left comes back to slot 1.
    tick(&mut w, &g, Buttons::RIGHT);
    let f = tick(&mut w, &g, Buttons::RIGHT);
    assert_eq!(w.move_cursor, 4);
    let e = text_rows(&f);
    assert_eq!(at(&e, 21, 15), 1);
    assert_eq!(at(&e, 21, 16), 2);
    assert_eq!(at(&e, 10, 15), 0x1016);
    for b in [Buttons::UP, Buttons::DOWN, Buttons::RIGHT] {
        tick(&mut w, &g, b);
        assert_eq!(w.move_cursor, 4);
    }
    tick(&mut w, &g, Buttons::LEFT);
    assert_eq!(w.move_cursor, 1);
    tick(&mut w, &g, Buttons::DOWN);
    tick(&mut w, &g, Buttons::RIGHT);
    assert_eq!(w.move_cursor, 4, "from slot 3 too");
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.move_result(), Some(MoveChoice::Back));
    w.learn_move_menu(&g, &p, EMBER, &mut Frame::default());
    assert_eq!(w.move_cursor, 4);
    tick(&mut w, &g, Buttons::LEFT);
    tick(&mut w, &g, Buttons::LEFT);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.move_result(), Some(MoveChoice::Move(0)));
    w.learn_move_menu(&g, &p, EMBER, &mut Frame::default());
    tick(&mut w, &g, Buttons::B);
    assert_eq!(w.move_result(), Some(MoveChoice::Back));
    // Back to a plain move menu: the fifth entry is gone.
    w.move_menu(&g, &p, &mut Frame::default());
    assert!(w.move_cursor < 4);
}

fn bg1_rows(f: &Frame) -> Option<Vec<u16>> {
    f.cmds.iter().rev().find_map(|c| match c {
        Cmd::MapEntries {
            screen_block: 28,
            start,
            entries,
        } => {
            assert_eq!(*start, 8 * 32);
            Some(entries.clone())
        }
        _ => None,
    })
}

#[test]
fn yes_no_box_on_bg1() {
    let g = game();
    let mut w = loaded(&g);
    w.print(&g, enc("A"), MsgWait::Frames(0), &mut Frame::default());
    tick(&mut w, &g, NONE);
    let mut f = Frame::default();
    w.yes_no(&g, &mut f);
    assert!(w.is_menu_active());
    assert!(f.cmds.iter().all(|c| !matches!(
        c,
        Cmd::MapEntries {
            screen_block: 24,
            ..
        }
    )));
    let e = bg1_rows(&f).unwrap();
    let at = |x: usize, y: usize| e[(y - 8) * 32 + x];
    assert_eq!(at(23, 8), 0);
    assert_eq!(at(24, 8), 0x1022);
    assert_eq!(at(27, 8), 0x1023);
    assert_eq!(at(29, 8), 0x1024);
    assert_eq!(at(24, 10), 0x1025);
    assert_eq!(at(29, 12), 0x1027);
    assert_eq!(at(24, 13), 0x1028);
    assert_eq!(at(26, 13), 0x1029);
    assert_eq!(at(29, 13), 0x102A);
    assert_eq!(at(30, 10), 0);
    // B_WIN_YESNO at (26, 9), 3x4, palette 5, tiles from 0x90 in char block 1.
    for y in 0..4 {
        for x in 0..3 {
            assert_eq!(at(26 + x, 9 + y), 0x5000 | (0x90 + y as u16 * 3 + x as u16));
        }
    }
    assert_eq!((at(25, 9), at(25, 10)), (1, 2));
    assert_eq!((at(25, 11), at(25, 12)), (0x1016, 0x1016));
    assert!(f.cmds.iter().any(|c| matches!(
        c,
        Cmd::BgTile {
            char_block: 1,
            index: 0x90,
            ..
        }
    )));
    // YES/NO in its own colors (13 on 14).
    assert_eq!(w.yes_no.pixel(0, 1), 13);
    assert_eq!(w.yes_no.pixel(0, 17), 13);
    // Up at yes: nothing; down: cursor on no; down again: nothing.
    assert!(bg1_rows(&tick(&mut w, &g, Buttons::UP)).is_none());
    let e = bg1_rows(&tick(&mut w, &g, Buttons::DOWN)).unwrap();
    assert_eq!(e[3 * 32 + 25], 1);
    assert_eq!(e[32 + 25], 0x1016);
    assert!(bg1_rows(&tick(&mut w, &g, Buttons::DOWN)).is_none());
    let f = tick(&mut w, &g, Buttons::A);
    assert_eq!(w.yes_no_result(), Some(false));
    assert!(!w.is_menu_active());
    assert_eq!(bg1_rows(&f).unwrap(), vec![0; 6 * 32]);
    // A on yes; B is no.
    w.yes_no(&g, &mut Frame::default());
    assert_eq!(w.yes_no_result(), None);
    tick(&mut w, &g, Buttons::A);
    assert_eq!(w.yes_no_result(), Some(true));
    w.yes_no(&g, &mut Frame::default());
    tick(&mut w, &g, Buttons::B);
    assert_eq!(w.yes_no_result(), Some(false));
}

#[test]
fn print_returns_to_the_message_view() {
    let g = game();
    let mut w = loaded(&g);
    let p = player();
    w.move_menu(&g, &p, &mut Frame::default());
    tick(&mut w, &g, Buttons::A);
    let mut f = Frame::default();
    w.print(&g, enc("A"), MsgWait::Frames(0), &mut f);
    let e = text_rows(&f);
    assert_eq!(at(&e, 0, 14), map_at(0, 14));
    assert_window(&e, MSG_WINDOW, 0);
    // Printing again in the message view does not redraw the rows.
    let mut f = Frame::default();
    w.print(&g, enc("B"), MsgWait::Frames(0), &mut f);
    assert!(!f.cmds.iter().any(|c| matches!(c, Cmd::MapEntries { .. })));
}

#[test]
fn per_frame_uploads_stay_small() {
    let g = game();
    let mut w = loaded(&g);
    let (p, o) = (player(), player());
    let budget = |f: &Frame| {
        assert!(
            !f.cmds
                .iter()
                .any(|c| matches!(c, Cmd::BgTiles { .. } | Cmd::MapFill { .. })),
            "bulk upload while visible"
        );
        f.cmds.len()
    };
    w.print(&g, enc("ABCDEF"), MsgWait::Button, &mut Frame::default());
    for _ in 0..30 {
        assert!(budget(&tick(&mut w, &g, NONE)) <= 4);
    }
    let mut f = Frame::default();
    w.action_menu(&g, &names_of(&p, &o), &mut f);
    budget(&f);
    assert!(budget(&tick(&mut w, &g, Buttons::RIGHT)) <= 1);
    let mut f = Frame::default();
    w.move_menu(&g, &p, &mut f);
    budget(&f);
    // A cursor move: the rows, the PP and type tiles that changed and the palette.
    let n = budget(&tick(&mut w, &g, Buttons::RIGHT));
    assert!(n <= 2 + 8 + 16, "{n}");
}

#[test]
fn hash_tracks_state() {
    let g = game();
    let h = |w: &BattleWindows| {
        let mut h = Fnv64::new();
        w.hash(&mut h);
        h.finish()
    };
    let mut a = loaded(&g);
    let mut b = loaded(&g);
    assert_eq!(h(&a), h(&b));
    assert_ne!(h(&a), h(&BattleWindows::new()));
    let p = player();
    a.move_menu(&g, &p, &mut Frame::default());
    b.move_menu(&g, &p, &mut Frame::default());
    assert_eq!(h(&a), h(&b));
    tick(&mut a, &g, Buttons::RIGHT);
    assert_ne!(h(&a), h(&b));
    tick(&mut b, &g, Buttons::RIGHT);
    assert_eq!(h(&a), h(&b));
    a.print(&g, enc("AB"), MsgWait::Frames(2), &mut Frame::default());
    b.print(&g, enc("AB"), MsgWait::Frames(3), &mut Frame::default());
    assert_ne!(h(&a), h(&b));
}

#[test]
fn leaving_an_unanswered_yes_no_erases_the_box() {
    let g = game();
    let (p, o) = (player(), player());
    let mut w = loaded(&g);
    // Each way out of the yes/no menu without an answer clears BG1 rows 8..14.
    for way in 0..3 {
        w.yes_no(&g, &mut Frame::default());
        let mut f = Frame::default();
        match way {
            0 => w.print(&g, enc("A"), MsgWait::Frames(0), &mut f),
            1 => w.action_menu(&g, &names_of(&p, &o), &mut f),
            _ => w.move_menu(&g, &p, &mut f),
        }
        assert_eq!(bg1_rows(&f), Some(vec![0; 6 * 32]), "way {way}");
        assert_eq!(w.yes_no_result(), None);
        if way == 0 {
            tick(&mut w, &g, NONE);
        }
    }
    // Leaving another menu does not touch BG1.
    let mut f = Frame::default();
    w.action_menu(&g, &names_of(&p, &o), &mut f);
    assert_eq!(bg1_rows(&f), None);
}

#[test]
fn opening_a_menu_drops_a_pending_wait() {
    let g = game();
    let p = player();
    let mut w = loaded(&g);
    w.print(&g, enc("A"), MsgWait::Frames(50), &mut Frame::default());
    tick(&mut w, &g, NONE);
    assert!(w.is_printing());
    w.move_menu(&g, &p, &mut Frame::default());
    assert!(!w.is_printing());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "still printing")]
fn opening_a_menu_mid_message_is_a_bug() {
    let g = game();
    let p = player();
    let mut w = loaded(&g);
    w.print(&g, enc("ABC"), MsgWait::Frames(0), &mut Frame::default());
    tick(&mut w, &g, NONE);
    w.move_menu(&g, &p, &mut Frame::default());
}
