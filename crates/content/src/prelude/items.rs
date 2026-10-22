//! Items and money (owned by Milestone 8 Task H). Wave 0: each issues its request and returns
//! the host's default.

use super::*;

/// Gen3 `Std_ObtainItem` (`giveitem`): adds the item and says so; whether it fit.
pub async fn give_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    ctx.item_add(ItemId(item), qty).await
}

/// Gen3 `Std_FindItem` (item balls): adds the item and says so; whether it fit.
pub async fn find_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    ctx.item_add(ItemId(item), qty).await
}

/// Gen3 `checkitem`.
pub async fn check_item(ctx: &Ctx, item: u16, qty: u16) -> bool {
    ctx.item_check(ItemId(item), qty).await
}

/// Gen3 `addmoney`.
pub async fn money_add(ctx: &Ctx, n: u32) {
    ctx.money_add(n).await
}

/// Gen3 `removemoney`.
pub async fn money_remove(ctx: &Ctx, n: u32) {
    ctx.money_remove(n).await
}

/// Gen3 `checkmoney`.
pub async fn money_check(ctx: &Ctx, n: u32) -> bool {
    ctx.money_check(n).await
}
