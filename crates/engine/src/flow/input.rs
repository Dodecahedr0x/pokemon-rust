//! Per-frame button edges.

use crate::data::Dir;
use crate::hash::Fnv64;
use crate::input::Buttons;

/// Buttons held this frame and those newly pressed since the previous frame (Gen3
/// `gMain.heldKeys` / `gMain.newKeys`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Input {
    pub held: Buttons,
    pub pressed: Buttons,
}

impl Input {
    pub const fn new(prev: Buttons, cur: Buttons) -> Self {
        Self {
            held: cur,
            pressed: Buttons(cur.0 & !prev.0),
        }
    }

    /// Next frame: `cur` becomes held, the previous `held` is the reference for edges.
    pub fn update(&mut self, cur: Buttons) {
        *self = Self::new(self.held, cur);
    }

    pub const fn is_pressed(&self, b: Buttons) -> bool {
        self.pressed.contains(b)
    }

    pub const fn is_held(&self, b: Buttons) -> bool {
        self.held.contains(b)
    }

    /// Held d-pad direction with Gen3 priority Up, Down, Left, Right (`FieldGetPlayerInput`).
    pub const fn dpad(&self) -> Option<Dir> {
        if self.held.contains(Buttons::UP) {
            Some(Dir::Up)
        } else if self.held.contains(Buttons::DOWN) {
            Some(Dir::Down)
        } else if self.held.contains(Buttons::LEFT) {
            Some(Dir::Left)
        } else if self.held.contains(Buttons::RIGHT) {
            Some(Dir::Right)
        } else {
            None
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.held.0);
        h.write_u16(self.pressed.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_is_the_rising_edge() {
        let mut i = Input::default();
        i.update(Buttons::A);
        assert!(i.is_pressed(Buttons::A) && i.is_held(Buttons::A));
        i.update(Buttons::A | Buttons::B);
        assert!(!i.is_pressed(Buttons::A) && i.is_held(Buttons::A));
        assert!(i.is_pressed(Buttons::B));
        i.update(Buttons::NONE);
        assert_eq!(i, Input::default());
        i.update(Buttons::A);
        assert!(i.is_pressed(Buttons::A), "re-press after release");
    }

    #[test]
    fn new_from_prev_and_cur() {
        let i = Input::new(Buttons::A | Buttons::UP, Buttons::UP | Buttons::B);
        assert_eq!(i.held, Buttons::UP | Buttons::B);
        assert_eq!(i.pressed, Buttons::B);
    }

    #[test]
    fn dpad_priority_up_down_left_right() {
        let d = |b| Input::new(Buttons::NONE, b).dpad();
        assert_eq!(d(Buttons::NONE), None);
        assert_eq!(d(Buttons::RIGHT), Some(Dir::Right));
        assert_eq!(d(Buttons::RIGHT | Buttons::LEFT), Some(Dir::Left));
        assert_eq!(d(Buttons::LEFT | Buttons::DOWN), Some(Dir::Down));
        assert_eq!(
            d(Buttons::DOWN | Buttons::UP | Buttons::RIGHT),
            Some(Dir::Up)
        );
    }

    #[test]
    fn hash_covers_both_fields() {
        let h = |i: Input| {
            let mut h = Fnv64::new();
            i.hash(&mut h);
            h.finish()
        };
        let a = Input::new(Buttons::NONE, Buttons::A);
        let b = Input::new(Buttons::A, Buttons::A);
        assert_ne!(h(a), h(b));
    }
}
