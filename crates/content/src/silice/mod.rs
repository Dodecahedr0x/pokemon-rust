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

use crate::{NEW_GAME_SCRIPT, data};
use engine::data::{GameData, MapId, ScriptId, SpecialIds};
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
    crate::game_data_tables!();

    fn special_ids(&self) -> SpecialIds {
        SpecialIds {
            // Any id: `heal_location` answers the new-game cell for all (Task E).
            respawn_default: 0,
            var_wild_set: None,
            script_whiteout: None,
            ..crate::gen3_special_ids()
        }
    }

    /// The new game: the player's bedroom (`build/silice/mod.rs` `START`).
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == NEW_GAME_SCRIPT {
            return alloc::boxed::Box::pin(new_game::new_game(ctx));
        }
        data::script(id.0, ctx)
    }

    fn heal_location(&self, _id: u8) -> Option<(MapId, i16, i16)> {
        let (map, x, y) = data::NEW_GAME;
        Some((map, x, y))
    }
}
