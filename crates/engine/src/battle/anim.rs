//! Gen3 move animations (`gBattleAnims_Moves`, `battle_anim.c`).
//!
//! A script is the ROM command list, decoded by the extract (not ARM). [`Player`] runs it:
//! Tackle's horizontal lunge, hit splat and shake, plus any other sprite whose sheet was
//! loaded. `GameData::move_anim` returns `None` when the game has no script, and the scene
//! keeps the old lunge. Sound ids are queued; playback is the audio gap. `setalpha` is
//! recorded for timing only (the PPU has no per-sprite blend).

use super::gfx::{self, MonSprite};
use super::{GfxImage, Side};
use crate::hash::Fnv64;
use crate::video::{Cmd as VideoCmd, Frame, Obj, ObjShape};
use alloc::vec::Vec;

/// One decoded battle-anim command.
pub type Script = [Cmd];

/// Commands the player runs. Addresses stay in the extract; this is the behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    LoadGfx(u16),
    UnloadGfx(u16),
    MonBg(u8),
    ClearMonBg(u8),
    SetAlpha(u8, u8),
    BlendOff,
    Delay(u8),
    Wait,
    End,
    PlaySe {
        se: u16,
        pan: u8,
    },
    /// `DoHorizontalLunge`: `frames` steps of `delta` px, then the same back.
    Lunge {
        frames: u8,
        delta: i16,
    },
    /// `AnimHitSplatBasic`. `anim` picks the affine (`0x100`, `0xD8`, `0xB0`, `0x80`).
    HitSplat {
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
        anim: u8,
    },
    /// `AnimTask_ShakeMon`.
    Shake {
        who: u8,
        x: i16,
        y: i16,
        times: u8,
        delay: u8,
    },
    /// A sheet sprite the player does not have a special callback for.
    Show {
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
    },
}

/// `Move_TACKLE`: the French/US script (load impact, mon bg, alpha 12/8, lunge 4×4,
/// delay 6, hit splat anim 2, shake the target by 3, `SE_M_COMET_PUNCH` at pan 63).
pub fn tackle() -> &'static Script {
    &TACKLE
}

const TACKLE: [Cmd; 12] = [
    Cmd::LoadGfx(10135),
    Cmd::MonBg(1),
    Cmd::SetAlpha(12, 8),
    Cmd::Lunge {
        frames: 4,
        delta: 4,
    },
    Cmd::Delay(6),
    Cmd::HitSplat {
        tag: 10135,
        x: 0,
        y: 0,
        relative: 1,
        anim: 2,
    },
    Cmd::Shake {
        who: 1,
        x: 3,
        y: 0,
        times: 6,
        delay: 1,
    },
    Cmd::PlaySe { se: 139, pan: 63 },
    Cmd::Wait,
    Cmd::ClearMonBg(1),
    Cmd::BlendOff,
    Cmd::End,
];

const SPLAT_SCALE: [u16; 4] = [0x100, 0xD8, 0xB0, 0x80];
const VRAM_END: u16 = 512;
const PAL_FIRST: u8 = 12;

struct Sheet {
    tag: u16,
    first: u16,
    count: u16,
    pal: u8,
    live: bool,
}

struct Pending {
    tiles: &'static [crate::video::Tile],
    pal: &'static [u16; 16],
    sent: usize,
    first: u16,
    pal_bank: u8,
    pal_sent: bool,
    stalls: u8,
}

struct Particle {
    x: i16,
    y: i16,
    tile: u16,
    pal: u8,
    size: u8,
    scale: u16,
}

enum Actor {
    Lunge {
        who: usize,
        x: i16,
        left: i16,
        dx: i16,
        back: bool,
        span: i16,
    },
    Splat {
        p: Particle,
        life: u8,
        fresh: bool,
    },
    Shake {
        who: usize,
        x: i16,
        y: i16,
        times: u8,
        delay: u8,
        timer: u8,
        on: bool,
        fresh: bool,
    },
    Show {
        p: Particle,
        life: u8,
        fresh: bool,
    },
}

/// One playing script.
pub struct Player {
    script: &'static Script,
    pc: usize,
    wait: u8,
    ended: bool,
    done: bool,
    attacker: Side,
    target: Side,
    actors: Vec<Actor>,
    sheets: Vec<Sheet>,
    pending: Option<Pending>,
    sounds: Vec<(u16, u8)>,
    /// `setalpha` / `blendoff`, kept so the commands are not dropped.
    alpha: Option<(u8, u8)>,
}

impl Player {
    pub fn start(script: &'static Script, attacker: Side, target: Side) -> Self {
        Player {
            script,
            pc: 0,
            wait: 0,
            ended: false,
            done: script.is_empty(),
            attacker,
            target,
            actors: Vec::new(),
            sheets: Vec::new(),
            pending: None,
            sounds: Vec::new(),
            alpha: None,
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Sound effects the script queued (`SE_*`, pan). Not played here.
    pub fn sounds(&self) -> &[(u16, u8)] {
        &self.sounds
    }

    pub fn hash(&self, h: &mut Fnv64) {
        h.write(&[0xA5, self.pc as u8, self.wait, self.actors.len() as u8]);
    }

    /// One frame. `gfx` is `GameData::anim_gfx`. Offsets land on `mons` before the scene draws them.
    pub fn step(
        &mut self,
        mons: &mut [MonSprite],
        gfx: impl Fn(u16) -> Option<&'static GfxImage>,
        f: &mut Frame,
    ) {
        if self.done {
            return;
        }
        if self.pump(f) {
            self.tick(mons);
            self.draw(f);
            self.finish(mons);
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
        } else if !self.ended {
            self.run(mons, &gfx, f);
        }
        self.tick(mons);
        self.draw(f);
        self.finish(mons);
    }

    fn finish(&mut self, mons: &mut [MonSprite]) {
        if self.ended && self.actors.is_empty() && self.pending.is_none() && self.wait == 0 {
            for s in [self.attacker, self.target] {
                if s.i() < mons.len() {
                    mons[s.i()].set_anim_offset((0, 0));
                }
            }
            self.done = true;
        }
    }

    fn run(
        &mut self,
        mons: &mut [MonSprite],
        gfx: &impl Fn(u16) -> Option<&'static GfxImage>,
        f: &mut Frame,
    ) {
        let mut fuel = 64;
        while fuel > 0 && self.pc < self.script.len() && self.wait == 0 && !self.ended {
            fuel -= 1;
            let cmd = self.script[self.pc];
            self.pc += 1;
            match cmd {
                Cmd::LoadGfx(tag) => {
                    self.load_gfx(tag, gfx, f);
                    if self.pending.is_some() {
                        break;
                    }
                    // Gen3's loadspritegfx waits one frame even when the copy is done.
                    self.wait = 1;
                    break;
                }
                Cmd::UnloadGfx(tag) => {
                    if let Some(s) = self.sheets.iter_mut().find(|s| s.live && s.tag == tag) {
                        s.live = false;
                    }
                }
                Cmd::MonBg(_) | Cmd::ClearMonBg(_) => {}
                Cmd::SetAlpha(a, b) => self.alpha = Some((a, b)),
                Cmd::BlendOff => self.alpha = None,
                Cmd::Delay(n) => {
                    self.wait = n.max(1);
                    break;
                }
                Cmd::Wait => {
                    if !self.actors.is_empty() || self.pending.is_some() {
                        self.pc -= 1;
                        self.wait = 1;
                        break;
                    }
                }
                Cmd::End => {
                    self.ended = true;
                    break;
                }
                Cmd::PlaySe { se, pan } => self.sounds.push((se, pan)),
                Cmd::Lunge { frames, delta } => self.spawn_lunge(frames, delta),
                Cmd::HitSplat {
                    tag,
                    x,
                    y,
                    relative,
                    anim,
                } => self.spawn_splat(mons, tag, x, y, relative, anim),
                Cmd::Shake {
                    who,
                    x,
                    y,
                    times,
                    delay,
                } => self.spawn_shake(mons, who, x, y, times, delay),
                Cmd::Show {
                    tag,
                    x,
                    y,
                    relative,
                } => self.spawn_show(mons, tag, x, y, relative),
            }
        }
    }

    fn load_gfx(
        &mut self,
        tag: u16,
        gfx: &impl Fn(u16) -> Option<&'static GfxImage>,
        f: &mut Frame,
    ) {
        let Some(img) = gfx(tag) else {
            return;
        };
        if img.tiles.is_empty() || img.palettes.is_empty() {
            return;
        }
        let count = img.tiles.len().min(64) as u16;
        let Some((first, pal)) = self.alloc(tag, count) else {
            return;
        };
        self.pending = Some(Pending {
            tiles: img.tiles,
            pal: &img.palettes[0],
            sent: 0,
            first,
            pal_bank: pal,
            pal_sent: false,
            stalls: 0,
        });
        self.pump(f);
    }

    /// Uploads the next budget-sized chunk. `true` while tiles remain.
    fn pump(&mut self, f: &mut Frame) -> bool {
        let Some(p) = self.pending.as_mut() else {
            return false;
        };
        let left = gfx::obj_tiles_left(f);
        if left == 0 {
            p.stalls += 1;
            if p.stalls >= 4 {
                self.pending = None;
                return false;
            }
            return true;
        }
        if !p.pal_sent {
            f.cmds.push(VideoCmd::ObjPalette {
                bank: p.pal_bank,
                colors: *p.pal,
            });
            p.pal_sent = true;
        }
        let end = (p.sent + left).min(p.tiles.len());
        if end > p.sent {
            f.cmds.push(VideoCmd::ObjTiles {
                first: p.first + p.sent as u16,
                tiles: &p.tiles[p.sent..end],
            });
            p.sent = end;
        }
        if p.sent >= p.tiles.len() {
            self.pending = None;
            false
        } else {
            true
        }
    }

    fn alloc(&mut self, tag: u16, count: u16) -> Option<(u16, u8)> {
        if let Some(s) = self.sheets.iter_mut().find(|s| !s.live && s.count >= count) {
            s.live = true;
            s.tag = tag;
            return Some((s.first, s.pal));
        }
        let first = self
            .sheets
            .last()
            .map(|s| s.first + s.count)
            .unwrap_or(gfx::obj::FREE);
        if u32::from(first) + u32::from(count) > u32::from(VRAM_END) {
            return None;
        }
        let pal = PAL_FIRST + self.sheets.len() as u8;
        if pal > 15 {
            return None;
        }
        self.sheets.push(Sheet {
            tag,
            first,
            count,
            pal,
            live: true,
        });
        Some((first, pal))
    }

    fn sheet(&self, tag: u16) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.live && s.tag == tag)
    }

    fn who(&self, battler: u8) -> usize {
        if battler == 0 {
            self.attacker.i()
        } else {
            self.target.i()
        }
    }

    fn spawn_lunge(&mut self, frames: u8, delta: i16) {
        let frames = i16::from(frames.max(1));
        let dx = if self.attacker.is_player() {
            delta
        } else {
            -delta
        };
        self.actors.push(Actor::Lunge {
            who: self.attacker.i(),
            x: 0,
            left: frames,
            dx,
            back: false,
            span: frames,
        });
    }

    fn spawn_shake(
        &mut self,
        mons: &mut [MonSprite],
        battler: u8,
        x: i16,
        y: i16,
        times: u8,
        delay: u8,
    ) {
        let who = self.who(battler);
        if who >= mons.len() || times == 0 {
            return;
        }
        // The task sets the offset, then its first call only decrements the delay.
        mons[who].set_anim_offset((x, y));
        self.actors.push(Actor::Shake {
            who,
            x,
            y,
            times,
            delay: delay.max(1),
            timer: delay.max(1).saturating_sub(1),
            on: true,
            fresh: true,
        });
    }

    fn spawn_splat(
        &mut self,
        mons: &[MonSprite],
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
        anim: u8,
    ) {
        let scale = SPLAT_SCALE[usize::from(anim) & 3];
        self.spawn_particle(mons, tag, x, y, relative, scale, true);
    }

    fn spawn_show(&mut self, mons: &[MonSprite], tag: u16, x: i16, y: i16, relative: u8) {
        self.spawn_particle(mons, tag, x, y, relative, 0, false);
    }

    fn spawn_particle(
        &mut self,
        mons: &[MonSprite],
        tag: u16,
        x: i16,
        y: i16,
        relative: u8,
        scale: u16,
        splat: bool,
    ) {
        let Some(sheet) = self.sheet(tag) else {
            return;
        };
        let who = self.who(relative);
        if who >= mons.len() {
            return;
        }
        let (size, half) = match sheet.count {
            0..=1 => (0, 4),
            2..=4 => (1, 8),
            5..=16 => (2, 16),
            _ => (3, 32),
        };
        let (cx, cy) = mons[who].center();
        let p = Particle {
            x: cx + x - half,
            y: cy + y - half,
            tile: sheet.first,
            pal: sheet.pal,
            size,
            scale,
        };
        if splat {
            self.actors.push(Actor::Splat {
                p,
                life: 8,
                fresh: true,
            });
        } else {
            self.actors.push(Actor::Show {
                p,
                life: 8,
                fresh: true,
            });
        }
    }

    fn tick(&mut self, mons: &mut [MonSprite]) {
        let mut i = 0;
        while i < self.actors.len() {
            let dead = match &mut self.actors[i] {
                Actor::Lunge {
                    who,
                    x,
                    left,
                    dx,
                    back,
                    span,
                } => {
                    let who = *who;
                    if !*back {
                        if *left > 0 {
                            *left -= 1;
                            *x += *dx;
                        } else {
                            *back = true;
                            *left = *span;
                            *dx = -*dx;
                        }
                        if who < mons.len() {
                            let y = mons[who].offset().1;
                            mons[who].set_anim_offset((*x, y));
                        }
                        false
                    } else if *left > 0 {
                        *left -= 1;
                        *x += *dx;
                        if who < mons.len() {
                            let y = mons[who].offset().1;
                            mons[who].set_anim_offset((*x, y));
                        }
                        false
                    } else {
                        if who < mons.len() {
                            mons[who].set_anim_offset((0, 0));
                        }
                        true
                    }
                }
                Actor::Shake {
                    who,
                    x,
                    y,
                    times,
                    delay,
                    timer,
                    on,
                    fresh,
                } => {
                    if *fresh {
                        *fresh = false;
                        false
                    } else if *timer == 0 {
                        *on = !*on;
                        let who = *who;
                        if who < mons.len() {
                            let off = if *on { (*x, *y) } else { (0, 0) };
                            mons[who].set_anim_offset(off);
                        }
                        *timer = *delay;
                        *times = times.saturating_sub(1);
                        if *times == 0 {
                            if who < mons.len() {
                                mons[who].set_anim_offset((0, 0));
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        *timer -= 1;
                        false
                    }
                }
                Actor::Splat { life, fresh, .. } | Actor::Show { life, fresh, .. } => {
                    if *fresh {
                        *fresh = false;
                        false
                    } else if *life <= 1 {
                        true
                    } else {
                        *life -= 1;
                        false
                    }
                }
            };
            if dead {
                self.actors.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
        for a in self.actors.iter().rev() {
            let p = match a {
                Actor::Splat { p, .. } | Actor::Show { p, .. } => p,
                _ => continue,
            };
            f.objs.insert(
                0,
                Obj {
                    x: p.x,
                    y: p.y,
                    tile: p.tile,
                    palette: p.pal,
                    shape: ObjShape::Square,
                    size: p.size,
                    priority: 1,
                    scale: p.scale,
                    ..Obj::default()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests;
