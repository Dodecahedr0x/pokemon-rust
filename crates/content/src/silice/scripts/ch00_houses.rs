//! Chapter 0, the houses: Noobody's (`BricoBourg_PlayerHouse_1F`, `_2F`) and Adrien's family's
//! (`BricoBourg_FriendsHouse`). `contenu-jeu/chapitres/00-prologue.md` §2 and §3
//! Player_House_Sc01 to Sc03: `VAR_SIL_STORY` 1 (home with the Mallette) to 3 (the morning).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "BricoBourg_PlayerHouse_1F",
    "BricoBourg_PlayerHouse_2F",
    "BricoBourg_FriendsHouse",
];

// ---- 1F ----

/// Player_House_Sc01, Maman sees nothing: on the 1F's first frame while `VAR_SIL_STORY` is 1
/// (the map's on-frame entry, so a coord event cannot be walked around). Sets 2. The player has
/// the Mallette: Brico-Bourg's Sc01 cells, around the Shaker's exit, keep them in the bar until
/// it is taken.
pub async fn BricoBourg_PlayerHouse_1F_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    const FACE_DOWN: &[Step] = &[face(Dir::Down)];
    move_wait(&ctx, LOCALID_PLAYER_HOUSE_MAMAN, FACE_DOWN).await;
    say(&ctx, Player_House_Sc01_T1).await;
    set_story(&ctx, 2).await;
    ctx.release_all().await;
}

/// MAMAN: Player_House_Sc03's 1F part from the morning on (`VAR_SIL_STORY` ≥ 3; the source
/// says 3): her line, the running shoes once, a heal (the house is Ch0's heal point). The night
/// before (not in the source), her Sc01 line again.
pub async fn BricoBourg_PlayerHouse_1F_EventScript_Maman(ctx: Ctx) {
    if story(&ctx).await < 3 {
        msgbox(&ctx, Player_House_Sc01_T1, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Player_House_Sc03_T2).await;
    // The shoes are a flag, not an item (`game.ron`'s running flag): given once.
    if !ctx.flag(FLAG_SIL_RUNNING_SHOES).await {
        ctx.set_flag(FLAG_SIL_RUNNING_SHOES, true).await;
        say(&ctx, Player_House_Sc03_T3).await;
    }
    ctx.heal_party().await;
    ctx.release().await;
}

/// The TV.
pub async fn BricoBourg_PlayerHouse_1F_EventScript_Tele(ctx: Ctx) {
    msgbox(&ctx, Player_House_Tele, MsgBox::Sign).await;
}

// ---- 2F ----

/// Player_House_Sc02 trigger A, the desk (`VAR_SIL_STORY` 2): the Mallette will not open.
/// Nothing at other steps.
pub async fn BricoBourg_PlayerHouse_2F_EventScript_Bureau(ctx: Ctx) {
    if story(&ctx).await != 2 {
        return;
    }
    ctx.lock().await;
    if ctx.flag(FLAG_SIL_MALLETTE_TRIED).await {
        say(&ctx, Player_House_Sc02_T2).await;
    } else {
        say(&ctx, Player_House_Sc02_T1).await;
        ctx.set_flag(FLAG_SIL_MALLETTE_TRIED, true).await;
    }
    ctx.release().await;
}

/// Player_House_Sc02 trigger B, the bed (`VAR_SIL_STORY` 2): after the desk, sleep through the
/// night (heal, story 3), then Player_House_Sc03's 2F part, the friends' voices outside.
/// Nothing at other steps.
pub async fn BricoBourg_PlayerHouse_2F_EventScript_Lit(ctx: Ctx) {
    if story(&ctx).await != 2 {
        return;
    }
    ctx.lock().await;
    if !ctx.flag(FLAG_SIL_MALLETTE_TRIED).await {
        say(&ctx, Player_House_Sc02_T3).await;
        ctx.release().await;
        return;
    }
    if !ask(&ctx, Player_House_Sc02_T4).await {
        ctx.release().await;
        return;
    }
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.delay(60).await;
    say(&ctx, Player_House_Sc02_T5).await;
    set_story(&ctx, 3).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.release().await;
    // Player_House_Sc03, chained after the fade.
    ctx.lock_all().await;
    say(&ctx, Player_House_Sc03_T1).await;
    ctx.release_all().await;
}

/// The PC. Chapter 12: it opens on Silas's mail first, once, before its usual line
/// (`Brico_Bourg_PG_Sc09`, `ch12_reveil.rs`).
pub async fn BricoBourg_PlayerHouse_2F_EventScript_Pc(ctx: Ctx) {
    super::ch12_reveil::pg_mail_silas(&ctx).await;
    msgbox(&ctx, Player_House_Pc, MsgBox::Sign).await;
}

/// The window.
pub async fn BricoBourg_PlayerHouse_2F_EventScript_Fenetre(ctx: Ctx) {
    msgbox(&ctx, Player_House_Fenetre, MsgBox::Sign).await;
}

// ---- Adrien's family's house ----

/// Adrien's mother at the stove.
pub async fn BricoBourg_FriendsHouse_EventScript_Mere(ctx: Ctx) {
    msgbox(&ctx, Friends_House_Mere, MsgBox::Npc).await;
}

/// The sleeping Skitty: it does not turn (a sign box).
pub async fn BricoBourg_FriendsHouse_EventScript_Skitty(ctx: Ctx) {
    msgbox(&ctx, Friends_House_Skitty, MsgBox::Sign).await;
}

/// The film poster.
pub async fn BricoBourg_FriendsHouse_EventScript_Affiche(ctx: Ctx) {
    msgbox(&ctx, Friends_House_Affiche, MsgBox::Sign).await;
}
