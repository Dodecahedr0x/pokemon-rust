//! The trainer approach (owned by Milestone 8 Task I): after a sighting, the trainer walks up
//! to the player (Gen3 `TrainerSeeFunc` steps, `trainer_see.c`).

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

/// Wave 0: answers at once.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = (&h.s, r);
    UNIT
}

/// Whether the approach is done (Wave 0 never waits).
pub(super) fn done<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> bool {
    let _ = (&h.s, r);
    true
}
