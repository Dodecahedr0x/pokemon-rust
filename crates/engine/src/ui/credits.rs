//! The credits sequence (Gen3 `credits.c`): the 57 text pages, and the bike scenes and Pokémon
//! interludes between them.
//!
//! `Task_UpdatePage` waits 72 frames, then each page fades in, holds 115 frames and fades out.
//! `CheckChangeScene` (`PAGE_COUNT / 9`) swaps the picture every 6 pages: ocean morning, a
//! Pokémon interlude, ocean sunset, forest (the rival arrives), an interlude, forest (the rival
//! is caught), an interlude, the city at night. START does not skip a page (Gen3 has no skip).
//!
//! The original ocean, forest and city tiles are not in the extract, so each scene is its
//! palette on a scrolling backdrop (`CreateBicycleBgAnimationTask`'s BG1 step, 2px a frame)
//! with the player sprite as the two riders (`SetBikeScene`'s positions). A Pokémon interlude
//! shows caught Pokémon (`Task_ShowMons`), front pics when the game has them.
//!
//! Deviation: the text fade is the whole screen's brightness, and THE END after the pages stays
//! the key-wait in `hall_of_fame` (the letter graphic is not extracted). A page is drawn again
//! at the start of its hold, so a visible frame carries the glyphs.

use super::credits_pages::{PAGE_COUNT, PAGES};
use super::kit::frame_budget;
use super::kit::window::{self, MapBuf, Panel, frame_kind};
use super::{ScreenCx, kit::Gfx};
use crate::data::{Font, GameData, SpeciesId, WindowGfx};
use crate::pokedex::DexFlag;
use crate::text::{Canvas, Colors, Pen};
use crate::video::{BgControl, Cmd, Obj, ObjShape, Tile};
use alloc::boxed::Box;
use alloc::vec::Vec;

#[allow(unused_imports)] // the page-0 test reads it; the library build does not
pub use super::credits_pages::FIRST_PAGE_LINE;

/// `Task_UpdatePage`'s opening delay.
const LEAD: u16 = 72;
/// A 16-step palette fade, one level a frame (`FADE_FRAMES`).
const FADE: u16 = 16;
/// `Task_UpdatePage` case 4: `tDelay = 115`.
const HOLD: u16 = 115;
/// `PAGE_INTERVAL` (`PAGE_COUNT / 9`).
const PAGE_INTERVAL: u8 = (PAGE_COUNT / 9) as u8;
/// Text window (`sWindowTemplates[0]`): full width, from tile row 9, 12 tiles tall.
pub(crate) const TEXT_WIN: (u8, u8, u8, u8) = (0, 9, 30, 12);
/// `GetStringCenterAlignXOffsetWithLetterSpacing(..., DISPLAY_WIDTH, 1)`.
const SCREEN_W: i16 = 240;
const LETTER_GAP: i16 = 1;
/// Scenery tile in the text char block, clear of the text canvas and of screen block 26.
const SCENE_TILE: u16 = 600;
const SCENE_MAP: u8 = 26;
const SCENE_PAL: u8 = 1;
const RIDER_Y: i16 = 46;
const RIDER_TILE: u16 = 0;
const MON_TILE: u16 = 32;
/// `Task_ShowMons` case 3: `tDelay = 50`.
const MON_HOLD: u16 = 50;

/// The nine pictures, in Gen3's order. The four interludes share [`Scene::Mons`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    OceanMorning = 0,
    Mons = 1,
    OceanSunset = 2,
    ForestRivalArrive = 3,
    ForestCatchRival = 4,
    CityNight = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Lead,
    FadeIn,
    Hold,
    FadeOut,
    SceneIn,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Roll {
    /// Page on screen, `0..PAGE_COUNT`.
    page: u8,
    phase: Phase,
    left: u16,
    scene: Scene,
    /// Repaint the page on the first hold frame (a visible upload).
    repost: bool,
    scroll: u16,
    riders: bool,
    rider_vram: bool,
    player_x: i16,
    rival_x: i16,
    picked: bool,
    from_dex: bool,
    mon_n: u8,
    mon_i: u8,
    mons: [u16; 16],
    mon_x: i16,
    mon_y: i16,
    mon_side: u8,
    mon_timer: u16,
    mon_species: u16,
    mon_vram: bool,
}

impl Roll {
    pub fn new() -> Self {
        Roll {
            page: 0,
            phase: Phase::Lead,
            left: LEAD,
            scene: Scene::OceanMorning,
            repost: false,
            scroll: 0,
            riders: true,
            rider_vram: false,
            player_x: SCREEN_W + 32,
            rival_x: SCREEN_W + 32,
            picked: false,
            from_dex: false,
            mon_n: 0,
            mon_i: 0,
            mons: [0; 16],
            mon_x: 104,
            mon_y: 36,
            mon_side: 0,
            mon_timer: MON_HOLD,
            mon_species: 0,
            mon_vram: false,
        }
    }

    pub fn page(&self) -> u8 {
        self.page
    }

    pub fn scene(&self) -> Scene {
        self.scene
    }

    pub fn frames_left(&self) -> u16 {
        self.left
    }

    /// One frame. `true` when the last page has faded out.
    pub fn tick<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, gfx: &mut Gfx<Panel>) -> bool {
        if self.left == 0 && self.advance(cx, gfx) {
            return true;
        }
        if !self.picked {
            self.pick(cx);
            self.picked = true;
        }
        self.draw(cx, gfx);
        self.left = self.left.saturating_sub(1);
        false
    }

    /// `true` when the pages are over.
    fn advance<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, gfx: &mut Gfx<Panel>) -> bool {
        match self.phase {
            Phase::FadeOut => {
                let next = self.page.saturating_add(1);
                if next as usize >= PAGE_COUNT {
                    return true;
                }
                self.page = next;
                if let Some(scene) = scene_after(self.page) {
                    self.place(scene);
                    if let Some(p) = gfx.0.as_mut() {
                        p.canvas.fill(0);
                    }
                    self.phase = Phase::SceneIn;
                    self.left = FADE;
                } else {
                    self.begin(cx, gfx);
                }
            }
            Phase::Lead | Phase::SceneIn => self.begin(cx, gfx),
            Phase::FadeIn => {
                self.phase = Phase::Hold;
                self.left = HOLD;
                self.repost = true;
            }
            Phase::Hold => {
                self.phase = Phase::FadeOut;
                self.left = FADE;
            }
        }
        false
    }

    fn begin<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, gfx: &mut Gfx<Panel>) {
        self.phase = Phase::FadeIn;
        self.left = FADE;
        let (l, t, w, h) = TEXT_WIN;
        let panel = gfx
            .0
            .get_or_insert_with(|| Box::new(Panel::new(l, t, w, h, window::FULL_BASE, false, 0)));
        paint_page(
            &mut panel.canvas,
            cx.game.font(),
            cx.game.window(),
            self.page as usize,
        );
        let mut map = MapBuf::overlay(frame_kind(cx.game, cx.options.frame));
        map.put(panel);
        map.flush(cx.f);
    }

    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, gfx: &mut Gfx<Panel>) {
        cx.f.brightness = self.brightness();
        self.scenery(cx);
        if self.riders {
            self.riders(cx);
        } else {
            self.mons(cx);
        }
        if self.repost {
            if let Some(p) = gfx.0.as_mut() {
                p.canvas.touch();
            }
            self.repost = false;
        }
        if matches!(self.phase, Phase::FadeIn | Phase::Hold | Phase::SceneIn) {
            if let Some(p) = gfx.0.as_mut() {
                let mut budget = frame_budget(cx.f);
                p.flush_within(cx.f, &mut budget);
            }
        }
    }

    fn brightness(&self) -> u8 {
        match self.phase {
            Phase::Lead => FADE as u8,
            Phase::FadeIn | Phase::SceneIn => self.left.min(FADE) as u8,
            Phase::Hold => 0,
            Phase::FadeOut => (FADE + 1 - self.left.min(FADE)) as u8,
        }
    }

    fn scenery<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if self.riders {
            self.scroll = self.scroll.wrapping_sub(2);
            cx.f.scroll[1].0 = self.scroll;
        } else {
            cx.f.scroll[1].0 = 0;
        }
        cx.f.cmds.push(Cmd::BgPalette {
            bank: SCENE_PAL,
            colors: scene_palette(self.scene),
        });
        cx.f.cmds.push(Cmd::BgTile {
            char_block: window::CHAR_BLOCK,
            index: SCENE_TILE,
            tile: SCENE_PATTERN,
        });
        cx.f.cmds.push(Cmd::MapFill {
            screen_block: SCENE_MAP,
            entry: SCENE_TILE | (SCENE_PAL as u16) << 12,
        });
        cx.f.cmds.push(Cmd::Bg {
            layer: 1,
            control: BgControl {
                enabled: true,
                priority: 1,
                char_block: window::CHAR_BLOCK,
                screen_block: SCENE_MAP,
            },
        });
    }

    fn riders<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        // `Task_CycleSceneryPalette`: ocean morning sends the player toward center on page 2.
        if self.scene == Scene::OceanMorning && self.page >= 2 && self.player_x > SCREEN_W / 2 {
            self.player_x -= 1;
        }
        let sprite = cx.game.player_sprite();
        let Some(frame) = sprite.frames.first().copied() else {
            return;
        };
        let Some((shape, size)) = Obj::shape_for(sprite.width, sprite.height) else {
            return;
        };
        if !self.rider_vram {
            cx.f.cmds.push(Cmd::ObjTiles {
                first: RIDER_TILE,
                tiles: frame,
            });
            cx.f.cmds.push(Cmd::ObjPalette {
                bank: 0,
                colors: *sprite.palette,
            });
            self.rider_vram = true;
        }
        for x in [self.player_x, self.rival_x] {
            cx.f.objs.push(Obj {
                x,
                y: RIDER_Y,
                tile: RIDER_TILE,
                palette: 0,
                shape,
                size,
                hflip: false,
                vflip: false,
                priority: 1,
                scale: 0,
            });
        }
    }

    fn mons<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        if self.mon_n == 0 {
            return;
        }
        self.mon_timer = self.mon_timer.saturating_sub(1);
        if self.mon_side == 0 {
            self.mon_x -= 2;
        } else {
            self.mon_x += 2;
        }
        if self.mon_timer == 0 {
            self.mon_i = (self.mon_i + 1) % self.mon_n;
            self.mon_side ^= 1;
            self.spawn();
        }
        let tables = cx.game.battle();
        let id = self.mon_species as usize;
        if id == 0 || id >= tables.species.len() {
            return;
        }
        let Some(gfx) = tables.species_info(SpeciesId(self.mon_species)).gfx else {
            return;
        };
        if gfx.front.len() < 64 {
            return;
        }
        if !self.mon_vram {
            cx.f.cmds.push(Cmd::ObjTiles {
                first: MON_TILE,
                tiles: &gfx.front[..64],
            });
            cx.f.cmds.push(Cmd::ObjPalette {
                bank: 2,
                colors: *gfx.palette,
            });
            self.mon_vram = true;
        }
        cx.f.objs.push(Obj {
            x: self.mon_x,
            y: self.mon_y,
            tile: MON_TILE,
            palette: 2,
            shape: ObjShape::Square,
            size: 3,
            hflip: false,
            vflip: false,
            priority: 1,
            scale: 0,
        });
    }

    fn spawn(&mut self) {
        self.mon_timer = MON_HOLD;
        self.mon_species = self.mons[self.mon_i as usize];
        self.mon_x = if self.mon_side == 0 { 104 } else { 136 };
        self.mon_y = 36;
        self.mon_vram = false;
    }

    fn place(&mut self, scene: Scene) {
        self.scene = scene;
        self.riders = scene != Scene::Mons;
        self.scroll = 0;
        match scene {
            Scene::OceanMorning => {
                self.player_x = SCREEN_W + 32;
                self.rival_x = SCREEN_W + 32;
            }
            Scene::OceanSunset | Scene::ForestRivalArrive => {
                self.player_x = 120;
                self.rival_x = SCREEN_W + 32;
            }
            Scene::ForestCatchRival => {
                self.player_x = 120;
                self.rival_x = -32;
            }
            Scene::CityNight => {
                self.player_x = 88;
                self.rival_x = 152;
            }
            Scene::Mons => self.spawn(),
        }
    }

    /// `DeterminePokemonToShow`: caught species, shuffled once. A dex with nothing caught uses
    /// the party in party order and does not draw (`Random` would divide by zero).
    fn pick<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>) {
        let tables = cx.game.battle();
        let mut caught = Vec::new();
        let n = tables
            .species
            .len()
            .min(crate::pokedex::MAX_SPECIES as usize);
        for i in 1..n {
            if cx.pokedex.get(DexFlag::Caught, SpeciesId(i as u16)) {
                caught.push(i as u16);
            }
        }
        self.from_dex = !caught.is_empty();
        if caught.is_empty() {
            for mon in cx.party.iter() {
                if mon.species.0 != 0 {
                    caught.push(mon.species.0);
                }
            }
        }
        let take = caught.len().min(self.mons.len());
        let mut live = caught.len();
        for j in 0..take {
            let at = if self.from_dex {
                cx.rng.random() as usize % live
            } else {
                j
            };
            self.mons[j] = caught[at];
            if self.from_dex {
                live -= 1;
                caught.swap(at, live);
            }
        }
        self.mon_n = take as u8;
        self.mon_i = 0;
        self.mon_side = 0;
        if self.mon_n > 0 {
            self.mon_species = self.mons[0];
        }
    }
}

/// `CheckChangeScene`: after pages 6, 12, …, 48.
fn scene_after(done: u8) -> Option<Scene> {
    if done == 0 || done > PAGE_INTERVAL * 8 || done % PAGE_INTERVAL != 0 {
        return None;
    }
    Some(match done / PAGE_INTERVAL {
        1 | 3 | 5 | 7 => Scene::Mons,
        2 => Scene::OceanSunset,
        4 => Scene::ForestRivalArrive,
        6 => Scene::ForestCatchRival,
        _ => Scene::CityNight,
    })
}

/// Stripes so the backdrop's scroll is visible. Palette entries 1 and 2 are the scene colors.
const SCENE_PATTERN: Tile = [
    0x1111_1111,
    0x1111_1111,
    0x2222_2222,
    0x2222_2222,
    0x1111_1111,
    0x1111_1111,
    0x2222_2222,
    0x2222_2222,
];

fn rgb(r: u16, g: u16, b: u16) -> u16 {
    (r & 31) | (g & 31) << 5 | (b & 31) << 10
}

fn scene_palette(scene: Scene) -> [u16; 16] {
    let (a, b) = match scene {
        // Light green text-fade color sits over a morning sky.
        Scene::OceanMorning => (rgb(12, 20, 28), rgb(18, 26, 31)),
        Scene::OceanSunset => (rgb(31, 18, 8), rgb(28, 10, 6)),
        Scene::ForestRivalArrive => (rgb(8, 20, 10), rgb(14, 26, 12)),
        Scene::ForestCatchRival => (rgb(6, 16, 8), rgb(10, 22, 8)),
        Scene::CityNight => (rgb(4, 4, 12), rgb(8, 8, 18)),
        // `COLOR_DARK_GREEN`.
        Scene::Mons => (rgb(7, 11, 6), rgb(4, 8, 4)),
    };
    let mut pal = [0; 16];
    pal[1] = a;
    pal[2] = b;
    pal
}

/// Draws page `page` centered, titles and names (`PrintCreditsText`).
pub(crate) fn paint_page(canvas: &mut Canvas, font: &Font, window: &WindowGfx, page: usize) {
    canvas.fill(0);
    let Some(lines) = PAGES.get(page) else {
        return;
    };
    for (i, line) in lines.iter().enumerate() {
        if line.text.first().copied().unwrap_or(0xFF) == 0xFF {
            continue;
        }
        let width = text_width(font, line.text);
        let x = (SCREEN_W - width) / 2;
        let y = 5 + i as i16 * 16;
        let pen = Pen::new(line_colors(window, line.title));
        let mut cx = x.max(0);
        for (n, &b) in line.text.iter().enumerate() {
            if b == 0xFF {
                break;
            }
            if n > 0 {
                cx += LETTER_GAP;
            }
            cx += canvas.draw_glyph(font, b as u16, cx, y, &pen);
        }
    }
}

fn line_colors(window: &WindowGfx, title: bool) -> Colors {
    let fg = if title {
        let n = window.text_fg.wrapping_add(1) & 0xF;
        if n == 0 { 1 } else { n }
    } else if window.text_fg == 0 {
        1
    } else {
        window.text_fg
    };
    Colors::new(fg, window.text_shadow, 0)
}

fn text_width(font: &Font, text: &[u8]) -> i16 {
    let mut w = 0i16;
    let mut n = 0i16;
    for &b in text {
        if b == 0xFF {
            break;
        }
        if n > 0 {
            w += LETTER_GAP;
        }
        w += font.widths.get(b as usize).copied().unwrap_or(0) as i16;
        n += 1;
    }
    w
}
