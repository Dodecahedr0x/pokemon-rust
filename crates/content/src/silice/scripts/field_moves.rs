//! The Mallette's water modules on the field (SIL4 Ch9): Gen3's `EventScript_UseSurf`,
//! `EventScript_UseWaterfall`, `EventScript_CannotUseWaterfall`, `EventScript_UseDive` and
//! `EventScript_UseDiveUnderwater` (`data/scripts/surf.inc`, `field_move_scripts.inc`), which the
//! engine starts for an A press facing water, a waterfall or on deep water, and a B press
//! underwater (`GameData::field_move_script`, `waterfall_refused_script`, `dive_emerge_script`).
//!
//! Silice's field abilities are modules, flags (`crate::silice::Game::field_move_gate`: the
//! Hydro-Propulseur `FLAG_SIL_MODULE_SURF`, the Turbine à Flux `FLAG_SIL_MODULE_CASCADE`, the
//! Sonde de Pression `FLAG_SIL_MODULE_PLONGEE`), not a party mon's move: so no `checkpartymove`,
//! and no "{STR_VAR_1} used …" line (no mon uses it). The questions are Gen3's own texts
//! (`gText_WantToUseSurf`, `Text_WantToDive`, `Text_WantToSurface`, `Text_WantToWaterfall`,
//! `Text_CantWaterfall`, kept by the Silice build), placeholders until the author words the
//! modules' prompts (an author question in the plan's Ch9 section).
//!
//! This file claims no map.

use crate::silice::prelude::*;
use engine::data::FieldMove;

/// Gen3 `EventScript_UseSurf`: the question, then the Surf effect (it ends on its own).
pub async fn EventScript_UseSurf(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, gText_WantToUseSurf).await {
        ctx.field_move_effect(FieldMove::Surf).await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseWaterfall`: the question, then the climb.
pub async fn EventScript_UseWaterfall(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Text_WantToWaterfall).await {
        ctx.field_move_effect(FieldMove::Waterfall).await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_CannotUseWaterfall`: facing a waterfall without the module (or not
/// surfing north).
pub async fn EventScript_CannotUseWaterfall(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Text_CantWaterfall).await;
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseDive`: the question, then the dive (the effect warps).
pub async fn EventScript_UseDive(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Text_WantToDive).await {
        ctx.field_move_effect(FieldMove::Dive).await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseDiveUnderwater`: the question, then back up (the effect warps).
pub async fn EventScript_UseDiveUnderwater(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Text_WantToSurface).await {
        ctx.field_move_effect(FieldMove::Dive).await;
    }
    ctx.release_all().await;
}
