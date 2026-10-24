#![cfg_attr(not(test), no_std)]
//! The game: all Hoenn-specific data and scripts. Depends on `engine`, never the reverse.
//!
//! `build/main.rs` compiles `content/extracted/` into `consts` (Gen3 constants, text ids),
//! `data` (static tables), `battle` (battle tables, strings and graphics, with a byte blob:
//! [`blob`]) and `scripts` (the generated per-map script modules). Without an extracted dir it
//! builds a small built-in world and sets `cfg(pkr_stub)`; without battle data (the stub, test
//! fixtures) it uses built-in battle data and sets `cfg(pkr_stub_battle)`.
//!
//! **Flavours** ([`FLAVOR`], SIL1): the stub, Emerald, and Silice (`PKR_GAME=silice`,
//! `cfg(pkr_silice)`: the authored `games/silice` on the extract's base assets, with the
//! game root and scripts of [`silice`]). Every flavour has the same API shape: `consts`, `data`
//! (with the script dispatch), `battle`, `scripts`, `overrides`, `prelude`, [`Game`] (the
//! [`GameData`] root; [`Emerald`] is its old name) and [`BUILD_ID`]. Emerald-only items
//! ([`Start`], `Game::{before_rescue, after_starter}`) are `cfg(not(pkr_silice))`.

extern crate alloc;

use engine::data::{FlagId, MapId, ScriptId};
#[cfg(not(pkr_silice))]
use {
    engine::data::{GameData, SpecialIds},
    engine::script::{Ctx, Script},
};

pub mod blob;
#[cfg(not(pkr_silice))]
mod emerald_story;
#[cfg(not(pkr_silice))]
mod new_game;
#[cfg(not(pkr_silice))]
mod step_count;
/// Hand-written script overrides (Milestone 8): a script the lowering cannot express as Gen3
/// does, replaced by hand. The build digests each override's source into the script's
/// `SCRIPT_DIGESTS` entry, and fails on an override naming no script of the IR.
///
/// Convention: one file per script, `src/overrides/<Name>.rs`, holding
/// `pub async fn <Name>(ctx: Ctx)` (the decomp script name), using `crate::prelude::*`; texts and
/// movements are referred to by their generated ids, never by ROM text or data
/// (`tests/m8_e_overrides.rs`). The build declares each applied one (`$OUT_DIR/overrides.rs`,
/// `build/scripts.rs`) and lists it in `OVERRIDES`.
pub mod overrides {
    include!(concat!(env!("OUT_DIR"), "/overrides.rs"));
}
pub mod prelude;
pub mod samples;
#[cfg(pkr_silice)]
pub mod silice;

/// Gen3 constants (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
/// `OBJ_EVENT_GFX_*`, `SPECIES_*`, `MOVE_*`, `TRAINER_*`, `HEAL_LOCATION_*`,
/// `MOVEMENT_TYPE_*`) and one `TextId` per text, named after its key.
#[allow(non_upper_case_globals, dead_code, unused_imports)]
pub mod consts {
    // A glob: the `MOVEMENT_TYPE_*` values (`classify::movement`) name whatever engine types
    // they need (`Dir`, and `Dirs` once Task I lands); the generated names shadow it.
    use engine::data::*;
    include!(concat!(env!("OUT_DIR"), "/consts.rs"));
}

/// Battle tables (`TABLES`), trainers, encounters (every wild header), evolutions, trainer
/// money, heal locations, battle strings and graphics (`GFX`, with every species and trainer
/// pic the extract has), the starters and `SUMMARY`.
#[allow(non_upper_case_globals, dead_code, clippy::all)]
pub mod battle {
    include!(concat!(env!("OUT_DIR"), "/battle.rs"));
}

/// Static tables: tilesets (`TS_*`), maps (`M_*`), sprites (`S_*`), font, window, texts, items,
/// multichoice lists, std strings, map sections and emotes; their bulk data is in the `DATA`
/// blob.
#[allow(non_upper_case_globals, non_snake_case, dead_code, clippy::all)]
pub mod data {
    include!(concat!(env!("OUT_DIR"), "/data.rs"));
    include!(concat!(env!("OUT_DIR"), "/dispatch.rs"));
    include!(concat!(env!("OUT_DIR"), "/ui.rs"));
}

/// Generated scripts, one module per map file (`include!`d from the extracted dir).
pub mod scripts {
    include!(concat!(env!("OUT_DIR"), "/scripts.rs"));
}

pub use data::script_ids;

/// The build's report: the script classification (`build/scripts.rs`), then the data
/// approximations (`build/codegen.rs`).
pub const REPORT: &str = include_str!(concat!(env!("OUT_DIR"), "/report.txt"));

/// The lowering's sites per command, special and script module, its reports and stale
/// `specialvar` sites (`build/parity.rs`), for the parity ledger (`cargo xtask ledger`); empty
/// for the stub and Silice.
pub const PARITY: &str = include_str!(concat!(env!("OUT_DIR"), "/parity.tsv"));

/// Which game this content build is (`build/main.rs`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Flavor {
    /// No usable extracted dir: the small built-in world (`cfg(pkr_stub)`).
    Stub,
    /// Pokémon Emerald from the extract (the default).
    Emerald,
    /// Pokémon Silice: `games/silice` on the extract's base assets (`PKR_GAME=silice`,
    /// `cfg(pkr_silice)`).
    Silice,
}

impl Flavor {
    pub const ALL: [Flavor; 3] = [Flavor::Stub, Flavor::Emerald, Flavor::Silice];

    /// `stub`, `emerald` or `silice`: the directory name of the flavour's checkpoint tree
    /// (`checkpoints/<name>`) and explorer db (`target/explore/<name>`).
    pub const fn name(self) -> &'static str {
        match self {
            Flavor::Stub => "stub",
            Flavor::Emerald => "emerald",
            Flavor::Silice => "silice",
        }
    }
}

/// This build's flavour.
pub const FLAVOR: Flavor = if cfg!(pkr_stub) {
    Flavor::Stub
} else if cfg!(pkr_silice) {
    Flavor::Silice
} else {
    Flavor::Emerald
};
const _: () = assert!(!(cfg!(pkr_stub) && cfg!(pkr_silice)));

/// Whether the content was built without an extracted dir (the small built-in world):
/// `FLAVOR == Flavor::Stub`.
pub const STUB: bool = matches!(FLAVOR, Flavor::Stub);

/// Whether an extracted dir exists but in an older format, so the content fell back to the
/// stub world ([`STUB`] is set too). It still builds so that `cargo xtask extract` (which links
/// this crate) can run and replace it; everything else that needs the real data should refuse
/// it with [`STALE_EXTRACT_MSG`].
pub const STALE_EXTRACT: bool = cfg!(pkr_stale_extract);
const _: () = assert!(!STALE_EXTRACT || STUB);

/// The error for a [`STALE_EXTRACT`] build.
pub const STALE_EXTRACT_MSG: &str = "content/extracted/ is in an older format, so the content \
    built the stub world: run `cargo xtask extract` to refresh it";

/// Identity of this content build (`build/build_id.rs`): the content sources plus the extracted
/// dir, or `stub`. Never 0 (`TreeHash::finish`). [`GameData::build_id`] returns it.
pub const BUILD_ID: u64 = include!(concat!(env!("OUT_DIR"), "/build_id.rs"));

/// Script id of the new-game script (each flavour's `new_game::new_game`); never a generated
/// id.
pub const NEW_GAME_SCRIPT: ScriptId = ScriptId(u16::MAX);

/// Gen3 `MAX_MONEY` (`include/constants/global.h`).
pub const MAX_MONEY: u32 = 999_999;
/// Gen3 `MULTI_B_PRESSED` (`include/constants/script_menu.h`): a multichoice's answer to B.
pub const MULTI_B_PRESSED: u16 = 127;
/// Gen3 `BAG_ITEMS_COUNT`, `BAG_KEYITEMS_COUNT`, `BAG_POKEBALLS_COUNT`, `BAG_TMHM_COUNT` and
/// `BAG_BERRIES_COUNT` (`include/constants/global.h`), in `Pocket::ALL` order.
pub const POCKET_SLOTS: [u8; 5] = [30, 30, 16, 64, 46];

/// The game's content root under its old name (kept for one milestone, SIL1).
pub type Emerald = Game;

#[cfg(pkr_silice)]
pub use silice::Game;

/// Flags Gen3 `ClearTempFieldEventData` clears on every map change.
pub(crate) static CLEARED_ON_MAP_CHANGE: [FlagId; 5] = [
    consts::FLAG_SYS_ENC_UP_ITEM,
    consts::FLAG_SYS_ENC_DOWN_ITEM,
    consts::FLAG_SYS_USE_STRENGTH,
    consts::FLAG_SYS_CTRL_OBJ_DELETE,
    consts::FLAG_NURSE_UNION_ROOM_REMINDER,
];

/// Where a new game starts (the default: [`Start::BeforeRescue`]).
#[cfg(not(pkr_silice))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Start {
    /// Littleroot after meeting the rival, sent to find Birch on Route 101: no Pokémon yet.
    #[default]
    BeforeRescue,
    /// After the rescue and the lab, with Mudkip (level 5), sent to meet the rival on Route
    /// 103 (the Milestone 3 start).
    AfterStarter,
}

/// The game's content root (Emerald and the stub; the Silice flavour's is `silice::Game`).
/// `Game::default()` starts [`Start::BeforeRescue`]; the shells and the parity route run
/// [`Game::for_shells`]. [`Game::after_starter`] is the Milestone 3 start, kept for fast tests.
#[cfg(not(pkr_silice))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Game {
    pub start: Start,
}

#[cfg(not(pkr_silice))]
impl Game {
    pub const fn before_rescue() -> Game {
        Game {
            start: Start::BeforeRescue,
        }
    }

    pub const fn after_starter() -> Game {
        Game {
            start: Start::AfterStarter,
        }
    }

    /// What the shells and the parity route play: [`Start::BeforeRescue`] on the extracted
    /// world, [`Start::AfterStarter`] on the stub world (no story, no Route 101), so that the
    /// stub route has a Pokémon to battle with in its tall grass.
    pub const fn for_shells() -> Game {
        if STUB {
            Game::after_starter()
        } else {
            Game::before_rescue()
        }
    }
}

/// Whether `id` is an exported map. `GameData::map` gives [`data::MISSING_MAP`] (0x0 cells,
/// no events) for the others, e.g. warp destinations outside the slice.
pub fn has_map(id: MapId) -> bool {
    data::map(id.0).is_some()
}

/// The [`SpecialIds`](engine::data::SpecialIds) every flavour shares (Gen3's system flags and
/// vars, which the shared prelude also uses). Each root's `special_ids` sets only its own
/// fields (respawn, wild-set var, whiteout script) and takes the rest from here
/// (`..gen3_special_ids()`), so a new shared field reaches every flavour; the placeholders below
/// for the per-flavour fields are always overridden.
pub(crate) fn gen3_special_ids() -> engine::data::SpecialIds {
    engine::data::SpecialIds {
        var_facing: Some(consts::VAR_FACING),
        var_last_talked: Some(consts::VAR_LAST_TALKED),
        flag_running: Some(consts::FLAG_SYS_B_DASH),
        flags_cleared_on_map_change: &CLEARED_ON_MAP_CHANGE,
        respawn_default: 0,
        var_wild_set: None,
        script_whiteout: None,
        max_money: MAX_MONEY,
        multichoice_cancel: MULTI_B_PRESSED,
        pocket_slots: POCKET_SLOTS,
        // Milestone 9.
        pc_boxes: PC_BOXES,
        pc_item_slots: PC_ITEM_SLOTS,
        max_coins: MAX_COINS,
        flag_badges: [
            Some(consts::FLAG_BADGE01_GET),
            Some(consts::FLAG_BADGE02_GET),
            Some(consts::FLAG_BADGE03_GET),
            Some(consts::FLAG_BADGE04_GET),
            Some(consts::FLAG_BADGE05_GET),
            Some(consts::FLAG_BADGE06_GET),
            Some(consts::FLAG_BADGE07_GET),
            Some(consts::FLAG_BADGE08_GET),
        ],
        var_repel_steps: Some(consts::VAR_REPEL_STEP_COUNT),
        var_friendship_steps: Some(consts::VAR_FRIENDSHIP_STEP_COUNTER),
        script_repel_wore_off: data::SCRIPT_REPEL_WORE_OFF,
        flag_pokedex: Some(consts::FLAG_SYS_POKEDEX_GET),
        var_pc_box_to_send_mon: Some(consts::VAR_PC_BOX_TO_SEND_MON),
        var_mon_box_id: Some(consts::VAR_MON_BOX_ID),
        var_mon_box_pos: Some(consts::VAR_MON_BOX_POS),
        flag_shown_box_was_full: Some(consts::FLAG_SHOWN_BOX_WAS_FULL_MESSAGE),
        var_daily_wilds: None,
        berry_trees: 0,
        var_input_map: None,
        var_idle_frames: None,
    }
}

/// PC boxes kept. Gen3 `TOTAL_BOXES_COUNT` is 14; Milestone 9 decision 5 keeps the most that
/// the 64 KiB flash slot holds full: the worst-case save is 9,748 bytes plus 1,950 per full box
/// against `MAX_ENCODED` 24,016, so 7.
pub const PC_BOXES: u8 = 7;
/// Gen3 `PC_ITEMS_COUNT`.
pub const PC_ITEM_SLOTS: u8 = 50;
/// Gen3 `MAX_COINS`.
pub const MAX_COINS: u16 = 9999;

/// The [`GameData`](engine::data::GameData) methods every flavour answers from its generated
/// tables (`data`, `battle`, `scripts`), defined once so the flavours cannot drift: a new
/// defaulted trait method that the tables can answer goes here, and both roots get it. Each
/// root adds only what differs: `special_ids` (over [`gen3_special_ids`]), `script` (its
/// new-game script) and
/// `heal_location`. `form_changes` keeps the trait default (no flavour has form-change data
/// yet; Task A's Silice data adds it here).
/// The flavour's START menu: Gen3's (`build/ui.rs`), or Silice's with the Mallette.
#[cfg(not(pkr_silice))]
fn flavor_start_menu() -> &'static [engine::data::StartItem] {
    data::START_MENU
}
#[cfg(pkr_silice)]
fn flavor_start_menu() -> &'static [engine::data::StartItem] {
    &silice::mallette::START_MENU
}

/// The flavour's multichoice lists: Gen3's, and Silice's Mallette lists.
#[cfg(not(pkr_silice))]
fn flavor_multichoice(list: u8) -> Option<&'static [&'static [u8]]> {
    data::multichoice(list)
}
#[cfg(pkr_silice)]
fn flavor_multichoice(list: u8) -> Option<&'static [&'static [u8]]> {
    silice::mallette::multichoice(list)
}

macro_rules! game_data_tables {
    () => {
        fn map(&self, id: ::engine::data::MapId) -> &'static ::engine::data::MapData {
            $crate::data::map(id.0).unwrap_or(&$crate::data::MISSING_MAP)
        }

        fn map_layout(
            &self,
            id: ::engine::data::MapId,
            layout: u16,
        ) -> Option<&'static ::engine::data::MapData> {
            $crate::data::map_layout(id.0, layout)
        }

        fn has_map(&self, id: ::engine::data::MapId) -> bool {
            $crate::has_map(id)
        }

        fn sprite(
            &self,
            gfx: ::engine::data::GfxId,
        ) -> Option<&'static ::engine::data::SpriteData> {
            $crate::data::sprite(gfx.0)
        }

        fn player_sprite(&self) -> &'static ::engine::data::SpriteData {
            $crate::data::PLAYER_SPRITE
        }

        fn font(&self) -> &'static ::engine::data::Font {
            &$crate::data::FONT
        }

        fn window(&self) -> &'static ::engine::data::WindowGfx {
            &$crate::data::WINDOW
        }

        fn text(&self, id: ::engine::data::TextId) -> &'static [u8] {
            // The start menu's labels (`build/ui.rs`) sit past every script text.
            if let Some(i) = id.0.checked_sub($crate::data::START_LABEL_BASE) {
                return $crate::data::START_LABELS
                    .get(i as usize)
                    .copied()
                    .unwrap_or(&[0xFF]);
            }
            $crate::data::TEXTS
                .get(id.0 as usize)
                .copied()
                .unwrap_or(&[0xFF])
        }

        fn ui_text(&self, t: ::engine::data::UiText) -> &'static [u8] {
            match t {
                ::engine::data::UiText::Yes => $crate::data::UI_YES,
                ::engine::data::UiText::No => $crate::data::UI_NO,
                ::engine::data::UiText::RivalName => $crate::data::UI_RIVAL_NAME,
                // Milestone 9 screen texts (Tasks X2 and C): empty until extracted.
                _ => &[0xFF],
            }
        }

        // ---- Milestone 9 (tables from `build/ui.rs`, empty until Task C) ----

        fn start_menu(&self) -> &'static [::engine::data::StartItem] {
            $crate::flavor_start_menu()
        }

        fn item_use(&self, id: ::engine::data::ItemId) -> ::engine::data::ItemUse {
            $crate::data::ITEM_USE
                .get(id.0 as usize)
                .copied()
                .unwrap_or(::engine::data::ItemUse::None)
        }

        fn item_description(&self, id: ::engine::data::ItemId) -> Option<&'static [u8]> {
            $crate::data::ITEM_DESCRIPTIONS
                .get(id.0 as usize)
                .copied()
                .flatten()
        }

        fn move_description(&self, id: ::engine::data::MoveId) -> Option<&'static [u8]> {
            $crate::data::MOVE_DESCRIPTIONS
                .get(id.0 as usize)
                .copied()
                .flatten()
        }

        fn tm_move(&self, id: ::engine::data::ItemId) -> Option<::engine::data::MoveId> {
            let tms = $crate::data::TM_MOVES;
            let i = tms.binary_search_by_key(&id.0, |&(item, _)| item).ok()?;
            Some(::engine::data::MoveId(tms[i].1))
        }

        fn can_learn_tm(
            &self,
            species: ::engine::data::SpeciesId,
            tm: ::engine::data::ItemId,
        ) -> bool {
            let tms = $crate::data::TM_MOVES;
            let Ok(bit) = tms.binary_search_by_key(&tm.0, |&(item, _)| item) else {
                return false;
            };
            let compat = $crate::data::TM_COMPAT;
            compat
                .binary_search_by_key(&species.0, |&(s, _)| s)
                .is_ok_and(|i| bit < 64 && compat[i].1 & (1 << bit) != 0)
        }

        fn ui_gfx(&self, g: ::engine::data::UiGfx) -> Option<&'static ::engine::data::UiPicture> {
            $crate::data::UI_GFX
                .iter()
                .find(|(k, _)| *k == g)
                .map(|(_, p)| p)
        }

        fn window_frame(&self, n: u8) -> Option<&'static ::engine::data::WindowGfx> {
            $crate::data::WINDOW_FRAMES.get(n as usize)
        }

        fn item_icon(&self, id: ::engine::data::ItemId) -> Option<&'static ::engine::data::Icon> {
            let icons = $crate::data::ITEM_ICONS;
            let i = icons.binary_search_by_key(&id.0, |&(k, _)| k).ok()?;
            Some(&icons[i].1)
        }

        fn mon_icon(
            &self,
            species: ::engine::data::SpeciesId,
        ) -> Option<&'static ::engine::data::Icon> {
            let icons = $crate::data::MON_ICONS;
            let i = icons.binary_search_by_key(&species.0, |&(k, _)| k).ok()?;
            Some(&icons[i].1)
        }

        fn in_game_trade(&self, n: u8) -> Option<&'static ::engine::data::InGameTrade> {
            $crate::data::IN_GAME_TRADES.get(n as usize)
        }

        fn nature_name(&self, n: u8) -> Option<&'static [u8]> {
            $crate::data::NATURE_NAMES.get(n as usize).copied()
        }

        fn behavior(&self, raw: u8) -> ::engine::data::Behavior {
            $crate::data::BEHAVIORS[raw as usize]
        }

        /// The flavour's start and facing (`data::NEW_GAME`), running its new-game script.
        fn new_game(&self) -> ::engine::data::NewGame {
            let (map, x, y, facing) = $crate::data::NEW_GAME;
            ::engine::data::NewGame {
                map,
                x,
                y,
                facing,
                script: Some($crate::NEW_GAME_SCRIPT),
                player_name: $crate::data::PLAYER_NAME,
            }
        }

        fn battle(&self) -> &'static ::engine::mon::BattleTables {
            &$crate::battle::TABLES
        }

        fn wild_encounters(
            &self,
            map: ::engine::data::MapId,
            wild_set: u16,
        ) -> Option<&'static ::engine::mon::LandEncounters> {
            $crate::battle::wild_encounters(map.0, wild_set)
        }

        fn water_encounters(
            &self,
            map: ::engine::data::MapId,
            wild_set: u16,
        ) -> Option<&'static ::engine::mon::WaterEncounters> {
            $crate::battle::water(map.0, wild_set)
        }

        fn rock_smash_encounters(
            &self,
            map: ::engine::data::MapId,
            wild_set: u16,
        ) -> Option<&'static ::engine::mon::WaterEncounters> {
            $crate::battle::rock_smash(map.0, wild_set)
        }

        fn trainer(
            &self,
            id: ::engine::data::TrainerId,
        ) -> Option<&'static ::engine::mon::TrainerData> {
            $crate::battle::trainer(id.0)
        }

        fn partner(&self) -> Option<&'static ::engine::mon::PartnerData> {
            $crate::battle::PARTNER.as_ref()
        }

        fn battle_text(&self, s: ::engine::battle::Str) -> &'static [u8] {
            $crate::battle::text(s)
        }

        fn battle_gfx(&self) -> &'static ::engine::battle::BattleGfx {
            &$crate::battle::GFX
        }

        fn build_id(&self) -> u64 {
            $crate::BUILD_ID
        }

        fn item(&self, id: ::engine::data::ItemId) -> Option<&'static ::engine::data::ItemInfo> {
            $crate::data::ITEMS.get(id.0 as usize)?.as_ref()
        }

        fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
            $crate::flavor_multichoice(list)
        }

        fn std_string(&self, id: u8) -> Option<&'static [u8]> {
            $crate::data::std_string(id)
        }

        fn emote(&self, e: ::engine::data::Emote) -> Option<&'static ::engine::data::SpriteData> {
            $crate::data::emote(e)
        }

        fn trainer_money(&self, id: ::engine::data::TrainerId) -> Option<u8> {
            $crate::battle::trainer(id.0)?;
            $crate::battle::TRAINER_MONEY.get(id.0 as usize).copied()
        }

        fn evolutions(
            &self,
            species: ::engine::data::SpeciesId,
        ) -> &'static [::engine::data::Evolution] {
            $crate::battle::evolutions(species.0)
        }

        fn hold_effect(&self, id: ::engine::data::ItemId) -> Option<::engine::mon::HoldEffect> {
            $crate::battle::hold_effect(id.0)
        }

        fn map_type(&self, map: ::engine::data::MapId) -> ::engine::data::MapType {
            $crate::data::map_type(map.0)
        }

        fn trainer_flag_of(
            &self,
            script: ::engine::data::ScriptId,
        ) -> Option<::engine::data::FlagId> {
            let (trainer, _) = $crate::scripts::trainer_script(script.0)?;
            Some($crate::prelude::trainer_flag(::engine::data::TrainerId(
                trainer,
            )))
        }

        fn trainer_battle_kind(&self, script: ::engine::data::ScriptId) -> Option<u8> {
            $crate::scripts::trainer_script(script.0).map(|(_, kind)| kind)
        }
    };
}
#[cfg_attr(not(pkr_silice), allow(unused_imports))] // the Silice root names it by path
pub(crate) use game_data_tables;

#[cfg(not(pkr_silice))]
impl GameData for Game {
    game_data_tables!();

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            // `InsideOfTruck_EventScript_SetIntroFlagsMale`.
            respawn_default: consts::HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F,
            var_wild_set: Some(consts::VAR_ALTERING_CAVE_WILD_SET),
            // `EventScript_WhiteOut`, once the scripts carry decomp names (Task D2).
            script_whiteout: data::SCRIPT_WHITEOUT,
            var_daily_wilds: Some(emerald_story::DAILY_WILDS),
            // Gen3 `BERRY_TREES_COUNT`.
            berry_trees: 128,
            ..gen3_special_ids()
        }
    }

    /// The new game: Littleroot, outside Brendan's house (see [`new_game`]).
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == NEW_GAME_SCRIPT {
            return alloc::boxed::Box::pin(new_game::new_game(ctx, self.start));
        }
        data::script(id.0, ctx)
    }

    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        battle::heal_location(id)
    }

    /// Gen3's HM moves (`sHMMoves`, `IsHMMove2`).
    fn is_hm_move(&self, m: engine::data::MoveId) -> bool {
        data::HM_MOVES.contains(&m)
    }

    /// The HMs' field moves and their badges (Gen3 `sFieldMoves`, the party menu's
    /// `SetUpFieldMove_*` and `GetInteractedWaterScript`: Cut needs badge 1, Flash 2, Rock
    /// Smash 3, Strength 4, Surf 5, Fly 6, Dive 7, Waterfall 8); the move is the HM's.
    fn field_move_gate(&self, m: engine::data::FieldMove) -> engine::data::FieldMoveGate {
        use engine::data::FieldMove as F;
        // (HM number - 1, badge - 1).
        let (hm, badge) = match m {
            F::Cut => (0, 0),
            F::Fly => (1, 5),
            F::Surf => (2, 4),
            F::Strength => (3, 3),
            F::Flash => (4, 1),
            F::RockSmash => (5, 2),
            F::Waterfall => (6, 7),
            F::Dive => (7, 6),
            _ => return engine::data::FieldMoveGate::Never,
        };
        engine::data::FieldMoveGate::PartyMove {
            move_id: data::HM_MOVES[hm],
            badge: Some(FlagId(consts::FLAG_BADGE01_GET.0 + badge)),
        }
    }

    fn field_move_script(&self, m: engine::data::FieldMove) -> Option<ScriptId> {
        match m {
            engine::data::FieldMove::Surf => data::SCRIPT_USE_SURF,
            engine::data::FieldMove::Waterfall => data::SCRIPT_USE_WATERFALL,
            engine::data::FieldMove::Dive => data::SCRIPT_USE_DIVE,
            _ => None,
        }
    }

    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        data::SCRIPT_CANNOT_USE_WATERFALL
    }

    fn dive_emerge_script(&self) -> Option<ScriptId> {
        data::SCRIPT_USE_DIVE_UNDERWATER
    }

    fn dive_warp(&self, map: MapId, emerge: bool) -> Option<(MapId, Option<(i16, i16)>)> {
        prelude::dive_warp(map, emerge)
    }

    fn fly_destination(&self, i: u8) -> Option<engine::data::FlyDestination> {
        prelude::fly_destination(i)
    }

    /// `Task_ReturnToItemListAfterItemPurchase`: 10 Poké Balls bought at once add a Premier
    /// Ball.
    fn mart_bonus(&self) -> Option<(engine::data::ItemId, u16, engine::data::ItemId)> {
        Some((
            engine::data::ItemId(consts::ITEM_POKE_BALL),
            10,
            engine::data::ItemId(consts::ITEM_PREMIER_BALL),
        ))
    }

    /// `TryPushBoulder`.
    fn pushable_boulder(&self) -> Option<(engine::data::GfxId, FlagId)> {
        Some((
            engine::data::GfxId(consts::OBJ_EVENT_GFX_PUSHABLE_BOULDER),
            consts::FLAG_SYS_USE_STRENGTH,
        ))
    }

    fn step_callback(&self, map: MapId) -> Option<engine::data::StepCallback> {
        prelude::step_callback(map)
    }

    fn hole_warp(&self, map: MapId) -> Option<MapId> {
        prelude::hole_warp(map)
    }

    fn fall_script(&self) -> Option<ScriptId> {
        data::SCRIPT_FALL_DOWN_HOLE
    }

    fn step_count_script(&self, step: engine::data::StepCount<'_>) -> Option<ScriptId> {
        step_count::step_count_script(self, step)
    }
}

/// The Emerald and stub tables and scripts (the Silice flavour's are `silice::tests`).
#[cfg(all(test, not(pkr_silice)))]
mod data_tests;

#[cfg(test)]
mod build_id_tests {
    use super::*;

    /// `crates/content/tests/build_modes.rs` reads the printed id of each build mode.
    #[test]
    fn build_id_is_set() {
        use engine::data::GameData;
        std::println!("CONTENT BUILD_ID={BUILD_ID:#018x}");
        assert_ne!(BUILD_ID, 0);
        assert_eq!(Game::for_shells().build_id(), BUILD_ID);
        assert_eq!(Emerald::default().build_id(), BUILD_ID);
    }

    #[test]
    fn flavor_names_and_stub() {
        let names: std::vec::Vec<_> = Flavor::ALL.iter().map(|f| f.name()).collect();
        assert_eq!(names, ["stub", "emerald", "silice"]);
        assert_eq!(STUB, FLAVOR == Flavor::Stub);
        assert_eq!(cfg!(pkr_silice), FLAVOR == Flavor::Silice);
    }
}
