//! Chapter 2, the Tour d'Observation (`GrisEchec_TourObservation` and `_Top`):
//! `contenu-jeu/chapitres/02-gris-echec.md` §2 (the hall, the screens), §3 Tour_Sc01 (Duval and
//! the Carte Perforée, `VAR_SIL_STORY` 24 → 25) and §5 (the two sbires). The door opens with
//! `FLAG_SIL_TOUR_OPEN` (Gris_Echec_Sc03, `ch02_town.rs`).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["GrisEchec_TourObservation", "GrisEchec_TourObservation_Top"];

// ---- the hall ----

/// Sbire tour 1, at the gate.
pub async fn GrisEchec_TourObservation_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_SBIRE_1,
        TRAINER_SIL_SBIRE_TOUR_1,
        Tour_Sbire1_Intro,
        Tour_Sbire1_Defeat,
    )
    .await
    {
        msgbox(&ctx, Tour_Sbire1_Defeat, MsgBox::Autoclose).await;
    }
}

/// Sbire tour 2, before the lift.
pub async fn GrisEchec_TourObservation_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_SBIRE_2,
        TRAINER_SIL_SBIRE_TOUR_2,
        Tour_Sbire2_Intro,
        Tour_Sbire2_Defeat,
    )
    .await
    {
        msgbox(&ctx, Tour_Sbire2_Defeat, MsgBox::Autoclose).await;
    }
}

/// The surveillance screens (Tour_Ecran).
pub async fn GrisEchec_TourObservation_EventScript_Ecran(ctx: Ctx) {
    msgbox(&ctx, Tour_Ecran, MsgBox::Sign).await;
}

// ---- the glass room ----

/// Tour_Sc01, the head of security DUVAL (5, 2): his speech, the battle, the Carte Perforée
/// (`FLAG_SIL_CARTE_PERFOREE`, a key item; T3 announces it and shows its code). Sets
/// `VAR_SIL_STORY` 25. Talked to again, his parting line (T2).
pub async fn GrisEchec_TourObservation_Top_Sc01(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_CARTE_PERFOREE).await {
        msgbox(&ctx, Tour_Sc01_T2, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Tour_Sc01_T1).await;
    battle_trainer(&ctx, TRAINER_SIL_CHEF_SECURITE_DUVAL, Tour_Sc01_Defeat).await;
    say(&ctx, Tour_Sc01_T2).await;
    gift(
        &ctx,
        FLAG_SIL_CARTE_PERFOREE,
        ITEM_SIL_CARTE_PERFOREE,
        1,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Tour_Sc01_T3).await;
    if story(&ctx).await < 25 {
        set_story(&ctx, 25).await;
    }
    ctx.release().await;
}

/// The consoles: the source gives them no line; the surveillance screens' (Tour_Ecran) stands
/// in.
pub async fn GrisEchec_TourObservation_Top_EventScript_Console(ctx: Ctx) {
    msgbox(&ctx, Tour_Ecran, MsgBox::Sign).await;
}
