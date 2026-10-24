//! Screen effects and timed waits (Milestone 9 Task R owns the file): `ScreenFlash`,
//! `ScreenShake`, `SetTint` (`crate::fx`), `WaitInputOrTimeout`, the trainer battle's
//! `no_whiteout`, and Silice's text on black and camera pan.
//!
//! The flash, the shake and the pan wait until they end (the script goes on after them, as
//! Gen3's `waitstate` after `ShakeCamera`); a tint and the text on black are set and answered at
//! once. A map script (immediate) never waits: the effect is logged and skipped.

use super::{Host, UNIT};
use crate::Buttons;
use crate::data::GameData;
use crate::fx::{Flash, Pan, Shake, Tint};
use crate::script::{Request, Response};

/// Starts the effect; `None` when the script waits for it ([`resume`]).
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let s = &mut *h.s;
    let waits = matches!(
        r,
        Request::ScreenFlash { .. }
            | Request::ScreenShake { .. }
            | Request::CameraPan { .. }
            | Request::WaitInputOrTimeout(_)
            | Request::ShowOverlay { .. }
    );
    if waits && h.immediate {
        s.log("screen effect in a map script skipped");
        return Some(match r {
            Request::WaitInputOrTimeout(_) => Response::Bool(false),
            _ => Response::Unit,
        });
    }
    match *r {
        Request::ScreenFlash { color, frames } => {
            if frames == 0 {
                return UNIT;
            }
            s.fx.flash = Some(Flash {
                color,
                frames,
                t: 0,
            });
            None
        }
        Request::ScreenShake { magnitude, frames } => {
            if frames == 0 {
                return UNIT;
            }
            s.fx.shake = Some(Shake {
                magnitude,
                frames,
                t: 0,
            });
            None
        }
        Request::SetTint {
            banks,
            obj_banks,
            color,
            amount,
            frames,
        } => {
            let current = s.fx.tint.map_or(0, |t| t.current);
            s.fx.tint = (amount != 0 || current != 0).then_some(Tint {
                banks,
                obj_banks,
                color,
                amount: amount.min(16),
                current,
                frames,
                t: 0,
            });
            UNIT
        }
        Request::TextOnBlack(on) => {
            s.fx.text_on_black = on;
            UNIT
        }
        Request::CameraPan { x, y, speed } => {
            let (cx, cy) = s.fx.pan.map_or((0, 0), |p| (p.x, p.y));
            if (cx, cy, x, y) == (0, 0, 0, 0) {
                return UNIT;
            }
            s.fx.pan = Some(Pan {
                x: cx,
                y: cy,
                tx: x,
                ty: y,
                speed,
            });
            None
        }
        // Weather and palettes (`crate::weather`, `crate::sky`): set and answered at once.
        Request::SetWeather(w) => {
            s.weather.set_saved(w);
            UNIT
        }
        Request::DoWeather => {
            s.weather.do_current();
            UNIT
        }
        Request::ResetWeather => {
            s.weather.reset_saved();
            UNIT
        }
        Request::SetPalette {
            mode,
            color,
            amount,
            frames,
            pulse,
            keep,
        } => {
            let current = s.palette.map_or(0, |p| p.current);
            s.palette = (amount != 0 || current != 0).then_some(crate::sky::PaletteFx {
                mode,
                color,
                amount: amount.min(16),
                current: if frames == 0 { amount.min(16) } else { current },
                frames,
                t: 0,
                pulse,
                keep,
            });
            UNIT
        }
        Request::ShowOverlay { id, frames } => {
            if frames == 0 || h.game.overlay(id).is_none() {
                s.log("overlay the game lacks: skipped");
                return UNIT;
            }
            s.fx.overlay = Some(crate::sky::OverlayRun { id, frames, t: 0 });
            None
        }
        Request::WaitInputOrTimeout(0) => Some(Response::Bool(false)),
        Request::WaitInputOrTimeout(n) => {
            s.delay = n;
            None
        }
        _ => UNIT,
    }
}

/// The answer once the effect or the wait is over, polled every frame after [`begin`] answered
/// `None`.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let s = &mut *h.s;
    match *r {
        Request::ScreenFlash { .. } => s.fx.flash.is_none().then_some(Response::Unit),
        Request::ScreenShake { .. } => s.fx.shake.is_none().then_some(Response::Unit),
        Request::CameraPan { .. } => s.fx.pan.is_none_or(|p| p.done()).then_some(Response::Unit),
        Request::ShowOverlay { .. } => s.fx.overlay.is_none().then_some(Response::Unit),
        Request::WaitInputOrTimeout(_) => {
            if s.input.pressed.0 & (Buttons::A.0 | Buttons::B.0) != 0 {
                s.delay = 0;
                return Some(Response::Bool(true));
            }
            s.delay = s.delay.saturating_sub(1);
            (s.delay == 0).then_some(Response::Bool(false))
        }
        _ => Some(Response::Unit),
    }
}

/// Whether losing the trainer battle the script waits on whites out (Gen3 `DoWhiteOut`).
/// `false` for `BattleTrainer { no_whiteout: true }`: the loss answers `Lost` and the script
/// goes on (Silice's Samuel, R-7).
pub(in crate::world) fn whiteout_allowed(waiting: Option<&Request>) -> bool {
    !matches!(
        waiting,
        Some(Request::BattleTrainer {
            no_whiteout: true,
            ..
        })
    )
}
