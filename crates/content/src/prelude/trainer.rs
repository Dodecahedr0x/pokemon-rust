//! Trainer battles (owned by Milestone 8 Task I): every `trainerbattle` type
//! (`include/constants/battle_setup.h` `TRAINER_BATTLE_*`), as Gen3 `data/scripts/trainer_battle.inc`.

use super::*;
use engine::script::{Request, Response};

/// Gen3 `TRAINER_FLAGS_START`: trainer `id` is beaten while flag `0x500 + id` is set.
pub const TRAINER_FLAGS_START: u16 = 0x500;

/// Gen3 `TRAINER_BATTLE_*` (`include/constants/battle_setup.h`).
pub mod kind {
    pub const SINGLE: u8 = 0;
    pub const CONTINUE_SCRIPT_NO_MUSIC: u8 = 1;
    pub const CONTINUE_SCRIPT: u8 = 2;
    pub const SINGLE_NO_INTRO_TEXT: u8 = 3;
    pub const DOUBLE: u8 = 4;
    pub const REMATCH: u8 = 5;
    pub const CONTINUE_SCRIPT_DOUBLE: u8 = 6;
    pub const REMATCH_DOUBLE: u8 = 7;
    pub const CONTINUE_SCRIPT_DOUBLE_NO_MUSIC: u8 = 8;
    pub const PYRAMID: u8 = 9;
    pub const SET_TRAINER_A: u8 = 10;
    pub const SET_TRAINER_B: u8 = 11;
    pub const HILL: u8 = 12;
    /// Every kind with its decomp name, in value order.
    pub const ALL: [(u8, &str); 13] = [
        (SINGLE, "TRAINER_BATTLE_SINGLE"),
        (
            CONTINUE_SCRIPT_NO_MUSIC,
            "TRAINER_BATTLE_CONTINUE_SCRIPT_NO_MUSIC",
        ),
        (CONTINUE_SCRIPT, "TRAINER_BATTLE_CONTINUE_SCRIPT"),
        (SINGLE_NO_INTRO_TEXT, "TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT"),
        (DOUBLE, "TRAINER_BATTLE_DOUBLE"),
        (REMATCH, "TRAINER_BATTLE_REMATCH"),
        (
            CONTINUE_SCRIPT_DOUBLE,
            "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE",
        ),
        (REMATCH_DOUBLE, "TRAINER_BATTLE_REMATCH_DOUBLE"),
        (
            CONTINUE_SCRIPT_DOUBLE_NO_MUSIC,
            "TRAINER_BATTLE_CONTINUE_SCRIPT_DOUBLE_NO_MUSIC",
        ),
        (PYRAMID, "TRAINER_BATTLE_PYRAMID"),
        (SET_TRAINER_A, "TRAINER_BATTLE_SET_TRAINER_A"),
        (SET_TRAINER_B, "TRAINER_BATTLE_SET_TRAINER_B"),
        (HILL, "TRAINER_BATTLE_HILL"),
    ];
}

/// The flag of a beaten trainer.
pub const fn trainer_flag(trainer: TrainerId) -> FlagId {
    FlagId(TRAINER_FLAGS_START + trainer.0)
}

/// One `trainerbattle` site: its type ([`kind`], 0..=12) and operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrainerBattle {
    pub kind: u8,
    pub trainer: u16,
    pub local_id: u8,
    pub intro: Option<TextId>,
    pub lose: TextId,
    /// The "not enough Pokémon" text of the double kinds (Gen3 `sTrainerCannotBattleSpeech`).
    pub not_enough: Option<TextId>,
    /// The continue script of the `CONTINUE_SCRIPT` kinds (Gen3 `sTrainerABattleScriptRetAddr`).
    pub event_script: Option<ScriptId>,
    /// Whether the kind plays the trainer's encounter music (no audio yet).
    pub music: bool,
}

const fn is_double(k: u8) -> bool {
    matches!(
        k,
        kind::DOUBLE | kind::CONTINUE_SCRIPT_DOUBLE | kind::CONTINUE_SCRIPT_DOUBLE_NO_MUSIC
    )
}

const fn continues(k: u8) -> bool {
    matches!(
        k,
        kind::CONTINUE_SCRIPT
            | kind::CONTINUE_SCRIPT_NO_MUSIC
            | kind::CONTINUE_SCRIPT_DOUBLE
            | kind::CONTINUE_SCRIPT_DOUBLE_NO_MUSIC
    )
}

/// The Gen3 `BATTLE_TYPE_*` flags a battle of `trainerbattle` kind `k` has as the engine runs
/// it: (exact, simplified). The double kinds are double battles (Wave G); the set-trainer and
/// facility kinds run no battle.
pub const fn battle_types_of_kind(k: u8) -> (&'static [&'static str], &'static [&'static str]) {
    const TRAINER: &[&str] = &["BATTLE_TYPE_IS_MASTER", "BATTLE_TYPE_TRAINER"];
    const TRAINER_DOUBLE: &[&str] = &[
        "BATTLE_TYPE_IS_MASTER",
        "BATTLE_TYPE_TRAINER",
        "BATTLE_TYPE_DOUBLE",
    ];
    match k {
        kind::PYRAMID | kind::HILL => (&[], &[]),
        kind::SET_TRAINER_A | kind::SET_TRAINER_B => (&[], &[]),
        k if is_double(k) || k == kind::REMATCH_DOUBLE => (TRAINER_DOUBLE, &[]),
        _ => (TRAINER, &[]),
    }
}

/// The flags of the battles scripts start without `trainerbattle`, as the engine runs them:
/// (exact, simplified). Birch's rescue is `BattleKind::Wild { first_battle: true }`; Wally's
/// tutorial (`SPECIAL_StartWallyTutorialBattle`) is skipped (`Degraded`).
pub const WILD_BATTLE_TYPES: (&[&str], &[&str]) = (
    &["BATTLE_TYPE_IS_MASTER", "BATTLE_TYPE_FIRST_BATTLE"],
    &["BATTLE_TYPE_WALLY_TUTORIAL"],
);

/// The flags of the battles neither a `trainerbattle` kind nor a wild battle starts (Wave G):
/// (exact, simplified). Two trainers seeing the player at once fight together
/// (`BATTLE_TYPE_TWO_OPPONENTS`, trainer sight); Steven's multi battle
/// (`SPECIAL_DoSpecialTrainerBattle`, `SPECIAL_BATTLE_STEVEN`) adds the in-game partner.
pub const OTHER_BATTLE_TYPES: (&[&str], &[&str]) = (
    &[
        "BATTLE_TYPE_TWO_OPPONENTS",
        "BATTLE_TYPE_MULTI",
        "BATTLE_TYPE_INGAME_PARTNER",
    ],
    &[],
);

/// Every Gen3 `BATTLE_TYPE_*` flag the engine's battles model as Gen3 does (the parity ledger,
/// `cargo xtask ledger`): [`battle_types_of_kind`] over every kind, [`WILD_BATTLE_TYPES`] and
/// [`OTHER_BATTLE_TYPES`] (`battle_type_tables_agree_with_the_kinds`). The other flags (link,
/// the facilities, the legendaries, roamers, Safari, …) are missing.
pub const BATTLE_TYPES_HANDLED: &[&str] = &[
    "BATTLE_TYPE_DOUBLE",
    "BATTLE_TYPE_FIRST_BATTLE",
    "BATTLE_TYPE_INGAME_PARTNER",
    "BATTLE_TYPE_IS_MASTER",
    "BATTLE_TYPE_MULTI",
    "BATTLE_TYPE_TRAINER",
    "BATTLE_TYPE_TWO_OPPONENTS",
];

/// The `BATTLE_TYPE_*` flags the engine runs in a simplified way (see [`BATTLE_TYPES_HANDLED`]).
pub const BATTLE_TYPES_DEGRADED: &[&str] = &["BATTLE_TYPE_WALLY_TUTORIAL"];

/// `trainerbattle`: runs the type's `data/scripts/trainer_battle.inc` path and returns whether
/// the calling script goes on with the code after the `trainerbattle` (Gen3
/// `gotopostbattlescript`, `true`) or stops there (`false`). Generated code is only
/// `if !trainer_battle(&ctx, b).await { return Flow::End; }`.
///
/// First, for the regular kinds 0–8, `TrainerApproach { obj: local_id }`: when the trainer saw
/// the player (the engine's sight), the engine walks it up and answers `Unit`, and the script
/// takes Gen3 `EventScript_StartTrainerApproach` (no lock, face or flag check: sight checked the
/// flag) straight to the intro and the battle. Otherwise the engine answers `Value(n)`, the
/// player's usable mons, and the talk path below runs.
///
/// Gen3, per kind when talked to (`BattleSetup_ConfigureTrainerBattle`, battle_setup.c):
/// - 0 `SINGLE`, 1 `CONTINUE_SCRIPT_NO_MUSIC`, 2 `CONTINUE_SCRIPT` (and 9 `PYRAMID`, 12 `HILL`
///   with facility trainers): `EventScript_TryDoNormalTrainerBattle`. Lock, face the player; if
///   its flag is set, `gotopostbattlescript` (true). Else the intro message (left open: the
///   transition starts over it), the battle, then `gotobeatenscript`.
/// - 3 `SINGLE_NO_INTRO_TEXT`: `EventScript_DoNoIntroTrainerBattle`. The battle, then
///   `gotopostbattlescript` (true).
/// - 4 `DOUBLE`, 6 `CONTINUE_SCRIPT_DOUBLE`, 8 `CONTINUE_SCRIPT_DOUBLE_NO_MUSIC`:
///   `EventScript_TryDoDoubleTrainerBattle`. As 0–2, but with fewer than two usable mons
///   (`HasEnoughMonsForDoubleBattle`) `EventScript_NotEnoughMonsForDoubleBattle`: the
///   `not_enough` text, `waitbuttonpress`, `release; end` (false). With two or more it is a
///   double battle (the trainer data's `doubleBattle`, Wave G).
/// - 5 `REMATCH`, 7 `REMATCH_DOUBLE`: `EventScript_TryDo(Double)RematchBattle`. Unless
///   `IsTrainerReadyForRematch` (never: no Match Call yet), `gotopostbattlescript` (true).
/// - 10 `SET_TRAINER_A`, 11 `SET_TRAINER_B`: only load operands and return a NULL script: the
///   calling script stops (false), reported `Unsupported`.
/// - 9 and 12 report `Unsupported` and take the post-battle path (true).
///
/// After the battle the trainer flag is set unless the player lost or drew
/// (`CB2_EndTrainerBattle`: those white out). `gotobeatenscript` then jumps to `event_script`
/// for the continue-script kinds (the calling script ends where it ends), else
/// `EventScript_TryGetTrainerScript`: `releaseall; end` (false). The music and the reveal
/// movement of disguised trainers are not modelled. A second trainer that saw the player
/// (`Request::TrainerPair`, Gen3 `TryPrepareSecondApproachingTrainer`) runs its own script up
/// to its intro, then the first one's battle fights both (`BATTLE_TYPE_TWO_OPPONENTS`).
pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) -> bool {
    match b.kind {
        kind::PYRAMID | kind::HILL => {
            ctx.report(Report::Unsupported("trainerbattle (facility)"))
                .await;
            return true;
        }
        kind::SET_TRAINER_A | kind::SET_TRAINER_B => {
            ctx.report(Report::Unsupported("trainerbattle (set trainer)"))
                .await;
            return false;
        }
        0..=8 => {}
        _ => {
            ctx.report(Report::Unsupported("trainerbattle (unknown type)"))
                .await;
            return true;
        }
    }
    let answer = ctx
        .request(Request::TrainerApproach { obj: b.local_id })
        .await;
    let approached = matches!(answer, Response::Unit);
    let trainer = TrainerId(b.trainer);
    if !approached {
        match b.kind {
            kind::SINGLE_NO_INTRO_TEXT => {
                battle(ctx, trainer, b.lose).await;
                return true;
            }
            kind::REMATCH | kind::REMATCH_DOUBLE => return true,
            _ => {}
        }
        ctx.lock().await;
        ctx.face_player().await;
        if ctx.flag(trainer_flag(trainer)).await {
            return true;
        }
        let usable = match answer {
            Response::Value(n) => n,
            _ => 0,
        };
        if is_double(b.kind) && usable < 2 {
            if let Some(t) = b.not_enough {
                ctx.message(t).await;
                ctx.wait_message().await;
                ctx.wait_button().await;
            }
            ctx.release().await;
            return false;
        }
    }
    // `EventScript_ShowTrainerIntroMsg`.
    if let Some(intro) = b.intro {
        ctx.message(intro).await;
        ctx.wait_message().await;
        ctx.wait_button().await;
    }
    // `TryPrepareSecondApproachingTrainer`: a second trainer that saw the player walks up and
    // speaks (its own script, up to here), then this battle fights both.
    if approached {
        let pair = Request::TrainerPair {
            trainer,
            defeat: b.lose,
        };
        match ctx.request(pair).await {
            Response::Value(second) => crate::data::script(second, ctx.clone()).await,
            // This is the second trainer's script: back to the first one's.
            Response::Bool(true) => return false,
            _ => {}
        }
    }
    battle(ctx, trainer, b.lose).await;
    // `gotobeatenscript`.
    if continues(b.kind) {
        match b.event_script {
            Some(id) => crate::data::script(id.0, ctx.clone()).await,
            None => {
                ctx.report(Report::Degraded("trainerbattle continue script"))
                    .await;
                ctx.release_all().await;
            }
        }
        return false;
    }
    ctx.release_all().await;
    false
}

/// `dotrainerbattle` and `CB2_EndTrainerBattle`'s flag, then `RegisterTrainerInMatchCall`
/// (Wave C): with Match Call, a beaten trainer of `gRematchTable` is registered.
async fn battle(ctx: &Ctx, trainer: TrainerId, lose: TextId) {
    let outcome = ctx.battle_trainer(trainer, lose).await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(trainer_flag(trainer), true).await;
        super::register_in_match_call(ctx, trainer).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::std::collections::BTreeSet;

    #[test]
    fn battle_type_tables_agree_with_the_kinds() {
        let mut exact: BTreeSet<&str> = WILD_BATTLE_TYPES.0.iter().copied().collect();
        let mut simplified: BTreeSet<&str> = WILD_BATTLE_TYPES.1.iter().copied().collect();
        exact.extend(OTHER_BATTLE_TYPES.0);
        simplified.extend(OTHER_BATTLE_TYPES.1);
        for (k, _) in kind::ALL {
            let (e, s) = battle_types_of_kind(k);
            exact.extend(e);
            simplified.extend(s);
        }
        assert_eq!(exact, BATTLE_TYPES_HANDLED.iter().copied().collect());
        assert_eq!(simplified, BATTLE_TYPES_DEGRADED.iter().copied().collect());
    }
}
