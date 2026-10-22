use super::*;
use crate::data::{TextId, TrainerId};
use alloc::vec::Vec;

const WILD: BattleKind = BattleKind::Wild {
    first_battle: false,
};
const TRAINER: BattleKind = BattleKind::Trainer {
    trainer: TrainerId(1),
    defeat: TextId(0),
};

/// Runs a transition to the end, returning every frame.
fn run(kind: &BattleKind) -> Vec<Frame> {
    let mut tr = BattleTransition::new(kind);
    let mut frames = Vec::new();
    loop {
        let mut f = Frame::default();
        let done = tr.update(&mut f);
        frames.push(f);
        if done {
            return frames;
        }
        assert!(frames.len() < 1000, "never ends");
    }
}

fn rows(t: Tile) -> [&'static str; 8] {
    t.map(|r| {
        let s: alloc::string::String = (0..8)
            .map(|x| {
                if (r >> (4 * x)) & 0xF == 0xF {
                    'f'
                } else {
                    '0'
                }
            })
            .collect();
        &*alloc::boxed::Box::leak(s.into_boxed_str())
    })
}

#[test]
fn shrinking_box_matches_the_decomp_stages() {
    // graphics/battle_transitions/shrinking_box.png, tiles 0..15 (pixel 15 = 'f').
    assert_eq!(rows(shrinking_box(0)), ["00000000"; 8]);
    assert_eq!(
        rows(shrinking_box(1)),
        [
            "ffffffff", "00000000", "00000000", "00000000", "00000000", "00000000", "00000000",
            "00000000"
        ]
    );
    assert_eq!(
        rows(shrinking_box(2)),
        [
            "ffffffff", "0000000f", "0000000f", "0000000f", "0000000f", "0000000f", "0000000f",
            "0000000f"
        ]
    );
    assert_eq!(
        rows(shrinking_box(4)),
        [
            "ffffffff", "f000000f", "f000000f", "f000000f", "f000000f", "f000000f", "f000000f",
            "ffffffff"
        ]
    );
    assert_eq!(
        rows(shrinking_box(6)),
        [
            "ffffffff", "ffffffff", "f00000ff", "f00000ff", "f00000ff", "f00000ff", "f00000ff",
            "ffffffff"
        ]
    );
    assert_eq!(
        rows(shrinking_box(10)),
        [
            "ffffffff", "ffffffff", "ffffffff", "ff000fff", "ff000fff", "ff000fff", "ffffffff",
            "ffffffff"
        ]
    );
    assert_eq!(
        rows(shrinking_box(12)),
        [
            "ffffffff", "ffffffff", "ffffffff", "fff00fff", "fff00fff", "ffffffff", "ffffffff",
            "ffffffff"
        ]
    );
    assert_eq!(
        rows(shrinking_box(13)),
        [
            "ffffffff", "ffffffff", "ffffffff", "ffffffff", "fff00fff", "ffffffff", "ffffffff",
            "ffffffff"
        ]
    );
    assert_eq!(rows(shrinking_box(14)), ["ffffffff"; 8]);
    assert_eq!(shrinking_box(15), shrinking_box(14));
}

#[test]
fn intro_flashes_three_times_with_lighten() {
    let frames = run(&WILD);
    let levels: Vec<(bool, u8)> = frames[..INTRO_FRAMES as usize]
        .iter()
        .map(|f| (f.lighten, f.brightness))
        .collect();
    let flash: Vec<(bool, u8)> = [2, 4, 6, 8, 10, 12, 14, 16, 14, 12, 10, 8, 6, 4, 2, 0]
        .iter()
        .map(|&b| (b > 0, b))
        .collect();
    assert_eq!(levels, [flash.clone(), flash.clone(), flash].concat());
    assert!(
        frames[..INTRO_FRAMES as usize]
            .iter()
            .all(|f| f.cmds.is_empty()),
        "the intro only changes BLDY"
    );
}

#[test]
fn wild_grid_squares_frame_count_and_ends_black() {
    let frames = run(&WILD);
    assert_eq!(frames.len(), WILD_FRAMES as usize);
    // 48 intro + 1 init + 13 * 3 to the last stage + 15 held + the black frame.
    assert_eq!(WILD_FRAMES, 48 + 1 + 39 + 15 + 1);
    let last = frames.last().unwrap();
    assert!(last.is_black());
    assert!(frames[..frames.len() - 1].iter().all(|f| !f.is_black()));
    // Stages 1..=14 upload the pattern tile at init + 1 + 3k.
    let stage_frames: Vec<usize> = frames
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            f.cmds
                .iter()
                .any(|c| matches!(c, Cmd::BgTile { index, .. } if *index == TILE_PATTERN))
        })
        .map(|(i, _)| i)
        .collect();
    let init = INTRO_FRAMES as usize;
    let expected: Vec<usize> = core::iter::once(init)
        .chain((0..14).map(|k| init + 1 + 3 * k))
        .collect();
    assert_eq!(stage_frames, expected);
    let Cmd::BgTile { tile, .. } = frames[init + 1 + 3 * 13].cmds[0] else {
        panic!()
    };
    assert_eq!(tile, shrinking_box(14));
}

#[test]
fn trainer_bars_cover_every_visible_entry_then_black() {
    let frames = run(&TRAINER);
    assert_eq!(frames.len(), TRAINER_FRAMES as usize);
    assert_eq!(TRAINER_FRAMES, 48 + 1 + 15 + 1);
    assert!(frames.last().unwrap().is_black());
    let mut map = [0u16; 1024];
    let empty = TILE_PATTERN | 15 << 12;
    let black = TILE_BLACK | 15 << 12;
    for f in &frames {
        for c in &f.cmds {
            match c {
                Cmd::MapFill { entry, .. } => map.fill(*entry),
                Cmd::MapEntries { start, entries, .. } => {
                    map[*start as usize..*start as usize + entries.len()].copy_from_slice(entries)
                }
                _ => {}
            }
        }
    }
    for row in 0..20 {
        for col in 0..30 {
            assert_eq!(map[row * 32 + col], black, "({col}, {row})");
        }
    }
    assert_eq!(map[20 * 32], empty, "rows below the screen stay clear");
    // Halfway, even rows are closed from the left and odd rows from the right.
    let mut half = [0u16; 1024];
    for f in &frames[..INTRO_FRAMES as usize + 1 + 7] {
        for c in &f.cmds {
            match c {
                Cmd::MapFill { entry, .. } => half.fill(*entry),
                Cmd::MapEntries { start, entries, .. } => {
                    half[*start as usize..*start as usize + entries.len()].copy_from_slice(entries)
                }
                _ => {}
            }
        }
    }
    assert_eq!(half[13], black);
    assert_eq!(half[14], empty);
    assert_eq!(half[32 + 15], empty);
    assert_eq!(half[32 + 16], black);
}

#[test]
fn touches_only_bg0_and_palette_15_before_black() {
    for kind in [WILD, TRAINER] {
        for f in run(&kind) {
            assert_eq!(f.validate(), Ok(()));
            for c in &f.cmds {
                let ok = match c {
                    Cmd::BgTile {
                        char_block, index, ..
                    } => {
                        *char_block == layout::TEXT_CHAR_BLOCK
                            && [TILE_BLACK, TILE_PATTERN].contains(index)
                    }
                    Cmd::BgPalette { bank, .. } => *bank == layout::WINDOW_PALETTE,
                    Cmd::MapFill { screen_block, .. } | Cmd::MapEntries { screen_block, .. } => {
                        *screen_block == 31
                    }
                    Cmd::Bg { layer, control } => {
                        *layer == layout::TEXT_BG
                            && control.char_block == layout::TEXT_CHAR_BLOCK
                            && control.screen_block == 31
                    }
                    _ => false,
                };
                assert!(ok, "{c:?}");
            }
        }
    }
    // The pattern tiles lie in char block 2, clear of the field (char blocks 0-1) and of
    // the screen blocks (char block 3).
    assert!((TILE_PATTERN as usize + 1) * 32 <= 0x4000);
}

#[test]
fn stays_black_after_done_and_hash_tracks_progress() {
    let h = |t: &BattleTransition| {
        let mut h = Fnv64::new();
        t.hash(&mut h);
        h.finish()
    };
    let mut a = BattleTransition::new(&WILD);
    let b = BattleTransition::new(&TRAINER);
    assert_ne!(h(&a), h(&b));
    let before = h(&a);
    let mut f = Frame::default();
    a.update(&mut f);
    assert_ne!(h(&a), before);
    while !a.update(&mut Frame::default()) {}
    assert!(a.is_done());
    let mut f = Frame::default();
    assert!(a.update(&mut f));
    assert!(f.is_black() && f.cmds.is_empty());
}
