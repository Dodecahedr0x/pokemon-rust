//! Silice's Champ Magnétique (`FLAG_SIL_MODULE_FORCE`, the Mallette's Strength): Gen3's
//! Strength push (`TryPushBoulder`) on the puzzles' objects, within each puzzle's rule.
//!
//! - **A on a pushable object** (`ch04_port::push_object`, Gen3 `EventScript_StrengthBoulder`
//!   with the module for the party move): without the module, `Depot_Bloc_T1`; with it, the
//!   first time on a map load, `Depot_Bloc_T2` and Strength in use (`FLAG_SYS_USE_STRENGTH`,
//!   cleared on every map load as Gen3's). The player then pushes by walking into the object.
//! - **Where it may go** ([`push_allowed`], `GameData::push_allowed`): Gen3's rule (a free cell,
//!   no object, no door) and the puzzle's own ([`Puzzle::free`]: the floor the port gave each
//!   puzzle, so that no push strands the player).
//! - **After each push** ([`pushed_script`], `scripts::field::Mallette_Pushed`): the object's
//!   cell in its temp var (`y * 64 + x + 1`; temp vars clear on every load, as the objects go
//!   home), then the map's completion check (the Tour du Scribe's niches and slabs, the
//!   Citadelle's plates).

use crate::data::script_ids;
use crate::scripts::{
    ch03_route_s03, ch04_gym, ch04_port, ch08_gym, ch08_tour, ch09_route_s09, ch10_cave, ch11_apex,
    ch12_breche, ch12_dehors,
};
use engine::data::{MapId, ScriptId, VarId};

/// A pushable object: its local id, the temp var that holds its cell once pushed
/// (`y * 64 + x + 1`; 0: its template's cell), and its template's cell.
pub type Pushable = (u8, VarId, (i16, i16));

/// A map's push puzzle.
pub struct Puzzle {
    pub map: MapId,
    pub objects: &'static [Pushable],
    /// Whether an object may be pushed from the first cell onto the second (on top of Gen3's
    /// rule).
    pub free: fn((i16, i16), (i16, i16)) -> bool,
}

/// Every push puzzle.
pub static PUZZLES: [&Puzzle; 12] = [
    &ch03_route_s03::DEPOT,
    &ch04_port::CONTAINER,
    &ch04_gym::FUTS,
    &ch08_tour::RAYONNAGES,
    &ch08_tour::CAISSES,
    &ch08_gym::GOLEMS_PUSH,
    &ch09_route_s09::CONTENEURS,
    &ch09_route_s09::ARMOIRE,
    &ch10_cave::FUTS,
    &ch11_apex::CAISSES,
    &ch12_breche::FUTS,
    &ch12_dehors::POUTRE,
];

/// The puzzle of `map` and object `local_id`'s entry in it.
pub fn find(map: MapId, local_id: u8) -> Option<(&'static Puzzle, &'static Pushable)> {
    PUZZLES
        .iter()
        .filter(|p| p.map == map)
        .find_map(|p| Some((*p, p.objects.iter().find(|o| o.0 == local_id)?)))
}

/// `GameData::push_allowed`: the puzzle's rule (any other pushable object: Gen3's alone).
pub fn push_allowed(map: MapId, local_id: u8, from: (i16, i16), to: (i16, i16)) -> bool {
    find(map, local_id).is_none_or(|(p, _)| (p.free)(from, to))
}

/// `GameData::pushed_script`: the bookkeeping script on the puzzles' maps.
pub fn pushed_script(map: MapId) -> Option<ScriptId> {
    PUZZLES
        .iter()
        .any(|p| p.map == map)
        .then_some(script_ids::Mallette_Pushed)
}

/// How long a pushed object takes to reach its cell (Gen3 `walk_slow`: 16 pixels, one every
/// other frame), which a completion waits for before its scene.
pub const PUSH_FRAMES: u16 = 32;

/// A cell as the temp vars hold it.
pub const fn cell_var((x, y): (i16, i16)) -> u16 {
    (y * 64 + x + 1) as u16
}
