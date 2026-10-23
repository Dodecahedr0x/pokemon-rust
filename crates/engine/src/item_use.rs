//! Item use out of battle (Gen3 `item_use.c`; Milestone 9 Task B owns the file).
//!
//! Wave 0 adds the field hooks `World::control` calls, both no-ops until Task B fills them:
//! - [`on_step`] once per finished player step that started no coord or warp script, after the
//!   friendship counter and before the wild encounter (Gen3 `UpdateRepelCounter` in
//!   `TryStartStepBasedScript`): the repel counter; a script it returns (the wear-off message,
//!   `SpecialIds::script_repel_wore_off`) starts instead of the encounter check;
//! - [`select_pressed`] on SELECT while the field is idle (Gen3 `UseRegisteredKeyItemOnField`):
//!   the registered item; a script it returns starts.

use crate::data::{GameData, ScriptId};
use crate::world::State;

/// Gen3 `UpdateRepelCounter` (Task B). Wave 0: nothing.
pub(crate) fn on_step(s: &mut State, game: &impl GameData) -> Option<ScriptId> {
    let _ = (s, game);
    None
}

/// Gen3 `UseRegisteredKeyItemOnField` (Task B). Wave 0: nothing (SELECT does nothing, as
/// before).
pub(crate) fn select_pressed(s: &mut State, game: &impl GameData) -> Option<ScriptId> {
    let _ = (s, game);
    None
}
