//! Mon and trainer pics: 64x64 OBJs at the Gen3 battler coordinates (`sBattlerCoords`,
//! `GetBattlerSpriteCoord`, `GetBattlerSpriteFinal_Y`) and their animations.
//!
//! Gen3 sources: wild intro `SpriteCB_MoveWildMonToRight` (2 px/frame, gray tint while
//! sliding), trainer slides `SpriteCB_TrainerSlideIn` and `StartAnimLinearTranslation`
//! (`*HandleIntroTrainerBallThrow`, `OpponentHandleTrainerSlide`), send-out
//! `Task_DoPokeballSendOutAnim` (player ball arc from (24, 68) in 25 frames, opponent ball 16
//! frames at its spot), faint `SpriteCB_AnimFaintOpponent` (8 px down every 2 frames, erasing a
//! tile row) and `SpriteCB_FaintSlideAnim` (5 px/frame until below the screen), hit blink
//! `DoHitAnimBlinkSpriteEffect` (visibility toggled every 4 frames for 32 frames; a hidden
//! sprite stays hidden). Tint palettes are uploaded only when the coefficient changes.
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
    Cmd, Frame, Obj, ObjShape, SCREEN_H, SCREEN_W, Tile, layout::battle::OBJ_PRIORITY_MON,
};

/// Stat raised / lowered tints (RGB555).
pub const TINT_STAT_UP: u16 = 31;
pub const TINT_STAT_DOWN: u16 = 31 << 10;

/// `sBattlerCoords[0]`: singles battler centres.
const PLAYER_XY: (i16, i16) = (72, 80);
const OPPONENT_XY: (i16, i16) = (176, 40);
/// `sBattlerCoords[1]` (doubles), by battler position.
const DOUBLE_XY: [(i16, i16); 4] = [(32, 80), (200, 40), (90, 88), (152, 32)];

/// The Gen3 resting centre of `side`'s pic (`sBattlerCoords`).
const fn coords(side: Side, double: bool) -> (i16, i16) {
    if double {
        DOUBLE_XY[side.i()]
    } else if side.is_player() {
        PLAYER_XY
    } else {
        OPPONENT_XY
    }
}
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
/// Tiles per [`MonSprite::load_step`].
const LOAD_STEP_TILES: usize = 16;

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
    /// Doubles positions (`sBattlerCoords[1]`).
    double: bool,
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
    /// Tint coefficient last uploaded (palettes are sent only when it changes).
    tint_coeff: Option<u8>,
    /// Hidden by the hit blink (`visible` is kept).
    blink_off: bool,
    /// Ball OBJ centre while it shows.
    ball: Option<(i16, i16)>,
    /// A `load_step` in progress: (species, shiny, tiles uploaded).
    loading: Option<(SpeciesId, bool, u8)>,
}

impl MonSprite {
    pub fn new(side: Side, gfx: &'static BattleGfx) -> Self {
        Self::with(side, false, gfx)
    }

    /// [`MonSprite::new`] at the singles (`double` false) or doubles positions.
    pub fn with(side: Side, double: bool, gfx: &'static BattleGfx) -> Self {
        MonSprite {
            side,
            double,
            species: None,
            gfx,
            palette: [0; 16],
            home: coords(side, double),
            front_y: 0,
            off: (0, 0),
            visible: false,
            anim: MonAnim::Idle,
            tint: None,
            tint_coeff: None,
            blink_off: false,
            ball: None,
            loading: None,
        }
    }

    const fn tile_base(&self) -> u16 {
        match (self.side.is_player(), self.side.is_right()) {
            (true, false) => obj::PLAYER_PIC,
            (false, false) => obj::ENEMY_PIC,
            (true, true) => obj::PLAYER2_PIC,
            (false, true) => obj::ENEMY2_PIC,
        }
    }

    const fn palette_bank(&self) -> u8 {
        match (self.side.is_player(), self.side.is_right()) {
            (true, false) => obj::PAL_PLAYER_MON,
            (false, false) => obj::PAL_ENEMY_MON,
            (true, true) => obj::PAL_PLAYER2_MON,
            (false, true) => obj::PAL_ENEMY2_MON,
        }
    }

    fn mon_gfx(game: &impl GameData, species: SpeciesId) -> Option<&'static MonGfx> {
        game.battle()
            .species
            .get(species.0 as usize)
            .and_then(|s| s.gfx)
    }

    /// Loads `species`' pic (front for the opponent, back for the player), its palette and the
    /// ball frame in one go: a bulk upload, for black frames only. Hidden and at rest
    /// afterwards. A species without pics draws nothing. Mid-battle, use
    /// [`MonSprite::load_step`].
    pub fn load(&mut self, game: &impl GameData, species: SpeciesId, shiny: bool, f: &mut Frame) {
        self.load_gfx(species, Self::mon_gfx(game, species), shiny, f);
    }

    /// [`MonSprite::load`] from the pic data directly.
    pub fn load_gfx(
        &mut self,
        species: SpeciesId,
        gfx: Option<&'static MonGfx>,
        shiny: bool,
        f: &mut Frame,
    ) {
        let tiles = self.prepare(species, gfx, shiny);
        if gfx.is_none() {
            return;
        }
        if !tiles.is_empty() {
            f.cmds.push(Cmd::ObjTiles {
                first: self.tile_base(),
                tiles,
            });
        }
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

    /// Mid-battle load (a send-out, a pic erased by a faint): as [`MonSprite::load`] but at
    /// most 16 tiles per frame and only what is left of the frame's
    /// [`super::OBJ_TILES_PER_FRAME`] (4 frames for a pic), the palette with the last chunk.
    /// The sprite is hidden meanwhile; call it every frame with the same arguments until it
    /// returns true. The ball frame is not re-sent ([`MonSprite::load`] uploaded it at battle
    /// start).
    pub fn load_step(
        &mut self,
        game: &impl GameData,
        species: SpeciesId,
        shiny: bool,
        f: &mut Frame,
    ) -> bool {
        let gfx = Self::mon_gfx(game, species);
        let done = match self.loading {
            Some((s, sh, done)) if s == species && sh == shiny => done as usize,
            _ => {
                self.prepare(species, gfx, shiny);
                0
            }
        };
        let tiles = self.pic_tiles(gfx);
        let n = (tiles.len() - done)
            .min(LOAD_STEP_TILES)
            .min(super::obj_tiles_left(f));
        if n > 0 {
            f.cmds.push(Cmd::ObjTiles {
                first: self.tile_base() + done as u16,
                tiles: &tiles[done..done + n],
            });
        }
        if done + n < tiles.len() {
            self.loading = Some((species, shiny, (done + n) as u8));
            return false;
        }
        if gfx.is_some() {
            f.cmds.push(Cmd::ObjPalette {
                bank: self.palette_bank(),
                colors: self.palette,
            });
        }
        self.loading = None;
        true
    }

    /// Whether a [`MonSprite::load_step`] is in progress.
    pub fn loading(&self) -> bool {
        self.loading.is_some()
    }

    fn pic_tiles(&self, gfx: Option<&'static MonGfx>) -> &'static [Tile] {
        let Some(g) = gfx else {
            return &[];
        };
        let tiles = if self.side.is_player() {
            g.back
        } else {
            g.front
        };
        &tiles[..tiles.len().min(PIC_TILES)]
    }

    /// Resets the sprite for `species` (hidden, at rest, Gen3 position) and returns its tiles.
    fn prepare(
        &mut self,
        species: SpeciesId,
        gfx: Option<&'static MonGfx>,
        shiny: bool,
    ) -> &'static [Tile] {
        self.species = Some(species);
        self.off = (0, 0);
        self.visible = false;
        self.blink_off = false;
        self.anim = MonAnim::Idle;
        self.tint = None;
        self.tint_coeff = None;
        self.ball = None;
        self.loading = None;
        let (cx, cy) = coords(self.side, self.double);
        let Some(g) = gfx else {
            self.home = (cx, cy);
            self.palette = [0; 16];
            self.front_y = 0;
            return &[];
        };
        // `GetBattlerSpriteFinal_Y` (default, no pic-offset clamp): the player adds the back
        // pic offset, the opponent the front offset minus its elevation.
        self.home = if self.side.is_player() {
            (cx, cy + g.back_y as i16)
        } else {
            (cx, cy + g.front_y as i16 - g.elevation as i16)
        };
        self.front_y = g.front_y;
        self.palette = if shiny { *g.shiny } else { *g.palette };
        self.pic_tiles(gfx)
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
        self.tint_coeff = None;
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
        self.anim = if self.side.is_player() {
            MonAnim::FaintSlide
        } else {
            MonAnim::FaintSink {
                row: 8 - (self.front_y / 8) as i8,
                wait: 1,
            }
        };
    }

    /// Hit: the pic blinks (hidden / shown every 4 frames) for 32 frames. A hidden sprite
    /// stays hidden.
    pub fn hit(&mut self) {
        self.blink_off = false;
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
        self.tint_coeff = None;
    }

    /// Hides the pic for the target cursor's blink (no animation: the scene stays idle).
    pub fn set_blink_off(&mut self, off: bool) {
        if self.anim == MonAnim::Idle {
            self.blink_off = off;
        }
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
        if self.side.is_player() {
            BALL_START
        } else {
            // `GetBattlerSpriteCoord(X)`, `(Y) + 24`.
            let (x, y) = coords(self.side, self.double);
            (x, y + 24)
        }
    }

    /// Where the player's ball lands: `GetBattlerSpriteCoord(X_2)`, `(Y_PIC_OFFSET) + 24`.
    fn ball_target(&self) -> (i16, i16) {
        match self.side.side() {
            Side::Player => {
                let y = (self.home.1 + 8).min((SCREEN_H as i16) - 64 + 8);
                (self.home.0, y + 24)
            }
            _ => self.ball_start(),
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
        if self.visible && !self.blink_off {
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
                match self.side.side() {
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
                    _ => {
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
                    self.blink_off = false;
                    MonAnim::Idle
                } else {
                    if t % 4 == 0 {
                        self.blink_off = !self.blink_off;
                    }
                    MonAnim::Blink { t: t + 1 }
                }
            }
            MonAnim::Lunge { t } => {
                let dir = if self.side.is_player() { 2 } else { -2 };
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
        if self.tint_coeff != Some(coeff) {
            f.cmds.push(Cmd::ObjPalette {
                bank: self.palette_bank(),
                colors: blend(&self.palette, coeff, tint.color),
            });
            self.tint_coeff = Some(coeff);
        }
        let finished = match tint.kind {
            TintKind::Hold(_) => false,
            TintKind::Fade(_) => coeff == 0,
            TintKind::Pulse => tint.t == 0,
        };
        self.tint = (!finished).then_some(tint);
        if finished {
            self.tint_coeff = None;
        }
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[
            side_tag(self.side),
            self.visible as u8,
            self.front_y,
            self.blink_off as u8,
            self.tint_coeff.unwrap_or(0xFF),
        ]);
        h.write_u16(self.species.map_or(0xFFFF, |s| s.0));
        for c in self.palette {
            h.write_u16(c);
        }
        match self.loading {
            None => h.write(&[0xFF]),
            Some((s, shiny, n)) => {
                h.write_u16(s.0);
                h.write(&[shiny as u8, n]);
            }
        }
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
    /// The pic's Gen3 `.size` (`TrainerPic::y`), 8 until a pic is loaded.
    size: u8,
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
            size: 8,
            home: Self::centre(side, 8),
            off: 0,
            visible: false,
            anim: TrainerAnim::Idle,
        }
    }

    /// Gen3 centres: `OpponentHandleDrawTrainerPic` y = `(8 - size) * 4 + 40` and
    /// `PlayerHandleDrawTrainerPic` y = `(8 - size) * 4 + 80`, `size` being the pic's
    /// `gTrainer{Front,Back}PicCoords[].size` ([`TrainerPic::y`]). Every Emerald pic has size 8,
    /// hence (176, 40) and (80, 80).
    const fn centre(side: Side, size: u8) -> (i16, i16) {
        let dy = (8 - size as i16) * 4;
        if side.is_player() {
            (80, dy + 80)
        } else {
            (176, dy + 40)
        }
    }

    const fn tile_base(&self) -> u16 {
        if self.side.is_player() {
            obj::PLAYER_BACK
        } else {
            obj::TRAINER_PIC
        }
    }

    const fn palette_bank(&self) -> u8 {
        if self.side.is_player() {
            obj::PAL_PLAYER_BACK
        } else {
            obj::PAL_TRAINER
        }
    }

    /// Uploads `pic` (frame 0: its first 64 tiles) and its palette; hidden at rest. A bulk
    /// upload, for black frames only.
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
        debug_assert!(pic.y <= 8, "trainer pic size {} > 8", pic.y);
        self.loaded = true;
        self.size = pic.y;
        self.home = Self::centre(self.side, pic.y);
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
        self.home = Self::centre(self.side, self.size);
        self.visible = true;
        let w = SCREEN_W as i16;
        let (off, speed) = if self.side.is_player() {
            (w, -2)
        } else {
            (-w, 2)
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
        let (target, frames) = if self.side.is_player() {
            (-40, 50)
        } else {
            (280, 35)
        };
        self.anim = TrainerAnim::Out(Linear::new(target - x, frames));
    }

    /// The defeated opponent comes back (`OpponentHandleTrainerSlide`): centre x 208, from
    /// 96 px to the right at 2 px/frame.
    pub fn slide_in_defeated(&mut self) {
        let (x, y) = Self::centre(self.side, self.size);
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
        h.write(&[
            side_tag(self.side),
            self.loaded as u8,
            self.visible as u8,
            self.size,
        ]);
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
mod tests;
