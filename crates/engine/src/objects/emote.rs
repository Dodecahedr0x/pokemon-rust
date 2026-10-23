//! Emote bubbles over objects: Gen3 field effects `FLDEFF_EXCLAMATION_MARK_ICON`,
//! `FLDEFF_QUESTION_MARK_ICON` and `FLDEFF_HEART_ICON` (`trainer_see.c`).
//!
//! A bubble lives [`EMOTE_FRAMES`] + 1 frames: its sprite animation is one 60-frame frame then
//! `ANIMCMD_END`, and `SpriteCB_TrainerIcons` stops the effect on the frame after the animation
//! ended, or as soon as its object is gone. Each frame its y offset bounces (`sYVelocity` from
//! -5, `sYOffset += sYVelocity`, the velocity growing until the offset is back to 0).
//!
//! **Palette.** Gen3 draws `!` and `?` with OBJ palette 0 (the player's, `paletteNum = 0` with
//! no tag) and the heart with palette 2 (`FldEff_HeartIcon` forces `paletteNum = 2`). The engine
//! has no fixed banks: it draws the bubble with the palette of the sprite `GameData::emote`
//! returns, through the same bank allocator as objects (so a bubble whose palette equals an
//! object's shares that bank). The exported pics carry the `npc_1` palette: `!` and `?` use
//! indices 0, 14 and 15, which hold the same colors in `npc_1` and in the player palettes, so
//! they draw exactly. The heart also uses index 5, whose Gen3 color is that of whatever palette
//! sits in bank 2: the `npc_1` color is an approximation there.

use super::{Object, Objects};
use crate::data::{Emote, MapId};
use crate::hash::Fnv64;

/// Frames of the bubble's animation frame (`ANIMCMD_FRAME(_, 60)`); it shows for one more.
pub const EMOTE_FRAMES: u8 = 60;
/// Bubbles shown at once; further ones are not started.
pub const EMOTES_MAX: usize = 4;
/// `SetIconSpriteData`: `sYVelocity = -5`.
const START_VELOCITY: i8 = -5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct EmoteFx {
    /// The object it follows (Gen3 `sLocalId`, `sMapNum`, `sMapGroup`).
    pub local_id: u8,
    pub map: MapId,
    pub kind: Emote,
    /// Sprite callbacks run so far.
    pub age: u8,
    pub y_off: i8,
    pub y_vel: i8,
    /// Render cache (not hashed): OBJ VRAM slot (0..[`EMOTES_MAX`]) and the sprite uploaded
    /// there (address).
    pub slot: u8,
    pub uploaded: Option<usize>,
}

impl Objects {
    /// Starts a bubble over object (`local`, `map`) (Gen3 `FieldEffectStart`). Its first sprite
    /// callback runs in this frame's [`Objects::update`].
    pub(crate) fn start_emote(&mut self, local: u8, map: MapId, kind: Emote) {
        let Some(slot) = (0..EMOTES_MAX as u8).find(|s| self.emotes.iter().all(|e| e.slot != *s))
        else {
            return;
        };
        self.emotes.push(EmoteFx {
            local_id: local,
            map,
            kind,
            age: 0,
            y_off: 0,
            y_vel: START_VELOCITY,
            slot,
            uploaded: None,
        });
    }

    /// Whether a bubble shows over the current map's object `local`.
    pub fn emote_active(&self, local: u8) -> bool {
        self.emotes
            .iter()
            .any(|e| e.local_id == local && e.map == self.map)
    }

    /// The bubbles' sprite callbacks (`SpriteCB_TrainerIcons`), after the objects moved.
    pub(super) fn update_emotes(&mut self) {
        let objs = &self.objs;
        self.emotes.retain_mut(|e| {
            let alive = objs
                .iter()
                .any(|o: &Object| o.local_id == e.local_id && o.map == e.map);
            if !alive || e.age > EMOTE_FRAMES {
                return false;
            }
            e.y_off = e.y_off.wrapping_add(e.y_vel);
            e.y_vel = if e.y_off != 0 { e.y_vel + 1 } else { 0 };
            e.age += 1;
            true
        });
    }

    pub(super) fn hash_emotes(&self, h: &mut Fnv64) {
        h.write(&[self.emotes.len() as u8]);
        for e in &self.emotes {
            h.write(&[
                e.local_id,
                e.kind as u8,
                e.age,
                e.y_off as u8,
                e.y_vel as u8,
            ]);
            h.write_u16(e.map.0);
        }
    }
}
