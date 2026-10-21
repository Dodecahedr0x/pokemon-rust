use crate::scene::MAP_TILES;
use crate::video::{BgControl, Cmd, Frame};
use crate::{Buttons, Content, Rng, hash::Fnv64};
use alloc::vec;

/// M1 scene placement: BG0, char block 0, screen block 31, palette bank 0.
const SCENE_BG: u8 = 0;
const SCENE_CHAR_BLOCK: u8 = 0;
const SCENE_SCREEN_BLOCK: u8 = 31;

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

    /// Frame 0: uploads the scene (palette, tiles, map, BG0 control) and sets the registers.
    /// Shells apply it before the first step.
    pub fn initial_frame(&self, content: &dyn Content) -> Frame {
        let scene = content.scene();
        let mut f = self.registers();
        f.cmds = vec![
            Cmd::BgPalette {
                bank: 0,
                colors: scene.palette,
            },
            Cmd::BgTiles {
                char_block: SCENE_CHAR_BLOCK,
                first: 0,
                tiles: scene.tiles,
            },
            Cmd::MapEntries {
                screen_block: SCENE_SCREEN_BLOCK,
                start: 0,
                entries: scene.map[..MAP_TILES * MAP_TILES].to_vec(),
            },
            Cmd::Bg {
                layer: SCENE_BG,
                control: BgControl {
                    enabled: true,
                    priority: 0,
                    char_block: SCENE_CHAR_BLOCK,
                    screen_block: SCENE_SCREEN_BLOCK,
                },
            },
        ];
        f
    }

    /// Steps the world; the returned frame only moves BG0 (the scene is uploaded once).
    pub fn step(&mut self, keys: Buttons) -> Frame {
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
        self.registers()
    }

    fn registers(&self) -> Frame {
        let mut f = Frame::default();
        f.scroll[SCENE_BG as usize] = (self.scroll_x, self.scroll_y);
        f
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
        assert_eq!(out.scroll[0], (1, 1));
        let out = w.step(Buttons::LEFT);
        assert_eq!(out.scroll[0], (0, 1));
        let out = w.step(Buttons::LEFT | Buttons::UP);
        assert_eq!(out.scroll[0], (u16::MAX, 0));
        assert_eq!(w.frame(), 3);
    }

    #[test]
    fn initial_frame_uploads_scene_and_steps_only_scroll() {
        struct One;
        static TILES: [crate::Tile; 1] = [[0x1111_1111; 8]];
        static MAP: [u16; 1024] = [0; 1024];
        static SCENE: crate::Scene = crate::Scene {
            palette: [7; 16],
            tiles: &TILES,
            map: &MAP,
        };
        impl Content for One {
            fn scene(&self) -> &crate::Scene {
                &SCENE
            }
        }
        let mut w = World::new();
        let f0 = w.initial_frame(&One);
        assert_eq!(f0.scroll[0], (0, 0));
        assert!(matches!(f0.cmds[0], Cmd::BgPalette { bank: 0, colors } if colors == [7; 16]));
        assert!(
            matches!(f0.cmds[1], Cmd::BgTiles { char_block: 0, first: 0, tiles } if tiles.len() == 1)
        );
        assert!(
            matches!(&f0.cmds[2], Cmd::MapEntries { screen_block: 31, start: 0, entries } if entries.len() == 1024)
        );
        assert!(matches!(f0.cmds[3], Cmd::Bg { layer: 0, control } if control.enabled));
        let f1 = w.step(Buttons::RIGHT);
        assert!(f1.cmds.is_empty());
        assert_eq!(f1.scroll, [(1, 0), (0, 0), (0, 0), (0, 0)]);
        assert!(f1.objs.is_empty());
        assert_eq!(f1.brightness, 0);
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

    // Golden values pin the GBA/native parity contract; computed independently
    // (FNV-1a 64 over LE frame u32, scroll_x u16, scroll_y u16, rng state u32).
    #[test]
    fn seed_is_pinned() {
        assert_eq!(World::SEED, 0x5EED_0001);
    }

    #[test]
    fn golden_hash_initial() {
        assert_eq!(World::new().hash(), 0xb4ea_8cd1_c745_212f);
    }

    #[test]
    fn golden_hash_after_inputs() {
        let mut w = World::new();
        for i in [Buttons::RIGHT, Buttons::NONE, Buttons::A, Buttons::DOWN] {
            w.step(i);
        }
        assert_eq!(w.hash(), 0x701c_8d8b_3e66_1200);
    }

    #[test]
    fn rng_advances_every_frame() {
        let mut a = World::new();
        let h0 = a.hash();
        a.step(Buttons::NONE);
        assert_ne!(a.hash(), h0);
    }
}
