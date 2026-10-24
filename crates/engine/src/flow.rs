//! Game flow state: event flags and vars, screen fades, step/interaction triggers and the warp
//! transition. Everything here is pure state plus pure functions over [`crate::data`]; the
//! integration (`World`) owns the instances and wires them to the field, objects and scripts.

mod fade;
mod input;
mod transition;
mod trigger;
mod vars;

#[cfg(test)]
mod test_data;

pub use fade::{FADE_MAX, Fader};
pub use input::Input;
pub use transition::{Transition, TransitionStep};
pub use trigger::{
    Target, behavior_at, block_behavior, coord_trigger, door_warp, elevation_matches,
    event_matches, facing_elevation, interact_target, map_script_on_frame, on_warp_into,
    warp_on_enter, warp_on_push,
};
pub use vars::{
    FLAG_COUNT, SPECIAL_VARS_END, SPECIAL_VARS_START, TEMP_FLAGS_END, TEMP_VARS_END, VARS_END,
    VARS_START, Vars,
};
