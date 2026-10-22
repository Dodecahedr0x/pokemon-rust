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

/// `trainerbattle`: runs the type's `data/scripts/trainer_battle.inc` path and returns whether
/// the calling script goes on with the code after the `trainerbattle` (Gen3
/// `gotopostbattlescript`, `true`) or stops there (`false`). Generated code is only
/// `if !trainer_battle(&ctx, b).await { return Flow::End; }`: every kind's flag, rematch, double
/// and continue-script logic lives here (owned by Task I).
///
/// Gen3, per kind (`BattleSetup_ConfigureTrainerBattle`, battle_setup.c; the script paths in
/// trainer_battle.inc):
/// - 0 `SINGLE`, 1 `CONTINUE_SCRIPT`, 2 `CONTINUE_SCRIPT_NO_MUSIC` (and 9 `PYRAMID`, 12 `HILL`
///   with facility trainers): `EventScript_TryDoNormalTrainerBattle`. Lock, face the player and
///   reveal the trainer; if its flag is set, `gotopostbattlescript` (true). Else the intro
///   message (left open: the transition starts over it), the battle, then `gotobeatenscript`:
///   the continue script `event_script` for 1/2, else `EventScript_TryGetTrainerScript`
///   (rematch registration, then `releaseall; end`). Either way the calling script stops.
/// - 3 `SINGLE_NO_INTRO_TEXT`: `EventScript_DoNoIntroTrainerBattle`. Reveal, the battle, then
///   `gotopostbattlescript` (true).
/// - 4 `DOUBLE`, 6 `CONTINUE_SCRIPT_DOUBLE`, 7 `CONTINUE_SCRIPT_DOUBLE_NO_MUSIC`:
///   `EventScript_TryDoDoubleTrainerBattle`. As 0/1/2, but with fewer than two usable mons the
///   `not_enough` text, `release; end` (false).
/// - 5 `REMATCH`, 8 `REMATCH_DOUBLE`: `EventScript_TryDo(Double)RematchBattle`. Unless
///   `IsTrainerReadyForRematch`, `gotopostbattlescript` (true); else the intro, the rematch
///   party (`GetRematchTrainerId`), `releaseall; end` (false). No trainer-flag check.
/// - 10 `SET_TRAINER_A`, 11 `SET_TRAINER_B`: only load the operands for a later battle and
///   return a NULL script, so the calling script stops (false).
///
/// After any battle the trainer flag is set unless the player lost or drew
/// (`CB2_EndTrainerBattle`: those white out).
///
/// Wave 0 default (today's behaviour, Task I replaces it): the regular kinds as above with the
/// trainer flag; no reveal movement, music, second approaching trainer or two-mon check (the
/// double kinds battle as singles); rematches are never ready (no Match Call yet), so 5 and 8
/// take the post-battle path; after a continue-script kind's battle `event_script` is not run
/// (reported `Degraded`) and the script stops. Kinds 9–12 report `Unsupported`: 9 and 12 take
/// the post-battle path, 10 and 11 stop.
pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) -> bool {
    match b.kind {
        9 | 12 => {
            ctx.report(Report::Unsupported("trainerbattle (facility)"))
                .await;
            return true;
        }
        10 | 11 => {
            ctx.report(Report::Unsupported("trainerbattle (set trainer)"))
                .await;
            return false;
        }
        5 | 8 => return true,
        _ => {}
    }
    let trainer = TrainerId(b.trainer);
    let flag = trainer_flag(trainer);
    if b.kind != 3 {
        ctx.lock().await;
        ctx.face_player().await;
        if ctx.flag(flag).await {
            return true;
        }
        if let Some(intro) = b.intro {
            ctx.message(intro).await;
            ctx.wait_message().await;
            ctx.wait_button().await;
        }
    }
    let outcome = ctx.battle_trainer(trainer, b.lose).await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(flag, true).await;
    }
    match b.kind {
        3 => true,
        1 | 2 | 6 | 7 => {
            ctx.report(Report::Degraded("trainerbattle continue script"))
                .await;
            ctx.release_all().await;
            false
        }
        _ => {
            ctx.release_all().await;
            false
        }
    }
}
