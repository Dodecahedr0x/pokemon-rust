//! Starter choice screen, as Gen3 `starter_choose.c` (`CB2_ChooseStarter`). **Task G.**
//!
//! Bag background, three Poké Ball OBJs and a bobbing hand; left/right move the hand, A shows
//! the circle with the mon's front pic and asks "Do you choose this POKéMON?" with a yes/no;
//! B or no returns to the choice; yes makes `update` return the index on that frame, with the
//! screen still visible: as Gen3 `CB2_GiveStarter`, the caller runs the battle transition
//! straight over it.
//!
//! Gen3 order, frame by frame (task functions):
//! - `Task_StarterChoose`: the species label is drawn and the message box printed; input is read
//!   from the next frame (`Task_HandleStarterChooseInput`), also during the fade-in, as Gen3.
//! - left/right: the selection changes at once (hand and ball animation follow it), the label is
//!   cleared on the next frame (`Task_MoveStarterChooseCursor`) and redrawn on the one after
//!   (`Task_CreateStarterLabel`); input resumes after that.
//! - A: the label is cleared and the circle and pic appear at the ball, moving 4 px in x and
//!   2 px in y per frame toward (120, 64) (`SpriteCB_StarterPokemon`); once there and the
//!   16-frame affine grow is over (`Task_WaitForStarterSprite`), the question is printed and
//!   the yes/no shown (`Task_AskConfirmStarter`).
//! - yes: `gSpecialVar_Result` is set and the screen exits at once
//!   (`Task_HandleConfirmStarterInput`): `update` returns `Some(index)` from that frame on and
//!   keeps drawing the screen (the sprites keep animating; no input is read any more).
//! - no / B: the circle and pic go (`Task_HandleConfirmStarterInput`), then a frame later
//!   (`Task_DeclineStarter`) back to `Task_StarterChoose`.
//!
//! Simplifications (documented):
//! - no affine scaling: the circle and pic are drawn at full size while they move (Gen3 grows
//!   the pic from 1/16 to 1x and the circle to 1.25x);
//! - no window/blend: Gen3 darkens the label's rectangle (WIN0 + BLDY 7); here the label canvas
//!   is filled with the window palette's text color (`WindowGfx::text_fg`, dark gray) as a
//!   cheap stand-in, and the category line (FONT_NARROW) is left out;
//! - the message box is the field dialogue box (`text::TextBox`, its frame and yes/no
//!   positions), and it prints at the field text speed (Gen3 prints these instantly). Gen3
//!   prints `gText_BirchInTrouble` while choosing; there is no `battle::Str` for it yet, so
//!   the box stays empty until the question;
//! - no cry, no sound.
//!
//! VRAM (the screen is owned while this runs; the field restores everything afterwards):
//! - BG3: bag (`birch_bag.bin`), char block 0, screen block 6, priority 1; BG2: grass
//!   (`birch_grass.bin`), char block 0, screen block 7, priority 3 (Gen3 `sBgTemplates`);
//!   BG palettes from bank 0. BG1 is turned off.
//! - BG0: the text box (`layout::TEXT_CHAR_BLOCK`, its usual tiles) and the label canvas at
//!   [`LABEL_BASE`], palette `WINDOW_PALETTE`.
//! - OBJ tiles from 512: ball sheet (64: still 0, tilts 16 and 32, hand 48), circle (64), then
//!   the three front pics (64 each); OBJ palettes 0 ball, 1 circle, 2..5 pics.
//!
//! `starter_bag.tilemap` holds `birch_bag.bin` padded to 32x32 entries (1024), followed by
//! `birch_grass.bin` (1024); a shorter tilemap draws what it has.

use crate::data::{Font, GameData, SpeciesId};
use crate::flow::{FADE_MAX, Fader, Input};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::script::Fade;
use crate::text::TextBox;
use crate::video::{BgControl, Cmd, Frame, MAP_SIZE, Obj, ObjShape, Tile, layout};
use alloc::vec;
use alloc::vec::Vec;

/// `sPokeballCoords`: ball sprite centers.
pub const BALL_POS: [(i16, i16); 3] = [(60, 64), (120, 88), (180, 64)];
/// `sCursorCoords`: hand sprite centers.
pub const HAND_POS: [(i16, i16); 3] = [(60, 32), (120, 56), (180, 32)];
/// `sStarterLabelCoords`: label window (left, top) in tiles.
pub const LABEL_POS: [(u8, u8); 3] = [(0, 9), (16, 10), (8, 4)];
/// `STARTER_PKMN_POS_X` / `_Y`: where the circle and pic stop.
pub const PIC_POS: (i16, i16) = (120, 64);
/// `sWindowTemplate_StarterLabel` size in tiles.
const LABEL_W: usize = 13;
const LABEL_H: usize = 4;
/// Label canvas in `layout::TEXT_CHAR_BLOCK`, past the text box canvases.
pub const LABEL_BASE: u16 = 200;
/// The name is printed at y = 17: only label tile rows 2 and 3 hold glyph pixels.
const NAME_Y: usize = 17;
const NAME_ROW: usize = 2;
/// Gen3 `sTextColors` {transparent, white, light gray} in the text box palette.
const LABEL_FG: u32 = 1;
const LABEL_SHADOW: u32 = 3;
/// Frames of the affine grow (`AFFINEANIMCMD_FRAME(.., 0)` then `(.., 15)`).
const GROW_FRAMES: u8 = 16;

pub const OBJ_BALL: u16 = 512;
pub const OBJ_CIRCLE: u16 = 576;
pub const OBJ_PICS: u16 = 640;
const TILE_HAND: u16 = 48;
const PAL_BALL: u8 = 0;
const PAL_CIRCLE: u8 = 1;
const PAL_PICS: u8 = 2;
const PRIORITY: u8 = 1;
const BAG_BG: u8 = 3;
const BAG_SCREEN: u8 = 6;
const GRASS_BG: u8 = 2;
const GRASS_SCREEN: u8 = 7;

/// `Sin(i, 8)` for i = 0, 4, .. 252 (`(8 * gSineTable[i]) >> 8`): the hand's bob
/// (`SpriteCB_SelectionHand`, index + 4 per frame).
const BOB: [i8; 64] = [
    0, 0, 1, 2, 3, 3, 4, 5, 5, 6, 6, 7, 7, 7, 7, 7, 8, 7, 7, 7, 7, 7, 6, 6, 5, 5, 4, 3, 3, 2, 1, 0,
    0, -1, -2, -3, -4, -4, -5, -6, -6, -7, -7, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -7, -7,
    -6, -6, -5, -4, -4, -3, -2, -1,
];

/// `sAnim_Pokeball_Moving`: (tile, frames), looping.
const BALL_ANIM: [(u16, u16); 17] = [
    (16, 4),
    (0, 4),
    (32, 4),
    (0, 4),
    (16, 4),
    (0, 4),
    (32, 4),
    (0, 4),
    (0, 32),
    (16, 8),
    (0, 8),
    (32, 8),
    (0, 8),
    (16, 8),
    (0, 8),
    (32, 8),
    (0, 8),
];

/// Ball sheet tile of the selected ball `t` frames after it was selected.
fn ball_frame(t: u16) -> u16 {
    let total: u16 = BALL_ANIM.iter().map(|a| a.1).sum();
    let mut t = t % total;
    for (tile, d) in BALL_ANIM {
        if t < d {
            return tile;
        }
        t -= d;
    }
    0
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    /// `load_step` chunk to do next.
    Load(u8),
    /// `Task_StarterChoose`: draw the label and the message box.
    Enter,
    /// `Task_HandleStarterChooseInput`.
    Choose,
    /// `Task_MoveStarterChooseCursor`: clear the label.
    MoveCursor,
    /// `Task_CreateStarterLabel`.
    CreateLabel,
    /// Circle and pic moving to the center; frames since A.
    Zoom(u8),
    /// The question is printing.
    Ask,
    YesNo,
    /// `Task_DeclineStarter`.
    Decline,
    /// Chosen (`Some(selection)`).
    Done,
}

impl Stage {
    fn tag(self) -> [u8; 2] {
        match self {
            Stage::Load(n) => [0, n],
            Stage::Enter => [1, 0],
            Stage::Choose => [2, 0],
            Stage::MoveCursor => [3, 0],
            Stage::CreateLabel => [4, 0],
            Stage::Zoom(n) => [5, n],
            Stage::Ask => [6, 0],
            Stage::YesNo => [7, 0],
            Stage::Decline => [8, 0],
            Stage::Done => [9, 0],
        }
    }
}

pub struct StarterChoice {
    choices: [SpeciesId; 3],
    stage: Stage,
    /// `tStarterSelection` (Gen3 starts on the middle ball).
    selection: u8,
    /// Hand bob index / 4 (`SpriteCB_SelectionHand` `data[1]`).
    bob: u8,
    /// Frames since the selected ball started its animation.
    ball_t: u16,
    /// The selection whose label is on screen.
    label: Option<u8>,
    /// Circle and pic center, while shown.
    pic: Option<(i16, i16)>,
    fader: Fader,
    text: TextBox,
}

impl StarterChoice {
    pub fn new(choices: [SpeciesId; 3]) -> Self {
        let mut fader = Fader::new();
        fader.start(Fade::FromBlack);
        Self {
            choices,
            stage: Stage::Load(0),
            selection: 1,
            bob: 0,
            ball_t: 0,
            label: None,
            pic: None,
            fader,
            text: TextBox::new(),
        }
    }

    /// The highlighted ball (0..3).
    pub const fn selection(&self) -> u8 {
        self.selection
    }

    /// Uploads one budgeted chunk while black; true when everything is loaded.
    /// Once loading is done it returns true and leaves `f` alone.
    pub fn load_step(&mut self, game: &impl GameData, f: &mut Frame) -> bool {
        let Stage::Load(n) = self.stage else {
            return true;
        };
        f.brightness = FADE_MAX;
        f.lighten = false;
        f.objs.clear();
        let gfx = game.battle_gfx();
        match n {
            0 => {
                let off = |layer| Cmd::Bg {
                    layer,
                    control: BgControl::default(),
                };
                f.cmds.push(off(1));
                for (layer, screen_block, priority) in
                    [(BAG_BG, BAG_SCREEN, 1), (GRASS_BG, GRASS_SCREEN, 3)]
                {
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
                let bag = &gfx.starter_bag;
                for (bank, colors) in bag.palettes.iter().enumerate().take(15) {
                    f.cmds.push(Cmd::BgPalette {
                        bank: bank as u8,
                        colors: *colors,
                    });
                }
                if !bag.tiles.is_empty() {
                    f.cmds.push(Cmd::BgTiles {
                        char_block: 0,
                        first: 0,
                        tiles: &bag.tiles[..bag.tiles.len().min(512)],
                    });
                }
            }
            1 => {
                let map = gfx.starter_bag.tilemap;
                let screen = MAP_SIZE * MAP_SIZE;
                for (i, sb) in [BAG_SCREEN, GRASS_SCREEN].into_iter().enumerate() {
                    f.cmds.push(Cmd::MapFill {
                        screen_block: sb,
                        entry: 0,
                    });
                    let part = map.get(i * screen..).unwrap_or(&[]);
                    let part = &part[..part.len().min(screen)];
                    if !part.is_empty() {
                        f.cmds.push(Cmd::MapEntries {
                            screen_block: sb,
                            start: 0,
                            entries: part.to_vec(),
                        });
                    }
                }
            }
            2 => {
                for (img, first, bank) in [
                    (&gfx.starter_ball, OBJ_BALL, PAL_BALL),
                    (&gfx.starter_circle, OBJ_CIRCLE, PAL_CIRCLE),
                ] {
                    if !img.tiles.is_empty() {
                        f.cmds.push(Cmd::ObjTiles {
                            first,
                            tiles: &img.tiles[..img.tiles.len().min(64)],
                        });
                    }
                    if let Some(colors) = img.palettes.first() {
                        f.cmds.push(Cmd::ObjPalette {
                            bank,
                            colors: *colors,
                        });
                    }
                }
                // The label rows above the name stay filled (see `label_name_tiles`).
                let fill = fill_word(game.window().text_fg);
                for i in 0..(NAME_ROW * LABEL_W) as u16 {
                    f.cmds.push(Cmd::BgTile {
                        char_block: layout::TEXT_CHAR_BLOCK,
                        index: LABEL_BASE + i,
                        tile: [fill; 8],
                    });
                }
            }
            3 => {
                for (i, &species) in self.choices.iter().enumerate() {
                    let Some(pic) = game
                        .battle()
                        .species
                        .get(species.0 as usize)
                        .and_then(|s| s.gfx)
                    else {
                        continue;
                    };
                    f.cmds.push(Cmd::ObjTiles {
                        first: OBJ_PICS + 64 * i as u16,
                        tiles: &pic.front[..pic.front.len().min(64)],
                    });
                    f.cmds.push(Cmd::ObjPalette {
                        bank: PAL_PICS + i as u8,
                        colors: *pic.palette,
                    });
                }
            }
            _ => {
                self.text.open(game, f);
                self.stage = Stage::Enter;
                return true;
            }
        }
        self.stage = Stage::Load(n + 1);
        false
    }

    /// One frame; `Some(index)` once a starter is confirmed and the screen is black.
    pub fn update(&mut self, game: &impl GameData, input: &Input, f: &mut Frame) -> Option<u8> {
        if let Stage::Load(_) = self.stage {
            self.load_step(game, f);
            return None;
        }
        f.scroll = [(0, 0); 4];
        f.lighten = false;
        let pressed = input.pressed;
        match self.stage {
            Stage::Load(_) => unreachable!(),
            Stage::Enter => {
                self.draw_label(game, f);
                self.stage = Stage::Choose;
            }
            Stage::Choose => {
                if pressed.contains(Buttons::A) {
                    self.clear_label(f);
                    let (x, y) = BALL_POS[self.selection as usize];
                    self.pic = Some((x, y));
                    self.move_pic();
                    self.stage = Stage::Zoom(1);
                } else if pressed.contains(Buttons::LEFT) && self.selection > 0 {
                    self.select(self.selection - 1);
                } else if pressed.contains(Buttons::RIGHT) && self.selection < 2 {
                    self.select(self.selection + 1);
                }
            }
            Stage::MoveCursor => {
                self.clear_label(f);
                self.stage = Stage::CreateLabel;
            }
            Stage::CreateLabel => {
                self.draw_label(game, f);
                self.stage = Stage::Choose;
            }
            Stage::Zoom(n) => {
                if n >= GROW_FRAMES && self.pic == Some(PIC_POS) {
                    // `Task_AskConfirmStarter`.
                    let q = game.battle_text(crate::battle::Str::ConfirmStarterChoice);
                    self.text.print(game, q, f);
                    self.stage = Stage::Ask;
                } else {
                    self.stage = Stage::Zoom(n.saturating_add(1));
                }
                self.move_pic();
            }
            Stage::Ask => {
                self.text.update(game, pressed, input.held, f);
                if !self.text.is_printing() {
                    self.text.start_yes_no(game, f);
                    self.stage = Stage::YesNo;
                }
            }
            // `Menu_ProcessInputNoWrapClearOnChoose` inside the task: the answer counts on the
            // frame it is pressed.
            Stage::YesNo => {
                self.text.update(game, pressed, input.held, f);
                match self.text.yes_no_result() {
                    Some(true) => self.stage = Stage::Done,
                    Some(false) => {
                        self.pic = None;
                        self.stage = Stage::Decline;
                    }
                    None => {}
                }
            }
            Stage::Decline => {
                // Back to `Task_StarterChoose`: the box is cleared (it would reprint
                // `gText_BirchInTrouble`).
                self.text.open(game, f);
                self.stage = Stage::Enter;
            }
            Stage::Done => {}
        }
        self.animate(f);
        f.brightness = self.fader.update();
        (self.stage == Stage::Done).then_some(self.selection)
    }

    fn select(&mut self, s: u8) {
        self.selection = s;
        self.ball_t = 0;
        self.stage = Stage::MoveCursor;
    }

    /// `SpriteCB_StarterPokemon`.
    fn move_pic(&mut self) {
        if let Some((x, y)) = &mut self.pic {
            *x += (PIC_POS.0 - *x).clamp(-4, 4);
            *y += (PIC_POS.1 - *y).clamp(-2, 2);
        }
    }

    /// Sprites for this frame, then the per-frame sprite callbacks.
    fn animate(&mut self, f: &mut Frame) {
        f.objs.clear();
        let (square32, size32) = (ObjShape::Square, 2);
        let obj = |x: i16, y: i16, half: i16, tile: u16, palette: u8, priority: u8, size: u8| Obj {
            x: x - half,
            y: y - half,
            tile,
            palette,
            shape: square32,
            size,
            priority,
            ..Obj::default()
        };
        if let Some((x, y)) = self.pic {
            let i = self.selection as u16;
            f.objs
                .push(obj(x, y, 32, OBJ_PICS + 64 * i, PAL_PICS + i as u8, 0, 3));
            f.objs
                .push(obj(x, y, 32, OBJ_CIRCLE, PAL_CIRCLE, PRIORITY, 3));
        }
        let (hx, hy) = HAND_POS[self.selection as usize];
        let bob = BOB[self.bob as usize] as i16;
        f.objs.push(obj(
            hx,
            hy + bob,
            16,
            OBJ_BALL + TILE_HAND,
            PAL_BALL,
            PRIORITY,
            size32,
        ));
        for (i, &(x, y)) in BALL_POS.iter().enumerate() {
            let frame = if i as u8 == self.selection {
                ball_frame(self.ball_t)
            } else {
                0
            };
            f.objs
                .push(obj(x, y, 16, OBJ_BALL + frame, PAL_BALL, PRIORITY, size32));
        }
        self.bob = (self.bob + 1) % BOB.len() as u8;
        self.ball_t = self.ball_t.wrapping_add(1);
    }

    fn label_entries(sel: u8, visible: bool) -> Vec<(u16, Vec<u16>)> {
        let (left, top) = LABEL_POS[sel as usize];
        let pal = (layout::WINDOW_PALETTE as u16) << 12;
        (0..LABEL_H)
            .map(|row| {
                let start = (top as usize + row) * MAP_SIZE + left as usize;
                let entries = (0..LABEL_W)
                    .map(|col| {
                        if visible {
                            (LABEL_BASE + (row * LABEL_W + col) as u16) | pal
                        } else {
                            0
                        }
                    })
                    .collect();
                (start as u16, entries)
            })
            .collect()
    }

    /// `CreateStarterPokemonLabel`: the species name centered in 104 px at y = 17.
    fn draw_label(&mut self, game: &impl GameData, f: &mut Frame) {
        let sel = self.selection;
        let name = game
            .battle()
            .species
            .get(self.choices[sel as usize].0 as usize)
            .map_or(&[0xFFu8][..], |s| s.name);
        let tiles = label_name_tiles(game.font(), name, game.window().text_fg);
        for (i, tile) in tiles.into_iter().enumerate() {
            f.cmds.push(Cmd::BgTile {
                char_block: layout::TEXT_CHAR_BLOCK,
                index: LABEL_BASE + (NAME_ROW * LABEL_W) as u16 + i as u16,
                tile,
            });
        }
        for (start, entries) in Self::label_entries(sel, true) {
            f.cmds.push(Cmd::MapEntries {
                screen_block: text_screen_block(),
                start,
                entries,
            });
        }
        self.label = Some(sel);
    }

    /// `ClearStarterLabel`.
    fn clear_label(&mut self, f: &mut Frame) {
        if let Some(sel) = self.label.take() {
            for (start, entries) in Self::label_entries(sel, false) {
                f.cmds.push(Cmd::MapEntries {
                    screen_block: text_screen_block(),
                    start,
                    entries,
                });
            }
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&self.stage.tag());
        h.write(&[self.selection, self.bob, self.label.map_or(0xFF, |l| l)]);
        h.write_u16(self.ball_t);
        let (px, py) = self.pic.unwrap_or((-1, -1));
        h.write(&[self.pic.is_some() as u8]);
        h.write_u16(px as u16);
        h.write_u16(py as u16);
        for c in self.choices {
            h.write_u16(c.0);
        }
        self.fader.hash(h);
        self.text.hash(h);
    }
}

const fn fill_word(c: u8) -> u32 {
    (c as u32 & 0xF) * 0x1111_1111
}

const fn text_screen_block() -> u8 {
    layout::SCREEN_BLOCKS[layout::TEXT_BG as usize]
}

/// Label tile rows 2..4 (13 tiles each) with `name` (Gen3-encoded) centered in 104 px
/// (`GetStringCenterAlignXOffset(FONT_NORMAL, name, 0x68)`) at y = 17, as Gen3
/// `CopyGlyphToWindow`: 2bpp values 1 -> white, 2 -> light gray, 0/3 -> background, on a
/// canvas filled with color `fill`.
fn label_name_tiles(font: &Font, name: &[u8], fill: u8) -> Vec<Tile> {
    let glyphs: Vec<u16> = name
        .iter()
        .take_while(|&&b| b != 0xFF)
        .map(|&b| b as u16)
        .collect();
    let width_of = |g: u16| font.widths.get(g as usize).copied().unwrap_or(0) as usize;
    let width: usize = glyphs.iter().map(|&g| width_of(g)).sum();
    let mut x = (LABEL_W * 8).saturating_sub(width) / 2;
    let mut tiles = vec![[fill_word(fill); 8]; LABEL_W * 2];
    let y0 = NAME_Y - NAME_ROW * 8;
    for g in glyphs {
        let (Some(glyph), w) = (font.glyphs.get(g as usize), width_of(g)) else {
            continue;
        };
        for row in 0..15 {
            let py = y0 + row;
            if py >= 16 {
                break;
            }
            for col in 0..w.min(16) {
                let px = x + col;
                if px >= LABEL_W * 8 {
                    break;
                }
                // `.latfont` rows are u16s stored little-endian: the high byte holds the left
                // four pixels, leftmost in bits 7-6.
                let off = ((row / 8) * 2 + col / 8) * 16 + (row % 8) * 2 + 1 - (col % 8) / 4;
                let v = (glyph[off] >> (6 - 2 * (col % 4))) & 3;
                let c = match v {
                    1 => LABEL_FG,
                    2 => LABEL_SHADOW,
                    _ => continue,
                };
                let t = &mut tiles[(py / 8) * LABEL_W + px / 8][py % 8];
                let shift = 4 * (px % 8);
                *t = (*t & !(0xF << shift)) | c << shift;
            }
        }
        x += w;
    }
    tiles
}

#[cfg(test)]
mod tests;
