//! Host shell: runs engine + content natively, renders the virtual PPU in software.

use engine::scene::{MAP_TILES, map_tile_index, tile_pixel};
use engine::{Buttons, Content, FrameOut, Scene, World};

pub const SCREEN_W: usize = 240;
pub const SCREEN_H: usize = 160;
const MAP_PX: usize = MAP_TILES * 8;

/// Render BG0 into an RGB555 framebuffer, matching GBA text-BG semantics (256x256 wrap).
pub fn render(scene: &Scene, scroll: (u16, u16), fb: &mut [u16]) {
    for y in 0..SCREEN_H {
        let py = (y + scroll.1 as usize) % MAP_PX;
        for x in 0..SCREEN_W {
            let px = (x + scroll.0 as usize) % MAP_PX;
            let entry = scene.map[(py / 8) * MAP_TILES + px / 8];
            let tile = &scene.tiles[map_tile_index(entry)];
            let idx = tile_pixel(tile, px % 8, py % 8);
            fb[y * SCREEN_W + x] = scene.palette[idx as usize] & 0x7FFF;
        }
    }
}

/// A native game instance: World + content + last rendered frame.
pub struct Session<C: Content> {
    content: C,
    world: World,
    out: FrameOut,
    fb: Vec<u16>,
}

impl<C: Content> Session<C> {
    pub fn new(content: C) -> Self {
        let world = World::new();
        let out = world.frame_out();
        let mut s = Self {
            content,
            world,
            out,
            fb: vec![0; SCREEN_W * SCREEN_H],
        };
        s.redraw();
        s
    }

    pub fn step(&mut self, keys: Buttons) {
        self.out = self.world.step(keys);
        self.redraw();
    }

    fn redraw(&mut self) {
        render(self.content.scene(), self.out.bg0_scroll, &mut self.fb);
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn framebuffer(&self) -> &[u16] {
        &self.fb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Buttons, Scene, Tile};

    fn scene() -> Scene {
        let tiles: &'static [Tile] =
            Box::leak(Box::new([[0x1111_1111u32; 8], [0x2222_2222u32; 8]]));
        let mut map = [0u16; 1024];
        map[1] = 1; // tile column 1, row 0
        let mut palette = [0u16; 16];
        palette[1] = 0x001F;
        palette[2] = 0x7C00;
        Scene {
            palette,
            tiles,
            map: Box::leak(Box::new(map)),
        }
    }

    #[test]
    fn renders_tiles_at_zero_scroll() {
        let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
        render(&scene(), (0, 0), &mut fb);
        assert_eq!(fb[7], 0x001F);
        assert_eq!(fb[8], 0x7C00);
        assert_eq!(fb[16], 0x001F);
    }

    #[test]
    fn scroll_shifts_and_wraps_at_256() {
        let mut fb = vec![0u16; SCREEN_W * SCREEN_H];
        render(&scene(), (1, 0), &mut fb);
        assert_eq!(fb[7], 0x7C00);
        render(&scene(), (250, 0), &mut fb);
        assert_eq!(fb[0], 0x001F); // map x 250 -> tile 31
        assert_eq!(fb[14], 0x7C00); // map x 264 & 255 = 8 -> tile 1
    }

    #[test]
    fn session_steps_and_renders() {
        let mut s = Session::new(content::Emerald);
        s.step(Buttons::RIGHT);
        assert_eq!(s.world().frame(), 1);
        assert_eq!(s.framebuffer().len(), SCREEN_W * SCREEN_H);
    }
}
