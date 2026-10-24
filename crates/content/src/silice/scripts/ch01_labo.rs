//! Chapter 1, the Brico-Bourg Labo (`BricoBourg_Labo`, optional): `contenu-jeu/chapitres/
//! 01-bois-ether.md` §2 and §3 Brico_Labo_Sc01. Reached after the badge, through the brambles
//! (Brico-Bourg (4, 9)) that the Lame-Frequence cuts. `VAR_SIL_BRICO_LABO_STATE`: 0 never
//! entered, 1 entered, 2 the Chimiste beaten.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BricoBourg_Labo"];

/// The Chimiste is gone once beaten. The strobes flash on every visit (§2: a night club; the
/// source does not stop them once he is gone).
pub async fn BricoBourg_Labo_OnTransition(ctx: Ctx) {
    let state = ctx.var(VAR_SIL_BRICO_LABO_STATE).await;
    ctx.set_flag(FLAG_SIL_HIDE_LABO_CHIMISTE, state == 2).await;
    set_palette(&ctx, STROBES, 0, false).await;
}

/// Brico_Labo_Sc01, the night club: one step in from the mat (6–7, 10), the first time
/// (`VAR_SIL_BRICO_LABO_STATE` 0). Sets 1.
pub async fn BricoBourg_Labo_Sc01(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Brico_Labo_Sc01_T1).await;
    ctx.set_var(VAR_SIL_BRICO_LABO_STATE, 1).await;
    ctx.release().await;
}

/// The Chimiste (2, 1), the boss: talked to, his intro, the battle, his confession; then he
/// slips away (`VAR_SIL_BRICO_LABO_STATE` 2). A loss whites out.
pub async fn BricoBourg_Labo_EventScript_Chimiste(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Brico_Labo_Chimiste_Intro).await;
    battle_trainer(&ctx, TRAINER_SIL_CHIMISTE_LABO, Brico_Labo_Chimiste_Defeat).await;
    say(&ctx, Brico_Labo_Chimiste_Apres).await;
    ctx.set_var(VAR_SIL_BRICO_LABO_STATE, 2).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_LABO_CHIMISTE).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.release().await;
}

/// SBIRE 1 (1, 8), facing right, sight 5.
pub async fn BricoBourg_Labo_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_LABO_SBIRE_1,
        TRAINER_SIL_SBIRE_LABO_1,
        Brico_Labo_Sbire1_Intro,
        Brico_Labo_Sbire1_Defeat,
    )
    .await
    {
        post_battle(&ctx, Brico_Labo_Sbire1_Defeat).await;
    }
}

/// SBIRE 2 (2, 3), facing left, sight 4: she watches the way up to the Chimiste.
pub async fn BricoBourg_Labo_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_LABO_SBIRE_2,
        TRAINER_SIL_SBIRE_LABO_2,
        Brico_Labo_Sbire2_Intro,
        Brico_Labo_Sbire2_Defeat,
    )
    .await
    {
        post_battle(&ctx, Brico_Labo_Sbire2_Defeat).await;
    }
}

/// The chest's TM06 Toxic (12, 3).
pub async fn BricoBourg_Labo_EventScript_ItemTm(ctx: Ctx) {
    find_item(&ctx, ITEM_TM06, 1).await;
}

/// The Silice dust on the bench (6, 7): the jar's label (Labo_Echantillon, which says it is
/// obtained), the key item, the ball gone (`FLAG_SIL_ITEM_BRICO_LABO_SILICE`, its hide flag).
pub async fn BricoBourg_Labo_EventScript_ItemSilice(ctx: Ctx) {
    ctx.lock().await;
    if add_item(&ctx, ITEM_SIL_POUSSIERE_SILICE, 1).await {
        ctx.remove_object(LOCALID_LABO_ITEM_SILICE).await;
        say(&ctx, Labo_Echantillon).await;
    }
    ctx.release().await;
}

/// The Revive on the floor (9, 9).
pub async fn BricoBourg_Labo_EventScript_ItemRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_REVIVE, 1).await;
}

/// The memo on the desk (11–12, 2).
pub async fn BricoBourg_Labo_EventScript_Memo(ctx: Ctx) {
    msgbox(&ctx, Labo_Memo, MsgBox::Sign).await;
}
