//! Mon and trainer pics: 64x64 OBJs at the Gen3 battler coordinates (`sBattlerCoords`,
//! `GetBattlerSpriteCoord`, `GetBattlerSpriteFinal_Y`) and their animations.
//!
//! Gen3 sources: wild intro `SpriteCB_MoveWildMonToRight` (2 px/frame, gray tint while
//! sliding), trainer slides `SpriteCB_TrainerSlideIn` and `StartAnimLinearTranslation`
//! (`*HandleIntroTrainerBallThrow`, `OpponentHandleTrainerSlide`), send-out
//! `Task_DoPokeballSendOutAnim` (player ball arc from (24, 68) in 25 frames, opponent ball 16
//! frames at its spot), faint `SpriteCB_AnimFaintOpponent` (8 px down every 2 frames, erasing a
//! tile row) and `SpriteCB_FaintSlideAnim` (5 px/frame until below the screen), hit blink
//! `DoHitAnimBlinkSpriteEffect` (visibility toggled every 4 frames for 32 frames).
//! Approximations: the ball arc is a parabola, the ball pops after 4 frames and the mon appears
//! at full size (no affine "emerge"), recall shows the ball for 8 frames, the lunge is 8 px out
//! and back in 8 frames, and the tint blends the pic palette toward a color (coefficient up to
//! 8/16 over 16 frames, then back over 16).

use super::super::{BattleGfx, Side};
use super::{Linear, hash_i16, obj, side_tag};
use crate::data::{GameData, SpeciesId};
use crate::hash::Fnv64;
use crate::mon::{MonGfx, TrainerPic};
use crate::video::{
    Cmd, Frame, Obj, ObjShape, SCREEN_H, SCREEN_W, layout::battle::OBJ_PRIORITY_MON,
};

/// Stat raised / lowered tints (RGB555).
pub const TINT_STAT_UP: u16 = 31;
pub const TINT_STAT_DOWN: u16 = 31 << 10;

/// `sBattlerCoords[0]`: singles battler centres.
const PLAYER_XY: (i16, i16) = (72, 80);
const OPPONENT_XY: (i16, i16) = (176, 40);
/// Half of a 64x64 pic (`centerToCornerVec`).
const HALF: i16 = 32;
const PIC_TILES: usize = 64;
/// `BeginNormalPaletteFade(0x20000, 0, 10, 10, RGB(8, 8, 8))` on the sliding wild mon, then
/// back to 0 by 2 per frame when it arrives.
const WILD_GRAY: u16 = 8 | 8 << 5 | 8 << 10;
const WILD_GRAY_Y: u8 = 10;
const SLIDE_SPEED: i16 = 2;
/// Player send-out: the ball starts at (24, 68) and arcs 30 px high for 25 frames.
const BALL_START: (i16, i16) = (24, 68);
const BALL_FRAMES: u8 = 25;
const BALL_ARC: i32 = 30;
/// `SpriteCB_OpponentMonSendOut`: the ball waits until its counter passes 15.
const BALL_WAIT: u8 = 16;
const BALL_POP: u8 = 4;
const RECALL_FRAMES: u8 = 8;
const BLINK_FRAMES: u8 = 32;
const LUNGE_FRAMES: u8 = 8;
const TINT_FRAMES: u8 = 32;
/// `SpriteCB_FaintSlideAnim` speed for the player's mon.
const FAINT_SPEED: i16 = 5;

/// Gen3 `BlendPalette`: each channel moves `coeff`/16 of the way to `color`.
fn blend(pal: &[u16; 16], coeff: u8, color: u16) -> [u16; 16] {
    let ch = |c: u16, s: u16| ((c >> s) & 31) as i32;
    let mut out = *pal;
    for c in out.iter_mut() {
        let mut v = 0;
        for s in [0, 5, 10] {
            let (a, b) = (ch(*c, s), ch(color, s));
            v |= ((a + (((b - a) * coeff as i32) >> 4)) as u16) << s;
        }
        *c = v;
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MonAnim {
    Idle,
    /// Wild intro slide: the offset grows by 2 to 0.
    SlideIn,
    /// Ball travelling (`t` frames done); then `Pop`.
    Ball {
        t: u8,
    },
    Pop {
        t: u8,
    },
    Recall {
        t: u8,
    },
    /// Opponent: `steps` left (`data[3]`), `wait` frames to the next step (`data[4]`).
    FaintSink {
        row: i8,
        wait: u8,
    },
    FaintSlide,
    Blink {
        t: u8,
    },
    Lunge {
        t: u8,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Tint {
    color: u16,
    /// Frames left; `None` coefficient path: wild gray uses a fixed coefficient until `fade`.
    t: u8,
    kind: TintKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TintKind {
    /// `coeff` held (wild mon while sliding).
    Hold(u8),
    /// Fading from `coeff` down by 2 per frame.
    Fade(u8),
    /// Up to 8 over 16 frames and back (`t` counts down from 32).
    Pulse,
}

/// One battler's pic.
#[derive(Clone)]
pub struct MonSprite {
    side: Side,
    species: Option<SpeciesId>,
    gfx: &'static BattleGfx,
    palette: [u16; 16],
    /// Gen3 sprite centre (x, y) of the resting position.
    home: (i16, i16),
    /// `gMonFrontPicCoords[].y_offset` (the faint erases from row `8 - y_offset / 8`).
    front_y: u8,
    off: (i16, i16),
    visible: bool,
    anim: MonAnim,
    tint: Option<Tint>,
    /// Ball OBJ centre while it shows.
    ball: Option<(i16, i16)>,
}

impl MonSprite {
    pub fn new(side: Side, gfx: &'static BattleGfx) -> Self {
        MonSprite {
            side,
            species: None,
            gfx,
            palette: [0; 16],
            home: match side {
                Side::Player => PLAYER_XY,
                Side::Opponent => OPPONENT_XY,
            },
            front_y: 0,
            off: (0, 0),
            visible: false,
            anim: MonAnim::Idle,
            tint: None,
            ball: None,
        }
    }

    const fn tile_base(&self) -> u16 {
        match self.side {
            Side::Player => obj::PLAYER_PIC,
            Side::Opponent => obj::ENEMY_PIC,
        }
    }

    const fn palette_bank(&self) -> u8 {
        match self.side {
            Side::Player => obj::PAL_PLAYER_MON,
            Side::Opponent => obj::PAL_ENEMY_MON,
        }
    }

    /// Loads `species`' pic (front for the opponent, back for the player), its palette and the
    /// ball frame; hidden and at rest afterwards. A species without pics draws nothing.
    pub fn load(&mut self, game: &impl GameData, species: SpeciesId, shiny: bool, f: &mut Frame) {
        let gfx = game
            .battle()
            .species
            .get(species.0 as usize)
            .and_then(|s| s.gfx);
        self.load_gfx(species, gfx, shiny, f);
    }

    /// [`MonSprite::load`] from the pic data directly.
    pub fn load_gfx(
        &mut self,
        species: SpeciesId,
        gfx: Option<&'static MonGfx>,
        shiny: bool,
        f: &mut Frame,
    ) {
        self.species = Some(species);
        self.off = (0, 0);
        self.visible = false;
        self.anim = MonAnim::Idle;
        self.tint = None;
        self.ball = None;
        let (cx, cy) = match self.side {
            Side::Player => PLAYER_XY,
            Side::Opponent => OPPONENT_XY,
        };
        let Some(g) = gfx else {
            self.home = (cx, cy);
            self.palette = [0; 16];
            return;
        };
        // `GetBattlerSpriteFinal_Y` (default, no pic-offset clamp): the player adds the back
        // pic offset, the opponent the front offset minus its elevation.
        self.home = match self.side {
            Side::Player => (cx, cy + g.back_y as i16),
            Side::Opponent => (cx, cy + g.front_y as i16 - g.elevation as i16),
        };
        self.front_y = g.front_y;
        let tiles = match self.side {
            Side::Player => g.back,
            Side::Opponent => g.front,
        };
        let n = tiles.len().min(PIC_TILES);
        if n > 0 {
            f.cmds.push(Cmd::ObjTiles {
                first: self.tile_base(),
                tiles: &tiles[..n],
            });
        }
        self.palette = if shiny { *g.shiny } else { *g.palette };
        f.cmds.push(Cmd::ObjPalette {
            bank: self.palette_bank(),
            colors: self.palette,
        });
        let ball = self.gfx.ball.tiles;
        if !ball.is_empty() {
            f.cmds.push(Cmd::ObjTiles {
                first: obj::BALL,
                tiles: &ball[..ball.len().min(4)],
            });
        }
        if let Some(p) = self.gfx.ball.palettes.first() {
            f.cmds.push(Cmd::ObjPalette {
                bank: obj::PAL_BALL,
                colors: *p,
            });
        }
    }

    /// Visible at rest.
    pub fn show(&mut self) {
        self.visible = true;
        self.off = (0, 0);
        self.anim = MonAnim::Idle;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.anim = MonAnim::Idle;
        self.ball = None;
    }

    /// Wild intro: slides in from 240 px to the left at 2 px/frame, tinted gray, which fades
    /// once it arrives (`SpriteCB_WildMon`, `SpriteCB_WildMonShowHealthbox`).
    pub fn slide_in(&mut self) {
        self.visible = true;
        self.off = (-(SCREEN_W as i16), 0);
        self.anim = MonAnim::SlideIn;
        self.tint = Some(Tint {
            color: WILD_GRAY,
            t: 0,
            kind: TintKind::Hold(WILD_GRAY_Y),
        });
    }

    /// Send-out: the ball travels (player) or waits (opponent), pops, and the mon appears.
    pub fn send_out(&mut self) {
        self.visible = false;
        self.off = (0, 0);
        self.anim = MonAnim::Ball { t: 0 };
        self.ball = Some(self.ball_start());
    }

    /// Back into the ball: the mon disappears and the ball shows for 8 frames.
    pub fn recall(&mut self) {
        self.visible = false;
        self.anim = MonAnim::Recall { t: 0 };
        self.ball = Some(self.ball_target());
    }

    /// Faint: the opponent sinks 8 px every 2 frames, erasing its pic row by row; the player's
    /// mon slides down 5 px/frame until it leaves the screen. Hidden at the end; reload the
    /// pic before showing it again.
    pub fn faint(&mut self) {
        self.anim = match self.side {
            Side::Opponent => MonAnim::FaintSink {
                row: 8 - (self.front_y / 8) as i8,
                wait: 1,
            },
            Side::Player => MonAnim::FaintSlide,
        };
    }

    /// Hit: visibility toggles every 4 frames for 32 frames.
    pub fn hit(&mut self) {
        self.anim = MonAnim::Blink { t: 0 };
    }

    /// The attacker moves 8 px toward its target and back.
    pub fn lunge(&mut self) {
        self.anim = MonAnim::Lunge { t: 0 };
    }

    /// Blends the palette toward `color` (RGB555) and back over 32 frames.
    pub fn tint(&mut self, color: u16) {
        self.tint = Some(Tint {
            color,
            t: TINT_FRAMES,
            kind: TintKind::Pulse,
        });
    }

    pub fn busy(&self) -> bool {
        self.anim != MonAnim::Idle || self.tint.is_some()
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn species(&self) -> Option<SpeciesId> {
        self.species
    }

    /// OBJ top-left this frame.
    pub fn pos(&self) -> (i16, i16) {
        (
            self.home.0 + self.off.0 - HALF,
            self.home.1 + self.off.1 - HALF,
        )
    }

    /// Ball centre at send-out start.
    fn ball_start(&self) -> (i16, i16) {
        match self.side {
            Side::Player => BALL_START,
            // `GetBattlerSpriteCoord(X)`, `(Y) + 24`.
            Side::Opponent => (OPPONENT_XY.0, OPPONENT_XY.1 + 24),
        }
    }

    /// Where the player's ball lands: `GetBattlerSpriteCoord(X_2)`, `(Y_PIC_OFFSET) + 24`.
    fn ball_target(&self) -> (i16, i16) {
        match self.side {
            Side::Player => {
                let y = (self.home.1 + 8).min((SCREEN_H as i16) - 64 + 8);
                (self.home.0, y + 24)
            }
            Side::Opponent => self.ball_start(),
        }
    }

    pub fn update(&mut self, f: &mut Frame) {
        self.step_anim(f);
        self.step_tint(f);
        if let Some((bx, by)) = self.ball {
            f.objs.push(Obj {
                x: bx - 8,
                y: by - 8,
                tile: obj::BALL,
                palette: obj::PAL_BALL,
                shape: ObjShape::Square,
                size: 1,
                priority: OBJ_PRIORITY_MON,
                ..Obj::default()
            });
        }
        if self.visible {
            let (x, y) = self.pos();
            f.objs.push(Obj {
                x,
                y,
                tile: self.tile_base(),
                palette: self.palette_bank(),
                shape: ObjShape::Square,
                size: 3,
                priority: OBJ_PRIORITY_MON,
                ..Obj::default()
            });
        }
    }

    fn step_anim(&mut self, f: &mut Frame) {
        self.anim = match self.anim {
            MonAnim::Idle => MonAnim::Idle,
            MonAnim::SlideIn => {
                self.off.0 += SLIDE_SPEED;
                if self.off.0 >= 0 {
                    self.off.0 = 0;
                    if let Some(t) = &mut self.tint {
                        t.kind = TintKind::Fade(WILD_GRAY_Y);
                    }
                    MonAnim::Idle
                } else {
                    MonAnim::SlideIn
                }
            }
            MonAnim::Ball { t } => {
                let t = t + 1;
                let (sx, sy) = self.ball_start();
                match self.side {
                    Side::Player => {
                        let (tx, ty) = self.ball_target();
                        let n = BALL_FRAMES as i32;
                        let k = t as i32;
                        let lerp = |a: i16, b: i16| a + ((b - a) as i32 * k / n) as i16;
                        let arc = (BALL_ARC * 4 * k * (n - k) / (n * n)) as i16;
                        self.ball = Some((lerp(sx, tx), lerp(sy, ty) - arc));
                        if t >= BALL_FRAMES {
                            MonAnim::Pop { t: 0 }
                        } else {
                            MonAnim::Ball { t }
                        }
                    }
                    Side::Opponent => {
                        if t >= BALL_WAIT {
                            MonAnim::Pop { t: 0 }
                        } else {
                            MonAnim::Ball { t }
                        }
                    }
                }
            }
            MonAnim::Pop { t } => {
                if t + 1 >= BALL_POP {
                    self.ball = None;
                    self.visible = true;
                    MonAnim::Idle
                } else {
                    MonAnim::Pop { t: t + 1 }
                }
            }
            MonAnim::Recall { t } => {
                if t + 1 >= RECALL_FRAMES {
                    self.ball = None;
                    MonAnim::Idle
                } else {
                    MonAnim::Recall { t: t + 1 }
                }
            }
            MonAnim::FaintSink { row, wait } => {
                if wait > 1 {
                    MonAnim::FaintSink {
                        row,
                        wait: wait - 1,
                    }
                } else {
                    self.off.1 += 8;
                    let row = row - 1;
                    if row < 0 {
                        self.visible = false;
                        MonAnim::Idle
                    } else {
                        // Erase the pic's tile row `row` (8 tiles of the 64x64 1D layout).
                        let first = self.tile_base() + row as u16 * 8;
                        for i in 0..8 {
                            f.cmds.push(Cmd::ObjTile {
                                index: first + i,
                                tile: [0; 8],
                            });
                        }
                        MonAnim::FaintSink { row, wait: 2 }
                    }
                }
            }
            MonAnim::FaintSlide => {
                self.off.1 += FAINT_SPEED;
                if self.home.1 + self.off.1 > SCREEN_H as i16 {
                    self.visible = false;
                    MonAnim::Idle
                } else {
                    MonAnim::FaintSlide
                }
            }
            MonAnim::Blink { t } => {
                if t == BLINK_FRAMES {
                    self.visible = true;
                    MonAnim::Idle
                } else {
                    if t % 4 == 0 {
                        self.visible = !self.visible;
                    }
                    MonAnim::Blink { t: t + 1 }
                }
            }
            MonAnim::Lunge { t } => {
                let dir = match self.side {
                    Side::Player => 2,
                    Side::Opponent => -2,
                };
                let t = t + 1;
                self.off.0 += if t <= LUNGE_FRAMES / 2 { dir } else { -dir };
                if t >= LUNGE_FRAMES {
                    self.off.0 = 0;
                    MonAnim::Idle
                } else {
                    MonAnim::Lunge { t }
                }
            }
        };
    }

    fn step_tint(&mut self, f: &mut Frame) {
        let Some(mut tint) = self.tint else {
            return;
        };
        let coeff = match tint.kind {
            TintKind::Hold(c) => c,
            TintKind::Fade(c) => {
                let next = c.saturating_sub(2);
                tint.kind = TintKind::Fade(next);
                c
            }
            TintKind::Pulse => {
                tint.t -= 1;
                // 32 frames: coefficient (32 - t) / 2 rising to 8, then t / 2 back to 0.
                let up = TINT_FRAMES - tint.t;
                if up <= TINT_FRAMES / 2 {
                    up / 2
                } else {
                    tint.t / 2
                }
            }
        };
        f.cmds.push(Cmd::ObjPalette {
            bank: self.palette_bank(),
            colors: blend(&self.palette, coeff, tint.color),
        });
        let finished = match tint.kind {
            TintKind::Hold(_) => false,
            TintKind::Fade(_) => coeff == 0,
            TintKind::Pulse => tint.t == 0,
        };
        self.tint = (!finished).then_some(tint);
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[side_tag(self.side), self.visible as u8, self.front_y]);
        h.write_u16(self.species.map_or(0xFFFF, |s| s.0));
        for v in [self.home.0, self.home.1, self.off.0, self.off.1] {
            hash_i16(h, v);
        }
        let (tag, a, b) = match self.anim {
            MonAnim::Idle => (0, 0, 0),
            MonAnim::SlideIn => (1, 0, 0),
            MonAnim::Ball { t } => (2, t, 0),
            MonAnim::Pop { t } => (3, t, 0),
            MonAnim::Recall { t } => (4, t, 0),
            MonAnim::FaintSink { row, wait } => (5, row as u8, wait),
            MonAnim::FaintSlide => (6, 0, 0),
            MonAnim::Blink { t } => (7, t, 0),
            MonAnim::Lunge { t } => (8, t, 0),
        };
        h.write(&[tag, a, b]);
        match self.tint {
            None => h.write(&[0xFF]),
            Some(t) => {
                let (k, c) = match t.kind {
                    TintKind::Hold(c) => (0, c),
                    TintKind::Fade(c) => (1, c),
                    TintKind::Pulse => (2, 0),
                };
                h.write(&[k, c, t.t]);
                h.write_u16(t.color);
            }
        }
        match self.ball {
            None => h.write(&[0xFF]),
            Some((x, y)) => {
                hash_i16(h, x);
                hash_i16(h, y);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrainerAnim {
    Idle,
    /// `SpriteCB_TrainerSlideIn`: x2 moves by `speed` until 0.
    Slide {
        speed: i16,
    },
    /// `StartAnimLinearTranslation` off screen, hidden at the end.
    Out(Linear),
}

/// The opponent trainer's front pic or the player's back pic (frame 0).
#[derive(Clone)]
pub struct TrainerSprite {
    side: Side,
    loaded: bool,
    /// Gen3 sprite centre.
    home: (i16, i16),
    off: i16,
    visible: bool,
    anim: TrainerAnim,
}

impl TrainerSprite {
    pub fn new(side: Side) -> Self {
        TrainerSprite {
            side,
            loaded: false,
            home: Self::centre(side),
            off: 0,
            visible: false,
            anim: TrainerAnim::Idle,
        }
    }

    /// `(8 - size) * 4 + 40` / `+ 80` with Emerald's pic size 8: (176, 40) and (80, 80).
    const fn centre(side: Side) -> (i16, i16) {
        match side {
            Side::Opponent => (176, 40),
            Side::Player => (80, 80),
        }
    }

    const fn tile_base(&self) -> u16 {
        match self.side {
            Side::Player => obj::PLAYER_BACK,
            Side::Opponent => obj::TRAINER_PIC,
        }
    }

    const fn palette_bank(&self) -> u8 {
        match self.side {
            Side::Player => obj::PAL_PLAYER_BACK,
            Side::Opponent => obj::PAL_TRAINER,
        }
    }

    /// Uploads `pic` (frame 0: its first 64 tiles) and its palette; hidden at rest.
    pub fn load(&mut self, pic: &'static TrainerPic, f: &mut Frame) {
        let n = pic.tiles.len().min(PIC_TILES);
        if n > 0 {
            f.cmds.push(Cmd::ObjTiles {
                first: self.tile_base(),
                tiles: &pic.tiles[..n],
            });
        }
        f.cmds.push(Cmd::ObjPalette {
            bank: self.palette_bank(),
            colors: *pic.palette,
        });
        self.loaded = true;
        self.home = Self::centre(self.side);
        self.off = 0;
        self.visible = false;
        self.anim = TrainerAnim::Idle;
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.off = 0;
        self.anim = TrainerAnim::Idle;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.anim = TrainerAnim::Idle;
    }

    /// Battle intro: from 240 px outside (left for the opponent, right for the player) at
    /// 2 px/frame.
    pub fn slide_in(&mut self) {
        self.home = Self::centre(self.side);
        self.visible = true;
        let w = SCREEN_W as i16;
        let (off, speed) = match self.side {
            Side::Opponent => (-w, 2),
            Side::Player => (w, -2),
        };
        self.off = off;
        self.anim = TrainerAnim::Slide { speed };
    }

    /// Leaves the screen: the opponent to x 280 in 35 frames (`OpponentHandleTrainerSlideBack`
    /// / intro ball throw), the player to x -40 in 50 frames (`PlayerHandleIntroTrainerBallThrow`).
    pub fn slide_out(&mut self) {
        let x = self.home.0 + self.off;
        self.home.0 = x;
        self.off = 0;
        let (target, frames) = match self.side {
            Side::Opponent => (280, 35),
            Side::Player => (-40, 50),
        };
        self.anim = TrainerAnim::Out(Linear::new(target - x, frames));
    }

    /// The defeated opponent comes back (`OpponentHandleTrainerSlide`): centre x 208, from
    /// 96 px to the right at 2 px/frame.
    pub fn slide_in_defeated(&mut self) {
        let (x, y) = Self::centre(self.side);
        self.home = (x + 32, y);
        self.visible = true;
        self.off = 96;
        self.anim = TrainerAnim::Slide { speed: -2 };
    }

    pub fn busy(&self) -> bool {
        self.anim != TrainerAnim::Idle
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// OBJ top-left this frame.
    pub fn pos(&self) -> (i16, i16) {
        (self.home.0 + self.off - HALF, self.home.1 - HALF)
    }

    pub fn update(&mut self, f: &mut Frame) {
        match &mut self.anim {
            TrainerAnim::Idle => {}
            TrainerAnim::Slide { speed } => {
                self.off += *speed;
                if self.off == 0 {
                    self.anim = TrainerAnim::Idle;
                }
            }
            TrainerAnim::Out(l) => match l.next() {
                Some(v) => self.off = v,
                None => {
                    self.visible = false;
                    self.anim = TrainerAnim::Idle;
                }
            },
        }
        if self.visible && self.loaded {
            let (x, y) = self.pos();
            f.objs.push(Obj {
                x,
                y,
                tile: self.tile_base(),
                palette: self.palette_bank(),
                shape: ObjShape::Square,
                size: 3,
                priority: OBJ_PRIORITY_MON,
                ..Obj::default()
            });
        }
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[side_tag(self.side), self.loaded as u8, self.visible as u8]);
        hash_i16(h, self.home.0);
        hash_i16(h, self.home.1);
        hash_i16(h, self.off);
        match self.anim {
            TrainerAnim::Idle => h.write(&[0]),
            TrainerAnim::Slide { speed } => {
                h.write(&[1]);
                hash_i16(h, speed);
            }
            TrainerAnim::Out(l) => {
                h.write(&[2]);
                l.hash(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_gfx::*;
    use super::*;
    use alloc::vec::Vec;

    fn run(s: &mut MonSprite) -> (usize, Frame) {
        let mut n = 0;
        let mut last = Frame::default();
        while s.busy() {
            last = Frame::default();
            s.update(&mut last);
            n += 1;
            assert!(n < 1000);
        }
        (n, last)
    }

    #[test]
    fn load_uploads_pic_palette_ball_and_places_gen3_coords() {
        let game = game(Some(mon_gfx(9, 4, 6)));
        let mut enemy = MonSprite::new(Side::Opponent, game.gfx);
        let mut f = Frame::default();
        enemy.load(&game, SpeciesId(1), false, &mut f);
        let g = game.tables.species[1].gfx.unwrap();
        assert_eq!(
            f.cmds[..2],
            [
                Cmd::ObjTiles {
                    first: obj::ENEMY_PIC,
                    tiles: g.front,
                },
                Cmd::ObjPalette {
                    bank: obj::PAL_ENEMY_MON,
                    colors: *g.palette,
                },
            ]
        );
        assert!(f.cmds.contains(&Cmd::ObjTiles {
            first: obj::BALL,
            tiles: &game.gfx.ball.tiles[..4],
        }));
        // Centre (176, 40 + 9 - 6) -> top-left (144, 11).
        enemy.show();
        let mut f = Frame::default();
        enemy.update(&mut f);
        assert_eq!(
            f.objs,
            [Obj {
                x: 144,
                y: 11,
                tile: obj::ENEMY_PIC,
                palette: obj::PAL_ENEMY_MON,
                shape: ObjShape::Square,
                size: 3,
                priority: OBJ_PRIORITY_MON,
                ..Obj::default()
            }]
        );
        let mut player = MonSprite::new(Side::Player, game.gfx);
        let mut f = Frame::default();
        player.load(&game, SpeciesId(1), true, &mut f);
        assert_eq!(
            f.cmds[..2],
            [
                Cmd::ObjTiles {
                    first: obj::PLAYER_PIC,
                    tiles: g.back,
                },
                Cmd::ObjPalette {
                    bank: obj::PAL_PLAYER_MON,
                    colors: *g.shiny,
                },
            ]
        );
        // Centre (72, 80 + 4) -> (40, 52).
        assert_eq!(player.pos(), (40, 52));
        // No pics: nothing uploaded, centre unchanged.
        let mut none = MonSprite::new(Side::Opponent, game.gfx);
        let mut f = Frame::default();
        none.load(&game, SpeciesId(0), false, &mut f);
        assert!(f.cmds.is_empty());
        assert_eq!(none.pos(), (144, 8));
    }

    #[test]
    fn wild_slide_in_takes_120_frames_then_fades_gray() {
        let game = game(Some(mon_gfx(8, 0, 0)));
        let mut s = MonSprite::new(Side::Opponent, game.gfx);
        s.load(&game, SpeciesId(1), false, &mut Frame::default());
        s.slide_in();
        let mut xs = Vec::new();
        let mut coeffs = Vec::new();
        while s.busy() {
            let mut f = Frame::default();
            s.update(&mut f);
            xs.push(f.objs[0].x);
            let pal = f.cmds.iter().find_map(|c| match c {
                Cmd::ObjPalette { bank: 0, colors } => Some(colors[0]),
                _ => None,
            });
            coeffs.push(pal.unwrap());
        }
        // 240 / 2 = 120 sliding frames; the gray fades 10, 8, 6, 4, 2, 0 from the arrival
        // frame (the last one restores the palette).
        assert_eq!(xs[0], 144 - 238);
        assert_eq!(xs[119], 144);
        assert_eq!(xs.len(), 125);
        let base = 0x100u16; // palette color 0 of the synthetic pic
        assert_eq!(coeffs[0], blend(&[base; 16], 10, WILD_GRAY)[0]);
        assert_eq!(coeffs[118], coeffs[0]);
        assert_eq!(coeffs[123], blend(&[base; 16], 2, WILD_GRAY)[0]);
        assert_eq!(coeffs[124], base);
        let mut f = Frame::default();
        s.update(&mut f);
        assert!(f.cmds.is_empty());
    }

    #[test]
    fn send_out_ball_then_mon() {
        let game = game(Some(mon_gfx(8, 0, 0)));
        let mut p = MonSprite::new(Side::Player, game.gfx);
        p.load(&game, SpeciesId(1), false, &mut Frame::default());
        p.send_out();
        let mut frames = Vec::new();
        while p.busy() {
            let mut f = Frame::default();
            p.update(&mut f);
            frames.push(f);
        }
        // 25 ball frames + 4 pop frames.
        assert_eq!(frames.len(), 29);
        let ball = |f: &Frame| {
            f.objs
                .iter()
                .find(|o| o.tile == obj::BALL)
                .map(|o| (o.x, o.y))
        };
        // Lands at (72, min(80 + 8, 104) + 24) = (72, 112): top-left (64, 104).
        assert_eq!(ball(&frames[24]), Some((64, 104)));
        assert!(frames[..28].iter().all(|f| f.objs.len() == 1));
        assert_eq!(frames[28].objs.len(), 1);
        assert_eq!(frames[28].objs[0].tile, obj::PLAYER_PIC);
        // The arc goes above the straight line.
        assert!(ball(&frames[12]).unwrap().1 < 68 - 8);
        let mut o = MonSprite::new(Side::Opponent, game.gfx);
        o.load(&game, SpeciesId(1), false, &mut Frame::default());
        o.send_out();
        let (n, last) = run(&mut o);
        assert_eq!(n, 16 + 4);
        assert_eq!(last.objs.len(), 1);
        assert!(o.visible());
        o.recall();
        let (n, _) = run(&mut o);
        assert_eq!(n, 8);
        assert!(!o.visible());
    }

    #[test]
    fn opponent_faint_sinks_and_erases_rows() {
        // front_y 16: data[3] = 8 - 2 = 6: 7 steps every 2 frames, the 7th hides it.
        let game = game(Some(mon_gfx(16, 0, 0)));
        let mut s = MonSprite::new(Side::Opponent, game.gfx);
        s.load(&game, SpeciesId(1), false, &mut Frame::default());
        s.show();
        s.faint();
        let mut frames = Vec::new();
        while s.busy() {
            let mut f = Frame::default();
            s.update(&mut f);
            frames.push(f);
        }
        assert_eq!(frames.len(), 13);
        // Step 1 (frame 1): down 8, erase row 5 (tiles 40..48).
        let erased: Vec<u16> = frames[0]
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::ObjTile { index, tile } if *tile == [0; 8] => Some(*index),
                _ => None,
            })
            .collect();
        assert_eq!(erased, (40..48).collect::<Vec<_>>());
        assert_eq!(frames[0].objs[0].y, 8 + 16 + 8);
        assert!(frames[1].cmds.is_empty());
        assert_eq!(frames[11].objs[0].y, 8 + 16 + 48);
        assert!(frames[12].objs.is_empty());
        assert!(frames.iter().all(|f| f.cmds.len() <= 16));
    }

    #[test]
    fn player_faint_slides_off_screen() {
        // Centre y 80 + 4: below 160 after ceil((160 - 84 + 1) / 5) = 16 frames.
        let game = game(Some(mon_gfx(0, 4, 0)));
        let mut s = MonSprite::new(Side::Player, game.gfx);
        s.load(&game, SpeciesId(1), false, &mut Frame::default());
        s.show();
        s.faint();
        let (n, last) = run(&mut s);
        assert_eq!(n, 16);
        assert!(last.objs.is_empty());
    }

    #[test]
    fn hit_blink_lunge_and_tint() {
        let game = game(Some(mon_gfx(0, 0, 0)));
        let mut s = MonSprite::new(Side::Player, game.gfx);
        s.load(&game, SpeciesId(1), false, &mut Frame::default());
        s.show();
        s.hit();
        let mut shown = Vec::new();
        while s.busy() {
            let mut f = Frame::default();
            s.update(&mut f);
            shown.push(!f.objs.is_empty());
        }
        // 32 toggle frames + the completion frame; toggles at 0, 4, .., 28.
        assert_eq!(shown.len(), 33);
        assert_eq!(
            shown[..8],
            [false, false, false, false, true, true, true, true]
        );
        assert!(shown[28..].iter().all(|&v| v));

        s.lunge();
        let mut xs = Vec::new();
        while s.busy() {
            let mut f = Frame::default();
            s.update(&mut f);
            xs.push(f.objs[0].x - 40);
        }
        assert_eq!(xs, [2, 4, 6, 8, 6, 4, 2, 0]);

        s.tint(TINT_STAT_UP);
        let mut coeffs = Vec::new();
        while s.busy() {
            let mut f = Frame::default();
            s.update(&mut f);
            let Some(Cmd::ObjPalette { colors, .. }) = f.cmds.first() else {
                panic!("palette expected")
            };
            coeffs.push(colors[0]);
        }
        assert_eq!(coeffs.len(), 32);
        let base = [0x100u16; 16];
        assert_eq!(coeffs[15], blend(&base, 8, TINT_STAT_UP)[0]);
        assert_eq!(coeffs[31], base[0]);
    }

    #[test]
    fn blend_is_gen3_blend_palette() {
        // r 0 -> 31 at 8/16: 0 + (31 * 8 >> 4) = 15; b 31 -> 0: 31 + (-31 * 8 >> 4) = 15.
        let out = blend(&[31 << 10; 16], 8, 31);
        assert_eq!(out[0], 15 | 15 << 10);
    }

    #[test]
    fn trainers_slide_in_and_out() {
        let pic = trainer_pic();
        let mut t = TrainerSprite::new(Side::Opponent);
        let mut f = Frame::default();
        t.load(pic, &mut f);
        assert_eq!(
            f.cmds,
            [
                Cmd::ObjTiles {
                    first: obj::TRAINER_PIC,
                    tiles: pic.tiles,
                },
                Cmd::ObjPalette {
                    bank: obj::PAL_TRAINER,
                    colors: *pic.palette,
                },
            ]
        );
        t.slide_in();
        let mut n = 0;
        while t.busy() {
            t.update(&mut Frame::default());
            n += 1;
        }
        assert_eq!(n, 120);
        assert_eq!(t.pos(), (144, 8));
        t.slide_out();
        let mut n = 0;
        while t.busy() {
            t.update(&mut Frame::default());
            n += 1;
        }
        // 35 moves + the completion frame; 176 + 103 at the last move.
        assert_eq!(n, 36);
        assert!(!t.visible());
        t.slide_in_defeated();
        let mut n = 0;
        while t.busy() {
            t.update(&mut Frame::default());
            n += 1;
        }
        assert_eq!(n, 48);
        assert_eq!(t.pos(), (208 - 32, 8));

        let gfx = gfx();
        let back: &'static TrainerPic = Box::leak(Box::new(gfx.player_back));
        let mut p = TrainerSprite::new(Side::Player);
        let mut f = Frame::default();
        p.load(back, &mut f);
        assert_eq!(
            f.cmds[0],
            Cmd::ObjTiles {
                first: obj::PLAYER_BACK,
                tiles: &back.tiles[..64],
            }
        );
        p.slide_in();
        let mut f = Frame::default();
        p.update(&mut f);
        assert_eq!(f.objs[0].x, 48 + 238);
        while p.busy() {
            p.update(&mut Frame::default());
        }
        assert_eq!(p.pos(), (48, 48));
        p.slide_out();
        let mut last = (0, 0);
        let mut n = 0;
        while p.busy() {
            let mut f = Frame::default();
            p.update(&mut f);
            if let Some(o) = f.objs.first() {
                last = (o.x, o.y);
            }
            n += 1;
        }
        assert_eq!(n, 51);
        assert_eq!(last, (-40 - 32, 48));
    }

    #[test]
    fn hashes_differ_by_state() {
        let game = game(Some(mon_gfx(0, 0, 0)));
        let mut s = MonSprite::new(Side::Player, game.gfx);
        let h = |s: &MonSprite| {
            let mut h = Fnv64::new();
            s.hash(&mut h);
            h.finish()
        };
        let h0 = h(&s);
        s.load(&game, SpeciesId(1), false, &mut Frame::default());
        let h1 = h(&s);
        assert_ne!(h0, h1);
        s.send_out();
        assert_ne!(h(&s), h1);
        let mut t = TrainerSprite::new(Side::Player);
        let mut a = Fnv64::new();
        t.hash(&mut a);
        t.slide_in();
        let mut b = Fnv64::new();
        t.hash(&mut b);
        assert_ne!(a.finish(), b.finish());
    }
}
