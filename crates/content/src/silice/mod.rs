//! Pokémon Silice (SIL1, `cfg(pkr_silice)`): the game root over the generated Silice tables.
//!
//! The build (`build/silice/`) compiles `games/silice` on the extract's base assets into the
//! same generated modules as Emerald (`crate::{consts, data, battle, scripts}`). The scripts are
//! `src/silice/scripts/<module>.rs`, each `pub async fn <Name>(ctx: Ctx)` a script
//! (`content::scripts::<module>::<Name>`, `ScriptId`s in name order), written against
//! [`prelude`] and naming texts, flags and maps by their generated constants only.
//!
//! **Wave 0 stub** (Task E owns this file, [`prelude`] and [`new_game`]): the new game on the
//! player's bedroom (`data::NEW_GAME`), no story state, the Gen3 system ids; every heal
//! location and the whiteout respawn lead to the new-game cell; no wild encounters (Task A merges
//! Silice's).

pub mod new_game;
pub mod prelude;

#[cfg(test)]
mod tests;

use crate::{BUILD_ID, CLEARED_ON_MAP_CHANGE, NEW_GAME_SCRIPT, battle, consts, data, scripts};
use engine::battle::{BattleGfx, Str};
use engine::data::{
    Behavior, Dir, Emote, Evolution, FlagId, Font, GameData, GfxId, ItemId, ItemInfo, MapData,
    MapId, NewGame, ScriptId, SpecialIds, SpeciesId, SpriteData, TextId, TrainerId, UiText,
    WindowGfx,
};
use engine::mon::{BattleTables, LandEncounters, TrainerData};
use engine::script::{Ctx, Script};

/// Silice's content root.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Game;

impl Game {
    /// What the shells play (the same as `Game::default()`).
    pub const fn for_shells() -> Game {
        Game
    }
}

impl GameData for Game {
    fn map(&self, id: MapId) -> &'static MapData {
        data::map(id.0).unwrap_or(&data::MISSING_MAP)
    }

    fn has_map(&self, id: MapId) -> bool {
        crate::has_map(id)
    }

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            var_facing: Some(consts::VAR_FACING),
            var_last_talked: Some(consts::VAR_LAST_TALKED),
            flag_running: Some(consts::FLAG_SYS_B_DASH),
            flags_cleared_on_map_change: &CLEARED_ON_MAP_CHANGE,
            // Any id: `heal_location` answers the new-game cell for all (Task E).
            respawn_default: 0,
            var_wild_set: None,
            script_whiteout: None,
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
            return alloc::boxed::Box::pin(new_game::new_game(ctx));
        }
        data::script(id.0, ctx)
    }

    /// The player's bedroom, facing down (`build/silice/mod.rs` `START`).
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

    fn heal_location(&self, _id: u8) -> Option<(MapId, i16, i16)> {
        let (map, x, y) = data::NEW_GAME;
        Some((map, x, y))
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

    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        let (trainer, _) = scripts::trainer_script(script.0)?;
        Some(crate::prelude::trainer_flag(TrainerId(trainer)))
    }

    fn trainer_battle_kind(&self, script: ScriptId) -> Option<u8> {
        scripts::trainer_script(script.0).map(|(_, kind)| kind)
    }
}
