//! `msgbox` (Gen3 `data/scripts/std_msgbox.inc`).

use super::*;

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
        // `Std_MsgboxGetPoints`: the Battle Points fanfare (Gen3 `MUS_OBTAIN_B_POINTS`).
        MsgBox::GetPoints => {
            ctx.message(text).await;
            ctx.play_fanfare(MUS_OBTAIN_B_POINTS).await;
            ctx.wait_fanfare().await;
            ctx.wait_message().await;
        }
        // Pokenav is a Match Call window in Gen3.
        MsgBox::Pokenav => {
            ctx.message(text).await;
            ctx.wait_message().await;
        }
    }
}

/// Gen3 `MUS_OBTAIN_B_POINTS` (`include/constants/songs.h`).
pub const MUS_OBTAIN_B_POINTS: u16 = 459;
