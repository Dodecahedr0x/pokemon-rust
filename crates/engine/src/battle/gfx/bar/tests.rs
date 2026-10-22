use super::super::test_gfx::*;
use super::*;
use alloc::vec::Vec;

fn obj_tiles(f: &Frame) -> Vec<(u16, Tile)> {
    f.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::ObjTile { index, tile } => Some((*index, *tile)),
            _ => None,
        })
        .collect()
}

#[test]
fn colors_follow_gen3_thresholds() {
    // 48 * 50 / 100 = 24, 48 * 20 / 100 = 9.
    assert_eq!(bar_color(48), BarColor::Green);
    assert_eq!(bar_color(25), BarColor::Green);
    assert_eq!(bar_color(24), BarColor::Yellow);
    assert_eq!(bar_color(10), BarColor::Yellow);
    assert_eq!(bar_color(9), BarColor::Red);
    assert_eq!(bar_color(0), BarColor::Red);
}

/// Golden from a throwaway transcription of `CalcNewBarValue` + `CalcBarFilledPixels`
/// (max 20 < 48: 8.8 path, step (20 << 8) / 48 = 106, rounding the value up).
#[test]
fn hp_bar_drains_20_to_6_of_20_frame_by_frame() {
    const VALUES: [u16; 34] = [
        20, 20, 19, 19, 18, 18, 18, 17, 17, 16, 16, 16, 15, 15, 14, 14, 13, 13, 13, 12, 12, 11, 11,
        11, 10, 10, 9, 9, 8, 8, 8, 7, 7, 6,
    ];
    let gfx = gfx();
    let mut bar = HpBar::new(Side::Player, gfx);
    bar.set(20, 20, &mut Frame::default());
    bar.start(20, 20, 6);
    let mut values = Vec::new();
    let mut pixels = Vec::new();
    let mut colors = Vec::new();
    let mut frames = 0;
    while !bar.done() {
        let mut f = Frame::default();
        bar.update(&mut f);
        assert!(obj_tiles(&f).len() <= HP_TILES);
        frames += 1;
        values.push(bar.value());
        let (c, p) = bar.drawn().unwrap();
        pixels.push(p);
        colors.push(c);
    }
    // 34 moving frames, then the frame `CalcNewBarValue` returns -1.
    assert_eq!(frames, 35);
    assert_eq!(values[..34], VALUES);
    assert_eq!(values[34], 6);
    let expected_px: Vec<u8> = (14..=47).rev().chain([14]).collect();
    assert_eq!(pixels, expected_px);
    assert!(colors[..23].iter().all(|&c| c == BarColor::Green));
    assert!(colors[23..].iter().all(|&c| c == BarColor::Yellow));
}

#[test]
fn hp_bar_uploads_only_changed_tiles_and_recolors() {
    let gfx = gfx();
    let mut bar = HpBar::new(Side::Opponent, gfx);
    let mut f = Frame::default();
    bar.set(20, 20, &mut f);
    // First draw: all 6 green full tiles (index 3 + 8).
    let t = obj_tiles(&f);
    assert_eq!(t.len(), 6);
    assert_eq!(t[0], (obj::ENEMY_BAR + 2, tile(TAG_HPBAR, 11)));
    // Full -> 47 px: only the last tile changes.
    bar.start(20, 20, 6);
    let mut f = Frame::default();
    bar.update(&mut f);
    assert_eq!(
        obj_tiles(&f),
        [(obj::ENEMY_BAR + 7, tile(TAG_HPBAR, 3 + 7))]
    );
    // Instant set to 4 of 20: 9 px, red: every tile is redrawn in red.
    let mut f = Frame::default();
    bar.set(4, 20, &mut f);
    let t = obj_tiles(&f);
    assert_eq!(t.len(), 6);
    assert_eq!(t[0].1, tile(TAG_HPBAR, 21 + 8));
    assert_eq!(t[1].1, tile(TAG_HPBAR, 21 + 1));
    assert_eq!(bar.drawn(), Some((BarColor::Red, 9)));
}

#[test]
fn hp_bar_direct_path_and_minimum_pixel() {
    // max 100 >= 48: one HP per frame, 100 -> 30 in 70 moves + the final frame.
    let mut bar = HpBar::new(Side::Player, gfx());
    bar.start(100, 100, 30);
    let mut frames = 0;
    while !bar.done() {
        bar.update(&mut Frame::default());
        frames += 1;
    }
    assert_eq!(frames, 71);
    assert_eq!(bar.value(), 30);
    // 1 of 200: 48 / 200 = 0 px, but a living mon shows 1.
    bar.set(1, 200, &mut Frame::default());
    assert_eq!(bar.drawn(), Some((BarColor::Red, 1)));
    bar.set(0, 200, &mut Frame::default());
    assert_eq!(bar.drawn(), Some((BarColor::Red, 0)));
}

/// Goldens from the same transcription of `MoveBattleBar(EXP_BAR)`.
#[test]
fn exp_bar_fills_one_level_then_wraps() {
    let gfx = gfx();
    let mut bar = ExpBar::new(gfx);
    let at = |l: u8| l as u32 * 100;
    let mut f = Frame::default();
    bar.set(5, at(5), at(6), 520, &mut f);
    assert_eq!(bar.drawn_pixels(), Some(12)); // 20 * 64 / 100
    assert_eq!(
        obj_tiles(&f),
        [
            (obj::PLAYER_BOX + 0x24, tile(TAG_EXPBAR, 8)),
            (obj::PLAYER_BOX + 0x25, tile(TAG_EXPBAR, 4)),
            (obj::PLAYER_BOX + 0x26, tile(TAG_EXPBAR, 0)),
            (obj::PLAYER_BOX + 0x27, tile(TAG_EXPBAR, 0)),
            (obj::PLAYER_BOX + 0x60, tile(TAG_EXPBAR, 0)),
            (obj::PLAYER_BOX + 0x61, tile(TAG_EXPBAR, 0)),
            (obj::PLAYER_BOX + 0x62, tile(TAG_EXPBAR, 0)),
            (obj::PLAYER_BOX + 0x63, tile(TAG_EXPBAR, 0)),
        ]
    );
    // The whole gain (+130) is `receivedValue`: `GetScaledExpFraction` = |12 - 64| = 52 px
    // (the new value clamps to the 100 max), step 130 / 52 = 2 exp per frame.
    bar.start(5, 520, 650, at);
    let mut pixels = Vec::new();
    let mut frames = 0;
    while !bar.done() {
        let mut f = Frame::default();
        bar.update(&mut f);
        assert!(obj_tiles(&f).len() <= 16);
        frames += 1;
        if frames > EXP_DELAY as usize && bar.level() == 5 {
            pixels.push(bar.drawn_pixels().unwrap());
        }
    }
    // 13 waiting frames, 40 moves, then the -1 frame wraps and stops.
    assert_eq!(frames, 13 + 41);
    assert_eq!(pixels.len(), 40);
    assert_eq!(pixels[..6], [14, 15, 16, 17, 19, 20]);
    assert_eq!(pixels[38..], [62, 64]);
    assert_eq!(bar.drawn_pixels(), Some(0));
    assert!(bar.leveled_up());
    assert_eq!(bar.level(), 6);
    assert_eq!(bar.remaining(), Some((600, 650)));
    // Stopped until the scene resumes it.
    let mut f = Frame::default();
    bar.update(&mut f);
    assert!(f.cmds.is_empty());
    // Level 6: 600 -> 650 of a 100 span: |0 - 32| px, 50 / 32 = 1 per frame, after its own
    // 13-frame wait.
    bar.start(6, 600, 650, at);
    let mut frames = 0;
    while !bar.done() {
        bar.update(&mut Frame::default());
        frames += 1;
    }
    assert_eq!(frames, 13 + 50 + 1);
    assert_eq!(bar.level(), 6);
    assert!(!bar.leveled_up());
    assert_eq!(bar.remaining(), None);
    assert_eq!(bar.drawn_pixels(), Some(32));
}

/// Span 40 < 64: the 8.8 path, (40 << 8) / 64 = 160 per frame (0.625 exp).
#[test]
fn exp_bar_small_span_and_exact_level_up() {
    let mut bar = ExpBar::new(gfx());
    let at = |l: u8| 1000 + (l as u32 - 5) * 40;
    bar.start(5, 1010, 1040, at);
    let mut pixels = Vec::new();
    while bar.level() == 5 {
        bar.update(&mut Frame::default());
        if let (5, Some(p)) = (bar.level(), bar.drawn_pixels()) {
            pixels.push(p);
        }
    }
    // 48 moves; the 49th frame (-1) wraps.
    let expected: Vec<u8> = (17..=64).collect();
    assert_eq!(pixels, expected);
    // Exactly the next level's exp: the bar wraps to empty and the gain is over.
    assert!(bar.done() && bar.leveled_up());
    assert_eq!(bar.remaining(), None);
    assert_eq!(bar.level(), 6);
    assert_eq!(bar.drawn_pixels(), Some(0));
}

#[test]
fn exp_bar_is_empty_at_max_level_and_hash_changes() {
    let mut bar = ExpBar::new(gfx());
    bar.set(100, 1000, 1000, 1000, &mut Frame::default());
    assert_eq!(bar.drawn_pixels(), Some(0));
    let h = |b: &ExpBar| {
        let mut h = Fnv64::new();
        b.hash(&mut h);
        h.finish()
    };
    let h0 = h(&bar);
    bar.start(99, 9900, 9910, |l| l as u32 * 100);
    assert_ne!(h(&bar), h0);
}
