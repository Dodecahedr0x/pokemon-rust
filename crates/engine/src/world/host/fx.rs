//! Screen effects and timed waits (Milestone 9 Task R owns the file): `ScreenFlash`,
//! `ScreenShake`, `SetTint` (`crate::fx`), `WaitInputOrTimeout`, and the trainer battle's
//! `no_whiteout`.
//!
//! The effects do nothing and the wait times out at once; `no_whiteout` is honoured.

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

/// The answer once the effect or the wait is over, polled every frame after [`begin`] answered
/// `None` (Task R). Wave 0: [`begin`] never waits.
pub(super) fn resume<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    begin(h, r)
}

/// Whether losing the trainer battle the script waits on whites out (Gen3 `DoWhiteOut`).
/// `false` for `BattleTrainer { no_whiteout: true }`: the loss answers `Lost` to the script,
/// which goes on where the battle was (the party is left fainted; the script heals it).
pub(in crate::world) fn whiteout_allowed(waiting: Option<&Request>) -> bool {
    !matches!(
        waiting,
        Some(Request::BattleTrainer {
            no_whiteout: true,
            ..
        })
    )
}
