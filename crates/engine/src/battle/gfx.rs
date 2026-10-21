//! Battle visuals drawn with `video::layout::battle` (Task E).
//!
//! Components, each with `hash` and a per-frame `update(&mut self, f: &mut Frame)`; the scene
//! (Task K) drives them from `BattleEvent`s and calls every `update` once per frame:
//! - [`Background`]: the environment on ENV (`DrawMainBattleBackground`), loaded in chunks.
//! - [`MonSprite`] / [`TrainerSprite`]: 64x64 pics at the Gen3 coordinates, with the slides,
//!   send-out ball, faint, hit blink, lunge and palette tint animations.
//! - [`Healthbox`]: the singles healthboxes (`CreateBattlerHealthboxSprites`): nickname, gender,
//!   level, the player's HP numbers and status icon, drawn into the box tiles and uploaded as
//!   `Cmd::ObjTile` diffs. It also places the HP bar sprite.
//! - [`HpBar`] (48 px) and [`ExpBar`] (64 px): `MoveBattleBar` / `CalcNewBarValue`.
//!
//! Gfx data (`BattleGfx`) layout this module expects, matching Gen3 `gHealthboxElementsGfxTable`:
//! - `healthbox_player`: 128 tiles, two 64x64 halves in OBJ 1D order (`-mwidth 8 -mheight 8`);
//!   `healthbox_opponent`: 64 tiles, two 64x32 halves. Palette in `palettes[0]` of each.
//! - `hpbar`: `hpbar.png` then `hpbar_anim.png`: 0 black, 1 "H", 2 "P", then green, yellow and
//!   red bars of 0..=8 pixels (tiles 3.., 12.., 21..); the HP bar palette in `palettes[0]`.
//! - `expbar`: 0..=8 pixels (9 tiles).
//! - `status`: `status.png` (player, color 12) then `status2.png` (opponent, color 13): PSN,
//!   PRZ, SLP, FRZ, BRN, 3 tiles each.
//! - `misc`: `misc.png` (11 tiles, `HEALTHBOX_GFX_36..`) then `misc_frameend.png` (tile 11).
//! - `ball`: `gBallGfx_Poke`, 16x16 frames of 4 tiles (frame 0 is used); palette `palettes[0]`.
//! - `small_font`: `FONT_SMALL`, same glyph layout as the normal font, glyph `0x100 + n` for
//!   `{EXTRA_SYMBOL n}`.
//!
//! Positions are Gen3 sprite centres turned into OAM top-left corners (`centerToCornerVec` of
//! the sprite's template). Documented approximations: no affine scaling (the send-out ball pops
//! and the mon appears at full size), no move animations beyond lunge/blink/tint, no healthbox
//! bob while choosing an action, no caught-ball icon, no Nidoran gender special case.

mod background;
mod bar;
mod healthbox;
mod sprite;
#[cfg(test)]
mod test_gfx;

pub use background::{BG_CHUNK_TILES, Background};
pub use bar::{BarColor, ExpBar, HpBar, bar_color};
pub use healthbox::{HEALTHBOX_TILES_PER_FRAME, Healthbox};
pub use sprite::{MonSprite, TINT_STAT_DOWN, TINT_STAT_UP, TrainerSprite};

use super::Side;
use crate::hash::Fnv64;

/// The battle's OBJ VRAM (tile numbers) and OBJ palette banks.
///
/// Deviation from the Wave 0 comment in `video::layout::battle`: the player healthbox takes
/// 128 tiles (two 64x64 halves, as Gen3: its exp bar row is below 32 px), so the opponent box,
/// HP bars and ball move up, and the opponent box gets its own palette bank (its status color
/// lives in the box palette, as Gen3 colors 12/13).
pub mod obj {
    pub const ENEMY_PIC: u16 = 0;
    pub const PLAYER_PIC: u16 = 64;
    pub const TRAINER_PIC: u16 = 128;
    pub const PLAYER_BACK: u16 = 192;
    /// Two 64x64 halves.
    pub const PLAYER_BOX: u16 = 256;
    /// Two 64x32 halves.
    pub const ENEMY_BOX: u16 = 384;
    /// HP bar sprites, 8 tiles each: "HP" label (2) + 48 px bar (6).
    pub const PLAYER_BAR: u16 = 448;
    pub const ENEMY_BAR: u16 = 456;
    /// One 16x16 ball frame.
    pub const BALL: u16 = 464;
    /// First tile free for other battle uses (up to 512, then the starter screen area).
    pub const FREE: u16 = 468;

    pub const PAL_ENEMY_MON: u8 = 0;
    pub const PAL_PLAYER_MON: u8 = 1;
    pub const PAL_TRAINER: u8 = 2;
    pub const PAL_PLAYER_BACK: u8 = 3;
    pub const PAL_PLAYER_BOX: u8 = 4;
    pub const PAL_HP_BAR: u8 = 5;
    pub const PAL_BALL: u8 = 6;
    pub const PAL_ENEMY_BOX: u8 = 7;
}

const fn side_tag(side: Side) -> u8 {
    match side {
        Side::Player => 0,
        Side::Opponent => 1,
    }
}

fn hash_i16(h: &mut Fnv64, v: i16) {
    h.write_u16(v as u16);
}

/// Gen3 `StartAnimLinearTranslation` / `AnimTranslateLinear`: an offset that moves by `dist` px
/// in `frames` frames in 8.8 fixed point, the direction kept in bit 0 of the step.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Linear {
    frames: u16,
    step: u16,
    acc: u16,
}

impl Linear {
    fn new(dist: i16, frames: u16) -> Self {
        let frames = frames.max(1);
        let step = ((dist.unsigned_abs() as u32) << 8) as u16 / frames;
        let step = if dist < 0 { step | 1 } else { step & !1 };
        Linear {
            frames,
            step,
            acc: 0,
        }
    }

    /// One frame: the new offset, or `None` once finished (the frame after the last move).
    fn next(&mut self) -> Option<i16> {
        if self.frames == 0 {
            return None;
        }
        self.acc = self.acc.wrapping_add(self.step);
        self.frames -= 1;
        let v = (self.acc >> 8) as i16;
        Some(if self.step & 1 != 0 { -v } else { v })
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.frames);
        h.write_u16(self.step);
        h.write_u16(self.acc);
    }
}

#[cfg(test)]
mod tests {
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

    /// A drain with its numbers, a hit blink, a box shake and a tint in the same frames: every
    /// frame is valid and sends at most 16 `ObjTile`s.
    #[test]
    fn busiest_frames_stay_within_the_obj_tile_budget() {
        use super::super::BattleMon;
        use crate::data::SpeciesId;
        use crate::mon::{Gender, NAME_LEN, Status};
        use crate::video::{Cmd, Frame};
        let game = test_gfx::game(Some(test_gfx::mon_gfx(8, 0, 0)));
        let mon = BattleMon {
            party: 0,
            species: SpeciesId(1),
            nickname: [0xBB; NAME_LEN],
            level: 9,
            hp: 999,
            max_hp: 999,
            status: Status::None,
            gender: Gender::Female,
            moves: [None; 4],
            pp: [0; 4],
            max_pp: [0; 4],
        };
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
            boxes.set_hp(bar.value());
            boxes.update(&mut f);
            sprite.update(&mut f);
            let tiles = f
                .cmds
                .iter()
                .filter(|c| matches!(c, Cmd::ObjTile { .. }))
                .count();
            assert!(tiles <= 16, "{tiles} ObjTiles");
            assert!(f.validate().is_ok());
        }
        assert_eq!(boxes.pending(), 0);
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
}
