//! The Emerald story's flags, vars and maps that the shared Gen3 specials (`prelude::special`)
//! name (Wave C). The Silice flavour has none of them: there `prelude::special`'s `story` is
//! all `None`. Kept out of `prelude/` so that `extract::games::SHARED_GEN3` does not reserve
//! their numbers in Silice.

use crate::consts::*;
use crate::prelude::{GameClear, Rusturf};
use engine::data::{FlagId, VarId};

pub const BLACK_GLASSES: Option<FlagId> = Some(FLAG_HIDDEN_ITEM_ROUTE_116_BLACK_GLASSES);
pub const SHIP_KEYS: [Option<FlagId>; 4] = [
    Some(FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_1_KEY),
    Some(FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_2_KEY),
    Some(FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_4_KEY),
    Some(FLAG_HIDDEN_ITEM_ABANDONED_SHIP_RM_6_KEY),
];
pub const MIRAGE_RND_H: Option<VarId> = Some(VAR_MIRAGE_RND_H);
/// `SetMirageTowerVisibility`'s names: `VAR_MIRAGE_TOWER_STATE`, `FLAG_FORCE_MIRAGE_TOWER_VISIBLE`,
/// `FLAG_MIRAGE_TOWER_VISIBLE`.
pub const MIRAGE_TOWER: Option<(VarId, FlagId, FlagId)> = Some((
    VAR_MIRAGE_TOWER_STATE,
    FLAG_FORCE_MIRAGE_TOWER_VISIBLE,
    FLAG_MIRAGE_TOWER_VISIBLE,
));
/// `tv.c`'s daily wild battles (`SpecialIds::var_daily_wilds`).
pub const DAILY_WILDS: VarId = VAR_DAILY_WILDS;
/// `tv.c`'s daily berry counters.
pub const DAILY_PICKED_BERRIES: Option<VarId> = Some(VAR_DAILY_PICKED_BERRIES);
pub const DAILY_PLANTED_BERRIES: Option<VarId> = Some(VAR_DAILY_PLANTED_BERRIES);
pub const HAS_MATCH_CALL: Option<FlagId> = Some(FLAG_HAS_MATCH_CALL);
pub const PACIFIDLOG_TM_DAY: Option<VarId> = Some(VAR_PACIFIDLOG_TM_RECEIVED_DAY);
pub const RUSTURF: Option<Rusturf> = Some(Rusturf {
    opened: FLAG_RUSTURF_TUNNEL_OPENED,
    map: MAP_RUSTURF_TUNNEL,
    rocks: [
        (FLAG_HIDE_RUSTURF_TUNNEL_ROCK_1, 4),
        (FLAG_HIDE_RUSTURF_TUNNEL_ROCK_2, 5),
    ],
    state: VAR_RUSTURF_TUNNEL_STATE,
});
/// `GameClear`'s flag and CONTINUE warps.
pub const GAME_CLEAR: Option<GameClear> = Some(GameClear {
    flag: FLAG_SYS_GAME_CLEAR,
    rooms: [
        HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F,
        HEAL_LOCATION_LITTLEROOT_TOWN_MAYS_HOUSE_2F,
    ],
});

/// `ChooseHalfPartyForBattle` sets it to `FACILITY_MULTI_OR_EREADER`.
pub const FRONTIER_FACILITY: Option<VarId> = Some(VAR_FRONTIER_FACILITY);

/// `DoSpecialTrainerBattle`'s `SPECIAL_BATTLE_STEVEN` foes (`BattleSetup_ConfigureTrainerBattle`
/// of `MossdeepCity_SpaceCenter_2F_EventScript_MaxieTrainer` and `_TabithaTrainer`): the
/// trainer and its defeat text's label, Maxie first. The ids are `include/constants/opponents.h`'s
/// `TRAINER_MAXIE_MOSSDEEP` and `TRAINER_TABITHA_MOSSDEEP` (the stub build has no trainer
/// constants).
pub const STEVEN_MULTI_FOES: Option<[(engine::data::TrainerId, &str); 2]> = Some([
    (
        engine::data::TrainerId(734),
        "MossdeepCity_SpaceCenter_2F_Text_JustWantToExpandLand",
    ),
    (
        engine::data::TrainerId(514),
        "MossdeepCity_SpaceCenter_Text_TabithaDefeat",
    ),
]);

/// The fly map's destinations (Gen3 `region_map.c`): the town and city sections
/// `MAPSEC_LITTLEROOT_TOWN` (0) to `MAPSEC_EVER_GRANDE_CITY` (15) in order, each listed once its
/// `FLAG_VISITED_*` is set (`GetMapsecType`), landing on its `sMapHealLocations` heal location
/// (`SetFlyDestination` → `SetWarpDestinationToHealLocation`). Not yet: Littleroot's house by
/// the player's gender (Brendan's here), Ever Grande's League entrance once
/// `FLAG_LANDMARK_POKEMON_LEAGUE` is set, Southern Island and the Battle Frontier.
pub const FLY_DESTINATIONS: [(FlagId, u8); 16] = [
    (
        FLAG_VISITED_LITTLEROOT_TOWN,
        HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE,
    ),
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
    (
        FLAG_VISITED_EVER_GRANDE_CITY,
        HEAL_LOCATION_EVER_GRANDE_CITY,
    ),
];
