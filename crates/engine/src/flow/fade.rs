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
}

impl Fader {
    pub const fn new() -> Self {
        Self {
            level: 0,
            target: 0,
        }
    }

    /// `ToBlack` fades from the current level; `FromBlack` first snaps to black, as Gen3
    /// `FillPalBufferBlack` before `FADE_FROM_BLACK`.
    pub fn start(&mut self, fade: Fade) {
        // Milestone 9 Wave 0: the white fades run as the black ones until Task R lightens
        // them (`Frame::lighten`); no Emerald script issues them before Task J.
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
        self.level
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
        }
    }
}

#[cfg(test)]
mod tests;
