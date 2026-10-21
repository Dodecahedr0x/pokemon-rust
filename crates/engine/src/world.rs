use crate::{Buttons, Rng, hash::Fnv64};

/// Virtual-PPU output of one frame. Grows into full layer/sprite commands later.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FrameOut {
    pub bg0_scroll: (u16, u16),
}

/// All mutable game state. Must stay deterministic: same state + same input => same next state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct World {
    frame: u32,
    scroll_x: u16,
    scroll_y: u16,
    rng: Rng,
}

impl World {
    pub const SEED: u32 = 0x5EED_0001;

    pub const fn new() -> Self {
        Self {
            frame: 0,
            scroll_x: 0,
            scroll_y: 0,
            rng: Rng::new(Self::SEED),
        }
    }

    pub fn step(&mut self, keys: Buttons) -> FrameOut {
        if keys.contains(Buttons::RIGHT) {
            self.scroll_x = self.scroll_x.wrapping_add(1);
        }
        if keys.contains(Buttons::LEFT) {
            self.scroll_x = self.scroll_x.wrapping_sub(1);
        }
        if keys.contains(Buttons::DOWN) {
            self.scroll_y = self.scroll_y.wrapping_add(1);
        }
        if keys.contains(Buttons::UP) {
            self.scroll_y = self.scroll_y.wrapping_sub(1);
        }
        self.rng.next_u32();
        self.frame = self.frame.wrapping_add(1);
        self.frame_out()
    }

    pub const fn frame_out(&self) -> FrameOut {
        FrameOut {
            bg0_scroll: (self.scroll_x, self.scroll_y),
        }
    }

    pub const fn frame(&self) -> u32 {
        self.frame
    }

    pub const fn scroll(&self) -> (u16, u16) {
        (self.scroll_x, self.scroll_y)
    }

    /// Field order is part of the parity contract; append new fields at the end.
    pub fn hash(&self) -> u64 {
        let mut h = Fnv64::new();
        h.write_u32(self.frame);
        h.write_u16(self.scroll_x);
        h.write_u16(self.scroll_y);
        h.write_u32(self.rng.state());
        h.finish()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Buttons;

    #[test]
    fn new_world_starts_at_frame_zero() {
        let w = World::new();
        assert_eq!(w.frame(), 0);
        assert_eq!(w.scroll(), (0, 0));
    }

    #[test]
    fn dpad_scrolls_and_wraps() {
        let mut w = World::new();
        let out = w.step(Buttons::RIGHT | Buttons::DOWN);
        assert_eq!(out.bg0_scroll, (1, 1));
        let out = w.step(Buttons::LEFT);
        assert_eq!(out.bg0_scroll, (0, 1));
        let out = w.step(Buttons::LEFT | Buttons::UP);
        assert_eq!(out.bg0_scroll, (u16::MAX, 0));
        assert_eq!(w.frame(), 3);
    }

    #[test]
    fn same_inputs_same_hash() {
        let inputs = [Buttons::RIGHT, Buttons::NONE, Buttons::A, Buttons::DOWN];
        let (mut a, mut b) = (World::new(), World::new());
        for i in inputs {
            a.step(i);
            b.step(i);
        }
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn different_inputs_different_hash() {
        let (mut a, mut b) = (World::new(), World::new());
        a.step(Buttons::RIGHT);
        b.step(Buttons::LEFT);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn rng_advances_every_frame() {
        let mut a = World::new();
        let h0 = a.hash();
        a.step(Buttons::NONE);
        assert_ne!(a.hash(), h0);
    }
}
