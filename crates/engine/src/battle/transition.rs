//! Field-to-battle transition. **Task G.**
//!
//! Gen3 runs a transition in two parts (`Task_BattleTransition`, `battle_transition.c`):
//! 1. **Intro** (`Task_Intro` → `CreateIntroTask(0, 0, 3, 2, 2)`): three flashes. Each one
//!    blends every palette toward gray by 2/16 per frame up to 16/16
//!    (`TransitionIntro_FadeToGray`), then back down by 2 per frame (`FadeFromGray`): 16 frames
//!    per flash, 48 in all. We brighten with `Frame::lighten` and the same BLDY levels instead
//!    of blending toward `RGB(11, 11, 11)`, so the flash is white rather than gray.
//! 2. **Pattern** on BG0, then black:
//!    - wild battles: `GRID_SQUARES` (`GridSquares_Init` / `_Main` / `_End`): every BG0 tile
//!      shows the same `shrinking_box` tile, a black frame closing in a spiral, one stage every
//!      3 frames for 14 stages, then 15 frames of a fully black BG0 and `FadeScreenBlack`.
//!    - trainer battles: a horizontal-bars sweep: tile rows close 2 columns per frame, even
//!      rows from the left and odd rows from the right (15 frames), then black.
//!
//!    Gen3 picks a wild transition from `sBattleTransitionTable_Wild` (`battle_setup.c`) by
//!    map type, the first entry when the enemy is lower level than the player's mon and the
//!    second otherwise: normal SLICE / WHITE_BARS_FADE, cave CLOCKWISE_WIPE / GRID_SQUARES,
//!    flash BLUR / GRID_SQUARES, water WAVE / RIPPLE; the first battle uses BLUR. Trainers use
//!    `sBattleTransitionTable_Trainer` (`GetTrainerBattleTransition`). Most are HBlank/window
//!    effects; this module approximates them with the two tile patterns above (wild, including
//!    the first battle: grid squares; trainer: bars).
//!
//! The screen ends fully black (`brightness = BRIGHTNESS_BLACK`, not lightened): `update`
//! returns true from that frame on.
//!
//! VRAM: until the screen is black it writes only BG0 (layer 0 control, scroll 0, tiles
//! [`TILE_BLACK`] and [`TILE_PATTERN`] of `layout::TEXT_CHAR_BLOCK`, screen block
//! `SCREEN_BLOCKS[TEXT_BG]`) and BG palette bank `WINDOW_PALETTE` (Gen3 loads the pattern
//! palette into bank 15 too). The field text box must be re-initialised after the battle.
//!
//! Frame contract: every `update` sets `f.brightness`, `f.lighten` and, once the pattern has
//! started, `f.scroll[0]`. The caller keeps drawing the field (objects frozen) underneath.

use super::BattleKind;
use crate::hash::Fnv64;
use crate::video::{BRIGHTNESS_BLACK, BgControl, Cmd, Frame, MAP_SIZE, Tile, layout};
use alloc::vec;

/// Number of intro flashes (`CreateIntroTask` `numFades`).
pub const INTRO_FADES: u16 = 3;
/// BLDY change per frame (`fadeToGrayIncrement` / `fadeFromGrayIncrement`).
const INTRO_STEP: u8 = 2;
/// Frames of one flash: 8 up to 16, 8 down to 0.
const FLASH_FRAMES: u16 = 2 * (BRIGHTNESS_BLACK as u16 / INTRO_STEP as u16);
/// Frames of the intro; the pattern is set up on the next frame.
pub const INTRO_FRAMES: u16 = INTRO_FADES * FLASH_FRAMES;

/// Grid squares: the last `shrinking_box` stage (fully black).
const GRID_STAGES: u8 = 14;
/// Frames per stage (`GridSquares_Main` `tDelay = 3`).
const GRID_DELAY: u16 = 3;
/// Frames between the last stage and the fade to black (`tDelay = 16`, decremented on the
/// frame it is set, then `--tDelay == 0` in `GridSquares_End`).
const GRID_HOLD: u16 = 15;
/// Updates a wild transition takes; the last one returns true.
pub const WILD_FRAMES: u16 =
    INTRO_FRAMES + 1 + (GRID_STAGES as u16 - 1) * GRID_DELAY + GRID_HOLD + 1;

/// Bars: visible tile rows and columns, columns closed per frame.
const BAR_ROWS: u16 = 20;
const BAR_COLS: u16 = 30;
const BAR_STEP: u16 = 2;
const BAR_FRAMES: u16 = BAR_COLS / BAR_STEP;
/// Updates a trainer transition takes; the last one returns true.
pub const TRAINER_FRAMES: u16 = INTRO_FRAMES + 1 + BAR_FRAMES + 1;

/// BG0 tiles in `layout::TEXT_CHAR_BLOCK`, past every text box canvas.
pub const TILE_BLACK: u16 = 510;
pub const TILE_PATTERN: u16 = 511;
/// Pattern pixels use color 15 (black) of this bank, as Gen3 `sFieldEffectPal_Pokeball`.
const PALETTE: u8 = layout::WINDOW_PALETTE;
const BLACK_INDEX: u32 = 0xF;

/// `graphics/battle_transitions/shrinking_box.png` stage `stage` (0 = empty, 14 = full): the
/// edges of the remaining square fill with color 15 one at a time, clockwise from the top;
/// the last stage fills the 2-pixel hole left by stage 13 at once.
pub const fn shrinking_box(stage: u8) -> Tile {
    if stage >= GRID_STAGES {
        return [0xFFFF_FFFF; 8];
    }
    let mut t: Tile = [0; 8];
    let (mut top, mut bottom, mut left, mut right) = (0i32, 7i32, 0i32, 7i32);
    let mut k = 0;
    while k < stage && top <= bottom && left <= right {
        // (fixed coordinate, from, to, horizontal)
        let (fixed, from, to, row) = match k % 4 {
            0 => (top, left, right, true),
            1 => (right, top, bottom, false),
            2 => (bottom, left, right, true),
            _ => (left, top, bottom, false),
        };
        let mut i = from;
        while i <= to {
            let (x, y) = if row { (i, fixed) } else { (fixed, i) };
            t[y as usize] |= BLACK_INDEX << (4 * x);
            i += 1;
        }
        match k % 4 {
            0 => top += 1,
            1 => right -= 1,
            2 => bottom -= 1,
            _ => left += 1,
        }
        k += 1;
    }
    t
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pattern {
    GridSquares,
    Bars,
}

pub struct BattleTransition {
    pattern: Pattern,
    /// Updates done so far.
    t: u16,
}

impl BattleTransition {
    /// Picks the pattern from the battle kind (wild: grid squares; trainer: bars).
    pub fn new(kind: &BattleKind) -> Self {
        let pattern = match kind {
            BattleKind::Wild { .. } => Pattern::GridSquares,
            BattleKind::Trainer { .. } => Pattern::Bars,
        };
        Self { pattern, t: 0 }
    }

    /// Updates this transition takes (the last one returns true).
    pub const fn frames(&self) -> u16 {
        match self.pattern {
            Pattern::GridSquares => WILD_FRAMES,
            Pattern::Bars => TRAINER_FRAMES,
        }
    }

    /// True once the screen is fully black.
    pub const fn is_done(&self) -> bool {
        self.t >= self.frames()
    }

    /// One frame; true once the screen is fully black.
    pub fn update(&mut self, f: &mut Frame) -> bool {
        let t = self.t;
        if self.is_done() || t + 1 == self.frames() {
            f.brightness = BRIGHTNESS_BLACK;
            f.lighten = false;
            self.t = self.frames();
            return true;
        }
        self.t += 1;
        f.brightness = 0;
        f.lighten = false;
        if t < INTRO_FRAMES {
            let c = (t % FLASH_FRAMES) as u8;
            let half = FLASH_FRAMES as u8 / 2;
            f.brightness = if c < half {
                INTRO_STEP * (c + 1)
            } else {
                BRIGHTNESS_BLACK - INTRO_STEP * (c + 1 - half)
            };
            f.lighten = f.brightness > 0;
            return false;
        }
        f.scroll[0] = (0, 0);
        let k = t - INTRO_FRAMES;
        if k == 0 {
            Self::setup(f);
            return false;
        }
        match self.pattern {
            Pattern::GridSquares => {
                let k = k - 1;
                if k.is_multiple_of(GRID_DELAY) && k / GRID_DELAY < GRID_STAGES as u16 {
                    f.cmds.push(Cmd::BgTile {
                        char_block: layout::TEXT_CHAR_BLOCK,
                        index: TILE_PATTERN,
                        tile: shrinking_box((k / GRID_DELAY) as u8 + 1),
                    });
                }
            }
            Pattern::Bars => {
                let cols = k * BAR_STEP;
                let entry = TILE_BLACK | (PALETTE as u16) << 12;
                for row in 0..BAR_ROWS {
                    let col = if row % 2 == 0 {
                        cols - BAR_STEP
                    } else {
                        BAR_COLS - cols
                    };
                    f.cmds.push(Cmd::MapEntries {
                        screen_block: screen_block(),
                        start: row * MAP_SIZE as u16 + col,
                        entries: vec![entry; BAR_STEP as usize],
                    });
                }
            }
        }
        false
    }

    /// `GridSquares_Init`: pattern tiles, palette, every BG0 entry on the empty pattern tile,
    /// BG0 on top.
    fn setup(f: &mut Frame) {
        f.cmds.push(Cmd::BgTile {
            char_block: layout::TEXT_CHAR_BLOCK,
            index: TILE_BLACK,
            tile: shrinking_box(GRID_STAGES),
        });
        f.cmds.push(Cmd::BgTile {
            char_block: layout::TEXT_CHAR_BLOCK,
            index: TILE_PATTERN,
            tile: shrinking_box(0),
        });
        f.cmds.push(Cmd::BgPalette {
            bank: PALETTE,
            colors: [0; 16],
        });
        f.cmds.push(Cmd::MapFill {
            screen_block: screen_block(),
            entry: TILE_PATTERN | (PALETTE as u16) << 12,
        });
        f.cmds.push(Cmd::Bg {
            layer: layout::TEXT_BG,
            control: BgControl {
                enabled: true,
                priority: 0,
                char_block: layout::TEXT_CHAR_BLOCK,
                screen_block: screen_block(),
            },
        });
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.pattern as u8]);
        h.write_u16(self.t);
    }
}

const fn screen_block() -> u8 {
    layout::SCREEN_BLOCKS[layout::TEXT_BG as usize]
}

#[cfg(test)]
mod tests {
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
                    Cmd::MapEntries { start, entries, .. } => map
                        [*start as usize..*start as usize + entries.len()]
                        .copy_from_slice(entries),
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
                    Cmd::MapEntries { start, entries, .. } => half
                        [*start as usize..*start as usize + entries.len()]
                        .copy_from_slice(entries),
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
                        Cmd::MapFill { screen_block, .. }
                        | Cmd::MapEntries { screen_block, .. } => *screen_block == 31,
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
}
