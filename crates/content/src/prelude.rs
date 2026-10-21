//! What generated scripts (`content/extracted/scripts/<Map>.rs`) may use: the engine's script
//! types, every Gen3 constant (`FLAG_*`, `VAR_*`, `MAP_*`, `SPECIAL_*`, `STD_*`, `ITEM_*`,
//! `OBJ_EVENT_GFX_*`), every text id, and the Gen3 std-script helpers below.

pub use crate::consts::*;
pub use alloc::boxed::Box;
pub use core::cmp::Ordering;
pub use engine::data::{Dir, FlagId, MapId, MovementType, PLAYER, TextId, VarId};
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

/// `special <id>` / `specialvar`: logs the special's name and returns 0. No special the slice
/// reaches needs more yet: every value-returning one it uses (`ShouldTryRematchBattle`,
/// `IsTrainerRegistered`, `HasAllHoennMons`) means "no" at 0.
pub async fn special(ctx: &Ctx, id: u16) -> u16 {
    ctx.log(name_of(&crate::data::SPECIAL_NAMES, id).unwrap_or("SPECIAL_?"))
        .await;
    0
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
