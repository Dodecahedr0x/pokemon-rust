//! Chapter 9, under the Tower: `contenu-jeu/chapitres/09-nebula.md` §2.8 (the Salle des Pompes,
//! `Nebula_TourHall_B1F`), §2.9 (the hall), §3 Nebula_Sc04 (the dive's arrival), Tour_Hall_Sc01
//! (the security corridor) and Tour_Hall_Sc02 (the crystal elevator), §5 (the six Tower
//! trainers). `VAR_SIL_STORY` 95 → 96 (under the Tower) → 97 (Kroll beaten) → 99 (the elevator).
//!
//! **The elevator (§5.7, pending ruling, flagged).** The source's `warp(MAP_TOUR_NEBULA_1F, 10,
//! 14)` predates Chapter 10's cabin map: as proposed, Tour_Hall_Sc02 warps into the cabin,
//! `TourNebulaAscenseur` (3, 6) (its warp 5, a landing), where Chapter 10's scenes take over.
//! **§5.6:** Chapter 10 also names `Tour_Sbire1_Intro`/`Tour_Sbire2_Intro`; Chapter 9's lines are
//! provisionally `Tour_Sbire1_Intro_Ch09`/`Tour_Sbire2_Intro_Ch09` (`text/fr.ron`).
//!
//! **The laser sas.** Each sas opens when its guards are beaten (`set_metatile` to the corridor
//! floor; `Nebula_TourHall_OnLoad` redraws them from the trainers' flags): sas 1 (19–21, 11)
//! Sbire 3's, sas 2 (14, 9–10) Viktor's and Nadia's (both), sas 3 (11–13, 5) Kroll's.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Nebula_TourHall_B1F", "Nebula_TourHall"];

// ---- B1F, the Salle des Pompes ----

/// Nebula_Sc04, the dive under the Tower: B1F's on-frame script at `VAR_SIL_STORY` 95, as the
/// player surfaces in the cuve (the Bassin's dive, `crate::silice::DIVE_WARPS`). The source runs
/// it before its warp; its text is about the dive, so it reads the same on arrival. Sets 96.
pub async fn Nebula_TourHall_B1F_Sc04(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Nebula_Sc04_T1).await;
    set_story(&ctx, 96).await;
    ctx.release_all().await;
}

/// Nothing: B1F's cuve dives back to the Bassin by `crate::silice::DIVE_WARPS` (the Gen3
/// `setdivewarp MAP_NEBULA 51 11` this script would run).
pub async fn Nebula_TourHall_B1F_OnDiveWarp(_ctx: Ctx) {}

/// By the rail gap (13, 10), sight 2 right. The gap is three cells wide (13–15, 10): his
/// approach never closes it, and the way round by the east (rows 2–8) stays open.
pub async fn Nebula_TourHall_B1F_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_B1F_SBIRE_1,
        TRAINER_SIL_TOUR_SBIRE_1,
        Tour_Sbire1_Intro_Ch09,
        Tour_Sbire1_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Sbire1_Defaite).await;
    }
}

/// Before the Max Elixir (17, 13), sight 3 left over the open floor.
pub async fn Nebula_TourHall_B1F_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_B1F_SBIRE_2,
        TRAINER_SIL_TOUR_SBIRE_2,
        Tour_Sbire2_Intro_Ch09,
        Tour_Sbire2_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Sbire2_Defaite).await;
    }
}

pub async fn Nebula_TourHall_B1F_EventScript_ItemMaxElixir(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_ELIXIR, 1).await;
}

// ---- the hall ----

/// The corridor's floor (SilNebulaTourHall), a sas once open.
const METATILE_SOL: u16 = 537;

const SAS_1: &[(i16, i16)] = &[(19, 11), (20, 11), (21, 11)];
const SAS_2: &[(i16, i16)] = &[(14, 9), (14, 10)];
const SAS_3: &[(i16, i16)] = &[(11, 5), (12, 5), (13, 5)];

async fn open(ctx: &Ctx, sas: &[(i16, i16)]) {
    for &(x, y) in sas {
        ctx.set_metatile(x, y, METATILE_SOL, false).await;
    }
}

/// Sas 2 opens once both of its guards are beaten.
async fn sas_2_open(ctx: &Ctx) -> bool {
    defeated(ctx, TRAINER_SIL_GARDE_COCKET_VIKTOR).await
        && defeated(ctx, TRAINER_SIL_GARDE_COCKET_NADIA).await
}

/// The sas from the trainers' flags; Kroll beaten while the story is still 96 (his after-battle
/// scene cut short) sets 97, so that the elevator always fires.
pub async fn Nebula_TourHall_OnLoad(ctx: Ctx) {
    if defeated(&ctx, TRAINER_SIL_TOUR_SBIRE_3).await {
        open(&ctx, SAS_1).await;
    }
    if sas_2_open(&ctx).await {
        open(&ctx, SAS_2).await;
    }
    if defeated(&ctx, TRAINER_SIL_CHEF_SECURITE_KROLL).await {
        open(&ctx, SAS_3).await;
        if story(&ctx).await == 96 {
            set_story(&ctx, 97).await;
        }
    }
}

/// Sas 1's sbire (18, 12), sight 3 right over the three cells before it. The lower hall is three
/// rows deep: his approach never closes it.
pub async fn Nebula_TourHall_EventScript_Sbire3(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_HALL_SBIRE_3,
        TRAINER_SIL_TOUR_SBIRE_3,
        Tour_Sbire3_Intro,
        Tour_Sbire3_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Sbire3_Defaite).await;
        return;
    }
    // `trainer` also returns false without a battle (a lost one whites out; a second trainer of
    // a pair): the sas opens on the flag.
    if defeated(&ctx, TRAINER_SIL_TOUR_SBIRE_3).await {
        open(&ctx, SAS_1).await;
        ctx.redraw_map().await;
    }
}

/// Sas 2's first guard (16, 8), sight 2 down over the room's column 16. After the battle, his
/// line about credit.
pub async fn Nebula_TourHall_EventScript_Viktor(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_HALL_VIKTOR,
        TRAINER_SIL_GARDE_COCKET_VIKTOR,
        Tour_Viktor_Intro,
        Tour_Viktor_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Viktor_Apres).await;
        return;
    }
    if sas_2_open(&ctx).await {
        open(&ctx, SAS_2).await;
        ctx.redraw_map().await;
    }
}

/// Sas 2's second guard (15, 11), sight 2 up over column 15 (no cell is in both guards' sight).
pub async fn Nebula_TourHall_EventScript_Nadia(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_HALL_NADIA,
        TRAINER_SIL_GARDE_COCKET_NADIA,
        Tour_Nadia_Intro,
        Tour_Nadia_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Nadia_Defaite).await;
        return;
    }
    if sas_2_open(&ctx).await {
        open(&ctx, SAS_2).await;
        ctx.redraw_map().await;
    }
}

/// Sas 3's chief (14, 6), sight 3 left over the corridor's row 6. Tour_Hall_Sc01's end: his
/// after-battle line, sas 3 open, `VAR_SIL_STORY` 97. Talked to again: the same line.
pub async fn Nebula_TourHall_EventScript_Kroll(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_HALL_KROLL,
        TRAINER_SIL_CHEF_SECURITE_KROLL,
        Tour_Kroll_Intro,
        Tour_Kroll_Defaite,
    )
    .await
    {
        post_battle(&ctx, Tour_Hall_Kroll_Apres).await;
        return;
    }
    if !defeated(&ctx, TRAINER_SIL_CHEF_SECURITE_KROLL).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Tour_Hall_Kroll_Apres).await;
    open(&ctx, SAS_3).await;
    ctx.redraw_map().await;
    if story(&ctx).await < 97 {
        set_story(&ctx, 97).await;
    }
    ctx.release_all().await;
}

/// Tour_Hall_Sc02, the crystal elevator: the coord row (11–13, 3) before its door, at
/// `VAR_SIL_STORY` 97. The cabin's two lines, the heal (the source's comfort choice: an author
/// question, see the plan), 99, and the ride.
///
/// **§5.7 (pending ruling, flagged):** the source warps to `MAP_TOUR_NEBULA_1F` (10, 14); as
/// proposed, the warp lands in Chapter 10's cabin, `TourNebulaAscenseur` (3, 6).
pub async fn Nebula_TourHall_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    play_music(&ctx, "MUS_NONE").await;
    say(&ctx, Tour_Hall_Sc02_T1).await;
    say(&ctx, Tour_Hall_Sc02_T2).await;
    ctx.heal_party().await;
    set_story(&ctx, 99).await;
    ctx.fade(Fade::ToBlack).await;
    warp(&ctx, MAP_TOUR_NEBULA_ASCENSEUR, 3, 6).await;
}

// The elevator's door (12, 1), `Nebula_TourHall_EventScript_Ascenseur`, is Chapter 10's
// (`ch10_ascenseur.rs`: the cabin from story 99).

/// The bay windows: the diversion outside.
pub async fn Nebula_TourHall_EventScript_Vitre(ctx: Ctx) {
    msgbox(&ctx, Tour_Hall_Vitre, MsgBox::Sign).await;
}

/// The wall of screens.
pub async fn Nebula_TourHall_EventScript_Ecran(ctx: Ctx) {
    msgbox(&ctx, Nebula_Hall_Ecran, MsgBox::Sign).await;
}

/// The empty reception desk.
pub async fn Nebula_TourHall_EventScript_Accueil(ctx: Ctx) {
    msgbox(&ctx, Nebula_Hall_Accueil, MsgBox::Sign).await;
}

