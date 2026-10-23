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
    engine::battle::{BattleGfx, Str},
    engine::data::{
        Behavior, Dir, Emote, Evolution, Font, GameData, GfxId, ItemId, ItemInfo, MapData, NewGame,
        SpecialIds, SpeciesId, SpriteData, TextId, TrainerId, UiText, WindowGfx,
    },
    engine::mon::{BattleTables, LandEncounters, TrainerData},
    engine::script::{Ctx, Script},
};

pub mod blob;
#[cfg(not(pkr_silice))]
mod new_game;
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
}

/// Generated scripts, one module per map file (`include!`d from the extracted dir).
pub mod scripts {
    include!(concat!(env!("OUT_DIR"), "/scripts.rs"));
}

pub use data::script_ids;

/// The build's script classification report (`build/scripts.rs`; Milestone 8 Task D2 fills it).
pub const REPORT: &str = include_str!(concat!(env!("OUT_DIR"), "/report.txt"));

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

#[cfg(not(pkr_silice))]
impl GameData for Game {
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
            var_wild_set: Some(consts::VAR_ALTERING_CAVE_WILD_SET),
            // `EventScript_WhiteOut`, once the scripts carry decomp names (Task D2).
            script_whiteout: data::SCRIPT_WHITEOUT,
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

    fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
        battle::wild_encounters(map.0, wild_set)
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

    fn build_id(&self) -> u64 {
        BUILD_ID
    }

    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        data::ITEMS.get(id.0 as usize)?.as_ref()
    }

    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        data::multichoice(list)
    }

    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        data::std_string(id)
    }

    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        data::emote(e)
    }

    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        battle::trainer(id.0)?;
        battle::TRAINER_MONEY.get(id.0 as usize).copied()
    }

    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        battle::evolutions(species.0)
    }

    fn hold_effect(&self, id: ItemId) -> Option<engine::mon::HoldEffect> {
        battle::hold_effect(id.0)
    }

    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        let (trainer, _) = scripts::trainer_script(script.0)?;
        Some(prelude::trainer_flag(TrainerId(trainer)))
    }

    fn trainer_battle_kind(&self, script: ScriptId) -> Option<u8> {
        scripts::trainer_script(script.0).map(|(_, kind)| kind)
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
