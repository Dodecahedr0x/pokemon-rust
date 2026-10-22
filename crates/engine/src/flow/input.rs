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

// Milestone 5 Task B.
impl Input {
    pub const fn snapshot(&self) -> crate::checkpoint::InputSnap {
        crate::checkpoint::InputSnap {
            held: self.held,
            pressed: self.pressed,
        }
    }

    pub const fn from_snapshot(s: crate::checkpoint::InputSnap) -> Input {
        Input {
            held: s.held,
            pressed: s.pressed,
        }
    }
}

#[cfg(test)]
mod tests;
