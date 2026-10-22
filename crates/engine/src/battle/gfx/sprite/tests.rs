use super::super::test_gfx::*;
use super::*;
use alloc::vec::Vec;

fn run(s: &mut MonSprite) -> (usize, Frame) {
    let mut n = 0;
    let mut last = Frame::default();
    while s.busy() {
        last = Frame::default();
        s.update(&mut last);
        n += 1;
        assert!(n < 1000);
    }
    (n, last)
}

#[test]
fn load_uploads_pic_palette_ball_and_places_gen3_coords() {
    let game = game(Some(mon_gfx(9, 4, 6)));
    let mut enemy = MonSprite::new(Side::Opponent, game.gfx);
    let mut f = Frame::default();
    enemy.load(&game, SpeciesId(1), false, &mut f);
    let g = game.tables.species[1].gfx.unwrap();
    assert_eq!(
        f.cmds[..2],
        [
            Cmd::ObjTiles {
                first: obj::ENEMY_PIC,
                tiles: g.front,
            },
            Cmd::ObjPalette {
                bank: obj::PAL_ENEMY_MON,
                colors: *g.palette,
            },
        ]
    );
    assert!(f.cmds.contains(&Cmd::ObjTiles {
        first: obj::BALL,
        tiles: &game.gfx.ball.tiles[..4],
    }));
    // Centre (176, 40 + 9 - 6) -> top-left (144, 11).
    enemy.show();
    let mut f = Frame::default();
    enemy.update(&mut f);
    assert_eq!(
        f.objs,
        [Obj {
            x: 144,
            y: 11,
            tile: obj::ENEMY_PIC,
            palette: obj::PAL_ENEMY_MON,
            shape: ObjShape::Square,
            size: 3,
            priority: OBJ_PRIORITY_MON,
            ..Obj::default()
        }]
    );
    let mut player = MonSprite::new(Side::Player, game.gfx);
    let mut f = Frame::default();
    player.load(&game, SpeciesId(1), true, &mut f);
    assert_eq!(
        f.cmds[..2],
        [
            Cmd::ObjTiles {
                first: obj::PLAYER_PIC,
                tiles: g.back,
            },
            Cmd::ObjPalette {
                bank: obj::PAL_PLAYER_MON,
                colors: *g.shiny,
            },
        ]
    );
    // Centre (72, 80 + 4) -> (40, 52).
    assert_eq!(player.pos(), (40, 52));
    // No pics: nothing uploaded, centre unchanged.
    let mut none = MonSprite::new(Side::Opponent, game.gfx);
    let mut f = Frame::default();
    none.load(&game, SpeciesId(0), false, &mut f);
    assert!(f.cmds.is_empty());
    assert_eq!(none.pos(), (144, 8));
}

#[test]
fn wild_slide_in_takes_120_frames_then_fades_gray() {
    let game = game(Some(mon_gfx(8, 0, 0)));
    let mut s = MonSprite::new(Side::Opponent, game.gfx);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    s.slide_in();
    let mut xs = Vec::new();
    let mut coeffs = Vec::new();
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        xs.push(f.objs[0].x);
        let pal = f.cmds.iter().find_map(|c| match c {
            Cmd::ObjPalette { bank: 0, colors } => Some(colors[0]),
            _ => None,
        });
        coeffs.push(pal);
    }
    // 240 / 2 = 120 sliding frames; the gray fades 10, 8, 6, 4, 2, 0 from the arrival
    // frame (the last one restores the palette). Palettes go out only on a change.
    assert_eq!(xs[0], 144 - 238);
    assert_eq!(xs[119], 144);
    assert_eq!(xs.len(), 125);
    let base = 0x100u16; // palette color 0 of the synthetic pic
    assert_eq!(coeffs[0], Some(blend(&[base; 16], 10, WILD_GRAY)[0]));
    assert!(coeffs[1..120].iter().all(Option::is_none));
    assert_eq!(coeffs[120], Some(blend(&[base; 16], 8, WILD_GRAY)[0]));
    assert_eq!(coeffs[123], Some(blend(&[base; 16], 2, WILD_GRAY)[0]));
    assert_eq!(coeffs[124], Some(base));
    let mut f = Frame::default();
    s.update(&mut f);
    assert!(f.cmds.is_empty());
}

#[test]
fn send_out_ball_then_mon() {
    let game = game(Some(mon_gfx(8, 0, 0)));
    let mut p = MonSprite::new(Side::Player, game.gfx);
    p.load(&game, SpeciesId(1), false, &mut Frame::default());
    p.send_out();
    let mut frames = Vec::new();
    while p.busy() {
        let mut f = Frame::default();
        p.update(&mut f);
        frames.push(f);
    }
    // 25 ball frames + 4 pop frames.
    assert_eq!(frames.len(), 29);
    let ball = |f: &Frame| {
        f.objs
            .iter()
            .find(|o| o.tile == obj::BALL)
            .map(|o| (o.x, o.y))
    };
    // Lands at (72, min(80 + 8, 104) + 24) = (72, 112): top-left (64, 104).
    assert_eq!(ball(&frames[24]), Some((64, 104)));
    assert!(frames[..28].iter().all(|f| f.objs.len() == 1));
    assert_eq!(frames[28].objs.len(), 1);
    assert_eq!(frames[28].objs[0].tile, obj::PLAYER_PIC);
    // The arc goes above the straight line.
    assert!(ball(&frames[12]).unwrap().1 < 68 - 8);
    let mut o = MonSprite::new(Side::Opponent, game.gfx);
    o.load(&game, SpeciesId(1), false, &mut Frame::default());
    o.send_out();
    let (n, last) = run(&mut o);
    assert_eq!(n, 16 + 4);
    assert_eq!(last.objs.len(), 1);
    assert!(o.visible());
    o.recall();
    let (n, _) = run(&mut o);
    assert_eq!(n, 8);
    assert!(!o.visible());
}

#[test]
fn opponent_faint_sinks_and_erases_rows() {
    // front_y 16: data[3] = 8 - 2 = 6: 7 steps every 2 frames, the 7th hides it.
    let game = game(Some(mon_gfx(16, 0, 0)));
    let mut s = MonSprite::new(Side::Opponent, game.gfx);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    s.show();
    s.faint();
    let mut frames = Vec::new();
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        frames.push(f);
    }
    assert_eq!(frames.len(), 13);
    // Step 1 (frame 1): down 8, erase row 5 (tiles 40..48).
    let erased: Vec<u16> = frames[0]
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::ObjTile { index, tile } if *tile == [0; 8] => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(erased, (40..48).collect::<Vec<_>>());
    assert_eq!(frames[0].objs[0].y, 8 + 16 + 8);
    assert!(frames[1].cmds.is_empty());
    assert_eq!(frames[11].objs[0].y, 8 + 16 + 48);
    assert!(frames[12].objs.is_empty());
    assert!(frames.iter().all(|f| f.cmds.len() <= 16));
}

#[test]
fn player_faint_slides_off_screen() {
    // Centre y 80 + 4: below 160 after ceil((160 - 84 + 1) / 5) = 16 frames.
    let game = game(Some(mon_gfx(0, 4, 0)));
    let mut s = MonSprite::new(Side::Player, game.gfx);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    s.show();
    s.faint();
    let (n, last) = run(&mut s);
    assert_eq!(n, 16);
    assert!(last.objs.is_empty());
}

#[test]
fn hit_blink_lunge_and_tint() {
    let game = game(Some(mon_gfx(0, 0, 0)));
    let mut s = MonSprite::new(Side::Player, game.gfx);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    s.show();
    s.hit();
    let mut shown = Vec::new();
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        shown.push(!f.objs.is_empty());
    }
    // 32 toggle frames + the completion frame; toggles at 0, 4, .., 28.
    assert_eq!(shown.len(), 33);
    assert_eq!(
        shown[..8],
        [false, false, false, false, true, true, true, true]
    );
    assert!(shown[28..].iter().all(|&v| v));
    assert!(s.visible());

    // A hidden sprite stays hidden through a blink.
    s.hide();
    s.hit();
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        assert!(f.objs.is_empty());
    }
    assert!(!s.visible());
    s.show();

    s.lunge();
    let mut xs = Vec::new();
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        xs.push(f.objs[0].x - 40);
    }
    assert_eq!(xs, [2, 4, 6, 8, 6, 4, 2, 0]);

    s.tint(TINT_STAT_UP);
    let mut coeffs = Vec::new();
    let mut frames = 0;
    while s.busy() {
        let mut f = Frame::default();
        s.update(&mut f);
        frames += 1;
        if let Some(Cmd::ObjPalette { colors, .. }) = f.cmds.first() {
            coeffs.push(colors[0]);
        }
    }
    // 32 frames; coefficients 0, 1, 1, 2, .., 8 then 7, 7, .., 0, 0: one upload per
    // change (0..=8 then 7..=0).
    assert_eq!(frames, 32);
    assert_eq!(coeffs.len(), 17);
    let base = [0x100u16; 16];
    assert_eq!(coeffs[8], blend(&base, 8, TINT_STAT_UP)[0]);
    assert_eq!(coeffs[16], base[0]);
}

#[test]
fn load_step_uploads_16_tiles_per_frame_palette_last() {
    let game = game(Some(mon_gfx(8, 0, 0)));
    let g = game.tables.species[1].gfx.unwrap();
    let mut s = MonSprite::new(Side::Opponent, game.gfx);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    s.show();
    let mut frames = Vec::new();
    loop {
        let mut f = Frame::default();
        let done = s.load_step(&game, SpeciesId(1), true, &mut f);
        s.update(&mut f);
        assert!(f.objs.is_empty(), "hidden while loading");
        frames.push(f);
        if done {
            break;
        }
    }
    assert_eq!(frames.len(), 4);
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(
            f.cmds[0],
            Cmd::ObjTiles {
                first: obj::ENEMY_PIC + 16 * i as u16,
                tiles: &g.front[16 * i..16 * (i + 1)],
            }
        );
        assert_eq!(f.cmds.len(), if i == 3 { 2 } else { 1 });
    }
    assert_eq!(
        frames[3].cmds[1],
        Cmd::ObjPalette {
            bank: obj::PAL_ENEMY_MON,
            colors: *g.shiny,
        }
    );
    assert!(!s.loading());
    // Only what the frame's budget leaves: 10 tiles already queued leaves 6, none at 16.
    let mut f = Frame::default();
    for i in 0..10 {
        f.cmds.push(Cmd::ObjTile {
            index: obj::FREE + i,
            tile: [0; 8],
        });
    }
    assert!(!s.load_step(&game, SpeciesId(1), false, &mut f));
    assert_eq!(super::super::obj_tiles_sent(&f), 16);
    for i in 0..6 {
        f.cmds.push(Cmd::ObjTile {
            index: obj::FREE + i,
            tile: [0; 8],
        });
    }
    let n = f.cmds.len();
    assert!(!s.load_step(&game, SpeciesId(1), false, &mut f));
    assert_eq!(f.cmds.len(), n);
    let mut steps = 0;
    while !s.load_step(&game, SpeciesId(1), false, &mut Frame::default()) {
        steps += 1;
    }
    // 58 tiles left: 3 more frames of 16, the 4th with 10 and the palette.
    assert_eq!(steps, 3);
    s.show();
    let mut f = Frame::default();
    s.update(&mut f);
    assert_eq!(f.objs.len(), 1);
}

#[test]
fn blend_is_gen3_blend_palette() {
    // r 0 -> 31 at 8/16: 0 + (31 * 8 >> 4) = 15; b 31 -> 0: 31 + (-31 * 8 >> 4) = 15.
    let out = blend(&[31 << 10; 16], 8, 31);
    assert_eq!(out[0], 15 | 15 << 10);
}

#[test]
fn trainers_slide_in_and_out() {
    let pic = trainer_pic();
    let mut t = TrainerSprite::new(Side::Opponent);
    let mut f = Frame::default();
    t.load(pic, &mut f);
    assert_eq!(
        f.cmds,
        [
            Cmd::ObjTiles {
                first: obj::TRAINER_PIC,
                tiles: pic.tiles,
            },
            Cmd::ObjPalette {
                bank: obj::PAL_TRAINER,
                colors: *pic.palette,
            },
        ]
    );
    t.slide_in();
    let mut n = 0;
    while t.busy() {
        t.update(&mut Frame::default());
        n += 1;
    }
    assert_eq!(n, 120);
    // `trainer_pic()` has size 1: centre y (8 - 1) * 4 + 40 = 68.
    assert_eq!(t.pos(), (144, 68 - 32));
    t.slide_out();
    let mut n = 0;
    while t.busy() {
        t.update(&mut Frame::default());
        n += 1;
    }
    // 35 moves + the completion frame; 176 + 103 at the last move.
    assert_eq!(n, 36);
    assert!(!t.visible());
    t.slide_in_defeated();
    let mut n = 0;
    while t.busy() {
        t.update(&mut Frame::default());
        n += 1;
    }
    assert_eq!(n, 48);
    assert_eq!(t.pos(), (208 - 32, 68 - 32));

    let gfx = gfx();
    let back: &'static TrainerPic = Box::leak(Box::new(gfx.player_back));
    let mut p = TrainerSprite::new(Side::Player);
    let mut f = Frame::default();
    p.load(back, &mut f);
    assert_eq!(
        f.cmds[0],
        Cmd::ObjTiles {
            first: obj::PLAYER_BACK,
            tiles: &back.tiles[..64],
        }
    );
    p.slide_in();
    let mut f = Frame::default();
    p.update(&mut f);
    assert_eq!(f.objs[0].x, 48 + 238);
    while p.busy() {
        p.update(&mut Frame::default());
    }
    // The test back pic has size 4: centre y (8 - 4) * 4 + 80 = 96.
    assert_eq!(p.pos(), (48, 96 - 32));
    p.slide_out();
    let mut last = (0, 0);
    let mut n = 0;
    while p.busy() {
        let mut f = Frame::default();
        p.update(&mut f);
        if let Some(o) = f.objs.first() {
            last = (o.x, o.y);
        }
        n += 1;
    }
    assert_eq!(n, 51);
    assert_eq!(last, (-40 - 32, 96 - 32));
}

#[test]
fn hashes_differ_by_state() {
    let game = game(Some(mon_gfx(0, 0, 0)));
    let mut s = MonSprite::new(Side::Player, game.gfx);
    let h = |s: &MonSprite| {
        let mut h = Fnv64::new();
        s.hash(&mut h);
        h.finish()
    };
    let h0 = h(&s);
    s.load(&game, SpeciesId(1), false, &mut Frame::default());
    let h1 = h(&s);
    assert_ne!(h0, h1);
    s.send_out();
    assert_ne!(h(&s), h1);
    let mut t = TrainerSprite::new(Side::Player);
    let mut a = Fnv64::new();
    t.hash(&mut a);
    t.slide_in();
    let mut b = Fnv64::new();
    t.hash(&mut b);
    assert_ne!(a.finish(), b.finish());
}
