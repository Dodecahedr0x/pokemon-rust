//! Chapter 5, the Sea of Wrecks (`RouteS05`) and its sub-maps (the Banc de Silice, the wreck of
//! the Vainqueur, the Grey Coral Cave, the sea rift): `contenu-jeu/chapitres/05-owlys.md` §2
//! (the buoys, the islet's NPCs, the captain's log, the coral, the gargoyle), §3 Route_S05_Sc01
//! (`VAR_SIL_STORY` 49 → 50), Route_S05_Sc02 (the sailor's Punch Bottle, then his shop) and
//! Route_S05_Grotte_Sc01 (Lokhlass), §5 (the route's trainers and Fabrice) and §7 (the items).
//!
//! **Surf.** Everything here but the islet's NPCs, the wreck and the rift's ramp is on water,
//! and the engine has no Surf yet (M10): the scripts are written, the maps keep their water.
//! The magnetic currents (Route 134's behaviours) do not move the player yet either (the forced
//! movement work; they need Surf first).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "RouteS05",
    "RouteS05_Banc",
    "RouteS05_Epave",
    "RouteS05_GrotteCorail",
    "RouteS05_Faille",
];

// ---- Route S5 ----

/// Route_S05_Sc01, the first wake: the route's first frame at `VAR_SIL_STORY` 49. Sets 50, so
/// the on-frame entry stops matching on every path.
pub async fn RouteS05_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S05_Sc01_T1).await;
    set_story(&ctx, 50).await;
    ctx.release_all().await;
}

/// The islet's shop (§7, "îlot du marin"): the Punch Bottle and Lemonade.
const ILOT_STOCK: &[ItemId] = &[ItemId(ITEM_SIL_BOUTEILLE_PUNCH), ItemId(ITEM_LEMONADE)];

/// Route_S05_Sc02, Captain Samuel's sailor on the islet: the Punch Bottle once
/// (`FLAG_SIL_ROUTE_S05_PUNCH`), then his shop.
pub async fn RouteS05_Sc02(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_ROUTE_S05_PUNCH).await {
        say(&ctx, Route_S05_Sc02_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ROUTE_S05_PUNCH,
            ITEM_SIL_BOUTEILLE_PUNCH,
            1,
            Obtain::Message,
        )
        .await;
        say(&ctx, Route_S05_Sc02_T2).await;
        ctx.release().await;
    } else {
        say(&ctx, Route_S05_Sc02_T3).await;
        ctx.release().await;
        // fallback: mart (no shop screen before M9): greeting and goodbye, nothing sold.
        mart(&ctx, ILOT_STOCK).await;
    }
}

/// The islet's fisherman: the Super Rod once (`FLAG_SIL_ROUTE_S05_SUPER_CANNE`); afterwards he
/// only turns to the player (his one line offers the rod). No fishing yet (M10): the rod is a
/// key item the bag keeps.
pub async fn RouteS05_EventScript_Pecheur(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_ROUTE_S05_SUPER_CANNE).await {
        say(&ctx, Route_S05_Npc_Pecheur_Ilot).await;
        gift(
            &ctx,
            FLAG_SIL_ROUTE_S05_SUPER_CANNE,
            ITEM_SUPER_ROD,
            1,
            Obtain::Message,
        )
        .await;
    }
    ctx.release().await;
}

// ---- the sea trainers (§5; beaten, each repeats its defeat line) ----

pub async fn RouteS05_EventScript_Hugo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S05_HUGO,
        TRAINER_SIL_ROUTE_S05_NAGEUR_HUGO,
        Route_S05_Hugo_Intro,
        Route_S05_Hugo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Hugo_Defeat).await;
    }
}

pub async fn RouteS05_EventScript_Ines(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S05_INES,
        TRAINER_SIL_ROUTE_S05_NAGEUSE_INES,
        Route_S05_Ines_Intro,
        Route_S05_Ines_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Ines_Defeat).await;
    }
}

pub async fn RouteS05_EventScript_Yvon(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S05_YVON,
        TRAINER_SIL_ROUTE_S05_MARIN_YVON,
        Route_S05_Yvon_Intro,
        Route_S05_Yvon_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Yvon_Defeat).await;
    }
}

pub async fn RouteS05_EventScript_Gaston(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S05_GASTON,
        TRAINER_SIL_ROUTE_S05_PECHEUR_GASTON,
        Route_S05_Gaston_Intro,
        Route_S05_Gaston_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Gaston_Defeat).await;
    }
}

pub async fn RouteS05_EventScript_Coralie(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S05_CORALIE,
        TRAINER_SIL_ROUTE_S05_NAGEUSE_CORALIE,
        Route_S05_Coralie_Intro,
        Route_S05_Coralie_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Coralie_Defeat).await;
    }
}

pub async fn RouteS05_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S05_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS05_EventScript_SignParoi(ctx: Ctx) {
    msgbox(&ctx, Route_S05_Sign_Paroi, MsgBox::Sign).await;
}

/// The Hyper Potion on an overturned hull (5, 40).
pub async fn RouteS05_EventScript_ItemHyperPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_HYPER_POTION, 1).await;
}

/// The Max Repel behind the islet's hut (16, 5).
pub async fn RouteS05_EventScript_ItemMaxRepel(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REPEL, 1).await;
}

// ---- the Banc de Silice ----

// ---- the wreck of the Vainqueur ----

/// FABRICE, the wreck hunter in the gangway.
pub async fn RouteS05_Epave_EventScript_Fabrice(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_EPAVE_FABRICE,
        TRAINER_SIL_ROUTE_S05_EPAVE_FABRICE,
        Route_S05_Fabrice_Intro,
        Route_S05_Fabrice_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S05_Fabrice_Defeat).await;
    }
}

/// The captain's log on the mess table (the premonition).
pub async fn RouteS05_Epave_EventScript_Journal(ctx: Ctx) {
    msgbox(&ctx, Route_S05_Epave_Journal, MsgBox::Sign).await;
}

/// TM17 Protect in the forward hold (the source wants light: the engine draws no darkness).
pub async fn RouteS05_Epave_EventScript_ItemProtection(ctx: Ctx) {
    find_item(&ctx, ITEM_TM17, 1).await;
}

/// The Big Pearl in the officers' mess.
pub async fn RouteS05_Epave_EventScript_ItemGrossePerle(ctx: Ctx) {
    find_item(&ctx, ITEM_BIG_PEARL, 1).await;
}

// ---- the Grey Coral Cave ----

/// Route_S05_Grotte_Sc01, the sad Lokhlass (the source's `SPECIES_LAPRAS` at 36): clean its
/// shell (yes/no), then a wild battle. Caught: `FLAG_SIL_ROUTE_S05_LOKHLASS` (its hide flag)
/// keeps it gone. Otherwise (fainted, or the player ran) it dives: removed for this visit
/// only, its flag cleared, so it is back on the next load, as the source says.
pub async fn RouteS05_GrotteCorail_Sc01(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Route_S05_Grotte_Sc01_T1).await;
    if !ask(&ctx, Route_S05_Grotte_Sc01_T2).await {
        say(&ctx, Route_S05_Grotte_Sc01_T3).await;
        ctx.release().await;
        return;
    }
    say(&ctx, Route_S05_Grotte_Sc01_T4).await;
    let outcome = battle_wild(&ctx, SPECIES_LAPRAS, 36).await;
    if outcome == BattleOutcome::Caught {
        ctx.set_flag(FLAG_SIL_ROUTE_S05_LOKHLASS, true).await;
        hide(&ctx, LOCALID_GROTTE_CORAIL_LOKHLASS).await;
    } else {
        say(&ctx, Route_S05_Grotte_Sc01_T5).await;
        ctx.fade(Fade::ToBlack).await;
        hide(&ctx, LOCALID_GROTTE_CORAIL_LOKHLASS).await;
        // Gen3 `removeobject` set its hide flag, which means "caught": cleared, so the next
        // load shows it again.
        ctx.set_flag(FLAG_SIL_ROUTE_S05_LOKHLASS, false).await;
        ctx.fade(Fade::FromBlack).await;
    }
    ctx.release().await;
}

pub async fn RouteS05_GrotteCorail_EventScript_Corail(ctx: Ctx) {
    msgbox(&ctx, Route_S05_Grotte_Corail_Sign, MsgBox::Sign).await;
}

/// The Water Stone on the west beach.
pub async fn RouteS05_GrotteCorail_EventScript_ItemPierreEau(ctx: Ctx) {
    find_item(&ctx, ITEM_WATER_STONE, 1).await;
}

/// TM18 Rain Dance at the back of the west beach.
pub async fn RouteS05_GrotteCorail_EventScript_ItemDansePluie(ctx: Ctx) {
    find_item(&ctx, ITEM_TM18, 1).await;
}

// ---- the sea rift ----

/// The inscription under the gargoyle by the iron door.
pub async fn RouteS05_Faille_EventScript_Gargouille(ctx: Ctx) {
    msgbox(&ctx, Route_S05_Faille_Sign, MsgBox::Sign).await;
}

/// The Escape Rope on the natural quay.
pub async fn RouteS05_Faille_EventScript_ItemCorde(ctx: Ctx) {
    find_item(&ctx, ITEM_ESCAPE_ROPE, 1).await;
}
