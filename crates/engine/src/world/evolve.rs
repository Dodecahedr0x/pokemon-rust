//! Evolution after a battle (Gen3 `TryEvolvePokemon` from `CB2_ReturnToField`; moved out of
//! `world/battle.rs` by Milestone 9 Wave 0, unchanged; Task E owns the file and replaces the
//! silent evolution with the scene).

use super::State;
use super::battle::report;
use crate::battle::Battle;
use crate::data::{EvoMethod, GameData};
use crate::script::Report;

/// After a win, each mon that gained a level evolves if a level evolution is reached, without
/// the scene (`Report::Degraded("evolution scene")`).
pub(super) fn after_win(game: &impl GameData, s: &mut State, battle: &Battle) {
    let t = game.battle();
    for i in 0..6 {
        if battle.leveled_up() & (1 << i) == 0 {
            continue;
        }
        let Some(mon) = s.party.get_mut(i) else {
            continue;
        };
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
