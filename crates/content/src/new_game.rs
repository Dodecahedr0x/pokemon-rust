//! The new-game state: content data, run once by `World::new` before the first map loads. A
//! male player (Brendan, named SACHA; the rival is May), at one of two points of the story
//! ([`Start`]). The vars and flags are what the decomp scripts leave behind on that path:
//!
//! - `InsideOfTruck_EventScript_SetIntroFlagsMale` (male intro flags),
//! - `LittlerootTown_BrendansHouse_*` / `LittlerootTown_MaysHouse_*` (intro, meeting May),
//! - `Route101_EventScript_*` (the rescue: `VAR_ROUTE101_STATE` 3, `FLAG_RESCUED_BIRCH`),
//! - `LittlerootTown_ProfessorBirchsLab_EventScript_*` (`VAR_BIRCH_LAB_STATE` 3, starter),
//! - `data/scripts/new_game.inc` `EventScript_ResetAllMapFlags` (objects hidden from the start).
//!
//! [`Start::BeforeRescue`] (`Emerald::default()`, and `Emerald::for_shells()` on the extracted
//! world): May has been met in her bedroom and the player has left the houses
//! (`VAR_LITTLEROOT_TOWN_STATE` 1: go and find Birch; `VAR_LITTLEROOT_INTRO_STATE` 7,
//! `VAR_LITTLEROOT_RIVAL_STATE` 3). Nothing of Route 101 has happened (`VAR_ROUTE101_STATE` 0,
//! `VAR_BIRCH_LAB_STATE` 0): Birch, the Zigzagoon and the bag are on Route 101
//! (`FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE` / `FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG` /
//! `FLAG_HIDE_ROUTE_101_ZIGZAGOON` clear), Birch is not in his lab
//! (`FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH` set, as `ResetAllMapFlags` leaves it) and the
//! Route 101 boy is hidden (`FLAG_HIDE_ROUTE_101_BOY`, cleared by the lab later). May is back
//! in her bedroom (`FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM` clear) until the rescue.
//! No party, no `FLAG_SYS_POKEMON_GET`.
//!
//! [`Start::AfterStarter`] (the Milestone 3 start, `Emerald::after_starter()`): Birch saved, the
//! starter received in the lab (Mudkip, level 5, `VAR_STARTER_MON` 2), told to meet May on
//! Route 103.
//!
//! Both:
//! - `FLAG_SYS_B_DASH` stays clear: the running shoes come later (Littleroot state 4).
//! - `FLAG_ADVENTURE_STARTED` stays clear, as in Gen3 at this point: Oldale's west exit is
//!   still blocked by the sketching man and the mart employee gives his Potion tour.
//! - Start: outside Brendan's house, facing down (`data::NEW_GAME`).

use crate::Start;
use crate::prelude::*;

/// Flags set when the starter has been received.
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
    // `Route101_EventScript_StartBirchRescue`: `removeobject LOCALID_ROUTE101_ZIGZAGOON`.
    FLAG_HIDE_ROUTE_101_ZIGZAGOON,
    FLAG_HIDE_ROUTE_103_BIRCH,
    FLAG_HIDE_OLDALE_TOWN_RIVAL,
];

/// Flags of [`SET`] that are still clear before the rescue.
const CLEAR_BEFORE_RESCUE: &[FlagId] = &[
    FLAG_SYS_POKEMON_GET,
    FLAG_RESCUED_BIRCH,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
    FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE,
    FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG,
    FLAG_HIDE_ROUTE_101_ZIGZAGOON,
];

/// Flags set before the rescue that the rescue and the lab clear (`ResetAllMapFlags`).
const SET_BEFORE_RESCUE: &[FlagId] = &[
    FLAG_HIDE_LITTLEROOT_TOWN_BIRCHS_LAB_BIRCH,
    FLAG_HIDE_ROUTE_101_BOY,
];

/// Vars before the rescue.
const VARS_BEFORE_RESCUE: &[(VarId, u16)] = &[
    (VAR_LITTLEROOT_INTRO_STATE, 7),          // told to go meet the rival
    (VAR_LITTLEROOT_TOWN_STATE, 1),           // rival met: go and find Birch
    (VAR_LITTLEROOT_HOUSES_STATE_BRENDAN, 2), // met the rival's mom
    (VAR_LITTLEROOT_RIVAL_STATE, 3),          // met the rival
    (VAR_BIRCH_LAB_STATE, 0),
    (VAR_ROUTE101_STATE, 0),
];

/// Vars when the starter has been received (see the state lists in the decomp map scripts).
const VARS: &[(VarId, u16)] = &[
    (VAR_LITTLEROOT_INTRO_STATE, 7),          // told to go meet the rival
    (VAR_LITTLEROOT_TOWN_STATE, 2),           // saving Birch (until the Pokédex)
    (VAR_LITTLEROOT_HOUSES_STATE_BRENDAN, 2), // met the rival's mom
    (VAR_LITTLEROOT_RIVAL_STATE, 3),          // met the rival
    (VAR_BIRCH_LAB_STATE, 3),                 // received the starter
    (VAR_ROUTE101_STATE, 3),                  // Birch rescued
];

/// Mudkip, as `sStarterMon` index (`VAR_STARTER_MON`).
const STARTER: u16 = 2;

/// The new-game script (`NewGame::script`).
pub async fn new_game(ctx: Ctx, start: Start) {
    match start {
        Start::BeforeRescue => {
            for &f in SET.iter().filter(|f| !CLEAR_BEFORE_RESCUE.contains(f)) {
                ctx.set_flag(f, true).await;
            }
            for &f in SET_BEFORE_RESCUE {
                ctx.set_flag(f, true).await;
            }
            for &(v, x) in VARS_BEFORE_RESCUE {
                ctx.set_var(v, x).await;
            }
        }
        Start::AfterStarter => {
            for &f in SET {
                ctx.set_flag(f, true).await;
            }
            for &(v, x) in VARS {
                ctx.set_var(v, x).await;
            }
            // `CB2_GiveStarter`.
            ctx.set_var(VAR_STARTER_MON, STARTER).await;
            ctx.give_mon(crate::battle::STARTERS[STARTER as usize], 5)
                .await;
        }
    }
}
