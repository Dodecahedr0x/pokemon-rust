//! The power-on title (Gen3 `title_screen.c`, after the copyright hold): copyright, a blinking
//! PRESS START, and a Pokémon flying across the screen. Start (or A) opens the main menu;
//! B there comes back (`world/screens.rs`).
//!
//! The Pokémon is the first species front pic that has pixels (`MonGfx::front`, 64×64). A game
//! without pics gets a small silhouette, so the flight still plays. Deviation: no logo, no
//! Game Freak copyright screen before this, and the flight is a straight slide with a bob
//! rather than Rayquaza's scene.

use super::kit::Gfx;
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::GameData;
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::video::{Cmd, Obj, ObjShape, Tile, layout};
use alloc::boxed::Box;

/// OBJ tile and palette of the flying Pokémon (menu layout: 512.. and bank 6 are free).
pub const MON_TILE: u16 = layout::menu::OBJ_BAG + 256;
pub const MON_PAL: u8 = 6;
/// What Start answers, so the host opens the main menu instead of leaving for the field.
pub const TO_MENU: u16 = 2;
/// Frames PRESS START stays shown, then hidden (`Task_TitleScreen_BlinkPressStart` cadence).
pub const BLINK: u16 = 32;
/// Pixels the Pokémon moves per frame.
const SPEED: i16 = 2;
const BASE_Y: i16 = 12;
const BOB: [i16; 8] = [0, 1, 2, 3, 2, 1, 0, -1];

/// `PRESS START` (Gen3 charmap, `A` = 0xBB).
const PRESS_START: &[u8] = &[
    0xCA, 0xCC, 0xBF, 0xCD, 0xCD, 0x00, 0xCD, 0xCE, 0xBB, 0xCC, 0xCE, 0xFF,
];
/// `(C)1995-2005 GAME FREAK inc.`
const COPYRIGHT: &[u8] = &[
    0x5C, 0xBD, 0x5D, 0xA2, 0xAA, 0xAA, 0xA6, 0xAE, 0xA3, 0xA1, 0xA1, 0xA6, 0x00, 0xC1, 0xBB, 0xC7,
    0xBF, 0x00, 0xC0, 0xCC, 0xBF, 0xBB, 0xC5, 0x00, 0xDD, 0xE2, 0xD7, 0xAD, 0xFF,
];

const fn nibbles(px: [u8; 8]) -> u32 {
    let mut v = 0u32;
    let mut i = 0;
    while i < 8 {
        v |= (px[i] as u32) << (4 * i);
        i += 1;
    }
    v
}

/// A 32×32 silhouette used when the game has no species pics.
const fn body(px: i32, py: i32) -> u8 {
    let dx = px - 18;
    let dy = py - 16;
    let torso = dx * dx * 64 + dy * dy * 100 < 6400;
    let tail = px > 2 && px < 12 && py > 13 && py < 20;
    if torso || tail { 1 } else { 0 }
}

const fn fallback_tile(tx: usize, ty: usize) -> Tile {
    let mut tile = [0u32; 8];
    let mut y = 0;
    while y < 8 {
        let mut row = [0u8; 8];
        let mut x = 0;
        while x < 8 {
            row[x] = body((tx * 8 + x) as i32, (ty * 8 + y) as i32);
            x += 1;
        }
        tile[y] = nibbles(row);
        y += 1;
    }
    tile
}

const FALLBACK: [Tile; 16] = [
    fallback_tile(0, 0),
    fallback_tile(1, 0),
    fallback_tile(2, 0),
    fallback_tile(3, 0),
    fallback_tile(0, 1),
    fallback_tile(1, 1),
    fallback_tile(2, 1),
    fallback_tile(3, 1),
    fallback_tile(0, 2),
    fallback_tile(1, 2),
    fallback_tile(2, 2),
    fallback_tile(3, 2),
    fallback_tile(0, 3),
    fallback_tile(1, 3),
    fallback_tile(2, 3),
    fallback_tile(3, 3),
];
const FALLBACK_PAL: [u16; 16] = [0, 0x03E0, 0x0140, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

struct Drawn {
    press: Panel,
    copy: Panel,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Title {
    /// Top-left of the flying Pokémon.
    x: i16,
    y: i16,
    /// OAM size (2 = 32×32 silhouette, 3 = 64×64 species pic).
    size: u8,
    tick: u16,
    /// PRESS START was drawn in this state.
    painted: bool,
    pic_sent: bool,
    gfx: Gfx<Drawn>,
}

impl Default for Title {
    fn default() -> Self {
        Self::new()
    }
}

impl Title {
    pub fn new() -> Self {
        Title {
            x: 20,
            y: BASE_Y,
            size: 2,
            tick: 0,
            painted: false,
            pic_sent: false,
            gfx: Gfx::none(),
        }
    }

    /// The copyright line has been drawn.
    pub fn shows_copyright(&self) -> bool {
        self.gfx.0.is_some()
    }

    /// PRESS START is in its shown half (it blinks every [`BLINK`] frames).
    pub fn shows_press_start(&self) -> bool {
        (self.tick / BLINK).is_multiple_of(2)
    }

    /// Where the Pokémon is, in pixels from the left.
    pub fn mon_x(&self) -> i16 {
        self.x
    }

    fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.x += SPEED;
        if self.x > crate::video::SCREEN_W as i16 {
            self.x = -64;
        }
        self.y = BASE_Y + BOB[(self.tick as usize / 4) % BOB.len()];
    }

    fn paint_press<G: GameData>(&self, game: &G, p: &mut Panel) {
        p.clear(game);
        if self.shows_press_start() {
            p.text(game, PRESS_START, 8, 1);
        }
    }

    fn compose<G: GameData>(cx: &mut ScreenCx<'_, G>, drawn: &mut Drawn) {
        let mut map = MapBuf::full(frame_kind(cx.game, cx.options.frame));
        map.put(&drawn.press);
        map.put(&drawn.copy);
        map.flush(cx.f);
        let mut budget = super::kit::frame_budget(cx.f);
        drawn.press.flush_within(cx.f, &mut budget);
        drawn.copy.flush_within(cx.f, &mut budget);
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        self.upload_pic(cx);
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let mut press = tiles.panel(game, 7, 12, 16, 2, false);
        let mut copy = tiles.panel(game, 1, 16, 28, 2, false);
        self.paint_press(game, &mut press);
        copy.text(game, COPYRIGHT, 0, 1);
        let mut drawn = Drawn { press, copy };
        Self::compose(cx, &mut drawn);
        self.painted = self.shows_press_start();
        self.gfx.0 = Some(Box::new(drawn));
    }

    fn repaint<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let show = self.shows_press_start();
        self.painted = show;
        let Some(drawn) = self.gfx.0.as_mut() else {
            return;
        };
        drawn.press.clear(cx.game);
        if show {
            drawn.press.text(cx.game, PRESS_START, 8, 1);
        }
        Self::compose(cx, drawn);
    }

    fn upload_pic<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if self.pic_sent {
            return;
        }
        let (tiles, palette, size) = species_pic(cx.game);
        self.size = size;
        cx.f.cmds.push(Cmd::ObjTiles {
            first: MON_TILE,
            tiles,
        });
        cx.f.cmds.push(Cmd::ObjPalette {
            bank: MON_PAL,
            colors: palette,
        });
        self.pic_sent = true;
    }

    fn place(&self, f: &mut crate::video::Frame) {
        f.objs.push(Obj {
            x: self.x,
            y: self.y,
            tile: MON_TILE,
            palette: MON_PAL,
            shape: ObjShape::Square,
            size: self.size,
            priority: 0,
            ..Obj::default()
        });
    }
}

/// The first species front pic that has a pixel, or the silhouette.
fn species_pic(game: &impl GameData) -> (&'static [Tile], [u16; 16], u8) {
    if let Some(gfx) = game.battle().species.iter().find_map(|s| {
        let g = s.gfx?;
        g.front
            .iter()
            .take(64)
            .any(|t| t.iter().any(|w| *w != 0))
            .then_some(g)
    }) && gfx.front.len() >= 64
    {
        return (&gfx.front[..64], *gfx.palette, 3);
    }
    (&FALLBACK, FALLBACK_PAL, 2)
}

impl Screen for Title {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        self.advance();
        if self.gfx.0.is_none() {
            self.draw(cx);
        } else if self.shows_press_start() != self.painted {
            self.repaint(cx);
        }
        self.place(cx.f);
        if input.pressed.contains(Buttons::START) || input.pressed.contains(Buttons::A) {
            ScreenOut::Done(ScreenResult::Value(TO_MENU))
        } else {
            ScreenOut::Running
        }
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write_u16(self.x as u16);
        h.write_u16(self.y as u16);
        h.write(&[self.size, u8::from(self.shows_press_start())]);
    }
}
