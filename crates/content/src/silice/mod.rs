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

pub mod new_game;
pub mod prelude;

#[cfg(test)]
mod game_tests;
#[cfg(test)]
mod tests;

use crate::consts::{FLAG_SIL_MODULE_CASCADE, FLAG_SIL_MODULE_PLONGEE, FLAG_SIL_MODULE_SURF};
use crate::consts::{
    SIL_FLAGS_CLEARED_ON_MAP_CHANGE, SIL_HEAL_LOCATIONS, SIL_RESPAWN, SIL_RUNNING_FLAG,
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
            ..crate::gen3_special_ids()
        }
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

    /// The Mallette's water modules (SIL4 Ch9): usable while the module's flag is set, whatever
    /// the party knows. Surf is the Hydro-Propulseur (Ch4), Waterfall the Turbine à Flux (Ch7),
    /// Dive the Sonde de Pression (Ch8). The other field abilities stay scripts (Cut, Strength,
    /// Flash and Rock Smash are their obstacles' own scripts; no fly map yet).
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        match m {
            FieldMove::Surf => FieldMoveGate::Flag(FLAG_SIL_MODULE_SURF),
            FieldMove::Waterfall => FieldMoveGate::Flag(FLAG_SIL_MODULE_CASCADE),
            FieldMove::Dive => FieldMoveGate::Flag(FLAG_SIL_MODULE_PLONGEE),
            _ => FieldMoveGate::Never,
        }
    }

    /// `scripts/field_moves.rs`'s `EventScript_Use*`.
    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        match m {
            FieldMove::Surf => data::SCRIPT_USE_SURF,
            FieldMove::Waterfall => data::SCRIPT_USE_WATERFALL,
            FieldMove::Dive => data::SCRIPT_USE_DIVE,
            _ => None,
        }
    }

    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        data::SCRIPT_CANNOT_USE_WATERFALL
    }

    fn dive_emerge_script(&self) -> Option<ScriptId> {
        data::SCRIPT_USE_DIVE_UNDERWATER
    }

    /// The map's dive or emerge connection, keeping the cell, unless that cell is outside the
    /// connected map (Route S9's conduit runs on south of the lake: its exit's arrow warps
    /// would surface the player off Route S9; Gen3 would warp there); else [`DIVE_WARPS`].
    fn dive_warp(
        &self,
        map: MapId,
        emerge: bool,
        (x, y): (i16, i16),
    ) -> Option<(MapId, Option<(i16, i16)>)> {
        if let Some(to) = data::dive_connection(map.0, emerge) {
            let m = self.map(to);
            let inside = (0..i32::from(m.width)).contains(&i32::from(x))
                && (0..i32::from(m.height)).contains(&i32::from(y));
            return inside.then_some((to, None));
        }
        let &(.., to, tx, ty) = DIVE_WARPS.iter().find(|w| {
            let (from, up, (x0, y0), (x1, y1)) = (w.0, w.1, w.2, w.3);
            from == map && up == emerge && (x0..=x1).contains(&x) && (y0..=y1).contains(&y)
        })?;
        Some((to, Some((tx, ty))))
    }
}

/// The fixed dive warps (Gen3 `setdivewarp` in a map's `on_dive_warp` script, for a map without
/// a dive connection): (map, emerge, the cells' corners, inclusive, destination map and cell).
/// The maps' `on_dive_warp` scripts (`Nebula_OnDiveWarp`, `Nebula_TourHall_B1F_OnDiveWarp`)
/// point here; the engine asks [`GameData::dive_warp`] with the player's cell.
pub const DIVE_WARPS: &[(MapId, bool, (i16, i16), (i16, i16), MapId, i16, i16)] = &[
    // Ch9: the Bassin de Captage dives under the Tower, into B1F's cuve.
    (
        crate::consts::MAP_NEBULA,
        false,
        (50, 10),
        (52, 13),
        crate::consts::MAP_NEBULA_TOUR_HALL_B1F,
        6,
        10,
    ),
    // Ch9: the quai's deep water dives back into the conduit's exit (the way back to Route S9).
    (
        crate::consts::MAP_NEBULA,
        false,
        (29, 54),
        (30, 55),
        crate::consts::MAP_ROUTE_S09_UNDERWATER,
        39,
        72,
    ),
    // Ch9: B1F's cuve dives back to the Bassin (the way back to the town).
    (
        crate::consts::MAP_NEBULA_TOUR_HALL_B1F,
        false,
        (5, 9),
        (7, 11),
        crate::consts::MAP_NEBULA,
        51,
        11,
    ),
];
