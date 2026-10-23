//! `pokemart` (Milestone 9 Task J owns the file): the mart screen with Gen3's std texts around
//! it (`Std_...` "How may I serve you?" and the closing line come from the clerk's script).
//!
//! Wave 0: issues the request; the host answers at once (no screen yet).

use super::*;

/// Gen3 `pokemart <list>`: the buy/sell screen over `items` (Task K and M's screen).
pub async fn mart(ctx: &Ctx, items: &'static [ItemId]) {
    ctx.mart(items).await;
}
