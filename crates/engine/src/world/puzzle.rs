//! The map's per-step callback ([`GameData::step_callback`]): Gen3's
//! `SootopolisGymIcePerStepCallback` and `CrackedFloorPerStepCallback` (`field_tasks.c`), run
//! once per field frame after the objects moved (Gen3 runs the task every frame, whatever owns
//! the player: input, scripts, warps).
//!
//! Gen3's task notices a step one frame after it starts (the player's coordinates change in
//! `AnimateSprites`, after `RunTasks`), then counts its delay down a frame at a time. Both
//! callbacks finish within one walk or run step, so here they are timed by the frames the
//! player's step has played ([`Objects::player_step_frames`]: 1 on the frame it starts) and
//! need no state of their own:
//! - ice: the step is counted on its frame [`NOTICED`] (thin ice: `VAR_ICE_STEP_COUNT` plus
//!   one; cracked ice: 0), and the cell cracks or breaks on frame [`ICE_CRACKS`] (`tDelay` 4);
//! - cracked floor: on frame [`NOTICED`] a cracked floor sets the var to 0 (below the Mach
//!   Bike's top speed, always on foot), and becomes a hole on frame [`FLOOR_BREAKS`]
//!   (`tFloor1Delay` 3); on every frame the player's cell is a hole, the var is 0.
//!
//! Differences from Gen3: a step shorter than [`ICE_CRACKS`] frames (`walk_faster`, the bikes
//! M10 brings) ends before its effect, which Gen3 still applies after the step; Gen3's second
//! pending cracked floor (`tFloor2*`) only matters for such steps.
//!
//! [`Objects::player_step_frames`]: crate::objects::Objects::player_step_frames

use super::State;
use crate::data::{
    BLOCK_METATILE_MASK, Behavior, CrackedFloor, GameData, IcePuzzle, StepCallback, block_metatile,
};
use crate::flow::block_behavior;
use crate::video::Frame;

/// The step frame Gen3's task notices the player's new cell on.
pub const NOTICED: u8 = 2;
/// The step frame thin ice cracks (cracked ice breaks) on: noticed, then `tDelay` 4 frames.
pub const ICE_CRACKS: u8 = NOTICED + 5;
/// The step frame a cracked floor becomes a hole on: noticed, then `tFloor1Delay` 3 frames.
pub const FLOOR_BREAKS: u8 = NOTICED + 3;

/// Block bits a puzzle's metatile change keeps (Gen3 `MapGridSetMetatileIdAt`: the elevation;
/// the collision is cleared).
const ELEVATION_MASK: u16 = 0xF000;

/// Runs the current map's per-step callback for this frame.
pub(super) fn step(game: &impl GameData, s: &mut State, f: &mut Frame) {
    let Some(cb) = game.step_callback(s.field.map()) else {
        return;
    };
    match cb {
        StepCallback::Ice(p) => ice(game, s, f, p),
        StepCallback::CrackedFloor(c) => cracked_floor(game, s, f, c),
    }
}

/// The player's cell and its behavior.
fn here(game: &impl GameData, s: &State) -> ((i16, i16), u16, Behavior) {
    let (x, y) = s.objects.player().pos();
    let block = s.field.block(game, x, y);
    let b = block_behavior(game, s.field.data(game), block);
    ((x, y), block, b)
}

/// Gen3 `MapGridSetMetatileIdAt` then `CurrentMapDrawMetatileAt`.
fn set_metatile(game: &impl GameData, s: &mut State, f: &mut Frame, (x, y): (i16, i16), mt: u16) {
    let old = s.field.block(game, x, y);
    let block = (old & ELEVATION_MASK) | (mt & BLOCK_METATILE_MASK);
    s.field.set_block(game, x, y, block, f);
}

fn ice(game: &impl GameData, s: &mut State, f: &mut Frame, p: &IcePuzzle) {
    let Some(t) = s.objects.player_step_frames() else {
        return;
    };
    let (cell, _, b) = here(game, s);
    match (t, b) {
        (NOTICED, Behavior::ThinIce) => {
            if let Some(v) = p.steps {
                let n = s.vars.var(v).wrapping_add(1);
                s.vars.set_var(v, n);
            }
        }
        (NOTICED, Behavior::CrackedIce) => {
            if let Some(v) = p.steps {
                s.vars.set_var(v, 0);
            }
        }
        (ICE_CRACKS, Behavior::ThinIce) => {
            set_metatile(game, s, f, cell, p.cracked);
            // Gen3 `MarkIcePuzzleCoordVisited`.
            if let Some((v, bit)) = p.rows.and_then(|r| r.bit(cell.0, cell.1)) {
                let n = s.vars.var(v) | bit;
                s.vars.set_var(v, n);
            }
        }
        (ICE_CRACKS, Behavior::CrackedIce) => set_metatile(game, s, f, cell, p.broken),
        _ => {}
    }
}

fn cracked_floor(game: &impl GameData, s: &mut State, f: &mut Frame, c: &CrackedFloor) {
    // Gen3 reads the behavior first, so a hole made this frame counts from the next one.
    let (cell, block, b) = here(game, s);
    let t = s.objects.player_step_frames();
    if t == Some(FLOOR_BREAKS) && b == Behavior::CrackedFloor {
        set_metatile(game, s, f, cell, c.hole_for(block_metatile(block)));
    }
    let fall =
        b == Behavior::CrackedFloorHole || (t == Some(NOTICED) && b == Behavior::CrackedFloor);
    if fall && let Some(v) = c.steps {
        s.vars.set_var(v, 0);
    }
}
