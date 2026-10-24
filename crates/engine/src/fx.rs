//! Screen effects scripts start (SIL2's Wave 0 requests; Milestone 9 Task R owns the file):
//! a full-screen flash (`ScreenFlash`), a camera shake (`ScreenShake`) and a palette tint
//! (`SetTint`).
//!
//! [`apply`] runs last in `World::step` and post-processes the frame's palette uploads and
//! scroll offsets, so an effect survives every field restore, map load and screen return
//! without an edit in `field.rs`, `world/screens.rs` or `world/battle.rs`. Wave 0 declares
//! the state and calls the no-op [`apply`]; the state is not snapshotted (a save or a capture
//! needs it clear) and is hashed only while an effect is active.

use crate::hash::Fnv64;
use crate::video::Frame;

/// A full-screen colour shown for `frames`, then back (Gen3 `BeginNormalPaletteFade` to a
/// colour and back).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flash {
    /// BGR555.
    pub color: u16,
    pub frames: u16,
    /// Frames shown so far.
    pub t: u16,
}

/// BG and OBJ offsets alternating ±`magnitude` every frame, ending at 0 (Gen3 `ShakeCamera`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shake {
    pub magnitude: u8,
    pub frames: u16,
    pub t: u16,
}

/// A blend of palette banks towards a colour (Gen3 `BlendPalettes`-style), stepped over
/// `frames`; kept until cleared with amount 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tint {
    /// BG palette banks, one bit each.
    pub banks: u16,
    /// OBJ palette banks, one bit each.
    pub obj_banks: u16,
    /// BGR555.
    pub color: u16,
    /// Target blend, 0..=16.
    pub amount: u8,
    /// The blend reached so far, 0..=16.
    pub current: u8,
    pub frames: u16,
    pub t: u16,
}

/// A camera pan (Silice `camera_pan`; Gen3 moves the camera object with `applymovement
/// OBJ_EVENT_ID_CAMERA`): the field BGs and the objects shift by `(x, y)` pixels, moving
/// `speed` pixels a frame towards the target `(tx, ty)`; a pan back to `(0, 0)` ends it.
/// Degraded: the view is shifted, not re-centred (no new cells are drawn past the camera's
/// margin).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pan {
    pub x: i16,
    pub y: i16,
    pub tx: i16,
    pub ty: i16,
    pub speed: u8,
}

impl Pan {
    pub fn done(&self) -> bool {
        (self.x, self.y) == (self.tx, self.ty)
    }
}

/// The active screen effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScreenFx {
    pub flash: Option<Flash>,
    pub shake: Option<Shake>,
    pub tint: Option<Tint>,
    /// Text on a black screen (Silice `show_text_on_black`): only the text window shows.
    pub text_on_black: bool,
    pub pan: Option<Pan>,
    /// The field BGs were switched off by [`ScreenFx::text_on_black`] and must be switched
    /// back on (a display detail, not hashed).
    layers_off: bool,
}

impl ScreenFx {
    /// Whether any effect is active.
    pub fn is_active(&self) -> bool {
        self.flash.is_some()
            || self.shake.is_some()
            || self.tint.is_some()
            || self.text_on_black
            || self.pan.is_some()
    }

    /// Feeds the effects into `h`, only while one is active.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        if !self.is_active() {
            return;
        }
        h.write(&[0xA5]);
        match self.flash {
            None => h.write(&[0]),
            Some(f) => {
                h.write(&[1]);
                for v in [f.color, f.frames, f.t] {
                    h.write_u16(v);
                }
            }
        }
        match self.shake {
            None => h.write(&[0]),
            Some(s) => {
                h.write(&[1, s.magnitude]);
                h.write_u16(s.frames);
                h.write_u16(s.t);
            }
        }
        match self.tint {
            None => h.write(&[0]),
            Some(t) => {
                h.write(&[1, t.amount, t.current]);
                for v in [t.banks, t.obj_banks, t.color, t.frames, t.t] {
                    h.write_u16(v);
                }
            }
        }
        // Silice's text on black and camera pan: bytes only when present.
        if self.text_on_black {
            h.write(&[0xA6]);
        }
        if let Some(p) = self.pan {
            h.write(&[0xA9, p.speed]);
            for v in [p.x, p.y, p.tx, p.ty] {
                h.write_u16(v as u16);
            }
        }
    }
}

/// Whether a BGR555 colour is closer to white than to black (a flash shows as the brightness
/// raised to white, else lowered to black: the frame has no full-screen colour).
fn light(color: u16) -> bool {
    let (r, g, b) = (color & 31, (color >> 5) & 31, (color >> 10) & 31);
    r + g + b >= 48
}

/// One channel of `c` blended `amount`/16 towards `to` (Gen3 `BlendPalette`).
fn blend(c: u16, to: u16, amount: u8) -> u16 {
    let mut out = 0;
    for shift in [0, 5, 10] {
        let a = ((c >> shift) & 31) as i32;
        let b = ((to >> shift) & 31) as i32;
        let v = a + (b - a) * i32::from(amount) / 16;
        out |= (v as u16 & 31) << shift;
    }
    out
}

/// Applies the effects to the frame about to be shown and advances them one frame.
///
/// - **Flash:** the whole frame at full brightness towards white (a light colour) or black for
///   `frames` frames (degraded: not the colour itself), then back.
/// - **Shake:** the field BGs (BG1–BG3) and the objects move vertically by +`magnitude`, then
///   −`magnitude`, alternating every frame (Gen3 `ShakeCamera`), and end at 0.
/// - **Tint:** `current` steps towards `amount` over `frames`; the palette uploads of the frame
///   are blended (degraded: palettes not re-uploaded this frame keep their colours); amount 0
///   reached ends it.
/// - **Text on black:** the field BGs and the objects are hidden, the text window stays.
/// - **Pan:** the field BGs and the objects shift by the pan's offset, moving towards its
///   target.
pub fn apply(fx: &mut ScreenFx, f: &mut Frame) {
    use crate::video::{BRIGHTNESS_BLACK, BgControl, Cmd, layout};
    let field_bgs = [
        layout::FIELD_TOP_BG,
        layout::FIELD_MIDDLE_BG,
        layout::FIELD_BOTTOM_BG,
    ];
    let mut dy: i16 = 0;
    let mut dx: i16 = 0;
    if let Some(p) = &mut fx.pan {
        let step = |v: &mut i16, t: i16, s: i16| {
            *v = if *v < t {
                (*v + s).min(t)
            } else {
                (*v - s).max(t)
            }
        };
        let s = i16::from(p.speed.max(1));
        step(&mut p.x, p.tx, s);
        step(&mut p.y, p.ty, s);
        dx += p.x;
        dy += p.y;
        if p.done() && (p.x, p.y) == (0, 0) {
            fx.pan = None;
        }
    }
    if let Some(s) = &mut fx.shake {
        s.t += 1;
        if s.t >= s.frames {
            fx.shake = None;
        } else {
            let m = i16::from(s.magnitude);
            dy += if s.t % 2 == 1 { m } else { -m };
        }
    }
    if (dx, dy) != (0, 0) {
        for bg in field_bgs {
            let (x, y) = &mut f.scroll[bg as usize];
            // Scrolling the BG by +d moves the picture by -d.
            *x = x.wrapping_add(dx as u16);
            *y = y.wrapping_add(dy as u16);
        }
        for o in &mut f.objs {
            o.x = o.x.wrapping_sub(dx);
            o.y = o.y.wrapping_sub(dy);
        }
    }
    if let Some(t) = &mut fx.tint {
        if t.current != t.amount {
            let remaining = t.frames.saturating_sub(t.t).max(1) as i16;
            let diff = i16::from(t.amount) - i16::from(t.current);
            let mut step = diff / remaining;
            if step == 0 {
                step = diff.signum();
            }
            t.current = (i16::from(t.current) + step) as u8;
        }
        t.t = t.t.saturating_add(1);
        let (banks, obj_banks, color, amount) = (t.banks, t.obj_banks, t.color, t.current);
        for c in &mut f.cmds {
            match c {
                Cmd::BgPalette { bank, colors } if banks & (1 << *bank) != 0 => colors
                    .iter_mut()
                    .for_each(|x| *x = blend(*x, color, amount)),
                Cmd::ObjPalette { bank, colors } if obj_banks & (1 << *bank) != 0 => colors
                    .iter_mut()
                    .for_each(|x| *x = blend(*x, color, amount)),
                _ => {}
            }
        }
        if t.current == 0 && t.amount == 0 {
            fx.tint = None;
        }
    }
    if fx.text_on_black {
        for bg in field_bgs {
            f.cmds.push(Cmd::Bg {
                layer: bg,
                control: BgControl {
                    enabled: false,
                    priority: layout::BG_PRIORITY[bg as usize],
                    char_block: layout::FIELD_CHAR_BLOCK,
                    screen_block: layout::SCREEN_BLOCKS[bg as usize],
                },
            });
        }
        f.objs.clear();
        f.brightness = 0;
        f.lighten = false;
        fx.layers_off = true;
    } else if fx.layers_off {
        for bg in field_bgs {
            f.cmds.push(Cmd::Bg {
                layer: bg,
                control: BgControl {
                    enabled: true,
                    priority: layout::BG_PRIORITY[bg as usize],
                    char_block: layout::FIELD_CHAR_BLOCK,
                    screen_block: layout::SCREEN_BLOCKS[bg as usize],
                },
            });
        }
        fx.layers_off = false;
    }
    if let Some(fl) = &mut fx.flash {
        f.brightness = BRIGHTNESS_BLACK;
        f.lighten = light(fl.color);
        fl.t += 1;
        if fl.t >= fl.frames {
            fx.flash = None;
        }
    }
}

#[cfg(test)]
mod tests;
