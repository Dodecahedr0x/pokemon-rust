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
//! Upload budget: `load` methods are bulk uploads for black frames. On visible frames all
//! components share [`OBJ_TILES_PER_FRAME`]: update the sprites and bars first (fixed amounts),
//! then [`Healthbox::update`] and [`MonSprite::load_step`], which take what is left.
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
//! the sprite's template). The send-out uses `sAffineAnim_Battler_Emerge` (scale `0x28`, then
//! `+0x12` twelve times) and move scripts run in [`super::anim`]. Approximations: no healthbox
//! bob while choosing an action, no caught-ball icon, no Nidoran gender special case.

mod background;
mod bar;
mod healthbox;
mod sprite;
#[cfg(test)]
pub(crate) mod test_gfx;

pub use background::{BG_CHUNK_TILES, Background};
pub use bar::{BarColor, ExpBar, HpBar, bar_color};
pub use healthbox::{HEALTHBOX_TILES_PER_FRAME, Healthbox};
pub use sprite::{MonSprite, TINT_STAT_DOWN, TINT_STAT_UP, TrainerSprite};

use super::Side;
use crate::hash::Fnv64;
use crate::video::{Cmd, Frame};

/// The battle's OBJ VRAM (tile numbers) and OBJ palette banks.
///
/// The player healthbox takes 128 tiles (two 64x64 halves, as Gen3: its exp bar row is below
/// 32 px), and the opponent box has its own palette bank (its status color lives in the box
/// palette, as Gen3 colors 12/13); the `video::layout::battle` comment matches.
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

    // Doubles: the right battlers (512.., the starter screen's area, unused in battle).
    pub const ENEMY2_PIC: u16 = 512;
    pub const PLAYER2_PIC: u16 = 576;
    /// Two 64x64 halves (only their top 64x32 shows in doubles).
    pub const PLAYER2_BOX: u16 = 640;
    pub const ENEMY2_BOX: u16 = 768;
    pub const PLAYER2_BAR: u16 = 832;
    pub const ENEMY2_BAR: u16 = 840;
    pub const PAL_ENEMY2_MON: u8 = 8;
    pub const PAL_PLAYER2_MON: u8 = 9;
    pub const PAL_PLAYER2_BOX: u8 = 10;
    pub const PAL_ENEMY2_BOX: u8 = 11;
}

/// OBJ tiles a visible battle frame may upload in all (`Cmd::ObjTile`s, plus the tiles of
/// `Cmd::ObjTiles` from [`MonSprite::load_step`]). Bars (6 or 8) and the faint erase (8) send
/// fixed amounts; [`Healthbox`] and [`MonSprite::load_step`] take only what is left, so the
/// scene updates them last.
pub const OBJ_TILES_PER_FRAME: usize = 16;

/// OBJ tiles already queued in `f`.
pub fn obj_tiles_sent(f: &Frame) -> usize {
    f.cmds
        .iter()
        .map(|c| match c {
            Cmd::ObjTile { .. } => 1,
            Cmd::ObjTiles { tiles, .. } => tiles.len(),
            _ => 0,
        })
        .sum()
}

/// What is left of [`OBJ_TILES_PER_FRAME`] in `f`.
pub fn obj_tiles_left(f: &Frame) -> usize {
    OBJ_TILES_PER_FRAME.saturating_sub(obj_tiles_sent(f))
}

const fn side_tag(side: Side) -> u8 {
    side.code()
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
mod tests;
