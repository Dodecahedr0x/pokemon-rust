//! `special` / `specialvar` (Gen3 `gSpecials`, Milestone 8 Task J).
//!
//! The script lowering classes every special in `build/support/specials.ron`: only the
//! `Native` ones call [`special`]; `Ignore` emits nothing, `Default(v)` writes `v` where the
//! special writes its result, and `Unsupported` reports. Every `Native` special has an arm
//! below (checked by `tests/m8_j_specials.rs`). Specials that Gen3 runs through a screen this
//! engine does not have yet (naming screen, bag, party menu, …) answer as if the player had
//! cancelled it, report `Report::Degraded`, and fade the field back in when the script faded
//! it out first, so the script never waits on a black screen.

use super::*;

/// Gen3 `PARTY_NOTHING_CHOSEN` (`include/constants/party_menu.h`).
const PARTY_NOTHING_CHOSEN: u16 = 0xFF;
/// Gen3 `PLAYER_HAS_TWO_USABLE_MONS` (`include/constants/pokemon.h`).
const PLAYER_HAS_TWO_USABLE_MONS: u16 = 0;
/// Gen3 `PLAYER_HAS_ONE_MON`.
const PLAYER_HAS_ONE_MON: u16 = 1;
/// Gen3 `BERRY_STAGE_NO_BERRY` (`include/constants/berry.h`).
const BERRY_STAGE_NO_BERRY: u16 = 0;
/// The PokéNav entry of the start menu (`ScriptMenu_CreateStartMenuForPokenavTutorial`: Pokédex,
/// Pokémon, Bag, PokéNav, …), the only answer Rustboro's tutorial accepts.
const START_MENU_POKENAV: u16 = 3;
/// Gen3 `CB2_HandleGivenWaldaPhrase`: no phrase given and none before.
const WALDA_CANCEL_FIRST_PHRASE: u16 = 2;

/// `crate::data::std_string` ids of the field strings (`FIELD_STRING_BASE` +
/// `extract::tables::FIELD_STRINGS` index).
const BIG_GUY: u8 = crate::data::FIELD_STRING_BASE;
const BIG_GIRL: u8 = crate::data::FIELD_STRING_BASE + 1;
const SON: u8 = crate::data::FIELD_STRING_BASE + 2;
const DAUGHTER: u8 = crate::data::FIELD_STRING_BASE + 3;
const MOM: u8 = crate::data::FIELD_STRING_BASE + 4;
const DAD: u8 = crate::data::FIELD_STRING_BASE + 5;
/// Gen3 `MALE` (`checkplayergender`).
const MALE: u16 = 0;
/// The male and the female player's house, ground floor (`GetMomOrDadStringForTVMessage`).
#[cfg(not(pkr_silice))]
const PLAYER_HOUSES_1F: [MapId; 2] = [
    MAP_LITTLEROOT_TOWN_BRENDANS_HOUSE_1F,
    MAP_LITTLEROOT_TOWN_MAYS_HOUSE_1F,
];
/// Silice has one player's house.
#[cfg(pkr_silice)]
const PLAYER_HOUSES_1F: [MapId; 2] = [MAP_BRICO_BOURG_PLAYER_HOUSE_1F; 2];

/// The cable car's stations, `[up, down]`: where `CableCarWarp` sends a ride going up
/// (`VAR_0x8004` = 0) or down.
#[cfg(not(pkr_silice))]
const CABLE_CAR_STATIONS: [MapId; 2] = [
    MAP_MT_CHIMNEY_CABLE_CAR_STATION,
    MAP_ROUTE112_CABLE_CAR_STATION,
];
/// Silice has no cable car: a ride warps nowhere (the host ignores a missing map).
#[cfg(pkr_silice)]
const CABLE_CAR_STATIONS: [MapId; 2] = [MapId(0xFFFF); 2];
/// Both stations' arrival cell (`CableCarWarp`, `WARP_ID_NONE`).
const CABLE_CAR_ARRIVAL: (i16, i16) = (6, 4);
/// The ride between the fade to black and the warp (`cable_car.c`): `Task_CableCar` fades out
/// at timer 570, and that fade (`BeginNormalPaletteFade(…, 3, 0, 16, …)`, 8 steps of 4 frames)
/// takes 32 more.
const CABLE_CAR_RIDE_FRAMES: u16 = 570 + 32;

/// Gen3 `sPetalburgGymSlidingDoorMetatiles` (`METATILE_PetalburgGym_SlidingDoor_Frame0..4`,
/// `include/constants/metatile_labels.h`): the door's top half; the bottom half is a metatile
/// row (`METATILE_ROW_WIDTH`, 8) further.
const PETALBURG_GYM_DOOR_FRAMES: [u16; 5] = [0x218, 0x219, 0x21A, 0x21B, 0x21C];
const METATILE_ROW_WIDTH: u16 = 8;
/// `Task_PetalburgGymSlideOpenRoomDoors`' `sSlidingDoorNextFrameDelay` {0, 1, 1, 1, 1}: the
/// first frame is drawn at once, each later one 2 frames after the previous.
const PETALBURG_GYM_DOOR_FRAME_GAP: u16 = 2;

/// `PetalburgGymSetDoorMetatiles`' doors of room `room` (`VAR_0x8004`, 1 to 8): the top cells.
fn petalburg_gym_doors(room: u16) -> &'static [(i16, i16)] {
    match room {
        1 => &[(1, 104), (7, 104)],
        2 => &[(1, 78), (7, 78)],
        3 => &[(1, 91), (7, 91)],
        4 => &[(7, 39)],
        5 => &[(1, 52), (7, 52)],
        6 => &[(1, 65)],
        7 => &[(7, 13)],
        8 => &[(1, 26)],
        _ => &[],
    }
}

/// Gen3 `PetalburgGymSetDoorMetatiles`: room `VAR_0x8004`'s doors (both halves, impassable)
/// show `metatile`, then `DrawWholeMapView`.
async fn petalburg_gym_set_doors(ctx: &Ctx, room: u16, metatile: u16) {
    for &(x, y) in petalburg_gym_doors(room) {
        ctx.set_metatile(x, y, metatile, true).await;
        ctx.set_metatile(x, y + 1, metatile + METATILE_ROW_WIDTH, true)
            .await;
    }
    ctx.redraw_map().await;
}

/// Gen3 `FRONTIER_UTIL_FUNC_*` (`include/constants/frontier_util.h`), `CallFrontierUtilFunc`'s
/// `VAR_0x8004`, by name (index = value).
const FRONTIER_UTIL_FUNCS: [&str; 23] = [
    "FRONTIER_UTIL_FUNC_GET_STATUS",
    "FRONTIER_UTIL_FUNC_GET_DATA",
    "FRONTIER_UTIL_FUNC_SET_DATA",
    "FRONTIER_UTIL_FUNC_SET_PARTY_ORDER",
    "FRONTIER_UTIL_FUNC_SOFT_RESET",
    "FRONTIER_UTIL_FUNC_SET_TRAINERS",
    "FRONTIER_UTIL_FUNC_SAVE_PARTY",
    "FRONTIER_UTIL_FUNC_RESULTS_WINDOW",
    "FRONTIER_UTIL_FUNC_CHECK_AIR_TV_SHOW",
    "FRONTIER_UTIL_FUNC_GET_BRAIN_STATUS",
    "FRONTIER_UTIL_FUNC_IS_BRAIN",
    "FRONTIER_UTIL_FUNC_GIVE_BATTLE_POINTS",
    "FRONTIER_UTIL_FUNC_GET_FACILITY_SYMBOLS",
    "FRONTIER_UTIL_FUNC_GIVE_FACILITY_SYMBOL",
    "FRONTIER_UTIL_FUNC_CHECK_BATTLE_TYPE",
    "FRONTIER_UTIL_FUNC_CHECK_INELIGIBLE",
    "FRONTIER_UTIL_FUNC_CHECK_VISIT_TRAINER",
    "FRONTIER_UTIL_FUNC_INCREMENT_STREAK",
    "FRONTIER_UTIL_FUNC_RESTORE_HELD_ITEMS",
    "FRONTIER_UTIL_FUNC_SAVE_BATTLE",
    "FRONTIER_UTIL_FUNC_BUFFER_TRAINER_NAME",
    "FRONTIER_UTIL_FUNC_RESET_SKETCH_MOVES",
    "FRONTIER_UTIL_FUNC_SET_BRAIN_OBJECT",
];
const FRONTIER_UTIL_FUNC_GET_STATUS: u16 = 0;
const FRONTIER_UTIL_FUNC_GET_DATA: u16 = 1;
const FRONTIER_UTIL_FUNC_SET_DATA: u16 = 2;
/// `GetChallengeStatus` with `frontier.challengeStatus` 0 (no challenge in progress).
const CHALLENGE_STATUS_NONE: u16 = 0xFF;

/// Gen3 `CallFrontierUtilFunc` (`frontier_util.c`), with no Battle Frontier or Battle Tent save
/// data yet (Milestone 13): no challenge ever starts, so `frontier.challengeStatus` is 0.
/// - `GET_STATUS` (`GetChallengeStatus`): `VAR_TEMP_CHALLENGE_STATUS` = 0xFF, exact: a lobby's
///   `ON_FRAME` table then has no match (it reran `frontier_getstatus` every frame while the
///   var stayed 0).
/// - `GET_DATA` (`GetFrontierData`): `VAR_RESULT` = 0, a new save's value (degraded);
///   `SET_DATA` stores nothing (degraded).
/// - The others report `Unsupported` with their `FRONTIER_UTIL_FUNC_*` name.
async fn call_frontier_util_func(ctx: &Ctx, id: u16) {
    match ctx.var(VAR_0x8004).await {
        FRONTIER_UTIL_FUNC_GET_STATUS => {
            ctx.set_var(VAR_TEMP_CHALLENGE_STATUS, CHALLENGE_STATUS_NONE)
                .await;
        }
        FRONTIER_UTIL_FUNC_GET_DATA => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_RESULT, 0).await;
        }
        FRONTIER_UTIL_FUNC_SET_DATA => degraded(ctx, id).await,
        f => {
            let name = FRONTIER_UTIL_FUNCS
                .get(usize::from(f))
                .copied()
                .unwrap_or("FRONTIER_UTIL_FUNC_?");
            ctx.report(Report::Unsupported(name)).await;
        }
    }
}

/// Reports `id` as run in a simplified way (its decomp name).
async fn degraded(ctx: &Ctx, id: u16) {
    let name = name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?");
    ctx.report(Report::Degraded(name)).await;
}

/// Gen3 `CableCarWarp`'s destination map: Mt. Chimney's station going up (`VAR_0x8004` = 0),
/// Route 112's going down.
async fn cable_car_destination(ctx: &Ctx) -> MapId {
    let [up, down] = CABLE_CAR_STATIONS;
    if ctx.var(VAR_0x8004).await != 0 {
        down
    } else {
        up
    }
}

/// `special <id>` / `specialvar <var>, <id>` for the `Native` specials: logs the special's name,
/// then runs it; the result (written by `specialvar`) is the Gen3 return value, 0 for a void
/// special. A special with no arm here is only logged and returns 0: the lowering calls this for
/// `Native` specials only, so only a hand-written call (an override, a test) gets there.
///
/// - `ChooseStarter`: Gen3 `ChooseStarter` + `CB2_GiveStarter` + `CB2_StartFirstBattle`: the
///   starter screen, `VAR_STARTER_MON` = the choice, the starter at level 5, then the first
///   battle (`battle_controllers.c`: Zigzagoon, level 2).
/// - `HealPlayerParty`: heals the party.
/// - `ChangePokemonNickname`, `ChangeBoxPokemonNickname`: no naming screen, the name is kept;
///   the field fades back in from the script's `fadescreen FADE_TO_BLACK` as
///   `CB2_ReturnToFieldContinueScriptPlayMapMusic` does.
/// - `StartWallClock`, `Special_ViewWallClock`: no clock screen (the time stays the default;
///   the RTC is Milestone 10); the field fades back in. `StartWallClock` first runs
///   `InitTimeBasedEvents` (`clock.c`): `FLAG_SYS_CLOCK_SET`, `VAR_DAYS` = 0 (no RTC).
/// - `DrawWholeMapView`: redraws the map.
/// - `InitBirchState`: `VAR_BIRCH_STATE` = 0.
/// - `GetPCBoxToSendMon`: `VAR_PC_BOX_TO_SEND_MON`. Degraded: Gen3 gives `sPCBoxToSendMon`, the
///   box that was full before `IsDestinationBoxFull` moved the var on; this gives the new box.
///   Unreachable while `ShouldShowBoxWasFullMessage` is `Default(0)`.
/// - `SetHiddenItemFlag`: sets flag `VAR_0x8004` (the hidden item's).
/// - `CalculatePlayerPartyCount`, `CountPartyNonEggMons`: the party size (no eggs yet).
/// - `HasEnoughMonsForDoubleBattle`: `VAR_RESULT` from the party size (degraded with 2 or more:
///   fainted mons count as usable).
/// - `GetPlayerBigGuyGirlString`, `GetRivalSonDaughterString` (`field_specials.c`):
///   `STR_VAR_1` = `gText_BigGuy` / `gText_Daughter` for a male player, else `gText_BigGirl` /
///   `gText_Son` (the extract's field strings; an older extract buffers nothing and reports).
/// - `GetMomOrDadStringForTVMessage` (`tv.c`): in the player's own house (Brendan's 1F for a
///   male player, May's else) Mom, `VAR_TEMP_3` = 1, with no draw; else `VAR_TEMP_3` picks
///   (1 Mom, 2 Dad, above 2 by parity), and when unset Mom or Dad at random, kept in
///   `VAR_TEMP_3`; `STR_VAR_1` gets `gText_Mom` or `gText_Dad`.
/// - `CableCarWarp` (`field_specials.c`): Gen3 sets `gWarpDestination` to the other station's
///   (6, 4) by `VAR_0x8004` (0: up to Mt. Chimney); only the ride's end reads it, so this
///   reads the var and `CableCar` warps there.
/// - `CableCar` (`cable_car.c`), degraded: no ride scene. The field fades to black as Gen3's
///   `CableCar` does, stays black for the ride's frames, then warps to `CableCarWarp`'s
///   destination (`CB2_EndCableCar`'s `WarpIntoMap`); the warp's map load ends the script, as
///   `CB2_LoadMap` does in Gen3 (the script's `waitstate` is never reached).
/// - `PetalburgGymUnlockRoomDoors`, `PetalburgGymSlideOpenRoomDoors` (`field_specials.c`): room
///   `VAR_0x8004`'s doors show the open frame at once, or slide through the five frames (2
///   frames apart, `Task_PetalburgGymSlideOpenRoomDoors`; the script's `waitstate` then
///   passes). The unlock sound is not modelled.
/// - `CallFrontierUtilFunc`: see [`call_frontier_util_func`].
/// - Degraded (reported), as if cancelled:
///   - `StartWallyTutorialBattle`: no battle; the script goes on as after Wally's catch;
///   - `ChoosePartyMon`: `VAR_0x8004` = `PARTY_NOTHING_CHOSEN`;
///   - `DoWaldaNamingScreen`: `VAR_0x8004` = cancelled with no phrase before, so no phrase is
///     saved (and `TryGetWallpaperWithWaldaPhrase` is never reached);
///   - `ShowEasyChatScreen`: `VAR_RESULT` = 0;
///   - `Bag_ChooseBerry`: `VAR_ITEM_ID` = 0, then the field fades back in;
///   - `FieldShowRegionMap`: the field fades back in;
///   - `ObjectEventInteractionGetBerryTreeData`: an empty soil patch (no berry trees yet);
///   - `ScriptMenu_CreatePCMultichoice`: `VAR_RESULT` = `MULTI_B_PRESSED` (the PC turns off);
///   - `ScriptMenu_CreateStartMenuForPokenavTutorial`: the PokéNav entry.
pub async fn special(ctx: &Ctx, id: u16) -> u16 {
    ctx.log(name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?"))
        .await;
    // Guards, not patterns: a mistyped constant would silently become a catch-all binding.
    match id {
        id if id == SPECIAL_ChooseStarter => {
            let starters = crate::battle::STARTERS;
            let i = ctx.choose_starter(starters).await;
            debug_assert!(i < 3, "starter index {i}");
            // `CB2_GiveStarter` stores the raw choice; `GetStarterPokemon` maps an out-of-range
            // one to the first starter.
            ctx.set_var(VAR_STARTER_MON, i.into()).await;
            let species = starters.get(usize::from(i)).unwrap_or(&starters[0]);
            ctx.give_mon(*species, 5).await;
            let (foe, level) = crate::battle::FIRST_BATTLE_FOE;
            ctx.battle_wild(foe, level, true).await;
        }
        id if id == SPECIAL_HealPlayerParty => ctx.heal_party().await,
        id if id == SPECIAL_StartWallClock => {
            // `InitTimeBasedEvents` (`clock.c`); `VAR_DAYS` = the RTC's days, 0 until M10.
            ctx.set_flag(FLAG_SYS_CLOCK_SET, true).await;
            ctx.set_var(VAR_DAYS, 0).await;
            ctx.fade(Fade::FromBlack).await
        }
        id if id == SPECIAL_ChangePokemonNickname
            || id == SPECIAL_ChangeBoxPokemonNickname
            || id == SPECIAL_Special_ViewWallClock =>
        {
            ctx.fade(Fade::FromBlack).await
        }
        id if id == SPECIAL_DrawWholeMapView => ctx.redraw_map().await,
        id if id == SPECIAL_InitBirchState => ctx.set_var(VAR_BIRCH_STATE, 0).await,
        id if id == SPECIAL_GetPCBoxToSendMon => return ctx.var(VAR_PC_BOX_TO_SEND_MON).await,
        id if id == SPECIAL_CalculatePlayerPartyCount || id == SPECIAL_CountPartyNonEggMons => {
            return ctx.party_size().await;
        }
        id if id == SPECIAL_HasEnoughMonsForDoubleBattle => {
            let r = if ctx.party_size().await >= 2 {
                degraded(ctx, id).await;
                PLAYER_HAS_TWO_USABLE_MONS
            } else {
                PLAYER_HAS_ONE_MON
            };
            ctx.set_var(VAR_RESULT, r).await;
        }
        id if id == SPECIAL_SetHiddenItemFlag => {
            let flag = ctx.var(VAR_0x8004).await;
            ctx.set_flag(FlagId(flag), true).await;
        }
        id if id == SPECIAL_GetPlayerBigGuyGirlString => {
            let male = check_player_gender(ctx).await == MALE;
            let s = if male { BIG_GUY } else { BIG_GIRL };
            ctx.buffer(0, BufferSrc::Std(s)).await;
        }
        id if id == SPECIAL_GetRivalSonDaughterString => {
            let male = check_player_gender(ctx).await == MALE;
            let s = if male { DAUGHTER } else { SON };
            ctx.buffer(0, BufferSrc::Std(s)).await;
        }
        id if id == SPECIAL_StartWallyTutorialBattle => degraded(ctx, id).await,
        id if id == SPECIAL_PetalburgGymUnlockRoomDoors => {
            let room = ctx.var(VAR_0x8004).await;
            petalburg_gym_set_doors(ctx, room, PETALBURG_GYM_DOOR_FRAMES[4]).await;
        }
        id if id == SPECIAL_PetalburgGymSlideOpenRoomDoors => {
            let room = ctx.var(VAR_0x8004).await;
            for (i, &frame) in PETALBURG_GYM_DOOR_FRAMES.iter().enumerate() {
                if i > 0 {
                    ctx.delay(PETALBURG_GYM_DOOR_FRAME_GAP).await;
                }
                petalburg_gym_set_doors(ctx, room, frame).await;
            }
        }
        id if id == SPECIAL_CallFrontierUtilFunc => call_frontier_util_func(ctx, id).await,
        id if id == SPECIAL_CableCarWarp => {
            cable_car_destination(ctx).await;
        }
        id if id == SPECIAL_CableCar => {
            degraded(ctx, id).await;
            let map = cable_car_destination(ctx).await;
            ctx.fade(Fade::ToBlack).await;
            ctx.delay(CABLE_CAR_RIDE_FRAMES).await;
            let (x, y) = CABLE_CAR_ARRIVAL;
            ctx.warp(map, None, x, y).await;
        }
        id if id == SPECIAL_GetMomOrDadStringForTVMessage => {
            let [male_house, female_house] = PLAYER_HOUSES_1F;
            let house = if check_player_gender(ctx).await == MALE {
                male_house
            } else {
                female_house
            };
            if ctx.current_map().await == house {
                ctx.set_var(VAR_TEMP_3, 1).await;
            }
            let mom = match ctx.var(VAR_TEMP_3).await {
                0 => {
                    let mom = ctx.random(2).await != 0;
                    ctx.set_var(VAR_TEMP_3, if mom { 1 } else { 2 }).await;
                    mom
                }
                1 => true,
                2 => false,
                // Above 2: another use of the var; Gen3 picks by parity.
                t3 => t3 % 2 == 0,
            };
            ctx.buffer(0, BufferSrc::Std(if mom { MOM } else { DAD }))
                .await;
        }
        id if id == SPECIAL_ChoosePartyMon => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_0x8004, PARTY_NOTHING_CHOSEN).await;
        }
        id if id == SPECIAL_DoWaldaNamingScreen => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_0x8004, WALDA_CANCEL_FIRST_PHRASE).await;
        }
        id if id == SPECIAL_ShowEasyChatScreen => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_RESULT, 0).await;
        }
        id if id == SPECIAL_Bag_ChooseBerry => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_ITEM_ID, 0).await;
            ctx.fade(Fade::FromBlack).await;
        }
        id if id == SPECIAL_FieldShowRegionMap => {
            degraded(ctx, id).await;
            ctx.fade(Fade::FromBlack).await;
        }
        id if id == SPECIAL_ObjectEventInteractionGetBerryTreeData => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_0x8004, BERRY_STAGE_NO_BERRY).await;
            ctx.set_var(VAR_0x8005, 0).await;
            ctx.set_var(VAR_0x8006, 0).await;
        }
        id if id == SPECIAL_ScriptMenu_CreatePCMultichoice => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_RESULT, crate::MULTI_B_PRESSED).await;
        }
        id if id == SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_RESULT, START_MENU_POKENAV).await;
        }
        // ---- Wave C: specials that read the player's state (`Request::Query`) ----
        id if id == SPECIAL_ScriptGetPokedexInfo => {
            // `birch_pc.c`: the Hoenn counts for VAR_0x8004 0, else the National ones.
            let among: &'static [SpeciesId] = if ctx.var(VAR_0x8004).await == 0 {
                &crate::battle::HOENN_DEX
            } else {
                &[]
            };
            let seen = dex_count(ctx, false, among).await;
            let caught = dex_count(ctx, true, among).await;
            ctx.set_var(VAR_0x8005, seen).await;
            ctx.set_var(VAR_0x8006, caught).await;
            return national_dex_enabled(ctx).await as u16;
        }
        id if id == SPECIAL_HasAllHoennMons => return has_all_hoenn_mons(ctx).await as u16,
        id if id == SPECIAL_ShowPokedexRatingMessage => {
            // `ShowPokedexRatingMessage` (`birch_pc.c`): `ShowFieldMessage`, no wait.
            let count = ctx.var(VAR_0x8004).await;
            let name = pokedex_rating_text(ctx, count).await;
            show_text_named(ctx, id, name).await;
        }
        id if id == SPECIAL_EnableNationalPokedex => {
            // `event_data.c`; `nationalMagic` goes with the var and the flag, the Pokédex
            // screen's mode and order are not kept.
            ctx.set_var(VAR_NATIONAL_DEX, NATIONAL_DEX_MAGIC_VAR).await;
            ctx.set_flag(FLAG_SYS_NATIONAL_DEX, true).await;
        }
        id if id == SPECIAL_CountPlayerTrainerStars => {
            // `trainer_card.c`: `GAME_STAT_ENTERED_HOF` (set with `FLAG_SYS_GAME_CLEAR` by
            // `GameClear`), the Hoenn dex, the four museum paintings (no contests: 0), and
            // every frontier symbol.
            let mut stars = ctx.flag(FLAG_SYS_GAME_CLEAR).await as u16;
            stars += has_all_hoenn_mons(ctx).await as u16;
            return stars + has_all_frontier_symbols(ctx).await as u16;
        }
        id if id == SPECIAL_ShouldTryRematchBattle || id == SPECIAL_IsTrainerReadyForRematch => {
            // Lowered to `special_vs` after a `trainerbattle`; here the opponent is unknown.
            degraded(ctx, id).await;
        }
        id if id == SPECIAL_IsTrainerRegistered => {
            // `field_specials.c`: `GetRematchIdxByTrainerIdx(VAR_0x8004)`.
            let trainer = ctx.var(VAR_0x8004).await;
            if let Some(i) = rematch_index(trainer) {
                return ctx.flag(registered_flag(i)).await as u16;
            }
        }
        id if id == SPECIAL_GetPlayerFacingDirection => {
            // Gen3 `DIR_SOUTH` 1, `DIR_NORTH` 2, `DIR_WEST` 3, `DIR_EAST` 4.
            return match ctx.player_facing().await {
                Dir::Down => 1,
                Dir::Up => 2,
                Dir::Left => 3,
                Dir::Right => 4,
            };
        }
        id if id == SPECIAL_StorePlayerCoordsInVars => {
            let (x, y) = ctx.player_pos().await;
            ctx.set_var(VAR_0x8004, x as u16).await;
            ctx.set_var(VAR_0x8005, y as u16).await;
        }
        id if id == SPECIAL_GetPlayerTrainerIdOnesDigit => {
            return ctx.query(Query::PlayerIdLow).await % 10;
        }
        id if id == SPECIAL_IsStarterInParty => {
            // `GetStarterPokemon`: an out-of-range choice is the first starter.
            let starters = crate::battle::STARTERS;
            let v = usize::from(ctx.var(VAR_STARTER_MON).await);
            let starter = starters.get(v).unwrap_or(&starters[0]).0;
            let n = ctx.party_size().await;
            for slot in 0..n as u8 {
                if ctx.query(Query::PartySpecies(slot)).await == starter {
                    return 1;
                }
            }
            return 0;
        }
        id if id == SPECIAL_TryUpdateRusturfTunnelState => {
            let Some(t) = story::RUSTURF else {
                return not_in_game(ctx, id).await;
            };
            if !ctx.flag(t.opened).await && ctx.current_map().await == t.map {
                for (flag, state) in t.rocks {
                    if ctx.flag(flag).await {
                        ctx.set_var(t.state, state).await;
                        return 1;
                    }
                }
            }
            return 0;
        }
        id if id == SPECIAL_FoundBlackGlasses => {
            let Some(flag) = story::BLACK_GLASSES else {
                return not_in_game(ctx, id).await;
            };
            return ctx.flag(flag).await as u16;
        }
        id if id == SPECIAL_FoundAbandonedShipRoom1Key
            || id == SPECIAL_FoundAbandonedShipRoom2Key
            || id == SPECIAL_FoundAbandonedShipRoom4Key
            || id == SPECIAL_FoundAbandonedShipRoom6Key =>
        {
            let room = match id {
                id if id == SPECIAL_FoundAbandonedShipRoom1Key => 0,
                id if id == SPECIAL_FoundAbandonedShipRoom2Key => 1,
                id if id == SPECIAL_FoundAbandonedShipRoom4Key => 2,
                _ => 3,
            };
            let Some(flag) = story::SHIP_KEYS[room] else {
                return not_in_game(ctx, id).await;
            };
            ctx.set_var(VAR_0x8004, flag.0).await;
            return ctx.flag(flag).await as u16;
        }
        id if id == SPECIAL_PlayerHasBerries => {
            return ctx
                .query(Query::PocketNonEmpty(engine::data::Pocket::Berries))
                .await;
        }
        id if id == SPECIAL_Special_AreLeadMonEVsMaxedOut => {
            let lead = lead_mon_index(ctx).await;
            return (ctx.query(Query::PartyEvTotal(lead)).await >= MAX_TOTAL_EVS) as u16;
        }
        id if id == SPECIAL_GetLeadMonFriendshipScore => {
            // `FRIENDSHIP_NONE` 0 … `FRIENDSHIP_MAX` 6.
            let lead = lead_mon_index(ctx).await;
            let f = ctx.query(Query::PartyFriendship(lead)).await;
            return match f {
                255 => 6,
                200.. => 5,
                150.. => 4,
                100.. => 3,
                50.. => 2,
                1.. => 1,
                0 => 0,
            };
        }
        id if id == SPECIAL_GetBattleOutcome => {
            // Lowered to the function's `dowildbattle` outcome; here no battle of this script
            // ran (the battle specials are unsupported).
            degraded(ctx, id).await;
        }
        id if id == SPECIAL_ScriptGetPartyMonSpecies => {
            let slot = ctx.var(VAR_0x8004).await;
            return ctx.query(Query::PartySpecies(slot as u8)).await;
        }
        id if id == SPECIAL_IsMirageIslandPresent => {
            // `time_events.c`: `GetMirageRnd() >> 16` is the high mirage var.
            let Some(var) = story::MIRAGE_RND_H else {
                return not_in_game(ctx, id).await;
            };
            let rnd = ctx.var(var).await;
            for slot in 0..PARTY_SIZE as u8 {
                if ctx.query(Query::PartySpecies(slot)).await != 0
                    && ctx.query(Query::PartyPersonalityLow(slot)).await == rnd
                {
                    return 1;
                }
            }
            return 0;
        }
        id if id == SPECIAL_CheckRelicanthWailord => {
            // `braille_puzzles.c`: Wailord first, Relicanth last.
            if ctx.query(Query::PartySpecies(0)).await == mons::WAILORD.0 {
                let last = ctx.party_size().await.saturating_sub(1) as u8;
                return (ctx.query(Query::PartySpecies(last)).await == mons::RELICANTH.0) as u16;
            }
            return 0;
        }
        id if id == SPECIAL_CountPartyAliveNonEggMons => {
            // `egg_hatch.c`: the PC's non-egg mons plus the party's alive ones; the PC's are
            // not queried.
            degraded(ctx, id).await;
            return party_alive_except(ctx, PARTY_SIZE).await;
        }
        id if id == SPECIAL_CountPartyAliveNonEggMons_IgnoreVar0x8004Slot => {
            let except = ctx.var(VAR_0x8004).await;
            return party_alive_except(ctx, except).await;
        }
        id if id == SPECIAL_GetSelectedMonNicknameAndSpecies => {
            // `daycare.c`: the party menu's selection (`GetCursorSelectionMonId`), which
            // `ChoosePartyMon` leaves in VAR_0x8004.
            let slot = ctx.var(VAR_0x8004).await as u8;
            ctx.buffer(0, BufferSrc::PartyNick(slot)).await;
            return ctx.query(Query::PartySpecies(slot)).await;
        }
        id if id == SPECIAL_IsEnoughForCostInVar0x8005 => {
            let cost = ctx.var(VAR_0x8005).await;
            return ctx.money_check(cost.into()).await as u16;
        }
        id if id == SPECIAL_GetDaysUntilPacifidlogTMAvailable => {
            // `field_specials.c` on day 0 (`gLocalTime.days`; no RTC, the clock is M10).
            let Some(var) = story::PACIFIDLOG_TM_DAY else {
                return not_in_game(ctx, id).await;
            };
            let received = i32::from(ctx.var(var).await);
            let elapsed = RTC_DAYS - received;
            return if elapsed >= 7 {
                0
            } else {
                (7 - elapsed) as u16
            };
        }
        id if id == SPECIAL_GiveLeadMonEffortRibbon => {
            // `GAME_STAT_RECEIVED_RIBBONS` and the ribbon itself are not kept.
            degraded(ctx, id).await;
            ctx.set_flag(FLAG_SYS_RIBBON_GET, true).await;
        }
        // ---- Wave C: Mauville's gym (`field_specials.c`) ----
        id if id == SPECIAL_MauvilleGymPressSwitch => {
            // The switch of `VAR_0x8004` pressed, the others raised.
            let pressed = ctx.var(VAR_0x8004).await;
            for (i, &(x, y)) in mauville_gym::SWITCHES.iter().enumerate() {
                let m = if i as u16 == pressed {
                    mauville_gym::PRESSED_SWITCH
                } else {
                    mauville_gym::RAISED_SWITCH
                };
                ctx.set_metatile(x, y, m, false).await;
            }
        }
        id if id == SPECIAL_MauvilleGymSetDefaultBarriers => {
            mauville_gym_scan(ctx, mauville_gym::toggle).await;
        }
        id if id == SPECIAL_MauvilleGymDeactivatePuzzle => {
            for &(x, y) in &mauville_gym::SWITCHES {
                ctx.set_metatile(x, y, mauville_gym::PRESSED_SWITCH, false)
                    .await;
            }
            mauville_gym_scan(ctx, mauville_gym::deactivate).await;
        }
        // Not a `Native` special: the lowering never passes one (it reports the `Unsupported`
        // ones itself); a hand-written call is only logged.
        _ => {}
    }
    0
}

/// `special_vs` (Wave C): the specials of `OPPONENT_SPECIALS` (extract's lowering) after a
/// `trainerbattle` against `trainer` (Gen3 `gTrainerBattleOpponent_A`), `battle_setup.c`:
/// - `ShouldTryRematchBattle`: `IsFirstTrainerIdReadyForRematch` (its `trainerRematches`
///   entry) or `WasSecondRematchWon` (the trainer of its second stage beaten);
/// - `IsTrainerReadyForRematch`: `trainerRematches` of its rematch entry (any stage).
///
/// `trainerRematches` is never set: Gen3 sets it from five badges on, by the rematch step
/// counter (`TryUpdateRandomTrainerRematches`) and Match Call, which the engine has not. Past
/// five badges, for a registered trainer, the answer is reported `Degraded`.
pub async fn special_vs(ctx: &Ctx, id: u16, trainer: TrainerId) -> u16 {
    ctx.log(name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?"))
        .await;
    let first = id == SPECIAL_ShouldTryRematchBattle;
    let entry = if first {
        rematch_index(trainer.0)
    } else {
        rematch_index_any(trainer.0)
    };
    let Some(i) = entry else {
        return 0;
    };
    if ctx.flag(registered_flag(i)).await && five_badges(ctx).await {
        degraded(ctx, id).await;
    }
    if !first {
        return 0;
    }
    let second = crate::battle::REMATCHES[usize::from(i)][1];
    ctx.flag(trainer_flag(TrainerId(second))).await as u16
}

/// Gen3 `checkpartymove` (`ScrCmd_checkpartymove`): `VAR_RESULT` = the first party slot whose
/// mon knows `m` and `VAR_0x8004` = its species, else `VAR_RESULT` = `PARTY_SIZE`.
pub async fn check_party_move(ctx: &Ctx, m: MoveId) {
    let slot = ctx.query(Query::PartyMoveSlot(m)).await;
    ctx.set_var(VAR_RESULT, slot).await;
    if slot < PARTY_SIZE {
        let species = ctx.query(Query::PartySpecies(slot as u8)).await;
        ctx.set_var(VAR_0x8004, species).await;
    }
}

/// The beam area of Mauville's gym (cells (0..9, 5..17), Gen3 less `MAP_OFFSET`): each
/// metatile is replaced by `change`'s answer (metatile, impassable), in Gen3's row order. The
/// floor's answer depends on the cell above, already changed.
async fn mauville_gym_scan(ctx: &Ctx, change: fn(u16, u16) -> Option<(u16, bool)>) {
    for y in 5..17 {
        for x in 0..9 {
            let m = ctx.query(Query::Metatile { x, y }).await;
            let above = if m == mauville_gym::FLOOR {
                ctx.query(Query::Metatile { x, y: y - 1 }).await
            } else {
                0
            };
            if let Some((to, blocked)) = change(m, above) {
                ctx.set_metatile(x, y, to, blocked).await;
            }
        }
    }
}

/// Gen3 `METATILE_MauvilleGym_*` (`include/constants/metatile_labels.h`) and the beam toggles
/// of `MauvilleGymSetDefaultBarriers` and `MauvilleGymDeactivatePuzzle` (`field_specials.c`).
mod mauville_gym {
    pub const FLOOR: u16 = 0x21A;
    const GREEN_H1_OFF: u16 = 0x230;
    const GREEN_H1_ON: u16 = 0x220;
    const GREEN_H2_OFF: u16 = 0x231;
    const GREEN_H2_ON: u16 = 0x221;
    const GREEN_H3_OFF: u16 = 0x238;
    const GREEN_H3_ON: u16 = 0x228;
    const GREEN_H4_OFF: u16 = 0x239;
    const GREEN_H4_ON: u16 = 0x229;
    const GREEN_V1_ON: u16 = 0x240;
    const GREEN_V2_ON: u16 = 0x248;
    const POLE_BOTTOM_OFF: u16 = 0x243;
    const POLE_BOTTOM_ON: u16 = 0x242;
    const POLE_TOP_OFF: u16 = 0x251;
    const POLE_TOP_ON: u16 = 0x250;
    pub const PRESSED_SWITCH: u16 = 0x206;
    pub const RAISED_SWITCH: u16 = 0x205;
    const RED_H1_OFF: u16 = 0x232;
    const RED_H1_ON: u16 = 0x222;
    const RED_H2_OFF: u16 = 0x233;
    const RED_H2_ON: u16 = 0x223;
    const RED_H3_OFF: u16 = 0x23A;
    const RED_H3_ON: u16 = 0x22A;
    const RED_H4_OFF: u16 = 0x23B;
    const RED_H4_ON: u16 = 0x22B;
    const RED_V1_ON: u16 = 0x241;
    const RED_V2_ON: u16 = 0x249;

    /// `sMauvilleGymSwitchCoords`.
    pub const SWITCHES: [(i16, i16); 4] = [(0, 15), (4, 12), (3, 9), (8, 9)];

    /// `MauvilleGymSetDefaultBarriers`: every beam flips; a flipped-on lower beam blocks.
    pub fn toggle(m: u16, above: u16) -> Option<(u16, bool)> {
        Some(match m {
            GREEN_H1_ON => (GREEN_H1_OFF, false),
            GREEN_H2_ON => (GREEN_H2_OFF, false),
            GREEN_H3_ON => (GREEN_H3_OFF, false),
            GREEN_H4_ON => (GREEN_H4_OFF, false),
            GREEN_H1_OFF => (GREEN_H1_ON, false),
            GREEN_H2_OFF => (GREEN_H2_ON, false),
            GREEN_H3_OFF => (GREEN_H3_ON, true),
            GREEN_H4_OFF => (GREEN_H4_ON, true),
            RED_H1_ON => (RED_H1_OFF, false),
            RED_H2_ON => (RED_H2_OFF, false),
            RED_H3_ON => (RED_H3_OFF, false),
            RED_H4_ON => (RED_H4_OFF, false),
            RED_H1_OFF => (RED_H1_ON, false),
            RED_H2_OFF => (RED_H2_ON, false),
            RED_H3_OFF => (RED_H3_ON, true),
            RED_H4_OFF => (RED_H4_ON, true),
            GREEN_V1_ON => (POLE_BOTTOM_ON, true),
            GREEN_V2_ON => (FLOOR, false),
            RED_V1_ON => (POLE_BOTTOM_OFF, true),
            RED_V2_ON => (FLOOR, false),
            POLE_BOTTOM_ON => (GREEN_V1_ON, true),
            FLOOR if above == GREEN_V1_ON => (GREEN_V2_ON, true),
            FLOOR => (RED_V2_ON, true),
            POLE_BOTTOM_OFF => (RED_V1_ON, true),
            POLE_TOP_OFF => (POLE_TOP_ON, true),
            POLE_TOP_ON => (POLE_TOP_OFF, false),
            _ => return None,
        })
    }

    /// `MauvilleGymDeactivatePuzzle`: every beam off.
    pub fn deactivate(m: u16, _above: u16) -> Option<(u16, bool)> {
        Some(match m {
            GREEN_H1_ON => (GREEN_H1_OFF, false),
            GREEN_H2_ON => (GREEN_H2_OFF, false),
            GREEN_H3_ON => (GREEN_H3_OFF, false),
            GREEN_H4_ON => (GREEN_H4_OFF, false),
            RED_H1_ON => (RED_H1_OFF, false),
            RED_H2_ON => (RED_H2_OFF, false),
            RED_H3_ON => (RED_H3_OFF, false),
            RED_H4_ON => (RED_H4_OFF, false),
            GREEN_V1_ON => (POLE_BOTTOM_ON, true),
            RED_V1_ON => (POLE_BOTTOM_OFF, true),
            GREEN_V2_ON | RED_V2_ON => (FLOOR, false),
            POLE_TOP_ON => (POLE_TOP_OFF, false),
            _ => return None,
        })
    }
}

/// Gen3 `PARTY_SIZE`.
const PARTY_SIZE: u16 = engine::script::PARTY_SIZE;
/// Gen3 `MAX_TOTAL_EVS`.
const MAX_TOTAL_EVS: u16 = 510;
/// Gen3 `NUM_FRONTIER_FACILITIES`.
const NUM_FRONTIER_FACILITIES: u16 = 7;
/// What `EnableNationalPokedex` writes to `VAR_NATIONAL_DEX` (and `IsNationalPokedexEnabled`
/// checks).
const NATIONAL_DEX_MAGIC_VAR: u16 = 0x302;
/// Gen3 `TRAINER_REGISTERED_FLAGS_START` (`include/constants/flags.h`): flag `+ i` is set when
/// the trainer of `gRematchTable[i]` is registered in Match Call.
pub const TRAINER_REGISTERED_FLAGS_START: u16 = 0x15C;
/// The engine's day (`gLocalTime.days`): no RTC before Milestone 10.
const RTC_DAYS: i32 = 0;
/// The Emerald story's flags, vars and maps that the specials name (`crate::emerald_story`); a
/// game without them (Silice) has `None`, and the special reports itself degraded.
#[cfg(not(pkr_silice))]
use crate::emerald_story as story;
#[cfg(pkr_silice)]
mod story {
    use super::*;
    pub const BLACK_GLASSES: Option<FlagId> = None;
    pub const SHIP_KEYS: [Option<FlagId>; 4] = [None; 4];
    pub const MIRAGE_RND_H: Option<VarId> = None;
    pub const HAS_MATCH_CALL: Option<FlagId> = None;
    pub const PACIFIDLOG_TM_DAY: Option<VarId> = None;
    pub const RUSTURF: Option<Rusturf> = None;
}

/// `TryUpdateRusturfTunnelState`'s names: the tunnel's map, its opened flag, its two rocks'
/// flags with the state each sets, and the state var.
pub struct Rusturf {
    pub opened: FlagId,
    pub map: MapId,
    pub rocks: [(FlagId, u16); 2],
    pub state: VarId,
}

/// The Match Call flag, which `RegisterTrainerInMatchCall` reads after each won trainer battle
/// (`None`: the game has no Match Call).
pub const MATCH_CALL_FLAG: Option<FlagId> = story::HAS_MATCH_CALL;

/// Gen3 `RegisterTrainerInMatchCall` (battle_setup.c, after a won trainer battle): with
/// the Match Call flag, `GetTrainerMatchCallFlag(trainer)` is set.
pub async fn register_in_match_call(ctx: &Ctx, trainer: TrainerId) {
    if let Some(match_call) = MATCH_CALL_FLAG
        && ctx.flag(match_call).await
        && let Some(i) = rematch_index(trainer.0)
    {
        ctx.set_flag(registered_flag(i), true).await;
    }
}

/// The flag registering `gRematchTable[i]`'s trainer.
pub const fn registered_flag(i: u8) -> FlagId {
    FlagId(TRAINER_REGISTERED_FLAGS_START + i as u16)
}

/// Gen3 `FirstBattleTrainerIdToRematchTableId` (and `GetRematchIdxByTrainerIdx`): the entry
/// whose first stage is `trainer`.
pub fn rematch_index(trainer: u16) -> Option<u8> {
    (crate::battle::REMATCHES.iter())
        .position(|r| r[0] == trainer)
        .map(|i| i as u8)
}

/// Gen3 `TrainerIdToRematchTableId`: the entry with `trainer` at any stage (up to its first
/// `TRAINER_NONE`).
fn rematch_index_any(trainer: u16) -> Option<u8> {
    (crate::battle::REMATCHES.iter())
        .position(|r| r.iter().take_while(|&&t| t != 0).any(|&t| t == trainer))
        .map(|i| i as u8)
}

/// Gen3 `HasAtLeastFiveBadges`.
async fn five_badges(ctx: &Ctx) -> bool {
    let mut n = 0;
    for f in [
        FLAG_BADGE01_GET,
        FLAG_BADGE02_GET,
        FLAG_BADGE03_GET,
        FLAG_BADGE04_GET,
        FLAG_BADGE05_GET,
        FLAG_BADGE06_GET,
        FLAG_BADGE07_GET,
        FLAG_BADGE08_GET,
    ] {
        n += ctx.flag(f).await as u8;
    }
    n >= 5
}

async fn dex_count(ctx: &Ctx, caught: bool, among: &'static [SpeciesId]) -> u16 {
    ctx.query(Query::DexCount { caught, among }).await
}

/// Gen3 `IsNationalPokedexEnabled` (`nationalMagic` is written with the var).
async fn national_dex_enabled(ctx: &Ctx) -> bool {
    ctx.var(VAR_NATIONAL_DEX).await == NATIONAL_DEX_MAGIC_VAR
        && ctx.flag(FLAG_SYS_NATIONAL_DEX).await
}

/// Gen3 `HasAllHoennMons` (`pokedex.c`): every Hoenn species but the last two (Jirachi,
/// Deoxys) caught.
async fn has_all_hoenn_mons(ctx: &Ctx) -> bool {
    let dex = &crate::battle::HOENN_DEX;
    let among = &dex[..dex.len().saturating_sub(2)];
    dex_count(ctx, true, among).await as usize == among.len() && !among.is_empty()
}

/// Gen3 `HasAllFrontierSymbols` (`trainer_card.c`): stops at the first missing symbol.
async fn has_all_frontier_symbols(ctx: &Ctx) -> bool {
    for i in 0..NUM_FRONTIER_FACILITIES {
        if !ctx.flag(FlagId(FLAG_SYS_TOWER_SILVER.0 + 2 * i)).await
            || !ctx.flag(FlagId(FLAG_SYS_TOWER_GOLD.0 + 2 * i)).await
        {
            return false;
        }
    }
    true
}

static JIRACHI: [SpeciesId; 1] = [mons::JIRACHI];
static DEOXYS: [SpeciesId; 1] = [mons::DEOXYS];

/// The species the specials name; without battle data (`cfg(pkr_stub_battle)`) no species has
/// them.
#[cfg(not(pkr_stub_battle))]
mod mons {
    use super::*;
    pub const WAILORD: SpeciesId = SPECIES_WAILORD;
    pub const RELICANTH: SpeciesId = SPECIES_RELICANTH;
    pub const JIRACHI: SpeciesId = SPECIES_JIRACHI;
    pub const DEOXYS: SpeciesId = SPECIES_DEOXYS;
}
#[cfg(pkr_stub_battle)]
mod mons {
    use super::SpeciesId;
    pub const WAILORD: SpeciesId = SpeciesId(u16::MAX);
    pub const RELICANTH: SpeciesId = SpeciesId(u16::MAX);
    pub const JIRACHI: SpeciesId = SpeciesId(u16::MAX);
    pub const DEOXYS: SpeciesId = SpeciesId(u16::MAX);
}

/// The labels of Gen3 `GetPokedexRatingText`'s texts by tens: `gBirchDexRatingText_LessThan10`
/// … `_LessThan200`.
const RATING_TEXTS: [&str; 20] = [
    "gBirchDexRatingText_LessThan10",
    "gBirchDexRatingText_LessThan20",
    "gBirchDexRatingText_LessThan30",
    "gBirchDexRatingText_LessThan40",
    "gBirchDexRatingText_LessThan50",
    "gBirchDexRatingText_LessThan60",
    "gBirchDexRatingText_LessThan70",
    "gBirchDexRatingText_LessThan80",
    "gBirchDexRatingText_LessThan90",
    "gBirchDexRatingText_LessThan100",
    "gBirchDexRatingText_LessThan110",
    "gBirchDexRatingText_LessThan120",
    "gBirchDexRatingText_LessThan130",
    "gBirchDexRatingText_LessThan140",
    "gBirchDexRatingText_LessThan150",
    "gBirchDexRatingText_LessThan160",
    "gBirchDexRatingText_LessThan170",
    "gBirchDexRatingText_LessThan180",
    "gBirchDexRatingText_LessThan190",
    "gBirchDexRatingText_LessThan200",
];
const DEX_COMPLETED: &str = "gBirchDexRatingText_DexCompleted";

/// Gen3 `GetPokedexRatingText` (`birch_pc.c`) for `count` Hoenn mons caught.
pub async fn pokedex_rating_text(ctx: &Ctx, count: u16) -> &'static str {
    const HOENN_DEX_COUNT: u16 = 202;
    if count < 200 {
        return RATING_TEXTS[usize::from(count / 10)];
    }
    let jirachi = dex_count(ctx, true, &JIRACHI).await != 0;
    let deoxys = dex_count(ctx, true, &DEOXYS).await != 0;
    match count {
        200 if jirachi || deoxys => RATING_TEXTS[19],
        200 => DEX_COMPLETED,
        c if c == HOENN_DEX_COUNT - 1 && jirachi && deoxys => RATING_TEXTS[19],
        c if c == HOENN_DEX_COUNT - 1 || c == HOENN_DEX_COUNT => DEX_COMPLETED,
        _ => RATING_TEXTS[0],
    }
}

/// The message `name` (a text of extract's `C_TEXT_RUNS`); a build without it (the stub, a
/// game that dropped it) reports `id` degraded.
async fn show_text_named(ctx: &Ctx, id: u16, name: &str) {
    match text_named(name) {
        Some(t) => ctx.message(t).await,
        None => degraded(ctx, id).await,
    }
}

/// A special naming a flag or var of the Emerald story that this game does not have.
async fn not_in_game(ctx: &Ctx, id: u16) -> u16 {
    degraded(ctx, id).await;
    0
}

/// Gen3 `GetLeadMonIndex`: the first party mon (no eggs yet), 0 for an empty party.
async fn lead_mon_index(ctx: &Ctx) -> u8 {
    let _ = ctx;
    0
}

/// Gen3 `CountPartyAliveNonEggMonsExcept`: the party's mons with HP, but slot `except`.
async fn party_alive_except(ctx: &Ctx, except: u16) -> u16 {
    let mut n = 0;
    for slot in 0..PARTY_SIZE {
        if slot != except
            && ctx.query(Query::PartySpecies(slot as u8)).await != 0
            && ctx.query(Query::PartyHp(slot as u8)).await != 0
        {
            n += 1;
        }
    }
    n
}
