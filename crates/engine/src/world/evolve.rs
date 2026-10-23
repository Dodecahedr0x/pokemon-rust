//! Evolution after a battle (Gen3 `TryEvolvePokemon` from `CB2_ReturnToField`; moved out of
//! `world/battle.rs` by Milestone 9 Wave 0, unchanged; Task E owns the file and replaces the
//! silent evolution with the scene).
//!
//! Two hooks, both called by `world/battle.rs`:
//! - [`after_win`] when a battle is won, with the battle still open (Wave 0: the silent level
//!   evolution of Milestone 8);
//! - [`after_battle`] when a won battle leaves, over the black screen and before the field is
//!   restored (Gen3 `FreeResetData_ReturnToOvOrDoEvolutions` → `TryEvolvePokemon`, for the
//!   mons in `Battle::leveled_up`): Task E opens the evolution scene there.

use super::State;
use super::battle::report;
use crate::battle::Battle;
use crate::data::{EvoMethod, GameData};
use crate::mon::HoldEffect;
use crate::script::Report;

/// After a win, each mon that gained a level evolves if a level evolution is reached (not with
/// an Everstone), without the scene (`Report::Degraded("evolution scene")`).
pub(super) fn after_win(game: &impl GameData, s: &mut State, battle: &Battle) {
    let t = game.battle();
    for i in 0..6 {
        if battle.leveled_up() & (1 << i) == 0 {
            continue;
        }
        let Some(mon) = s.party.get_mut(i) else {
            continue;
        };
        // GetEvolutionTargetSpecies: an Everstone (`HOLD_EFFECT_PREVENT_EVOLVE`) stops it.
        if mon.held.and_then(|id| game.hold_effect(id)) == Some(HoldEffect::PreventEvolve) {
            continue;
        }
        let evos = game.evolutions(mon.species);
        let target = mon.evolution_target(evos);
        if evos.iter().any(|e| e.method == EvoMethod::Friendship) {
            // EVO_FRIENDSHIP* (`friendship >= 220`) is not evaluated (M9).
            report(s, Report::Degraded("friendship evolution"));
        }
        let Some((target, ambiguous)) = target else {
            continue;
        };
        let Some(mon) = s.party.get_mut(i) else {
            continue;
        };
        mon.evolve(t, target);
        if ambiguous {
            report(s, Report::Degraded("evolution branch"));
        }
        report(s, Report::Degraded("evolution scene"));
    }
}

/// Over the black screen a won battle leaves (Gen3 `TryEvolvePokemon`): `true` when it opened
/// the evolution scene (`world/screens.rs::open`, which sets `Mode::Screens`); the battle then
/// ends without restoring the field, and the screens' host restores it when the last screen
/// closes. `false` returns to the field as before. Wave 0: `false` (the evolution happened in
/// [`after_win`]).
pub(super) fn after_battle(game: &impl GameData, s: &mut State, battle: &Battle) -> bool {
    let _ = (game, s, battle);
    false
}
