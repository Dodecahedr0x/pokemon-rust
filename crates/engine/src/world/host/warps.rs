//! Warp slots (owned by Milestone 8 Task F): Gen3 `setdynamicwarp` and `setescapewarp`.

use super::{Host, UNIT};
use crate::data::GameData;
use crate::script::{Request, Response, WarpSlot};
use crate::world::WarpDest;

/// Stores the warp in its slot (Gen3 `SetDynamicWarpWithCoords` / `SetEscapeWarp`). A warp to
/// `grid::MAP_DYNAMIC` reads the dynamic slot (`World::start_warp`); the escape slot waits for
/// Escape Rope and Dig.
pub(super) fn begin<G: GameData>(h: &mut Host<'_, G>, r: &Request) -> Option<Response> {
    if let Request::SetWarp {
        slot,
        map,
        warp,
        x,
        y,
    } = *r
    {
        let d = Some(WarpDest { map, warp, x, y });
        match slot {
            WarpSlot::Dynamic => h.s.warp_slots.dynamic = d,
            WarpSlot::Escape => h.s.warp_slots.escape = d,
        }
    }
    UNIT
}
