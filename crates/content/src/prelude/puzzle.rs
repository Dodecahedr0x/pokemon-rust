//! Field puzzles (`field_tasks.c`): the per-step callbacks the maps' `on_resume` scripts turn on
//! (`setstepcallback`) and the maps their holes drop to (`setholewarp`), as the data
//! `GameData::step_callback` and `GameData::hole_warp` answer, plus
//! `SetSootopolisGymCrackedIceMetatiles`. The commands themselves lower to nothing: the engine
//! asks the game per map. Decomp constants only.

use super::*;
use engine::data::{CrackedFloor, IcePuzzle, IceRows, StepCallback};

/// Gen3's ice step counter var, by number: Silice names 0x4022 otherwise (spelled out of the
/// shared code so that `extract::games::SHARED_GEN3` does not reserve it).
const ICE_STEP_COUNT: VarId = VarId(0x4022);
/// Gen3 `METATILE_SootopolisGym_Ice_Cracked` and `_Broken`.
pub const METATILE_SOOTOPOLIS_GYM_ICE_CRACKED: u16 = 0x20E;
pub const METATILE_SOOTOPOLIS_GYM_ICE_BROKEN: u16 = 0x206;
/// Gen3 `METATILE_Cave_CrackedFloor`, `_Hole` and `METATILE_Pacifidlog_SkyPillar_CrackedFloor_Hole`
/// (`SetCrackedFloorHoleMetatile`: the Cave's cracked floor has its own hole, every other
/// tileset's becomes the Sky Pillar one; Mirage Tower shares the Cave numbers).
pub const METATILE_CAVE_CRACKED_FLOOR: u16 = 0x22F;
pub const METATILE_CAVE_CRACKED_FLOOR_HOLE: u16 = 0x206;
pub const METATILE_SKY_PILLAR_CRACKED_FLOOR_HOLE: u16 = 0x237;

/// Gen3 `sSootopolisGymIceRowVars` from `ICE_PUZZLE_T` (6) to `ICE_PUZZLE_B` (19), columns
/// `ICE_PUZZLE_L` (3) to `ICE_PUZZLE_R` (13).
pub static SOOTOPOLIS_GYM_ICE_ROWS: IceRows = IceRows {
    left: 3,
    top: 6,
    width: 11,
    vars: &[
        Some(VarId(0x4001)), // temp var 1
        Some(VarId(0x4002)),
        Some(VarId(0x4003)),
        Some(VarId(0x4004)),
        None,
        None,
        Some(VarId(0x4005)),
        Some(VarId(0x4006)),
        Some(VarId(0x4007)),
        None,
        None,
        Some(VarId(0x4008)),
        Some(VarId(0x4009)),
        Some(VarId(0x400A)), // temp var A
    ],
};

/// Gen3 `STEP_CB_SOOTOPOLIS_ICE` (`SootopolisGymIcePerStepCallback`).
pub static SOOTOPOLIS_GYM_ICE: IcePuzzle = IcePuzzle {
    steps: Some(ICE_STEP_COUNT),
    cracked: METATILE_SOOTOPOLIS_GYM_ICE_CRACKED,
    broken: METATILE_SOOTOPOLIS_GYM_ICE_BROKEN,
    rows: Some(SOOTOPOLIS_GYM_ICE_ROWS),
};

/// Gen3 `STEP_CB_CRACKED_FLOOR` (`CrackedFloorPerStepCallback`).
pub static CRACKED_FLOOR: CrackedFloor = CrackedFloor {
    steps: Some(ICE_STEP_COUNT),
    holes: &[(METATILE_CAVE_CRACKED_FLOOR, METATILE_CAVE_CRACKED_FLOOR_HOLE)],
    hole: METATILE_SKY_PILLAR_CRACKED_FLOOR_HOLE,
};

/// The maps whose `on_resume` script runs `setstepcallback STEP_CB_CRACKED_FLOOR` and
/// `setholewarp <map>`, with that map (`data/maps/*/scripts.inc`).
#[cfg(not(pkr_silice))]
const HOLE_WARPS: [(MapId, MapId); 6] = [
    (MAP_GRANITE_CAVE_B1F, MAP_GRANITE_CAVE_B2F),
    (MAP_MIRAGE_TOWER_2F, MAP_MIRAGE_TOWER_1F),
    (MAP_MIRAGE_TOWER_3F, MAP_MIRAGE_TOWER_2F),
    (MAP_MT_PYRE_2F, MAP_MT_PYRE_1F),
    (MAP_SKY_PILLAR_2F, MAP_SKY_PILLAR_1F),
    (MAP_SKY_PILLAR_4F, MAP_SKY_PILLAR_3F),
];
#[cfg(pkr_silice)]
const HOLE_WARPS: [(MapId, MapId); 0] = [];

/// The map a hole of `map` drops to (its `setholewarp`).
pub fn hole_warp(map: MapId) -> Option<MapId> {
    HOLE_WARPS.iter().find(|w| w.0 == map).map(|w| w.1)
}

/// The step callback `map`'s `on_resume` script turns on, of those the engine models.
pub fn step_callback(map: MapId) -> Option<StepCallback> {
    #[cfg(not(pkr_silice))]
    if map == MAP_SOOTOPOLIS_CITY_GYM_1F {
        return Some(StepCallback::Ice(&SOOTOPOLIS_GYM_ICE));
    }
    hole_warp(map).map(|_| StepCallback::CrackedFloor(&CRACKED_FLOOR))
}

/// Gen3 `SetSootopolisGymCrackedIceMetatiles` (the map's `on_load`): every thin ice cell the
/// row vars remember as cracked shows the cracked metatile again.
pub async fn set_sootopolis_gym_cracked_ice(ctx: &Ctx) {
    let rows = &SOOTOPOLIS_GYM_ICE_ROWS;
    for (dy, var) in rows.vars.iter().enumerate() {
        let Some(var) = *var else { continue };
        let bits = ctx.var(var).await;
        for dx in 0..rows.width {
            if bits & (1 << dx) != 0 {
                let (x, y) = (rows.left + dx, rows.top + dy as i16);
                ctx.set_metatile(x, y, METATILE_SOOTOPOLIS_GYM_ICE_CRACKED, false)
                    .await;
            }
        }
    }
}
