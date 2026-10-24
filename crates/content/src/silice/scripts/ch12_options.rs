//! Chapter 12, the post-game, optional scenes: Benou's lab, the Zone 404 and its Core
//! (`contenu-jeu/chapitres/12-post-game.md` §3 `Benou_Lab_PG_Sc23` to `Zone_404_PG_Sc26b`).
//!
//! Benou's lab under Nébula (from 126): Benou's battle and his two Link Stones, the Debug Key
//! installed (Silas's Point Zéro), the blast door opened once the Beacon shines (130), the
//! original logs on his PC. The Zone 404 behind the door: six fragments joined by missing tiles
//! (the map's warps: the right tile leads on, the two others back to the fragment's start), one
//! sign each, four glitched trainers. Its Core: the player's Clone (a copy of the party), then
//! Mewtwo (its clear point: the Clone's defeat).

use super::ch08_evalon::lost;
use super::ch12_reveil::{battle_lost, legendaire};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BenouLab", "Zone404", "Zone404_Noyau"];

// ---- Benou's lab ----

/// `Benou_Lab_PG_Sc23`, the System Architect (talking to Benou, from 126): his battle once
/// (`FLAG_SIL_BENOU_BATTU`: two Link Stones), then the Debug Key installed if the player has it,
/// or the blast door opened from 130, or his reminder. Before 126 he only turns.
pub async fn BenouLab_PG_Sc23(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if story(&ctx).await < 126 {
        ctx.release().await;
        return;
    }
    if !ctx.flag(FLAG_SIL_BENOU_BATTU).await {
        say(&ctx, Benou_Lab_PG_Sc23_T1).await;
        say(&ctx, Benou_Lab_PG_Sc23_T2).await;
        if battle_lost(&ctx, TRAINER_SIL_BENOU_9, Benou_Lab_PG_Sc23_T3).await {
            return;
        }
        give_item(&ctx, ITEM_SIL_PIERRE_LIEN, 2).await;
        say(&ctx, Benou_Lab_PG_Sc23_T3b).await;
        ctx.set_flag(FLAG_SIL_BENOU_BATTU, true).await;
    }
    if ctx.flag(FLAG_SIL_CLE_DEBUG).await && !ctx.flag(FLAG_SIL_CLE_DEBUG_INSTALLEE).await {
        say(&ctx, Benou_Lab_PG_Sc23_T4).await;
        ctx.fade(Fade::ToBlack).await;
        ctx.delay(60).await;
        ctx.fade(Fade::FromBlack).await;
        msgbox(&ctx, Benou_Lab_PG_Sc23_T5, MsgBox::Default).await;
        ctx.set_flag(FLAG_SIL_CLE_DEBUG_INSTALLEE, true).await;
        say(&ctx, Benou_Lab_PG_Sc23_T6).await;
    } else if story(&ctx).await >= 130 && !ctx.flag(FLAG_SIL_ZONE404_OUVERTE).await {
        say(&ctx, Benou_Lab_PG_Sc23_T7).await;
        // The source's `set_metatile(BENOU_PORTE_BLINDEE, OUVERTE)`: the door is an object here
        // (its flag is `FLAG_SIL_ZONE404_OUVERTE`), removed.
        play_se(&ctx, "SE_DOOR").await;
        hide(&ctx, LOCALID_BENOU_LAB_PORTE).await;
        ctx.set_flag(FLAG_SIL_ZONE404_OUVERTE, true).await;
    } else {
        say(&ctx, Benou_Lab_PG_Sc23_T8).await;
    }
    ctx.release().await;
}

/// `Benou_Lab_PG_Sc24`, the original logs (the left PC, once Benou is beaten): read in order, with
/// "next?" between them (the source's own fallback for the multichoice), the Casino's proof as a
/// ninth when the Mallette copied it (`FLAG_SIL_CASINO_PREUVE`). Before, the PC is silent.
pub async fn BenouLab_PG_Sc24(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_BENOU_BATTU).await {
        return;
    }
    const LOGS: [TextId; 9] = [
        Benou_Lab_PG_Sc24_L1,
        Benou_Lab_PG_Sc24_L2,
        Benou_Lab_PG_Sc24_L3,
        Benou_Lab_PG_Sc24_L4,
        Benou_Lab_PG_Sc24_L5,
        Benou_Lab_PG_Sc24_L6,
        Benou_Lab_PG_Sc24_L7,
        Benou_Lab_PG_Sc24_L8,
        Benou_Lab_PG_Sc24_L9,
    ];
    ctx.lock().await;
    let last = if ctx.flag(FLAG_SIL_CASINO_PREUVE).await { 9 } else { 8 };
    for (n, log) in LOGS.iter().take(last).enumerate() {
        msgbox(&ctx, *log, MsgBox::Default).await;
        if n + 1 < last && !ask(&ctx, Benou_Lab_PG_Sc24_T0).await {
            break;
        }
    }
    ctx.set_flag(FLAG_SIL_LOGS_ORIGINAUX, true).await;
    ctx.release().await;
}

/// The blast door (shown until 130's opening): the source gives it no line; Benou's reminder
/// about it is his.
pub async fn BenouLab_EventScript_Porte(_ctx: Ctx) {}

/// The cork board covered with the Mallette's schematics: the source gives it no line.
pub async fn BenouLab_EventScript_Tableau(_ctx: Ctx) {}

/// The "NE PAS OUVRIR (SÉRIEUX)" sign: the source names it in the map's description but writes
/// it no text entry (an author question); silent.
pub async fn BenouLab_EventScript_PanneauPorte(_ctx: Ctx) {}

// ---- the Zone 404 ----

/// A fragment's sign.
pub async fn Zone404_PG_P1(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P1, MsgBox::Sign).await;
}

pub async fn Zone404_PG_P2(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P2, MsgBox::Sign).await;
}

pub async fn Zone404_PG_P3(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P3, MsgBox::Sign).await;
}

pub async fn Zone404_PG_P4(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P4, MsgBox::Sign).await;
}

pub async fn Zone404_PG_P5(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P5, MsgBox::Sign).await;
}

pub async fn Zone404_PG_P6(ctx: Ctx) {
    msgbox(&ctx, Zone_404_PG_P6, MsgBox::Sign).await;
}

/// Évalon's three marked slabs (1, 12, 20): the marks are the tiles' art; the source gives them
/// no line. The slab's warp is the step beside it.
pub async fn Zone404_EventScript_Dalle1(_ctx: Ctx) {}

pub async fn Zone404_EventScript_Dalle12(_ctx: Ctx) {}

pub async fn Zone404_EventScript_Dalle20(_ctx: Ctx) {}

/// GOGO? (fragment 2).
pub async fn Zone404_EventScript_Gogo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ZONE404_GOGO,
        TRAINER_SIL_ZONE404_GOGO_1,
        Zone_404_PG_Gogo_Intro,
        Zone_404_PG_Gogo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Zone_404_PG_Gogo_Defeat).await;
    }
}

/// ROMAIN? (fragment 3).
pub async fn Zone404_EventScript_Romain(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ZONE404_ROMAIN,
        TRAINER_SIL_ZONE404_ROMAIN_1,
        Zone_404_PG_Romain_Intro,
        Zone_404_PG_Romain_Defeat,
    )
    .await
    {
        post_battle(&ctx, Zone_404_PG_Romain_Defeat).await;
    }
}

/// POUSSIN? (fragment 5).
pub async fn Zone404_EventScript_Poussin(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ZONE404_POUSSIN,
        TRAINER_SIL_ZONE404_POUSSIN_1,
        Zone_404_PG_Poussin_Intro,
        Zone_404_PG_Poussin_Defeat,
    )
    .await
    {
        post_battle(&ctx, Zone_404_PG_Poussin_Defeat).await;
    }
}

/// ????? (fragment 6).
pub async fn Zone404_EventScript_Inconnu(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ZONE404_INCONNU,
        TRAINER_SIL_ZONE404_ERREUR_1,
        Zone_404_PG_Erreur_Intro,
        Zone_404_PG_Erreur_Defeat,
    )
    .await
    {
        post_battle(&ctx, Zone_404_PG_Erreur_Defeat).await;
    }
}

pub async fn Zone404_EventScript_ItemPPMax(ctx: Ctx) {
    find_item(&ctx, ITEM_PP_MAX, 1).await;
}

pub async fn Zone404_EventScript_ItemRareCandy(ctx: Ctx) {
    find_item(&ctx, ITEM_RARE_CANDY, 1).await;
}

pub async fn Zone404_EventScript_ItemPierreLien(ctx: Ctx) {
    find_item(&ctx, ITEM_SIL_PIERRE_LIEN, 1).await;
}

// ---- the Core ----

const CLONE_FACE: &[Step] = &[Step::FacePlayer];

/// `Zone_404_PG_Sc26`, Noobody's Clone (row 6, three cells before it, until
/// `FLAG_SIL_CLONE_BATTU`, the Clone's map flag): no words, a copy of the player's party
/// (`battle_mirror`, under the starter's Clone trainer: the source's fallback names them), then
/// the Clone comes undone and Mewtwo appears (its clear point). A loss: whiteout, the Clone waits.
pub async fn Zone404_Noyau_PG_Sc26(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_CLONE_BATTU).await {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_NOYAU_CLONE, CLONE_FACE).await;
    msgbox(&ctx, Zone_404_PG_Sc26_T1, MsgBox::Default).await;
    let clone = match ctx.var(VAR_SIL_STARTER).await {
        0 => TRAINER_SIL_CLONE_1,
        1 => TRAINER_SIL_CLONE_2,
        _ => TRAINER_SIL_CLONE_3,
    };
    if lost(battle_mirror(&ctx, clone, Zone_404_PG_Sc26_T2).await) {
        return;
    }
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_NOYAU_CLONE).await;
    show(&ctx, LOCALID_NOYAU_MEWTWO, FLAG_SIL_HIDE_MEWTWO).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_CLONE_BATTU, true).await;
    ctx.release_all().await;
}

/// `Zone_404_PG_Sc26b`, the Prototype (Mewtwo, level 85): caught, it leaves both Mega Stones.
pub async fn Zone404_Noyau_PG_Sc26b(ctx: Ctx) {
    legendaire(
        &ctx,
        Zone_404_PG_Sc26b_T1,
        SPECIES_MEWTWO,
        85,
        LOCALID_NOYAU_MEWTWO,
    )
    .await;
    if ctx.flag(FLAG_SIL_HIDE_MEWTWO).await {
        ctx.lock().await;
        msgbox(&ctx, Zone_404_PG_Sc26b_T2, MsgBox::Default).await;
        give_item(&ctx, ITEM_SIL_MEWTWONITE_X, 1).await;
        give_item(&ctx, ITEM_SIL_MEWTWONITE_Y, 1).await;
        ctx.release().await;
    }
}
