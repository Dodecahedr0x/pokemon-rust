//! Chapter 11's endings, on other chapters' maps (`11-apex-zenith.md` §3 `Nebula_Sc13`,
//! `Brico_Bourg_Sc14`, "Fin alternative : Le Sommeil Doré" `Elysium_E1`–`E5`, `Elysium_Fin`).
//!
//! This file claims no map: Nébula is Chapter 9's, the player's house Chapter 0's
//! (`ch00_houses.rs`). Their edits name the scripts below (`edits/Nebula/07_ch11_scenes.ron`,
//! `edits/BricoBourg_PlayerHouse_*/03_ch11_epilogue.ron`).
//!
//! **The scenes run on-frame on `VAR_SIL_FIN_SCENE`** (a map's on-frame entry, as Chapter 0's
//! arrivals): each scene sets the next value and warps; the last sets 0. Canonical: 13 Nébula's
//! waking (`Nebula_Sc13`: the four people, the final text, the credits, `FLAG_SIL_GAME_CLEAR`),
//! 14 the house's 2F three days later (`Brico_Bourg_Sc14`'s first half, then the save); the
//! second half is the 1F's coord at the stairs' foot (118 → 119). Elysium: 2 Nébula in gold, 3
//! the Apex's eternal party, 4 the house, 5 the Zenith, then `Elysium_Fin`: the alternate final
//! text, the credits, and `reload_last_save` (R-6: nothing is written, the shells reload the last
//! save). `FLAG_SIL_FIN_ELYSIUM_EN_COURS` is set for the whole sequence and cleared before the
//! reload.
//!
//! **Fallbacks** (§9): the Elysium scenes play their lines in order instead of giving the player
//! the hand (`elysium_free_roam`: no 30-second timer, no talk-to-all gate); the final texts are
//! white on black (`show_text_on_black`, not the alternate end's gold on white); the credits are Emerald's credits screen
//! alone, without the vignettes (`credits_montage`); the gold title screen is not shown before the
//! reload (`show_title`). The gold palette is real (kept on every map from `Elysium_E1`).

use super::ch11_apex::E3_CELL;
use crate::silice::prelude::*;

/// Nébula's Place, where the waking people stand (Chapter 9's `NEBULA_PLACE`).
pub const NEBULA_PLACE: (i16, i16) = (30, 31);
/// The player's house's 1F, one cell in from the door mat.
const HOUSE_DOOR_IN: (i16, i16) = (4, 6);
/// The 2F's bed side (`HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE`).
const HOUSE_BED: (i16, i16) = (4, 2);
/// Where Elysium_E5 sets the player on the Zenith (the source's (7, 8)).
const ZENITH_E5: (i16, i16) = (7, 8);

/// Where the alternate ending was chosen: the Apex (`Sommet_Apex_Sc05`) or the Zenith
/// (`Trone_Zenith_Sc08`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ElysiumFrom {
    Apex,
    Zenith,
}

const KNEEL: &[Step] = &[Step::WalkInPlace(Dir::Down, Speed::Slow)];
const JULIEN_COMES: &[Step] = &[Step::FacePlayer];

/// `Elysium_E1`, the Poké Balls laid down: the sequence's flag, Julien's welcome, the white,
/// then Nébula in gold (`VAR_SIL_FIN_SCENE` 2).
pub(super) async fn elysium_e1(ctx: &Ctx, from: ElysiumFrom) {
    ctx.lock_all().await;
    ctx.set_flag(FLAG_SIL_FIN_ELYSIUM_EN_COURS, true).await;
    // fallback: MUS_SIL_ELYSIUM (play_music); the kneeling sprite (set_player_sprite KNEEL):
    // a step in place.
    fallback(ctx, "play_music").await;
    move_wait(ctx, PLAYER, KNEEL).await;
    say(ctx, Elysium_E1_T1).await;
    // The source's hand-written approach: Julien turns to the player where he stands.
    let julien = match from {
        ElysiumFrom::Apex => LOCALID_APEX_JULIEN,
        ElysiumFrom::Zenith => LOCALID_ZENITH_JULIEN,
    };
    move_wait(ctx, julien, JULIEN_COMES).await;
    say(ctx, Elysium_E1_T2).await;
    // GOLD: the prelude's nearest colour is YELLOW (no gold constant).
    screen_flash(ctx, YELLOW).await;
    // set_palette(ALL, ELYSIUM_GOLD): every map in gold until the end (kept across loads; the
    // reload of the last save ends it).
    set_palette(ctx, ELYSIUM_GOLD, 0, true).await;
    fade_white(ctx, true).await;
    say(ctx, Elysium_E1_T3).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 2).await;
    warp(ctx, MAP_NEBULA, NEBULA_PLACE.0, NEBULA_PLACE.1).await;
}

/// The four waking people (Chapter 9's Nébula, local ids 40–43) and their hide flags.
const EVEIL: [(u8, FlagId); 4] = [
    (LOCALID_NEBULA_EVEIL_1, FLAG_SIL_HIDE_NEBULA_EVEIL),
    (LOCALID_NEBULA_EVEIL_2, FLAG_SIL_HIDE_NEBULA_EVEIL_2),
    (LOCALID_NEBULA_EVEIL_3, FLAG_SIL_HIDE_NEBULA_EVEIL_3),
    (LOCALID_NEBULA_EVEIL_4, FLAG_SIL_HIDE_NEBULA_EVEIL_4),
];

async fn show_eveil(ctx: &Ctx) {
    for (obj, flag) in EVEIL {
        show(ctx, obj, flag).await;
    }
}

async fn hide_eveil(ctx: &Ctx) {
    for (obj, _) in EVEIL {
        hide(ctx, obj).await;
    }
}

/// `Elysium_E2`, Nébula in gold (`VAR_SIL_FIN_SCENE` 2): nobody walks, everybody smiles; then the
/// Apex (3).
pub async fn Nebula_Elysium_E2(ctx: Ctx) {
    ctx.lock_all().await;
    show_eveil(&ctx).await;
    say(&ctx, Elysium_E2_T0).await;
    fallback(&ctx, "elysium_free_roam").await;
    for t in [Elysium_E2_T1, Elysium_E2_T2, Elysium_E2_T3, Elysium_E2_T4] {
        say(&ctx, t).await;
    }
    ctx.fade(Fade::ToWhite).await;
    hide_eveil(&ctx).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 3).await;
    warp(&ctx, MAP_SOMMET_APEX, E3_CELL.0, E3_CELL.1).await;
}

/// `Elysium_E3`, the eternal party (the Apex's on-frame, `VAR_SIL_FIN_SCENE` 3): Gogo and Romain
/// on their crates; then the house (4).
pub(super) async fn elysium_e3(ctx: &Ctx) {
    ctx.lock_all().await;
    say(ctx, Elysium_E3_T1).await;
    say(ctx, Elysium_E3_T2).await;
    ctx.fade(Fade::ToWhite).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 4).await;
    warp(
        ctx,
        MAP_BRICO_BOURG_PLAYER_HOUSE_1F,
        HOUSE_DOOR_IN.0,
        HOUSE_DOOR_IN.1,
    )
    .await;
}

const TO_THE_CHAIR: &[Step] = &[walk(Dir::Up), face(Dir::Up)];

/// `Elysium_E4`, the house (the 1F's on-frame, `VAR_SIL_FIN_SCENE` 4): the three friends frozen
/// round the table, Maman, the chair; then the Zenith (5). The stairs' line
/// (`Elysium_E4_Escalier`) is never needed: the player has no hand here.
pub async fn BricoBourg_PlayerHouse_1F_Elysium_E4(ctx: Ctx) {
    ctx.lock_all().await;
    show(&ctx, LOCALID_HOUSE_MATHIEU_ELY, FLAG_SIL_HIDE_HOUSE_ELYSIUM).await;
    show(&ctx, LOCALID_HOUSE_ADRIEN_ELY, FLAG_SIL_HIDE_HOUSE_ELYSIUM_2).await;
    show(&ctx, LOCALID_HOUSE_GWADA_ELY, FLAG_SIL_HIDE_HOUSE_ELYSIUM_3).await;
    fallback(&ctx, "elysium_free_roam").await;
    say(&ctx, Elysium_E4_T1).await;
    say(&ctx, Elysium_E4_T2).await;
    say(&ctx, Elysium_E4_T3).await;
    ctx.turn(LOCALID_PLAYER_HOUSE_MAMAN, Dir::Right).await;
    say(&ctx, Elysium_E4_T4).await;
    move_wait(&ctx, PLAYER, TO_THE_CHAIR).await;
    say(&ctx, Elysium_E4_T5).await;
    ctx.fade(Fade::ToWhite).await;
    hide(&ctx, LOCALID_HOUSE_MATHIEU_ELY).await;
    hide(&ctx, LOCALID_HOUSE_ADRIEN_ELY).await;
    hide(&ctx, LOCALID_HOUSE_GWADA_ELY).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 5).await;
    warp(&ctx, MAP_TRONE_ZENITH, ZENITH_E5.0, ZENITH_E5.1).await;
}

/// `Elysium_E5`, the only one awake (the Zenith's on-frame, `VAR_SIL_FIN_SCENE` 5): Julien
/// watching, the Mallette's screen, then `Elysium_Fin`.
pub(super) async fn elysium_e5(ctx: &Ctx) {
    ctx.lock_all().await;
    // fallback: hide_player (the camera without the player): the player stands below.
    fallback(ctx, "hide_player").await;
    ctx.delay(60).await;
    say(ctx, Elysium_E5_T1).await;
    play_se(ctx, "SE_PC_ON").await;
    say(ctx, Elysium_E5_T2).await;
    ctx.delay(60).await;
    elysium_fin(ctx).await;
}

/// `Elysium_Fin`: the alternate final text, the short credits, the flag cleared, then the last
/// save reloaded as it is (`Event::ReloadLastSave`; nothing written, R-6).
async fn elysium_fin(ctx: &Ctx) {
    // GOLD_ON_WHITE: the prelude's text on black only (white letters on black).
    show_text_on_black(ctx, Elysium_Fin_T1).await;
    fallback(ctx, "credits_montage").await;
    ctx.credits().await;
    show_text_on_black(ctx, Elysium_Fin_FIN).await;
    ctx.set_flag(FLAG_SIL_FIN_ELYSIUM_EN_COURS, false).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 0).await;
    // fallback: the gold title screen shown once (show_title GOLD): straight to the reload.
    fallback(ctx, "show_title").await;
    ctx.reload_last_save().await;
}

/// `Nebula_Sc13`, the people's waking (`VAR_SIL_FIN_SCENE` 13, set by `Ascenseur_Sc12` with
/// story 118): the four people under the true sky, the final text, the credits, `FIN`, the game
/// cleared, the respawn at home, then the house's 2F three days later (14).
pub async fn Nebula_Sc13(ctx: Ctx) {
    ctx.lock_all().await;
    show_eveil(&ctx).await;
    camera_pan(&ctx, 0, -4).await;
    say(&ctx, Nebula_Sc13_T1).await;
    ctx.delay(30).await;
    say(&ctx, Nebula_Sc13_T2).await;
    ctx.delay(30).await;
    say(&ctx, Nebula_Sc13_T3).await;
    ctx.delay(30).await;
    say(&ctx, Nebula_Sc13_T4).await;
    camera_pan(&ctx, 0, 0).await;
    ctx.fade(Fade::ToBlack).await;
    hide_eveil(&ctx).await;
    ctx.fade(Fade::FromBlack).await;
    show_text_on_black(&ctx, Fin_Texte_T1).await;
    fallback(&ctx, "credits_montage").await;
    ctx.credits().await;
    show_text_on_black(&ctx, Fin_Texte_FIN).await;
    ctx.set_flag(FLAG_SIL_GAME_CLEAR, true).await;
    ctx.set_respawn(HEAL_LOCATION_SIL_BRICO_BOURG_PLAYER_HOUSE).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 14).await;
    warp(
        &ctx,
        MAP_BRICO_BOURG_PLAYER_HOUSE_2F,
        HOUSE_BED.0,
        HOUSE_BED.1,
    )
    .await;
}

/// A waking person talked to: the alternate ending's line during it, else the waking's (the
/// objects are only shown during the two scenes, which hold the player).
async fn eveil(ctx: &Ctx, waking: TextId, gold: TextId) {
    let t = if ctx.flag(FLAG_SIL_FIN_ELYSIUM_EN_COURS).await {
        gold
    } else {
        waking
    };
    msgbox(ctx, t, MsgBox::Npc).await;
}

pub async fn Nebula_EventScript_Eveil1(ctx: Ctx) {
    eveil(&ctx, Nebula_Sc13_T1, Elysium_E2_T1).await;
}
pub async fn Nebula_EventScript_Eveil2(ctx: Ctx) {
    eveil(&ctx, Nebula_Sc13_T2, Elysium_E2_T2).await;
}
pub async fn Nebula_EventScript_Eveil3(ctx: Ctx) {
    eveil(&ctx, Nebula_Sc13_T3, Elysium_E2_T3).await;
}
pub async fn Nebula_EventScript_Eveil4(ctx: Ctx) {
    eveil(&ctx, Nebula_Sc13_T4, Elysium_E2_T4).await;
}

/// `Brico_Bourg_Sc14`'s first half, three days later (the 2F's on-frame, `VAR_SIL_FIN_SCENE`
/// 14): the true blue sky; Gwada and Julien wait at 1F; then the game saves (§9: the Hall of
/// Fame's save, `save_game`, without the dialog).
///
/// Deviation: the source saves on Nébula right after the credits, before the warp; the save is
/// made here so that Continue resumes at home with the epilogue's second half ahead.
pub async fn BricoBourg_PlayerHouse_2F_Sc14(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Brico_Bourg_Sc14_T1).await;
    ctx.set_flag(FLAG_SIL_HIDE_HOUSE_EPILOGUE, false).await;
    ctx.set_flag(FLAG_SIL_HIDE_HOUSE_EPILOGUE_2, false).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 0).await;
    ctx.save_game().await;
    ctx.release_all().await;
}

const MAMAN_TO_KITCHEN: &[Step] = &[walk(Dir::Right), walk(Dir::Right), face(Dir::Up)];
const GWADA_STEPS_UP: &[Step] = &[walk(Dir::Up)];

/// `Brico_Bourg_Sc14`'s second half (the 1F's coords off the stairs' foot (8, 2), story 118, once
/// the 2F's half has shown Gwada and Julien): Maman, the cracked Mallette given back
/// (`FLAG_SIL_MALLETTE_FELEE`), the white line on the Dome, the meeting at Nébula (119).
pub async fn BricoBourg_PlayerHouse_1F_Sc14(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_HIDE_HOUSE_EPILOGUE).await {
        return;
    }
    ctx.lock_all().await;
    ctx.turn(LOCALID_PLAYER_HOUSE_MAMAN, Dir::Right).await;
    say(&ctx, Brico_Bourg_Sc14_T2).await;
    move_wait(&ctx, LOCALID_PLAYER_HOUSE_MAMAN, MAMAN_TO_KITCHEN).await;
    move_wait(&ctx, LOCALID_HOUSE_GWADA_EPI, GWADA_STEPS_UP).await;
    say(&ctx, Brico_Bourg_Sc14_T3).await;
    say(&ctx, Brico_Bourg_Sc14_T4).await;
    add_item(&ctx, ITEM_SIL_MALLETTE, 1).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_PRISE, false).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_FELEE, true).await;
    say(&ctx, Brico_Bourg_Sc14_T5).await;
    say(&ctx, Brico_Bourg_Sc14_T6).await;
    play_se(&ctx, "SE_ITEMFINDER").await;
    say(&ctx, Brico_Bourg_Sc14_T7).await;
    say(&ctx, Brico_Bourg_Sc14_T8).await;
    say(&ctx, Brico_Bourg_Sc14_T9).await;
    say(&ctx, Brico_Bourg_Sc14_T10).await;
    say(&ctx, Brico_Bourg_Sc14_T11).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_HOUSE_GWADA_EPI).await;
    hide(&ctx, LOCALID_HOUSE_JULIEN_EPI).await;
    ctx.fade(Fade::FromBlack).await;
    set_story(&ctx, 119).await;
    ctx.release_all().await;
}
