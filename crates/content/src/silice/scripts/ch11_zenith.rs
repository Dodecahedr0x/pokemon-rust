//! Chapter 11, the Zenith: the crystal elevator's cabin (`TroneZenith_Ascenseur`), the Zenith's
//! throne (`TroneZenith`) and its night variant (`TroneZenith_Nuit`); `11-apex-zenith.md` §3
//! `Sommet_Apex_Sc07`'s ascent, `Trone_Zenith_Sc08`–`Sc11`, `Ascenseur_Sc12`.
//!
//! The way (115 → 118): Cocorico on the throne (`Sc08`, the alternate ending's last chance, the
//! battle without whiteout, `Sc08_Lose`), the Rupture (`Sc09`, 116). The source's fallback for the
//! night palette is the map variant and a warp on black (§9 `set_palette`): `Sc09` ends on black
//! and the night map's on-frame (116) plays `Sc10`–`Sc11` (117), then the cabin's descent
//! (`Sc12`, 118) leaves for Nébula's Place, where `Nebula_Sc13` plays the ending.

use super::ch08_evalon::lost;
use super::ch11_apex::{TUBE_FOOT, chooses_elysium};
use super::ch11_endings::{ElysiumFrom, NEBULA_PLACE, elysium_e1, elysium_e5};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TroneZenith", "TroneZenith_Ascenseur", "TroneZenith_Nuit"];

/// The tube's stairs on the Zenith (both maps' warp 0, a landing).
const ZENITH_ARRIVAL: (i16, i16) = (7, 13);
/// Where the Rupture leaves the player on the night map (`Sc10` walks one cell south of it).
const NUIT_PLAYER: (i16, i16) = (6, 8);

// ---- the cabin ----

/// The four friends ride down with the player at 117 (`Ascenseur_Sc12`), hidden otherwise.
pub async fn TroneZenith_Ascenseur_OnTransition(ctx: Ctx) {
    let hide = story(&ctx).await != 117;
    for f in [
        FLAG_SIL_HIDE_ZENITH_ASC_MATHIEU,
        FLAG_SIL_HIDE_ZENITH_ASC_ADRIEN,
        FLAG_SIL_HIDE_ZENITH_ASC_GWADA,
        FLAG_SIL_HIDE_ZENITH_ASC_JULIEN,
    ] {
        ctx.set_flag(f, hide).await;
    }
}

/// The cabin's dispatch (every arrival; on-frame on `VAR_TEMP_1` 0): the ascent at 114 (and the
/// retry at 115) to the Zenith, the descent at 117 to Nébula; otherwise (back from the Zenith's
/// stairs) the Zenith again, the point of no return. The cabin is never walked.
pub async fn TroneZenith_Ascenseur_Sc00(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    match story(&ctx).await {
        114 => TroneZenith_Ascenseur_Sc07(&ctx).await,
        117 => TroneZenith_Ascenseur_Sc12(&ctx).await,
        // 115–116 (after the throne's stairs or a retry), and the post-game's visits.
        s if s >= 118 => {
            warp(&ctx, MAP_TRONE_ZENITH_NUIT, ZENITH_ARRIVAL.0, ZENITH_ARRIVAL.1).await;
        }
        115 | 116 => warp(&ctx, MAP_TRONE_ZENITH, ZENITH_ARRIVAL.0, ZENITH_ARRIVAL.1).await,
        _ => warp(&ctx, MAP_SOMMET_APEX, TUBE_FOOT.0, TUBE_FOOT.1).await,
    }
}

/// `Sommet_Apex_Sc07`'s second half, the ascent (114 → 115).
async fn TroneZenith_Ascenseur_Sc07(ctx: &Ctx) {
    ctx.lock_all().await;
    // fallback: the cabin's scrolling backdrop (set_cabin_background): the window is fixed.
    fallback(ctx, "cabin_background").await;
    ctx.delay(60).await;
    say(ctx, Ascenseur_Sc07_T2).await;
    screen_flash(ctx).await;
    say(ctx, Ascenseur_Sc07_T3).await;
    ctx.fade(Fade::ToBlack).await;
    set_story(ctx, 115).await;
    warp(ctx, MAP_TRONE_ZENITH, ZENITH_ARRIVAL.0, ZENITH_ARRIVAL.1).await;
}

/// `Ascenseur_Sc12`, the new day (117 → 118): the descent with the four, then Nébula.
///
/// Deviation (the plan's "SIL5 Ch10–Ch11 maps as built", pending links): the source lands on
/// the tower's exit (Ch9's (28, 11)); the scene lands on the Place (30, 31), where the four
/// waking people stand, so `Nebula_Sc13` is seen there (`VAR_SIL_FIN_SCENE` 13).
async fn TroneZenith_Ascenseur_Sc12(ctx: &Ctx) {
    ctx.lock_all().await;
    say(ctx, Ascenseur_Sc12_T1).await;
    ctx.fade(Fade::ToBlack).await;
    set_story(ctx, 118).await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 13).await;
    warp(ctx, MAP_NEBULA, NEBULA_PLACE.0, NEBULA_PLACE.1).await;
}

// ---- the Zenith ----

/// The three friends hidden until `Sc11` shows them (on the night map).
pub async fn TroneZenith_OnTransition(ctx: Ctx) {
    for f in [
        FLAG_SIL_HIDE_ZENITH_MATHIEU,
        FLAG_SIL_HIDE_ZENITH_ADRIEN,
        FLAG_SIL_HIDE_ZENITH_GWADA,
    ] {
        ctx.set_flag(f, true).await;
    }
}

/// `Trone_Zenith_Sc08`, the call of the new day (the coords (3–11, 8), 115): Cocorico's speech,
/// the alternate ending's last chance (two yes, cursors on No: `Elysium_E1`), then the battle
/// (`TRAINER_SIL_COCORICO_1`, no whiteout: a loss is `Sc08_Lose`), then `Sc09`.
pub async fn TroneZenith_Sc08(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Trone_Zenith_Sc08_T1).await;
    say(&ctx, Trone_Zenith_Sc08_T2).await;
    ctx.turn(LOCALID_ZENITH_JULIEN, Dir::Down).await;
    screen_flash(&ctx).await;
    say(&ctx, Trone_Zenith_Sc08_T3).await;
    if chooses_elysium(&ctx, Trone_Zenith_Sc08_C1).await {
        elysium_e1(&ctx, ElysiumFrom::Zenith).await;
        return;
    }
    say(&ctx, Trone_Zenith_Sc08_C3).await;
    screen_flash(&ctx).await;
    // fallback: the mid-battle lines (§4.2: the weather changed, Entei sent out, under 25 %) and
    // the Primal Reversion and Mega staging lines.
    fallback(&ctx, "battle_lines").await;
    let outcome = ctx
        .battle_trainer_no_whiteout(TRAINER_SIL_COCORICO_1, Trone_Zenith_Sc08_Defeat)
        .await;
    if lost(outcome) {
        TroneZenith_Sc08_Lose(&ctx).await;
        return;
    }
    ctx.set_flag(trainer_flag(TRAINER_SIL_COCORICO_1), true).await;
    TroneZenith_Sc09(&ctx).await;
}

/// `Trone_Zenith_Sc08_Lose`, the golden sleep (a loss to Cocorico, never the alternate ending):
/// no whiteout, Cocorico's soft voice, the party healed, the Apex's tube foot; Romain's line
/// there (`SommetApex_Sc08_Lose`, `VAR_SIL_FIN_SCENE` 8). The story stays 115: the tube takes
/// the player up again.
async fn TroneZenith_Sc08_Lose(ctx: &Ctx) {
    // fallback: the gold palette over everything (set_palette ALL): a fade to black.
    ctx.fade(Fade::ToBlack).await;
    say(ctx, Trone_Zenith_Sc08_Lose_T1).await;
    ctx.heal_party().await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 8).await;
    warp(ctx, MAP_SOMMET_APEX, TUBE_FOOT.0, TUBE_FOOT.1).await;
}

const JULIEN_KNEELS: &[Step] = &[
    Step::WalkInPlace(Dir::Down, Speed::Normal),
    Step::Delay(16),
    Step::WalkInPlace(Dir::Down, Speed::Normal),
];

/// `Trone_Zenith_Sc09`, the Zenith's rupture: the terminals overheat, the diffuser explodes, the
/// fake sky cracks (`FLAG_SIL_CIEL_VRAI`, `FLAG_SIL_ELYSIUM_ACTIF` cleared), Julien on his knees
/// (116), then black and the night map (`TroneZenith_Nuit_Sc10`).
async fn TroneZenith_Sc09(ctx: &Ctx) {
    ctx.lock_all().await;
    ctx.delay(30).await;
    play_se(ctx, "SE_THUNDER").await;
    // fallback: screen_flicker(4): a flash.
    screen_flash(ctx).await;
    say(ctx, Trone_Zenith_Sc09_T1).await;
    play_se(ctx, "SE_M_EXPLOSION").await;
    screen_shake(ctx, 4, 30).await;
    // fallback: METATILE_PRISME_BRISE has no tile: the diffuser stays drawn until the night map.
    say(ctx, Trone_Zenith_Sc09_T2).await;
    // fallback: the sky's crack (sky_crack_animation) and the true-night palette: a white flash,
    // then the night variant of the map (the source's own fallback).
    fallback(ctx, "sky_crack").await;
    ctx.fade(Fade::ToWhite).await;
    ctx.fade(Fade::FromWhite).await;
    ctx.set_flag(FLAG_SIL_ELYSIUM_ACTIF, false).await;
    ctx.set_flag(FLAG_SIL_CIEL_VRAI, true).await;
    say(ctx, Trone_Zenith_Sc09_T3).await;
    ctx.delay(60).await;
    say(ctx, Trone_Zenith_Sc09_T4).await;
    move_wait(ctx, LOCALID_ZENITH_JULIEN, JULIEN_KNEELS).await;
    say(ctx, Trone_Zenith_Sc09_T5).await;
    set_story(ctx, 116).await;
    ctx.fade(Fade::ToBlack).await;
    warp(ctx, MAP_TRONE_ZENITH_NUIT, NUIT_PLAYER.0, NUIT_PLAYER.1).await;
}

/// Julien on the throne, before the coords (unreachable: the coord row spans the platform).
/// The source gives him no line.
pub async fn TroneZenith_EventScript_Julien(_ctx: Ctx) {
    // fallback: no text in the source.
}

/// The ELYSIUM diffuser (7, 5). The source gives it no text.
pub async fn TroneZenith_EventScript_Diffuseur(_ctx: Ctx) {
    // fallback: no text in the source.
}

/// Elysium_E5, the only one awake (`VAR_SIL_FIN_SCENE` 5, on-frame), then `Elysium_Fin`.
pub async fn TroneZenith_Elysium_E5(ctx: Ctx) {
    elysium_e5(&ctx).await;
}

// ---- the night ----

/// No broken-prism tile (`METATILE_PRISME_BRISE`): the diffuser is drawn whole.
pub async fn TroneZenith_Nuit_OnLoad(_ctx: Ctx) {}

/// The friends hidden on arrival (`Sc11` shows them), as `TroneZenith`'s.
pub async fn TroneZenith_Nuit_OnTransition(ctx: Ctx) {
    TroneZenith_OnTransition(ctx).await;
}

const PLAYER_TO_EDGE: &[Step] = &[walk(Dir::Down), face(Dir::Down)];
const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const FRIENDS_RUN_UP: &[Step] = &[
    Step::Walk(Dir::Up, Speed::Fast),
    Step::Walk(Dir::Up, Speed::Fast),
];
const GWADA_LOOKS: &[Step] = &[walk(Dir::Up), face(Dir::Up)];
const MATHIEU_STARES: &[Step] = &[Step::WalkInPlace(Dir::Up, Speed::Normal)];
const ADRIEN_TO_JULIEN: &[Step] = &[walk(Dir::Up), walk(Dir::Up), face(Dir::Up)];

/// `Trone_Zenith_Sc10`, Noobody's wisdom (on-frame on 116, the night map), then `Sc11`, the
/// gathering: the friends run up the tube's stairs, the truth shared, Adrien's hand, Gwada's
/// white line (117), then the cabin (`Ascenseur_Sc12`).
///
/// Deviation: the source walks the player three cells south from the coord row, into the
/// terminals from its ends; the Rupture leaves the player at (6, 8) and `Sc10` walks one cell
/// south, so the friends stop on rows 10–11 and Adrien walks up to (7, 9) beside the player.
pub async fn TroneZenith_Nuit_Sc10(ctx: Ctx) {
    ctx.lock_all().await;
    // fallback: MUS_SIL_VRAI_CIEL (play_music): the night map's music (MUS_END) plays.
    move_wait(&ctx, PLAYER, PLAYER_TO_EDGE).await;
    say(&ctx, Trone_Zenith_Sc10_T1).await;
    move_wait(&ctx, PLAYER, FACE_UP).await;
    say(&ctx, Trone_Zenith_Sc10_T2).await;
    // Sc11.
    play_se(&ctx, "SE_ELEVATOR").await;
    ctx.delay(30).await;
    play_se(&ctx, "SE_DOOR").await;
    show(&ctx, LOCALID_ZENITH_NUIT_MATHIEU, FLAG_SIL_HIDE_ZENITH_MATHIEU).await;
    show(&ctx, LOCALID_ZENITH_NUIT_ADRIEN, FLAG_SIL_HIDE_ZENITH_ADRIEN).await;
    show(&ctx, LOCALID_ZENITH_NUIT_GWADA, FLAG_SIL_HIDE_ZENITH_GWADA).await;
    ctx.move_obj(LOCALID_ZENITH_NUIT_MATHIEU, FRIENDS_RUN_UP).await;
    ctx.move_obj(LOCALID_ZENITH_NUIT_ADRIEN, FRIENDS_RUN_UP).await;
    move_wait(&ctx, LOCALID_ZENITH_NUIT_GWADA, FRIENDS_RUN_UP).await;
    ctx.wait_move(LOCALID_ZENITH_NUIT_MATHIEU).await;
    ctx.wait_move(LOCALID_ZENITH_NUIT_ADRIEN).await;
    move_wait(&ctx, PLAYER, FACE_DOWN).await;
    say(&ctx, Trone_Zenith_Sc11_T1).await;
    ctx.turn(LOCALID_ZENITH_NUIT_ADRIEN, Dir::Up).await;
    say(&ctx, Trone_Zenith_Sc11_T2).await;
    move_wait(&ctx, LOCALID_ZENITH_NUIT_GWADA, GWADA_LOOKS).await;
    say(&ctx, Trone_Zenith_Sc11_T3).await;
    say(&ctx, Trone_Zenith_Sc11_T4).await;
    ctx.turn(LOCALID_ZENITH_NUIT_ADRIEN, Dir::Down).await;
    move_wait(&ctx, LOCALID_ZENITH_NUIT_MATHIEU, MATHIEU_STARES).await;
    say(&ctx, Trone_Zenith_Sc11_T5).await;
    say(&ctx, Trone_Zenith_Sc11_T6).await;
    say(&ctx, Trone_Zenith_Sc11_T7).await;
    say(&ctx, Trone_Zenith_Sc11_T8).await;
    move_wait(&ctx, LOCALID_ZENITH_NUIT_ADRIEN, ADRIEN_TO_JULIEN).await;
    say(&ctx, Trone_Zenith_Sc11_T9).await;
    say(&ctx, Trone_Zenith_Sc11_T10).await;
    ctx.turn(LOCALID_ZENITH_NUIT_JULIEN, Dir::Down).await;
    say(&ctx, Trone_Zenith_Sc11_T11).await;
    say(&ctx, Trone_Zenith_Sc11_T12).await;
    set_story(&ctx, 117).await;
    ctx.fade(Fade::ToBlack).await;
    warp(&ctx, MAP_TRONE_ZENITH_ASCENSEUR, 3, 4).await;
}

/// Julien on the night throne: no line in the source (the player leaves at once).
pub async fn TroneZenith_Nuit_EventScript_Julien(_ctx: Ctx) {
    // fallback: no text in the source.
}

/// The broken diffuser: no text in the source.
pub async fn TroneZenith_Nuit_EventScript_Diffuseur(_ctx: Ctx) {
    // fallback: no text in the source.
}
