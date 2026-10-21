//! The new-game state: content data, run once by `World::new` before the first map loads.
//!
//! The slice starts where Littleroot, Route 101 and Oldale are freely walkable: a male player
//! (Brendan, named SACHA; the rival is May) who has saved Birch on Route 101, received his
//! starter in the lab and been told to meet May on Route 103. The vars and flags are what the
//! decomp scripts leave behind on that path:
//!
//! - `InsideOfTruck_EventScript_SetIntroFlagsMale` (male intro flags),
//! - `LittlerootTown_BrendansHouse_*` / `LittlerootTown_MaysHouse_*` (intro, meeting May),
//! - `Route101_EventScript_*` (the rescue: `VAR_ROUTE101_STATE` 3, `FLAG_RESCUED_BIRCH`),
//! - `LittlerootTown_ProfessorBirchsLab_EventScript_*` (`VAR_BIRCH_LAB_STATE` 3, starter),
//! - `data/scripts/new_game.inc` `ResetAllMapFlags` (objects hidden from the start).
//!
//! Choices:
//! - `FLAG_SYS_B_DASH` stays clear: the running shoes come later (Littleroot state 4).
//! - `FLAG_ADVENTURE_STARTED` stays clear, as in Gen3 at this point: Oldale's west exit is
//!   still blocked by the sketching man and the mart employee gives his Potion tour.
//! - Birch is back in his lab (`FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH` clear), May is on
//!   Route 103 (every other May / rival object hidden).
//! - There is no party yet (battles are a later milestone): only the flags are set.
//! - Start: outside Brendan's house, facing down (`data::NEW_GAME`).

use crate::prelude::*;

/// Flags set by the story so far.
const SET: &[FlagId] = &[
    FLAG_SYS_POKEMON_GET,
    FLAG_RESCUED_BIRCH,
    // Male intro (`InsideOfTruck_EventScript_SetIntroFlagsMale`).
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_MOM,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_TRUCK,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_RIVAL_MOM,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_RIVAL_SIBLING,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F_POKE_BALL,
    // The truck left, Mom is inside, the clock is set, May has been met.
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_TRUCK,
    FLAG_HIDE_LITTLEROOT_TOWN_MOM_OUTSIDE,
    FLAG_HIDE_LITTLEROOT_TOWN_PLAYERS_BEDROOM_MOM,
    FLAG_HIDE_LITTLEROOT_TOWN_PLAYERS_HOUSE_VIGOROTH_1,
    FLAG_HIDE_LITTLEROOT_TOWN_PLAYERS_HOUSE_VIGOROTH_2,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_BRENDAN,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_RIVAL_BEDROOM,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_MAY,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_2F_POKE_BALL,
    // `ResetAllMapFlags`.
    FLAG_HIDE_PLAYERS_HOUSE_DAD,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_2F_PICHU_DOLL,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F_SWABLU_DOLL,
    FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN,
    // Rescue and starter: Birch and May left Route 101 / the town, the lab balls are taken.
    FLAG_HIDE_LITTLEROOT_TOWN_RIVAL,
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCH,
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_RIVAL,
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_POKEBALL_CYNDAQUIL,
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_POKEBALL_TOTODILE,
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_POKEBALL_CHIKORITA,
    FLAG_HIDE_ROUTE_101_BIRCH,
    FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE,
    FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG,
    FLAG_HIDE_ROUTE_103_BIRCH,
    FLAG_HIDE_OLDALE_TOWN_RIVAL,
];

/// Vars set by the story so far (see the state lists in the decomp map scripts).
const VARS: &[(VarId, u16)] = &[
    (VAR_LITTLEROOT_INTRO_STATE, 7),          // told to go meet the rival
    (VAR_LITTLEROOT_TOWN_STATE, 2),           // saving Birch (until the Pokédex)
    (VAR_LITTLEROOT_HOUSES_STATE_BRENDAN, 2), // met the rival's mom
    (VAR_LITTLEROOT_RIVAL_STATE, 3),          // met the rival
    (VAR_BIRCH_LAB_STATE, 3),                 // received the starter
    (VAR_ROUTE101_STATE, 3),                  // Birch rescued
];

/// The new-game script (`NewGame::script`).
pub async fn new_game(ctx: Ctx) {
    for &f in SET {
        ctx.set_flag(f, true).await;
    }
    for &(v, x) in VARS {
        ctx.set_var(v, x).await;
    }
}
