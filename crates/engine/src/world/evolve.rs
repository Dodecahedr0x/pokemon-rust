//! Evolution after a battle (Gen3 `TryEvolvePokemon` from `CB2_ReturnToField`).
//!
//! [`after_battle`] runs over the black screen a won battle leaves, before the field is
//! restored (`FreeResetData_ReturnToOvOrDoEvolutions`). Each mon in `Battle::leveled_up`
//! evolves when a level evolution is reached or its friendship is at least 220, unless it
//! holds an Everstone. The evolution scene plays them one after another; B can cancel
//! (`canStopEvo`).

use super::State;
use super::battle::report;
use crate::battle::Battle;
use crate::data::GameData;
use crate::mon::HoldEffect;
use crate::script::Report;
use crate::ui::evolution::EvoJob;
use crate::ui::{ScreenKind, ScreenState};
use crate::video::Frame;

/// Over the black screen a won battle leaves (Gen3 `TryEvolvePokemon`): `true` when it opened
/// the evolution scene. The battle then ends without restoring the field, and the screen host
/// restores it when the scene closes.
pub(super) fn after_battle(
    game: &impl GameData,
    s: &mut State,
    battle: &Battle,
    f: &mut Frame,
) -> bool {
    let mut jobs = [EvoJob {
        slot: 0,
        into: crate::data::SpeciesId(0),
        can_cancel: true,
    }; 6];
    let mut n = 0u8;
    for i in 0..6 {
        if battle.leveled_up() & (1 << i) == 0 {
            continue;
        }
        let Some(mon) = s.party.get(i) else {
            continue;
        };
        // GetEvolutionTargetSpecies: an Everstone (`HOLD_EFFECT_PREVENT_EVOLVE`) stops it.
        if mon.held.and_then(|id| game.hold_effect(id)) == Some(HoldEffect::PreventEvolve) {
            continue;
        }
        let evos = game.evolutions(mon.species);
        let Some((target, ambiguous)) = mon.evolution_target(evos) else {
            continue;
        };
        if ambiguous {
            // `EvoMethod::Level` still folds Gen3's stat and personality methods.
            report(s, Report::Degraded("evolution branch"));
        }
        jobs[n as usize] = EvoJob {
            slot: i as u8,
            into: target,
            can_cancel: true,
        };
        n += 1;
    }
    if n == 0 {
        return false;
    }
    let first = jobs[0];
    super::screens::open(
        s,
        game,
        ScreenKind::Evolution {
            slot: first.slot,
            into: first.into,
            can_cancel: first.can_cancel,
        },
        false,
        f,
    );
    if let super::mode::Mode::Screens(stack) = &mut s.mode
        && let Some(ScreenState::Evolution(scene)) = stack.top_mut()
    {
        scene.set_jobs(&jobs[..n as usize]);
    }
    true
}
