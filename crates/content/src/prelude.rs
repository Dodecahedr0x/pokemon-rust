//! What generated scripts (`content/extracted/scripts/<Map>.rs`) may use: the engine's script
//! types, every Gen3 constant (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
//! `OBJ_EVENT_GFX_*`, `SPECIES_*`, `MOVE_*`, `TRAINER_*`, `HEAL_LOCATION_*`), every text id,
//! and the Gen3 std-script helpers below.

pub use crate::consts::*;
pub use alloc::boxed::Box;
pub use core::cmp::Ordering;
pub use engine::battle::BattleOutcome;
pub use engine::data::{
    Dir, FlagId, MapId, MoveId, MovementType, PLAYER, SpeciesId, TextId, TrainerId, VarId,
};
pub use engine::script::{Ctx, Emote, Fade, Speed, Step};

/// Gen3 `MSGBOX_*` kinds (`callstd MSGBOX_*` after `loadword 0, <text>`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MsgBox {
    Npc,
    Sign,
    Default,
    YesNo,
    Autoclose,
    GetPoints,
    Pokenav,
}

/// `msgbox <text>, <kind>`, as Gen3 `data/scripts/std_msgbox.inc` (and `Std_MsgboxAutoclose`
/// in `trainer_battle.inc`). `YesNo` stores the answer in `VAR_RESULT` (1 yes, 0 no or B), as
/// Gen3 `yesnobox` does.
pub async fn msgbox(ctx: &Ctx, text: TextId, kind: MsgBox) {
    match kind {
        MsgBox::Npc => {
            ctx.lock().await;
            ctx.face_player().await;
            ctx.say(text).await;
            ctx.release().await;
        }
        MsgBox::Sign => {
            ctx.lock_all().await;
            ctx.say(text).await;
            ctx.release_all().await;
        }
        MsgBox::Default => ctx.say(text).await,
        MsgBox::YesNo => {
            ctx.message(text).await;
            ctx.wait_message().await;
            let yes = ctx.yes_no().await;
            ctx.set_var(VAR_RESULT, yes as u16).await;
        }
        MsgBox::Autoclose => {
            ctx.say(text).await;
            ctx.release().await;
        }
        // GetPoints plays a fanfare (no sound yet); Pokenav is a Match Call window in Gen3.
        MsgBox::GetPoints | MsgBox::Pokenav => {
            ctx.message(text).await;
            ctx.wait_message().await;
        }
    }
}

fn name_of(table: &'static [(u16, &'static str)], id: u16) -> Option<&'static str> {
    table.iter().find(|&&(v, _)| v == id).map(|&(_, n)| n)
}

/// `callstd <std>` other than the msgbox pattern. Item stds act as if the bag had room
/// (`VAR_RESULT` = TRUE) without giving anything; `STD_FIND_ITEM` also removes the item ball
/// that was talked to. Everything else is logged.
pub async fn callstd(ctx: &Ctx, std: u8) {
    ctx.log(name_of(&crate::data::STD_NAMES, std.into()).unwrap_or("STD_?"))
        .await;
    match std {
        STD_OBTAIN_ITEM => ctx.set_var(VAR_RESULT, 1).await,
        STD_FIND_ITEM => {
            ctx.lock().await;
            ctx.face_player().await;
            ctx.set_var(VAR_RESULT, 1).await;
            let obj = ctx.var(VAR_LAST_TALKED).await;
            ctx.remove_object(obj as u8).await;
            ctx.release().await;
        }
        _ => {}
    }
}

/// `special <id>` / `specialvar`: logs the special's name, runs the ones below and returns 0.
/// Every value-returning special the slice uses (`ShouldTryRematchBattle`,
/// `IsTrainerRegistered`, `HasAllHoennMons`) means "no" at 0.
///
/// - `ChooseStarter`: Gen3 `ChooseStarter` + `CB2_GiveStarter` + `CB2_StartFirstBattle`: the
///   starter screen, `VAR_STARTER_MON` = the choice, the starter at level 5, then the first
///   battle (`battle_controllers.c`: Zigzagoon, level 2).
/// - `HealPlayerParty`: heals the party.
/// - `ChangePokemonNickname`: no naming screen, the name is kept; the field fades back in from
///   the script's `fadescreen FADE_TO_BLACK` as `CB2_ReturnToFieldContinueScriptPlayMapMusic`
///   does.
/// - `PlayerFaceTrainerAfterBattle`: cosmetic (the player turns to the trainer), only logged.
pub async fn special(ctx: &Ctx, id: u16) -> u16 {
    ctx.log(name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?"))
        .await;
    // Guards, not patterns: a mistyped constant would silently become a catch-all binding.
    match id {
        id if id == SPECIAL_ChooseStarter => {
            let starters = crate::battle::STARTERS;
            let i = ctx.choose_starter(starters).await;
            debug_assert!(i < 3, "starter index {i}");
            // `CB2_GiveStarter` stores the raw choice; `GetStarterPokemon` maps an out-of-range
            // one to the first starter.
            ctx.set_var(VAR_STARTER_MON, i.into()).await;
            let species = starters.get(usize::from(i)).unwrap_or(&starters[0]);
            ctx.give_mon(*species, 5).await;
            let (foe, level) = crate::battle::FIRST_BATTLE_FOE;
            ctx.battle_wild(foe, level, true).await;
        }
        id if id == SPECIAL_HealPlayerParty => ctx.heal_party().await,
        id if id == SPECIAL_ChangePokemonNickname => ctx.fade(Fade::FromBlack).await,
        _ => {}
    }
    0
}

/// Gen3 `TRAINER_FLAGS_START`: trainer `id` is beaten while flag `0x500 + id` is set.
pub const TRAINER_FLAGS_START: u16 = 0x500;

/// The flag of a beaten trainer.
pub const fn trainer_flag(trainer: TrainerId) -> FlagId {
    FlagId(TRAINER_FLAGS_START + trainer.0)
}

/// A `trainerbattle` (`include/constants/battle_setup.h` types 0/5, 1/2 and 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrainerBattle {
    pub trainer: TrainerId,
    /// Local id of the trainer's object (Gen3 reveals it; unused here).
    pub local: u16,
    /// `None` for `TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT`.
    pub intro: Option<TextId>,
    pub defeat: TextId,
    /// `releaseall` after the battle (`EventScript_EndTrainerBattle` with no continue script).
    pub release: bool,
}

impl TrainerBattle {
    /// `TRAINER_BATTLE_SINGLE` / `_REMATCH`.
    pub const fn single(trainer: TrainerId, local: u16, intro: TextId, defeat: TextId) -> Self {
        TrainerBattle {
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
        TrainerBattle {
            trainer,
            local,
            intro: Some(intro),
            defeat,
            release: false,
        }
    }

    /// `TRAINER_BATTLE_SINGLE_NO_INTRO_TEXT`.
    pub const fn no_intro(trainer: TrainerId, defeat: TextId) -> Self {
        TrainerBattle {
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
pub async fn trainer_battle(ctx: &Ctx, b: TrainerBattle) -> bool {
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

/// `checkplayergender`: Gen3 `MALE` (0). The player is SACHA (Brendan) and the rival is May.
pub async fn check_player_gender(_ctx: &Ctx) -> u16 {
    0
}

/// A script command the engine does not implement (`what` is its macro name): logged.
pub async fn unsupported(ctx: &Ctx, what: &'static str) {
    ctx.log("unsupported script command:").await;
    ctx.log(what).await;
}
