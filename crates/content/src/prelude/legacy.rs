//! The Milestone 4 `trainerbattle` helper the legacy script generator
//! (`extract::codegen::generate`) emits, named `trainer_battle`/`TrainerBattle` until Milestone 8
//! (whose `prelude::trainer` takes those names). Milestone 8 Task D2 deletes this module with the
//! legacy generator.

use super::*;

/// A `trainerbattle` (`include/constants/battle_setup.h` types 0/5, 1/2 and 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LegacyTrainerBattle {
    pub trainer: TrainerId,
    /// Local id of the trainer's object (Gen3 reveals it; unused here).
    pub local: u16,
    /// `None` for `TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT`.
    pub intro: Option<TextId>,
    pub defeat: TextId,
    /// `releaseall` after the battle (`EventScript_EndTrainerBattle` with no continue script).
    pub release: bool,
}

impl LegacyTrainerBattle {
    /// `TRAINER_BATTLE_SINGLE` / `_REMATCH`.
    pub const fn single(trainer: TrainerId, local: u16, intro: TextId, defeat: TextId) -> Self {
        LegacyTrainerBattle {
            trainer,
            local,
            intro: Some(intro),
            defeat,
            release: true,
        }
    }

    /// `TRAINER_BATTLE_CONTINUE_SCRIPT` (`_NO_MUSIC`): the script continues after a win.
    pub const fn continue_script(
        trainer: TrainerId,
        local: u16,
        intro: TextId,
        defeat: TextId,
    ) -> Self {
        LegacyTrainerBattle {
            trainer,
            local,
            intro: Some(intro),
            defeat,
            release: false,
        }
    }

    /// `TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT`.
    pub const fn no_intro(trainer: TrainerId, defeat: TextId) -> Self {
        LegacyTrainerBattle {
            trainer,
            local: 0,
            intro: None,
            defeat,
            release: false,
        }
    }
}

/// `trainerbattle`, as `data/scripts/trainer_battle.inc`. With an intro
/// (`EventScript_TryDoNormalTrainerBattle`): lock, face the player, and if the trainer is
/// already beaten return false (the script goes on with its post-battle code); else the intro
/// message (left open: the battle transition starts over it, as in Gen3), the battle, and true
/// (`gotobeatenscript`: `releaseall` for a single battle; a continue script jumps to its label).
/// Without (`EventScript_DoNoIntroTrainerBattle`): the battle, then true. The trainer flag is
/// set unless the player was defeated (`CB2_EndTrainerBattle`: `IsPlayerDefeated` is true for
/// `B_OUTCOME_LOST` and `B_OUTCOME_DREW`, which white out; otherwise
/// `SetBattledTrainersFlags`); the reveal movement and the trainer music are not modelled.
pub async fn legacy_trainer_battle(ctx: &Ctx, b: LegacyTrainerBattle) -> bool {
    let flag = trainer_flag(b.trainer);
    if let Some(intro) = b.intro {
        ctx.lock().await;
        ctx.face_player().await;
        if ctx.flag(flag).await {
            return false;
        }
        ctx.message(intro).await;
        ctx.wait_message().await;
        ctx.wait_button().await;
    }
    let outcome = ctx.battle_trainer(b.trainer, b.defeat).await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(flag, true).await;
    }
    if b.release {
        ctx.release_all().await;
    }
    true
}
