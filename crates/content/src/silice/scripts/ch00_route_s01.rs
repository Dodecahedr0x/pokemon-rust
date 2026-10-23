//! Chapter 0, Route S1, the Sentier des Racines (`RouteS01`): `contenu-jeu/chapitres/00-prologue.md`
//! §2 (signs, ambient NPCs), §3 Route_S01_Sc01 and Sc02, §5 (the four trainers) and §7 (the
//! items). `VAR_SIL_STORY` runs 5 (the starter chosen) to 9 (in sight of Bois-Éther). The wild
//! encounters are `battle/encounters.ron`'s; the north exit to Bois-Éther is Chapter 1's.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS01"];

// ---- scenes ----

/// Route_S01_Sc01, the first wild battle (tutorial): the first tall grass row (x 8–13, y 30)
/// at `VAR_SIL_STORY` 5. A Sentret, level 3, that the player may run from or catch. Sets 6
/// (losing whites out home and leaves 5, so the scene comes again).
pub async fn RouteS01_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S01_Sc01_T1).await;
    battle_wild(&ctx, SPECIES_SENTRET, 3).await;
    say(&ctx, Route_S01_Sc01_T2).await;
    set_story(&ctx, 6).await;
    ctx.release_all().await;
}

/// Route_S01_Sc02, in sight of Bois-Éther: the north edge (x 7–10, y 2). The source's
/// "`VAR_SIL_STORY` ≤ 8" is the map's value 6, the only one that reaches here (Sc01's row sets
/// it). Sets 9: Chapter 1 takes over at `MAP_BOIS_ETHER`.
pub async fn RouteS01_Sc02(ctx: Ctx) {
    if story(&ctx).await > 8 {
        return;
    }
    ctx.lock().await;
    say(&ctx, Route_S01_Sc02_T1).await;
    set_story(&ctx, 9).await;
    ctx.release().await;
}

// ---- trainers (§5; sight from the map's trainer objects) ----

/// GAMIN TÉO (3, 24).
pub async fn RouteS01_EventScript_Teo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S01_TEO,
        TRAINER_SIL_GAMIN_TEO,
        Route_S01_Teo_Intro,
        Route_S01_Teo_Defeat,
    )
    .await
    {
        after_battle(&ctx, Route_S01_Teo_Defeat).await;
    }
}

/// SCOUT NOAH (12, 18).
pub async fn RouteS01_EventScript_Noah(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S01_NOAH,
        TRAINER_SIL_SCOUT_NOAH,
        Route_S01_Noah_Intro,
        Route_S01_Noah_Defeat,
    )
    .await
    {
        after_battle(&ctx, Route_S01_Noah_Defeat).await;
    }
}

/// FILLETTE LILA (3, 12).
pub async fn RouteS01_EventScript_Lila(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S01_LILA,
        TRAINER_SIL_FILLETTE_LILA,
        Route_S01_Lila_Intro,
        Route_S01_Lila_Defeat,
    )
    .await
    {
        after_battle(&ctx, Route_S01_Lila_Defeat).await;
    }
}

/// CAMPEUR YANIS (12, 4); optional, passed on the east.
pub async fn RouteS01_EventScript_Yanis(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S01_YANIS,
        TRAINER_SIL_CAMPEUR_YANIS,
        Route_S01_Yanis_Intro,
        Route_S01_Yanis_Defeat,
    )
    .await
    {
        after_battle(&ctx, Route_S01_Yanis_Defeat).await;
    }
}

/// A beaten trainer talked to again. The source writes no post-battle lines: the defeat text
/// stands in (Gen3 `msgbox <PostBattle>, MSGBOX_AUTOCLOSE`; the trainer is locked and faces the
/// player already).
async fn after_battle(ctx: &Ctx, text: TextId) {
    msgbox(ctx, text, MsgBox::Autoclose).await;
}

// ---- items (§7) ----

/// The Poké Ball near the camper (14, 6).
pub async fn RouteS01_EventScript_ItemPokeBall(ctx: Ctx) {
    find_item(&ctx, ITEM_POKE_BALL, 1).await;
}

/// The Potion (14, 22).
pub async fn RouteS01_EventScript_ItemPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_POTION, 1).await;
}

/// The hidden Antidote at the foot of a root (4, 22), once (`FLAG_SIL_ITEM_ROUTE_S01_ANTIDOTE`).
pub async fn RouteS01_EventScript_HiddenAntidote(ctx: Ctx) {
    // fallback: hidden items (the engine has ignored `BgKind::HiddenItem` since M3, and no
    // milestone owns it yet): the map has a sign here instead, and this script gives the item
    // with Gen3's obtain messages, once, behind the item's flag.
    hidden_item(&ctx, FLAG_SIL_ITEM_ROUTE_S01_ANTIDOTE, ITEM_ANTIDOTE).await;
}

// ---- ambient NPCs ----

/// The walker by the entrance; after the first wild battle (`VAR_SIL_STORY` ≥ 6) she gives two
/// Oran Berries, once (`FLAG_SIL_ITEM_ROUTE_S01_ORAN`, §7).
pub async fn RouteS01_EventScript_Promeneuse(ctx: Ctx) {
    if story(&ctx).await < 6 || ctx.flag(FLAG_SIL_ITEM_ROUTE_S01_ORAN).await {
        msgbox(&ctx, Route_S01_Npc_Promeneuse, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Route_S01_Npc_Promeneuse_Oran).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_ROUTE_S01_ORAN,
        ITEM_ORAN_BERRY,
        2,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}

/// The Cocket technician by the cables.
pub async fn RouteS01_EventScript_Technicien(ctx: Ctx) {
    msgbox(&ctx, Route_S01_Npc_Technicien, MsgBox::Npc).await;
}

// ---- signs ----

pub async fn RouteS01_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S01_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS01_EventScript_SignHerbes(ctx: Ctx) {
    msgbox(&ctx, Route_S01_Sign_Herbes, MsgBox::Sign).await;
}

pub async fn RouteS01_EventScript_SignDresseurs(ctx: Ctx) {
    msgbox(&ctx, Route_S01_Sign_Dresseurs, MsgBox::Sign).await;
}

pub async fn RouteS01_EventScript_SignCables(ctx: Ctx) {
    msgbox(&ctx, Route_S01_Sign_Cables, MsgBox::Sign).await;
}
