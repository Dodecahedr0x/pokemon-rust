//! The cable-car ride (Gen3 `cable_car.c`), played by [`crate::script::Request::CableCar`].
//!
//! `CableCar` has already faded the field to black. This draws the ride for
//! [`RIDE_FRAMES`] (timer 1 through 602) and the script then warps, as `CB2_EndCableCar` does.
//!
//! - **The car.** The gondola, its door and the nine cable links. They move with Gen3's
//!   per-frame deltas (`SpriteCB_CableCar`: `0.14` and `0.067` pixels a frame, truncating)
//!   and `gSpriteCoordOffsetX` (`Task_AnimateBgGoingUp` / `GoingDown`).
//! - **The view.** BG0 ground, BG1 trees, BG2 mountains and BG3 pylons scroll with
//!   `Task_AnimateBgGoingUp` / `GoingDown` (the ground's 16-pixel segment step included).
//! - **The arrival.** Timer 570 starts the fade to black (`BeginNormalPaletteFade(…, 3, 0, 16)`);
//!   timer 602 is black, and the next frame the script warps.
//!
//! The opening fade-in (`CB2_LoadCableCar` case 8, the same 32-frame fade) runs over timers
//! 1..=32, on top of the field's fade to black.
//!
//! Deviations: no affine sprites (the gondola is 64×64, not a doubled 64×64), no ash weather
//! and no hiker, so the ride draws no `Random`. The pictures are drawn here, not from the ROM.

use crate::video::{BG_LAYERS, BgControl, Cmd, Frame, Obj, ObjShape, Tile};
use alloc::vec::Vec;

/// `Task_CableCar` fades out at timer 570. That fade is 32 frames, so the ride is 602.
pub const RIDE_FRAMES: u16 = 570 + 32;
/// Timer at which the arrival fade starts.
pub const ARRIVAL_TIMER: u16 = 570;
const FADE_FRAMES: u16 = 32;
/// Going down, `gSpriteCoordOffsetX` advances only while `timer < weatherDelay` (265).
const DOWN_OFFSET_FRAMES: u16 = 265;

const CAR_UP: (i16, i16) = (176, 43);
const CAR_DOWN: (i16, i16) = (104, 9);
const DOOR_UP: (i16, i16) = (200, 99);
const DOOR_DOWN: (i16, i16) = (128, 65);

const CAR_TILE: u16 = 0;
const DOOR_TILE: u16 = 64;
const CABLE_TILE: u16 = 66;

/// One frame of the ride, from the delay still left (`RIDE_FRAMES` on the first frame).
pub(crate) struct Pose {
    pub car: (i16, i16),
    pub door: (i16, i16),
    pub sprite_off: u16,
    pub scroll: [(u16, u16); BG_LAYERS],
    pub brightness: u8,
}

struct Sim {
    bg0h: u8,
    bg0v: u8,
    bg1h: u8,
    bg1v: u8,
    bg3h: u8,
    bg3v: u8,
    gx: u8,
    gy: u8,
    gxb: u8,
    gyb: u8,
    sprite_off: u16,
}

impl Sim {
    fn start(down: bool) -> Self {
        let (bg3h, bg3v, bg1v) = if down { (96, 232, 4) } else { (176, 16, 80) };
        Self {
            bg0h: 0,
            bg0v: 0,
            bg1h: 0,
            bg1v,
            bg3h,
            bg3v,
            gx: 0,
            gy: 0,
            gxb: 0,
            gyb: 0,
            sprite_off: 0,
        }
    }

    /// One frame of `Task_AnimateBgGoingUp` / `GoingDown` plus the ground scroll.
    fn step(&mut self, down: bool, timer: u16) {
        if down {
            self.bg3h = self.bg3h.wrapping_add(1);
            if timer % 2 == 0 {
                self.bg3v = self.bg3v.wrapping_add(1);
            }
            if timer % 8 == 0 {
                self.bg1h = self.bg1h.wrapping_add(1);
                self.bg1v = self.bg1v.wrapping_add(1);
            }
            if self.bg3h == 16 {
                self.bg3v = 192;
            }
        } else {
            self.bg3h = self.bg3h.wrapping_sub(1);
            if timer % 2 == 0 {
                self.bg3v = self.bg3v.wrapping_sub(1);
            }
            if timer % 8 == 0 {
                self.bg1h = self.bg1h.wrapping_sub(1);
                self.bg1v = self.bg1v.wrapping_sub(1);
            }
            if self.bg3h == 16 {
                self.bg3v = 64;
            }
        }
        self.ground(down);
        if down && timer < DOWN_OFFSET_FRAMES {
            self.sprite_off = (self.sprite_off + 247) % 248;
        } else if !down {
            self.sprite_off = (self.sprite_off + 1) % 128;
        }
    }

    /// `AnimateGroundGoingUp` / `GoingDown`: one pixel across, and down every fourth.
    fn ground(&mut self, down: bool) {
        if down {
            self.bg0h = self.gxb.wrapping_add(self.gx);
            self.bg0v = self.gyb.wrapping_add(self.gy);
        } else {
            self.bg0h = self.gxb.wrapping_sub(self.gx);
            self.bg0v = self.gyb.wrapping_sub(self.gy);
        }
        self.gx = self.gx.wrapping_add(1);
        if self.gx % 4 == 0 {
            self.gy = self.gy.wrapping_add(1);
        }
        if self.gx > 16 {
            self.gxb = self.bg0h;
            self.gyb = self.bg0v;
            self.gx = 0;
            self.gy = 0;
        }
    }
}

fn timer_of(delay: u16) -> u16 {
    RIDE_FRAMES + 1 - delay
}

fn delta(timer: u16) -> (i16, i16) {
    let t = u32::from(timer);
    // `(u8)(0.14f * timer)` and `(u8)(0.067f * timer)`, truncating.
    ((t * 14 / 100) as i16, (t * 67 / 1000) as i16)
}

fn place(origin: (i16, i16), down: bool, timer: u16, sprite_off: u16) -> (i16, i16) {
    let (dx, dy) = delta(timer);
    let (x, y) = if down {
        (origin.0 + dx, origin.1 + dy)
    } else {
        (origin.0 - dx, origin.1 - dy)
    };
    (x - sprite_off as i16, y)
}

fn brightness(timer: u16) -> u8 {
    if timer <= FADE_FRAMES {
        (16 - timer * 16 / FADE_FRAMES) as u8
    } else if timer < ARRIVAL_TIMER {
        0
    } else {
        let t = timer - ARRIVAL_TIMER;
        ((t * 16) / FADE_FRAMES).min(16) as u8
    }
}

pub(crate) fn pose(down: bool, delay: u16) -> Pose {
    let timer = timer_of(delay);
    let mut sim = Sim::start(down);
    for t in 1..=timer {
        sim.step(down, t);
    }
    let (car_at, door_at) = if down {
        (CAR_DOWN, DOOR_DOWN)
    } else {
        (CAR_UP, DOOR_UP)
    };
    Pose {
        car: place(car_at, down, timer, sim.sprite_off),
        door: place(door_at, down, timer, sim.sprite_off),
        sprite_off: sim.sprite_off,
        scroll: [
            (u16::from(sim.bg0h), u16::from(sim.bg0v)),
            (u16::from(sim.bg1h), u16::from(sim.bg1v)),
            (0, 0),
            (u16::from(sim.bg3h), u16::from(sim.bg3v)),
        ],
        brightness: brightness(timer),
    }
}

const fn rgb(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

fn solid(color: u8) -> Tile {
    let row = u32::from(color & 0xF) * 0x1111_1111;
    [row; 8]
}

fn push_pixels(tiles: &mut Vec<Tile>, pixels: &[u8], w: usize, h: usize) {
    let (tw, th) = (w / 8, h / 8);
    for ty in 0..th {
        for tx in 0..tw {
            let mut tile = [0u32; 8];
            for row in 0..8 {
                let mut bits = 0u32;
                for col in 0..8 {
                    let px = u32::from(pixels[(ty * 8 + row) * w + tx * 8 + col] & 0xF);
                    bits |= px << (col * 4);
                }
                tile[row] = bits;
            }
            tiles.push(tile);
        }
    }
}

fn bg_tiles() -> Vec<Tile> {
    let mut mountain = solid(1);
    for row in 4..8 {
        mountain[row] = 0x2222_2222;
    }
    mountain[3] = 0x0012_2100;
    let mut pylon = solid(0);
    for row in &mut pylon {
        *row = 0x0055_5000;
    }
    let mut rock = solid(3);
    rock[7] = 0x6666_6666;
    alloc::vec![
        solid(0),
        solid(1),
        mountain,
        solid(3),
        solid(4),
        pylon,
        rock,
    ]
}

fn obj_tiles() -> Vec<Tile> {
    let mut car = [0u8; 64 * 64];
    let mut put = |x0: usize, y0: usize, x1: usize, y1: usize, c: u8| {
        for y in y0..y1 {
            for x in x0..x1 {
                car[y * 64 + x] = c;
            }
        }
    };
    put(10, 8, 54, 16, 4);
    put(6, 16, 58, 48, 1);
    put(12, 22, 30, 36, 2);
    put(34, 22, 52, 36, 2);
    put(8, 48, 56, 54, 3);
    put(14, 50, 26, 60, 3);
    put(38, 50, 50, 60, 3);
    let mut tiles = Vec::with_capacity(70);
    push_pixels(&mut tiles, &car, 64, 64);
    let mut door = [0u8; 16 * 8];
    for y in 0..8 {
        for x in 0..16 {
            door[y * 16 + x] = if (2..14).contains(&x) && (1..7).contains(&y) {
                2
            } else {
                3
            };
        }
    }
    push_pixels(&mut tiles, &door, 16, 8);
    let mut cable = [0u8; 16 * 16];
    for i in 0..16 {
        cable[i * 16 + i] = 4;
        if i + 1 < 16 {
            cable[i * 16 + i + 1] = 4;
        }
    }
    push_pixels(&mut tiles, &cable, 16, 16);
    tiles
}

fn layer_map(tile_at: impl Fn(usize, usize) -> u16) -> Vec<u16> {
    let mut entries = Vec::with_capacity(32 * 32);
    for y in 0..32 {
        for x in 0..32 {
            entries.push(tile_at(x, y));
        }
    }
    entries
}

fn obj(x: i16, y: i16, tile: u16, shape: ObjShape, size: u8, priority: u8) -> Obj {
    Obj {
        x,
        y,
        tile,
        palette: 0,
        shape,
        size,
        hflip: false,
        vflip: false,
        priority,
    }
}

/// Replaces `f` with this frame of the ride. `delay` is the frames still left, counting this one.
pub(crate) fn paint(f: &mut Frame, down: bool, delay: u16) {
    if delay == 0 || delay > RIDE_FRAMES {
        return;
    }
    let pose = pose(down, delay);
    let off = pose.sprite_off;
    f.cmds.clear();
    f.objs.clear();
    f.scroll = pose.scroll;
    f.brightness = pose.brightness;
    f.lighten = false;
    f.cmds.push(Cmd::BgPalette {
        bank: 0,
        colors: [
            rgb(16, 22, 31),
            rgb(16, 22, 31),
            rgb(18, 14, 12),
            rgb(22, 18, 10),
            rgb(6, 16, 6),
            rgb(8, 8, 10),
            rgb(14, 10, 6),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    });
    f.cmds.push(Cmd::ObjPalette {
        bank: 0,
        colors: [
            0,
            rgb(28, 6, 6),
            rgb(26, 28, 31),
            rgb(6, 6, 8),
            rgb(20, 20, 22),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    });
    for (i, tile) in bg_tiles().into_iter().enumerate() {
        f.cmds.push(Cmd::BgTile {
            char_block: 0,
            index: i as u16,
            tile,
        });
    }
    for (i, tile) in obj_tiles().into_iter().enumerate() {
        f.cmds.push(Cmd::ObjTile {
            index: i as u16,
            tile,
        });
    }
    let maps = [
        (
            0u8,
            28u8,
            1u8,
            layer_map(|x, y| if (x + y) % 5 == 0 { 6 } else { 3 }),
        ),
        (1, 29, 2, layer_map(|_, y| if y >= 18 { 4 } else { 0 })),
        (
            2,
            30,
            3,
            layer_map(|_, y| {
                if y < 12 {
                    1
                } else if y < 22 {
                    2
                } else {
                    1
                }
            }),
        ),
        (
            3,
            31,
            0,
            layer_map(|x, y| if x % 16 == 0 && y > 2 { 5 } else { 0 }),
        ),
    ];
    for (layer, screen_block, priority, entries) in maps {
        f.cmds.push(Cmd::MapEntries {
            screen_block,
            start: 0,
            entries,
        });
        f.cmds.push(Cmd::Bg {
            layer,
            control: BgControl {
                enabled: true,
                priority,
                char_block: 0,
                screen_block,
            },
        });
    }
    f.objs.push(obj(
        pose.door.0,
        pose.door.1,
        DOOR_TILE,
        ObjShape::Wide,
        0,
        1,
    ));
    f.objs.push(obj(
        pose.car.0,
        pose.car.1,
        CAR_TILE,
        ObjShape::Square,
        3,
        1,
    ));
    for i in 0..9 {
        let (x, y) = (16 * i + 96 - off as i16, 8 * i - 8);
        f.objs.push(obj(x, y, CABLE_TILE, ObjShape::Square, 1, 2));
    }
}
