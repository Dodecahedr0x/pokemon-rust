#![cfg_attr(not(test), no_std)]
//! The game: all Hoenn-specific data and scripts. Depends on `engine`, never the reverse.
//!
//! `build/main.rs` compiles `content/extracted/` into `consts` (Gen3 constants, text ids),
//! `data` (static tables), `battle` (battle tables, strings and graphics, with a byte blob:
//! [`blob`]) and `scripts` (the generated per-map script modules). Without an extracted dir it
//! builds a small built-in world and sets `cfg(pkr_stub)`; without battle data (the stub, test
//! fixtures) it uses built-in battle data and sets `cfg(pkr_stub_battle)`.

extern crate alloc;

use engine::battle::{BattleGfx, Str};
use engine::data::{
    Behavior, Dir, FlagId, Font, GameData, GfxId, MapData, MapId, NewGame, ScriptId, SpecialIds,
    SpriteData, TextId, TrainerId, UiText, WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Script};

pub mod blob;
mod new_game;
pub mod prelude;

/// Gen3 constants (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
/// `OBJ_EVENT_GFX_*`, `SPECIES_*`, `MOVE_*`, `TRAINER_*`, `HEAL_LOCATION_*`) and one `TextId` per
/// text, named after its key.
#[allow(non_upper_case_globals, dead_code, unused_imports)]
pub mod consts {
    use engine::data::{FlagId, MapId, MoveId, SpeciesId, TextId, TrainerId, VarId};
    include!(concat!(env!("OUT_DIR"), "/consts.rs"));
}

/// Battle tables (`TABLES`), trainers, encounters, heal locations, battle strings and
/// graphics (`GFX`), and the slice (`STARTERS`, `SLICE_SPECIES`, `SLICE_MOVES`).
#[allow(non_upper_case_globals, dead_code, clippy::all)]
pub mod battle {
    include!(concat!(env!("OUT_DIR"), "/battle.rs"));
}

/// Static tables: tilesets (`TS_*`), maps (`M_*`), sprites (`S_*`), font, window, texts.
#[allow(non_upper_case_globals, non_snake_case, dead_code, clippy::all)]
pub mod data {
    include!(concat!(env!("OUT_DIR"), "/data.rs"));
}

/// Generated scripts, one module per map file (`include!`d from the extracted dir).
pub mod scripts {
    include!(concat!(env!("OUT_DIR"), "/scripts.rs"));
}

pub use data::script_ids;

/// Whether the content was built without an extracted dir (the small built-in world).
pub const STUB: bool = cfg!(pkr_stub);

/// Script id of the new-game script ([`new_game::new_game`]); never a generated id.
pub const NEW_GAME_SCRIPT: ScriptId = ScriptId(u16::MAX);

/// Flags Gen3 `ClearTempFieldEventData` clears on every map change.
static CLEARED_ON_MAP_CHANGE: [FlagId; 5] = [
    consts::FLAG_SYS_ENC_UP_ITEM,
    consts::FLAG_SYS_ENC_DOWN_ITEM,
    consts::FLAG_SYS_USE_STRENGTH,
    consts::FLAG_SYS_CTRL_OBJ_DELETE,
    consts::FLAG_NURSE_UNION_ROOM_REMINDER,
];

/// Where a new game starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Start {
    /// Littleroot after meeting the rival, sent to find Birch on Route 101: no Pokémon yet.
    #[default]
    BeforeRescue,
    /// After the rescue and the lab, with Mudkip (level 5), sent to meet the rival on Route
    /// 103 (the Milestone 3 start).
    AfterStarter,
}

/// The game's content root.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Emerald {
    pub start: Start,
}

impl Emerald {
    pub const fn before_rescue() -> Emerald {
        Emerald {
            start: Start::BeforeRescue,
        }
    }

    pub const fn after_starter() -> Emerald {
        Emerald {
            start: Start::AfterStarter,
        }
    }
}

/// The game as the shells and the parity route run it: [`Start::AfterStarter`] until the
/// integration (Milestone 4 Task J) moves them to `Emerald::default()`.
#[allow(non_upper_case_globals)]
pub const Emerald: Emerald = Emerald::after_starter();

/// Whether `id` is an exported map. `GameData::map` gives [`data::MISSING_MAP`] (0x0 cells,
/// no events) for the others, e.g. warp destinations outside the slice.
pub fn has_map(id: MapId) -> bool {
    data::map(id.0).is_some()
}

impl GameData for Emerald {
    fn map(&self, id: MapId) -> &'static MapData {
        data::map(id.0).unwrap_or(&data::MISSING_MAP)
    }

    fn has_map(&self, id: MapId) -> bool {
        has_map(id)
    }

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_facing: Some(consts::VAR_FACING),
            var_last_talked: Some(consts::VAR_LAST_TALKED),
            flag_running: Some(consts::FLAG_SYS_B_DASH),
            flags_cleared_on_map_change: &CLEARED_ON_MAP_CHANGE,
            // `InsideOfTruck_EventScript_SetIntroFlagsMale`.
            respawn_default: consts::HEAL_LOCATION_LITTLEROOT_TOWN_BRENDANS_HOUSE_2F,
        }
    }

    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        data::sprite(gfx.0)
    }

    fn player_sprite(&self) -> &'static SpriteData {
        data::PLAYER_SPRITE
    }

    fn font(&self) -> &'static Font {
        &data::FONT
    }

    fn window(&self) -> &'static WindowGfx {
        &data::WINDOW
    }

    fn text(&self, id: TextId) -> &'static [u8] {
        data::TEXTS.get(id.0 as usize).copied().unwrap_or(&[0xFF])
    }

    fn ui_text(&self, t: UiText) -> &'static [u8] {
        match t {
            UiText::Yes => data::UI_YES,
            UiText::No => data::UI_NO,
            UiText::RivalName => data::UI_RIVAL_NAME,
        }
    }

    fn behavior(&self, raw: u8) -> Behavior {
        data::BEHAVIORS[raw as usize]
    }

    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == NEW_GAME_SCRIPT {
            return alloc::boxed::Box::pin(new_game::new_game(ctx, self.start));
        }
        data::script(id.0, ctx)
    }

    /// Littleroot, outside Brendan's house, facing down (see [`new_game`]).
    fn new_game(&self) -> NewGame {
        let (map, x, y) = data::NEW_GAME;
        NewGame {
            map,
            x,
            y,
            facing: Dir::Down,
            script: Some(NEW_GAME_SCRIPT),
            player_name: data::PLAYER_NAME,
        }
    }

    fn battle(&self) -> &'static BattleTables {
        &battle::TABLES
    }

    fn encounters(&self, map: MapId) -> Option<&'static LandEncounters> {
        battle::encounters(map.0)
    }

    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        battle::trainer(id.0)
    }

    fn battle_text(&self, s: Str) -> &'static [u8] {
        battle::text(s)
    }

    fn battle_gfx(&self) -> &'static BattleGfx {
        &battle::GFX
    }

    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        battle::heal_location(id)
    }
}

#[cfg(test)]
mod data_tests;
