//! Chapter 5, the East Cave, the convoys' road (`Owlys_GrotteEst`):
//! `contenu-jeu/chapitres/05-owlys.md` §2 (the sabotaged convoy), §3 Owlys_Grotte_Est_Sc01 (the
//! Grand Praugect's duo battle, Owl-Justice, the Shaded Feather; `VAR_SIL_STORY` 52 → 53), §5
//! (Lilith) and §7 (the items).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Owlys_GrotteEst"];

/// The duo scene's actors (Mathieu, both drivers) wait in the hall at `VAR_SIL_STORY` 52 only;
/// Owl-Justice and the Feather are the scene's (hidden on every load). Added by
/// `edits/Owlys_GrotteEst/05_ch5_scripts.ron`: the map had no transition script, so its actors
/// could never show.
pub async fn Owlys_GrotteEst_OnTransition(ctx: Ctx) {
    let waiting = story(&ctx).await == 52;
    for f in [
        FLAG_SIL_HIDE_OWLYS_GROTTE_EST_MATHIEU,
        FLAG_SIL_HIDE_OWLYS_GROTTE_EST_CONVOYEUR_A,
        FLAG_SIL_HIDE_OWLYS_GROTTE_EST_CONVOYEUR_B,
    ] {
        ctx.set_flag(f, !waiting).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GROTTE_EST_OWL_JUSTICE, true)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_OWLYS_GROTTE_EST_PLUME, true)
        .await;
    // Chapter 12: Silas at the Point Zéro from 123 until his Vestige is done (`Owlys_PG_Sc07`,
    // `ch12_reveil.rs`).
    let silas = story(&ctx).await >= 123 && !ctx.flag(FLAG_SIL_VESTIGE_SILAS).await;
    ctx.set_flag(FLAG_SIL_HIDE_SILAS_GROTTE, !silas).await;
}

/// Mathieu steps up and turns to the gallery.
const PRAUGECT_IN: &[Step] = &[walk(Dir::Up), face(Dir::Right)];
/// The drivers come out of the gallery (`Walk(Left) x2`).
const CONVOI_IN: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];
/// Owl-Justice leaps off the ledge (a jump in place).
const OWL_JUMP: &[Step] = &[Step::Jump(Dir::Down, 0)];
/// Mathieu runs to the feather (`Walk(Up) x2`).
const PRAUGECT_PLUME: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];

/// Owlys_Grotte_Est_Sc01, the Grand Praugect: the row y 19 of the hall at `VAR_SIL_STORY` 52.
/// The duo battle (a multi battle with Praugect as the partner) is the source's fallback: the
/// player fights driver A, Mathieu holds B "off screen", the party is healed, then the player
/// fights B (`_Repli`). Then Owl-Justice on the ledge, the Shaded Feather
/// (`FLAG_SIL_OWLYS_PLUME`), Mathieu leaves. Sets 53.
pub async fn Owlys_GrotteEst_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T1).await;
    move_wait(&ctx, LOCALID_GROTTE_EST_MATHIEU, PRAUGECT_IN).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T2).await;
    ctx.move_obj(LOCALID_GROTTE_EST_CONVOYEUR_A, CONVOI_IN)
        .await;
    move_wait(&ctx, LOCALID_GROTTE_EST_CONVOYEUR_B, CONVOI_IN).await;
    ctx.wait_move(LOCALID_GROTTE_EST_CONVOYEUR_A).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T3).await;
    ctx.heal_party().await;
    // fallback: battle_multi (a multi battle with an AI partner, M11; the source's §9 fallback):
    // two single battles, A then B, with a heal between them and Praugect's `_Repli` line.
    fallback(&ctx, "battle_multi").await;
    battle_trainer(
        &ctx,
        TRAINER_SIL_OWLYS_CONVOYEUR_A,
        Owlys_Grotte_Est_Sc01_DefeatA,
    )
    .await;
    say(&ctx, Owlys_Grotte_Est_Sc01_Repli).await;
    ctx.heal_party().await;
    battle_trainer(
        &ctx,
        TRAINER_SIL_OWLYS_CONVOYEUR_B,
        Owlys_Grotte_Est_Sc01_DefeatB,
    )
    .await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T4).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_GROTTE_EST_CONVOYEUR_A).await;
    hide(&ctx, LOCALID_GROTTE_EST_CONVOYEUR_B).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.delay(30).await;
    show(
        &ctx,
        LOCALID_GROTTE_EST_OWL_JUSTICE,
        FLAG_SIL_HIDE_OWLYS_GROTTE_EST_OWL_JUSTICE,
    )
    .await;
    ctx.turn(LOCALID_GROTTE_EST_MATHIEU, Dir::Up).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T5).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T6).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T7).await;
    move_wait(&ctx, LOCALID_GROTTE_EST_OWL_JUSTICE, OWL_JUMP).await;
    hide(&ctx, LOCALID_GROTTE_EST_OWL_JUSTICE).await;
    show(
        &ctx,
        LOCALID_GROTTE_EST_PLUME,
        FLAG_SIL_HIDE_OWLYS_GROTTE_EST_PLUME,
    )
    .await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T8).await;
    move_wait(&ctx, LOCALID_GROTTE_EST_MATHIEU, PRAUGECT_PLUME).await;
    hide(&ctx, LOCALID_GROTTE_EST_PLUME).await;
    gift(
        &ctx,
        FLAG_SIL_OWLYS_PLUME,
        ITEM_SIL_PLUME_OMBREE,
        1,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T9).await;
    move_wait(&ctx, LOCALID_GROTTE_EST_MATHIEU, &[Step::FacePlayer]).await;
    say(&ctx, Owlys_Grotte_Est_Sc01_T10).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_GROTTE_EST_MATHIEU).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_OWLYS_PLUME, true).await;
    set_story(&ctx, 53).await;
    ctx.release_all().await;
}

/// The Shaded Feather on the ground. Only Sc01 shows it, and hides it again in the same lock:
/// this is the guard of a feather left behind, picked up once.
pub async fn Owlys_GrotteEst_EventScript_Plume(ctx: Ctx) {
    ctx.lock().await;
    hide(&ctx, LOCALID_GROTTE_EST_PLUME).await;
    gift(
        &ctx,
        FLAG_SIL_OWLYS_PLUME,
        ITEM_SIL_PLUME_OMBREE,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}

/// LILITH, the Hex Maniac of the rail gallery.
pub async fn Owlys_GrotteEst_EventScript_Lilith(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GROTTE_EST_LILITH,
        TRAINER_SIL_OWLYS_GROTTE_MYSTI,
        Owlys_Lilith_Intro,
        Owlys_Lilith_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Lilith_Defeat).await;
    }
}

/// The sabotaged convoy: three empty carts, the note signed O.J.
pub async fn Owlys_GrotteEst_EventScript_Convoi(ctx: Ctx) {
    msgbox(&ctx, Owlys_Grotte_Est_Convoi, MsgBox::Sign).await;
}

/// The Moon Stone on a dark crystal vein.
pub async fn Owlys_GrotteEst_EventScript_ItemPierreLune(ctx: Ctx) {
    find_item(&ctx, ITEM_MOON_STONE, 1).await;
}

/// The Escape Rope in the rail gallery.
pub async fn Owlys_GrotteEst_EventScript_ItemCorde(ctx: Ctx) {
    find_item(&ctx, ITEM_ESCAPE_ROPE, 1).await;
}

