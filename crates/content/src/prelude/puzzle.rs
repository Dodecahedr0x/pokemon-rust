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
    holes: &[(
        METATILE_CAVE_CRACKED_FLOOR,
        METATILE_CAVE_CRACKED_FLOOR_HOLE,
    )],
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

/// `rotating_gate.c`'s puzzles: `{x, y, GATE_SHAPE_*, GATE_ORIENTATION_*}` (`L1`..`L4` are
/// shapes 0 to 3, `T1`..`T4` 4 to 7).
const fn gate(x: i16, y: i16, shape: u8, orientation: u8) -> engine::data::RotatingGate {
    engine::data::RotatingGate {
        x,
        y,
        shape,
        orientation,
    }
}
const L1: u8 = 0;
const L2: u8 = 1;
const L3: u8 = 2;
const L4: u8 = 3;
const T1: u8 = 4;
const T2: u8 = 5;
const T3: u8 = 6;

/// `sRotatingGate_FortreePuzzleConfig`.
static FORTREE_GATES: [engine::data::RotatingGate; 8] = [
    gate(6, 7, T2, 1),
    gate(9, 15, T2, 2),
    gate(3, 19, T2, 1),
    gate(2, 6, T1, 1),
    gate(9, 12, T1, 0),
    gate(6, 23, T1, 0),
    gate(12, 22, T1, 0),
    gate(6, 3, L4, 2),
];

/// `sRotatingGate_TrickHousePuzzleConfig`.
static TRICK_HOUSE_GATES: [engine::data::RotatingGate; 11] = [
    gate(14, 5, T1, 1),
    gate(10, 6, L2, 2),
    gate(6, 6, L4, 1),
    gate(14, 8, T1, 1),
    gate(3, 10, L3, 3),
    gate(9, 14, L1, 1),
    gate(3, 15, T3, 0),
    gate(2, 17, L2, 2),
    gate(12, 18, T3, 3),
    gate(5, 18, L4, 1),
    gate(10, 19, L3, 2),
];

/// Gen3 `GetCurrentMapRotatingGatePuzzleType`: the rotating gates of `map` (Fortree's gym,
/// the Trick House's sixth puzzle), empty elsewhere.
pub fn rotating_gates(map: MapId) -> &'static [engine::data::RotatingGate] {
    #[cfg(not(pkr_silice))]
    {
        if map == MAP_FORTREE_CITY_GYM {
            return &FORTREE_GATES;
        }
        if map == MAP_ROUTE110_TRICK_HOUSE_PUZZLE6 {
            return &TRICK_HOUSE_GATES;
        }
    }
    let _ = (map, &FORTREE_GATES, &TRICK_HOUSE_GATES);
    &[]
}

/// Gen3 `RotatingGate_InitPuzzle` (the maps' `on_transition`): every gate of the current map's
/// puzzle back to its starting orientation, in the temp vars' bytes
/// (`RotatingGate_ResetAllGateOrientations`).
pub async fn rotating_gate_init(ctx: &Ctx) {
    let gates = rotating_gates(ctx.current_map().await);
    for (pair, g) in gates.chunks(2).enumerate() {
        let lo = u16::from(g[0].orientation);
        let var = VarId(0x4000 + pair as u16);
        let hi = match g.get(1) {
            Some(g1) => u16::from(g1.orientation),
            None => ctx.var(var).await >> 8,
        };
        ctx.set_var(var, lo | hi << 8).await;
    }
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

/// The fixed dive warps the maps' `on_resume` scripts set (`setdivewarp <map>, <x>, <y>`,
/// `data/maps/*/scripts.inc`): where Dive leads from a map without a dive or emerge connection
/// (Gen3 `SetDiveWarp`'s `SetWarpDestinationToDiveWarp`). Left out:
/// `Underwater_SealedChamber`'s `MAP_SCRIPT_ON_DIVE_WARP`, which picks one by the player's
/// cell.
#[cfg(not(pkr_silice))]
const DIVE_WARPS: [(MapId, (MapId, i16, i16)); 10] = [
    (MAP_MARINE_CAVE_ENTRANCE, (MAP_UNDERWATER_MARINE_CAVE, 9, 6)),
    (MAP_ROUTE134, (MAP_UNDERWATER_ROUTE134, 8, 6)),
    (
        MAP_SEALED_CHAMBER_OUTER_ROOM,
        (MAP_UNDERWATER_SEALED_CHAMBER, 12, 44),
    ),
    (MAP_SOOTOPOLIS_CITY, (MAP_UNDERWATER_SOOTOPOLIS_CITY, 9, 6)),
    (
        MAP_ABANDONED_SHIP_UNDERWATER2,
        (MAP_ABANDONED_SHIP_ROOMS_B1F, 13, 7),
    ),
    (
        MAP_SEAFLOOR_CAVERN_ENTRANCE,
        (MAP_UNDERWATER_SEAFLOOR_CAVERN, 6, 5),
    ),
    (
        MAP_ABANDONED_SHIP_CORRIDORS_B1F,
        (MAP_ABANDONED_SHIP_UNDERWATER1, 5, 4),
    ),
    (
        MAP_UNDERWATER_SEAFLOOR_CAVERN,
        (MAP_SEAFLOOR_CAVERN_ENTRANCE, 10, 17),
    ),
    (
        MAP_UNDERWATER_MARINE_CAVE,
        (MAP_MARINE_CAVE_ENTRANCE, 10, 17),
    ),
    (
        MAP_UNDERWATER_SOOTOPOLIS_CITY,
        (MAP_SOOTOPOLIS_CITY, 29, 53),
    ),
];
#[cfg(pkr_silice)]
const DIVE_WARPS: [(MapId, (MapId, i16, i16)); 0] = [];

/// Gen3 `SetDiveWarp` from `map`: its dive (`emerge` false) or emerge connection, keeping the
/// player's cell, else its fixed dive warp ([`DIVE_WARPS`]).
pub fn dive_warp(map: MapId, emerge: bool) -> Option<(MapId, Option<(i16, i16)>)> {
    if let Some(to) = crate::data::dive_connection(map.0, emerge) {
        return Some((to, None));
    }
    let &(_, (to, x, y)) = DIVE_WARPS.iter().find(|w| w.0 == map)?;
    Some((to, Some((x, y))))
}

#[cfg(not(pkr_silice))]
use crate::emerald_story::FLY_DESTINATIONS;
/// Silice has no fly map yet.
#[cfg(pkr_silice)]
const FLY_DESTINATIONS: [(FlagId, u8); 0] = [];

/// Fly destination `i` (`emerald_story::FLY_DESTINATIONS`); its name is its section's
/// (`gRegionMapEntries`, section number = `i`).
pub fn fly_destination(i: u8) -> Option<engine::data::FlyDestination> {
    let &(visited, heal) = FLY_DESTINATIONS.get(i as usize)?;
    let (map, x, y) = crate::battle::heal_location(heal)?;
    Some(engine::data::FlyDestination {
        name: crate::data::map_section(i).unwrap_or(&[0xFF]),
        visited,
        map,
        x,
        y,
    })
}

/// Gen3 `METATILE_MossdeepGym_YellowArrow_Right` and
/// `METATILE_TrickHousePuzzle_Arrow_YellowOnWhite_Right`: the rotating tile puzzles' first
/// arrow (`rotating_tile_puzzle.c`'s `puzzleTileStart`).
pub const METATILE_MOSSDEEP_GYM_YELLOW_ARROW_RIGHT: u16 = 0x250;
pub const METATILE_TRICK_HOUSE_YELLOW_ARROW_RIGHT: u16 = 0x298;

/// Gen3 `initrotatingtilepuzzle isTrickHouse` (`InitRotatingTilePuzzle`): the puzzle's arrows
/// start at Mossdeep's or the Trick House's first; either way no cell below Mossdeep's first
/// is a puzzle tile (`MoveRotatingTileObjects` compares with it for both).
pub async fn rotating_tile_puzzle(ctx: &Ctx, is_trick_house: u16) {
    let start = if is_trick_house != 0 {
        METATILE_TRICK_HOUSE_YELLOW_ARROW_RIGHT
    } else {
        METATILE_MOSSDEEP_GYM_YELLOW_ARROW_RIGHT
    };
    ctx.rotating_tiles_init(start, METATILE_MOSSDEEP_GYM_YELLOW_ARROW_RIGHT)
        .await;
}
