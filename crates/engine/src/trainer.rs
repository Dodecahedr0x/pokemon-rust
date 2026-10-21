//! Trainer parties. **Task C.**

use crate::mon::{BattleTables, Party, TrainerData};
use crate::rng::Rng;

/// Gen3 `CreateNPCTrainerParty`: name hash over the trainer and species names, personality
/// 0x80 / 0x78 / 0x88 + `hash << 8`, `fixedIV = iv * 31 / 255`, OT id from `random32()`,
/// custom movesets. Task C.
#[allow(unused_variables)]
pub fn create_trainer_party(t: &BattleTables, trainer: &TrainerData, rng: &mut Rng) -> Party {
    todo!("Task C: CreateNPCTrainerParty")
}
