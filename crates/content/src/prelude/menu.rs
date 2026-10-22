//! Menus (owned by Milestone 8 Task H).

use super::*;

/// Gen3 `multichoice x, y, list, ignoreBPress` / `multichoicedefault` (`ScrCmd_multichoice`):
/// shows list `list` (`MULTI_*`) at tile (`x`, `y`) with the cursor on `default`, and waits.
/// `cancel` is Gen3's `!ignoreBPress`: B then answers `MULTI_B_PRESSED` (127). `VAR_RESULT` and
/// the return value: the chosen index or 127.
pub async fn multichoice(ctx: &Ctx, list: u8, x: u8, y: u8, default: u8, cancel: bool) -> u16 {
    let v = ctx.multichoice(list, x, y, default, cancel).await;
    ctx.set_var(VAR_RESULT, v).await;
    v
}
