//! `callstd` (Gen3 `gStdScripts`).

use super::*;

/// `callstd <std>` other than the msgbox pattern. The item stds run Task H's helpers on
/// `VAR_0x8000` (item) and `VAR_0x8001` (amount), as Gen3's `giveitem`/`finditem` set them
/// (the lowering calls the helpers directly; this covers a computed std). Everything else is
/// logged.
pub async fn callstd(ctx: &Ctx, std: u8) {
    ctx.log(name_of(&crate::data::STD_NAMES, std.into()).unwrap_or("STD_?"))
        .await;
    let item = || async { (ctx.var(VAR_0x8000).await, ctx.var(VAR_0x8001).await) };
    match std {
        STD_OBTAIN_ITEM => {
            let (i, q) = item().await;
            give_item(ctx, i, q).await;
        }
        STD_FIND_ITEM => {
            let (i, q) = item().await;
            find_item(ctx, i, q).await;
        }
        _ => {}
    }
}
