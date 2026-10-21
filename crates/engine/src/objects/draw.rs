//! Object sprites: OBJ VRAM slots, OBJ palette banks, and the `Obj` list.
//!
//! Objects own all of OBJ VRAM and the 16 OBJ palette banks. Each object gets a fixed run of
//! tiles (first fit) holding its current frame, re-uploaded only when the frame changes. A
//! palette goes to a bank already holding the same colors, else to an empty bank, else to the
//! lowest bank no object drawn this frame uses. An object whose palette finds no bank (more
//! than 16 distinct palettes on screen) is not drawn.

use super::Objects;
use crate::data::GameData;
use crate::video::{Cmd, Frame, MAX_OBJS, OBJ_TILES, Obj, SCREEN_H, SCREEN_W, layout};
use alloc::vec::Vec;

impl Objects {
    /// Pushes the visible objects' sprites, lower y behind (ties: the player, then spawn order,
    /// in front), plus the VRAM and palette uploads they need. `camera_px` is the map pixel at
    /// the screen's top-left corner.
    pub fn draw(&mut self, game: &impl GameData, camera_px: (i32, i32), f: &mut Frame) {
        let mut out: Vec<(i32, Obj)> = Vec::new();
        let mut used_banks = 0u16;
        for i in 0..self.objs.len() {
            let o = &self.objs[i];
            if !o.visible {
                continue;
            }
            let sprite = match o.gfx {
                None => game.player_sprite(),
                Some(g) => match game.sprite(g) {
                    Some(s) => s,
                    None => continue,
                },
            };
            let Some((shape, size)) = Obj::shape_for(sprite.width, sprite.height) else {
                continue;
            };
            if sprite.frames.is_empty() {
                continue;
            }
            let (w, h) = (sprite.width as i32, sprite.height as i32);
            let (px, py) = o.pixel_pos();
            let x = px - camera_px.0 + 8 - w / 2;
            let y = py - camera_px.1 + 16 - h + o.jump_y() as i32;
            if x <= -w || x >= SCREEN_W as i32 || y <= -h || y >= SCREEN_H as i32 {
                continue;
            }
            let (frame, hflip) = o.anim.frame(o.facing, sprite.frames.len());
            let frame = frame.min(sprite.frames.len() - 1);
            let tiles = sprite.frames[frame];
            let Some(first) = self.vram_slot(i, ((w / 8) * (h / 8)) as u16) else {
                continue;
            };
            let Some(palette) = self.palette_bank(sprite.palette, &mut used_banks, f) else {
                continue;
            };
            let o = &mut self.objs[i];
            let shown = Some((sprite as *const _ as usize, frame as u16));
            if o.uploaded != shown {
                f.cmds.push(Cmd::ObjTiles { first, tiles });
                o.uploaded = shown;
            }
            out.push((
                py,
                Obj {
                    x: x as i16,
                    y: y as i16,
                    tile: first,
                    palette,
                    shape,
                    size,
                    hflip,
                    vflip: false,
                    priority: layout::OBJ_PRIORITY, // TODO: priority from elevation (Gen3 `sElevationToPriority`).
                },
            ));
        }
        // Stable: equal y keeps object order, so the player wins ties.
        out.sort_by_key(|&(y, _)| core::cmp::Reverse(y));
        let room = MAX_OBJS.saturating_sub(f.objs.len());
        f.objs.extend(out.into_iter().take(room).map(|(_, o)| o));
    }

    /// Forgets what OBJ VRAM and palettes hold, so everything is uploaded again on the next
    /// `draw` (after the shell's VRAM was reset).
    pub fn invalidate_vram(&mut self) {
        for o in &mut self.objs {
            o.uploaded = None;
        }
        self.banks = [None; 16];
    }

    /// First tile of object `i`'s slot of `count` tiles, allocating it first fit if needed.
    fn vram_slot(&mut self, i: usize, count: u16) -> Option<u16> {
        if let Some((first, n)) = self.objs[i].vram
            && n == count
        {
            return Some(first);
        }
        let mut used: Vec<(u16, u16)> = (self.objs.iter().enumerate())
            .filter(|&(j, _)| j != i)
            .filter_map(|(_, o)| o.vram)
            .collect();
        used.sort_unstable();
        let mut first = 0u16;
        for (start, n) in used {
            if start >= first + count {
                break;
            }
            first = first.max(start + n);
        }
        if first as usize + count as usize > OBJ_TILES {
            return None;
        }
        let o = &mut self.objs[i];
        o.vram = Some((first, count));
        o.uploaded = None;
        Some(first)
    }

    pub(super) fn palette_bank(
        &mut self,
        colors: &[u16; 16],
        used: &mut u16,
        f: &mut Frame,
    ) -> Option<u8> {
        let bank = match self.banks.iter().position(|b| b.as_ref() == Some(colors)) {
            Some(b) => b,
            None => {
                let b = (self.banks.iter().position(Option::is_none))
                    .or_else(|| (0..16).find(|b| *used & (1 << b) == 0))?;
                self.banks[b] = Some(*colors);
                f.cmds.push(Cmd::ObjPalette {
                    bank: b as u8,
                    colors: *colors,
                });
                b
            }
        };
        *used |= 1 << bank;
        Some(bank as u8)
    }
}
