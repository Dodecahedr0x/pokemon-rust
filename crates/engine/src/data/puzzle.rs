//! Per-map field puzzles: the Gen3 per-step callbacks a map's scripts turn on
//! (`setstepcallback` in its `on_resume` script, `field_tasks.c`), as data the game answers
//! through [`super::GameData::step_callback`]. The engine runs them (`world/puzzle.rs`); the
//! game only names the metatiles and vars, so nothing here is Sootopolis- or Sky
//! Pillar-specific.

use super::VarId;

/// A map's per-step callback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepCallback {
    /// Gen3 `STEP_CB_SOOTOPOLIS_ICE` (`SootopolisGymIcePerStepCallback`): thin ice the player
    /// steps on cracks, cracked ice breaks.
    Ice(&'static IcePuzzle),
    /// Gen3 `STEP_CB_CRACKED_FLOOR` (`CrackedFloorPerStepCallback`): a cracked floor the player
    /// steps on becomes a hole.
    CrackedFloor(&'static CrackedFloor),
}

/// Gen3 `SootopolisGymIcePerStepCallback`'s data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IcePuzzle {
    /// Gen3 `VAR_ICE_STEP_COUNT`: plus one for each thin ice cell stepped on, 0 when a cracked
    /// one is (the map's `on_frame` scripts read it: stairs, the fall).
    pub steps: Option<VarId>,
    /// The metatile thin ice becomes once cracked (Gen3 `METATILE_SootopolisGym_Ice_Cracked`).
    pub cracked: u16,
    /// The metatile cracked ice becomes once broken (Gen3 `METATILE_SootopolisGym_Ice_Broken`).
    pub broken: u16,
    /// Where cracked cells are remembered, if anywhere (Gen3 `MarkIcePuzzleCoordVisited`).
    pub rows: Option<IceRows>,
}

/// Gen3 `sSootopolisGymIceRowVars`: one var per map row, bit `x - left` set once thin ice at
/// (x, y) has cracked, for cells with `left <= x < left + width` and `top <= y < top +
/// vars.len()` whose row has a var.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IceRows {
    pub left: i16,
    pub top: i16,
    /// Columns covered, at most 16 (a var's bits; Gen3 `ICE_PUZZLE_WIDTH` is 11).
    pub width: i16,
    pub vars: &'static [Option<VarId>],
}

impl IceRows {
    /// The var and bit remembering (x, y), if the cell is in the puzzle (Gen3
    /// `CoordInIcePuzzleRegion`).
    pub fn bit(&self, x: i16, y: i16) -> Option<(VarId, u16)> {
        let (dx, dy) = (x - self.left, y - self.top);
        if !(0..self.width.min(16)).contains(&dx) || dy < 0 {
            return None;
        }
        let var = (*self.vars.get(dy as usize)?)?;
        Some((var, 1 << dx))
    }
}

/// Gen3 `CrackedFloorPerStepCallback`'s data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CrackedFloor {
    /// Gen3 `VAR_ICE_STEP_COUNT` ("does double duty"): set to 0 when the player steps on a
    /// cracked floor below the Mach Bike's top speed (always, without bikes), and on every
    /// frame they stand on a hole (the map's `on_frame` script then drops them).
    pub steps: Option<VarId>,
    /// (cracked floor metatile, its hole metatile) pairs (Gen3 `SetCrackedFloorHoleMetatile`).
    pub holes: &'static [(u16, u16)],
    /// The hole a cracked floor whose metatile is not in `holes` becomes.
    pub hole: u16,
}

impl CrackedFloor {
    /// The hole metatile the cracked floor metatile `metatile` becomes.
    pub fn hole_for(&self, metatile: u16) -> u16 {
        self.holes
            .iter()
            .find(|h| h.0 == metatile)
            .map_or(self.hole, |h| h.1)
    }
}
