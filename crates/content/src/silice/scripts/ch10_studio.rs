//! Chapter 10, 2F: le Studio Prismatique, Kimiko's pillar (`TourNebula_2F`;
//! `10-tour-piliers.md` §3 `Tour_2F_Sc01`–`Sc05`, §4.2, the elites Basile and Lou).
//!
//! The way: the Régie's three spots cut in the logo's order, ROSE (console 2), VIOLET (console 3),
//! OR (console 1); a mistake lights them all again. The set (6–17, 4) is three segments of four
//! on the map: gold (6–9), pink (10–13), violet (14–17); each spot cuts its colour's segment (the
//! source's left/centre/right names do not match the map's colours, which win).

use super::ch08_evalon::lost;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TourNebula_2F"];

/// The set's row and the studio floor a cut segment becomes.
const DECOR_Y: i16 = 4;
const SOL_STUDIO: u16 = 608;
/// The three segments: (first x, the lit metatile).
const OR: (i16, u16) = (6, 668);
const ROSE: (i16, u16) = (10, 692);
const VIOLET: (i16, u16) = (14, 644);
/// "SORTIE ARTISTES" (20, 1), open from story 103.
const SORTIE: (i16, i16, u16) = (20, 1, 612);

/// A segment switched off (`cut`: the studio floor) or lit again. A switched-off segment stays
/// solid until the three are off ([`open_set`]): one dark segment must not let the player
/// through to Kimiko before the puzzle is solved (the plan's "SIL5 Ch10–Ch11 maps: review" counts
/// Kimiko unreachable while the set is on).
async fn segment(ctx: &Ctx, (x0, lit): (i16, u16), cut: bool) {
    for x in x0..x0 + 4 {
        ctx.set_metatile(x, DECOR_Y, if cut { SOL_STUDIO } else { lit }, true)
            .await;
    }
}

/// The three segments off and walkable: the set is cut.
async fn open_set(ctx: &Ctx) {
    for (x0, _) in [OR, ROSE, VIOLET] {
        for x in x0..x0 + 4 {
            ctx.set_metatile(x, DECOR_Y, SOL_STUDIO, false).await;
        }
    }
}

/// The set cut (`FLAG_SIL_STUDIO_DECOR_COUPE`, or from story 103), the exit from 103.
pub async fn TourNebula_2F_OnLoad(ctx: Ctx) {
    let s = story(&ctx).await;
    if ctx.flag(FLAG_SIL_STUDIO_DECOR_COUPE).await || s >= 103 {
        open_set(&ctx).await;
    }
    if s >= 103 {
        ctx.set_metatile(SORTIE.0, SORTIE.1, SORTIE.2, false).await;
    }
}

/// Nothing for Chapter 10 (the drone keeps its hide flag). Chapter 12 adds Gwada here at 127–129
/// (the plan's "SIL6 Ch12 maps as built").
pub async fn TourNebula_2F_OnTransition(ctx: Ctx) {
    super::ch12_breche::tour_2f_gwada_here(&ctx).await;
}

const DRONE_IN: &[Step] = &[walk(Dir::Down), Step::FacePlayer];
const DRONE_OUT: &[Step] = &[walk(Dir::Up), face(Dir::Down)];

/// `Tour_2F_Sc01`, live: the drone films the player (once, `FLAG_SIL_TOUR_2F_INTRO`). Coords
/// round the landing on `VAR_TEMP_1` 0.
///
/// Deviation: the drone (12, 17) comes down one cell, not three: the room ends on row 19.
pub async fn TourNebula_2F_Sc01(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_TOUR_2F_INTRO).await {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_TOUR_2F_DRONE, DRONE_IN).await;
    say(&ctx, Tour_2F_Sc01_T1).await;
    say(&ctx, Tour_2F_Sc01_T2).await;
    move_wait(&ctx, LOCALID_TOUR_2F_DRONE, DRONE_OUT).await;
    ctx.set_flag(FLAG_SIL_TOUR_2F_INTRO, true).await;
    ctx.release_all().await;
}

/// `Tour_2F_Sc02`, a chat screen: its own comment (the source's fallback without `random`), and
/// "off air" once Kimiko is beaten.
async fn chat(ctx: &Ctx, text: TextId) {
    let text = if story(ctx).await >= 103 {
        Tour_2F_Chat_Off
    } else {
        text
    };
    msgbox(ctx, text, MsgBox::Sign).await;
}

pub async fn TourNebula_2F_Sc02_01(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_01).await;
}
pub async fn TourNebula_2F_Sc02_02(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_02).await;
}
pub async fn TourNebula_2F_Sc02_03(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_03).await;
}
pub async fn TourNebula_2F_Sc02_04(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_04).await;
}
pub async fn TourNebula_2F_Sc02_05(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_05).await;
}
pub async fn TourNebula_2F_Sc02_06(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_06).await;
}
pub async fn TourNebula_2F_Sc02_07(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_07).await;
}
pub async fn TourNebula_2F_Sc02_08(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_08).await;
}
pub async fn TourNebula_2F_Sc02_09(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_09).await;
}
pub async fn TourNebula_2F_Sc02_10(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_10).await;
}
pub async fn TourNebula_2F_Sc02_11(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_11).await;
}
pub async fn TourNebula_2F_Sc02_12(ctx: Ctx) {
    chat(&ctx, Tour_2F_Chat_12).await;
}

/// `Tour_2F_Sc03_Erreur`: the wrong order, every spot lights again (`VAR_SIL_TOUR_STUDIO_STATE`
/// back to 0).
async fn erreur(ctx: &Ctx) {
    say(ctx, Tour_2F_Sc03_T5).await;
    segment(ctx, ROSE, false).await;
    segment(ctx, VIOLET, false).await;
    ctx.redraw_map().await;
    ctx.set_var(VAR_SIL_TOUR_STUDIO_STATE, 0).await;
}

/// The spots are all off once the set is cut.
async fn console_done(ctx: &Ctx) -> bool {
    if ctx.flag(FLAG_SIL_STUDIO_DECOR_COUPE).await {
        msgbox(ctx, Tour_2F_Sc03_T4, MsgBox::Sign).await;
        return true;
    }
    false
}

/// Console 1 (4, 5), the gold spot: last (state 2) cuts the set.
pub async fn TourNebula_2F_Sc03_1(ctx: Ctx) {
    if console_done(&ctx).await {
        return;
    }
    ctx.lock().await;
    if ctx.var(VAR_SIL_TOUR_STUDIO_STATE).await == 2 {
        say(&ctx, Tour_2F_Sc03_T3).await;
        open_set(&ctx).await;
        ctx.redraw_map().await;
        ctx.set_flag(FLAG_SIL_STUDIO_DECOR_COUPE, true).await;
    } else {
        erreur(&ctx).await;
    }
    ctx.release().await;
}

/// Console 2 (14, 5), the pink spot: first.
pub async fn TourNebula_2F_Sc03_2(ctx: Ctx) {
    if console_done(&ctx).await {
        return;
    }
    ctx.lock().await;
    if ctx.var(VAR_SIL_TOUR_STUDIO_STATE).await == 0 {
        say(&ctx, Tour_2F_Sc03_T1).await;
        segment(&ctx, ROSE, true).await;
        ctx.redraw_map().await;
        ctx.set_var(VAR_SIL_TOUR_STUDIO_STATE, 1).await;
    } else {
        say(&ctx, Tour_2F_Sc03_T4).await;
    }
    ctx.release().await;
}

/// Console 3 (20, 5), the violet spot: second.
pub async fn TourNebula_2F_Sc03_3(ctx: Ctx) {
    if console_done(&ctx).await {
        return;
    }
    ctx.lock().await;
    match ctx.var(VAR_SIL_TOUR_STUDIO_STATE).await {
        1 => {
            say(&ctx, Tour_2F_Sc03_T2).await;
            segment(&ctx, VIOLET, true).await;
            ctx.redraw_map().await;
            ctx.set_var(VAR_SIL_TOUR_STUDIO_STATE, 2).await;
        }
        0 => erreur(&ctx).await,
        _ => say(&ctx, Tour_2F_Sc03_T4).await,
    }
    ctx.release().await;
}

const KIMIKO_TROUBLEE: &[Step] = &[
    Step::WalkInPlace(Dir::Down, Speed::Normal),
    Step::Delay(16),
    face(Dir::Down),
];
const DRONE_AFFOLE: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Right),
    walk(Dir::Left),
    walk(Dir::Right),
];

/// `Tour_2F_Sc04`, Kimiko the Idol (story 102): Adrien's message, the battle, the ACCÈS BÊTA,
/// 90 %, the artists' exit (→ 103). `Tour_2F_Sc05` afterwards.
pub async fn TourNebula_2F_Sc04(ctx: Ctx) {
    if story(&ctx).await != 102 {
        msgbox(&ctx, Tour_2F_Sc05_T1, MsgBox::Npc).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Tour_2F_Sc04_T1).await;
    mallette_call(&ctx, "ADRIEN").await;
    say(&ctx, Tour_2F_Sc04_T2).await;
    say(&ctx, Tour_2F_Sc04_T3).await;
    end_call(&ctx).await;
    move_wait(&ctx, LOCALID_TOUR_2F_KIMIKO, KIMIKO_TROUBLEE).await;
    say(&ctx, Tour_2F_Sc04_T4).await;
    move_wait(&ctx, LOCALID_TOUR_2F_DRONE, DRONE_AFFOLE).await;
    say(&ctx, Tour_2F_Sc04_T5).await;
    // fallback: the "émotion" start effect (§4.2, §9: her lead's Sp. Atk −1) has no battle hook;
    // the source's fallback (iv 200 instead of 240) is battle data the import keeps at 240.
    // Reported; the battle is the full-strength one.
    fallback(&ctx, "kimiko_emotion").await;
    // The Mega line (Tour_2F_Sc04_Mega) is the battle's own (`silice::rules::trainer_lines`);
    // fallback: the send-out lines of §4.2 have no extracted text.
    fallback(&ctx, "battle_lines").await;
    if lost(battle_trainer(&ctx, TRAINER_SIL_KIMIKO_1, Tour_2F_Sc04_Defeat).await) {
        return;
    }
    say(&ctx, Tour_2F_Sc04_T6).await;
    hide(&ctx, LOCALID_TOUR_2F_DRONE).await;
    say(&ctx, Tour_2F_Sc04_T7).await;
    say(&ctx, Tour_2F_Sc04_T8).await;
    add_item(&ctx, ITEM_SIL_ACCES_BETA, 1).await;
    ctx.set_flag(FLAG_SIL_KEY_ACCES_BETA, true).await;
    ctx.set_var(VAR_SIL_SYNC, 90).await;
    show_sync(&ctx, 90).await;
    say(&ctx, Tour_2F_Sc04_T9).await;
    say(&ctx, Tour_2F_Sc04_T10).await;
    ctx.set_metatile(SORTIE.0, SORTIE.1, SORTIE.2, false).await;
    ctx.redraw_map().await;
    set_story(&ctx, 103).await;
    ctx.release_all().await;
}

/// Basile (5, 8), sight 4 south.
pub async fn TourNebula_2F_EventScript_Basile(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_2F_BASILE,
        TRAINER_SIL_TOUR_BASILE,
        Tour_2F_Basile_Intro,
        Tour_2F_Basile_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_2F_Basile_After).await;
    }
}

/// Lou (16, 8), sight 4 south: afterwards, the spots' order.
pub async fn TourNebula_2F_EventScript_Lou(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_2F_LOU,
        TRAINER_SIL_TOUR_LOU,
        Tour_2F_Lou_Intro,
        Tour_2F_Lou_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_2F_Lou_After).await;
    }
}

pub async fn TourNebula_2F_EventScript_Regisseuse(ctx: Ctx) {
    msgbox(&ctx, Tour_2F_Npc_Regisseuse, MsgBox::Npc).await;
}

pub async fn TourNebula_2F_EventScript_Drone(ctx: Ctx) {
    msgbox(&ctx, Tour_2F_Npc_Drone, MsgBox::Npc).await;
}

pub async fn TourNebula_2F_EventScript_Stagiaire(ctx: Ctx) {
    msgbox(&ctx, Tour_2F_Npc_Stagiaire, MsgBox::Npc).await;
}

/// The Max Revive ball (22, 10).
pub async fn TourNebula_2F_EventScript_ItemMaxRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REVIVE, 1).await;
}

/// The hidden PP Up (3, 11): a sign while the engine ignores hidden items.
pub async fn TourNebula_2F_EventScript_HiddenPpUp(ctx: Ctx) {
    hidden_item(&ctx, FLAG_SIL_ITEM_TOUR_2F_PP_UP, ITEM_PP_UP).await;
}

pub async fn TourNebula_2F_EventScript_Entree(ctx: Ctx) {
    msgbox(&ctx, Tour_2F_Sign_Entree, MsgBox::Sign).await;
}

pub async fn TourNebula_2F_EventScript_Logo(ctx: Ctx) {
    msgbox(&ctx, Tour_2F_Sign_Logo, MsgBox::Sign).await;
}

/// The elevator door the player came in by (10, 20), closed: no text in the source.
pub async fn TourNebula_2F_EventScript_PorteVerrouillee(_ctx: Ctx) {
    // fallback: no text in the source for the locked elevator door.
}
