//! Screen effects scripts start (SIL2's Wave 0 requests; Milestone 9 Task R owns the file):
//! a full-screen flash (`ScreenFlash`), a camera shake (`ScreenShake`) and a palette tint
//! (`SetTint`).
//!
//! [`apply`] runs last in `World::step` and post-processes the frame's palette uploads and
//! scroll offsets, so an effect survives every field restore, map load and screen return
//! without an edit in `field.rs`, `world/screens.rs` or `world/battle.rs`. Wave 0 declares
//! the state and calls the no-op [`apply`]; the state is not snapshotted (a save or a capture
//! needs it clear) and is hashed only while an effect is active.

use crate::hash::Fnv64;
use crate::video::Frame;

/// A full-screen colour shown for `frames`, then back (Gen3 `BeginNormalPaletteFade` to a
/// colour and back).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flash {
    /// BGR555.
    pub color: u16,
    pub frames: u16,
    /// Frames shown so far.
    pub t: u16,
}

/// BG and OBJ offsets alternating ±`magnitude` every frame, ending at 0 (Gen3 `ShakeCamera`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shake {
    pub magnitude: u8,
    pub frames: u16,
    pub t: u16,
}

/// A blend of palette banks towards a colour (Gen3 `BlendPalettes`-style), stepped over
/// `frames`; kept until cleared with amount 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tint {
    /// BG palette banks, one bit each.
    pub banks: u16,
    /// OBJ palette banks, one bit each.
    pub obj_banks: u16,
    /// BGR555.
    pub color: u16,
    /// Target blend, 0..=16.
    pub amount: u8,
    /// The blend reached so far, 0..=16.
    pub current: u8,
    pub frames: u16,
    pub t: u16,
}

/// The active screen effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScreenFx {
    pub flash: Option<Flash>,
    pub shake: Option<Shake>,
    pub tint: Option<Tint>,
}

impl ScreenFx {
    /// Whether any effect is active.
    pub fn is_active(&self) -> bool {
        self.flash.is_some() || self.shake.is_some() || self.tint.is_some()
    }

    /// Feeds the effects into `h`, only while one is active.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        if !self.is_active() {
            return;
        }
        h.write(&[0xA5]);
        match self.flash {
            None => h.write(&[0]),
            Some(f) => {
                h.write(&[1]);
                for v in [f.color, f.frames, f.t] {
                    h.write_u16(v);
                }
            }
        }
        match self.shake {
            None => h.write(&[0]),
            Some(s) => {
                h.write(&[1, s.magnitude]);
                h.write_u16(s.frames);
                h.write_u16(s.t);
            }
        }
        match self.tint {
            None => h.write(&[0]),
            Some(t) => {
                h.write(&[1, t.amount, t.current]);
                for v in [t.banks, t.obj_banks, t.color, t.frames, t.t] {
                    h.write_u16(v);
                }
            }
        }
    }
}

/// Applies the effects to the frame about to be shown and advances them one frame (Task R:
/// `t`, the tint's `current`, an effect removed when it ends). Wave 0: nothing.
pub fn apply(fx: &mut ScreenFx, f: &mut Frame) {
    let _ = (fx, f);
}
