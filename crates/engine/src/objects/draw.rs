//! Object sprites: OBJ VRAM slots, OBJ palette banks, and the `Obj` list.
//!
//! Objects own all of OBJ VRAM and the 16 OBJ palette banks. Each object gets a fixed run of
//! tiles (first fit) holding its current frame, re-uploaded only when the frame changes. A
//! palette goes to a bank already holding the same colors, else to an empty bank, else to the
//! lowest bank no object drawn this frame uses. An object whose palette finds no bank (more
//! than 16 distinct palettes on screen) is not drawn.
//!
//! Emote bubbles (Milestone 8) own the last [`EMOTES_MAX`] × [`EMOTE_TILES`] tiles, one fixed
//! slot each, and are drawn in front of every object (Gen3: OAM priority 1, above the field's
//! top layer and the objects).

use super::{EMOTES_MAX, Objects};
use crate::data::GameData;
use crate::video::{Cmd, Frame, MAX_OBJS, OBJ_TILES, Obj, SCREEN_H, SCREEN_W, layout};
use alloc::vec::Vec;

/// Tiles of one emote slot (a 16x16 bubble).
pub const EMOTE_TILES: u16 = 4;
/// First tile of the emote slots; objects use the tiles below.
const EMOTE_FIRST_TILE: u16 = OBJ_TILES as u16 - EMOTES_MAX as u16 * EMOTE_TILES;
/// Gen3 `sOamData_Icons.priority`.
const EMOTE_PRIORITY: u8 = 1;

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
        self.draw_virtuals(game, camera_px, &mut used_banks, &mut out, f);
        // Stable: equal y keeps object order, so the player wins ties.
        out.sort_by_key(|&(y, _)| core::cmp::Reverse(y));
        if !self.emotes.is_empty() {
            let bubbles = self.draw_emotes(game, camera_px, &mut used_banks, f);
            let room = MAX_OBJS.saturating_sub(f.objs.len());
            f.objs.extend(bubbles.into_iter().take(room));
        }
        let room = MAX_OBJS.saturating_sub(f.objs.len());
        f.objs.extend(out.into_iter().take(room).map(|(_, o)| o));
    }

    /// The bubbles' OBJs (Gen3 `SpriteCB_TrainerIcons`: centered 16 px above the object's
    /// sprite center, plus the bounce), and their uploads. A kind the game has no sprite for
    /// draws nothing.
    fn draw_emotes(
        &mut self,
        game: &impl GameData,
        camera_px: (i32, i32),
        used_banks: &mut u16,
        f: &mut Frame,
    ) -> Vec<Obj> {
        let mut out = Vec::new();
        for k in 0..self.emotes.len() {
            let e = self.emotes[k];
            let Some(sprite) = game.emote(e.kind) else {
                continue;
            };
            let Some(o) = self
                .objs
                .iter()
                .find(|o| o.local_id == e.local_id && o.map == e.map)
            else {
                continue;
            };
            if !o.visible {
                continue;
            }
            let Some((shape, size)) = Obj::shape_for(sprite.width, sprite.height) else {
                continue;
            };
            let Some(&tiles) = sprite.frames.first() else {
                continue;
            };
            if tiles.len() > EMOTE_TILES as usize {
                continue;
            }
            let height = o
                .gfx
                .map_or(game.player_sprite(), |g| game.sprite(g).unwrap_or(sprite))
                .height as i32;
            let (px, py) = o.pixel_pos();
            // The object sprite's center is (px + 8, py + 16 - h / 2) (Gen3 `sprite->y += 16 +
            // centerToCornerVecY`); the bubble's center is 16 px above it.
            let (w, h) = (sprite.width as i32, sprite.height as i32);
            let cx = px + 8 - camera_px.0;
            let cy = py + 16 - height / 2 - 16 - camera_px.1 + o.jump_y() as i32 + e.y_off as i32;
            let (x, y) = (cx - w / 2, cy - h / 2);
            if x <= -w || x >= SCREEN_W as i32 || y <= -h || y >= SCREEN_H as i32 {
                continue;
            }
            let Some(palette) = self.palette_bank(sprite.palette, used_banks, f) else {
                continue;
            };
            let first = EMOTE_FIRST_TILE + e.slot as u16 * EMOTE_TILES;
            let shown = Some(sprite as *const _ as usize);
            if self.emotes[k].uploaded != shown {
                f.cmds.push(Cmd::ObjTiles { first, tiles });
                self.emotes[k].uploaded = shown;
            }
            out.push(Obj {
                x: x as i16,
                y: y as i16,
                tile: first,
                palette,
                shape,
                size,
                hflip: false,
                vflip: false,
                priority: EMOTE_PRIORITY,
            });
        }
        out
    }

    /// The virtual objects' OBJs (Gen3 `CreateVirtualObject`: an object's sprite at a cell,
    /// standing, facing its direction), after the objects in the y sort's ties.
    fn draw_virtuals(
        &mut self,
        game: &impl GameData,
        camera_px: (i32, i32),
        used_banks: &mut u16,
        out: &mut Vec<(i32, Obj)>,
        f: &mut Frame,
    ) {
        for k in 0..self.virtuals.len() {
            let v = self.virtuals[k];
            let Some(sprite) = game.sprite(v.gfx) else {
                continue;
            };
            let Some((shape, size)) = Obj::shape_for(sprite.width, sprite.height) else {
                continue;
            };
            if sprite.frames.is_empty() {
                continue;
            }
            let (w, h) = (sprite.width as i32, sprite.height as i32);
            let (px, py) = (v.x as i32 * 16, v.y as i32 * 16);
            let x = px - camera_px.0 + 8 - w / 2;
            let y = py - camera_px.1 + 16 - h;
            if x <= -w || x >= SCREEN_W as i32 || y <= -h || y >= SCREEN_H as i32 {
                continue;
            }
            // Gen3 `GetFaceDirectionAnimNum`: the standing frame of the direction.
            let (frame, hflip) = super::anim::face_frame(v.facing, sprite.frames.len());
            let tiles = sprite.frames[frame.min(sprite.frames.len() - 1)];
            let count = ((w / 8) * (h / 8)) as u16;
            let first = match v.vram {
                Some((first, n)) if n == count => first,
                _ => {
                    let Some(first) = self.free_tiles(None, count) else {
                        continue;
                    };
                    self.virtuals[k].vram = Some((first, count));
                    self.virtuals[k].uploaded = None;
                    first
                }
            };
            let Some(palette) = self.palette_bank(sprite.palette, used_banks, f) else {
                continue;
            };
            let shown = Some((sprite as *const _ as usize, frame as u16));
            if self.virtuals[k].uploaded != shown {
                f.cmds.push(Cmd::ObjTiles { first, tiles });
                self.virtuals[k].uploaded = shown;
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
                    priority: layout::OBJ_PRIORITY,
                },
            ));
        }
    }

    /// Forgets what OBJ VRAM and palettes hold, so everything is uploaded again on the next
    /// `draw` (after the shell's VRAM was reset).
    pub fn invalidate_vram(&mut self) {
        for o in &mut self.objs {
            o.uploaded = None;
        }
        for e in &mut self.emotes {
            e.uploaded = None;
        }
        for v in &mut self.virtuals {
            v.uploaded = None;
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
        let first = self.free_tiles(Some(i), count)?;
        let o = &mut self.objs[i];
        o.vram = Some((first, count));
        o.uploaded = None;
        Some(first)
    }

    /// The first run of `count` free OBJ tiles (first fit) below the emote slots, besides
    /// object `except`'s own and the virtual objects'.
    fn free_tiles(&self, except: Option<usize>, count: u16) -> Option<u16> {
        let mut used: Vec<(u16, u16)> = (self.objs.iter().enumerate())
            .filter(|&(j, _)| Some(j) != except)
            .filter_map(|(_, o)| o.vram)
            .chain(self.virtuals.iter().filter_map(|v| v.vram))
            .collect();
        used.sort_unstable();
        let mut first = 0u16;
        for (start, n) in used {
            if start >= first + count {
                break;
            }
            first = first.max(start + n);
        }
        (first + count <= EMOTE_FIRST_TILE).then_some(first)
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
