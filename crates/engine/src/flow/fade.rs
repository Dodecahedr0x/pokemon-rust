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
        match fade {
            Fade::ToBlack => self.target = FADE_MAX,
            Fade::FromBlack => {
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

    pub fn hash_into(&self, h: &mut Fnv64) {
        h.write(&[self.level, self.target]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_visible_and_idle() {
        let f = Fader::new();
        assert_eq!(f.level(), 0);
        assert!(f.is_done());
    }

    #[test]
    fn to_black_takes_16_frames() {
        let mut f = Fader::new();
        f.start(Fade::ToBlack);
        assert!(!f.is_done());
        let levels: alloc::vec::Vec<u8> = (0..16).map(|_| f.update()).collect();
        assert_eq!(levels, (1..=16).collect::<alloc::vec::Vec<u8>>());
        assert!(f.is_done());
        assert_eq!(f.update(), FADE_MAX, "stays black");
    }

    #[test]
    fn from_black_snaps_black_then_takes_16_frames() {
        let mut f = Fader::new();
        f.start(Fade::FromBlack);
        assert_eq!(f.level(), FADE_MAX);
        for i in 1..=16u8 {
            assert!(!f.is_done());
            assert_eq!(f.update(), 16 - i);
        }
        assert!(f.is_done());
        assert_eq!(f.update(), 0);
    }

    #[test]
    fn to_black_from_partial_level_continues() {
        let mut f = Fader::new();
        f.start(Fade::FromBlack);
        for _ in 0..6 {
            f.update();
        }
        f.start(Fade::ToBlack);
        assert_eq!(f.update(), 11);
    }

    #[test]
    fn hash_covers_level_and_target() {
        let h = |f: &Fader| {
            let mut h = Fnv64::new();
            f.hash_into(&mut h);
            h.finish()
        };
        let a = Fader::new();
        let mut b = Fader::new();
        b.start(Fade::ToBlack);
        let mut c = b;
        c.update();
        assert_ne!(h(&a), h(&b));
        assert_ne!(h(&b), h(&c));
    }
}
