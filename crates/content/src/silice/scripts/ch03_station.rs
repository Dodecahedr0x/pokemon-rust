//! Chapter 3, the Station Lumière (`ViberCity_StationLumiere`): `contenu-jeu/chapitres/
//! 03-viber-city.md` §2.7 (the grid, the fuse puzzle, the notice), §3 Station_Lumiere_Sc01 to
//! Sc03 and §5 (its trainers). `VAR_SIL_STORY` stays 33: the black-out (Sc03) warps the player
//! back to Viber-City, whose Sc05 sets 34.
//!
//! **The fuse puzzle.** `VAR_SIL_STATION_STATE` bits 0–2 are the lit tubes (blue, yellow,
//! violet; 7 = all lit). Each lever flips some of them (L1 blue + yellow, L2 yellow, L3 yellow +
//! violet, L4 blue + violet); all three dark is the black-out. Lever R, the map load and leaving
//! the fuse room ((7, 4)) light them all again. Bits 3–6 are the levers' positions (down when
//! set), which the source draws; the engine has no lever or tube metatiles here (below).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["ViberCity_StationLumiere"];

/// All three tubes lit, every lever up.
const TOUS_ALLUMES: u16 = 0b111;
/// The tubes' bits of `VAR_SIL_STATION_STATE`.
const TUBES: u16 = 0b111;

// ---- map scripts ----

/// At `VAR_SIL_STORY` 33 the tubes are lit again on every load (the source's `on_load`
/// "valeur de départ : 7").
pub async fn ViberCity_StationLumiere_OnTransition(ctx: Ctx) {
    if story(&ctx).await == 33 {
        ctx.set_var(VAR_SIL_STATION_STATE, TOUS_ALLUMES).await;
    }
}

/// Once beaten, the foreman stands aside on (8, 5) (`FLAG_SIL_STATION_CONTREMAITRE`; Gen3
/// `setobjectxyperm`, before the objects spawn).
pub async fn ViberCity_StationLumiere_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_STATION_CONTREMAITRE).await {
        ctx.set_object_template_pos(LOCALID_STATION_CONTREMAITRE, 8, 5)
            .await;
    }
}

// ---- the foreman (Station_Lumiere_Sc01) ----

const ASIDE: &[Step] = &[walk(Dir::Right), face(Dir::Left)];

/// Station_Lumiere_Sc01, the foreman (7, 5), in front of the fuse room's door: his battle, then
/// he steps aside (`FLAG_SIL_STATION_CONTREMAITRE`). Afterwards, his last line.
pub async fn ViberCity_StationLumiere_Sc01(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_STATION_CONTREMAITRE).await {
        msgbox(&ctx, Station_Lumiere_Sc01_T2, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Station_Lumiere_Sc01_T1).await;
    battle_trainer(
        &ctx,
        TRAINER_SIL_CONTREMAITRE_1,
        Station_Lumiere_Sc01_Defaite,
    )
    .await;
    say(&ctx, Station_Lumiere_Sc01_T2).await;
    move_wait(&ctx, LOCALID_STATION_CONTREMAITRE, ASIDE).await;
    ctx.set_flag(FLAG_SIL_STATION_CONTREMAITRE, true).await;
    ctx.release().await;
}

// ---- trainers (§5) ----

/// SBIRE 1 in the hall (2, 9), facing right.
pub async fn ViberCity_StationLumiere_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_STATION_SBIRE_1,
        TRAINER_SIL_GRUNT_STATION_1,
        Station_Lumiere_Sbire1_Intro,
        Station_Lumiere_Sbire1_Defaite,
    )
    .await
    {
        post_battle(&ctx, Station_Lumiere_Sbire1_Apres).await;
    }
}

/// SBIRE 2 by the transformers (3, 5), facing right.
pub async fn ViberCity_StationLumiere_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_STATION_SBIRE_2,
        TRAINER_SIL_GRUNT_STATION_2,
        Station_Lumiere_Sbire2_Intro,
        Station_Lumiere_Sbire2_Defaite,
    )
    .await
    {
        post_battle(&ctx, Station_Lumiere_Sbire2_Apres).await;
    }
}

/// TECHNICIENNE KARINE (13, 6), facing down.
pub async fn ViberCity_StationLumiere_EventScript_Technicienne(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_STATION_TECHNICIENNE,
        TRAINER_SIL_TECHNICIENNE_1,
        Station_Lumiere_Karine_Intro,
        Station_Lumiere_Karine_Defaite,
    )
    .await
    {
        post_battle(&ctx, Station_Lumiere_Karine_Apres).await;
    }
}

// ---- the fuse room (Station_Lumiere_Sc02, Sc03) ----

/// Lever `n` (1–4): Station_Lumiere_Sc02 at `VAR_SIL_STORY` 33. "Tu abaisses le levier", the
/// tubes in `mask` flip; all three dark is Sc03. After the black-out, the tubes' dark line.
async fn levier(ctx: &Ctx, n: u16, mask: u16) {
    if story(ctx).await != 33 {
        msgbox(ctx, Station_Lumiere_Tube_T2, MsgBox::Sign).await;
        return;
    }
    ctx.lock_all().await;
    say(ctx, Station_Lumiere_Sc02_T1).await;
    let state = ctx.var(VAR_SIL_STATION_STATE).await;
    let state = (state ^ mask) ^ (1 << (2 + n));
    ctx.set_var(VAR_SIL_STATION_STATE, state).await;
    // fallback: the lever and tube metatiles (the source's METATILE_LEVIER_* / METATILE_TUBE_*):
    // the BikeShop set has no lever-down or dark-tube cell. Nothing is redrawn; each tube says
    // whether it is lit (`ViberCity_StationLumiere_EventScript_Tube`).
    fallback(ctx, "lever metatiles").await;
    if state & TUBES == 0 {
        black_out(ctx).await;
        return;
    }
    ctx.release_all().await;
}

/// Lever 1: blue and yellow.
pub async fn ViberCity_StationLumiere_EventScript_Levier1(ctx: Ctx) {
    levier(&ctx, 1, 0b011).await;
}

/// Lever 2: yellow.
pub async fn ViberCity_StationLumiere_EventScript_Levier2(ctx: Ctx) {
    levier(&ctx, 2, 0b010).await;
}

/// Lever 3: yellow and violet.
pub async fn ViberCity_StationLumiere_EventScript_Levier3(ctx: Ctx) {
    levier(&ctx, 3, 0b110).await;
}

/// Lever 4: blue and violet.
pub async fn ViberCity_StationLumiere_EventScript_Levier4(ctx: Ctx) {
    levier(&ctx, 4, 0b101).await;
}

/// Lever R (the crate in the east wall, (12, 2), read from (11, 2)): everything lit again, every
/// lever up.
pub async fn ViberCity_StationLumiere_EventScript_LevierR(ctx: Ctx) {
    if story(&ctx).await != 33 {
        return;
    }
    ctx.lock_all().await;
    ctx.set_var(VAR_SIL_STATION_STATE, TOUS_ALLUMES).await;
    say(&ctx, Station_Lumiere_Sc02_T2).await;
    ctx.release_all().await;
}

/// The three tubes (9–11, 1), each read from below: lit (`Station_Lumiere_Tube_T1`) or dark
/// (`_T2`). The tube is the one above the player.
pub async fn ViberCity_StationLumiere_EventScript_Tube(ctx: Ctx) {
    let (x, _) = ctx.player_pos().await;
    let bit = (x - 9).clamp(0, 2) as u16;
    let lit = story(&ctx).await == 33 && ctx.var(VAR_SIL_STATION_STATE).await & (1 << bit) != 0;
    let text = if lit {
        Station_Lumiere_Tube_T1
    } else {
        Station_Lumiere_Tube_T2
    };
    msgbox(&ctx, text, MsgBox::Sign).await;
}

/// The fuse room's doorway (7, 4), on every step (`VAR_TEMP_3` stays 0): leaving the room lights
/// the tubes again (§2.7, "sortir de la salle remet aussi à zéro"). Silent.
pub async fn ViberCity_StationLumiere_EventScript_SortieFusibles(ctx: Ctx) {
    if story(&ctx).await == 33 {
        ctx.set_var(VAR_SIL_STATION_STATE, TOUS_ALLUMES).await;
    }
}

/// Station_Lumiere_Sc03, the black-out (all three tubes dark, chained from a lever, inside its
/// `lock_all`): the sbires flee in the dark (`FLAG_SIL_HIDE_STATION_SBIRES`, `_SBIRE_2`),
/// `FLAG_SIL_VIBER_BLACKOUT`, and the warp to the Station's front (8, 5) in Viber-City, whose
/// Sc05 follows. The warp ends the script.
async fn black_out(ctx: &Ctx) {
    ctx.delay(30).await;
    say(ctx, Station_Lumiere_Sc03_T1).await;
    ctx.fade(Fade::ToBlack).await;
    // fallback: stop_music and set_flash_level(2) on the city (§9): a long fade to black.
    fallback(ctx, "stop_music").await;
    fallback(ctx, "set_flash_level").await;
    ctx.delay(60).await;
    ctx.set_flag(FLAG_SIL_VIBER_BLACKOUT, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_STATION_SBIRES, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_STATION_SBIRE_2, true).await;
    warp(ctx, MAP_VIBER_CITY, 8, 5).await;
}

// ---- decor and items ----

/// The notice on the hall's desk (9, 10): the puzzle's hint.
pub async fn ViberCity_StationLumiere_EventScript_Note(ctx: Ctx) {
    msgbox(&ctx, Station_Lumiere_Note_T1, MsgBox::Sign).await;
}

/// A transformer (2, 5, 8, 11 on row 6).
pub async fn ViberCity_StationLumiere_EventScript_Transfo(ctx: Ctx) {
    msgbox(&ctx, Station_Lumiere_Transfo_T1, MsgBox::Sign).await;
}

/// The Elixir between two transformers (4, 7).
pub async fn ViberCity_StationLumiere_EventScript_ItemElixir(ctx: Ctx) {
    find_item(&ctx, ITEM_ELIXIR, 1).await;
}
