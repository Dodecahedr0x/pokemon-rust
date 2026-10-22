//! The trainer approach (owned by Milestone 8 Task I): after a sighting, the trainer walks up
//! to the player (Gen3 `TrainerSeeFunc` steps, `trainer_see.c`; frames in
//! `objects/approach.rs`).
//!
//! `TrainerApproach { obj }` is issued by content's `trainer_battle` before every regular kind.
//! - If a sighting of `obj` is pending, the approach runs and the request is answered
//!   `Response::Unit` once the player faces the trainer (the host's waiting answer).
//! - Else it is answered at once with `Response::Value(n)`, `n` the player's usable mons (HP
//!   above 0; Gen3 `GetMonsStateToDoubles`), which the double kinds need.

use super::{Host, UNIT};
use crate::data::{GameData, MovementType};
use crate::objects::ApproachStep;
use crate::script::{Request, Response};

pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let Request::TrainerApproach { obj } = *r else {
        return UNIT;
    };
    if !h.immediate && h.s.objects.approach_begin(obj) {
        return None;
    }
    let usable = h.s.party.iter().filter(|m| m.hp != 0).count() as u16;
    Some(Response::Value(usable))
}

/// One frame of the approach; true once the script may go on.
pub(super) fn done<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> bool {
    let _ = r;
    let s = &mut *h.s;
    match s.objects.approach_step() {
        ApproachStep::Waiting => false,
        ApproachStep::Stopped { obj, dir, pos } => {
            // Gen3 `TryOverrideTemplateCoordsForObjectEvent` + `OverrideTemplateCoordsForObjectEvent`:
            // the trainer stays where it stopped for the rest of the visit.
            if let Some(t) = s.template_mut(obj) {
                t.movement = MovementType::Face(dir);
                (t.x, t.y) = pos;
            }
            false
        }
        ApproachStep::Done => true,
    }
}
