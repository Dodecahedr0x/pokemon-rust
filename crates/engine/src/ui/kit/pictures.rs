//! Menu pictures and icons.
//!
//! The party, summary, bag, mart and Hall of Fame upload the graphics `GameData` answers
//! (`ui_gfx`, `mon_icon`, `item_icon`, a species' front pic) inside the frame's upload budget
//! and place the sprite on the frame its tiles finish arriving. OBJ priority is 0, in front of
//! the opaque menu windows. A game that answers `None` uploads nothing, so those screens stay
//! frames and text.

use crate::data::{Icon, UiPicture};
use crate::video::{BgControl, Cmd, Frame, Obj, Tile};
use alloc::vec::Vec;

/// Tiles of one picture streamed across frames. Not screen state: the screen keeps it inside
/// [`super::Gfx`], which a clone drops.
#[derive(Clone, Copy)]
pub struct Load {
    key: u32,
    done: u16,
    pal: bool,
    tick: u16,
}

impl Load {
    pub const fn new() -> Self {
        Self {
            key: u32::MAX,
            done: 0,
            pal: false,
            tick: 0,
        }
    }

    /// Uploads `spec` within `budget`. Once the palette and every tile are in, pushes the OBJ
    /// (every frame from then on; OAM is not sticky). A new `spec.key` starts over.
    pub fn step(&mut self, f: &mut Frame, budget: &mut usize, spec: &Spec<'_>) {
        let total = spec.tile_count();
        if total == 0 || spec.palette.is_empty() {
            return;
        }
        if self.key != spec.key {
            *self = Self::new();
            self.key = spec.key;
        }
        if !self.pal {
            if *budget < 32 {
                return;
            }
            let mut colors = [0u16; 16];
            for (i, c) in spec.palette.iter().take(16).enumerate() {
                colors[i] = *c;
            }
            f.cmds.push(Cmd::ObjPalette {
                bank: spec.pal_bank,
                colors,
            });
            *budget -= 32;
            self.pal = true;
        }
        while (self.done as usize) < total && *budget >= 32 {
            let tile = spec.tile(self.done as usize);
            f.cmds.push(Cmd::ObjTile {
                index: spec.base.wrapping_add(self.done),
                tile,
            });
            *budget -= 32;
            self.done += 1;
        }
        if (self.done as usize) < total {
            return;
        }
        let frames = u16::from(spec.frames.max(1));
        let frame = if spec.frame_tiles == 0 || frames <= 1 {
            0
        } else {
            (self.tick / 16) % frames
        };
        self.tick = self.tick.wrapping_add(1);
        let Some((shape, size)) = Obj::shape_for(spec.w, spec.h) else {
            return;
        };
        f.objs.push(Obj {
            x: spec.x,
            y: spec.y,
            tile: spec.base.wrapping_add(frame.wrapping_mul(spec.frame_tiles)),
            palette: spec.pal_bank,
            shape,
            size,
            priority: 0,
            ..Obj::default()
        });
    }
}

/// A background `UiPicture` streamed onto one BG layer. The layer is enabled on the frame the
/// picture finishes, and again each frame after (the enable is cheap; tiles stay in VRAM).
#[derive(Clone, Copy)]
pub struct Bg {
    key: u32,
    stage: u8,
    at: u16,
}

impl Bg {
    pub const fn new() -> Self {
        Self {
            key: u32::MAX,
            stage: 0,
            at: 0,
        }
    }

    /// `(layer, char block, screen block, priority)`, as [`crate::video::layout::menu`].
    pub fn step(
        &mut self,
        f: &mut Frame,
        budget: &mut usize,
        pic: Option<&UiPicture>,
        layer: (u8, u8, u8, u8),
        key: u32,
    ) {
        let Some(pic) = pic else {
            return;
        };
        if pic.tiles.len() < 32 || pic.map.len() < 2 || pic.palette.len() < 16 {
            return;
        }
        if self.key != key {
            *self = Self::new();
            self.key = key;
        }
        let (layer_i, char_block, screen, pri) = layer;
        let pals = (pic.palette.len() / 16).min(14);
        let tiles = pic.tiles.len() / 32;
        let entries = pic.map.len() / 2;
        if self.stage == 0 {
            while (self.at as usize) < pals && *budget >= 32 {
                let i = self.at as usize;
                let mut colors = [0u16; 16];
                colors.copy_from_slice(&pic.palette[i * 16..i * 16 + 16]);
                f.cmds.push(Cmd::BgPalette {
                    bank: self.at as u8,
                    colors,
                });
                *budget -= 32;
                self.at += 1;
            }
            if (self.at as usize) >= pals {
                self.stage = 1;
                self.at = 0;
            }
        }
        if self.stage == 1 {
            while (self.at as usize) < tiles && *budget >= 32 {
                let i = self.at as usize;
                let bytes = &pic.tiles[i * 32..i * 32 + 32];
                f.cmds.push(Cmd::BgTile {
                    char_block,
                    index: self.at,
                    tile: tile_from(bytes),
                });
                *budget -= 32;
                self.at += 1;
            }
            if (self.at as usize) >= tiles {
                self.stage = 2;
                self.at = 0;
            }
        }
        if self.stage == 2 {
            while (self.at as usize) < entries && *budget >= 2 {
                let room = (*budget / 2).min(entries - self.at as usize);
                let start = self.at as usize;
                let raw = &pic.map[start * 2..(start + room) * 2];
                let entries: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let n = entries.len() as u16;
                f.cmds.push(Cmd::MapEntries {
                    screen_block: screen,
                    start: self.at,
                    entries,
                });
                *budget -= usize::from(n) * 2;
                self.at += n;
            }
            if (self.at as usize) >= entries {
                self.stage = 3;
            }
        }
        if self.stage == 3 {
            f.cmds.push(Cmd::Bg {
                layer: layer_i,
                control: BgControl {
                    enabled: true,
                    priority: pri,
                    char_block,
                    screen_block: screen,
                },
            });
        }
    }
}

pub struct Spec<'a> {
    pub key: u32,
    pub palette: &'a [u16],
    pub base: u16,
    pub pal_bank: u8,
    pub x: i16,
    pub y: i16,
    pub w: u8,
    pub h: u8,
    /// Tiles per animation frame. 0: the OBJ always starts at `base`.
    pub frame_tiles: u16,
    pub frames: u8,
    src: Src<'a>,
}

enum Src<'a> {
    Tiles(&'a [Tile]),
    Bytes(&'a [u8]),
}

impl Spec<'_> {
    fn tile_count(&self) -> usize {
        match self.src {
            Src::Tiles(t) => t.len(),
            Src::Bytes(b) => b.len() / 32,
        }
    }

    fn tile(&self, i: usize) -> Tile {
        match self.src {
            Src::Tiles(t) => t[i],
            Src::Bytes(b) => tile_from(&b[i * 32..i * 32 + 32]),
        }
    }
}

fn tile_from(s: &[u8]) -> Tile {
    core::array::from_fn(|r| {
        u32::from_le_bytes([s[r * 4], s[r * 4 + 1], s[r * 4 + 2], s[r * 4 + 3]])
    })
}

/// A 32×32 icon. One frame is 512 bytes; `Icon::frames` selects how many of those to upload
/// (two at most, the party's reserved pair).
pub fn icon<'a>(icon: &'a Icon, base: u16, pal_bank: u8, x: i16, y: i16, key: u32) -> Spec<'a> {
    let want = usize::from(icon.frames.max(1)).min(2);
    let have = icon.tiles.len() / 512;
    let frames = want.min(have);
    let n = frames * 512;
    Spec {
        key,
        palette: icon.palette,
        base,
        pal_bank,
        x,
        y,
        w: 32,
        h: 32,
        frame_tiles: 16,
        frames: frames as u8,
        src: Src::Bytes(&icon.tiles[..n]),
    }
}

/// A static OBJ built from `'static` tiles (a species front pic, the trainer picture).
pub fn tiles<'a>(
    tiles: &'a [Tile],
    palette: &'a [u16],
    base: u16,
    pal_bank: u8,
    x: i16,
    y: i16,
    w: u8,
    h: u8,
    key: u32,
) -> Spec<'a> {
    Spec {
        key,
        palette,
        base,
        pal_bank,
        x,
        y,
        w,
        h,
        frame_tiles: 0,
        frames: 1,
        src: Src::Tiles(tiles),
    }
}

/// A static OBJ built from 4bpp bytes (the bag sprite's frame).
pub fn bytes<'a>(
    bytes: &'a [u8],
    palette: &'a [u16],
    base: u16,
    pal_bank: u8,
    x: i16,
    y: i16,
    w: u8,
    h: u8,
    key: u32,
) -> Spec<'a> {
    let n = bytes.len() / 32 * 32;
    Spec {
        key,
        palette,
        base,
        pal_bank,
        x,
        y,
        w,
        h,
        frame_tiles: 0,
        frames: 1,
        src: Src::Bytes(&bytes[..n]),
    }
}
