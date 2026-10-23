//! Screen effects and timed waits (Milestone 9 Task R owns the file): `ScreenFlash`,
//! `ScreenShake`, `SetTint` (`crate::fx`), `WaitInputOrTimeout`, and the trainer battle's
//! `no_whiteout`.
//!
//! Wave 0: the effects do nothing and the wait times out at once.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

/// Starts the effect (Task R). Wave 0: `Unit`, and `WaitInputOrTimeout` answers "timed out".
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = &h.s;
    match r {
        Request::WaitInputOrTimeout(_) => Some(Response::Bool(false)),
        _ => UNIT,
    }
}

/// Whether losing the trainer battle the script waits on whites out (Gen3 `DoWhiteOut`).
/// `false` for `BattleTrainer { no_whiteout: true }` (Task R). Wave 0: always `true`.
pub(in crate::world) fn whiteout_allowed(waiting: Option<&Request>) -> bool {
    let _ = waiting;
    true
}
