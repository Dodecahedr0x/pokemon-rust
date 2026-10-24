//! Chapter 8, the Tour du Scribe (`Evalon_TourScribe`, `_2F`, `_3F`): Evalon_Sc03, the Épreuve
//! du Scribe (`contenu-jeu/chapitres/08-evalon.md` §3 Sc03): the iron-bound bookcases (1F), the
//! iron crates on the copper slabs and the Copiste (2F), the Chronique de la Clôture (3F), the
//! Scribe's validation.
//!
//! **The Champ Magnétique** is Gen3's Strength (`silice::push`: an A press on the object puts it
//! in use, `ch04_port::push_object`; then the player pushes it by walking into it, and
//! `scripts::field::Mallette_Pushed` runs [`rayonnage_pushed`] / [`caisse_pushed`]). The
//! objects are back on their template cells on every load (Gen3's Strength boulders), so:
//! - 1F: once both bookcases are in their niches, `FLAG_SIL_EVALON_TOUR_RAYONNAGES` keeps them
//!   there (`Evalon_TourScribe_OnTransition`), or a player coming down from 2F would be shut
//!   in behind them;
//! - 2F: once the four crates are on the slabs, `FLAG_SIL_EVALON_TOUR_TRAPPE` keeps the trapdoor
//!   open (`Evalon_TourScribe_2F_OnLoad`); the crates go home, which blocks nothing.
//!
//! Temp vars: 1F `VAR_TEMP_1`, `_2`, `_5` (the bookcases' cells), 2F `VAR_TEMP_1`, `_2`, `_5`,
//! `_6` (the crates').

use super::ch04_port::{Puzzle, push_object};
use super::ch08_evalon::{add_renom, epreuves_finies};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "Evalon_TourScribe",
    "Evalon_TourScribe_2F",
    "Evalon_TourScribe_3F",
];

use crate::silice::push::cell_var;

// ---- 1F: the bookcases ----

/// The three bookcases: 1 (5, 7) goes left into its niche (4, 7), 2 (8, 4) up into its niche
/// (8, 3), 3 (4, 5) is the decoy against the wall ("SIL4 Ch8 maps as built").
pub(crate) const RAYONNAGES: Puzzle = Puzzle {
    map: MAP_EVALON_TOUR_SCRIBE,
    objects: &[
    (LOCALID_TOUR_RAYONNAGE_1, VAR_TEMP_1, (5, 7)),
    (LOCALID_TOUR_RAYONNAGE_2, VAR_TEMP_2, (8, 4)),
    (LOCALID_TOUR_RAYONNAGE_3, VAR_TEMP_5, (4, 5)),
],
    free: rayonnage_free,
};
const NICHE_1: (i16, i16) = (4, 7);
const NICHE_2: (i16, i16) = (8, 3);

/// Each bookcase moves only the right way: into its niche.
fn rayonnage_free(_: (i16, i16), c: (i16, i16)) -> bool {
    c == NICHE_1 || c == NICHE_2
}

/// Both bookcases in their niches from `FLAG_SIL_EVALON_TOUR_RAYONNAGES` (Gen3
/// `setobjectxyperm`).
pub async fn Evalon_TourScribe_OnTransition(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_TOUR_RAYONNAGES).await {
        ctx.set_object_template_pos(LOCALID_TOUR_RAYONNAGE_1, NICHE_1.0, NICHE_1.1)
            .await;
        ctx.set_object_template_pos(LOCALID_TOUR_RAYONNAGE_2, NICHE_2.0, NICHE_2.1)
            .await;
    }
}

/// An iron-bound bookcase (Champ Magnétique). Once both are in their niches, the flag keeps
/// them there; they move no more.
pub async fn Evalon_TourScribe_EventScript_Rayonnage(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_TOUR_RAYONNAGES).await {
        return;
    }
    push_object(&ctx).await;
}

/// After a bookcase's push (`scripts::field::Mallette_Pushed`): both in their niches, the flag,
/// and Strength no more on this load.
pub(crate) async fn rayonnage_pushed(ctx: &Ctx) {
    if ctx.var(VAR_TEMP_1).await == cell_var(NICHE_1)
        && ctx.var(VAR_TEMP_2).await == cell_var(NICHE_2)
    {
        ctx.set_flag(FLAG_SIL_EVALON_TOUR_RAYONNAGES, true).await;
        ctx.set_flag(FLAG_SYS_USE_STRENGTH, false).await;
    }
}

/// The lectern's hint (3, 8).
pub async fn Evalon_TourScribe_EventScript_Lutrin(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Lutrin_T1, MsgBox::Sign).await;
}

/// Evalon_Sc03, the Scribe (7, 8): with the Chronique, the trial passed
/// (`FLAG_SIL_EVALON_EPREUVE_SCRIBE`, +2 reputation, the trials' end); without it, the rule.
/// Afterwards, his validation line again.
pub async fn Evalon_Sc03(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_EVALON_EPREUVE_SCRIBE).await {
        say(&ctx, Evalon_Sc03_Scribe_T2).await;
    } else if check_item(&ctx, ITEM_SIL_CHRONIQUE, 1).await {
        say(&ctx, Evalon_Sc03_Scribe_T2).await;
        ctx.set_flag(FLAG_SIL_EVALON_EPREUVE_SCRIBE, true).await;
        add_renom(&ctx, 2).await;
        epreuves_finies(&ctx).await;
    } else {
        say(&ctx, Evalon_Sc03_Scribe_T1).await;
    }
    ctx.release().await;
}

// ---- 2F: the crates and the slabs ----

/// The four crates, from the source's 7 × 7 grid at offset (3, 3): A (5, 5) goes left onto
/// (4, 5), B (7, 5) up onto (7, 4), C (5, 7) down onto (5, 8), D (8, 8) right onto (9, 8).
pub(crate) const CAISSES: Puzzle = Puzzle {
    map: MAP_EVALON_TOUR_SCRIBE_2F,
    objects: &[
    (LOCALID_TOUR2_CAISSE_A, VAR_TEMP_1, (5, 5)),
    (LOCALID_TOUR2_CAISSE_B, VAR_TEMP_2, (7, 5)),
    (LOCALID_TOUR2_CAISSE_C, VAR_TEMP_5, (5, 7)),
    (LOCALID_TOUR2_CAISSE_D, VAR_TEMP_6, (8, 8)),
],
    free: caisse_free,
};
const DALLES: [(i16, i16); 4] = [(4, 5), (7, 4), (5, 8), (9, 8)];

/// The room's floor (x 0–12, y 2–11) without the lecterns' and the stepladder's wall cells, the
/// Copiste's cell, and the cells below the two stair warps (a crate there would wall them off).
fn caisse_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    (0..=12).contains(&x)
        && (2..=11).contains(&y)
        && !matches!(
            (x, y),
            (0, 4) | (12, 4) | (0, 8) | (12, 8) | (0, 11) | (11, 10) | (2, 2) | (10, 2)
        )
}

/// The trapdoor (10, 1) open: the ladder (716, walkable) and its frame (715, 717; 707–709 above),
/// Facility's, solid.
const TRAPPE: [(i16, i16, u16, bool); 6] = [
    (10, 1, 716, false),
    (9, 1, 715, true),
    (11, 1, 717, true),
    (9, 0, 707, true),
    (10, 0, 708, true),
    (11, 0, 709, true),
];

async fn open_trappe(ctx: &Ctx) {
    for (x, y, m, blocked) in TRAPPE {
        ctx.set_metatile(x, y, m, blocked).await;
    }
}

/// The trapdoor open from `FLAG_SIL_EVALON_TOUR_TRAPPE`.
pub async fn Evalon_TourScribe_2F_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_TOUR_TRAPPE).await {
        open_trappe(&ctx).await;
    }
}

/// An iron crate (Champ Magnétique). When the four stand on the four slabs, the trapdoor opens
/// and the ladder comes down (`FLAG_SIL_EVALON_TOUR_TRAPPE`); then they move no more.
pub async fn Evalon_TourScribe_2F_EventScript_Caisse(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_EVALON_TOUR_TRAPPE).await {
        return;
    }
    push_object(&ctx).await;
}

/// After a crate's push (`scripts::field::Mallette_Pushed`): the four on the four slabs, the
/// trapdoor (once the crate has stopped), and Strength no more on this load.
pub(crate) async fn caisse_pushed(ctx: &Ctx) {
    for (i, &(_, var, _)) in CAISSES.objects.iter().enumerate() {
        if ctx.var(var).await != cell_var(DALLES[i]) {
            return;
        }
    }
    ctx.set_flag(FLAG_SYS_USE_STRENGTH, false).await;
    ctx.delay(PUSH_FRAMES).await;
    ctx.lock_all().await;
    // The source has no line for the trapdoor: its sound alone (SE_DOOR, the decomp's).
    play_se(ctx, "SE_DOOR").await;
    open_trappe(ctx).await;
    ctx.redraw_map().await;
    ctx.set_flag(FLAG_SIL_EVALON_TOUR_TRAPPE, true).await;
    ctx.release_all().await;
}

/// The Copiste (11, 10), a Ruin Maniac (sight 2 up): the battle, then the TM30 (Ball'Ombre)
/// once. Talked to again, his hint, and he puts the crates back on their cells (the source's
/// reset; nothing to reset once the trapdoor is open).
pub async fn Evalon_TourScribe_2F_EventScript_Copiste(ctx: Ctx) {
    let again = trainer(
        &ctx,
        LOCALID_TOUR2_COPISTE,
        TRAINER_SIL_EVALON_COPISTE,
        Evalon_Copiste_Intro,
        Evalon_Copiste_Defaite,
    )
    .await;
    if !defeated(&ctx, TRAINER_SIL_EVALON_COPISTE).await {
        return;
    }
    if !again {
        ctx.lock().await;
    }
    gift(
        &ctx,
        FLAG_SIL_ITEM_EVALON_CT_BALL_OMBRE,
        ITEM_TM30,
        1,
        Obtain::Message,
    )
    .await;
    if again {
        say(&ctx, Evalon_Sc03_Copiste_T1).await;
        if !ctx.flag(FLAG_SIL_EVALON_TOUR_TRAPPE).await {
            ctx.fade(Fade::ToBlack).await;
            for &(obj, var, (x, y)) in CAISSES.objects {
                ctx.set_object_pos(obj, x, y).await;
                ctx.set_var(var, 0).await;
            }
            ctx.fade(Fade::FromBlack).await;
        }
    }
    ctx.release().await;
}

/// The four numbered lecterns (I–IV): the 1F lectern's maxim (author question: the source
/// numbers them but writes no line for them).
pub async fn Evalon_TourScribe_2F_EventScript_LutrinI(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Lutrin_T1, MsgBox::Sign).await;
}

pub async fn Evalon_TourScribe_2F_EventScript_LutrinII(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Lutrin_T1, MsgBox::Sign).await;
}

pub async fn Evalon_TourScribe_2F_EventScript_LutrinIII(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Lutrin_T1, MsgBox::Sign).await;
}

pub async fn Evalon_TourScribe_2F_EventScript_LutrinIV(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Lutrin_T1, MsgBox::Sign).await;
}

/// The stepladder in the corner (0, 11): the source's reset is the Copiste's; the ladder says
/// his hint (author question).
pub async fn Evalon_TourScribe_2F_EventScript_Escabeau(ctx: Ctx) {
    msgbox(&ctx, Evalon_Sc03_Copiste_T1, MsgBox::Sign).await;
}

// ---- 3F: the Chronique ----

/// The Chronique de la Clôture on its desk (5, 5): read, then taken (a key item, no flag of its
/// own: the bag or the trial says it was taken). Read again afterwards.
pub async fn Evalon_TourScribe_3F_EventScript_Chronique(ctx: Ctx) {
    let taken = ctx.flag(FLAG_SIL_EVALON_EPREUVE_SCRIBE).await
        || check_item(&ctx, ITEM_SIL_CHRONIQUE, 1).await;
    if taken {
        msgbox(&ctx, Evalon_Sc03_Chronique_T1, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    say(&ctx, Evalon_Sc03_Chronique_T1).await;
    add_item(&ctx, ITEM_SIL_CHRONIQUE, 1).await;
    say(&ctx, Evalon_Sc03_Chronique_T2).await;
    ctx.release().await;
}
