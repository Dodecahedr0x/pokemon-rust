//! Host shell: runs engine + content natively and draws each [`Frame`] with a software PPU.

pub mod ppu;

use engine::video::Frame;
pub use engine::video::{SCREEN_H, SCREEN_W};
use engine::{Buttons, Content, World};
pub use ppu::Ppu;

/// A native game instance: World + content + PPU + last frame.
///
/// Every step applies its frame's commands to the PPU; the picture is only drawn when
/// [`Session::framebuffer`] asks for it, so headless runs (explorers, long replays) skip
/// rendering.
///
/// Unlike the GBA shell there is no display lag: after step `n` the framebuffer shows the
/// `Frame` returned by step `n` (the ROM shows it one emulator frame later, see
/// `engine::debug`).
pub struct Session<C: Content> {
    content: C,
    world: World,
    ppu: Ppu,
    frame: Frame,
    fb: Vec<u16>,
    /// `fb` shows `frame`.
    drawn: bool,
}

impl<C: Content> Session<C> {
    pub fn new(content: C) -> Self {
        let world = World::new();
        let frame = world.initial_frame(&content);
        let mut ppu = Ppu::new();
        ppu.apply(&frame);
        Self {
            content,
            world,
            ppu,
            frame,
            fb: vec![0; SCREEN_W * SCREEN_H],
            drawn: false,
        }
    }

    pub fn step(&mut self, keys: Buttons) {
        self.frame = self.world.step(keys);
        self.ppu.apply(&self.frame);
        self.drawn = false;
    }

    pub fn content(&self) -> &C {
        &self.content
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    /// The last frame (its commands already applied).
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The picture of the last frame, rendered on first request.
    pub fn framebuffer(&mut self) -> &[u16] {
        if !self.drawn {
            self.ppu.render(&self.frame, &mut self.fb);
            self.drawn = true;
        }
        &self.fb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Buttons;
    use engine::scene::{MAP_TILES, map_tile_index, tile_pixel};

    /// The M1 picture as the pre-Frame renderer drew it: BG0 only, palette bank 0, no flips.
    fn reference(scene: &engine::Scene, scroll: (u16, u16)) -> Vec<u16> {
        let mut fb = vec![0; SCREEN_W * SCREEN_H];
        for y in 0..SCREEN_H {
            let py = (y + scroll.1 as usize) % (MAP_TILES * 8);
            for x in 0..SCREEN_W {
                let px = (x + scroll.0 as usize) % (MAP_TILES * 8);
                let entry = scene.map[(py / 8) * MAP_TILES + px / 8];
                let idx = tile_pixel(&scene.tiles[map_tile_index(entry)], px % 8, py % 8);
                fb[y * SCREEN_W + x] = scene.palette[idx as usize] & 0x7FFF;
            }
        }
        fb
    }

    #[test]
    fn session_matches_the_m1_reference_renderer() {
        let mut s = Session::new(content::Emerald);
        let scene = content::Emerald.scene();
        assert_eq!(s.framebuffer(), reference(scene, (0, 0)));
        for keys in [Buttons::RIGHT, Buttons::DOWN, Buttons::LEFT, Buttons::LEFT] {
            s.step(keys);
            assert!(s.frame().cmds.is_empty());
            let want = reference(scene, s.world().scroll());
            assert_eq!(s.framebuffer(), want);
        }
        assert_eq!(s.world().scroll(), (u16::MAX, 1));
    }

    #[test]
    fn headless_steps_render_only_on_request() {
        let mut s = Session::new(content::Emerald);
        for _ in 0..5 {
            s.step(Buttons::DOWN);
        }
        assert!(!s.drawn);
        let want = reference(content::Emerald.scene(), (0, 5));
        assert_eq!(s.framebuffer(), want);
        assert!(s.drawn);
    }

    #[test]
    fn session_steps_and_renders() {
        let mut s = Session::new(content::Emerald);
        let before = s.framebuffer().to_vec();
        s.step(Buttons::RIGHT);
        assert_eq!(s.world().frame(), 1);
        let after = s.framebuffer();
        assert_eq!(after.len(), SCREEN_W * SCREEN_H);
        // A one-pixel right scroll shifts the image left by one.
        for y in [0, 37, 80, SCREEN_H - 1] {
            for x in 0..SCREEN_W - 1 {
                assert_eq!(after[y * SCREEN_W + x], before[y * SCREEN_W + x + 1]);
            }
        }
    }
}
