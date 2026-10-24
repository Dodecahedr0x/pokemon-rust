//! Chapter 7, the Pont de la Logique Brisée (`RouteS07`): `contenu-jeu/chapitres/07-reverse-city.md`
//! §2 (the signs, the ambient NPCs, the silica blocks C1, C2, C-Puzzle and C3), §3
//! Route_S07_Sc01 (the first data wall) and Route_S07_Sc02 (Adrien's holographic call), §5 (the
//! five route trainers) and §7 (the items). `VAR_SIL_STORY` 68 (Chapter 6's end) → 70 (on the
//! bridge) → 71 (the call heard). Reverse-City is `ch07_reverse_city.rs`.
//!
//! **The route's trainers have no lines in the source** (§5 lists their teams only, and
//! `text/fr.ron` has no intro or defeat for them): each one says a line of its own zone, as its
//! intro and its defeat (author question). The arrows and the flip tiles are plain floor until
//! the engine has forced movement ("SIL4 Ch7 maps as built").

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS07"];

// ---- the silica blocks (the Scanner de Faille) ----

/// A block of compressed silica (an object, `BREAKABLE_ROCK`), Gen3's `EventScript_RockSmash`
/// with the Mallette's module (`FLAG_SIL_MODULE_ECLATE_ROC`, Chapter 6's Scanner de Faille)
/// instead of a party move: without the module, the route's block label
/// (`Route_S07_Sign_Bloc`); with it, `Route_S07_Bloc_Scan` (yes/no), on yes
/// `Route_S07_Bloc_Casse` and the block removed. As Gen3's rocks, each block's flag is a temp
/// flag (`FLAG_TEMP_11`.., the phase file `06_ch7_scripts.ron`): it stays gone while the player
/// is on the map, is back on the next load, and breaks again. Shared by Route S7's four blocks
/// and Reverse-City's alley block.
pub(super) async fn bloc(ctx: &Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_ECLATE_ROC).await {
        msgbox(ctx, Route_S07_Sign_Bloc, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if ask(ctx, Route_S07_Bloc_Scan).await {
        say(ctx, Route_S07_Bloc_Casse).await;
        let obj = ctx.var(VAR_LAST_TALKED).await;
        ctx.remove_object(obj as u8).await;
    }
    ctx.release().await;
}

/// C1 (14, 62), the bridge's whole width (mandatory), C2 (10, 52) before the Twisted Spoon's
/// balcony, C-Puzzle (12, 40) the trap's way out, C3 (16, 9) before the Max Revive.
pub async fn RouteS07_EventScript_Bloc(ctx: Ctx) {
    bloc(&ctx).await;
}

// ---- scenes ----

/// Route_S07_Sc01, the first data wall: the route's on-frame entry at `VAR_SIL_STORY` 68
/// (Chapter 6's end, Manga_Square_Sc03), as the player comes through the Grand Torii. Sets 70.
pub async fn RouteS07_Sc01(ctx: Ctx) {
    if story(&ctx).await != 68 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Route_S07_Sc01_T1).await;
    set_story(&ctx, 70).await;
    ctx.release_all().await;
}

/// Route_S07_Sc02, Adrien's holographic call: the coord row (13–15, 34) at the entrance of the
/// Pile du Milieu, `VAR_SIL_STORY` 70. `FLAG_SIL_ADRIEN_TRADUCTION_NZ`; sets 71.
pub async fn RouteS07_Sc02(ctx: Ctx) {
    if story(&ctx).await != 70 {
        return;
    }
    ctx.lock_all().await;
    // fallback: play_se(SE_APPEL), the Mallette's ring (no sound yet): T1 says it.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    say(&ctx, Route_S07_Sc02_T1).await;
    // fallback: show_hologram(ADRIEN) (§9): the source's own fallback, the text boxes alone.
    fallback(&ctx, "show_hologram").await;
    for t in [
        Route_S07_Sc02_T2,
        Route_S07_Sc02_T3,
        Route_S07_Sc02_T4,
        Route_S07_Sc02_T5,
        Route_S07_Sc02_T6,
        Route_S07_Sc02_T7,
    ] {
        say(&ctx, t).await;
    }
    // fallback: hide_hologram (§9): nothing to hide; T8 says the hologram dissolves.
    fallback(&ctx, "hide_hologram").await;
    say(&ctx, Route_S07_Sc02_T8).await;
    ctx.set_flag(FLAG_SIL_ADRIEN_TRADUCTION_NZ, true).await;
    set_story(&ctx, 71).await;
    ctx.release_all().await;
}

// ---- trainers (no lines in the source: each says a line of its zone) ----

/// TECH.COCKET (S7-A, (17, 56), sight 5 left): the Cocket worker's line (`Route_S07_Npc_Ouvrier`,
/// "we laid these arrows").
pub async fn RouteS07_EventScript_Technicien1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S07_TECH1,
        TRAINER_SIL_TECHNICIEN_S07_1,
        Route_S07_Npc_Ouvrier,
        Route_S07_Npc_Ouvrier,
    )
    .await
    {
        post_battle(&ctx, Route_S07_Npc_Ouvrier).await;
    }
}

/// LOGICIEN PASCAL (the puzzle's niche (10, 36), sight 1 down): the puzzle's syllogism
/// (`Route_S07_Sign_Premisses`).
pub async fn RouteS07_EventScript_Pascal(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S07_PASCAL,
        TRAINER_SIL_LOGICIEN_PASCAL,
        Route_S07_Sign_Premisses,
        Route_S07_Sign_Premisses,
    )
    .await
    {
        post_battle(&ctx, Route_S07_Sign_Premisses).await;
    }
}

/// PSY SÉLÈNE (S7-C, (14, 26), sight 4 down): the philosopher's line of the same pile
/// (`Route_S07_Npc_Philosophe`).
pub async fn RouteS07_EventScript_Selene(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S07_SELENE,
        TRAINER_SIL_PSY_SELENE,
        Route_S07_Npc_Philosophe,
        Route_S07_Npc_Philosophe,
    )
    .await
    {
        post_battle(&ctx, Route_S07_Npc_Philosophe).await;
    }
}

/// MYSTIQUE MIRA (S7-C, (22, 27), sight 5 left): the tourist's line about the violet tiles
/// (`Route_S07_Npc_Touriste`).
pub async fn RouteS07_EventScript_Mira(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S07_MIRA,
        TRAINER_SIL_MYSTIQUE_MIRA,
        Route_S07_Npc_Touriste,
        Route_S07_Npc_Touriste,
    )
    .await
    {
        post_battle(&ctx, Route_S07_Npc_Touriste).await;
    }
}

/// TECH.COCKET (S7-D, (12, 16), sight 3 right, the walkway's whole width): the worker's line.
pub async fn RouteS07_EventScript_Technicien2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S07_TECH2,
        TRAINER_SIL_TECHNICIEN_S07_2,
        Route_S07_Npc_Ouvrier,
        Route_S07_Npc_Ouvrier,
    )
    .await
    {
        post_battle(&ctx, Route_S07_Npc_Ouvrier).await;
    }
}

// ---- the ambient NPCs ----

pub async fn RouteS07_EventScript_Ouvrier(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Npc_Ouvrier, MsgBox::Npc).await;
}

pub async fn RouteS07_EventScript_Philosophe(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Npc_Philosophe, MsgBox::Npc).await;
}

pub async fn RouteS07_EventScript_Touriste(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Npc_Touriste, MsgBox::Npc).await;
}

// ---- signs ----

pub async fn RouteS07_EventScript_SignPremisses(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Sign_Premisses, MsgBox::Sign).await;
}

pub async fn RouteS07_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS07_EventScript_SignBloc(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Sign_Bloc, MsgBox::Sign).await;
}

pub async fn RouteS07_EventScript_SignPorte(ctx: Ctx) {
    msgbox(&ctx, Route_S07_Sign_Porte, MsgBox::Sign).await;
}

// ---- items ----

/// S7-A, by the worker.
pub async fn RouteS07_EventScript_ItemFullHeal(ctx: Ctx) {
    find_item(&ctx, ITEM_FULL_HEAL, 1).await;
}

/// S7-A, on C2's balcony.
pub async fn RouteS07_EventScript_ItemTwistedSpoon(ctx: Ctx) {
    find_item(&ctx, ITEM_TWISTED_SPOON, 1).await;
}

/// S7-B, the puzzle's trap (behind C-Puzzle).
pub async fn RouteS07_EventScript_ItemRareCandy(ctx: Ctx) {
    find_item(&ctx, ITEM_RARE_CANDY, 1).await;
}

/// S7-B, below the puzzle's top row.
pub async fn RouteS07_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 1).await;
}

/// S7-D, behind C3.
pub async fn RouteS07_EventScript_ItemMaxRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REVIVE, 1).await;
}

