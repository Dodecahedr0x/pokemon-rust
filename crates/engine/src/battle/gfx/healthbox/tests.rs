use super::super::test_gfx::*;
use super::*;
use crate::data::{MoveId, SpeciesId};

fn mon(hp: u16, max_hp: u16) -> BattleMon {
    let mut nickname = [EOS; NAME_LEN];
    nickname[..3].copy_from_slice(&[0xBB, 0xBC, 0xBD]);
    BattleMon {
        party: 0,
        species: SpeciesId(1),
        nickname,
        level: 5,
        hp,
        max_hp,
        status: Status::None,
        gender: Gender::Male,
        moves: [Some(MoveId(1)), None, None, None],
        pp: [35, 0, 0, 0],
        max_pp: [35, 0, 0, 0],
    }
}

fn obj_tiles(f: &Frame) -> Vec<u16> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::ObjTile { index, .. } => Some(*index),
            _ => None,
        })
        .collect()
}

#[test]
fn glyph_rows_match_the_pixel_model() {
    // The Gen3 per-pixel copy, as the model of the word-at-a-time `Window::glyph`.
    fn model(px: &mut [[u8; 64]; 16], g: &[u8; 64], w: u8, x: usize, y: usize, fg: u8) {
        for gy in 0..GLYPH_HEIGHT {
            for gx in 0..(w as usize).min(16) {
                let off = ((gy / 8) * 2 + gx / 8) * 16 + (gy % 8) * 2;
                let word = u16::from_le_bytes([g[off], g[off + 1]]);
                let c = match (word >> (14 - 2 * (gx % 8))) & 3 {
                    1 => fg,
                    2 => COLOR_SHADOW,
                    _ => COLOR_BG,
                };
                if x + gx < 64 && y + gy < 16 {
                    px[y + gy][x + gx] = c;
                }
            }
        }
    }
    let mut seed = 0x9e37_79b9u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as u8
    };
    let glyphs: Vec<[u8; 64]> = (0..17).map(|_| core::array::from_fn(|_| next())).collect();
    let font = alloc::boxed::Box::leak(alloc::boxed::Box::new(Font {
        glyphs: alloc::boxed::Box::leak(glyphs.clone().into_boxed_slice()),
        widths: alloc::boxed::Box::leak((0..17u8).collect::<Vec<_>>().into_boxed_slice()),
    }));
    for id in 0..17u16 {
        for (x, y) in [(0, 0), (3, 3), (5, 5), (13, 2), (50, 4), (60, 3)] {
            let mut w = Window::new();
            let mut want = [[COLOR_BG; 64]; 16];
            w.glyph(font, id, x, y, COLOR_MALE);
            model(&mut want, &glyphs[id as usize], id as u8, x, y, COLOR_MALE);
            for (py, row) in want.iter().enumerate() {
                for (px, &c) in row.iter().enumerate() {
                    let got = (w.0[(py / 8) * 8 + px / 8][py % 8] >> ((px % 8) * 4)) & 0xF;
                    assert_eq!(got as u8, c, "glyph {id} at ({x}, {y}): ({px}, {py})");
                }
            }
        }
    }
}

#[test]
fn load_uploads_base_palettes_and_elements() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Player, gfx);
    let mut f = Frame::default();
    hb.load(&mon(20, 20), &mut f);
    assert_eq!(
        f.cmds[0],
        Cmd::ObjTiles {
            first: obj::PLAYER_BOX,
            tiles: gfx.healthbox_player.tiles,
        }
    );
    assert!(f.cmds.contains(&Cmd::ObjPalette {
        bank: obj::PAL_HP_BAR,
        colors: gfx.hpbar.palettes[0],
    }));
    assert!(f.cmds.contains(&Cmd::ObjPalette {
        bank: obj::PAL_PLAYER_BOX,
        colors: gfx.healthbox_player.palettes[0],
    }));
    let tiles = obj_tiles(&f);
    // "HP" label in the bar sprite.
    assert!(tiles.contains(&obj::PLAYER_BAR) && tiles.contains(&(obj::PLAYER_BAR + 1)));
    // Nickname (tiles 2..8, 10..16, 64, 72), level (65..68, 73..76), HP (31, 88..92) and
    // the blank status icon (26..29).
    for t in [2, 7, 10, 15, 64, 72, 65, 73, 31, 88, 89, 90, 91, 26, 27, 28] {
        assert!(tiles.contains(&(obj::PLAYER_BOX + t)), "tile {t}");
    }
    assert_eq!(hb.pending(), 0);
    assert!(!hb.visible());
    assert!(f.validate().is_ok());
    // Nothing more to send.
    let mut f = Frame::default();
    hb.update(&mut f);
    assert!(f.cmds.is_empty() && f.objs.is_empty());
}

#[test]
fn text_goes_into_the_last_rows_of_the_top_tiles() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Opponent, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    // Box tile 1 (first nickname tile): rows 0..5 keep the base graphic, rows 5..8 are the
    // window's rows 5..8. Glyph fg at (0, 3) lands at window row 3 + 3 = 6.
    let base = tile(TAG_BOX_ENEMY, 1);
    let t = hb.tiles[1];
    assert_eq!(t[..5], base[..5]);
    assert_eq!(t[6] & 0xF, COLOR_FG as u32);
    assert_eq!(t[5], 0x2222_2222);
    // Shadow at glyph (1, 12) -> window row 15 -> bottom tile 9, row 7, pixel 1.
    assert_eq!((hb.tiles[9][7] >> 4) & 0xF, COLOR_SHADOW as u32);
    // Gender glyph after 3 glyphs of 5 px: x 15 -> tile 2 (8..16), pixel 7, in the male
    // color.
    assert_eq!((hb.tiles[2][6] >> 28) & 0xF, COLOR_MALE as u32);
}

#[test]
fn hp_change_uploads_only_changed_tiles() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Player, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    // "␣20/" -> "␣19/" at x 4: spacer 4..9, digits at 9 and 14, "/" at 19. The synthetic
    // digit glyphs differ at (d % 4, 6) and (4, 4 + d % 8): "2" -> "1" changes x 10..12 and
    // 13 (window tile 9), "0" -> "9" changes x 14..15 (tile 9) and the shadow at x 18
    // (tile 10). Window tiles 9 and 10 go to box tiles 88 and 89; tile 31 (x 0..8) stays.
    hb.set_hp(19);
    let mut f = Frame::default();
    hb.update(&mut f);
    let mut tiles = obj_tiles(&f);
    tiles.sort();
    assert_eq!(tiles, [obj::PLAYER_BOX + 88, obj::PLAYER_BOX + 89]);
    // Same value again: nothing.
    hb.set_hp(19);
    let mut f = Frame::default();
    hb.update(&mut f);
    assert!(obj_tiles(&f).is_empty());
    // The opponent shows no numbers.
    let mut opp = Healthbox::new(Side::Opponent, gfx);
    opp.load(&mon(20, 20), &mut Frame::default());
    opp.set_hp(3);
    let mut f = Frame::default();
    opp.update(&mut f);
    assert!(obj_tiles(&f).is_empty());
}

#[test]
fn full_redraw_is_spread_over_frames() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Player, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    let mut other = mon(123, 200);
    other.nickname = [0xC0; NAME_LEN];
    other.level = 42;
    hb.set_mon(&other);
    let pending = hb.pending();
    assert!(pending as usize > HEALTHBOX_TILES_PER_FRAME);
    let mut sent = 0;
    while hb.pending() > 0 {
        let mut f = Frame::default();
        hb.update(&mut f);
        let n = obj_tiles(&f).len();
        assert!(n <= HEALTHBOX_TILES_PER_FRAME);
        sent += n;
    }
    assert_eq!(sent as u32, pending);
}

#[test]
fn status_icon_palette_and_opponent_label() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Opponent, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    hb.set_status(Status::Burn);
    let mut f = Frame::default();
    hb.update(&mut f);
    // BRN of the opponent set (status2): 15 + 4 * 3.
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::ENEMY_BOX + 0x11,
        tile: tile(TAG_STATUS, 27),
    }));
    let mut pal = gfx.healthbox_opponent.palettes[0];
    pal[13] = rgb(28, 14, 10);
    assert!(f.cmds.contains(&Cmd::ObjPalette {
        bank: obj::PAL_ENEMY_BOX,
        colors: pal,
    }));
    // The "HP" label becomes black + frame end.
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::ENEMY_BAR,
        tile: tile(TAG_HPBAR, 0),
    }));
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::ENEMY_BAR + 1,
        tile: tile(TAG_MISC, 11),
    }));
    // Cured: blank icon, "HP" again.
    hb.set_status(Status::None);
    let mut f = Frame::default();
    hb.update(&mut f);
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::ENEMY_BOX + 0x11,
        tile: tile(TAG_MISC, 3),
    }));
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::ENEMY_BAR,
        tile: tile(TAG_HPBAR, 1),
    }));
    // The player's sleep icon: first set, color 12.
    let mut p = Healthbox::new(Side::Player, gfx);
    p.load(&mon(20, 20), &mut Frame::default());
    p.set_status(Status::Sleep(3));
    let mut f = Frame::default();
    p.update(&mut f);
    assert!(f.cmds.contains(&Cmd::ObjTile {
        index: obj::PLAYER_BOX + 0x1A,
        tile: tile(TAG_STATUS, 6),
    }));
    assert!(!obj_tiles(&f).contains(&obj::PLAYER_BAR));
}

#[test]
fn placement_and_slide_in() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Player, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    hb.slide_in();
    let mut frames = 0;
    let mut first = None;
    while hb.busy() {
        let mut f = Frame::default();
        hb.update(&mut f);
        first.get_or_insert(f.objs[2].x);
        frames += 1;
    }
    // 0x73 / 5 = 23 frames; the first frame already moved 5 px.
    assert_eq!(frames, 23);
    assert_eq!(first, Some(126 + 110));
    let mut f = Frame::default();
    hb.update(&mut f);
    let o = &f.objs;
    assert_eq!(o.len(), 4);
    // Bar subsprites at the bar centre (174, 88) - 16 and + 16.
    assert_eq!((o[0].x, o[0].y, o[0].tile), (158, 88, obj::PLAYER_BAR));
    assert_eq!((o[1].x, o[1].y, o[1].tile), (190, 88, obj::PLAYER_BAR + 4));
    assert_eq!(
        (o[0].shape, o[0].size, o[0].palette),
        (ObjShape::Wide, 1, obj::PAL_HP_BAR)
    );
    // Box halves: centre (158, 88) with the 64x32 template's corner vector, 64x64 each.
    assert_eq!((o[2].x, o[2].y, o[2].tile), (126, 72, obj::PLAYER_BOX));
    assert_eq!((o[3].x, o[3].y, o[3].tile), (190, 72, obj::PLAYER_BOX + 64));
    assert_eq!((o[2].shape, o[2].size), (ObjShape::Square, 3));
    assert!(o.iter().all(|o| o.priority == OBJ_PRIORITY_BOX));

    let mut opp = Healthbox::new(Side::Opponent, gfx);
    opp.load(&mon(20, 20), &mut Frame::default());
    opp.show();
    let mut f = Frame::default();
    opp.update(&mut f);
    let o = &f.objs;
    assert_eq!((o[0].x, o[0].y), (36, 30));
    assert_eq!((o[1].x, o[1].y), (68, 30));
    assert_eq!((o[2].x, o[2].y, o[2].tile), (12, 14, obj::ENEMY_BOX));
    assert_eq!((o[3].x, o[3].y, o[3].tile), (76, 14, obj::ENEMY_BOX + 32));
    assert_eq!(
        (o[2].shape, o[2].size, o[2].palette),
        (ObjShape::Wide, 3, obj::PAL_ENEMY_BOX)
    );
}

#[test]
fn hit_shake_alternates_for_21_frames() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Opponent, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    hb.show();
    hb.shake();
    let mut ys = Vec::new();
    while hb.busy() {
        hb.update(&mut Frame::default());
        ys.push(hb.pos().1 - 14);
    }
    assert_eq!(ys.len(), 21);
    assert_eq!(ys[..4], [1, -1, 1, -1]);
    assert_eq!(ys[19], -1);
    assert_eq!(ys[20], 0);
}

#[test]
fn level_text_is_offset_by_missing_digits() {
    // `UpdateLvlInHealthbox`: x = 5 * (3 - digits).
    let gfx = gfx();
    let mut a = Healthbox::new(Side::Opponent, gfx);
    a.load(&mon(1, 1), &mut Frame::default());
    let mut b = a.clone();
    a.set_level(5);
    b.set_level(50);
    // Level 5: glyphs at 10 (Lv) and 15; level 50 at 5, 10, 15: tile 32 differs.
    assert_ne!(a.tiles[32], b.tiles[32]);
    let mut d = Text::new();
    decimal(7, true, &mut d);
    assert_eq!(d.as_slice(), [CHAR_SPACER, CHAR_SPACER, CHAR_0 + 7]);
    let mut d = Text::new();
    decimal(120, false, &mut d);
    assert_eq!(d.as_slice(), [CHAR_0 + 1, CHAR_0 + 2, CHAR_0]);
}

#[test]
fn hash_covers_values_and_motion() {
    let gfx = gfx();
    let mut hb = Healthbox::new(Side::Player, gfx);
    hb.load(&mon(20, 20), &mut Frame::default());
    let h = |b: &Healthbox| {
        let mut h = Fnv64::new();
        b.hash(&mut h);
        h.finish()
    };
    let h0 = h(&hb);
    hb.set_hp(19);
    let h1 = h(&hb);
    assert_ne!(h0, h1);
    hb.update(&mut Frame::default());
    assert_ne!(h(&hb), h1); // flushed
    let h2 = h(&hb);
    hb.slide_in();
    assert_ne!(h(&hb), h2);
}
