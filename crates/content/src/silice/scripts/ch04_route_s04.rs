//! Chapter 4, the Tunnel de Verre (`RouteS04`): `contenu-jeu/chapitres/04-port-ressac.md` §2
//! (signs, Julien's wall messages M1 to M8, ambient NPCs), §3 Route_S04_Sc01 to Sc03, §5 (the five
//! trainers) and §7 (the items). `VAR_SIL_STORY` runs 36 (Chapter 3's end) to 41 (Adrien met, the
//! walls readable). Port-Ressac is `ch04_port.rs`.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS04"];

// ---- map scripts ----

/// The dark map (§2 "Obscurité") and Adrien's Magikarp, whose hide flag follows Adrien's (the
/// map has one flag per object; Sc02 hides both).
pub async fn RouteS04_OnTransition(ctx: Ctx) {
    let adrien_gone = ctx.flag(FLAG_SIL_HIDE_ROUTE_S04_ADRIEN).await;
    ctx.set_flag(FLAG_SIL_HIDE_ROUTE_S04_MAGICARPE, adrien_gone)
        .await;
    // fallback: the dark map and its light (`cave: 1` and the Surcharge Optique module, M10's
    // Flash): the map is drawn lit; the wall messages follow `FLAG_SIL_ROUTE_S04_MURS` alone.
    fallback(&ctx, "dark_map").await;
}

// ---- scenes ----

/// Route_S04_Sc01, into the dark: the map's on-frame entry at `VAR_SIL_STORY` 36 (Chapter 3's
/// end), so it runs as the player comes in from Viber-City. Sets 40.
pub async fn RouteS04_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S04_Sc01_T1).await;
    set_story(&ctx, 40).await;
    ctx.release_all().await;
}

const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// Route_S04_Sc02, Adrien in the dark: talking to him (or to his Magikarp), or the gallery's
/// coord column (26, 5–9) two tiles before him, at `VAR_SIL_STORY` 40. The light, the wall
/// messages (`FLAG_SIL_ROUTE_S04_MURS`), then Adrien and his Magikarp leave for the Taverne
/// (`FLAG_SIL_ROUTE_S04_ADRIEN`). Sets 41.
pub async fn RouteS04_Sc02(ctx: Ctx) {
    if story(&ctx).await != 40 {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_S04_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Route_S04_Sc02_T1).await;
    // fallback: use_field_module(FLASH) (the Mallette's field modules, M10): the source's own
    // fallback, nothing; T2 tells the light and the walls become readable below.
    fallback(&ctx, "use_field_module").await;
    say(&ctx, Route_S04_Sc02_T2).await;
    move_wait(&ctx, LOCALID_S04_ADRIEN, FACE_UP).await;
    say(&ctx, Route_S04_Sc02_T3).await;
    ctx.set_flag(FLAG_SIL_ROUTE_S04_MURS, true).await;
    move_wait(&ctx, LOCALID_S04_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Route_S04_Sc02_T4).await;
    ctx.fade(Fade::ToBlack).await;
    // `hide` sets each object's hide flag (FLAG_SIL_HIDE_ROUTE_S04_ADRIEN, _MAGICARPE).
    hide(&ctx, LOCALID_S04_ADRIEN).await;
    hide(&ctx, LOCALID_S04_MAGICARPE).await;
    ctx.set_flag(FLAG_SIL_ROUTE_S04_ADRIEN, true).await;
    ctx.fade(Fade::FromBlack).await;
    set_story(&ctx, 41).await;
    ctx.release_all().await;
}

/// Route_S04_Sc03, the last message (M8, the wall by the exit): dark until Sc02; then the
/// message, and once (`FLAG_SIL_ROUTE_S04_M8`) the "1 correspondant" signal.
pub async fn RouteS04_Sc03(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_ROUTE_S04_MURS).await {
        msgbox(&ctx, Route_S04_Mur_Sombre, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    say(&ctx, Route_S04_Mur_M8).await;
    if !ctx.flag(FLAG_SIL_ROUTE_S04_M8).await {
        ctx.delay(30).await;
        say(&ctx, Route_S04_Sc03_T1).await;
        ctx.set_flag(FLAG_SIL_ROUTE_S04_M8, true).await;
    }
    ctx.release().await;
}

/// Adrien's Magikarp, glowing in the canal: at `VAR_SIL_STORY` 40 it is Adrien's scene (the
/// source has no line of its own for it).
pub async fn RouteS04_EventScript_Magicarpe(ctx: Ctx) {
    RouteS04_Sc02(ctx).await;
}

// ---- the walls (bg signs; the text follows FLAG_SIL_ROUTE_S04_MURS) ----

/// A wall message: `Mur_Sombre` until Adrien's scene lights the tunnel, then Julien's `text`.
async fn wall(ctx: &Ctx, text: TextId) {
    let lit = ctx.flag(FLAG_SIL_ROUTE_S04_MURS).await;
    let t = if lit { text } else { Route_S04_Mur_Sombre };
    msgbox(ctx, t, MsgBox::Sign).await;
}

pub async fn RouteS04_EventScript_MurM1(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M1).await;
}

pub async fn RouteS04_EventScript_MurM2(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M2).await;
}

pub async fn RouteS04_EventScript_MurM3(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M3).await;
}

pub async fn RouteS04_EventScript_MurM4(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M4).await;
}

pub async fn RouteS04_EventScript_MurM5(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M5).await;
}

pub async fn RouteS04_EventScript_MurM6(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M6).await;
}

pub async fn RouteS04_EventScript_MurM7(ctx: Ctx) {
    wall(&ctx, Route_S04_Mur_M7).await;
}

// ---- signs and NPCs ----

pub async fn RouteS04_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S04_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS04_EventScript_SignChantier(ctx: Ctx) {
    msgbox(&ctx, Route_S04_Sign_Chantier, MsgBox::Sign).await;
}

pub async fn RouteS04_EventScript_Ouvrier(ctx: Ctx) {
    msgbox(&ctx, Route_S04_Npc_Ouvrier, MsgBox::Npc).await;
}

pub async fn RouteS04_EventScript_Fetard(ctx: Ctx) {
    msgbox(&ctx, Route_S04_Npc_Fetard, MsgBox::Npc).await;
}

// ---- trainers (§5; sight from the map's trainer objects) ----

/// MONTAGNARD BERNARD (9, 9), in the hall.
pub async fn RouteS04_EventScript_Bernard(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S04_BERNARD,
        TRAINER_SIL_ROUTE_S04_MINEUR,
        Route_S04_Bernard_Intro,
        Route_S04_Bernard_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S04_Bernard_Defeat).await;
    }
}

/// GUITARISTE KEVIN (14, 4), in the hall.
pub async fn RouteS04_EventScript_Kevin(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S04_KEVIN,
        TRAINER_SIL_ROUTE_S04_TEUFEUR,
        Route_S04_Kevin_Intro,
        Route_S04_Kevin_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S04_Kevin_Defeat).await;
    }
}

/// KINESISTE LUNA (21, 5), in the gallery.
pub async fn RouteS04_EventScript_Luna(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S04_LUNA,
        TRAINER_SIL_ROUTE_S04_KINESISTE,
        Route_S04_Luna_Intro,
        Route_S04_Luna_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S04_Luna_Defeat).await;
    }
}

/// The Cocket grunt (34, 9), facing up across the whole gallery.
pub async fn RouteS04_EventScript_Sbire(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S04_SBIRE,
        TRAINER_SIL_ROUTE_S04_SBIRE,
        Route_S04_Sbire_Intro,
        Route_S04_Sbire_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S04_Sbire_Defeat).await;
    }
}

/// TRIATHLETE ROXANE (46, 4), on the ramp.
pub async fn RouteS04_EventScript_Roxane(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_S04_ROXANE,
        TRAINER_SIL_ROUTE_S04_COURSIERE,
        Route_S04_Roxane_Intro,
        Route_S04_Roxane_Defeat,
    )
    .await
    {
        post_battle(&ctx, Route_S04_Roxane_Defeat).await;
    }
}

// ---- items (§7) ----

pub async fn RouteS04_EventScript_SuperRepousse(ctx: Ctx) {
    find_item(&ctx, ITEM_SUPER_REPEL, 1).await;
}

pub async fn RouteS04_EventScript_PierreLune(ctx: Ctx) {
    find_item(&ctx, ITEM_MOON_STONE, 1).await;
}

pub async fn RouteS04_EventScript_HyperPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_HYPER_POTION, 1).await;
}

pub async fn RouteS04_EventScript_Corde(ctx: Ctx) {
    find_item(&ctx, ITEM_ESCAPE_ROPE, 1).await;
}

/// The Ether under M6 (50, 3).
pub async fn RouteS04_EventScript_HiddenEther(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the M9 finding of SIL1
    // Task F): a sign on the cell (`06_ch4_scripts.ron`) runs `hidden_item`.
    hidden_item(&ctx, FLAG_SIL_ITEM_ROUTE_S04_ETHER, ITEM_ETHER).await;
}
