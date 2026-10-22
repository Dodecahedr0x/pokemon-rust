//! Warp slots (owned by Milestone 8 Task F): Gen3 `setdynamicwarp` and `setescapewarp`.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

/// Wave 0: nothing is stored.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = (&h.s, r);
    UNIT
}
