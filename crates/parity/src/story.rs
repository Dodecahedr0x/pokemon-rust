//! The main story, hand-kept from the decomp's story order: the maps each act walks through
//! (the critical path, its Pokémon Centers, gyms and plot buildings; not the optional houses),
//! Acts 1–8 to each badge, then the League. `story_maps_are_decomp_maps` checks the names
//! against the decomp.
//!
//! [`BLOCKING`] lists the features that stop the main story in our engine (the parity plan's
//! "Spine blockers" table, Task S): each is tagged `story-blocking` in the ledger. The spine's
//! `Bypass` steps (Task S) are read too ([`bypassed`]).

use crate::Category;

/// (act, its maps), in story order. A map is in the first act that walks through it, except
/// a gym, which is in the act of its badge (Norman's gym, visited in Act 1, is Act 5's).
pub const ACTS: &[(&str, &[&str])] = &[
    (
        "act1",
        &[
            "LittlerootTown",
            "LittlerootTown_BrendansHouse_1F",
            "LittlerootTown_BrendansHouse_2F",
            "LittlerootTown_MaysHouse_1F",
            "LittlerootTown_MaysHouse_2F",
            "LittlerootTown_ProfessorBirchsLab",
            "Route101",
            "OldaleTown",
            "OldaleTown_PokemonCenter_1F",
            "OldaleTown_Mart",
            "Route103",
            "Route102",
            "PetalburgCity",
            "PetalburgCity_PokemonCenter_1F",
            "PetalburgCity_WallysHouse",
            "Route104",
            "PetalburgWoods",
            "RustboroCity",
            "RustboroCity_PokemonCenter_1F",
            "RustboroCity_Gym",
        ],
    ),
    (
        "act2",
        &[
            "Route116",
            "RusturfTunnel",
            "RustboroCity_DevonCorp_1F",
            "RustboroCity_DevonCorp_2F",
            "RustboroCity_DevonCorp_3F",
            "Route104_MrBrineysHouse",
            "DewfordTown",
            "DewfordTown_PokemonCenter_1F",
            "DewfordTown_Gym",
            "Route106",
            "GraniteCave_1F",
            "GraniteCave_B1F",
            "GraniteCave_B2F",
            "GraniteCave_StevensRoom",
        ],
    ),
    (
        "act3",
        &[
            "Route109",
            "SlateportCity",
            "SlateportCity_PokemonCenter_1F",
            "SlateportCity_OceanicMuseum_1F",
            "SlateportCity_OceanicMuseum_2F",
            "Route110",
            "MauvilleCity",
            "MauvilleCity_PokemonCenter_1F",
            "MauvilleCity_Gym",
        ],
    ),
    (
        "act4",
        &[
            "Route117",
            "VerdanturfTown",
            "VerdanturfTown_PokemonCenter_1F",
            "Route111",
            "Route112",
            "FieryPath",
            "Route113",
            "FallarborTown",
            "FallarborTown_PokemonCenter_1F",
            "Route114",
            "MeteorFalls_1F_1R",
            "Route112_CableCarStation",
            "MtChimney_CableCarStation",
            "MtChimney",
            "JaggedPass",
            "LavaridgeTown",
            "LavaridgeTown_PokemonCenter_1F",
            "LavaridgeTown_Gym_1F",
            "LavaridgeTown_Gym_B1F",
        ],
    ),
    // Norman (Petalburg's gym is already Act 1's map); Surf from Wally's father.
    ("act5", &["PetalburgCity_Gym", "Route118"]),
    (
        "act6",
        &[
            "Route119",
            "Route119_WeatherInstitute_1F",
            "Route119_WeatherInstitute_2F",
            "FortreeCity",
            "FortreeCity_PokemonCenter_1F",
            "Route120",
            "FortreeCity_Gym",
        ],
    ),
    (
        "act7",
        &[
            "Route121",
            "Route122",
            "MtPyre_Exterior",
            "MtPyre_Summit",
            "LilycoveCity",
            "LilycoveCity_PokemonCenter_1F",
            "AquaHideout_1F",
            "AquaHideout_B1F",
            "AquaHideout_B2F",
            "Route124",
            "MossdeepCity",
            "MossdeepCity_PokemonCenter_1F",
            "MossdeepCity_SpaceCenter_1F",
            "MossdeepCity_SpaceCenter_2F",
            "MossdeepCity_Gym",
        ],
    ),
    (
        "act8",
        &[
            "Route127",
            "Route128",
            "Underwater_Route128",
            "Underwater_SeafloorCavern",
            "SeafloorCavern_Entrance",
            "SeafloorCavern_Room1",
            "SeafloorCavern_Room2",
            "SeafloorCavern_Room5",
            "SeafloorCavern_Room6",
            "SeafloorCavern_Room7",
            "SeafloorCavern_Room8",
            "SeafloorCavern_Room9",
            "Route126",
            "Underwater_Route126",
            "Underwater_SootopolisCity",
            "SootopolisCity",
            "SootopolisCity_PokemonCenter_1F",
            "CaveOfOrigin_Entrance",
            "CaveOfOrigin_1F",
            "CaveOfOrigin_B1F",
            "Route131",
            "SkyPillar_Outside",
            "SkyPillar_Entrance",
            "SkyPillar_1F",
            "SkyPillar_2F",
            "SkyPillar_3F",
            "SkyPillar_4F",
            "SkyPillar_5F",
            "SkyPillar_Top",
            "SootopolisCity_Gym_1F",
            "SootopolisCity_Gym_B1F",
        ],
    ),
    (
        "league",
        &[
            "EverGrandeCity",
            "VictoryRoad_1F",
            "VictoryRoad_B1F",
            "VictoryRoad_B2F",
            "EverGrandeCity_PokemonCenter_1F",
            "EverGrandeCity_PokemonLeague_1F",
            "EverGrandeCity_SidneysRoom",
            "EverGrandeCity_PhoebesRoom",
            "EverGrandeCity_GlaciasRoom",
            "EverGrandeCity_DrakesRoom",
            "EverGrandeCity_ChampionsRoom",
            "EverGrandeCity_HallOfFame",
        ],
    ),
];

/// The story act index of `map` (0 = Act 1), if the story walks through it.
pub fn act_of(map: &str) -> Option<usize> {
    ACTS.iter().position(|(_, maps)| maps.contains(&map))
}

/// Features that stop the main story in our engine, with the step and why (the parity plan's
/// "Spine blockers" table; Task S adds a row per blocker it records).
pub const BLOCKING: &[(Category, &str, &str)] = &[];

/// The features the spine's `Bypass` steps step around (Task S), read from the spine source:
/// every string literal of a `Bypass` step that names a ledger entry (`"dofieldeffect"`,
/// `"SPECIAL_…"`, `"MB_…"`, …). Empty when the spine has none.
pub fn bypassed(spine_src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in spine_src.split("Bypass").skip(1) {
        // The step's own text: up to the end of its expression.
        let end = chunk.find("),\n").map_or(chunk.len(), |e| e + 1);
        let mut parts = chunk[..end].split('"');
        while let (Some(_), Some(lit)) = (parts.next(), parts.next()) {
            if !lit.is_empty() && !lit.contains(' ') && !out.iter().any(|o| o == lit) {
                out.push(lit.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acts_list_each_map_once() {
        let mut seen = std::collections::BTreeSet::new();
        for (_, maps) in ACTS {
            for m in *maps {
                assert!(seen.insert(*m), "{m} twice");
            }
        }
        assert_eq!(act_of("Route101"), Some(0));
        assert_eq!(act_of("EverGrandeCity_HallOfFame"), Some(8));
    }

    #[test]
    fn bypass_literals() {
        let src = "Step::Goto { map: X },\n    Step::Bypass { feature: \"dofieldeffect\", why: \"the heal\" },\n";
        assert_eq!(bypassed(src), ["dofieldeffect"]);
    }
}
