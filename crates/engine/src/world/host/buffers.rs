//! String buffers (owned by Milestone 8 Task H): Gen3 `buffer*` commands into `gStringVar1..3`.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response};

/// Wave 0: nothing is written.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    let _ = (&h.s, r);
    UNIT
}
