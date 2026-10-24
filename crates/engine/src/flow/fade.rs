//! Screen fade to and from black.
//!
//! Gen3 `FadeScreen(FADE_TO_BLACK, 0)` blends with `deltaY = 2`, BG and OBJ palettes on alternate
//! frames (`UpdateNormalPaletteFade`), so the level moves 0 -> 16 in 16 frames. We move one level
//! per frame for the same duration.

use crate::hash::Fnv64;
use crate::script::Fade;

/// Fully black.
pub const FADE_MAX: u8 = 16;

/// Darkening level: 0 = normal picture, [`FADE_MAX`] = black (GBA `BLDY` units).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Fader {
    level: u8,
    target: u8,
    /// The level brightens towards white (Gen3 `FADE_TO_WHITE` / `FADE_FROM_WHITE`). Cleared
    /// once a fade back ends at the normal picture.
    white: bool,
}

impl Fader {
    pub const fn new() -> Self {
        Self {
            level: 0,
            target: 0,
            white: false,
        }
    }

    /// `ToBlack` fades from the current level; `FromBlack` first snaps to black, as Gen3
    /// `FillPalBufferBlack` before `FADE_FROM_BLACK`.
    pub fn start(&mut self, fade: Fade) {
        // The white fades brighten (`Frame::lighten`) with the black fades' timing.
        self.white = matches!(fade, Fade::ToWhite | Fade::FromWhite);
        match fade {
            Fade::ToBlack | Fade::ToWhite => self.target = FADE_MAX,
            Fade::FromBlack | Fade::FromWhite => {
                self.level = FADE_MAX;
                self.target = 0;
            }
        }
    }

    /// Advances one frame and returns the level to display.
    pub fn update(&mut self) -> u8 {
        if self.level < self.target {
            self.level += 1;
        } else if self.level > self.target {
            self.level -= 1;
        }
        if self.level == 0 && self.target == 0 {
            self.white = false;
        }
        self.level
    }

    /// Whether the level brightens towards white (`Frame::lighten`) rather than darkens: a
    /// `ToWhite` / `FromWhite` fade. `draw_field` copies it into the frame.
    pub const fn is_white(&self) -> bool {
        self.white
    }

    pub const fn level(&self) -> u8 {
        self.level
    }

    pub const fn is_done(&self) -> bool {
        self.level == self.target
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.level, self.target]);
        // Only while white, so the black fades keep their bytes.
        if self.white {
            h.write(&[0xFF]);
        }
    }
}

// Milestone 5 Task B.
impl Fader {
    pub fn snapshot(&self) -> crate::checkpoint::FaderSnap {
        crate::checkpoint::FaderSnap {
            level: self.level,
            target: self.target,
        }
    }

    pub const fn from_snapshot(s: crate::checkpoint::FaderSnap) -> Fader {
        Fader {
            level: s.level,
            target: s.target,
            white: false,
        }
    }
}

#[cfg(test)]
mod tests;
