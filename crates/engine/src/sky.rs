//! What the field's palettes and sky show: the weather's colour map, blend and particles
//! (`crate::weather`), a script's palette override ([`PaletteFx`], `Request::SetPalette`) and
//! an overlay drawn over the field (`Request::ShowOverlay`, [`OverlayRun`]).
//!
//! [`apply`] post-processes the frame (in `World::step`, before `crate::fx::apply`): while the
//! field shows (the field, a script on it, a screen over it), every BG palette upload of banks
//! 0..=13 and every OBJ palette upload is transformed; when the transform changes, the banks
//! the frame did not upload are sent again from a shadow of their last colours
//! ([`Display`]), so a weather or an override set mid-map recolours the whole screen. A battle
//! or a full screen owns the screen: nothing is transformed then, but the shadow still follows
//! the uploads (the field's restore uploads its palettes under black), so the field's first
//! frame after it recolours every bank.
//!
//! The order on a colour: the weather's colour map (Gen3 `ApplyColorMap`: BG banks through the
//! darkened contrast maps, OBJ banks through the contrast maps), the weather's blend, then the
//! override.
//!
//! **Particles and overlays** use OBJ tiles [`RESERVED_FIRST_TILE`].. (below the emotes) and
//! OBJ palette banks [`PARTICLE_BANK`] and [`OVERLAY_BANK`], which the objects leave alone
//! while they are drawn (`Objects::reserve_banks`). They are drawn in front of the objects.

use crate::data::GameData;
use crate::hash::Fnv64;
use crate::video::{Cmd, Frame, MAX_OBJS, Obj, ObjShape, SCREEN_H, SCREEN_W, Tile};
use crate::weather::{self, Look, Weather};
use alloc::boxed::Box;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// What a [`PaletteFx`] does to a colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum PaletteMode {
    /// Towards `color` by the amount (Gen3 `BlendPalettes`).
    #[default]
    Blend,
    /// Towards the colour's negative.
    Invert,
    /// Towards the colour's grey (its channels' mean), then towards `color` by a quarter of
    /// the amount (a "faded" palette: greys tinted by the colour).
    Grey,
}

impl PaletteMode {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// A script's palette override (Silice's night, alerts, gold and destroyed palettes; not
/// Gen3): hashed and snapshotted while set. Cleared on every map load unless `keep`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PaletteFx {
    pub mode: PaletteMode,
    /// BGR555.
    pub color: u16,
    /// The amount aimed at, 0..=16 (0 ends the override once reached).
    pub amount: u8,
    /// The amount reached.
    pub current: u8,
    /// Frames the change to `amount` takes.
    pub frames: u16,
    /// Frames since the request.
    pub t: u16,
    /// A strobe: on for `pulse` frames, off for `pulse` (0: steady).
    pub pulse: u8,
    /// Kept across map loads.
    pub keep: bool,
}

impl PaletteFx {
    /// Feeds the override into `h`.
    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            0xB9,
            self.mode.code(),
            self.amount,
            self.current,
            self.pulse,
            self.keep as u8,
        ]);
        for v in [self.color, self.frames, self.t] {
            h.write_u16(v);
        }
    }

    /// The amount shown this frame (a strobe's off half shows nothing).
    pub fn shown(&self) -> u8 {
        if self.pulse != 0 && (self.t / u16::from(self.pulse)) % 2 == 1 {
            0
        } else {
            self.current
        }
    }

    /// One frame: `current` steps towards `amount` over what is left of `frames`. Returns
    /// whether the override is over (amount and current 0).
    pub fn tick(&mut self) -> bool {
        if self.current != self.amount {
            let remaining = self.frames.saturating_sub(self.t).max(1) as i16;
            let diff = i16::from(self.amount) - i16::from(self.current);
            let mut step = diff / remaining;
            if step == 0 {
                step = diff.signum();
            }
            self.current = (i16::from(self.current) + step) as u8;
        }
        self.t = self.t.wrapping_add(1);
        self.amount == 0 && self.current == 0
    }

    /// `c` with the override at amount `a`.
    pub fn apply(&self, c: u16, a: u8) -> u16 {
        if a == 0 {
            return c;
        }
        match self.mode {
            PaletteMode::Blend => weather::blend(c, self.color, a),
            PaletteMode::Invert => weather::blend(c, !c & 0x7FFF, a),
            PaletteMode::Grey => {
                let (r, g, b) = (c & 31, (c >> 5) & 31, (c >> 10) & 31);
                let m = (r + g + b) / 3;
                let grey = weather::blend(c, m | m << 5 | m << 10, a);
                weather::blend(grey, self.color, a / 4)
            }
        }
    }
}

/// An overlay's graphics (the game's, [`GameData::overlay`]): 8x8 OBJ cells at screen cells,
/// revealed in `stages` over the overlay's frames (Silice's sky crack).
#[derive(Clone, Copy, Debug)]
pub struct Overlay {
    /// At most [`OVERLAY_TILES`].
    pub tiles: &'static [Tile],
    pub palette: [u16; 16],
    pub cells: &'static [OverlayCell],
    pub stages: u8,
}

/// One cell of an [`Overlay`]: screen cell `(x, y)` (8 px units), tile index into
/// `Overlay::tiles`, shown from stage `stage` on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OverlayCell {
    pub x: u8,
    pub y: u8,
    pub tile: u8,
    pub stage: u8,
}

/// A running overlay (`Request::ShowOverlay`): screen effect RAM, hashed while it runs (the
/// script waits for it, so it is never captured).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OverlayRun {
    pub id: u8,
    pub frames: u16,
    pub t: u16,
}

impl OverlayRun {
    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[0xBA, self.id]);
        h.write_u16(self.frames);
        h.write_u16(self.t);
    }

    pub fn done(&self) -> bool {
        self.t >= self.frames
    }
}

/// Tiles the weather's particles use.
pub const PARTICLE_TILES: u16 = weather::PARTICLE_TILES as u16;
/// Tiles an overlay may use.
pub const OVERLAY_TILES: u16 = crate::objects::SKY_TILES - PARTICLE_TILES;
/// The first reserved OBJ tile: particles, then the overlay, just below the emote slots.
pub const RESERVED_FIRST_TILE: u16 = crate::objects::SKY_FIRST_TILE;
const OVERLAY_FIRST_TILE: u16 = RESERVED_FIRST_TILE + PARTICLE_TILES;
/// The OBJ palette bank of the particles, while drawn.
pub const PARTICLE_BANK: u8 = 15;
/// The OBJ palette bank of an overlay, while drawn.
pub const OVERLAY_BANK: u8 = 14;
/// BG banks the weather and the overrides recolour (Gen3 `sBasePaletteColorMapTypes`: 0..=13;
/// 14 and 15 are the windows').
const BG_BANKS: u16 = 0x3FFF;

/// What the screen holds of the sky's work (display only: neither hashed nor snapshotted).
#[derive(Clone, Debug)]
pub struct Display {
    /// The last colours uploaded, untransformed: BG banks 0..16, then OBJ banks (on the heap:
    /// the World is built on the stack).
    shadow: Vec<[u16; 16]>,
    /// Banks of `shadow` that hold the field's colours.
    valid: u32,
    /// The transform the screen shows ([`Key`]).
    applied: Key,
    /// The particles' tiles in VRAM (their address).
    particle_tiles: Option<usize>,
    /// The overlay's tiles in VRAM.
    overlay_tiles: Option<usize>,
    /// [`weather::color_maps`], on the heap.
    maps: Box<weather::ColorMaps>,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            shadow: alloc::vec![[0; 16]; 32],
            valid: 0,
            applied: Key::IDENTITY,
            particle_tiles: None,
            overlay_tiles: None,
            maps: weather::color_maps(),
        }
    }
}

/// A transform: the colour map, the weather blend, the override's mode, colour and amount.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Key {
    color_map: i8,
    blend: (u16, u8),
    over: (u8, u16, u8),
}

impl Key {
    const IDENTITY: Key = Key {
        color_map: 0,
        blend: (0, 0),
        over: (0, 0, 0),
    };
}

/// The look of weather `w` (the game's for its own kinds).
pub fn look(game: &impl GameData, w: u8) -> Option<Look> {
    if w >= weather::GAME_FIRST {
        game.weather_look(w).copied()
    } else {
        weather::builtin_look(w)
    }
}

/// The OBJ banks the field must leave free this frame (`Objects::reserve_banks`).
pub fn reserved_banks(game: &impl GameData, w: &Weather, overlay: Option<&OverlayRun>) -> u16 {
    let mut m = 0;
    if look(game, w.current).is_some_and(|l| l.particles.is_some()) {
        m |= 1 << PARTICLE_BANK;
    }
    if overlay.is_some() {
        m |= 1 << OVERLAY_BANK;
    }
    m
}

/// The sky's inputs for one frame.
pub struct Sky<'a> {
    pub weather: &'a Weather,
    pub palette: Option<&'a PaletteFx>,
    pub overlay: Option<&'a OverlayRun>,
    /// Whether the field shows (see the module docs).
    pub field: bool,
    /// The camera's map pixel at the screen's top-left (the particles move with the world).
    pub camera: (i32, i32),
}

/// Post-processes `f` (see the module docs).
pub fn apply(game: &impl GameData, d: &mut Display, sky: &Sky<'_>, f: &mut Frame) {
    if !sky.field {
        // A battle or a full screen: nothing is transformed, but the uploads are still kept
        // (the field's own restore uploads under them), so the field's first frame recolours
        // every bank.
        for c in &f.cmds {
            match c {
                Cmd::BgPalette { bank, colors } => {
                    d.shadow[*bank as usize & 15] = *colors;
                    d.valid |= 1 << (*bank & 15);
                }
                Cmd::ObjPalette { bank, colors } => {
                    d.shadow[16 + (*bank as usize & 15)] = *colors;
                    d.valid |= 1 << (16 + (*bank & 15));
                }
                _ => {}
            }
        }
        d.applied = Key::IDENTITY;
        d.particle_tiles = None;
        d.overlay_tiles = None;
        return;
    }
    let look = look(game, sky.weather.current);
    let mut color_map = sky.weather.color_map;
    if look.is_some_and(|l| l.pulse) && (sky.weather.t / 32) % 2 == 1 {
        color_map += 2;
    }
    let over = sky.palette.map(|p| (p.mode.code(), p.color, p.shown()));
    let key = Key {
        color_map,
        blend: look.and_then(|l| l.blend).unwrap_or((0, 0)),
        over: over.unwrap_or((0, 0, 0)),
    };
    let reserved = reserved_banks(game, sky.weather, sky.overlay);
    let maps = &*d.maps;
    let transform = |c: u16, obj: bool| -> u16 {
        let mut c = weather::map_color(c, key.color_map, maps.of(obj));
        if key.blend.1 != 0 {
            c = weather::blend(c, key.blend.0, key.blend.1);
        }
        if let Some(p) = sky.palette {
            c = p.apply(c, key.over.2);
        }
        c
    };
    let mut sent = 0u32;
    for c in &mut f.cmds {
        match c {
            Cmd::BgPalette { bank, colors } => {
                let i = *bank as usize & 15;
                d.shadow[i] = *colors;
                d.valid |= 1 << i;
                sent |= 1 << i;
                if BG_BANKS & (1 << i) != 0 {
                    colors.iter_mut().for_each(|x| *x = transform(*x, false));
                }
            }
            Cmd::ObjPalette { bank, colors } if reserved & (1 << *bank) == 0 => {
                let i = 16 + (*bank as usize & 15);
                d.shadow[i] = *colors;
                d.valid |= 1 << i;
                sent |= 1 << i;
                colors.iter_mut().for_each(|x| *x = transform(*x, true));
            }
            _ => {}
        }
    }
    if key != d.applied {
        for i in 0..32 {
            if d.valid & !sent & (1 << i) == 0 {
                continue;
            }
            if i < 16 && BG_BANKS & (1 << i) == 0 {
                continue;
            }
            if i >= 16 && reserved & (1 << (i - 16)) != 0 {
                continue;
            }
            let mut colors = d.shadow[i];
            colors.iter_mut().for_each(|x| *x = transform(*x, i >= 16));
            f.cmds.push(if i < 16 {
                Cmd::BgPalette {
                    bank: i as u8,
                    colors,
                }
            } else {
                Cmd::ObjPalette {
                    bank: (i - 16) as u8,
                    colors,
                }
            });
        }
        d.applied = key;
    }
    let mut front = alloc::vec::Vec::new();
    if let Some(run) = sky.overlay
        && let Some(o) = game.overlay(run.id)
    {
        draw_overlay(d, run, o, f, &mut front);
    } else {
        d.overlay_tiles = None;
    }
    match look.and_then(|l| l.particles) {
        Some(p) => draw_particles(d, sky, &p, f, &mut front),
        None => d.particle_tiles = None,
    }
    if !front.is_empty() {
        let room = MAX_OBJS.saturating_sub(front.len());
        f.objs.truncate(room);
        f.objs.splice(0..0, front);
    }
}

fn draw_particles(
    d: &mut Display,
    sky: &Sky<'_>,
    p: &weather::Particles,
    f: &mut Frame,
    out: &mut alloc::vec::Vec<Obj>,
) {
    let tiles = &p.tiles[..p.tiles.len().min(weather::PARTICLE_TILES)];
    let addr = tiles.as_ptr() as usize;
    if d.particle_tiles != Some(addr) {
        f.cmds.push(Cmd::ObjTiles {
            first: RESERVED_FIRST_TILE,
            tiles,
        });
        f.cmds.push(Cmd::ObjPalette {
            bank: PARTICLE_BANK,
            colors: p.palette,
        });
        d.particle_tiles = Some(addr);
    }
    let (w, h) = Obj::dimensions(p.shape, p.size);
    let (span_x, span_y) = (SCREEN_W as i32 + 32, SCREEN_H as i32 + 32);
    let t = i32::from(sky.weather.t);
    for i in 0..i32::from(p.count) {
        // Spread the particles over the screen (a fixed scatter), moving `d / 8` pixels a
        // frame, the world's way: the camera's move shifts them back.
        let bx = (i * 97 + (i * i * 13) % 61) % span_x;
        let by = (i * 59 + (i * i * 7) % 43) % span_y;
        let sway = if p.sway == 0 {
            0
        } else {
            let phase = (t + i * 16) % 64;
            let tri = if phase < 32 { phase } else { 64 - phase };
            (tri - 16) * i32::from(p.sway) / 16
        };
        let x = (bx + i32::from(p.dx) * t / 8 - sky.camera.0 + sway).rem_euclid(span_x) - 16;
        let y = (by + i32::from(p.dy) * t / 8 - sky.camera.1).rem_euclid(span_y) - 16;
        if x <= -(w as i32) || y <= -(h as i32) {
            continue;
        }
        out.push(Obj {
            x: x as i16,
            y: y as i16,
            tile: RESERVED_FIRST_TILE,
            palette: PARTICLE_BANK,
            shape: p.shape,
            size: p.size,
            hflip: false,
            vflip: false,
            // Gen3's rain: OAM priority 1, over the field's top layer.
            priority: 1,
            scale: 0,
        });
    }
}

fn draw_overlay(
    d: &mut Display,
    run: &OverlayRun,
    o: &Overlay,
    f: &mut Frame,
    out: &mut alloc::vec::Vec<Obj>,
) {
    let tiles = &o.tiles[..o.tiles.len().min(OVERLAY_TILES as usize)];
    let addr = tiles.as_ptr() as usize;
    if d.overlay_tiles != Some(addr) {
        f.cmds.push(Cmd::ObjTiles {
            first: OVERLAY_FIRST_TILE,
            tiles,
        });
        f.cmds.push(Cmd::ObjPalette {
            bank: OVERLAY_BANK,
            colors: o.palette,
        });
        d.overlay_tiles = Some(addr);
    }
    let stages = u32::from(o.stages.max(1));
    let stage = (u32::from(run.t) * stages / u32::from(run.frames.max(1))).min(stages - 1) as u8;
    for c in o.cells.iter().filter(|c| c.stage <= stage) {
        if usize::from(c.tile) >= tiles.len() {
            continue;
        }
        out.push(Obj {
            x: i16::from(c.x) * 8,
            y: i16::from(c.y) * 8,
            tile: OVERLAY_FIRST_TILE + u16::from(c.tile),
            palette: OVERLAY_BANK,
            shape: ObjShape::Square,
            size: 0,
            hflip: false,
            vflip: false,
            priority: 0,
            scale: 0,
        });
    }
}

#[cfg(test)]
mod tests;
