//! `callstd` (Gen3 `gStdScripts`).

use super::*;

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
