//! Chapter 7, Reverse-City's lighthouse (`ReverseCity_Phare`) and the prototypes' vault
//! (`ReverseCity_Coffre`): `contenu-jeu/chapitres/07-reverse-city.md` §2 (the lighthouse, the
//! vault room), §3 Reverse_City_Sc04 (the vault's dial, the Master Ball, locked until Reverse is
//! beaten: R-8) and Reverse_City_Sc06 (the black-light lamp's TM, Oscar), §5 (Oscar) and §7.

use crate::silice::prelude::*;

use super::ch07_reverse_city::set_input_inverted;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["ReverseCity_Phare", "ReverseCity_Coffre"];

// ---- the lighthouse ----

/// Normal controls (an interior, as the source lists the Phare). The dark ramp and the lamp's
/// black light are not drawn.
pub async fn ReverseCity_Phare_OnTransition(ctx: Ctx) {
    set_input_inverted(&ctx, false).await;
    // fallback: the dark map and its light (`cave: 1` and the Surcharge Optique module, as
    // Chapter 4's tunnel): the ramp is drawn lit.
    fallback(&ctx, "dark_map").await;
    // fallback: the lamp's black light (§9, an upper-layer beam sweeping the city): no
    // animation (the source's own fallback).
    fallback(&ctx, "black_light").await;
}

/// GARDIEN DE L'OMBRE OSCAR, half-way up the ramp (11, 13), sight 1 right: the only way up.
pub async fn ReverseCity_Phare_EventScript_Oscar(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_PHARE_OSCAR,
        TRAINER_SIL_GARDIEN_OSCAR,
        Phare_Oscar_Intro,
        Phare_Oscar_Defaite,
    )
    .await
    {
        post_battle(&ctx, Phare_Oscar_Defaite).await;
    }
}

/// Reverse_City_Sc06, the lamp's base (the TM ball (6, 7)): the lamp, TM11 Sunny Day ("CT
/// Zénith", §10), the ball gone (`FLAG_SIL_ITEM_PHARE_TM_ZENITH`, its hide flag), `Phare_T1`.
/// With a full TM pocket nothing is taken and the ball stays.
pub async fn ReverseCity_Phare_Sc06(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_PHARE_TM_ZENITH).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Phare_Sign_Lampe).await;
    if give_item(&ctx, ITEM_TM11, 1).await {
        hide(&ctx, LOCALID_PHARE_ITEM_TM_ZENITH).await;
        ctx.set_flag(FLAG_SIL_ITEM_PHARE_TM_ZENITH, true).await;
        say(&ctx, Phare_T1).await;
    }
    ctx.release().await;
}

/// Half-way up, in the dark (1, 11).
pub async fn ReverseCity_Phare_EventScript_ItemMaxEther(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_ETHER, 1).await;
}

// ---- the vault ----

/// Normal controls (an interior).
pub async fn ReverseCity_Coffre_OnTransition(ctx: Ctx) {
    set_input_inverted(&ctx, false).await;
}

/// The four plaques of the mirror signs (`ch07_reverse_city.rs`), read 4 to 1: 6-1-8-3.
const PLAQUES: [FlagId; 4] = [
    FLAG_SIL_REVERSE_PLAQUE_4,
    FLAG_SIL_REVERSE_PLAQUE_3,
    FLAG_SIL_REVERSE_PLAQUE_2,
    FLAG_SIL_REVERSE_PLAQUE_1,
];

/// Reverse_City_Sc04, the prototypes' safe (the dial, read facing up from (4–5, 8)): locked
/// while Reverse's signal runs (R-8: until `FLAG_SIL_REVERSE_BATTU`); then the dial, once
/// (`FLAG_SIL_ITEM_REVERSE_MASTER_BALL`). A wrong code shocks and costs nothing.
///
/// The source's dial is a four-code `multichoice` (3816, 6183, 1638, 8361), and its fallback
/// four yes/nos ("Composer 3816 ?"), whose texts are not written. The fallback here: `Coffre_T1`
/// ends on "which code?" as a yes/no; on yes, the player dials what the plaques gave: the right
/// code once all four mirror signs have been read (`FLAG_SIL_REVERSE_PLAQUE_1`–`_4`), else a
/// wrong one.
pub async fn ReverseCity_Coffre_Sc04(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ITEM_REVERSE_MASTER_BALL).await {
        return;
    }
    ctx.lock().await;
    if !ctx.flag(FLAG_SIL_REVERSE_BATTU).await {
        say(&ctx, Coffre_T0_Verrouille).await;
        ctx.release().await;
        return;
    }
    // fallback: multichoice([3816, 6183, 1638, 8361]) (§9): one yes/no, the plaques' code.
    fallback(&ctx, "multichoice").await;
    if !ask(&ctx, Coffre_T1).await {
        ctx.release().await;
        return;
    }
    let mut code_known = true;
    for f in PLAQUES {
        code_known &= ctx.flag(f).await;
    }
    if code_known {
        // fallback: play_se(SE_COFFRE) (no sound yet; the decomp's SE_UNLOCK stands for it).
        play_se(&ctx, "SE_UNLOCK").await;
        say(&ctx, Coffre_T2).await;
        if give_item(&ctx, ITEM_MASTER_BALL, 1).await {
            ctx.set_flag(FLAG_SIL_ITEM_REVERSE_MASTER_BALL, true).await;
            say(&ctx, Coffre_T3).await;
        }
    } else {
        say(&ctx, Coffre_T4).await;
    }
    ctx.release().await;
}
