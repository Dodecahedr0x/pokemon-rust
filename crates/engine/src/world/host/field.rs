//! Field effects and object commands (Wave C): `FieldEffectArg`, `FieldEffect`,
//! `WaitFieldEffect` (Gen3 `field_effect.c`'s argument array and active list),
//! `CopyObjectPosToTemplate` and `SetMapLayout`.
//!
//! Only the timing of a field effect is modelled: the content gives each effect's Gen3 length
//! in frames, and `WaitFieldEffect` waits until it has run out. The effect's sprites are not
//! drawn (TODO: the Center's glowing balls, the sparkle, the flying bird).

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};
use alloc::vec::Vec;

/// Gen3 `gFieldEffectArguments` length.
pub(in crate::world) const FIELD_EFFECT_ARGS: usize = 8;
/// Gen3 `FIELD_EFFECT_COUNT`-sized active list: at most this many effects at once (Gen3
/// `sFieldEffectActiveList` has 32 slots).
const ACTIVE_MAX: usize = 32;

/// The field effects running and their arguments. Transient (Gen3 RAM, not saved): neither
/// hashed nor snapshotted, as checkpoints are taken with no script running.
#[derive(Default)]
pub(in crate::world) struct FieldEffects {
    /// Gen3 `gFieldEffectArguments`.
    pub args: [i16; FIELD_EFFECT_ARGS],
    /// (effect id, frames left), in start order.
    active: Vec<(u16, u16)>,
}

impl FieldEffects {
    /// Whether effect `id` is running (Gen3 `FieldEffectActiveListContains`).
    pub fn contains(&self, id: u16) -> bool {
        self.active.iter().any(|&(e, _)| e == id)
    }

    /// One frame: every effect's countdown, the finished ones leave the list.
    pub fn tick(&mut self) {
        for e in &mut self.active {
            e.1 = e.1.saturating_sub(1);
        }
        self.active.retain(|&(_, left)| left > 0);
    }

    fn start(&mut self, id: u16, frames: u16) {
        if frames > 0 && self.active.len() < ACTIVE_MAX {
            self.active.push((id, frames));
        }
    }
}

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let s = &mut *h.s;
    match *r {
        Request::FieldEffectArg { n, value } => {
            if let Some(a) = s.field_effects.args.get_mut(n as usize) {
                *a = value;
            }
            UNIT
        }
        Request::FieldEffect { id, frames } => {
            s.field_effects.start(id, frames);
            UNIT
        }
        // Gen3 `ScrCmd_waitfieldeffect` returns TRUE: the check runs from the next frame on.
        Request::WaitFieldEffect(_) => h.wait(r),
        Request::CopyObjectPosToTemplate(obj) => {
            if let Some(pos) = s.objects.get(obj).map(|o| o.pos()) {
                s.edit_template(obj, |t| (t.x, t.y) = pos);
            }
            UNIT
        }
        Request::SetMapLayout(layout) => {
            s.field.set_next_layout(Some(layout));
            UNIT
        }
        _ => UNIT,
    }
}

/// Whether a waiting `WaitFieldEffect` is done.
pub(super) fn done<G: GameData>(h: &Host<'_, G>, r: &Request) -> bool {
    match *r {
        Request::WaitFieldEffect(id) => !h.s.field_effects.contains(id),
        _ => true,
    }
}
