//! Chapter 11, the Apex: the Tower's roof (`SommetApex`; `contenu-jeu/chapitres/11-apex-zenith.md`
//! §3 `Sommet_Apex_Sc01`–`Sc07`, §5 the roof's trainers, §7 its items).
//!
//! The way (`VAR_SIL_STORY` 109 → 115): the arrival from the vault elevator (`Sc01`, on-frame on
//! 109), Gogo and Romain on the catwalk (`Sc02`, 110), Poussin's solo and the duel (`Sc03`, 111,
//! then `Sc04`: the misunderstanding, the resignation, 112), Julien in white and gold (`Sc05`:
//! the console or the stage's steps; the Mallette, the dead world, Elysium, the first choice of
//! the alternate ending, 113), Romain's first-aid kit (`Sc06`, 114), the crystal tube (`Sc07`).
//!
//! The endings' scenes that play here (`VAR_SIL_FIN_SCENE`, on-frame): Elysium_E3 (3, Gogo and
//! Romain's eternal party) and `Trone_Zenith_Sc08_Lose`'s waking (8, Romain at the tube's foot).
//!
//! Map facts the scenes use (the plan's "SIL5 Ch10–Ch11 maps as built"): the console is on row 4
//! (10–13, 4), solid, read from the stage (row 5); the tube's foot is (11, 3); the stage is rows
//! 5–8 inside its ring, its steps (10–13, 9); Poussin (11, 6), Julien (14, 3) hidden, Gogo (8, 17)
//! and Romain (9, 17).

use super::ch04_port::{Puzzle, push_object};
use super::ch08_evalon::lost;
use super::ch11_endings::{elysium_e1, ElysiumFrom};
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["SommetApex"];

/// The crystal tube's foot (warp 1's landing), where `Trone_Zenith_Sc08_Lose` wakes the player.
pub const TUBE_FOOT: (i16, i16) = (11, 3);
/// Where Elysium_E3 puts the player: beside Gogo and Romain's crates (the review's (10, 17)).
pub const E3_CELL: (i16, i16) = (10, 17);
/// The crystal elevator's cabin cell from the Apex (`TroneZenith_Ascenseur` warp 0).
const CABIN_FROM_APEX: (i16, i16) = (3, 4);

/// No tile for the lit tube (`METATILE_CRISTAL_ON`, the plan's "Missing"): from story 113 the
/// tube stays drawn as it is; `Sc07` reads the story, not the tile.
pub async fn SommetApex_OnLoad(_ctx: Ctx) {}

/// The hide flags by story: Poussin gone from 112 (his `Sc04` sets it too), Julien hidden
/// outside his `Sc05`. Chapter 12's altar (`FLAG_SIL_HIDE_APEX_SOCLE`) shows from 120 (the plan's
/// "SIL6 Ch12 maps": the Apex's post-game socle).
///
/// The sky (§9 `set_weather`, `set_palette`): the header's shade, the fog after Poussin (112),
/// Elysium's drought and gold while `FLAG_SIL_ELYSIUM_ACTIF`, no weather under the true sky
/// (`FLAG_SIL_CIEL_VRAI`, from the Zenith's rupture).
pub async fn SommetApex_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    if ctx.flag(FLAG_SIL_ELYSIUM_ACTIF).await {
        ctx.set_weather(u16::from(engine::weather::DROUGHT)).await;
        set_palette(&ctx, ELYSIUM_GOLD, 0, false).await;
    } else if ctx.flag(FLAG_SIL_CIEL_VRAI).await {
        ctx.set_weather(u16::from(engine::weather::NONE)).await;
    } else if s >= 112 {
        ctx.set_weather(u16::from(engine::weather::FOG_HORIZONTAL)).await;
    }
    if s >= 112 {
        ctx.set_flag(FLAG_SIL_HIDE_APEX_POUSSIN, true).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_APEX_JULIEN, true).await;
    if s >= 120 {
        ctx.set_flag(FLAG_SIL_HIDE_APEX_SOCLE, false).await;
    }
}

const UP_2: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];

/// `Sommet_Apex_Sc01`, the riff in the wind (on-frame on 109, the vault elevator's arrival):
/// two steps north, the narration, 110.
pub async fn SommetApex_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    // fallback: MUS_SIL_APEX_SOLO (play_music): the map's music plays.
    fallback(&ctx, "play_music").await;
    move_wait(&ctx, PLAYER, UP_2).await;
    say(&ctx, Sommet_Apex_Sc01_T1).await;
    set_story(&ctx, 110).await;
    ctx.release_all().await;
}

const GOGO_JUMP: &[Step] = &[Step::Jump(Dir::Right, 0)];

/// `Sommet_Apex_Sc02`, Gogo and Romain worn out (the catwalk's coords (10–13, 18), 110): no
/// battle, Romain's rain hint, 111.
pub async fn SommetApex_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.turn(LOCALID_APEX_ROMAIN, Dir::Right).await;
    ctx.delay(16).await;
    say(&ctx, Sommet_Apex_Sc02_T1).await;
    move_wait(&ctx, LOCALID_APEX_GOGO, GOGO_JUMP).await;
    say(&ctx, Sommet_Apex_Sc02_T2).await;
    ctx.turn(LOCALID_APEX_ROMAIN, Dir::Left).await;
    say(&ctx, Sommet_Apex_Sc02_T3).await;
    ctx.turn(LOCALID_APEX_ROMAIN, Dir::Right).await;
    say(&ctx, Sommet_Apex_Sc02_T4).await;
    say(&ctx, Sommet_Apex_Sc02_T5).await;
    set_story(&ctx, 111).await;
    ctx.release_all().await;
}

const POUSSIN_ADJUST: &[Step] = &[Step::WalkInPlace(Dir::Down, Speed::Normal)];

/// `Sommet_Apex_Sc03`, the Grand-Duc's solo (the stage's steps (10–13, 9), 111): up onto the
/// stage, the silence, his speech, the duel (`TRAINER_SIL_POUSSIN_1`), then `Sc04`. A loss whites
/// out to Nébula and the scene plays again.
pub async fn SommetApex_Sc03(ctx: Ctx) {
    ctx.lock_all().await;
    move_wait(&ctx, PLAYER, UP_2).await;
    say(&ctx, Sommet_Apex_Sc03_T1).await;
    // fallback: stop_music() and the two seconds of silence: a pause.
    ctx.delay(120).await;
    play_se(&ctx, "SE_PIN").await;
    ctx.turn(LOCALID_APEX_POUSSIN, Dir::Down).await;
    say(&ctx, Sommet_Apex_Sc03_T2).await;
    move_wait(&ctx, LOCALID_APEX_POUSSIN, POUSSIN_ADJUST).await;
    say(&ctx, Sommet_Apex_Sc03_T3).await;
    // fallback: the mid-battle lines (§4.1: the first KO, Perish Song, Milotic sent out) and the
    // Primal Reversion's staging line; the battle itself (Primal Kyogre's rain) is the battle
    // engine's.
    fallback(&ctx, "battle_lines").await;
    if lost(battle_trainer(&ctx, TRAINER_SIL_POUSSIN_1, Sommet_Apex_Sc03_Defeat).await) {
        return;
    }
    SommetApex_Sc04(&ctx).await;
}

const POUSSIN_PUZZLED: &[Step] = &[Step::Emote(Emote::Question)];
const POUSSIN_TO_CONSOLE: &[Step] = &[walk(Dir::Up), face(Dir::Up)];
const POUSSIN_FROM_CONSOLE: &[Step] = &[walk(Dir::Down), face(Dir::Down)];
const POUSSIN_LEAVES: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    face(Dir::Up),
];

/// `Sommet_Apex_Sc04`, the misunderstanding and the resignation (straight after the win): the
/// fog, Poussin's logistics, the doubt about Adrien's translations, the resignation sent to the
/// Siège from the console, his exit by the service elevator (112).
///
/// Deviation: the console is one row north of the source's (the map's row 4, solid), so Poussin
/// walks up one cell to it (not three) and back; then east along the stage's row 6 (seven cells,
/// the ring's east edge) rather than ten, and vanishes towards the service elevator.
async fn SommetApex_Sc04(ctx: &Ctx) {
    // The rain of the battle gives way to a thick fog; fallback: MUS_SIL_APEX_BRUME (play_music).
    set_weather(ctx, engine::weather::FOG_HORIZONTAL).await;
    fallback(ctx, "play_music").await;
    say(ctx, Sommet_Apex_Sc04_T1).await;
    say(ctx, Sommet_Apex_Sc04_T2).await;
    move_wait(ctx, LOCALID_APEX_POUSSIN, POUSSIN_PUZZLED).await;
    say(ctx, Sommet_Apex_Sc04_T3).await;
    say(ctx, Sommet_Apex_Sc04_T4).await;
    say(ctx, Sommet_Apex_Sc04_T5).await;
    ctx.delay(60).await;
    say(ctx, Sommet_Apex_Sc04_T6).await;
    say(ctx, Sommet_Apex_Sc04_T7).await;
    move_wait(ctx, LOCALID_APEX_POUSSIN, POUSSIN_TO_CONSOLE).await;
    play_se(ctx, "SE_PC_LOGIN").await;
    say(ctx, Sommet_Apex_Sc04_T8).await;
    move_wait(ctx, LOCALID_APEX_POUSSIN, POUSSIN_FROM_CONSOLE).await;
    say(ctx, Sommet_Apex_Sc04_T9).await;
    move_wait(ctx, LOCALID_APEX_POUSSIN, POUSSIN_LEAVES).await;
    play_se(ctx, "SE_ELEVATOR").await;
    hide(ctx, LOCALID_APEX_POUSSIN).await;
    ctx.set_flag(FLAG_SIL_HIDE_APEX_POUSSIN, true).await;
    say(ctx, Sommet_Apex_Sc04_T10).await;
    set_story(ctx, 112).await;
    ctx.release_all().await;
}

/// A yes/no whose cursor starts on No (§9: the choices that end the game), the answer also in
/// `VAR_RESULT`.
pub(super) async fn ask_no(ctx: &Ctx, text: TextId) -> bool {
    ctx.message(text).await;
    ctx.wait_message().await;
    let yes = ctx.yes_no_default_no().await;
    ctx.set_var(VAR_RESULT, yes as u16).await;
    yes
}

/// The two-step choice of the alternate ending (§3 "Où se fait le choix"): the question, then
/// the confirmation (`Sommet_Apex_Sc05_C2`, both cursors on No). Whether both answered yes.
pub(super) async fn chooses_elysium(ctx: &Ctx, question: TextId) -> bool {
    ask_no(ctx, question).await && ask_no(ctx, Sommet_Apex_Sc05_C2).await
}

const JULIEN_ARRIVES: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Left),
    walk(Dir::Left),
];
const JULIEN_BACK_B: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    face(Dir::Up),
];
const JULIEN_TO_CONSOLE_A: &[Step] = &[walk(Dir::Left), walk(Dir::Left), face(Dir::Up)];
const JULIEN_CLOSER_A: &[Step] = &[Step::WalkInPlace(Dir::Down, Speed::Normal)];
const JULIEN_CLOSER_B: &[Step] = &[walk(Dir::Down)];
const FACE_UP: &[Step] = &[face(Dir::Up)];

/// `Sommet_Apex_Sc05`, the white-and-gold glove (story 112): trigger A, the console (its four
/// signs); trigger B, the stage's foot (coords (10–13, 10), the player going back down). Julien,
/// the Mallette taken (`FLAG_SIL_MALLETTE_PRISE`; the Mega module given back), 100 %, the dead
/// world, Elysium, the lit tube, then the alternate ending's first choice: two yes (both cursors
/// on No) play `Elysium_E1`; otherwise Julien vanishes (113) and `Sc06` follows. Before 112 the
/// console is a sign (the dead pixel); after, nothing.
///
/// Deviations: trigger A's Julien walks two cells west along row 3 behind the console (the
/// source's (12, 3) is that row); his step towards the player is in place there (the console is
/// solid). The tube has no lit tile (`METATILE_CRISTAL_ON`).
pub async fn SommetApex_Sc05(ctx: Ctx) {
    let s = story(&ctx).await;
    if s != 112 {
        if s < 112 {
            msgbox(&ctx, Sommet_Apex_Sign_Cristal_Off, MsgBox::Sign).await;
        }
        return;
    }
    // A sign (the console) runs with the player facing north; a coord, south.
    let console = ctx.player_facing().await == Dir::Up;
    ctx.lock_all().await;
    ctx.delay(60).await;
    if console {
        say(&ctx, Sommet_Apex_Sc05_T1a).await;
        show(&ctx, LOCALID_APEX_JULIEN, FLAG_SIL_HIDE_APEX_JULIEN).await;
        play_se(&ctx, "SE_PIN").await;
        say(&ctx, Sommet_Apex_Sc05_T2a).await;
    } else {
        show(&ctx, LOCALID_APEX_JULIEN, FLAG_SIL_HIDE_APEX_JULIEN).await;
        move_wait(&ctx, LOCALID_APEX_JULIEN, JULIEN_ARRIVES).await;
        move_wait(&ctx, PLAYER, FACE_UP).await;
    }
    say(&ctx, Sommet_Apex_Sc05_T3).await;
    say(&ctx, Sommet_Apex_Sc05_T4).await;
    // fallback: MUS_SIL_JULIEN_MESSIE (play_music).
    say(&ctx, Sommet_Apex_Sc05_T5).await;
    if ask(&ctx, Sommet_Apex_Sc05_T6).await {
        say(&ctx, Sommet_Apex_Sc05_T6y).await;
    } else {
        say(&ctx, Sommet_Apex_Sc05_T6n).await;
    }
    remove_item(&ctx, ITEM_SIL_MALLETTE, 1).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_PRISE, true).await;
    say(&ctx, Sommet_Apex_Sc05_T7).await;
    if ctx.flag(FLAG_SIL_MODULE_MEGA).await {
        say(&ctx, Sommet_Apex_Sc05_T7m).await;
        play_se(&ctx, "SE_PIN").await;
        say(&ctx, Sommet_Apex_Sc05_T7m2).await;
    }
    say(&ctx, Sommet_Apex_Sc05_T8).await;
    say(&ctx, Sommet_Apex_Sc05_T9).await;
    let back = if console {
        JULIEN_TO_CONSOLE_A
    } else {
        JULIEN_BACK_B
    };
    move_wait(&ctx, LOCALID_APEX_JULIEN, back).await;
    play_se(&ctx, "SE_PC_ON").await;
    // GOLD: the prelude's nearest colour is YELLOW (no gold constant).
    screen_flash(&ctx, YELLOW).await;
    ctx.set_var(VAR_SIL_SYNC, 100).await;
    show_sync(&ctx, 100).await;
    msgbox(&ctx, Sommet_Apex_Sc05_T10, MsgBox::Default).await;
    // set_palette(MAP_SOMMET_APEX, ELYSIUM_GOLD): the screens turn gold.
    set_palette(&ctx, ELYSIUM_GOLD, PALETTE_FRAMES, false).await;
    say(&ctx, Sommet_Apex_Sc05_T11).await;
    ctx.turn(LOCALID_APEX_JULIEN, Dir::Down).await;
    say(&ctx, Sommet_Apex_Sc05_T12).await;
    say(&ctx, Sommet_Apex_Sc05_T13).await;
    say(&ctx, Sommet_Apex_Sc05_T14).await;
    say(&ctx, Sommet_Apex_Sc05_T15).await;
    say(&ctx, Sommet_Apex_Sc05_T16).await;
    say(&ctx, Sommet_Apex_Sc05_T17).await;
    say(&ctx, Sommet_Apex_Sc05_T18).await;
    let closer = if console {
        JULIEN_CLOSER_A
    } else {
        JULIEN_CLOSER_B
    };
    move_wait(&ctx, LOCALID_APEX_JULIEN, closer).await;
    say(&ctx, Sommet_Apex_Sc05_T19).await;
    // set_weather(WEATHER_DROUGHT): the sky turns orange, the heat.
    set_weather(&ctx, engine::weather::DROUGHT).await;
    say(&ctx, Sommet_Apex_Sc05_T20).await;
    play_se(&ctx, "SE_ELEVATOR").await;
    say(&ctx, Sommet_Apex_Sc05_T21).await;
    // The alternate ending's first chance (R-3): two yes in a row.
    if ask_no(&ctx, Sommet_Apex_Sc05_C1).await {
        if ask_no(&ctx, Sommet_Apex_Sc05_C2).await {
            elysium_e1(&ctx, ElysiumFrom::Apex).await;
            return;
        }
        say(&ctx, Sommet_Apex_Sc05_C2n).await;
    } else {
        say(&ctx, Sommet_Apex_Sc05_C1n).await;
    }
    ctx.fade(Fade::ToWhite).await;
    hide(&ctx, LOCALID_APEX_JULIEN).await;
    ctx.fade(Fade::FromWhite).await;
    say(&ctx, Sommet_Apex_Sc05_T22).await;
    ctx.set_flag(FLAG_SIL_ELYSIUM_ACTIF, true).await;
    set_story(&ctx, 113).await;
    SommetApex_Sc06(&ctx).await;
}

const FRIENDS_RUN_IN: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];
const GOGO_TO_LIGHT: &[Step] = &[walk(Dir::Left)];
const ROMAIN_CATCHES: &[Step] = &[walk(Dir::Left), face(Dir::Left)];

/// `Sommet_Apex_Sc06`, the first-aid kit (straight after `Sc05`): Gogo and Romain run in, Romain
/// holds Gogo back, heals the party and gives two Full Restores (`FLAG_SIL_ITEM_APEX_ROMAIN`),
/// 114.
///
/// Deviation: they come up two cells from the catwalk's (11, 14)/(12, 14), not three, and step
/// west rather than north: trigger B leaves the player on row 10, right above them.
async fn SommetApex_Sc06(ctx: &Ctx) {
    ctx.set_object_pos(LOCALID_APEX_GOGO, 11, 14).await;
    ctx.set_object_pos(LOCALID_APEX_ROMAIN, 12, 14).await;
    ctx.move_obj(LOCALID_APEX_GOGO, FRIENDS_RUN_IN).await;
    move_wait(ctx, LOCALID_APEX_ROMAIN, FRIENDS_RUN_IN).await;
    ctx.wait_move(LOCALID_APEX_GOGO).await;
    say(ctx, Sommet_Apex_Sc06_T1).await;
    move_wait(ctx, LOCALID_APEX_GOGO, GOGO_TO_LIGHT).await;
    move_wait(ctx, LOCALID_APEX_ROMAIN, ROMAIN_CATCHES).await;
    say(ctx, Sommet_Apex_Sc06_T2).await;
    ctx.turn(LOCALID_APEX_ROMAIN, Dir::Up).await;
    say(ctx, Sommet_Apex_Sc06_T3).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.heal_party().await;
    ctx.fade(Fade::FromBlack).await;
    say(ctx, Sommet_Apex_Sc06_T4).await;
    gift(ctx, FLAG_SIL_ITEM_APEX_ROMAIN, ITEM_FULL_RESTORE, 2, Obtain::Message).await;
    say(ctx, Sommet_Apex_Sc06_T5).await;
    set_story(ctx, 114).await;
    ctx.release_all().await;
}

/// `Sommet_Apex_Sc07`, the crystal elevator (the tube's signs (11, 2), (12, 2)): from 114 (and
/// 115, a retry after `Trone_Zenith_Sc08_Lose`), the point of no return asked, then the cabin
/// (`TroneZenith_Ascenseur`, whose `Sc00` plays the ascent). Before, the dead tube.
pub async fn SommetApex_Sc07(ctx: Ctx) {
    if !matches!(story(&ctx).await, 114 | 115) {
        msgbox(&ctx, Sommet_Apex_Sign_Cristal_Off, MsgBox::Sign).await;
        return;
    }
    ctx.lock_all().await;
    if !ask(&ctx, Sommet_Apex_Sc07_T1).await {
        ctx.release_all().await;
        return;
    }
    play_se(&ctx, "SE_ELEVATOR").await;
    warp(
        &ctx,
        MAP_TRONE_ZENITH_ASCENSEUR,
        CABIN_FROM_APEX.0,
        CABIN_FROM_APEX.1,
    )
    .await;
}

/// Elysium_E3, the eternal party (`VAR_SIL_FIN_SCENE` 3, on-frame): Gogo and Romain on their
/// crates, then the player's house (`Elysium_E4`).
pub async fn SommetApex_Elysium_E3(ctx: Ctx) {
    super::ch11_endings::elysium_e3(&ctx).await;
}

/// `Trone_Zenith_Sc08_Lose`'s waking (`VAR_SIL_FIN_SCENE` 8, on-frame, at the tube's foot):
/// Romain beside the player, healed; the tube still takes the player up (115).
pub async fn SommetApex_Sc08_Lose(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.set_var(VAR_SIL_FIN_SCENE, 0).await;
    ctx.set_object_pos(LOCALID_APEX_ROMAIN, TUBE_FOOT.0 + 1, TUBE_FOOT.1)
        .await;
    ctx.turn(LOCALID_APEX_ROMAIN, Dir::Left).await;
    move_wait(&ctx, PLAYER, FACE_RIGHT).await;
    say(&ctx, Trone_Zenith_Sc08_Lose_T2).await;
    ctx.release_all().await;
}

const FACE_RIGHT: &[Step] = &[face(Dir::Right)];

/// Poussin talked to before his solo: the solo's own scene from the steps (111), else nothing
/// (the source gives him no idle line).
pub async fn SommetApex_EventScript_Poussin(_ctx: Ctx) {
    // fallback: no idle line in the source; the steps' coords start his scene.
}

/// Gogo: his idle line after `Sc02`, the second after `Sc06`.
pub async fn SommetApex_EventScript_Gogo(ctx: Ctx) {
    let t = if story(&ctx).await >= 114 {
        Sommet_Apex_Gogo_Idle2
    } else {
        Sommet_Apex_Gogo_Idle
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// Romain: his idle line after `Sc02`, the second after `Sc06`.
pub async fn SommetApex_EventScript_Romain(ctx: Ctx) {
    let t = if story(&ctx).await >= 114 {
        Sommet_Apex_Romain_Idle2
    } else {
        Sommet_Apex_Romain_Idle
    };
    msgbox(&ctx, t, MsgBox::Npc).await;
}

/// Élite Marc (17, 22), sight 4 west.
pub async fn SommetApex_EventScript_Marc(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_APEX_MARC,
        TRAINER_SIL_APEX_ELITE_MARC,
        Sommet_Apex_Marc_Intro,
        Sommet_Apex_Marc_Defeat,
    )
    .await
    {
        post_battle(&ctx, Sommet_Apex_Marc_After).await;
    }
}

/// Élite Sophie (3, 19), sight 3 east.
pub async fn SommetApex_EventScript_Sophie(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_APEX_SOPHIE,
        TRAINER_SIL_APEX_ELITE_SOPHIE,
        Sommet_Apex_Sophie_Intro,
        Sommet_Apex_Sophie_Defeat,
    )
    .await
    {
        post_battle(&ctx, Sommet_Apex_Sophie_After).await;
    }
}

/// Roadie Kevin (8, 13), sight 4 east.
pub async fn SommetApex_EventScript_Kevin(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_APEX_KEVIN,
        TRAINER_SIL_APEX_ROADIE_KEVIN,
        Sommet_Apex_Kevin_Intro,
        Sommet_Apex_Kevin_Defeat,
    )
    .await
    {
        post_battle(&ctx, Sommet_Apex_Kevin_After).await;
    }
}

/// The Full Restore ball (2, 23).
pub async fn SommetApex_EventScript_ItemFullRestore(ctx: Ctx) {
    find_item(&ctx, ITEM_FULL_RESTORE, 1).await;
}

/// The Max Revive ball (21, 9), behind the crates.
pub async fn SommetApex_EventScript_ItemMaxRevive(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_REVIVE, 1).await;
}

/// The two metal crates (19, 11), (20, 11) and their cells (`VAR_TEMP_2`, `VAR_TEMP_3`).
pub(crate) const CAISSES: Puzzle = Puzzle {
    map: MAP_SOMMET_APEX,
    objects: &[
    (LOCALID_APEX_CAISSE_1, VAR_TEMP_2, (19, 11)),
    (LOCALID_APEX_CAISSE_2, VAR_TEMP_3, (20, 11)),
],
    free: caisse_free,
};

/// The floor the crates may be pushed onto: the pocket east of the stage (19–22, 9–10), row 11
/// west of the wall (4–20), rows 12–13.
fn caisse_free(_: (i16, i16), (x, y): (i16, i16)) -> bool {
    matches!((x, y), (19..=22, 9..=10) | (4..=20, 11) | (1..=22, 12..=13))
}

/// The crates: the Champ Magnétique (`FLAG_SIL_MODULE_FORCE`), until Julien takes the Mallette
/// (`FLAG_SIL_MALLETTE_PRISE`: the source's line, the crates stay).
pub async fn SommetApex_EventScript_Caisse(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_MALLETTE_PRISE).await {
        msgbox(&ctx, Sommet_Apex_Sign_Caisses_NoMallette, MsgBox::Sign).await;
        return;
    }
    push_object(&ctx).await;
}

pub async fn SommetApex_EventScript_Ampli(ctx: Ctx) {
    msgbox(&ctx, Sommet_Apex_Sign_Ampli, MsgBox::Sign).await;
}

pub async fn SommetApex_EventScript_Service(ctx: Ctx) {
    msgbox(&ctx, Sommet_Apex_Sign_Service, MsgBox::Sign).await;
}

pub async fn SommetApex_EventScript_Ciel(ctx: Ctx) {
    msgbox(&ctx, Sommet_Apex_Sign_Ciel, MsgBox::Sign).await;
}

/// The vault elevator's doors (11–12, 28): they never go down (the point of no return).
pub async fn SommetApex_EventScript_AscenseurBloque(ctx: Ctx) {
    msgbox(&ctx, Sommet_Apex_Sign_Ascenseur_Bloque, MsgBox::Sign).await;
}

pub async fn SommetApex_EventScript_Maintenance(ctx: Ctx) {
    msgbox(&ctx, Sommet_Apex_Sign_Maintenance, MsgBox::Sign).await;
}

