use super::*;

#[test]
fn linear_translation_matches_gen3_fixed_point() {
    // PlayerHandleIntroTrainerBallThrow: x 80 -> -40 in 50 frames. (120 << 8) / 50 = 614,
    // moving left sets bit 0: 615; after 50 frames 30750 >> 8 = 120.
    let mut l = Linear::new(-120, 50);
    let mut last = 0;
    for _ in 0..50 {
        last = l.next().unwrap();
    }
    assert_eq!(last, -120);
    assert_eq!(l.next(), None);
    // OpponentHandleTrainerSlideBack: 176 -> 280 in 35: 26624 / 35 = 760, 35 * 760 >> 8 = 103.
    let mut l = Linear::new(104, 35);
    let first = l.next().unwrap();
    assert_eq!(first, 2); // 760 >> 8
    for _ in 1..35 {
        last = l.next().unwrap();
    }
    assert_eq!(last, 103);
}

fn battle_mon(nick: u8, level: u8, hp: u16, status: crate::mon::Status) -> super::super::BattleMon {
    use crate::data::SpeciesId;
    use crate::mon::{Gender, NAME_LEN};
    super::super::BattleMon {
        party: 0,
        species: SpeciesId(1),
        nickname: [nick; NAME_LEN],
        level,
        hp,
        max_hp: 999,
        status,
        gender: Gender::Female,
        moves: [None; 4],
        pp: [0; 4],
        max_pp: [0; 4],
    }
}

fn check_budget(f: &Frame) {
    let tiles = obj_tiles_sent(f);
    assert!(tiles <= OBJ_TILES_PER_FRAME, "{tiles} OBJ tiles");
    assert!(f.validate().is_ok());
}

/// A drain with its numbers, a hit blink, a box shake and a tint in the same frames: every
/// frame is valid and sends at most 16 OBJ tiles.
#[test]
fn busiest_frames_stay_within_the_obj_tile_budget() {
    use crate::data::SpeciesId;
    use crate::mon::Status;
    let game = test_gfx::game(Some(test_gfx::mon_gfx(8, 0, 0)));
    let mon = battle_mon(0xBB, 9, 999, Status::None);
    let mut sprite = MonSprite::new(Side::Player, game.gfx);
    let mut boxes = Healthbox::new(Side::Player, game.gfx);
    let mut bar = HpBar::new(Side::Player, game.gfx);
    let mut f = Frame::default();
    sprite.load(&game, SpeciesId(1), false, &mut f);
    boxes.load(&mon, &mut f);
    bar.set(999, 999, &mut f);
    sprite.show();
    boxes.show();
    bar.start(999, 999, 0);
    sprite.hit();
    sprite.tint(TINT_STAT_DOWN);
    boxes.shake();
    while !bar.done() {
        let mut f = Frame::default();
        bar.update(&mut f);
        sprite.update(&mut f);
        boxes.set_hp(bar.value());
        boxes.update(&mut f);
        check_budget(&f);
    }
    assert_eq!(boxes.pending(), 0);
}

/// The exp bar wrapping to empty at a level-up (up to 16 tiles in its last frame) with a
/// whole healthbox redraw queued: the box waits for the frames the bar leaves.
#[test]
fn exp_wrap_and_healthbox_flush_share_the_budget() {
    use crate::mon::Status;
    let game = test_gfx::game(Some(test_gfx::mon_gfx(8, 0, 0)));
    let exp_at = |l: u8| l as u32 * 100;
    let mut boxes = Healthbox::new(Side::Player, game.gfx);
    let mut exp = ExpBar::new(game.gfx);
    let mut f = Frame::default();
    boxes.load(&battle_mon(0xBB, 5, 999, Status::None), &mut f);
    exp.set(5, 500, 600, 501, &mut f);
    boxes.show();
    exp.start(5, 501, 650, exp_at);
    // A switch-in style redraw already pending when the bar wraps.
    boxes.set_mon(&battle_mon(0xBC, 5, 123, Status::Poison));
    let mut frames = 0;
    let mut wrapped = false;
    while !exp.done() || boxes.pending() > 0 {
        let mut f = Frame::default();
        exp.update(&mut f);
        if exp.done() && exp.leveled_up() && !wrapped {
            wrapped = true;
            boxes.set_mon(&battle_mon(0xBD, exp.level(), 999, Status::Burn));
            // The wrap frame: the last fill step and the empty bar.
            assert!(obj_tiles_sent(&f) >= 8);
        }
        boxes.update(&mut f);
        check_budget(&f);
        frames += 1;
        assert!(frames < 1000);
    }
    assert!(wrapped);
    assert_eq!(exp.remaining(), Some((600, 650)));
}

/// The opponent's HP drain, its healthbox redrawn for a new mon, its faint erasing rows and
/// the player's pic reloading in steps, all in the same frames.
#[test]
fn drain_redraw_faint_and_load_step_share_the_budget() {
    use crate::data::SpeciesId;
    use crate::mon::Status;
    let game = test_gfx::game(Some(test_gfx::mon_gfx(16, 0, 0)));
    let mut enemy = MonSprite::new(Side::Opponent, game.gfx);
    let mut player = MonSprite::new(Side::Player, game.gfx);
    let mut boxes = Healthbox::new(Side::Opponent, game.gfx);
    let mut bar = HpBar::new(Side::Opponent, game.gfx);
    let mut f = Frame::default();
    enemy.load(&game, SpeciesId(1), false, &mut f);
    boxes.load(&battle_mon(0xBB, 9, 999, Status::None), &mut f);
    bar.set(999, 999, &mut f);
    enemy.show();
    boxes.show();
    bar.start(999, 999, 0);
    enemy.faint();
    boxes.set_mon(&battle_mon(0xBC, 42, 0, Status::Sleep(3)));
    let mut loaded = false;
    let mut frames = 0;
    while !bar.done() || enemy.busy() || boxes.pending() > 0 || !loaded {
        let mut f = Frame::default();
        bar.update(&mut f);
        enemy.update(&mut f);
        boxes.set_hp(bar.value());
        boxes.update(&mut f);
        if !loaded {
            loaded = player.load_step(&game, SpeciesId(1), false, &mut f);
        }
        check_budget(&f);
        frames += 1;
        // 999 HP drain at 1 HP per frame (max above 48 px).
        assert!(frames <= 1000);
    }
}

#[test]
fn obj_layout_fits_and_does_not_overlap() {
    use obj::*;
    let ranges = [
        (ENEMY_PIC, 64),
        (PLAYER_PIC, 64),
        (TRAINER_PIC, 64),
        (PLAYER_BACK, 64),
        (PLAYER_BOX, 128),
        (ENEMY_BOX, 64),
        (PLAYER_BAR, 8),
        (ENEMY_BAR, 8),
        (BALL, 4),
    ];
    for w in ranges.windows(2) {
        assert_eq!(w[0].0 + w[0].1, w[1].0);
    }
    assert_eq!(BALL + 4, FREE);
    const { assert!(FREE <= 512) };
}
