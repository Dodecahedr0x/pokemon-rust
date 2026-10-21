//! A fixed sequence of `Frame`s that exercises the video paths the World route does not reach
//! yet: `Frame::lighten` at every BLDY level, `Cmd::ObjTile`, and both battle transitions
//! (`engine::battle::transition`) over a colorful BG and sprites.
//!
//! Shared by the probe ROM (`src/bin/video_probe.rs`, which applies frame `i` at vblank and
//! publishes `i` as the debug frame) and the parity test (`crates/emu/tests/parity.rs`, which
//! renders the same frames with the native PPU). No game data: only the engine.

use alloc::vec;
use alloc::vec::Vec;
use engine::battle::BattleKind;
use engine::battle::transition::BattleTransition;
use engine::data::{TextId, TrainerId};
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

fn sprites() -> Vec<Obj> {
    (0..6)
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
        .collect()
}

/// The registers every probe frame starts from: BG3 scrolled a little, six 16x16 sprites.
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
            colors: core::array::from_fn(|c| color(bank as u32 * 16 + c as u32)),
        });
    }
    for bank in 0..2u8 {
        f.cmds.push(Cmd::ObjPalette {
            bank,
            colors: core::array::from_fn(|c| color(512 + bank as u32 * 16 + c as u32)),
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
    for index in 0..8u16 {
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
    End,
}

/// The probe sequence; [`Probe::next`] yields frame 0, 1, 2, ...
pub struct Probe {
    part: Part,
    transition: BattleTransition,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe {
    pub fn new() -> Self {
        Self {
            part: Part::Setup,
            transition: BattleTransition::new(&BattleKind::Wild {
                first_battle: false,
            }),
        }
    }

    /// True once the sequence has ended (`next` then returns empty frames, black).
    pub fn ended(&self) -> bool {
        self.part == Part::End
    }

    pub fn next(&mut self) -> Frame {
        let mut f = base();
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
            Part::WildHold(n) | Part::TrainerHold(n) => {
                f.brightness = BRIGHTNESS_BLACK;
                self.part = match (self.part, n + 1 < HOLD) {
                    (Part::WildHold(_), true) => Part::WildHold(n + 1),
                    (Part::WildHold(_), false) => Part::Reset,
                    (_, true) => Part::TrainerHold(n + 1),
                    _ => Part::End,
                };
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
            Part::End => {
                f.brightness = BRIGHTNESS_BLACK;
            }
        }
        f
    }
}

/// Every frame of the sequence up to its end, for host-side checks.
pub fn all_frames() -> Vec<Frame> {
    let mut p = Probe::new();
    let mut v = vec![];
    while !p.ended() {
        v.push(p.next());
    }
    v
}
