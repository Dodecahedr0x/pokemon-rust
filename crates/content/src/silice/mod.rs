//! Pokémon Silice (SIL1, `cfg(pkr_silice)`): the game root over the generated Silice tables.
//!
//! The build (`build/silice/`) compiles `games/silice` on the extract's base assets into the
//! same generated modules as Emerald (`crate::{consts, data, battle, scripts}`). The scripts are
//! `src/silice/scripts/<module>.rs`, each `pub async fn <Name>(ctx: Ctx)` a script
//! (`content::scripts::<module>::<Name>`, `ScriptId`s in name order), written against
//! [`prelude`] and naming texts, flags and maps by their generated constants only.
//!
//! **Task E** (this file, [`prelude`] and [`new_game`]): the new game, the system ids and the
//! heal locations come from `games/silice/game.ron` (`build/silice/game.rs`, as the `SIL_*` and
//! `HEAL_LOCATION_SIL_*` constants of `crate::consts`):
//! - the new game on `game.ron`'s start cell and facing (`data::NEW_GAME`), with its player
//!   name (`data::PLAYER_NAME`), running [`new_game::new_game`] (its flags and vars);
//! - running on `SIL_RUNNING_FLAG` (`FLAG_SIL_RUNNING_SHOES`), the map-change flags of
//!   `SIL_FLAGS_CLEARED_ON_MAP_CHANGE`, the respawn `SIL_RESPAWN`;
//! - heal location `n` is `SIL_HEAL_LOCATIONS[n - 1]`;
//! - no whiteout script and no wild-set var yet.
//!
//! **Task A** (`build/silice/{text,data,merge}.rs`): the texts of `text/fr.ron`, Silice's
//! trainers, wild encounters (by Silice map), items (with their base items' hold effects) and
//! marts (`data::MARTS`), the map headers and `LOCALID_*`, through the shared `game_data_tables!`.

pub mod mallette;
pub mod new_game;
pub mod prelude;

#[cfg(test)]
mod game_tests;
#[cfg(test)]
mod tests;

use crate::consts::{
    SIL_FLAGS_CLEARED_ON_MAP_CHANGE, SIL_HEAL_LOCATIONS, SIL_RESPAWN, SIL_RUNNING_FLAG,
    VAR_SIL_IDLE_FRAMES, VAR_SIL_INPUT_MAP,
};
use crate::{NEW_GAME_SCRIPT, data};
use engine::data::{FieldMove, FieldMoveGate, GameData, MapId, ScriptId, SpecialIds};
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
            flag_running: Some(SIL_RUNNING_FLAG),
            flags_cleared_on_map_change: SIL_FLAGS_CLEARED_ON_MAP_CHANGE,
            respawn_default: SIL_RESPAWN,
            var_wild_set: None,
            script_whiteout: None,
            // Reverse-City's inverted controls and its emitters' idle timer (Ch7).
            var_input_map: Some(VAR_SIL_INPUT_MAP),
            var_idle_frames: Some(VAR_SIL_IDLE_FRAMES),
            ..crate::gen3_special_ids()
        }
    }

    /// The Mallette's modules ([`mallette::field_move_gate`]).
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        mallette::field_move_gate(m)
    }

    fn field_move_label(&self, m: FieldMove) -> Option<&'static [u8]> {
        mallette::field_move_label(self.battle(), m)
    }

    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        mallette::field_move_script(m)
    }

    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        Some(data::script_ids::Mallette_CannotUseWaterfall)
    }

    fn dive_emerge_script(&self) -> Option<ScriptId> {
        Some(data::script_ids::Mallette_UseDiveUnderwater)
    }

    /// The new game: `game.ron`'s start, running [`new_game::new_game`].
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        if id == NEW_GAME_SCRIPT {
            return alloc::boxed::Box::pin(new_game::new_game(ctx));
        }
        data::script(id.0, ctx)
    }

    /// `game.ron`'s heal location `id` (numbered from 1).
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        SIL_HEAL_LOCATIONS
            .get(usize::from(id).checked_sub(1)?)
            .copied()
    }
}
