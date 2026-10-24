//! Chapter 10, 3F: l'Horizon Virtuel, Arthur's pillar (`TourNebula_3F`; `10-tour-piliers.md`
//! §3 `Tour_3F_Sc01`–`Sc05`, §4.3, the elites Yann and Maëlle).
//!
//! The way: the virtual sea between the quay and the helm, crossed on the hidden 14-cell path the
//! three origami boats give (from (11, 11): north 4, east 3, north 3, west 3, north to (11, 3));
//! every other sea cell is a fall coord (`TourNebula_3F_Sc03`, story 104) back to the quay. The
//! compass on the desk is optional: given back before the battle, Arthur fights without items
//! (`TRAINER_SIL_ARTHUR_2`).

use super::ch08_evalon::lost;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TourNebula_3F"];

/// The screen behind Arthur (12, 1), open from story 105 (the map puts the exit one east of the
/// source's (11, 1)).
const ECRAN: (i16, i16, u16) = (12, 1, 700);
/// The quay cell a fall brings the player back to.
const QUAI: (i16, i16) = (11, 14);

/// The exit from story 105.
pub async fn TourNebula_3F_OnLoad(ctx: Ctx) {
    if story(&ctx).await >= 105 {
        ctx.set_metatile(ECRAN.0, ECRAN.1, ECRAN.2, false).await;
    }
}

/// `Tour_3F_Sc01`, the Horizon (once, `FLAG_SIL_TOUR_3F_INTRO`). Coords round the landing on
/// `VAR_TEMP_1` 0.
pub async fn TourNebula_3F_Sc01(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_TOUR_3F_INTRO).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Tour_3F_Sc01_T1).await;
    ctx.set_flag(FLAG_SIL_TOUR_3F_INTRO, true).await;
    ctx.release_all().await;
}

/// `Tour_3F_Sc02`, the compass on the desk (8, 13): taken to give it back
/// (`FLAG_SIL_ARTHUR_BOUSSOLE_PRISE`, `ITEM_SIL_BOUSSOLE_ARTHUR`).
pub async fn TourNebula_3F_Sc02(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_PRISE).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Tour_3F_Sc02_T1).await;
    if ask(&ctx, Tour_3F_Sc02_T2).await {
        hide(&ctx, LOCALID_TOUR_3F_BOUSSOLE).await;
        add_item(&ctx, ITEM_SIL_BOUSSOLE_ARTHUR, 1).await;
        ctx.set_flag(FLAG_SIL_ARTHUR_BOUSSOLE_PRISE, true).await;
        say(&ctx, Tour_3F_Sc02_T3).await;
    }
    ctx.release().await;
}

/// `Tour_3F_Sc03`, a fall into the virtual sea (every sea cell off the path, story 104): back
/// to the quay.
pub async fn TourNebula_3F_Sc03(ctx: Ctx) {
    ctx.lock_all().await;
    msgbox(&ctx, Tour_3F_Sc03_T1, MsgBox::Autoclose).await;
    warp(&ctx, MAP_TOUR_NEBULA_3F, QUAI.0, QUAI.1).await;
}

/// An origami boat: its heading, and with the compass in the bag, the needle's line.
async fn origami(ctx: &Ctx, text: TextId) {
    ctx.lock().await;
    say(ctx, text).await;
    if check_item(ctx, ITEM_SIL_BOUSSOLE_ARTHUR, 1).await {
        say(ctx, Tour_3F_Origami_Boussole).await;
    }
    ctx.release().await;
}

pub async fn TourNebula_3F_EventScript_Origami1(ctx: Ctx) {
    origami(&ctx, Tour_3F_Origami_1).await;
}
pub async fn TourNebula_3F_EventScript_Origami2(ctx: Ctx) {
    origami(&ctx, Tour_3F_Origami_2).await;
}
pub async fn TourNebula_3F_EventScript_Origami3(ctx: Ctx) {
    origami(&ctx, Tour_3F_Origami_3).await;
}

/// `Tour_3F_Sc04`, Arthur the Navigator (story 104): the compass given back or not, the battle
/// (with or without items), the ACCÈS GAMMA, 95 %, the origami boat for a returned compass, the
/// screen behind him (→ 105). `Tour_3F_Sc05` afterwards.
pub async fn TourNebula_3F_Sc04(ctx: Ctx) {
    if story(&ctx).await != 104 {
        TourNebula_3F_Sc05(&ctx).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Tour_3F_Sc04_T1).await;
    if ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_PRISE).await
        && !ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE).await
        && ask(&ctx, Tour_3F_Sc04_T2).await
    {
        say(&ctx, Tour_3F_Sc04_T3).await;
        ctx.set_flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE, true).await;
        remove_item(&ctx, ITEM_SIL_BOUSSOLE_ARTHUR, 1).await;
    }
    say(&ctx, Tour_3F_Sc04_T4).await;
    let rendue = ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE).await;
    // The Mega line (Tour_3F_Sc04_Mega, `silice::rules::trainer_lines`) and ARTHUR_1's two Full
    // Restores (the trainer's AI bag, `trainer_items`) are the battle's own.
    let who = if rendue {
        TRAINER_SIL_ARTHUR_2
    } else {
        TRAINER_SIL_ARTHUR_1
    };
    if lost(battle_trainer(&ctx, who, Tour_3F_Sc04_Defeat).await) {
        return;
    }
    ctx.set_flag(FLAG_SIL_ARTHUR_BATTU, true).await;
    say(&ctx, Tour_3F_Sc04_T5).await;
    say(&ctx, Tour_3F_Sc04_T6).await;
    say(&ctx, Tour_3F_Sc04_T7).await;
    add_item(&ctx, ITEM_SIL_ACCES_GAMMA, 1).await;
    ctx.set_flag(FLAG_SIL_KEY_ACCES_GAMMA, true).await;
    ctx.set_var(VAR_SIL_SYNC, 95).await;
    show_sync(&ctx, 95).await;
    say(&ctx, Tour_3F_Sc04_T8).await;
    say(&ctx, Tour_3F_Sc04_T9).await;
    if rendue {
        say(&ctx, Tour_3F_Sc04_T10).await;
        add_item(&ctx, ITEM_SIL_BATEAU_ORIGAMI, 1).await;
    }
    ctx.set_metatile(ECRAN.0, ECRAN.1, ECRAN.2, false).await;
    ctx.redraw_map().await;
    set_story(&ctx, 105).await;
    ctx.release_all().await;
}

/// `Tour_3F_Sc05`, Arthur after the battle: a compass kept until now is given back late (the
/// origami boat too).
async fn TourNebula_3F_Sc05(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_PRISE).await
        && !ctx.flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE).await
    {
        say(ctx, Tour_3F_Sc05_T1).await;
        remove_item(ctx, ITEM_SIL_BOUSSOLE_ARTHUR, 1).await;
        ctx.set_flag(FLAG_SIL_ARTHUR_BOUSSOLE_RENDUE, true).await;
        add_item(ctx, ITEM_SIL_BATEAU_ORIGAMI, 1).await;
        say(ctx, Tour_3F_Sc04_T10).await;
    } else {
        say(ctx, Tour_3F_Sc05_T2).await;
    }
    ctx.release().await;
}

/// Yann (3, 13), sight 4 east.
pub async fn TourNebula_3F_EventScript_Yann(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_3F_YANN,
        TRAINER_SIL_TOUR_YANN,
        Tour_3F_Yann_Intro,
        Tour_3F_Yann_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_3F_Yann_After).await;
    }
}

/// Maëlle (16, 13), sight 4 west.
pub async fn TourNebula_3F_EventScript_Maelle(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_3F_MAELLE,
        TRAINER_SIL_TOUR_MAELLE,
        Tour_3F_Maelle_Intro,
        Tour_3F_Maelle_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_3F_Maelle_After).await;
    }
}

pub async fn TourNebula_3F_EventScript_Technicien(ctx: Ctx) {
    msgbox(&ctx, Tour_3F_Npc_Technicien, MsgBox::Npc).await;
}

pub async fn TourNebula_3F_EventScript_Mousse(ctx: Ctx) {
    msgbox(&ctx, Tour_3F_Npc_Mousse, MsgBox::Npc).await;
}

pub async fn TourNebula_3F_EventScript_Entree(ctx: Ctx) {
    msgbox(&ctx, Tour_3F_Sign_Entree, MsgBox::Sign).await;
}

pub async fn TourNebula_3F_EventScript_Ecrans(ctx: Ctx) {
    msgbox(&ctx, Tour_3F_Sign_Ecrans, MsgBox::Sign).await;
}

/// The elevator door the player came in by (11, 16), closed: no text in the source.
pub async fn TourNebula_3F_EventScript_PorteVerrouillee(_ctx: Ctx) {
    // fallback: no text in the source for the locked elevator door.
}
