//! `callstd` (Gen3 `gStdScripts`, `data/event_scripts.s`; Milestone 8 Tasks H and J).

use super::*;

/// `callstd <std>` other than the msgbox pattern, every `gStdScripts` entry (the name is logged
/// first):
///
/// - `STD_OBTAIN_ITEM` / `STD_FIND_ITEM`: Task H's helpers ([`give_item`], [`find_item`]) on
///   `VAR_0x8000` (item) and `VAR_0x8001` (amount), as Gen3's `giveitem`/`finditem` set them;
///   they write `VAR_RESULT` as the std scripts do. The lowering calls the helpers directly;
///   this covers a computed std.
/// - `MSGBOX_*`: the `msgbox` macro is lowered to [`msgbox`] with its text. A bare
///   `callstd MSGBOX_*` prints the text last loaded into script word 0 (`message NULL`), which
///   the lowering does not track: reported `Unsupported` (by kind name).
/// - `STD_OBTAIN_DECORATION`: adds the decoration in `VAR_0x8000` to the inventory;
/// - `STD_REGISTER_MATCH_CALL` (`Std_RegisteredInMatchCall`): ignored. It only shows the
///   "registered in the PokéNav" message after `SetMatchCallRegisteredFlag` (also ignored); the
///   Match Call is Milestone 9.
/// - Anything else (not a `gStdScripts` index): reported `Unsupported`.
pub async fn callstd(ctx: &Ctx, std: u8) {
    let name = name_of(&crate::data::STD_NAMES, std.into()).unwrap_or("STD_?");
    ctx.log(name).await;
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
        STD_OBTAIN_DECORATION => {
            let id = ctx.var(VAR_0x8000).await;
            super::add_decoration(ctx, id).await;
        }
        STD_REGISTER_MATCH_CALL => {}
        _ => ctx.report(Report::Unsupported(name)).await,
    }
}
