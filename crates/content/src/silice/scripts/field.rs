//! Field scripts the engine starts by itself (not from a map event).

use crate::silice::prelude::*;

/// Gen3 `EventScript_HiddenItemScript`: a hidden item's bg event (`BgKind::HiddenItem`,
/// `GameData::hidden_item_script`), the engine having set `VAR_0x8004` (its flag) and
/// `VAR_0x8005` (the item).
pub async fn EventScript_HiddenItemScript(ctx: Ctx) {
    crate::prelude::hidden_item_script(&ctx).await;
}

/// After a Champ Magnétique push (`GameData::pushed_script`, `silice::push`; the engine has set
/// `VAR_0x8004` to the object and `VAR_0x8005`, `VAR_0x8006` to its new cell): the object's
/// cell in its temp var, then the map's completion check.
pub async fn Mallette_Pushed(ctx: Ctx) {
    use crate::silice::push;
    let map = ctx.current_map().await;
    let id = ctx.var(VAR_0x8004).await as u8;
    let cell = (
        ctx.var(VAR_0x8005).await as i16,
        ctx.var(VAR_0x8006).await as i16,
    );
    let Some((_, &(_, var, _))) = push::find(map, id) else {
        return;
    };
    ctx.set_var(var, push::cell_var(cell)).await;
    match map {
        MAP_EVALON_TOUR_SCRIBE => super::ch08_tour::rayonnage_pushed(&ctx).await,
        MAP_EVALON_TOUR_SCRIBE_2F => super::ch08_tour::caisse_pushed(&ctx).await,
        MAP_EVALON_CITADELLE => super::ch08_gym::golem_pushed(&ctx).await,
        _ => {}
    }
}
