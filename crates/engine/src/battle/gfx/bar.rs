//! HP and EXP bars as Gen3 `MoveBattleBar` (`battle_interface.c`): `CalcNewBarValue` moves the
//! value one step per frame (8.8 fixed point when the maximum has fewer units than the bar has
//! pixels), `CalcBarFilledPixels` turns it into per-tile pixel counts, and
//! `MoveBattleBarGraphically` copies the matching element tiles (`Cmd::ObjTile`, changed tiles
//! only).

use super::super::{BattleGfx, Side};
use super::{obj, side_tag};
use crate::Tile;
use crate::hash::Fnv64;
use crate::video::{Cmd, Frame};

/// `B_HEALTHBAR_PIXELS / 8`.
const HP_TILES: usize = 6;
/// `B_EXPBAR_PIXELS / 8`.
const EXP_TILES: usize = 8;
/// `MAX_LEVEL`.
const MAX_LEVEL: u8 = 100;
/// `Task_GiveExpWithExpBar` waits this many frames before moving the bar.
const EXP_DELAY: u8 = 13;
/// First HP bar tile of each color in `BattleGfx::hpbar` (`HEALTHBOX_GFX_HP_BAR_*`).
const HP_COLOR_BASE: [usize; 3] = [3, 12, 21];
/// Bar tiles start after the "HP" label in the HP bar sprite.
const HP_LABEL_TILES: u16 = 2;
/// Exp bar tiles inside the player healthbox (`MoveBattleBarGraphically`, `EXP_BAR`): 0x24 + i
/// for i < 4, else 0x5C + i (`0xB80` bytes past the box).
const EXP_TILE: [u16; EXP_TILES] = [0x24, 0x25, 0x26, 0x27, 0x60, 0x61, 0x62, 0x63];

/// HP bar color (`GetHPBarLevel` thresholds on the drawn pixels).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarColor {
    Green,
    Yellow,
    Red,
}

/// Gen3 `MoveBattleBarGraphically`: green above 50% of 48 px, yellow above 20%, red otherwise.
pub const fn bar_color(filled_pixels: u8) -> BarColor {
    if filled_pixels as usize > HP_TILES * 8 * 50 / 100 {
        BarColor::Green
    } else if filled_pixels as usize > HP_TILES * 8 * 20 / 100 {
        BarColor::Yellow
    } else {
        BarColor::Red
    }
}

/// `-32768`: "first call" marker of `currValue`.
const FIRST: i32 = -32768;

/// One `gBattleSpritesDataPtr->battleBars[]` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BarState {
    max: i32,
    old: i32,
    /// `old - new`: positive drains, negative fills.
    received: i32,
    cur: i32,
}

impl BarState {
    const fn new(max: i32, old: i32, received: i32) -> Self {
        BarState {
            max,
            old,
            received,
            cur: FIRST,
        }
    }

    fn target(&self) -> i32 {
        (self.old - self.received).clamp(0, self.max.max(0))
    }

    /// Gen3 `CalcNewBarValue`: the next displayed value, or -1 when the bar has arrived.
    fn step(&mut self, tiles: usize, to_add: i32) -> i32 {
        let scale = tiles as i32 * 8;
        let small = self.max < scale;
        if self.cur == FIRST {
            self.cur = if small { self.old << 8 } else { self.old };
        }
        let new = self.target();
        if small {
            if new == self.cur >> 8 && self.cur & 0xFF == 0 {
                return -1;
            }
        } else if new == self.cur {
            return -1;
        }
        if small {
            let to_add = (self.max << 8) / scale;
            if self.received < 0 {
                self.cur += to_add;
                let ret = self.cur >> 8;
                if ret >= new {
                    self.cur = new << 8;
                    return new;
                }
                ret
            } else {
                self.cur -= to_add;
                let mut ret = self.cur >> 8;
                if self.cur & 0xFF > 0 {
                    ret += 1;
                }
                if ret <= new {
                    self.cur = new << 8;
                    return new;
                }
                ret
            }
        } else if self.received < 0 {
            self.cur = (self.cur + to_add).min(new);
            self.cur
        } else {
            self.cur = (self.cur - to_add).max(new);
            self.cur
        }
    }

    /// Gen3 `CalcBarFilledPixels`: per-tile pixel counts and the total filled pixels.
    fn pixels(&self, out: &mut [u8]) -> u8 {
        let total = out.len() as i32 * 8;
        out.fill(0);
        if self.max <= 0 {
            return 0;
        }
        let cur = if self.cur == FIRST { 0 } else { self.cur };
        let mut pixels = if self.max < total {
            (cur * total / self.max) >> 8
        } else {
            cur * total / self.max
        } as u8;
        let filled = pixels;
        if filled == 0 && self.target() > 0 {
            out[0] = 1;
            return 1;
        }
        for o in out.iter_mut() {
            if pixels >= 8 {
                *o = 8;
            } else {
                *o = pixels;
                break;
            }
            pixels -= 8;
        }
        filled
    }

    fn hash(&self, h: &mut Fnv64) {
        for v in [self.max, self.old, self.received, self.cur] {
            h.write_u32(v as u32);
        }
    }
}

fn tile_or_blank(tiles: &[Tile], i: usize) -> Tile {
    tiles.get(i).copied().unwrap_or([0; 8])
}

/// The 48 px HP bar of one side, drawn into its HP bar sprite tiles (`obj::*_BAR + 2..`). The
/// sprite itself is placed by the side's [`super::Healthbox`].
#[derive(Clone)]
pub struct HpBar {
    side: Side,
    gfx: &'static BattleGfx,
    bar: BarState,
    active: bool,
    value: u16,
    /// What VRAM holds: (color index, pixels per tile); `None` before the first draw.
    drawn: Option<(u8, [u8; HP_TILES])>,
}

impl HpBar {
    pub fn new(side: Side, gfx: &'static BattleGfx) -> Self {
        HpBar {
            side,
            gfx,
            bar: BarState::new(0, 0, 0),
            active: false,
            value: 0,
            drawn: None,
        }
    }

    /// Shows `hp` of `max` at once (`UpdateHealthboxAttribute(HEALTHBOX_HEALTH_BAR)`); the
    /// changed tiles are written to `f` now.
    pub fn set(&mut self, hp: u16, max: u16, f: &mut Frame) {
        self.bar = BarState::new(max as i32, hp as i32, 0);
        self.bar.step(HP_TILES, 1);
        self.draw(f);
        self.active = false;
        self.value = hp;
    }

    /// Starts draining (or filling) from `from` to `to` of `max` (`SetBattleBarStruct`); each
    /// `update` then moves it one Gen3 step.
    pub fn start(&mut self, max: u16, from: u16, to: u16) {
        self.bar = BarState::new(max as i32, from as i32, from as i32 - to as i32);
        self.active = true;
        self.value = from;
    }

    /// One `MoveBattleBar(HEALTH_BAR)` call while animating.
    pub fn update(&mut self, f: &mut Frame) {
        if !self.active {
            return;
        }
        let ret = self.bar.step(HP_TILES, 1);
        self.draw(f);
        if ret == -1 {
            self.active = false;
            self.bar.cur = 0; // as Gen3 `MoveBattleBar`
            self.value = self.bar.target() as u16;
        } else {
            self.value = ret as u16;
        }
    }

    pub fn done(&self) -> bool {
        !self.active
    }

    /// The HP the numbers show this frame (the bar's current value, as Gen3).
    pub fn value(&self) -> u16 {
        self.value
    }

    /// Filled pixels and color of what is drawn.
    pub fn drawn(&self) -> Option<(BarColor, u8)> {
        self.drawn.map(|(c, px)| {
            let color = [BarColor::Green, BarColor::Yellow, BarColor::Red][c as usize];
            (color, px.iter().sum())
        })
    }

    fn draw(&mut self, f: &mut Frame) {
        let mut px = [0u8; HP_TILES];
        let filled = self.bar.pixels(&mut px);
        let color = bar_color(filled) as u8;
        let base = match (self.side.is_player(), self.side.is_right()) {
            (true, false) => obj::PLAYER_BAR,
            (false, false) => obj::ENEMY_BAR,
            (true, true) => obj::PLAYER2_BAR,
            (false, true) => obj::ENEMY2_BAR,
        } + HP_LABEL_TILES;
        for (i, &p) in px.iter().enumerate() {
            let same = matches!(self.drawn, Some((c, old)) if c == color && old[i] == p);
            if !same {
                f.cmds.push(Cmd::ObjTile {
                    index: base + i as u16,
                    tile: tile_or_blank(
                        self.gfx.hpbar.tiles,
                        HP_COLOR_BASE[color as usize] + p as usize,
                    ),
                });
            }
        }
        self.drawn = Some((color, px));
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[side_tag(self.side), self.active as u8]);
        self.bar.hash(h);
        h.write_u16(self.value);
        match self.drawn {
            None => h.write(&[0xFF]),
            Some((c, px)) => {
                h.write(&[c]);
                h.write(&px);
            }
        }
    }
}

/// The level of a gain the bar is filling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Segment {
    /// Exp at the bar's level and at the next one.
    min: u32,
    next: u32,
    /// The gain: `to` is its overall end, possibly past `next`.
    from: u32,
    to: u32,
}

/// The player's 64 px exp bar, drawn into the player healthbox tiles.
///
/// One `start` fills one level, as Gen3 `Task_PrepareToGiveExpWithExpBar` /
/// `Task_GiveExpWithExpBar`: 13 waiting frames, then one `MoveBattleBar(EXP_BAR)` per frame.
/// At a level-up the bar wraps to empty and stops ([`ExpBar::leveled_up`]); the scene shows
/// the level-up, then calls `start` again with [`ExpBar::remaining`].
#[derive(Clone)]
pub struct ExpBar {
    gfx: &'static BattleGfx,
    bar: BarState,
    level: u8,
    running: Option<Segment>,
    leveled_up: bool,
    /// What is left of the gain after a level-up: (from, to).
    remaining: Option<(u32, u32)>,
    delay: u8,
    drawn: Option<[u8; EXP_TILES]>,
}

impl ExpBar {
    pub fn new(gfx: &'static BattleGfx) -> Self {
        ExpBar {
            gfx,
            bar: BarState::new(0, 0, 0),
            level: 0,
            running: None,
            leveled_up: false,
            remaining: None,
            delay: 0,
            drawn: None,
        }
    }

    /// Shows `exp` at `level` at once (`UpdateHealthboxAttribute(HEALTHBOX_EXP_BAR)`): `min`
    /// and `next` are the exp of `level` and `level + 1`. Empty at `MAX_LEVEL`.
    pub fn set(&mut self, level: u8, min: u32, next: u32, exp: u32, f: &mut Frame) {
        self.level = level;
        self.running = None;
        self.leveled_up = false;
        self.remaining = None;
        if level >= MAX_LEVEL || next <= min {
            self.bar = BarState::new(0, 0, 0);
        } else {
            self.bar = BarState::new((next - min) as i32, exp.saturating_sub(min) as i32, 0);
            self.bar.step(EXP_TILES, 1);
        }
        self.draw(f);
    }

    /// Starts filling the bar of `level` from `from` toward `to` total exp: the whole
    /// remaining gain, as Gen3 passes it all as `receivedValue` and `CalcNewBarValue` stops at
    /// the level's end (a gain crossing a level fills faster). `exp_at(l)` is the exp of level
    /// `l` (`mon::exp_for_level` for the species' growth rate). Nothing to do at `MAX_LEVEL`.
    pub fn start(&mut self, level: u8, from: u32, to: u32, exp_at: impl Fn(u8) -> u32) {
        self.level = level;
        self.leveled_up = false;
        self.remaining = None;
        self.running = None;
        if level >= MAX_LEVEL || from >= to {
            return;
        }
        let seg = Segment {
            min: exp_at(level),
            next: exp_at(level + 1),
            from,
            to,
        };
        // `SetBattleBarStruct(battler, healthbox, expToNextLvl, exp, -gainedExp)`.
        self.bar = BarState::new(
            seg.next.saturating_sub(seg.min) as i32,
            seg.from.saturating_sub(seg.min) as i32,
            -(seg.to.saturating_sub(seg.from).min(i32::MAX as u32) as i32),
        );
        self.delay = 0;
        self.running = Some(seg);
    }

    /// One frame of `Task_GiveExpWithExpBar`.
    pub fn update(&mut self, f: &mut Frame) {
        let Some(seg) = self.running else {
            return;
        };
        if self.delay < EXP_DELAY {
            self.delay += 1;
            return;
        }
        // `MoveBattleBar(EXP_BAR)`: the step is the exp per pixel of the whole gain.
        let frac = scaled_exp_fraction(self.bar.old, self.bar.received, self.bar.max).max(1);
        let to_add = (self.bar.received / frac).abs();
        let ret = self.bar.step(EXP_TILES, to_add);
        self.draw(f);
        if ret != -1 {
            return;
        }
        self.running = None;
        if seg.to >= seg.next {
            // Level-up: the healthbox redraw shows the new level's empty bar.
            self.level += 1;
            self.leveled_up = true;
            self.remaining = (seg.to > seg.next).then_some((seg.next, seg.to));
            self.bar = BarState::new(0, 0, 0);
            self.draw(f);
        }
    }

    /// The fill of the current `start` is over (the bar is stopped).
    pub fn done(&self) -> bool {
        self.running.is_none()
    }

    /// The last fill ended with a level-up: the bar shows the new level, empty.
    pub fn leveled_up(&self) -> bool {
        self.leveled_up
    }

    /// After a level-up, the rest of the gain for the next `start`: (from, to).
    pub fn remaining(&self) -> Option<(u32, u32)> {
        self.remaining
    }

    /// The level the bar shows.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// Filled pixels drawn.
    pub fn drawn_pixels(&self) -> Option<u8> {
        self.drawn.map(|px| px.iter().sum())
    }

    fn draw(&mut self, f: &mut Frame) {
        let mut px = [0u8; EXP_TILES];
        if self.level < MAX_LEVEL {
            self.bar.pixels(&mut px);
        }
        for (i, &p) in px.iter().enumerate() {
            if self.drawn.is_some_and(|d| d[i] == p) {
                continue;
            }
            f.cmds.push(Cmd::ObjTile {
                index: obj::PLAYER_BOX + EXP_TILE[i],
                tile: tile_or_blank(self.gfx.expbar.tiles, p as usize),
            });
        }
        self.drawn = Some(px);
    }

    /// Forgets what VRAM holds (the healthbox was reloaded): the next draw writes every tile.
    pub fn invalidate(&mut self) {
        self.drawn = None;
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        self.bar.hash(h);
        h.write(&[self.level, self.delay, self.leveled_up as u8]);
        match self.running {
            None => h.write(&[0xFF]),
            Some(s) => {
                for v in [s.min, s.next, s.from, s.to] {
                    h.write_u32(v);
                }
            }
        }
        match self.remaining {
            None => h.write(&[0xFF]),
            Some((a, b)) => {
                h.write_u32(a);
                h.write_u32(b);
            }
        }
        match self.drawn {
            None => h.write(&[0xFF]),
            Some(px) => h.write(&px),
        }
    }
}

/// Gen3 `GetScaledExpFraction(old, received, max, 8)`: how many pixels the gain covers.
fn scaled_exp_fraction(old: i32, received: i32, max: i32) -> i32 {
    if max <= 0 {
        return 0;
    }
    let scale = EXP_TILES as i32 * 8;
    let new = (old - received).clamp(0, max);
    let old_px = (old * scale / max) as i8;
    let new_px = (new * scale / max) as i8;
    (old_px as i32 - new_px as i32).abs()
}

#[cfg(test)]
mod tests;
