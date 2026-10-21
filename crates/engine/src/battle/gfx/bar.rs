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
use alloc::vec::Vec;

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
        let base = match self.side {
            Side::Player => obj::PLAYER_BAR,
            Side::Opponent => obj::ENEMY_BAR,
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

/// One level's worth of an exp gain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Segment {
    level: u8,
    /// Exp at `level` and at `level + 1`.
    min: u32,
    next: u32,
    from: u32,
    to: u32,
}

/// The player's 64 px exp bar, drawn into the player healthbox tiles.
#[derive(Clone)]
pub struct ExpBar {
    gfx: &'static BattleGfx,
    bar: BarState,
    level: u8,
    /// Remaining segments of the gain, the running one first.
    segments: Vec<Segment>,
    delay: u8,
    drawn: Option<[u8; EXP_TILES]>,
}

impl ExpBar {
    pub fn new(gfx: &'static BattleGfx) -> Self {
        ExpBar {
            gfx,
            bar: BarState::new(0, 0, 0),
            level: 0,
            segments: Vec::new(),
            delay: 0,
            drawn: None,
        }
    }

    /// Shows `exp` at `level` at once (`UpdateHealthboxAttribute(HEALTHBOX_EXP_BAR)`): `min`
    /// and `next` are the exp of `level` and `level + 1`. Empty at `MAX_LEVEL`.
    pub fn set(&mut self, level: u8, min: u32, next: u32, exp: u32, f: &mut Frame) {
        self.level = level;
        self.segments.clear();
        if level >= MAX_LEVEL || next <= min {
            self.bar = BarState::new(0, 0, 0);
        } else {
            self.bar = BarState::new((next - min) as i32, exp.saturating_sub(min) as i32, 0);
            self.bar.step(EXP_TILES, 1);
        }
        self.draw(f);
    }

    /// Starts a gain from `from` to `to` total exp at `level`; `exp_at(l)` is the exp of level
    /// `l` (`mon::exp_for_level` for the species' growth rate). The bar fills one level at a
    /// time and wraps to empty at each level-up ([`ExpBar::level`] follows).
    pub fn start(&mut self, level: u8, from: u32, to: u32, exp_at: impl Fn(u8) -> u32) {
        self.segments.clear();
        self.level = level;
        let (mut lv, mut e) = (level, from);
        while lv < MAX_LEVEL && e < to {
            let (min, next) = (exp_at(lv), exp_at(lv + 1));
            let seg_to = to.min(next);
            self.segments.push(Segment {
                level: lv,
                min,
                next,
                from: e,
                to: seg_to,
            });
            if seg_to < next {
                break;
            }
            lv += 1;
            e = next;
        }
        self.begin_segment();
    }

    fn begin_segment(&mut self) {
        if let Some(s) = self.segments.first() {
            self.bar = BarState::new(
                s.next.saturating_sub(s.min) as i32,
                s.from.saturating_sub(s.min) as i32,
                -(s.to.saturating_sub(s.from) as i32),
            );
            self.delay = 0;
        }
    }

    /// One frame of `Task_GiveExpWithExpBar`.
    pub fn update(&mut self, f: &mut Frame) {
        let Some(&seg) = self.segments.first() else {
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
        self.segments.remove(0);
        if seg.to == seg.next {
            // Level-up: the healthbox redraw shows the new level's empty bar.
            self.level = seg.level + 1;
            self.bar = BarState::new(0, 0, 0);
            self.draw(f);
        }
        self.begin_segment();
    }

    pub fn done(&self) -> bool {
        self.segments.is_empty()
    }

    /// The level the bar currently shows (it wraps at each level-up of a gain).
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
        h.write(&[self.level, self.delay, self.segments.len() as u8]);
        for s in &self.segments {
            h.write(&[s.level]);
            for v in [s.min, s.next, s.from, s.to] {
                h.write_u32(v);
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
mod tests {
    use super::super::test_gfx::*;
    use super::*;

    fn obj_tiles(f: &Frame) -> Vec<(u16, Tile)> {
        f.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::ObjTile { index, tile } => Some((*index, *tile)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn colors_follow_gen3_thresholds() {
        // 48 * 50 / 100 = 24, 48 * 20 / 100 = 9.
        assert_eq!(bar_color(48), BarColor::Green);
        assert_eq!(bar_color(25), BarColor::Green);
        assert_eq!(bar_color(24), BarColor::Yellow);
        assert_eq!(bar_color(10), BarColor::Yellow);
        assert_eq!(bar_color(9), BarColor::Red);
        assert_eq!(bar_color(0), BarColor::Red);
    }

    /// Golden from a throwaway transcription of `CalcNewBarValue` + `CalcBarFilledPixels`
    /// (max 20 < 48: 8.8 path, step (20 << 8) / 48 = 106, rounding the value up).
    #[test]
    fn hp_bar_drains_20_to_6_of_20_frame_by_frame() {
        const VALUES: [u16; 34] = [
            20, 20, 19, 19, 18, 18, 18, 17, 17, 16, 16, 16, 15, 15, 14, 14, 13, 13, 13, 12, 12, 11,
            11, 11, 10, 10, 9, 9, 8, 8, 8, 7, 7, 6,
        ];
        let gfx = gfx();
        let mut bar = HpBar::new(Side::Player, gfx);
        bar.set(20, 20, &mut Frame::default());
        bar.start(20, 20, 6);
        let mut values = Vec::new();
        let mut pixels = Vec::new();
        let mut colors = Vec::new();
        let mut frames = 0;
        while !bar.done() {
            let mut f = Frame::default();
            bar.update(&mut f);
            assert!(obj_tiles(&f).len() <= HP_TILES);
            frames += 1;
            values.push(bar.value());
            let (c, p) = bar.drawn().unwrap();
            pixels.push(p);
            colors.push(c);
        }
        // 34 moving frames, then the frame `CalcNewBarValue` returns -1.
        assert_eq!(frames, 35);
        assert_eq!(values[..34], VALUES);
        assert_eq!(values[34], 6);
        let expected_px: Vec<u8> = (14..=47).rev().chain([14]).collect();
        assert_eq!(pixels, expected_px);
        assert!(colors[..23].iter().all(|&c| c == BarColor::Green));
        assert!(colors[23..].iter().all(|&c| c == BarColor::Yellow));
    }

    #[test]
    fn hp_bar_uploads_only_changed_tiles_and_recolors() {
        let gfx = gfx();
        let mut bar = HpBar::new(Side::Opponent, gfx);
        let mut f = Frame::default();
        bar.set(20, 20, &mut f);
        // First draw: all 6 green full tiles (index 3 + 8).
        let t = obj_tiles(&f);
        assert_eq!(t.len(), 6);
        assert_eq!(t[0], (obj::ENEMY_BAR + 2, tile(TAG_HPBAR, 11)));
        // Full -> 47 px: only the last tile changes.
        bar.start(20, 20, 6);
        let mut f = Frame::default();
        bar.update(&mut f);
        assert_eq!(
            obj_tiles(&f),
            [(obj::ENEMY_BAR + 7, tile(TAG_HPBAR, 3 + 7))]
        );
        // Instant set to 4 of 20: 9 px, red: every tile is redrawn in red.
        let mut f = Frame::default();
        bar.set(4, 20, &mut f);
        let t = obj_tiles(&f);
        assert_eq!(t.len(), 6);
        assert_eq!(t[0].1, tile(TAG_HPBAR, 21 + 8));
        assert_eq!(t[1].1, tile(TAG_HPBAR, 21 + 1));
        assert_eq!(bar.drawn(), Some((BarColor::Red, 9)));
    }

    #[test]
    fn hp_bar_direct_path_and_minimum_pixel() {
        // max 100 >= 48: one HP per frame, 100 -> 30 in 70 moves + the final frame.
        let mut bar = HpBar::new(Side::Player, gfx());
        bar.start(100, 100, 30);
        let mut frames = 0;
        while !bar.done() {
            bar.update(&mut Frame::default());
            frames += 1;
        }
        assert_eq!(frames, 71);
        assert_eq!(bar.value(), 30);
        // 1 of 200: 48 / 200 = 0 px, but a living mon shows 1.
        bar.set(1, 200, &mut Frame::default());
        assert_eq!(bar.drawn(), Some((BarColor::Red, 1)));
        bar.set(0, 200, &mut Frame::default());
        assert_eq!(bar.drawn(), Some((BarColor::Red, 0)));
    }

    /// Golden from the same transcription: span 100, 20 -> 100 (+80). `GetScaledExpFraction`
    /// = |12 - 64| = 52 px, so the step is 80 / 52 = 1 exp per frame.
    #[test]
    fn exp_bar_fills_one_level_then_wraps() {
        let gfx = gfx();
        let mut bar = ExpBar::new(gfx);
        let at = |l: u8| l as u32 * 100;
        let mut f = Frame::default();
        bar.set(5, at(5), at(6), 520, &mut f);
        assert_eq!(bar.drawn_pixels(), Some(12)); // 20 * 64 / 100
        assert_eq!(
            obj_tiles(&f),
            [
                (obj::PLAYER_BOX + 0x24, tile(TAG_EXPBAR, 8)),
                (obj::PLAYER_BOX + 0x25, tile(TAG_EXPBAR, 4)),
                (obj::PLAYER_BOX + 0x26, tile(TAG_EXPBAR, 0)),
                (obj::PLAYER_BOX + 0x27, tile(TAG_EXPBAR, 0)),
                (obj::PLAYER_BOX + 0x60, tile(TAG_EXPBAR, 0)),
                (obj::PLAYER_BOX + 0x61, tile(TAG_EXPBAR, 0)),
                (obj::PLAYER_BOX + 0x62, tile(TAG_EXPBAR, 0)),
                (obj::PLAYER_BOX + 0x63, tile(TAG_EXPBAR, 0)),
            ]
        );
        bar.start(5, 520, 650, at);
        let mut pixels = Vec::new();
        let mut frames = 0;
        while bar.level() == 5 {
            let mut f = Frame::default();
            bar.update(&mut f);
            assert!(obj_tiles(&f).len() <= 16);
            frames += 1;
            if frames > EXP_DELAY as usize && bar.level() == 5 {
                pixels.push(bar.drawn_pixels().unwrap());
            }
        }
        // 13 waiting frames, 80 moves, then the -1 frame wraps.
        assert_eq!(frames, 13 + 81);
        assert_eq!(pixels.len(), 80);
        assert_eq!(pixels[..6], [13, 14, 14, 15, 16, 16]);
        assert_eq!(pixels[79], 64);
        assert_eq!(bar.drawn_pixels(), Some(0));
        assert!(!bar.done());
        // Level 6: 600 -> 650 of a 100 span, after another 13-frame wait.
        let mut frames = 0;
        while !bar.done() {
            bar.update(&mut Frame::default());
            frames += 1;
        }
        assert_eq!(frames, 13 + 50 + 1);
        assert_eq!(bar.level(), 6);
        assert_eq!(bar.drawn_pixels(), Some(32));
    }

    /// Span 40 < 64: the 8.8 path, (40 << 8) / 64 = 160 per frame (0.625 exp).
    #[test]
    fn exp_bar_small_span_and_exact_level_up() {
        let mut bar = ExpBar::new(gfx());
        let at = |l: u8| 1000 + (l as u32 - 5) * 40;
        bar.start(5, 1010, 1040, at);
        let mut pixels = Vec::new();
        while bar.level() == 5 {
            bar.update(&mut Frame::default());
            if let (5, Some(p)) = (bar.level(), bar.drawn_pixels()) {
                pixels.push(p);
            }
        }
        // 48 moves; the 49th frame (-1) wraps.
        let expected: Vec<u8> = (17..=64).collect();
        assert_eq!(pixels, expected);
        // Exactly the next level's exp: the bar wraps to empty and the gain is over.
        assert!(bar.done());
        assert_eq!(bar.level(), 6);
        assert_eq!(bar.drawn_pixels(), Some(0));
    }

    #[test]
    fn exp_bar_is_empty_at_max_level_and_hash_changes() {
        let mut bar = ExpBar::new(gfx());
        bar.set(100, 1000, 1000, 1000, &mut Frame::default());
        assert_eq!(bar.drawn_pixels(), Some(0));
        let h = |b: &ExpBar| {
            let mut h = Fnv64::new();
            b.hash(&mut h);
            h.finish()
        };
        let h0 = h(&bar);
        bar.start(99, 9900, 9910, |l| l as u32 * 100);
        assert_ne!(h(&bar), h0);
    }
}
