//! Trainer battles (owned by Milestone 8 Task I): every `trainerbattle` type
//! (`include/constants/battle_setup.h` `TRAINER_BATTLE_*`), as Gen3 `data/scripts/trainer_battle.inc`.

use super::*;

/// Gen3 `TRAINER_FLAGS_START`: trainer `id` is beaten while flag `0x500 + id` is set.
pub const TRAINER_FLAGS_START: u16 = 0x500;

/// The flag of a beaten trainer.
pub const fn trainer_flag(trainer: TrainerId) -> FlagId {
    FlagId(TRAINER_FLAGS_START + trainer.0)
}

/// One `trainerbattle` site: its type (`TRAINER_BATTLE_*`, 0..=12) and operands. The nine
/// regular types are 0–8 (single, continue-script ± music, no-intro, double, rematch,
/// continue-script double ± music, rematch double); the facility ones 9–12 (Pyramid, set trainer
/// A/B, Hill) report `Unsupported`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrainerBattle {
    pub kind: u8,
    pub trainer: u16,
    pub local_id: u8,
    pub intro: Option<TextId>,
    pub lose: TextId,
    /// The "not enough Pokémon" text of the double kinds.
    pub not_enough: Option<TextId>,
    /// The continue script of the `CONTINUE_SCRIPT` kinds.
    pub event_script: Option<ScriptId>,
    /// Whether the `_MUSIC` variant plays the trainer's encounter music.
    pub music: bool,
}

/// `trainerbattle`. Wave 0: the regular kinds start the battle (no intro, no flag), the
/// facility kinds report `Unsupported`. Task I implements `trainer_battle.inc`.
pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) {
    if b.kind > 8 {
        ctx.report(Report::Unsupported("trainerbattle (facility)"))
            .await;
        return;
    }
    ctx.battle_trainer(TrainerId(b.trainer), b.lose).await;
}
