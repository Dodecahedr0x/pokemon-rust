//! Menus (owned by Milestone 8 Task H).

use super::*;

/// Gen3 `multichoice` / `multichoicedefault`: the chosen index, or `MULTI_B_PRESSED` (127).
pub async fn multichoice(ctx: &Ctx, list: u8, x: u8, y: u8, default: u8, cancel: bool) -> u16 {
    ctx.multichoice(list, x, y, default, cancel).await
}
