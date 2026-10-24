//! BPEE symbol addresses (RAM variables and the function addresses the mode is read from).
//!
//! [`BPEE`] is the committed table: addresses are not ROM content, and they hold for the one
//! ROM whose SHA-1 is [`crate::BPEE_SHA1`] (the decomp's `rom.sha1`). They were taken from a
//! matching decomp build (`make compare` with agbcc, then `arm-none-eabi-objdump -t
//! pokeemerald.elf`). With `PKR_VANILLA_MAP` set to a listing of that build ([`Sym::from_env`])
//! the addresses are resolved from it instead, and every difference is reported:
//! - `pokeemerald.map` (the linker map) has the global symbols only; the `static` ones
//!   (`sGlobalScriptContext`, `sLockFieldControls`, the task functions) then keep the table's;
//! - `pokeemerald.sym` (`make syms`) or `objdump -t pokeemerald.elf` have the statics too.
//!
//! Function addresses are even (Thumb bit clear); compare them with `addr & !1`.

use anyhow::{Context, Result, anyhow};
use std::collections::BTreeMap;
use std::path::Path;

macro_rules! symbols {
    ($($(#[$m:meta])* $field:ident = $name:literal @ $addr:literal,)*) => {
        /// Resolved symbol addresses (field docs name the decomp symbol).
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Sym {
            $($(#[$m])* #[doc = concat!("`", $name, "`")] pub $field: u32,)*
        }

        /// The committed BPEE table.
        pub const BPEE: Sym = Sym { $($field: $addr,)* };

        /// `(decomp name, BPEE address)` for every field.
        pub const NAMES: &[(&str, u32)] = &[$(($name, $addr)),*];

        impl Sym {
            fn set(&mut self, name: &str, addr: u32) {
                match name {
                    $($name => self.$field = addr,)*
                    _ => unreachable!("{name}"),
                }
            }
        }
    };
}

symbols! {
    save_block1_ptr = "gSaveBlock1Ptr" @ 0x03005d8c,
    save_block2_ptr = "gSaveBlock2Ptr" @ 0x03005d90,
    player_party = "gPlayerParty" @ 0x020244ec,
    player_party_count = "gPlayerPartyCount" @ 0x020244e9,
    enemy_party = "gEnemyParty" @ 0x02024744,
    object_events = "gObjectEvents" @ 0x02037350,
    player_avatar = "gPlayerAvatar" @ 0x02037590,
    main = "gMain" @ 0x030022c0,
    tasks = "gTasks" @ 0x03005e00,
    sprites = "gSprites" @ 0x02020630,
    palette_fade = "gPaletteFade" @ 0x02037fd4,
    text_printers = "sTextPrinters" @ 0x020201b0,
    battle_type_flags = "gBattleTypeFlags" @ 0x02022fec,
    battle_mons = "gBattleMons" @ 0x02024084,
    battlers_count = "gBattlersCount" @ 0x0202406c,
    action_selection_cursor = "gActionSelectionCursor" @ 0x020244ac,
    move_selection_cursor = "gMoveSelectionCursor" @ 0x020244b0,
    battle_outcome = "gBattleOutcome" @ 0x0202433a,
    battler_controller_funcs = "gBattlerControllerFuncs" @ 0x03005d60,
    script_context = "sGlobalScriptContext" @ 0x03000e40,
    script_context_status = "sGlobalScriptContextStatus" @ 0x03000e38,
    lock_field_controls = "sLockFieldControls" @ 0x03000f2c,
    field_message_box_mode = "sFieldMessageBoxMode" @ 0x020375bc,
    string_var1 = "gStringVar1" @ 0x02021cc4,
    string_var2 = "gStringVar2" @ 0x02021dc4,
    string_var3 = "gStringVar3" @ 0x02021ec4,
    special_var_8000 = "gSpecialVar_0x8000" @ 0x020375d8,
    special_var_result = "gSpecialVar_Result" @ 0x020375f0,
    naming_screen = "sNamingScreen" @ 0x02039f94,
    rng_value = "gRngValue" @ 0x03005d80,
    cb1_overworld = "CB1_Overworld" @ 0x08085e04,
    cb2_overworld = "CB2_Overworld" @ 0x08085e5c,
    cb2_load_map = "CB2_LoadMap" @ 0x08085fcc,
    cb2_new_game = "CB2_NewGame" @ 0x08085ef8,
    cb2_init_battle = "CB2_InitBattle" @ 0x08036760,
    battle_main_cb2 = "BattleMainCB2" @ 0x08038420,
    cb2_init_title_screen = "CB2_InitTitleScreen" @ 0x080aa7a4,
    cb2_main_menu = "CB2_MainMenu" @ 0x0802f6b0,
    main_cb2_intro = "MainCB2_Intro" @ 0x0816cc00,
    task_title_phase1 = "Task_TitleScreenPhase1" @ 0x080aab44,
    task_title_phase2 = "Task_TitleScreenPhase2" @ 0x080aac50,
    task_title_phase3 = "Task_TitleScreenPhase3" @ 0x080aad64,
    task_main_menu_input = "Task_HandleMainMenuInput" @ 0x0803024c,
    task_naming_screen = "Task_NamingScreen" @ 0x080e31b0,
    task_yes_no = "Task_HandleYesNoInput" @ 0x080e215c,
    task_multichoice = "Task_HandleMultichoiceInput" @ 0x080e2058,
    task_birch_choose_gender = "Task_NewGameBirchSpeech_ChooseGender" @ 0x08030e38,
    task_truck_sequence = "Task_HandleTruckSequence" @ 0x080fb36c,
    /// The wall clock's "Is this the correct time?" (its yes/no starts on NO).
    task_set_clock_confirm = "Task_SetClock_HandleConfirmInput" @ 0x08134e30,
    /// The starter screen (`tStarterSelection` = task data 0) and its yes/no.
    task_starter_choose = "Task_HandleStarterChooseInput" @ 0x0813425c,
    task_starter_confirm = "Task_HandleConfirmStarterInput" @ 0x08134400,
    /// `menu.c`'s (12 bytes; two other files have an `sMenu`): `cursorPos` at +2.
    menu = "sMenu" @ 0x0203cd90,
    /// `scrcmd.c`'s, the native of `waitbuttonpress`.
    wait_for_a_or_b_press = "WaitForAorBPress" @ 0x0809ac98,
    /// The native of `waitmessage`.
    is_field_message_box_hidden = "IsFieldMessageBoxHidden" @ 0x0809833c,
    /// `battle_controller_player.c`'s (the safari controller has another one).
    handle_input_choose_action = "HandleInputChooseAction" @ 0x08057588,
    handle_input_choose_move = "HandleInputChooseMove" @ 0x08057bfc,
    handle_input_choose_target = "HandleInputChooseTarget" @ 0x08057824,
    /// `struct BackupMapLayout { s32 width, height; u16 *map; }`: the live map grid, with the
    /// `setmetatile` changes (the planner's overrides).
    backup_map_layout = "gBackupMapLayout" @ 0x03005dc0,
    /// The battle script's next command (`yesnoboxlearnmove` 0x5A, `yesnoboxstoplearningmove`
    /// 0x5B: the learn-move prompts).
    battlescript_curr_instr = "gBattlescriptCurrInstr" @ 0x02024214,
    /// `gBattleCommunication[1]`: the battle yes/no box's cursor.
    battle_communication = "gBattleCommunication" @ 0x02024332,
    /// The level-up move a battle's learn prompt offers.
    move_to_learn = "gMoveToLearn" @ 0x020244e2,
    /// `struct MonSummaryScreenData *` (`pokemon_summary_screen.c`): `newMove` at +0x40C4,
    /// `firstMoveIndex` (the forget-a-move cursor) at +0x40C6.
    mon_summary_screen = "sMonSummaryScreen" @ 0x0203cf1c,
    /// A Match Call on the field (the PokéNav window's text waits for A).
    task_match_call = "ExecuteMatchCall" @ 0x081960e0,
    /// The PokéNav's main callback.
    cb2_pokenav = "CB2_Pokenav" @ 0x081c7400,
    /// `struct PokenavResources *`: `mode` at +8.
    pokenav_resources = "gPokenavResources" @ 0x0203cf40,
    /// The summary screen's forget-a-move input.
    task_replace_move = "Task_HandleReplaceMoveInput" @ 0x081c174c,
    /// `start_menu.c`'s input callback while the start menu takes input.
    menu_callback = "gMenuCallback" @ 0x03005df4,
    handle_start_menu_input = "HandleStartMenuInput" @ 0x0809fac4,
    /// The start menu's cursor, its action count and its `MENU_ACTION_*` list (9 bytes).
    start_menu_cursor_pos = "sStartMenuCursorPos" @ 0x0203760e,
    num_start_menu_actions = "sNumStartMenuActions" @ 0x0203760f,
    current_start_menu_actions = "sCurrentStartMenuActions" @ 0x02037610,
    /// The bag's main callback and its list and context-menu input tasks.
    cb2_bag_menu_run = "CB2_BagMenuRun" @ 0x081aad5c,
    task_bag_menu_handle_input = "Task_BagMenu_HandleInput" @ 0x081abd28,
    task_item_context_multiple_rows = "Task_ItemContext_MultipleRows" @ 0x081acc78,
    task_item_context_single_row = "Task_ItemContext_SingleRow" @ 0x081acc04,
    /// `struct BagPosition`: `pocket` at +5, `cursorPosition[5]` at +8, `scrollPosition[5]`
    /// at +0x12.
    bag_position = "gBagPosition" @ 0x0203ce58,
    /// The party menu's main callback, its choose-a-mon input task and `struct PartyMenu`
    /// (`slotId` at +9).
    cb2_update_party_menu = "CB2_UpdatePartyMenu" @ 0x081b01b0,
    task_handle_choose_mon_input = "Task_HandleChooseMonInput" @ 0x081b1370,
    party_menu = "gPartyMenu" @ 0x0203cec8,
    /// `struct PartyMenuInternal *`: `actions[8]` at +15, `numActions` at +23 (the field
    /// moves' selection menu).
    party_menu_internal = "sPartyMenuInternal" @ 0x0203cec4,
    /// The party menu's action list input (`sMenu`'s cursor).
    task_handle_selection_menu_input = "Task_HandleSelectionMenuInput" @ 0x081b3730,
    /// The fly map: its callback, its data (`regionMap` at +8) and the full map's input
    /// callback (the cursor is still).
    cb2_fly_map = "CB2_FlyMap" @ 0x081248d4,
    fly_map = "sFlyMap" @ 0x0203a148,
    process_region_map_input_full = "ProcessRegionMapInput_Full" @ 0x081230c4,
    /// `struct RegionMapLocation { u8 x, y, width, height; const u8 *name; }` per `MAPSEC`.
    region_map_entries = "gRegionMapEntries" @ 0x085a147c,
    /// The Poké Mart: the BUY / SELL / QUIT menu's task, the buy screen's callback, its list
    /// and quantity tasks, `struct ShopData *` (`selectedRow` at +0x2006, `scrollOffset` at
    /// +0x2008) and `struct MartInfo` (`itemList` at +8).
    task_shop_menu = "Task_ShopMenu" @ 0x080dfb88,
    cb2_buy_menu = "CB2_BuyMenu" @ 0x080dfd64,
    task_buy_menu = "Task_BuyMenu" @ 0x080e0ac8,
    task_buy_how_many = "Task_BuyHowManyDialogueHandleInput" @ 0x080e0d88,
    shop_data = "sShopData" @ 0x02039f70,
    mart_info = "sMartInfo" @ 0x02039f60,
    /// `GameClear`'s screens: the Hall of Fame, then the credits (then a reset).
    cb2_do_hall_of_fame_screen = "CB2_DoHallOfFameScreen" @ 0x08173694,
    cb2_hall_of_fame = "CB2_HallOfFame" @ 0x08173560,
    cb2_start_credits_sequence = "CB2_StartCreditsSequence" @ 0x08175620,
    cb2_credits = "CB2_Credits" @ 0x081754dc,
    /// The choose-half party menu (`PARTY_MENU_TYPE_CHOOSE_HALF`) and its order.
    selected_order_from_party = "gSelectedOrderFromParty" @ 0x0203cef8,
    // Rotating gates (`rotating_gate.c`): the puzzle and its ROM tables.
    rotating_gate_puzzle_config = "sRotatingGate_PuzzleConfig" @ 0x0203a044,
    rotating_gate_puzzle_count = "sRotatingGate_PuzzleCount" @ 0x0203a048,
    rotating_gate_rotation_north = "sRotatingGate_RotationInfoNorth" @ 0x08591ff8,
    rotating_gate_rotation_south = "sRotatingGate_RotationInfoSouth" @ 0x08592008,
    rotating_gate_rotation_west = "sRotatingGate_RotationInfoWest" @ 0x08592018,
    rotating_gate_rotation_east = "sRotatingGate_RotationInfoEast" @ 0x08592028,
    rotating_gate_arms_clockwise = "sRotatingGate_ArmPositionsClockwiseRotation" @ 0x08592038,
    rotating_gate_arms_anticlockwise = "sRotatingGate_ArmPositionsAntiClockwiseRotation" @ 0x08592058,
    rotating_gate_arm_layout = "sRotatingGate_ArmLayout" @ 0x08592078,
}

/// Every address of every name in a symbol listing: any line whose first token is a hex
/// address (`0x` optional) and whose last token is the name (or `addr name = .`). That reads
/// the linker map's symbol lines, `make syms`' `.sym` and `objdump -t`; other lines name no
/// wanted symbol.
pub fn parse_listing(text: &str) -> BTreeMap<&str, Vec<u32>> {
    let mut out: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        // `0x03005d8c gSaveBlock1Ptr = .`: the linker script's own symbols (the RAM ones).
        let name = match t.as_slice() {
            [_, name, "=", ..] => name,
            [_, .., last] => last,
            _ => continue,
        };
        let hex = t[0].strip_prefix("0x").unwrap_or(t[0]);
        if hex.len() != 8 {
            continue;
        }
        if let Ok(a) = u32::from_str_radix(hex, 16) {
            let v = out.entry(*name).or_default();
            if !v.contains(&a) {
                v.push(a);
            }
        }
    }
    out
}

impl Sym {
    /// Resolves every name from `listing`. A name listed once takes that address; a name
    /// listed several times (statics of the same name in two files) keeps [`BPEE`]'s when it
    /// is one of them; a name the listing lacks keeps [`BPEE`]'s. Returns the notes: each
    /// kept or changed address.
    pub fn from_listing(listing: &str) -> Result<(Sym, Vec<String>)> {
        let all = parse_listing(listing);
        let mut sym = BPEE;
        let mut notes = Vec::new();
        for &(name, committed) in NAMES {
            match all.get(name).map(Vec::as_slice) {
                None | Some([]) => notes.push(format!("{name}: not listed, kept {committed:#x}")),
                Some([a]) => {
                    if *a != committed {
                        notes.push(format!("{name}: {a:#x} (table {committed:#x})"));
                    }
                    sym.set(name, *a);
                }
                Some(many) => {
                    if !many.contains(&committed) {
                        return Err(anyhow!(
                            "{name}: listed at {many:x?}, none is the table's {committed:#x}"
                        ));
                    }
                }
            }
        }
        Ok((sym, notes))
    }

    /// [`Sym::from_listing`] on the file `PKR_VANILLA_MAP` names, or [`BPEE`] when unset.
    pub fn from_env() -> Result<(Sym, Vec<String>)> {
        match std::env::var_os("PKR_VANILLA_MAP") {
            None => Ok((BPEE, Vec::new())),
            Some(p) => Self::from_file(Path::new(&p)),
        }
    }

    pub fn from_file(path: &Path) -> Result<(Sym, Vec<String>)> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::from_listing(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listing_formats_parse() {
        let text = "\
 .bss           0x03005d8c        0x4 src/load_save.o
                0x03005d8c                        gSaveBlock1Ptr = .
03005d90 g     O iwram\t00000004 gSaveBlock2Ptr
03000e40 l 00000074 sGlobalScriptContext
08057588 l     F .text\t00000266 HandleInputChooseAction
081593d8 l     F .text\t00000184 HandleInputChooseAction
";
        let all = parse_listing(text);
        assert_eq!(all["gSaveBlock1Ptr"], [0x03005d8c]);
        assert_eq!(all["gSaveBlock2Ptr"], [0x03005d90]);
        assert_eq!(all["sGlobalScriptContext"], [0x03000e40]);
        assert_eq!(all["HandleInputChooseAction"].len(), 2);
        let (sym, notes) = Sym::from_listing(text).unwrap();
        assert_eq!(sym, BPEE);
        assert!(notes.iter().any(|n| n.starts_with("gMain: not listed")));
    }

    #[test]
    fn a_moved_symbol_is_taken_and_noted() {
        let (sym, notes) = Sym::from_listing("0x03000000 gMain").unwrap();
        assert_eq!(sym.main, 0x0300_0000);
        assert!(notes.contains(&"gMain: 0x3000000 (table 0x30022c0)".to_string()));
        assert!(
            Sym::from_listing(
                "08000000 x HandleInputChooseAction\n08000002 x HandleInputChooseAction"
            )
            .is_err()
        );
    }

    #[test]
    fn the_table_is_sane() {
        for &(name, a) in NAMES {
            let region = a >> 24;
            assert!(
                matches!(region, 2 | 3 | 8),
                "{name} at {a:#x}: not EWRAM, IWRAM or ROM"
            );
            if region == 8 {
                assert_eq!(a & 1, 0, "{name}: function addresses are even");
            }
        }
    }

    /// With `PKR_VANILLA_MAP` (a listing of a matching decomp build), the committed table
    /// agrees with it.
    #[test]
    fn the_table_matches_the_decomp_build() {
        let Some(p) = std::env::var_os("PKR_VANILLA_MAP") else {
            eprintln!("PKR_VANILLA_MAP unset: skipped");
            return;
        };
        let (sym, notes) = Sym::from_file(Path::new(&p)).unwrap();
        for n in &notes {
            eprintln!("{n}");
        }
        assert_eq!(sym, BPEE);
    }
}
