//! BPEE's state read from RAM: [`State`] (the story state) and [`Mode`] (what the game waits
//! for). Struct offsets are the decomp's (`include/global.h`, `global.fieldmap.h`, `pokemon.h`,
//! `script.h`, `text.h`, `task.h`, `palette.h`), checked by compiling `offsetof`s against the
//! decomp headers with **agbcc** (the decomp's compiler). A modern arm-none-eabi-gcc lays some
//! of them out differently (`SaveBlock1.flags` at 0x1248 instead of 0x1270, the SaveBlock2 key
//! at 0xA8 instead of 0xAC): agbcc rounds struct sizes up to 4 bytes.

use crate::sym::Sym;
use anyhow::{Result, anyhow};
use emu::Emu;
use engine::data::{Dir, MapId, Pocket};
use explore::semantic::{self, Semantic};
use std::collections::BTreeMap;

/// Offsets inside the decomp's structs.
pub mod layout {
    // struct SaveBlock1
    pub const SB1_LOCATION: u32 = 0x04; // struct WarpData: s8 mapGroup, s8 mapNum, s8 warpId, s16 x, y
    pub const SB1_LAST_HEAL_LOCATION: u32 = 0x1C;
    pub const SB1_MAP_LAYOUT_ID: u32 = 0x32;
    pub const SB1_MONEY: u32 = 0x490;
    pub const SB1_COINS: u32 = 0x494;
    /// `bagPocket_Items`, `_KeyItems`, `_PokeBalls`, `_TMHM`, `_Berries` (`struct ItemSlot`:
    /// u16 item, u16 quantity ^ key).
    pub const SB1_POCKETS: [(u32, usize); 5] = [
        (0x560, 30),
        (0x5D8, 30),
        (0x650, 16),
        (0x690, 64),
        (0x790, 46),
    ];
    pub const SB1_FLAGS: u32 = 0x1270;
    pub const SB1_VARS: u32 = 0x139C;
    pub const FLAGS_BYTES: usize = 300;
    pub const VARS_COUNT: usize = 256;
    // struct SaveBlock2
    pub const SB2_NAME: u32 = 0x00;
    pub const SB2_GENDER: u32 = 0x08;
    pub const SB2_TRAINER_ID: u32 = 0x0A;
    pub const SB2_ENCRYPTION_KEY: u32 = 0xAC;
    // struct Pokemon (100 bytes): BoxPokemon then the party fields.
    pub const MON_SIZE: u32 = 100;
    pub const MON_PERSONALITY: usize = 0;
    pub const MON_OT_ID: usize = 4;
    pub const MON_CHECKSUM: usize = 28;
    pub const MON_SECURE: usize = 32; // 4 substructs of 12 bytes, ^ (personality ^ otId)
    pub const MON_STATUS: usize = 80;
    pub const MON_LEVEL: usize = 84;
    pub const MON_HP: usize = 86;
    pub const MON_MAX_HP: usize = 88;
    // struct ObjectEvent (0x24 bytes), 16 of them.
    pub const OBJECT_EVENT_SIZE: u32 = 0x24;
    pub const OBJECT_EVENTS: u32 = 16;
    pub const OE_FLAGS: u32 = 0x00; // byte 0 bit 0: active; byte 1 bit 5: invisible; byte 2 bit 0: isPlayer
    pub const OE_GRAPHICS_ID: u32 = 0x05;
    pub const OE_MOVEMENT_TYPE: u32 = 0x06;
    pub const OE_LOCAL_ID: u32 = 0x08;
    pub const OE_MAP_NUM: u32 = 0x09;
    pub const OE_MAP_GROUP: u32 = 0x0A;
    pub const OE_ELEVATION: u32 = 0x0B; // low nibble: currentElevation
    pub const OE_INITIAL_COORDS: u32 = 0x0C;
    pub const OE_CURRENT_COORDS: u32 = 0x10;
    pub const OE_FACING: u32 = 0x18; // low nibble: DIR_SOUTH 1, NORTH 2, WEST 3, EAST 4
    /// `MAP_OFFSET`: object coordinates count the 7-cell border.
    pub const MAP_OFFSET: i16 = 7;
    // struct PlayerAvatar
    pub const PA_OBJECT_EVENT_ID: u32 = 0x05;
    /// `tileTransitionState`: T_NOT_MOVING 0, T_TILE_TRANSITION 1 (mid-step), T_TILE_CENTER 2.
    pub const PA_TILE_TRANSITION_STATE: u32 = 0x03;
    // struct Main
    pub const MAIN_CALLBACK1: u32 = 0x00;
    pub const MAIN_CALLBACK2: u32 = 0x04;
    // struct Task (40 bytes), 16 of them.
    pub const TASK_SIZE: u32 = 40;
    pub const TASKS: u32 = 16;
    pub const TASK_IS_ACTIVE: u32 = 4;
    pub const TASK_DATA: u32 = 8;
    // struct TextPrinter (36 bytes), by window id; the field message box is window 0.
    pub const TP_ACTIVE: u32 = 27;
    pub const TP_STATE: u32 = 28; // RENDER_STATE_WAIT 1, _CLEAR 2, _SCROLL_START 3: wait for A
    // gPaletteFade.active: byte 7, bit 7.
    pub const FADE_ACTIVE_BYTE: u32 = 7;
    // struct ScriptContext
    pub const CTX_MODE: u32 = 1; // SCRIPT_MODE_STOPPED 0, BYTECODE 1, NATIVE 2
    pub const CTX_NATIVE_PTR: u32 = 4;
    pub const CTX_SCRIPT_PTR: u32 = 8;
    /// `sGlobalScriptContextStatus`: CONTEXT_RUNNING 0, CONTEXT_WAITING 1, CONTEXT_SHUTDOWN 2.
    pub const CONTEXT_SHUTDOWN: u8 = 2;
    // struct BattlePokemon (88 bytes)
    pub const BATTLE_MON_SIZE: u32 = 88;
    pub const BM_SPECIES: u32 = 0;
    pub const BM_MOVES: u32 = 12;
    pub const BM_PP: u32 = 36;
    pub const BM_HP: u32 = 40;
    pub const BM_LEVEL: u32 = 42;
    pub const BM_MAX_HP: u32 = 44;
    // struct Sprite (68 bytes): s16 data[8] at 0x2E.
    pub const SPRITE_SIZE: u32 = 68;
    pub const SPRITE_DATA: u32 = 0x2E;
    // struct NamingScreenData (naming_screen.c)
    pub const NS_TEXT_BUFFER: u32 = 0x1800;
    pub const NS_STATE: u32 = 0x1E10;
    pub const NS_CURRENT_PAGE: u32 = 0x1E22;
    pub const NS_CURSOR_SPRITE_ID: u32 = 0x1E23;
    pub const EWRAM: std::ops::Range<u32> = 0x0200_0000..0x0204_0000;
    // gBattleTypeFlags
    pub const BATTLE_TYPE_TRAINER: u32 = 1 << 3;
    pub const BATTLE_TYPE_FIRST_BATTLE: u32 = 1 << 4;
    // gBattleOutcome
    pub const B_OUTCOME_LOST: u8 = 2;
    pub const B_OUTCOME_DREW: u8 = 3;
    // Battle script opcodes (`B_SCR_OP_*`).
    pub const B_SCR_OP_YESNOBOXLEARNMOVE: u8 = 0x5A;
    pub const B_SCR_OP_YESNOBOXSTOPLEARNINGMOVE: u8 = 0x5B;
    pub const B_SCR_OP_YESNOBOX: u8 = 0x67;
    // struct MonSummaryScreenData
    pub const PSS_NEW_MOVE: u32 = 0x40C4;
    pub const PSS_FIRST_MOVE_INDEX: u32 = 0x40C6;
    // struct PokenavResources
    pub const PN_MODE: u32 = 8;
    pub const POKENAV_MODE_FORCE_CALL_READY: u16 = 1;
    // struct BagPosition
    pub const BP_POCKET: u32 = 5;
    pub const BP_CURSOR_POSITION: u32 = 8;
    pub const BP_SCROLL_POSITION: u32 = 0x12;
    /// `MENU_ACTION_BAG` (`start_menu.c`).
    pub const MENU_ACTION_BAG: u8 = 2;
    /// The bag's pocket (`*_POCKET`, 0-based) of each `SB1_POCKETS` entry.
    pub const BAG_POCKET_OF_SB1: [u8; 5] = [0, 4, 1, 2, 3];
    // struct PartyMenu
    pub const PM_SLOT_ID: u32 = 9;
    /// `gPartyMenu.menuType` (low nibble), `PARTY_MENU_TYPE_CHOOSE_HALF`, and the confirm
    /// button's slot (`PARTY_SIZE`).
    pub const PM_MENU_TYPE: u32 = 8;
    pub const PARTY_MENU_TYPE_CHOOSE_HALF: u8 = 4;
    pub const PARTY_SIZE: usize = 6;
    pub const BATTLE_TYPE_INGAME_PARTNER: u32 = 1 << 22;
    // party_menu.c's `struct PartyMenuInternal` (agbcc): `actions[8]`, `numActions`; the
    // action ids (`MENU_FIELD_MOVES` 19 + `FIELD_MOVE_FLY` 5).
    pub const PMI_ACTIONS: u32 = 15;
    pub const PMI_NUM_ACTIONS: u32 = 23;
    pub const MENU_FIELD_MOVE_FLY: u8 = 24;
    // start_menu.c: `MENU_ACTION_POKEMON`.
    pub const MENU_ACTION_POKEMON: u8 = 1;
    // region_map.c's fly map (`sFlyMap`: callback, u16 state, u16 mapSecId, then
    // `struct RegionMap`) and `struct RegionMap`'s fields; the cursor's map origin
    // (`MAPCURSOR_X_MIN`, `MAPCURSOR_Y_MIN`).
    pub const FM_STATE: u32 = 4;
    pub const FM_REGION_MAP: u32 = 8;
    pub const RM_MAP_SEC_ID: u32 = 0x00;
    pub const RM_INPUT_CALLBACK: u32 = 0x18;
    pub const RM_CURSOR_X: u32 = 0x54;
    pub const RM_CURSOR_Y: u32 = 0x56;
    pub const MAPCURSOR_X_MIN: u16 = 1;
    pub const MAPCURSOR_Y_MIN: u16 = 2;
    // shop.c: `struct ShopData` (after `tilemapBuffers[4][0x400]` and `totalCost`) and
    // `struct MartInfo`; the buy screen's quantity is its task's `tItemCount` (data[1]).
    pub const SD_SELECTED_ROW: u32 = 0x2006;
    pub const SD_SCROLL_OFFSET: u32 = 0x2008;
    pub const MI_ITEM_LIST: u32 = 8;
    pub const MI_ITEM_COUNT: u32 = 12;
    // The battle script's `trygivecaughtmonnick` (the caught mon's nickname yes/no).
    pub const B_SCR_OP_TRYGIVECAUGHTMONNICK: u8 = 0xF3;
    // The bag screen's POKé BALLS pocket (`BALLS_POCKET`).
    pub const BAG_POCKET_BALLS: u8 = 1;
    pub const BAG_POCKET_BERRIES: u8 = 3;
}

use layout::*;

/// What the game is doing, read from RAM (see [`Vanilla::mode`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The overworld with the player in control: no script, no lock, no fade, not mid-step.
    FieldIdle,
    /// The overworld under a script or a lock (printing, moving objects, waiting).
    Script,
    /// A field message waits for A or B (`waitbuttonpress`, or the printer's down arrow).
    Message,
    YesNo,
    Multichoice,
    /// The battle's action menu (FIGHT / BAG / POKéMON / RUN) for the player's battler.
    BattleAction,
    BattleMove,
    BattleTarget,
    /// A battle, the player's controller not waiting for a choice.
    BattleWaiting,
    /// A warp, a fade, a battle start, or the player mid-step.
    Transition,
    /// Another screen (intro, title, menus, naming screen…): its `gMain.callback2`.
    Screen {
        callback2: u32,
    },
}

/// One party mon, decrypted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mon {
    pub personality: u32,
    pub ot_id: u32,
    pub species: u16,
    pub held_item: u16,
    pub exp: u32,
    pub friendship: u8,
    pub moves: [u16; 4],
    pub pp: [u8; 4],
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub status: u32,
    /// The secure substructs' checksum held (a mismatch is a Bad Egg).
    pub checksum_ok: bool,
}

/// One object event in RAM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub local_id: u8,
    pub map: MapId,
    pub gfx: u8,
    pub movement_type: u8,
    pub cell: (i16, i16),
    /// The spawn cell (`initialCoords`).
    pub home: (i16, i16),
    pub facing: Dir,
    pub visible: bool,
    pub is_player: bool,
}

/// BPEE's story state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub map: MapId,
    pub cell: (i16, i16),
    pub facing: Dir,
    /// Set flags, ascending.
    pub flags: Vec<u16>,
    /// Non-zero saved vars, ascending.
    pub vars: Vec<(u16, u16)>,
    pub party: Vec<Mon>,
    /// `(pocket, slot item, quantity)` of the non-empty slots, in slot order.
    pub bag: Vec<(Pocket, u16, u16)>,
    pub money: u32,
    pub coins: u16,
    pub objects: Vec<Object>,
    pub badges: u8,
    /// The last heal location's map; `None` while it is all zeros (a new game sets none).
    pub respawn: Option<MapId>,
    pub player_name: [u8; 8],
    pub gender: u8,
    pub trainer_id: u32,
    pub mode: Mode,
    /// `gSaveBlock1Ptr->mapLayoutId`: the layout the map is loaded with (its own, or a
    /// `setmaplayoutindex` override).
    pub layout: u16,
}

/// Gen3's `DIR_*` to ours (`DIR_NONE` reads as down).
pub fn dir(d: u8) -> Dir {
    match d & 0xF {
        2 => Dir::Up,
        3 => Dir::Left,
        4 => Dir::Right,
        _ => Dir::Down,
    }
}

/// The substruct order of `personality % 24`: indexes of Growth, Attacks, EVs, Misc.
fn substruct_positions(personality: u32) -> [usize; 4] {
    // `GAEM`, `GAME`, … in decomp order; each letter is a substruct, its position the slot.
    const ORDERS: [&[u8; 4]; 24] = [
        b"GAEM", b"GAME", b"GEAM", b"GEMA", b"GMAE", b"GMEA", b"AGEM", b"AGME", b"AEGM", b"AEMG",
        b"AMGE", b"AMEG", b"EGAM", b"EGMA", b"EAGM", b"EAMG", b"EMGA", b"EMAG", b"MGAE", b"MGEA",
        b"MAGE", b"MAEG", b"MEGA", b"MEAG",
    ];
    let order = ORDERS[(personality % 24) as usize];
    let at = |c: u8| order.iter().position(|&x| x == c).unwrap();
    [at(b'G'), at(b'A'), at(b'E'), at(b'M')]
}

/// Decrypts one 100-byte `struct Pokemon`; `None` for an empty slot (species 0).
pub fn decode_mon(raw: &[u8]) -> Option<Mon> {
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    let personality = u32_at(raw, MON_PERSONALITY);
    let ot_id = u32_at(raw, MON_OT_ID);
    let key = personality ^ ot_id;
    let mut secure = [0u8; 48];
    for (i, w) in secure.chunks_mut(4).enumerate() {
        let v = u32_at(raw, MON_SECURE + 4 * i) ^ key;
        w.copy_from_slice(&v.to_le_bytes());
    }
    let sum = (0..24).fold(0u16, |s, i| s.wrapping_add(u16_at(&secure, 2 * i)));
    let [g, a, _, _] = substruct_positions(personality).map(|p| 12 * p);
    let species = u16_at(&secure, g);
    if species == 0 && personality == 0 {
        return None;
    }
    Some(Mon {
        personality,
        ot_id,
        species,
        held_item: u16_at(&secure, g + 2),
        exp: u32_at(&secure, g + 4),
        friendship: secure[g + 9],
        moves: std::array::from_fn(|i| u16_at(&secure, a + 2 * i)),
        pp: std::array::from_fn(|i| secure[a + 8 + i]),
        level: raw[MON_LEVEL],
        hp: u16_at(raw, MON_HP),
        max_hp: u16_at(raw, MON_MAX_HP),
        status: u32_at(raw, MON_STATUS),
        checksum_ok: sum == u16_at(raw, MON_CHECKSUM),
    })
}

/// BPEE running in mGBA, with its symbols.
pub struct Vanilla {
    pub emu: Emu,
    pub sym: Sym,
    /// Frames run since power-on.
    pub frames: u64,
    /// The starter [`Vanilla::settle`] picks on Birch's screen (`sStarterMon` index: 0 Treecko,
    /// 1 Torchic, 2 Mudkip). Mudkip by default, as our engine's `Start::AfterStarter`.
    pub starter: u8,
    /// How [`Vanilla::settle`] answers (`explore::Opts`: yes/no, battles).
    pub answers: explore::Opts,
    /// The map [`Vanilla::answers`]' yes/no `choice` holds on: once a warp took the player
    /// elsewhere, yes/no is answered YES (the choice was for the interaction that warped; our
    /// engine's macro ends at that warp, before the next map's frame script asks).
    pub answers_map: Option<MapId>,
    /// Yes/no boxes opened since the answers were set (`policy::YES_ONCE`).
    pub yes_nos: u32,
    /// Our battle tables, for the move choice (`policy::first_damaging_slot_of`,
    /// `best_damaging_slot_of`); without them the first move is used.
    pub tables: Option<&'static engine::mon::BattleTables>,
    /// `gBattleTypeFlags` of every battle [`Vanilla::settle`] has seen start, in order.
    pub battles: Vec<u32>,
    /// `gBattleOutcome` of every battle [`Vanilla::settle`] has seen end (`B_OUTCOME_*`).
    pub outcomes: Vec<u8>,
    /// Print every mode change [`Vanilla::settle`] sees (`PKR_VANILLA_TRACE`).
    pub trace: bool,
    /// The purchase [`Vanilla::settle`] makes when a Poké Mart opens: `(item, quantity, the
    /// bag's count before)`; `None` quits the mart.
    pub buying: Option<(u16, u16, u16)>,
    /// [`Vanilla::settle`] saw `GameClear`'s Hall of Fame screen: vanilla goes on to the
    /// credits and resets, so the run ends there.
    pub hall_of_fame: bool,
    /// A `Train`, `TrainFresh`, `Grass` or `Catch` step is playing: wild battles are fought
    /// (or caught) whatever the lead's HP.
    pub training: bool,
}

impl Vanilla {
    pub fn new(emu: Emu, sym: Sym) -> Self {
        Self {
            emu,
            sym,
            frames: 0,
            starter: 2,
            answers: explore::Opts::default(),
            answers_map: None,
            yes_nos: 0,
            tables: None,
            battles: Vec::new(),
            outcomes: Vec::new(),
            trace: std::env::var_os("PKR_VANILLA_TRACE").is_some(),
            buying: None,
            hall_of_fame: false,
            training: false,
        }
    }

    /// Runs one frame with `keys` held.
    pub fn step(&mut self, keys: engine::Buttons) -> Result<()> {
        self.frames += 1;
        self.emu.run_frame(keys)
    }

    fn ptr(&mut self, at: u32, what: &str) -> Result<u32> {
        let p = self.emu.read_u32(at);
        if !EWRAM.contains(&p) {
            return Err(anyhow!(
                "{what} = {p:#x}: not in EWRAM (wrong ROM or symbols?)"
            ));
        }
        Ok(p)
    }

    /// `gSaveBlock1Ptr`, checked to point into EWRAM.
    pub fn save_block1(&mut self) -> Result<u32> {
        self.ptr(self.sym.save_block1_ptr, "gSaveBlock1Ptr")
    }

    pub fn save_block2(&mut self) -> Result<u32> {
        self.ptr(self.sym.save_block2_ptr, "gSaveBlock2Ptr")
    }

    pub fn callback2(&mut self) -> u32 {
        self.emu.read_u32(self.sym.main + MAIN_CALLBACK2) & !1
    }

    pub fn callback1(&mut self) -> u32 {
        self.emu.read_u32(self.sym.main + MAIN_CALLBACK1) & !1
    }

    /// The functions of the active tasks.
    pub fn tasks(&mut self) -> Vec<u32> {
        let all = self
            .emu
            .read_bytes(self.sym.tasks, (TASKS * TASK_SIZE) as usize);
        (all.chunks(TASK_SIZE as usize))
            .filter(|t| t[TASK_IS_ACTIVE as usize] != 0)
            .map(|t| u32::from_le_bytes(t[..4].try_into().unwrap()) & !1)
            .collect()
    }

    /// The `data` of the first active task running `func`.
    pub fn task_data(&mut self, func: u32) -> Option<[i16; 16]> {
        let all = self
            .emu
            .read_bytes(self.sym.tasks, (TASKS * TASK_SIZE) as usize);
        let t = (all.chunks(TASK_SIZE as usize)).find(|t| {
            t[TASK_IS_ACTIVE as usize] != 0
                && u32::from_le_bytes(t[..4].try_into().unwrap()) & !1 == func
        })?;
        let d = TASK_DATA as usize;
        Some(std::array::from_fn(|i| {
            i16::from_le_bytes([t[d + 2 * i], t[d + 2 * i + 1]])
        }))
    }

    pub fn task_active(&mut self, func: u32) -> bool {
        self.tasks().contains(&func)
    }

    pub fn fade_active(&mut self) -> bool {
        self.emu.read_u8(self.sym.palette_fade + FADE_ACTIVE_BYTE) & 0x80 != 0
    }

    /// Text printer `window` is active and waits for A (its down arrow or a clear).
    pub fn printer_waiting(&mut self, window: u32) -> bool {
        let p = self.sym.text_printers + 36 * window;
        self.emu.read_u8(p + TP_ACTIVE) != 0 && matches!(self.emu.read_u8(p + TP_STATE), 1..=3)
    }

    pub fn printer_active(&mut self, window: u32) -> bool {
        self.emu
            .read_u8(self.sym.text_printers + 36 * window + TP_ACTIVE)
            != 0
    }

    /// The mode, from `gMain`'s callbacks, the tasks, the fade, the field message printer, the
    /// global script context and the player's battle controller.
    pub fn mode(&mut self) -> Mode {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        if cb2 == s.battle_main_cb2 {
            let f = self.emu.read_u32(s.battler_controller_funcs) & !1;
            return match f {
                f if f == s.handle_input_choose_action => Mode::BattleAction,
                f if f == s.handle_input_choose_move => Mode::BattleMove,
                f if f == s.handle_input_choose_target => Mode::BattleTarget,
                _ => Mode::BattleWaiting,
            };
        }
        if cb2 == s.cb2_overworld {
            let tasks = self.tasks();
            if tasks.contains(&s.task_yes_no) {
                return Mode::YesNo;
            }
            if tasks.contains(&s.task_multichoice) {
                return Mode::Multichoice;
            }
            if tasks.contains(&s.task_match_call) {
                return Mode::Message;
            }
            if self.fade_active() {
                return Mode::Transition;
            }
            let ctx_mode = self.emu.read_u8(s.script_context + CTX_MODE);
            let native = self.emu.read_u32(s.script_context + CTX_NATIVE_PTR) & !1;
            let status = self.emu.read_u8(s.script_context_status);
            let running = status != CONTEXT_SHUTDOWN;
            if self.printer_waiting(0)
                || (running && ctx_mode == 2 && native == s.wait_for_a_or_b_press)
            {
                return Mode::Message;
            }
            if running
                || self.emu.read_u8(s.lock_field_controls) != 0
                || self.callback1() != s.cb1_overworld
            {
                return Mode::Script;
            }
            // Mid-step the field takes no new input (a warp or a trigger may follow).
            if self.emu.read_u8(s.player_avatar + PA_TILE_TRANSITION_STATE) == 1 {
                return Mode::Transition;
            }
            return Mode::FieldIdle;
        }
        if [s.cb2_load_map, s.cb2_new_game, s.cb2_init_battle].contains(&cb2) {
            return Mode::Transition;
        }
        Mode::Screen { callback2: cb2 }
    }

    /// The raw inputs of [`Vanilla::mode`], for error messages.
    pub fn mode_inputs(&mut self) -> String {
        let s = self.sym.clone();
        let p0 = s.text_printers;
        format!(
            "cb1 {:#x} cb2 {:#x} tasks {:x?} fade {} printer0 active {} state {} ctx status {} mode {} native {:#x} script {:#x} lock {} msgbox {}",
            self.callback1(),
            self.callback2(),
            self.tasks(),
            self.fade_active(),
            self.emu.read_u8(p0 + TP_ACTIVE),
            self.emu.read_u8(p0 + TP_STATE),
            self.emu.read_u8(s.script_context_status),
            self.emu.read_u8(s.script_context + CTX_MODE),
            self.emu.read_u32(s.script_context + CTX_NATIVE_PTR),
            self.emu.read_u32(s.script_context + CTX_SCRIPT_PTR),
            self.emu.read_u8(s.lock_field_controls),
            self.emu.read_u8(s.field_message_box_mode),
        )
    }

    /// The player's object event (`gPlayerAvatar.objectEventId`).
    pub fn player_object(&mut self) -> Option<Object> {
        let id = u32::from(
            self.emu
                .read_u8(self.sym.player_avatar + PA_OBJECT_EVENT_ID),
        );
        (id < OBJECT_EVENTS)
            .then(|| self.object_event(id))
            .flatten()
    }

    /// Object event `i` of `gObjectEvents`, `None` when inactive.
    pub fn object_event(&mut self, i: u32) -> Option<Object> {
        let o = self.emu.read_bytes(
            self.sym.object_events + i * OBJECT_EVENT_SIZE,
            OBJECT_EVENT_SIZE as usize,
        );
        if o[0] & 1 == 0 {
            return None;
        }
        let s16 = |at: u32| i16::from_le_bytes([o[at as usize], o[at as usize + 1]]);
        Some(Object {
            local_id: o[OE_LOCAL_ID as usize],
            map: MapId(
                u16::from(o[OE_MAP_GROUP as usize]) << 8 | u16::from(o[OE_MAP_NUM as usize]),
            ),
            gfx: o[OE_GRAPHICS_ID as usize],
            movement_type: o[OE_MOVEMENT_TYPE as usize],
            cell: (
                s16(OE_CURRENT_COORDS) - MAP_OFFSET,
                s16(OE_CURRENT_COORDS + 2) - MAP_OFFSET,
            ),
            home: (
                s16(OE_INITIAL_COORDS) - MAP_OFFSET,
                s16(OE_INITIAL_COORDS + 2) - MAP_OFFSET,
            ),
            facing: dir(o[OE_FACING as usize]),
            visible: o[1] & 0x20 == 0,
            is_player: o[2] & 1 != 0,
        })
    }

    /// Writes `mov` with `pp` into move slot `slot` of the lead (`gPlayerParty[0]`'s
    /// encrypted Attacks substruct, and its checksum): what teaching a TM or HM from the party
    /// menu leaves, which the oracle does not drive (a `Bypass` with `teach`).
    pub fn teach(&mut self, slot: u8, mov: u16, pp: u8) {
        let addr = self.sym.player_party;
        let mut raw = self.emu.read_bytes(addr, MON_SIZE as usize);
        let u32_at = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let personality = u32_at(&raw, MON_PERSONALITY);
        let key = personality ^ u32_at(&raw, MON_OT_ID);
        let mut secure = [0u8; 48];
        for (i, w) in secure.chunks_mut(4).enumerate() {
            w.copy_from_slice(&(u32_at(&raw, MON_SECURE + 4 * i) ^ key).to_le_bytes());
        }
        let [_, a, _, _] = substruct_positions(personality).map(|p| 12 * p);
        let s = usize::from(slot.min(3));
        secure[a + 2 * s..a + 2 * s + 2].copy_from_slice(&mov.to_le_bytes());
        secure[a + 8 + s] = pp;
        let sum = (0..24).fold(0u16, |acc, i| {
            acc.wrapping_add(u16::from_le_bytes([secure[2 * i], secure[2 * i + 1]]))
        });
        raw[MON_CHECKSUM..MON_CHECKSUM + 2].copy_from_slice(&sum.to_le_bytes());
        for (i, w) in secure.chunks(4).enumerate() {
            let v = u32::from_le_bytes(w.try_into().unwrap()) ^ key;
            raw[MON_SECURE + 4 * i..MON_SECURE + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        self.emu.write_bytes(addr, &raw);
    }

    /// Adds `qty` of `item` to bag pocket `pocket` (an owned stack, else the first free slot;
    /// quantities XOR the save key) and takes `cost` from the money: what a purchase at a Poké
    /// Mart leaves, which the oracle does not drive (`Step::Buy`). False (nothing written) when
    /// the money is short or the pocket full.
    pub fn buy(&mut self, pocket: Pocket, item: u16, qty: u16, cost: u32) -> Result<bool> {
        let sb1 = self.save_block1()?;
        let sb2 = self.save_block2()?;
        let key = self.emu.read_u32(sb2 + SB2_ENCRYPTION_KEY);
        let money = self.emu.read_u32(sb1 + SB1_MONEY) ^ key;
        let Some(left) = money.checked_sub(cost) else {
            return Ok(false);
        };
        let Some(i) = Pocket::ALL.iter().position(|&p| p == pocket) else {
            return Ok(false);
        };
        let (off, len) = SB1_POCKETS[i];
        let slots = self.emu.read_bytes(sb1 + off, 4 * len);
        let at = |s: &[u8]| u16::from_le_bytes([s[0], s[1]]);
        let n = |s: &[u8]| u16::from_le_bytes([s[2], s[3]]) ^ key as u16;
        let slot = (slots.chunks(4).position(|s| at(s) == item && n(s) != 0))
            .or_else(|| slots.chunks(4).position(|s| at(s) == 0 || n(s) == 0));
        let Some(slot) = slot else {
            return Ok(false);
        };
        let s = &slots[4 * slot..4 * slot + 4];
        let have = if at(s) == item { n(s) } else { 0 };
        let mut w = [0u8; 4];
        w[..2].copy_from_slice(&item.to_le_bytes());
        w[2..].copy_from_slice(&((have + qty) ^ key as u16).to_le_bytes());
        self.emu.write_bytes(sb1 + off + 4 * slot as u32, &w);
        self.emu
            .write_bytes(sb1 + SB1_MONEY, &(left ^ key).to_le_bytes());
        Ok(true)
    }

    /// Sets every party mon's HP to its max (the unencrypted party fields): the oracle's
    /// alignment after battles our side never fought.
    pub fn heal_party(&mut self) {
        let n = u32::from(self.emu.read_u8(self.sym.player_party_count)).min(6);
        for i in 0..n {
            let at = self.sym.player_party + i * MON_SIZE;
            let max = self.emu.read_u16(at + MON_MAX_HP as u32);
            self.emu.write_bytes(at + MON_HP as u32, &max.to_le_bytes());
        }
    }

    /// The live party (`gPlayerParty`, not the save block's copy).
    pub fn party(&mut self) -> Vec<Mon> {
        let n = u32::from(self.emu.read_u8(self.sym.player_party_count)).min(6);
        (0..n)
            .filter_map(|i| {
                let raw = self
                    .emu
                    .read_bytes(self.sym.player_party + i * MON_SIZE, MON_SIZE as usize);
                decode_mon(&raw)
            })
            .collect()
    }

    /// The current map (`gSaveBlock1Ptr->location`; the player's object event keeps the map
    /// it spawned on across connections).
    pub fn current_map(&mut self) -> Result<MapId> {
        let sb1 = self.save_block1()?;
        let loc = self.emu.read_bytes(sb1 + SB1_LOCATION, 2);
        Ok(MapId(u16::from(loc[0]) << 8 | u16::from(loc[1])))
    }

    /// The whole story state (errors when the save block pointers are not set up yet).
    pub fn state(&mut self) -> Result<State> {
        let sb1 = self.save_block1()?;
        let sb2 = self.save_block2()?;
        let key = self.emu.read_u32(sb2 + SB2_ENCRYPTION_KEY);
        let loc = self.emu.read_bytes(sb1 + SB1_LOCATION, 2);
        let map = MapId(u16::from(loc[0]) << 8 | u16::from(loc[1]));
        let heal = self.emu.read_bytes(sb1 + SB1_LAST_HEAL_LOCATION, 8);
        let flag_bytes = self.emu.read_bytes(sb1 + SB1_FLAGS, FLAGS_BYTES);
        let flags: Vec<u16> = (0..FLAGS_BYTES * 8)
            .filter(|&f| flag_bytes[f / 8] >> (f % 8) & 1 != 0)
            .map(|f| f as u16)
            .collect();
        let var_bytes = self.emu.read_bytes(sb1 + SB1_VARS, 2 * VARS_COUNT);
        let vars = (0..VARS_COUNT)
            .map(|i| {
                let v = u16::from_le_bytes([var_bytes[2 * i], var_bytes[2 * i + 1]]);
                (engine::flow::VARS_START + i as u16, v)
            })
            .filter(|&(_, v)| v != 0)
            .collect();
        let mut bag = Vec::new();
        for (p, (off, len)) in Pocket::ALL.into_iter().zip(SB1_POCKETS) {
            let b = self.emu.read_bytes(sb1 + off, 4 * len);
            for s in b.chunks(4) {
                let item = u16::from_le_bytes([s[0], s[1]]);
                let qty = u16::from_le_bytes([s[2], s[3]]) ^ key as u16;
                if item != 0 && qty != 0 {
                    bag.push((p, item, qty));
                }
            }
        }
        let objects: Vec<Object> = (0..OBJECT_EVENTS)
            .filter_map(|i| self.object_event(i))
            .collect();
        let player = self
            .player_object()
            .ok_or_else(|| anyhow!("no player object event"))?;
        let name: [u8; 8] = self.emu.read_bytes(sb2 + SB2_NAME, 8).try_into().unwrap();
        let badges = semantic::badges(|f| flags.binary_search(&f).is_ok());
        Ok(State {
            map,
            cell: player.cell,
            facing: player.facing,
            party: self.party(),
            bag,
            money: self.emu.read_u32(sb1 + SB1_MONEY) ^ key,
            coins: self.emu.read_u16(sb1 + SB1_COINS) ^ key as u16,
            objects,
            badges,
            respawn: (heal.iter().any(|&b| b != 0))
                .then(|| MapId(u16::from(heal[0]) << 8 | u16::from(heal[1]))),
            player_name: name,
            gender: self.emu.read_u8(sb2 + SB2_GENDER),
            trainer_id: self.emu.read_u32(sb2 + SB2_TRAINER_ID),
            mode: self.mode(),
            layout: self.emu.read_u16(sb1 + SB1_MAP_LAYOUT_ID),
            flags,
            vars,
        })
    }
}

impl State {
    /// The normalised view, comparable with our engine's [`Semantic::from_world`].
    pub fn semantic(&self) -> Semantic {
        let mut bag = BTreeMap::new();
        for &(p, item, qty) in &self.bag {
            *bag.entry((p, item)).or_insert(0) += qty;
        }
        Semantic {
            map: self.map,
            cell: self.cell,
            facing: self.facing,
            flags: Semantic::story_flags(self.flags.iter().copied()),
            vars: Semantic::story_vars(self.vars.iter().copied()),
            party: (self.party.iter())
                .map(|m| semantic::Mon {
                    species: m.species,
                    level: m.level,
                    exp: m.exp,
                    hp: m.hp,
                    max_hp: m.max_hp,
                    moves: m.moves,
                })
                .collect(),
            bag,
            money: self.money,
            badges: self.badges,
            respawn: self.respawn,
            objects: (self.objects.iter())
                .filter(|o| !o.is_player && o.visible && o.map == self.map)
                .map(|o| (o.local_id, o.cell))
                .collect(),
            homes: (self.objects.iter())
                .filter(|o| !o.is_player && o.visible && o.map == self.map)
                .map(|o| (o.local_id, o.home))
                .collect(),
            // Needs our data to tell an override ([`State::semantic_in`]).
            layout: None,
        }
    }

    /// [`State::semantic`] with the layout override (`explore::semantic::layout_override`).
    pub fn semantic_in<G: engine::data::GameData>(&self, game: &G) -> Semantic {
        Semantic {
            layout: semantic::layout_override(game, self.map, Some(self.layout)),
            ..self.semantic()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encrypts a mon the way `pokemon.c` does, so the decoder is checked without a ROM.
    fn encode(personality: u32, ot_id: u32, species: u16, moves: [u16; 4], exp: u32) -> Vec<u8> {
        let mut secure = [0u8; 48];
        let [g, a, _, _] = substruct_positions(personality).map(|p| 12 * p);
        secure[g..g + 2].copy_from_slice(&species.to_le_bytes());
        secure[g + 4..g + 8].copy_from_slice(&exp.to_le_bytes());
        secure[g + 9] = 70;
        for (i, m) in moves.iter().enumerate() {
            secure[a + 2 * i..a + 2 * i + 2].copy_from_slice(&m.to_le_bytes());
            secure[a + 8 + i] = 35;
        }
        let sum = (0..24).fold(0u16, |s, i| {
            s.wrapping_add(u16::from_le_bytes([secure[2 * i], secure[2 * i + 1]]))
        });
        let mut raw = vec![0u8; 100];
        raw[0..4].copy_from_slice(&personality.to_le_bytes());
        raw[4..8].copy_from_slice(&ot_id.to_le_bytes());
        raw[28..30].copy_from_slice(&sum.to_le_bytes());
        for (i, w) in secure.chunks(4).enumerate() {
            let v = u32::from_le_bytes(w.try_into().unwrap()) ^ personality ^ ot_id;
            raw[32 + 4 * i..36 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        raw[84] = 5;
        raw[86..88].copy_from_slice(&19u16.to_le_bytes());
        raw[88..90].copy_from_slice(&20u16.to_le_bytes());
        raw
    }

    #[test]
    fn mons_decrypt_in_every_substruct_order() {
        for p in 0..24u32 {
            let personality = 0x1234_5600 + p * 0x1_0001 + p;
            let raw = encode(personality, 0xBEEF_0042, 258, [33, 45, 0, 0], 135);
            let m = decode_mon(&raw).unwrap();
            assert!(m.checksum_ok, "order {}", personality % 24);
            assert_eq!(
                (
                    m.species,
                    m.moves,
                    m.exp,
                    m.level,
                    m.hp,
                    m.max_hp,
                    m.friendship
                ),
                (258, [33, 45, 0, 0], 135, 5, 19, 20, 70)
            );
            assert_eq!(m.pp, [35; 4]);
        }
        assert_eq!(decode_mon(&[0; 100]), None);
        let mut bad = encode(7, 9, 1, [1, 0, 0, 0], 0);
        bad[40] ^= 1;
        assert!(!decode_mon(&bad).unwrap().checksum_ok);
    }

    #[test]
    fn substruct_orders_are_permutations() {
        for p in 0..24 {
            let mut s = substruct_positions(p);
            s.sort();
            assert_eq!(s, [0, 1, 2, 3]);
        }
        assert_eq!(substruct_positions(0), [0, 1, 2, 3]); // GAEM
        assert_eq!(substruct_positions(23), [3, 2, 1, 0]); // MEAG
    }

    #[test]
    fn directions_map() {
        assert_eq!(dir(1), Dir::Down);
        assert_eq!(dir(2), Dir::Up);
        assert_eq!(dir(3), Dir::Left);
        assert_eq!(dir(4), Dir::Right);
        assert_eq!(dir(0x41), Dir::Down); // movementDirection in the high nibble
    }
}
