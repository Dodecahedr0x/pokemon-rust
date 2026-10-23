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
