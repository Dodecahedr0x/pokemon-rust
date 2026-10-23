//! The new-game state: content data, run once by `World::new` before the first map loads. A
//! male player (Brendan, named SACHA; the rival is May), at one of two points of the story
//! ([`Start`]). The vars and flags are what the decomp scripts leave behind on that path:
//!
//! - `InsideOfTruck_EventScript_SetIntroFlagsMale` (male intro flags),
//! - `LittlerootTown_BrendansHouse_*` / `LittlerootTown_MaysHouse_*` (intro, meeting May),
//! - `Route101_EventScript_*` (the rescue: `VAR_ROUTE101_STATE` 3, `FLAG_RESCUED_BIRCH`),
//! - `LittlerootTown_ProfessorBirchsLab_EventScript_*` (`VAR_BIRCH_LAB_STATE` 3, starter),
//! - `new_game.c` `NewGameInitData`: `EventScript_ResetAllMapFlags` (every object hidden from
//!   the start, `consts::RESET_ALL_MAP_FLAGS`, taken from the decomp script), 3000 money,
//!   `InitSeedotSizeRecord`/`InitLotadSizeRecord` and `NewGameInitPCItems` (a Potion in the PC).
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

/// Gen3 `DEFAULT_MAX_SIZE` (`InitSeedotSizeRecord`, `InitLotadSizeRecord`).
const DEFAULT_MAX_SIZE: u16 = 0x8000;

/// Flags the intro sets (male) on top of `ResetAllMapFlags`, up to meeting May.
const INTRO_SET: &[FlagId] = &[
    // `InsideOfTruck_EventScript_SetIntroFlagsMale`.
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_MOM,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_TRUCK,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_RIVAL_MOM,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_RIVAL_SIBLING,
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F_POKE_BALL,
    // `LittlerootTown_EventScript_*`: the truck leaves; the Vigoroth movers leave the house.
    FLAG_HIDE_LITTLEROOT_TOWN_BRENDANS_HOUSE_TRUCK,
    FLAG_HIDE_LITTLEROOT_TOWN_PLAYERS_HOUSE_VIGOROTH_1,
    FLAG_HIDE_LITTLEROOT_TOWN_PLAYERS_HOUSE_VIGOROTH_2,
    // `PlayersHouse_2F_EventScript_SetWallClock` (`players_house.inc`), `StartWallClock`
    // (`clock.c`).
    FLAG_SET_WALL_CLOCK,
    FLAG_SYS_CLOCK_SET,
    // `PlayersHouse_1F_EventScript_*` (the TV, `players_house.inc`), `tv.inc`.
    FLAG_SYS_TV_HOME,
    FLAG_SYS_TV_WATCH,
    // `LittlerootTown_MaysHouse_1F_EventScript_*` (meeting May's mom).
    FLAG_MET_RIVAL_MOM,
    // `SecretBase_EventScript_SetDecorationFlags`, on each 2F's transition.
    FLAG_DECORATION_1,
    FLAG_DECORATION_2,
    FLAG_DECORATION_3,
    FLAG_DECORATION_4,
    FLAG_DECORATION_5,
    FLAG_DECORATION_6,
    FLAG_DECORATION_7,
    FLAG_DECORATION_8,
    FLAG_DECORATION_9,
    FLAG_DECORATION_10,
    FLAG_DECORATION_11,
    FLAG_DECORATION_12,
    FLAG_DECORATION_13,
    FLAG_DECORATION_14,
    // May met in her bedroom: her Poké Ball goes.
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_2F_POKE_BALL,
];

/// `ResetAllMapFlags` flags the intro clears.
const INTRO_CLEAR: &[FlagId] = &[
    // `LittlerootTown_EventScript_*` (line "clearflag FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN"): the
    // town's fat man shows once the player is out of the house.
    FLAG_HIDE_LITTLEROOT_TOWN_FAT_MAN,
    // `LittlerootTown_MaysHouse_1F`/`_2F` scripts: May back in her bedroom.
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
];

/// Flags the rescue and the lab set (Route 101, `LittlerootTown_ProfessorBirchsLab_*`).
const STARTER_SET: &[FlagId] = &[
    FLAG_SYS_POKEMON_GET,
    FLAG_RESCUED_BIRCH,
    FLAG_HIDE_LITTLEROOT_TOWN_MAYS_HOUSE_RIVAL_BEDROOM,
    FLAG_HIDE_ROUTE_101_BIRCH_ZIGZAGOON_BATTLE,
    FLAG_HIDE_ROUTE_101_BIRCH_STARTERS_BAG,
    // `Route101_EventScript_StartBirchRescue`: `removeobject LOCALID_ROUTE101_ZIGZAGOON`.
    FLAG_HIDE_ROUTE_101_ZIGZAGOON,
];

/// `ResetAllMapFlags` flags the rescue and the lab clear: Birch back in his lab, the Route 101
/// boy out.
const STARTER_CLEAR: &[FlagId] = &[
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
    // `tv.c` counts the rescue's wild battle (`VarSet(VAR_DAILY_WILDS, … + 1)`).
    (VAR_DAILY_WILDS, 1),
];

/// Mudkip, as `sStarterMon` index (`VAR_STARTER_MON`).
const STARTER: u16 = 2;

/// `NewGameInitData`, the parts our engine keeps.
async fn init_data(ctx: &Ctx) {
    // `RunScriptImmediately(EventScript_ResetAllMapFlags)` (its berry trees aside).
    for &f in RESET_ALL_MAP_FLAGS {
        ctx.set_flag(f, true).await;
    }
    // `SetMoney(&gSaveBlock1Ptr->money, 3000)`.
    ctx.money_add(3000).await;
    ctx.set_var(VAR_SEEDOT_SIZE_RECORD, DEFAULT_MAX_SIZE).await;
    ctx.set_var(VAR_LOTAD_SIZE_RECORD, DEFAULT_MAX_SIZE).await;
    // `NewGameInitPCItems` (`sNewGamePCItems`, `player_pc.c`).
    ctx.pc_item_add(ItemId(ITEM_POTION), 1).await;
}

/// The new-game script (`NewGame::script`).
pub async fn new_game(ctx: Ctx, start: Start) {
    init_data(&ctx).await;
    for &f in INTRO_SET {
        ctx.set_flag(f, true).await;
    }
    for &f in INTRO_CLEAR {
        ctx.set_flag(f, false).await;
    }
    match start {
        Start::BeforeRescue => {
            for &(v, x) in VARS_BEFORE_RESCUE {
                ctx.set_var(v, x).await;
            }
        }
        Start::AfterStarter => {
            for &f in STARTER_SET {
                ctx.set_flag(f, true).await;
            }
            for &f in STARTER_CLEAR {
                ctx.set_flag(f, false).await;
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
