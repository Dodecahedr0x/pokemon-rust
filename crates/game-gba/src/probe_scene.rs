//! A fixed sequence of `Frame`s that exercises the video paths the World route does not reach
//! yet, over a colorful BG and sprites:
//! - `Frame::lighten` at every BLDY level (and darken);
//! - `Cmd::ObjTile`, including indices from 512 and 1023;
//! - both battle transitions (`engine::battle::transition`);
//! - the starter screen (`engine::starter::StarterChoice`) with a synthetic game of real sizes
//!   (256 bag tiles, 2x1024 map entries, 64-tile ball and circle sheets, three 64-tile pics):
//!   its black loading chunks, the choice, a decline, a confirmation, then the wild transition
//!   straight over it (as `CB2_GiveStarter`).
//!
//! Shared by the probe ROM (`src/bin/video_probe.rs`, which applies frame `i` at vblank and
//! publishes `i` as the debug frame) and the parity test (`crates/emu/tests/parity.rs`, which
//! renders the same frames with the native PPU and requires `late == 0`: every frame,
//! including the starter's loading chunks, applies inside vblank). No game data: only the
//! engine.
//!
//! What it does NOT prove (Task J's World route covers these): a transition mixed with the
//! field's own commands in the same frames (frozen objects, a field `TextBox` in BG0, a script
//! message on screen), the real content's sizes and text, the World's step time on top of the
//! transition, `late_black` accounting (nothing here is late, black or not), and restoring the
//! field after the starter screen or a battle.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use engine::Buttons;
use engine::battle::transition::BattleTransition;
use engine::battle::{BattleGfx, BattleKind, GfxImage, Str};
use engine::data::{
    Behavior, Dir, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpeciesId, SpriteData,
    TextId, TrainerId, UiText, WindowGfx,
};
use engine::flow::Input;
use engine::mon::{
    Ability, BattleTables, EMPTY_TABLES, GrowthRate, MonGfx, SpeciesInfo, TrainerPic, Type,
};
use engine::script::{Ctx, Script};
use engine::starter::StarterChoice;
use engine::video::{BRIGHTNESS_BLACK, BgControl, Cmd, Frame, MAP_SIZE, Obj, ObjShape, Tile};

/// Solid BG tiles of color 0..16 (color 0 shows the backdrop).
static SOLID: [Tile; 16] = {
    let mut t = [[0u32; 8]; 16];
    let mut c = 0;
    while c < 16 {
        t[c] = [c as u32 * 0x1111_1111; 8];
        c += 1;
    }
    t
};

const BG_LAYER: u8 = 3;
const BG_SCREEN: u8 = 30;
/// Frames held fully black after a transition, then the lighten and darken sweeps.
const HOLD: u32 = 2;
const SWEEP: u32 = BRIGHTNESS_BLACK as u32 + 1;

/// A deterministic spread of RGB555 colors (Knuth multiplicative hash).
const fn color(n: u32) -> u16 {
    ((n.wrapping_add(1).wrapping_mul(2_654_435_761) >> 15) & 0x7FFF) as u16
}

const fn palette(seed: u32) -> [u16; 16] {
    let mut p = [0u16; 16];
    let mut i = 0;
    while i < 16 {
        p[i] = color(seed * 16 + i as u32);
        i += 1;
    }
    p
}

/// `N` tiles of varied pixels.
const fn tiles<const N: usize>(seed: u32) -> [Tile; N] {
    let mut t = [[0u32; 8]; N];
    let mut i = 0;
    while i < N {
        let mut row = 0;
        while row < 8 {
            t[i][row] = (seed ^ (i as u32 * 8 + row as u32)).wrapping_mul(0x9E37_79B9);
            row += 1;
        }
        i += 1;
    }
    t
}

// ---- the synthetic game for the starter screen ----

static BAG_TILES: [Tile; 256] = tiles(1);
static BAG_PALETTES: [[u16; 16]; 2] = [palette(40), palette(41)];
/// Bag map (32x32, palette 0) then grass map (palette 1), as `BattleGfx::starter_bag` expects.
static BAG_MAPS: [u16; 2048] = {
    let mut m = [0u16; 2048];
    let mut i = 0;
    while i < 2048 {
        m[i] = ((i * 7 + i / 32) % 256) as u16 | ((i / 1024) as u16) << 12;
        i += 1;
    }
    m
};
static BALL_TILES: [Tile; 64] = tiles(2);
static CIRCLE_TILES: [Tile; 64] = tiles(3);
static OBJ_PALETTES: [[u16; 16]; 2] = [palette(50), palette(51)];
static PIC_TILES: [[Tile; 64]; 3] = [tiles(4), tiles(5), tiles(6)];
static PIC_PALETTES: [[u16; 16]; 3] = [palette(60), palette(61), palette(62)];
static PICS: [MonGfx; 3] = [pic(0), pic(1), pic(2)];

const fn pic(i: usize) -> MonGfx {
    MonGfx {
        front: &PIC_TILES[i],
        back: &PIC_TILES[i],
        palette: &PIC_PALETTES[i],
        shiny: &PIC_PALETTES[i],
        front_y: 0,
        back_y: 0,
        elevation: 0,
    }
}

const fn species(name: &'static [u8], i: usize) -> SpeciesInfo {
    SpeciesInfo {
        name,
        base: [50; 6],
        types: [Type::Normal; 2],
        catch_rate: 45,
        exp_yield: 64,
        ev_yield: [0; 6],
        gender_ratio: 31,
        friendship: 70,
        growth: GrowthRate::MediumSlow,
        abilities: [Ability::None; 2],
        learnset: &[],
        gfx: Some(&PICS[i]),
    }
}

static SPECIES: [SpeciesInfo; 3] = [
    species(&[0xBB, 0xC6, 0xC9, 0xFF], 0),
    species(&[0xC8, 0xC3, 0xBB, 0xC8, 0xC9, 0xFF], 1),
    species(&[0xBD, 0xCF, 0xC2, 0xFF], 2),
];
static TABLES: BattleTables = BattleTables {
    species: &SPECIES,
    ..EMPTY_TABLES
};

/// Every glyph: a 6-px wide pattern of fg, shadow and background pixels.
static GLYPHS: [[u8; 64]; 256] = {
    let mut g = [[0u8; 64]; 256];
    let mut c = 0;
    while c < 256 {
        let mut i = 0;
        while i < 64 {
            g[c][i] = ((c * 13 + i * 7) as u8) & 0b0110_1001;
            i += 1;
        }
        c += 1;
    }
    g
};
static WIDTHS: [u8; 256] = [6; 256];
static FONT: Font = Font {
    glyphs: &GLYPHS,
    widths: &WIDTHS,
};
static WINDOW_TILES: [Tile; 14] = tiles(7);
static WINDOW_PALETTE: [u16; 16] = palette(70);
static WINDOW: WindowGfx = WindowGfx {
    tiles: &WINDOW_TILES,
    palette: &WINDOW_PALETTE,
    text_fg: 2,
    text_shadow: 3,
    text_bg: 1,
};
static NO_PALETTE: [u16; 16] = [0; 16];
const fn image(tiles: &'static [Tile], palettes: &'static [[u16; 16]]) -> GfxImage {
    GfxImage {
        tiles,
        palettes,
        tilemap: &[],
    }
}
static GFX: BattleGfx = BattleGfx {
    textbox: GfxImage::EMPTY,
    environments: [GfxImage::EMPTY; 10],
    healthbox_player: GfxImage::EMPTY,
    healthbox_opponent: GfxImage::EMPTY,
    hpbar: GfxImage::EMPTY,
    expbar: GfxImage::EMPTY,
    status: GfxImage::EMPTY,
    misc: GfxImage::EMPTY,
    ball: GfxImage::EMPTY,
    small_font: &FONT,
    player_back: TrainerPic {
        tiles: &[],
        palette: &NO_PALETTE,
        y: 0,
    },
    starter_bag: GfxImage {
        tiles: &BAG_TILES,
        palettes: &BAG_PALETTES,
        tilemap: &BAG_MAPS,
    },
    starter_ball: image(&BALL_TILES, &OBJ_PALETTES),
    starter_circle: image(&CIRCLE_TILES, &OBJ_PALETTES),
};

/// Just enough of a game for the starter screen: no maps or scripts.
pub struct ProbeGame;

impl GameData for ProbeGame {
    fn map(&self, _: MapId) -> &'static MapData {
        unimplemented!("the probe has no maps")
    }
    fn sprite(&self, _: GfxId) -> Option<&'static SpriteData> {
        None
    }
    fn player_sprite(&self) -> &'static SpriteData {
        unimplemented!("the probe has no sprites")
    }
    fn font(&self) -> &'static Font {
        &FONT
    }
    fn window(&self) -> &'static WindowGfx {
        &WINDOW
    }
    fn text(&self, _: TextId) -> &'static [u8] {
        &[0xFF]
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => &[0xD3, 0xBF, 0xCD, 0xFF],
            UiText::No => &[0xC8, 0xC9, 0xFF],
            UiText::RivalName => &[0xFF],
        }
    }
    fn behavior(&self, _: u8) -> Behavior {
        Behavior::Normal
    }
    fn script(&self, _: ScriptId, _: Ctx) -> Script {
        Box::pin(async {})
    }
    fn new_game(&self) -> NewGame {
        NewGame {
            map: MapId(0),
            x: 0,
            y: 0,
            facing: Dir::Down,
            script: None,
            player_name: &[0xFF],
        }
    }
    fn battle(&self) -> &'static BattleTables {
        &TABLES
    }
    fn battle_text(&self, s: Str) -> &'static [u8] {
        match s {
            Str::ConfirmStarterChoice => &[0xBB, 0xBC, 0xBD, 0x00, 0xBE, 0xBF, 0xC0, 0xC1, 0xFF],
            _ => &[0xFF],
        }
    }
    fn battle_gfx(&self) -> &'static BattleGfx {
        &GFX
    }
}

/// Starter-screen input by update number (after loading): right, left, A, no (down + A),
/// A, yes. The confirmation lands on update [`STARTER_YES`].
fn starter_keys(k: u32) -> Buttons {
    match k {
        20 => Buttons::RIGHT,
        26 => Buttons::LEFT,
        32 => Buttons::A,
        100 => Buttons::DOWN,
        101 => Buttons::A,
        110 => Buttons::A,
        STARTER_YES => Buttons::A,
        _ => Buttons::NONE,
    }
}
pub const STARTER_YES: u32 = 180;

// ---- the sequence ----

fn sprites() -> Vec<Obj> {
    let mut v: Vec<Obj> = (0..6)
        .map(|j: i16| Obj {
            x: 12 + 37 * j,
            y: 24 + 19 * j,
            tile: (j as u16 % 2) * 4,
            palette: (j % 2) as u8,
            shape: ObjShape::Square,
            size: 1,
            priority: 2,
            ..Obj::default()
        })
        .collect();
    // The high OBJ tiles: a 16x16 at 512 and an 8x8 at 1023.
    v.push(Obj {
        x: 200,
        y: 20,
        tile: 512,
        palette: 1,
        size: 1,
        priority: 2,
        ..Obj::default()
    });
    v.push(Obj {
        x: 220,
        y: 140,
        tile: 1023,
        palette: 0,
        size: 0,
        priority: 2,
        ..Obj::default()
    });
    v
}

/// The registers every probe frame starts from: BG3 scrolled a little, the sprites.
fn base() -> Frame {
    let mut f = Frame {
        objs: sprites(),
        ..Frame::default()
    };
    f.scroll[BG_LAYER as usize] = (3, 5);
    f
}

/// Frame 0: palettes, BG3 tiles and map, OBJ tiles through `Cmd::ObjTile`.
fn setup() -> Frame {
    let mut f = base();
    for bank in 0..16u8 {
        f.cmds.push(Cmd::BgPalette {
            bank,
            colors: palette(bank as u32),
        });
    }
    for bank in 0..2u8 {
        f.cmds.push(Cmd::ObjPalette {
            bank,
            colors: palette(32 + bank as u32),
        });
    }
    for layer in 0..3 {
        f.cmds.push(Cmd::Bg {
            layer,
            control: BgControl::default(),
        });
    }
    f.cmds.push(Cmd::BgTiles {
        char_block: 0,
        first: 0,
        tiles: &SOLID,
    });
    let entries: Vec<u16> = (0..MAP_SIZE * MAP_SIZE)
        .map(|i| {
            let (x, y) = (i % MAP_SIZE, i / MAP_SIZE);
            (((x + y) % 16) | ((x / 2 + y) % 15) << 12) as u16
        })
        .collect();
    f.cmds.push(Cmd::MapEntries {
        screen_block: BG_SCREEN,
        start: 0,
        entries,
    });
    f.cmds.push(Cmd::Bg {
        layer: BG_LAYER,
        control: BgControl {
            enabled: true,
            priority: 3,
            char_block: 0,
            screen_block: BG_SCREEN,
        },
    });
    for index in (0..8u16).chain(512..516).chain([1023]) {
        let tile: Tile = core::array::from_fn(|row| {
            (0..8).fold(0u32, |w, x| {
                w | ((row as u32 + x + index as u32 * 3) % 16) << (4 * x)
            })
        });
        f.cmds.push(Cmd::ObjTile { index, tile });
    }
    f
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Setup,
    Wild,
    WildHold(u32),
    Reset,
    Lighten(u32),
    Darken(u32),
    Trainer,
    TrainerHold(u32),
    StarterLoad,
    /// Updates done.
    Starter(u32),
    /// The first battle's transition over the starter screen.
    StarterTransition,
    StarterHold(u32),
    End,
}

/// The probe sequence; [`Probe::next`] yields frame 0, 1, 2, ...
pub struct Probe {
    part: Part,
    transition: BattleTransition,
    starter: StarterChoice,
    chosen: Option<u8>,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

const WILD: BattleKind = BattleKind::Wild {
    first_battle: false,
};

impl Probe {
    pub fn new() -> Self {
        Self {
            part: Part::Setup,
            transition: BattleTransition::new(&WILD),
            starter: StarterChoice::new([SpeciesId(0), SpeciesId(1), SpeciesId(2)]),
            chosen: None,
        }
    }

    /// True once the sequence has ended (`next` then returns empty frames, black).
    pub fn ended(&self) -> bool {
        self.part == Part::End
    }

    /// The starter the scripted input confirmed.
    pub fn chosen(&self) -> Option<u8> {
        self.chosen
    }

    pub fn next(&mut self) -> Frame {
        let mut f = base();
        let hold = |n: u32, more: Part, then: Part| if n + 1 < HOLD { more } else { then };
        match self.part {
            Part::Setup => {
                f = setup();
                self.part = Part::Wild;
            }
            Part::Wild => {
                if self.transition.update(&mut f) {
                    self.part = Part::WildHold(0);
                }
            }
            Part::WildHold(n) => {
                f.brightness = BRIGHTNESS_BLACK;
                self.part = hold(n, Part::WildHold(n + 1), Part::Reset);
            }
            Part::Reset => {
                f.cmds.push(Cmd::Bg {
                    layer: 0,
                    control: BgControl::default(),
                });
                self.part = Part::Lighten(0);
            }
            Part::Lighten(n) => {
                f.lighten = true;
                f.brightness = n as u8;
                self.part = if n + 1 < SWEEP {
                    Part::Lighten(n + 1)
                } else {
                    Part::Darken(0)
                };
            }
            Part::Darken(n) => {
                f.brightness = n as u8;
                self.part = if n + 1 < SWEEP {
                    Part::Darken(n + 1)
                } else {
                    self.transition = BattleTransition::new(&BattleKind::Trainer {
                        trainer: TrainerId(0),
                        defeat: TextId(0),
                    });
                    Part::Trainer
                };
            }
            Part::Trainer => {
                if self.transition.update(&mut f) {
                    self.part = Part::TrainerHold(0);
                }
            }
            Part::TrainerHold(n) => {
                f.brightness = BRIGHTNESS_BLACK;
                self.part = hold(n, Part::TrainerHold(n + 1), Part::StarterLoad);
            }
            Part::StarterLoad => {
                f = Frame::default();
                if self.starter.load_step(&ProbeGame, &mut f) {
                    self.part = Part::Starter(0);
                }
            }
            Part::Starter(k) => {
                f = Frame::default();
                let keys = starter_keys(k);
                let input = Input {
                    held: keys,
                    pressed: keys,
                };
                self.chosen = self.starter.update(&ProbeGame, &input, &mut f);
                self.part = if self.chosen.is_some() {
                    self.transition =
                        BattleTransition::new(&BattleKind::Wild { first_battle: true });
                    Part::StarterTransition
                } else {
                    Part::Starter(k + 1)
                };
            }
            Part::StarterTransition => {
                f = Frame::default();
                self.starter.update(&ProbeGame, &Input::default(), &mut f);
                if self.transition.update(&mut f) {
                    self.part = Part::StarterHold(0);
                }
            }
            Part::StarterHold(n) => {
                f = Frame {
                    brightness: BRIGHTNESS_BLACK,
                    ..Frame::default()
                };
                self.part = hold(n, Part::StarterHold(n + 1), Part::End);
            }
            Part::End => {
                f.brightness = BRIGHTNESS_BLACK;
            }
        }
        f
    }
}

/// Every frame of the sequence up to its end, and the starter it confirmed.
pub fn all_frames() -> (Vec<Frame>, Option<u8>) {
    let mut p = Probe::new();
    let mut v = vec![];
    while !p.ended() {
        v.push(p.next());
    }
    (v, p.chosen())
}
