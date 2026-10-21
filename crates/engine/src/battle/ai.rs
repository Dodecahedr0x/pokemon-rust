//! Opponent move choice.
//!
//! Task D ships [`choose`] as the wild-random stub (`OpponentHandleChooseMove`: `Random() & 3`
//! until a non-empty slot with PP); **Task E2** (Wave 2) adds the trainer score model and
//! `AI_FirstBattle` (`battle_ai_script_commands.c`, `data/battle_ai_scripts.s`), reading
//! `Battle::trainer_ai` and the battlers.
//!
//! Gen3's first battle (`BATTLE_TYPE_FIRST_BATTLE`, Birch's rescue) is wild, but its opponent
//! goes through the trainer AI branch of `OpponentHandleChooseMove`, with `aiFlags =
//! AI_SCRIPT_FIRST_BATTLE` (`BattleAI_SetupAIData`), not the wild random choice: E2 must branch on
//! `BattleKind::Wild { first_battle: true }` too. The stub below draws as a wild battle.

use super::core::Battle;
use super::{Choice, Side};
use crate::mon::BattleTables;
use crate::rng::Rng;

/// The opponent's action for this turn, chosen when the turn starts (Gen3 asks the opponent's
/// controller during action selection, before the player's choice resolves). The core only
/// asks when a move is usable; it maps `Choice::Run` to fleeing (`B_ACTION_RUN`) and
/// `Choice::Switch` to a switch.
pub fn choose(battle: &Battle, t: &BattleTables, rng: &mut Rng) -> Choice {
    let _ = t;
    let m = battle.mon(Side::Opponent);
    if (0..4).all(|i| m.moves[i].is_none() || m.pp[i] == 0) {
        return Choice::Move(0);
    }
    loop {
        // OpponentHandleChooseMove; a slot without PP is refused by
        // TrySetCantSelectMoveBattleScript and chosen again.
        let slot = (rng.random() & 3) as usize;
        if m.moves[slot].is_some() && m.pp[slot] > 0 {
            return Choice::Move(slot as u8);
        }
    }
}
