//! Chapter 10, the Tower's elevator (`TourNebulaAscenseur`): the cabin's dispatch
//! (`contenu-jeu/chapitres/10-tour-piliers.md` §3 `Tour_Ascenseur_Sc00`–`Sc04`), and the hall's
//! elevator door, which Chapter 9's hall names and Chapter 10 writes (the plan's "SIL5 Ch10–Ch11
//! maps as built", pending links).
//!
//! The cabin is never walked: every arrival (its warp landings, and the hall door's warp to its
//! warp 5 cell (3, 6)) runs `TourNebulaAscenseur_Sc00` (on-frame on `VAR_TEMP_1` 0), which plays
//! the story's ride and always warps away by coordinates.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TourNebulaAscenseur"];

/// The cabin's arrival cell from the hall (warp 5).
pub const CABIN_FROM_HALL: (i16, i16) = (3, 6);
/// Each floor's landing (the floors' warp 0): 1F, 2F, 3F, 4F, and the Apex's vault-elevator door.
pub const LANDING_1F: (i16, i16) = (11, 21);
pub const LANDING_2F: (i16, i16) = (10, 19);
pub const LANDING_3F: (i16, i16) = (11, 15);
pub const LANDING_4F: (i16, i16) = (10, 23);
pub const LANDING_APEX: (i16, i16) = (11, 27);

/// `Tour_Ascenseur_Sc00`, the cabin's dispatch (every arrival): the ride the story asks for, then
/// the floor's landing.
pub async fn TourNebulaAscenseur_Sc00(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    let s = story(&ctx).await;
    match s {
        // The first climb (99), or back after a loss on 1F.
        ..=100 => TourNebulaAscenseur_Sc00a(ctx).await,
        101 => TourNebulaAscenseur_Sc01(ctx).await,
        102 => warp(&ctx, MAP_TOUR_NEBULA_2F, LANDING_2F.0, LANDING_2F.1).await,
        103 => TourNebulaAscenseur_Sc02(ctx).await,
        104 => warp(&ctx, MAP_TOUR_NEBULA_3F, LANDING_3F.0, LANDING_3F.1).await,
        105 => TourNebulaAscenseur_Sc03(ctx).await,
        // ≤ 106: before Revenge; 107–108: back from the hall after him (R-6).
        106..=108 => warp(&ctx, MAP_TOUR_NEBULA_4F, LANDING_4F.0, LANDING_4F.1).await,
        // 109+: boarded at 4F (`TourNebula_4F_Sc06`), or back after a loss in Chapter 11.
        _ => TourNebulaAscenseur_Sc04(ctx).await,
    }
}

/// `Tour_Ascenseur_Sc00a`, the first climb (99 → 100), then 1F.
async fn TourNebulaAscenseur_Sc00a(ctx: Ctx) {
    ctx.lock_all().await;
    if story(&ctx).await == 99 {
        ctx.delay(30).await;
        say(&ctx, Tour_Ascenseur_Sc00a_T1).await;
        say(&ctx, Tour_Ascenseur_Sc00a_T2).await;
        set_story(&ctx, 100).await;
    }
    warp(&ctx, MAP_TOUR_NEBULA_1F, LANDING_1F.0, LANDING_1F.1).await;
}

/// `Tour_Ascenseur_Sc01`, up to the Studio (101 → 102).
async fn TourNebulaAscenseur_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.delay(30).await;
    // fallback: the Mallette's call window (`mallette_call`): the message is a plain box.
    mallette_call(&ctx, "MATHIEU").await;
    say(&ctx, Tour_Ascenseur_Sc01_T1).await;
    end_call(&ctx).await;
    set_story(&ctx, 102).await;
    warp(&ctx, MAP_TOUR_NEBULA_2F, LANDING_2F.0, LANDING_2F.1).await;
}

const MATHIEU_DOWN: &[Step] = &[walk(Dir::Down)];
const MATHIEU_UP: &[Step] = &[walk(Dir::Up)];

/// `Tour_Ascenseur_Sc02`, Mathieu's meal between 2F and 3F (103 → 104): the Tower's only full
/// heal.
///
/// Deviation: Mathieu comes down one cell (3, 5), not two: the player may stand on (3, 6) (the
/// hall's landing, after a loss on 2F), and the cabin is 8 rows high.
async fn TourNebulaAscenseur_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.delay(20).await;
    say(&ctx, Tour_Ascenseur_Sc02_T1).await;
    show(&ctx, LOCALID_TOUR_ASCENSEUR_MATHIEU, FLAG_SIL_HIDE_TOUR_MATHIEU).await;
    move_wait(&ctx, LOCALID_TOUR_ASCENSEUR_MATHIEU, MATHIEU_DOWN).await;
    let (px, _) = ctx.player_pos().await;
    let to_player = match px {
        ..=2 => Dir::Left,
        3 => Dir::Down,
        _ => Dir::Right,
    };
    ctx.turn(LOCALID_TOUR_ASCENSEUR_MATHIEU, to_player).await;
    say(&ctx, Tour_Ascenseur_Sc02_T2).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Tour_Ascenseur_Sc02_T3).await;
    say(&ctx, Tour_Ascenseur_Sc02_T4).await;
    move_wait(&ctx, LOCALID_TOUR_ASCENSEUR_MATHIEU, MATHIEU_UP).await;
    hide(&ctx, LOCALID_TOUR_ASCENSEUR_MATHIEU).await;
    ctx.set_flag(FLAG_SIL_HIDE_TOUR_MATHIEU, true).await;
    ctx.set_flag(FLAG_SIL_TOUR_RAVITAILLEMENT, true).await;
    say(&ctx, Tour_Ascenseur_Sc02_T5).await;
    set_story(&ctx, 104).await;
    warp(&ctx, MAP_TOUR_NEBULA_3F, LANDING_3F.0, LANDING_3F.1).await;
}

/// `Tour_Ascenseur_Sc03`, towards the Reflets (105 → 106).
async fn TourNebulaAscenseur_Sc03(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.delay(30).await;
    say(&ctx, Tour_Ascenseur_Sc03_T1).await;
    mallette_call(&ctx, "GWADA").await;
    say(&ctx, Tour_Ascenseur_Sc03_T2).await;
    end_call(&ctx).await;
    set_story(&ctx, 106).await;
    warp(&ctx, MAP_TOUR_NEBULA_4F, LANDING_4F.0, LANDING_4F.1).await;
}

/// `Tour_Ascenseur_Sc04`, the last elevator: the two lines on the first ride (109), then the
/// Apex's arrival (Chapter 11 fixes it: the vault elevator's doors, (11, 27)).
async fn TourNebulaAscenseur_Sc04(ctx: Ctx) {
    ctx.lock_all().await;
    if story(&ctx).await == 109 {
        say(&ctx, Tour_Ascenseur_Sc04_T1).await;
        say(&ctx, Tour_Ascenseur_Sc04_T2).await;
    }
    warp(&ctx, MAP_SOMMET_APEX, LANDING_APEX.0, LANDING_APEX.1).await;
}

/// The floor panel (§2 "Panneau").
pub async fn TourNebulaAscenseur_EventScript_Panneau(ctx: Ctx) {
    msgbox(&ctx, Tour_Ascenseur_Panneau, MsgBox::Sign).await;
}

/// The hall's elevator door (Chapter 9's `Nebula_TourHall`, sign (12, 1)): from story 99 (Ch9's
/// hand-off) to the Zenith (117) it takes the player to the cabin, whose `Sc00` picks the floor.
/// Before 99 the door does not open; at 118 the ending holds the player, and from 119 Chapter
/// 12's elevator takes over.
///
/// Contradiction (the plan's "SIL5 Ch10–Ch11 maps: review", author question 1): Ch9's
/// `Tour_Hall_Sc02` warps straight to 1F; Ch10's `Tour_Ascenseur_Sc00a` plays the first climb.
/// The least invasive reading keeps both: this door leads to the cabin, which plays the climb at
/// 99 and sends a player already at 100 to 1F.
pub async fn Nebula_TourHall_EventScript_Ascenseur(ctx: Ctx) {
    let s = story(&ctx).await;
    if !(99..=117).contains(&s) {
        return;
    }
    ctx.lock_all().await;
    play_se(&ctx, "SE_ELEVATOR").await;
    warp(&ctx, MAP_TOUR_NEBULA_ASCENSEUR, CABIN_FROM_HALL.0, CABIN_FROM_HALL.1).await;
}
