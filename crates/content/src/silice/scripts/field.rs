//! Field scripts the engine starts by itself (not from a map event).

use crate::silice::prelude::*;

/// Gen3 `EventScript_HiddenItemScript`: a hidden item's bg event (`BgKind::HiddenItem`,
/// `GameData::hidden_item_script`), the engine having set `VAR_0x8004` (its flag) and
/// `VAR_0x8005` (the item).
pub async fn EventScript_HiddenItemScript(ctx: Ctx) {
    crate::prelude::hidden_item_script(&ctx).await;
}
