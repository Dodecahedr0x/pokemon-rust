//! `callstd` (Gen3 `gStdScripts`, `data/event_scripts.s`; Milestone 8 Task J).

use super::*;

/// `callstd <std>`, every `gStdScripts` entry (the name is logged first):
///
/// - `STD_OBTAIN_ITEM` / `STD_FIND_ITEM` (`Std_ObtainItem` / `Std_FindItem`): the item helpers
///   ([`give_item`], [`find_item`]) on `VAR_0x8000` (item) and `VAR_0x8001` (amount), the
///   `giveitem`/`finditem` operands; they write `VAR_RESULT` as the std scripts do.
/// - `MSGBOX_*`: the `msgbox` macro is lowered to [`msgbox`] with its text. A bare
///   `callstd MSGBOX_*` prints the text last loaded into script word 0 (`message NULL`), which
///   the lowering does not track: reported `Unsupported` (by kind name).
/// - `STD_OBTAIN_DECORATION`: no decorations yet (Milestone 9): reported `Unsupported`;
///   `VAR_RESULT` is unchanged.
/// - `STD_REGISTER_MATCH_CALL` (`Std_RegisteredInMatchCall`): ignored. It only shows the
///   "registered in the PokéNav" message after `SetMatchCallRegisteredFlag` (also ignored); the
///   Match Call is Milestone 9.
/// - Anything else (not a `gStdScripts` index): reported `Unsupported`.
pub async fn callstd(ctx: &Ctx, std: u8) {
    let name = name_of(&crate::data::STD_NAMES, std.into()).unwrap_or("STD_?");
    ctx.log(name).await;
    match std {
        STD_OBTAIN_ITEM | STD_FIND_ITEM => {
            let item = ctx.var(VAR_0x8000).await;
            let qty = ctx.var(VAR_0x8001).await;
            // The helpers write `VAR_RESULT` as the std scripts do (Task H).
            if std == STD_OBTAIN_ITEM {
                give_item(ctx, item, qty).await;
            } else {
                find_item(ctx, item, qty).await;
            }
        }
        STD_REGISTER_MATCH_CALL => {}
        _ => ctx.report(Report::Unsupported(name)).await,
    }
}
