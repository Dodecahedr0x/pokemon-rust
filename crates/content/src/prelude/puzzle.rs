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
    (MAP_UNDERWATER_MARINE_CAVE, (MAP_MARINE_CAVE_ENTRANCE, 10, 17)),
    (MAP_UNDERWATER_SOOTOPOLIS_CITY, (MAP_SOOTOPOLIS_CITY, 29, 53)),
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

/// The fly map's destinations (Gen3 `region_map.c`): the town and city sections
/// `MAPSEC_LITTLEROOT_TOWN` (0) to `MAPSEC_EVER_GRANDE_CITY` (15) in order, each listed once its
/// `FLAG_VISITED_*` is set (`GetMapsecType`), landing on its `sMapHealLocations` heal location
/// (`SetFlyDestination` → `SetWarpDestinationToHealLocation`). Not yet: Littleroot's house by
/// the player's gender (Brendan's here), Ever Grande's League entrance once
/// `FLAG_LANDMARK_POKEMON_LEAGUE` is set, Southern Island and the Battle Frontier.
#[cfg(not(pkr_silice))]
const FLY_DESTINATIONS: [(FlagId, u8); 16] = [
    (FLAG_VISITED_LITTLEROOT_TOWN, HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE),
    (FLAG_VISITED_OLDALE_TOWN, HEAL_LOCATION_OLDALE_TOWN),
    (FLAG_VISITED_DEWFORD_TOWN, HEAL_LOCATION_DEWFORD_TOWN),
    (FLAG_VISITED_LAVARIDGE_TOWN, HEAL_LOCATION_LAVARIDGE_TOWN),
    (FLAG_VISITED_FALLARBOR_TOWN, HEAL_LOCATION_FALLARBOR_TOWN),
    (FLAG_VISITED_VERDANTURF_TOWN, HEAL_LOCATION_VERDANTURF_TOWN),
    (FLAG_VISITED_PACIFIDLOG_TOWN, HEAL_LOCATION_PACIFIDLOG_TOWN),
    (FLAG_VISITED_PETALBURG_CITY, HEAL_LOCATION_PETALBURG_CITY),
    (FLAG_VISITED_SLATEPORT_CITY, HEAL_LOCATION_SLATEPORT_CITY),
    (FLAG_VISITED_MAUVILLE_CITY, HEAL_LOCATION_MAUVILLE_CITY),
    (FLAG_VISITED_RUSTBORO_CITY, HEAL_LOCATION_RUSTBORO_CITY),
    (FLAG_VISITED_FORTREE_CITY, HEAL_LOCATION_FORTREE_CITY),
    (FLAG_VISITED_LILYCOVE_CITY, HEAL_LOCATION_LILYCOVE_CITY),
    (FLAG_VISITED_MOSSDEEP_CITY, HEAL_LOCATION_MOSSDEEP_CITY),
    (FLAG_VISITED_SOOTOPOLIS_CITY, HEAL_LOCATION_SOOTOPOLIS_CITY),
    (FLAG_VISITED_EVER_GRANDE_CITY, HEAL_LOCATION_EVER_GRANDE_CITY),
];
#[cfg(pkr_silice)]
const FLY_DESTINATIONS: [(FlagId, u8); 0] = [];

/// Fly destination `i` ([`FLY_DESTINATIONS`]); its name is its section's
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
