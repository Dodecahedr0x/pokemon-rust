//! The Mallette (Silice Task C, `contenu-jeu/bible/mallette.md`): its START menu entry and the
//! modules the engine drives from the field. The tables are `silice::mallette`.
//!
//! **The menu** (`Mallette_Menu`, the START menu's first entry once `FLAG_SIL_MALLETTE_OPEN`
//! is set): ANALYSE, CARTE, APPELS, MODULES, RAPPORTS, SYNCHRO, RETOUR, until RETOUR or B. An
//! entry not unlocked yet says `Mallette_Verrouille`.
//! - ANALYSE (`FLAG_SIL_MALLETTE_DEX`): the seen and caught counts (the Pokédex's numbers; no
//!   Pokédex screen yet, a fallback).
//! - CARTE (`FLAG_SIL_MALLETTE_MAP`): one line; no region map screen with the RDV signal yet
//!   (a fallback).
//! - APPELS (`FLAG_SIL_MALLETTE_CALLS`): no call is listed (the source names the callers,
//!   Julien, Benou and Adrien, but no call to replay; a fallback).
//! - MODULES: each installed module's name (`FLAG_SIL_MODULE_*`, badge order).
//! - RAPPORTS (from `FLAG_SIL_LOG_01`, badge 1): the logs list; a log whose `FLAG_SIL_LOG_xx`
//!   is set is read again (its scene's text), else `Mallette_Log_Chiffre`.
//! - SYNCHRO (once `VAR_SIL_SYNC` is set, badge 1): `SYNCHRONISATION: n%`.
//!
//! **Modules on the field**: `Mallette_UseSurf`, `Mallette_UseWaterfall`, `Mallette_UseDive`,
//! `Mallette_UseDiveUnderwater` and `Mallette_CannotUseWaterfall` are Gen3's `EventScript_Use*`
//! without the party check (a module needs no Pokémon): the question, the line, the field
//! effect. The texts are Silice's own (author question: the source has none).

use crate::silice::mallette::{LIST_LOGS, LIST_MENU, MODULES};
use crate::silice::prelude::*;
use engine::data::FieldMove;

/// The logs in order: the flag that unlocks each and the text read again (the scene's own).
const LOGS: [(FlagId, TextId); 8] = [
    (FLAG_SIL_LOG_01, LOG_01_Texte),
    (FLAG_SIL_LOG_02, LOG_02_Texte),
    (FLAG_SIL_LOG_03, Nocturne_Sc02_T11),
    (FLAG_SIL_LOG_04, Port_Ressac_Navire_Log04),
    (FLAG_SIL_LOG_05, Owlys_Gym_Log05),
    (FLAG_SIL_LOG_06, Gym_Sc03_Log06),
    (FLAG_SIL_LOG_07, Gym07_Sc02_Log07),
    // The bible: "Illisible: la zone blanche brouille la Mallette".
    (FLAG_SIL_LOG_08, Mallette_Log08_Texte),
];

/// The START menu's MALLETTE entry (run locked, after the START menu closes).
pub async fn Mallette_Menu(ctx: Ctx) {
    let mut cursor = 0;
    loop {
        let choice = multichoice(&ctx, LIST_MENU, 0, 0, cursor, true).await;
        cursor = choice.min(6) as u8;
        match choice {
            0 => analyse(&ctx).await,
            1 => unlocked_line(&ctx, FLAG_SIL_MALLETTE_MAP, Mallette_Carte, "map screen").await,
            2 => unlocked_line(&ctx, FLAG_SIL_MALLETTE_CALLS, Mallette_Appels, "calls").await,
            3 => modules(&ctx).await,
            4 => logs(&ctx).await,
            5 => sync(&ctx).await,
            _ => break,
        }
    }
    ctx.close_message().await;
    ctx.release_all().await;
}

async fn analyse(ctx: &Ctx) {
    if !ctx.flag(FLAG_SIL_MALLETTE_DEX).await {
        say(ctx, Mallette_Verrouille).await;
        return;
    }
    // fallback: the Analyse screen (the Pokédex's list): its counts only.
    fallback(ctx, "analyse screen").await;
    let seen = ctx
        .query(Query::DexCount {
            caught: false,
            among: &[],
        })
        .await;
    let caught = ctx
        .query(Query::DexCount {
            caught: true,
            among: &[],
        })
        .await;
    buffer(ctx, 0, BufferSrc::Number(seen.into())).await;
    buffer(ctx, 1, BufferSrc::Number(caught.into())).await;
    say(ctx, Mallette_Analyse).await;
}

/// An entry with no screen yet: its line once unlocked (a fallback named `what`).
async fn unlocked_line(ctx: &Ctx, unlock: FlagId, text: TextId, what: &'static str) {
    if !ctx.flag(unlock).await {
        say(ctx, Mallette_Verrouille).await;
        return;
    }
    fallback(ctx, what).await;
    say(ctx, text).await;
}

async fn modules(ctx: &Ctx) {
    let mut any = false;
    for (flag, name, _) in MODULES {
        if ctx.flag(flag).await {
            any = true;
            buffer(ctx, 0, BufferSrc::Text(name)).await;
            say(ctx, Mallette_Module_Pret).await;
        }
    }
    if !any {
        say(ctx, Mallette_Modules_Aucun).await;
    }
}

async fn logs(ctx: &Ctx) {
    if !ctx.flag(FLAG_SIL_LOG_01).await {
        say(ctx, Mallette_Verrouille).await;
        return;
    }
    let mut cursor = 0;
    loop {
        let choice = multichoice(ctx, LIST_LOGS, 0, 0, cursor, true).await;
        let Some(&(flag, text)) = LOGS.get(usize::from(choice)) else {
            return;
        };
        cursor = choice as u8;
        if ctx.flag(flag).await {
            say(ctx, text).await;
        } else {
            say(ctx, Mallette_Log_Chiffre).await;
        }
    }
}

async fn sync(ctx: &Ctx) {
    let n = ctx.var(VAR_SIL_SYNC).await;
    if n == 0 {
        say(ctx, Mallette_Verrouille).await;
        return;
    }
    // fallback: show_sync's animated gauge: the value as a line.
    buffer(ctx, 0, BufferSrc::Number(n.into())).await;
    say(ctx, Mallette_Sync).await;
}

/// Gen3 `EventScript_UseSurf` with the Hydro-Propulseur (`FLAG_SIL_MODULE_SURF`, checked by the
/// engine before it runs this).
pub async fn Mallette_UseSurf(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Mallette_Surf_Ask).await {
        say(&ctx, Mallette_Surf_Used).await;
        ctx.close_message().await;
        ctx.field_move_effect(FieldMove::Surf).await;
    } else {
        ctx.close_message().await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseWaterfall` with the Turbine à Flux (`FLAG_SIL_MODULE_CASCADE`).
pub async fn Mallette_UseWaterfall(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Mallette_Cascade_Ask).await {
        say(&ctx, Mallette_Cascade_Used).await;
        ctx.close_message().await;
        ctx.field_move_effect(FieldMove::Waterfall).await;
    } else {
        ctx.close_message().await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_CannotUseWaterfall`: facing a waterfall without the module (or not
/// surfing north).
pub async fn Mallette_CannotUseWaterfall(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Mallette_Cascade_Bloque).await;
    ctx.close_message().await;
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseDive` with the Sonde de Pression (`FLAG_SIL_MODULE_PLONGEE`).
pub async fn Mallette_UseDive(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Mallette_Plongee_Ask).await {
        say(&ctx, Mallette_Plongee_Used).await;
        ctx.close_message().await;
        ctx.field_move_effect(FieldMove::Dive).await;
    } else {
        ctx.close_message().await;
    }
    ctx.release_all().await;
}

/// Gen3 `EventScript_UseDiveUnderwater`: B underwater where the player can surface.
pub async fn Mallette_UseDiveUnderwater(ctx: Ctx) {
    ctx.lock_all().await;
    if ask(&ctx, Mallette_Plongee_Remonter).await {
        say(&ctx, Mallette_Plongee_Used).await;
        ctx.close_message().await;
        ctx.field_move_effect(FieldMove::Dive).await;
    } else {
        ctx.close_message().await;
    }
    ctx.release_all().await;
}
