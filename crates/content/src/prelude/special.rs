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

/// Reports `id` as run in a simplified way (its decomp name).
async fn degraded(ctx: &Ctx, id: u16) {
    let name = name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?");
    ctx.report(Report::Degraded(name)).await;
}

/// `special <id>` / `specialvar <var>, <id>` for the `Native` specials: logs the special's name,
/// then runs it; the result (written by `specialvar`) is the Gen3 return value, 0 for a void
/// special. A special with no arm here is only logged and returns 0, as in Wave 0 (only the
/// legacy generator, which calls this for every special, gets there).
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
/// - Degraded (reported), as if cancelled:
///   - `GetPlayerBigGuyGirlString`, `GetRivalSonDaughterString`,
///     `GetMomOrDadStringForTVMessage`: `STR_VAR_1` is left as is (no table of the C strings).
///     `GetMomOrDadStringForTVMessage` still keeps Mom or Dad in `VAR_TEMP_3`, drawn at random
///     when unset, so the RNG advances as in Gen3; in the player's house Gen3 picks Mom with no
///     draw (a map check this host cannot make yet), so there one draw too many is made;
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
        id if id == SPECIAL_GetPlayerBigGuyGirlString
            || id == SPECIAL_GetRivalSonDaughterString
            || id == SPECIAL_StartWallyTutorialBattle =>
        {
            degraded(ctx, id).await
        }
        id if id == SPECIAL_GetMomOrDadStringForTVMessage => {
            degraded(ctx, id).await;
            // `tv.c`: with `VAR_TEMP_3` unset, Mom or Dad at random, kept in `VAR_TEMP_3`
            // (1 Mom, 2 Dad), so the RNG advances as in Gen3.
            if ctx.var(VAR_TEMP_3).await == 0 {
                let mom = ctx.random(2).await != 0;
                ctx.set_var(VAR_TEMP_3, if mom { 1 } else { 2 }).await;
            }
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
            ctx.set_var(VAR_RESULT, engine::script::MULTI_B_PRESSED)
                .await;
        }
        id if id == SPECIAL_ScriptMenu_CreateStartMenuForPokenavTutorial => {
            degraded(ctx, id).await;
            ctx.set_var(VAR_RESULT, START_MENU_POKENAV).await;
        }
        // Only the legacy generator gets here: the lowering calls `special` for `Native`
        // specials only, and reports the `Unsupported` ones itself.
        _ => {}
    }
    0
}
