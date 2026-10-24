//! Chapter 7, the Palais des Paradoxes (`ReverseCity_Gym`): `contenu-jeu/chapitres/07-reverse-city.md`
//! §4 (the inverted ice rink, the corridor of the lying doors, the hall of reflections, the
//! guide, Anna, Xela and Nomis) and §3 Reverse_City_Gym_Sc01 (Reverse, the inverse battle, his
//! flight; `VAR_SIL_STORY` 73 → 74) and Gym_Sc02 (Benou, the corrupted Mirror Badge, the Flux
//! Turbine, sync 70, `LOG_07`; 74 → 75).
//!
//! **The rooms** ("SIL4 Ch7 maps as built"): the hall (the mat, the guide), room 1 the ice grid
//! (plain floor until the engine has ice), room 2 the three mirror doors (coords on `VAR_TEMP_1`
//! 0, which nothing writes: they fire on every step), room 3 Reverse before the great mirror,
//! left by its exit mat back to room 1.

use crate::silice::prelude::*;

use super::ch07_reverse_city::set_input_inverted;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["ReverseCity_Gym"];

/// Room 1's S mat (warp 2), where the lying doors send the player back.
const TAPIS_S: (i16, i16) = (6, 29);
/// Room 3's arrival (warp 3), behind the middle door.
const SALLE_3: (i16, i16) = (6, 10);

// ---- map scripts ----

/// Chapter 12: Reverse (Thomas) stands in his gym for his rematches between his reflection at the
/// lighthouse (122) and the sky's split (127), when Thomas sets up his camp in the Breach for the
/// rest of the game (G-49, the author's ruling of 2026-10-24: Reverse is not in his gym while
/// Thomas is at the Breach).
fn reverse_in_gym(story: u16) -> bool {
    (123..127).contains(&story)
}

/// The inverted controls until Reverse is beaten (`set_input_inverted` on load, §4), and the
/// hide flags: Reverse until his flight (`FLAG_SIL_REVERSE_BATTU`), Benou only in Gym_Sc02, the
/// black pixels from Reverse's flight until Benou extracts the badge (`VAR_SIL_STORY` 74).
pub async fn ReverseCity_Gym_OnTransition(ctx: Ctx) {
    let battu = ctx.flag(FLAG_SIL_REVERSE_BATTU).await;
    set_input_inverted(&ctx, !battu).await;
    // Chapter 12: Reverse back in his gym for his rematches from 123 (after his reflection at the
    // lighthouse, `Reverse_City_PG_Sc04`, at 122), but not while Thomas (Reverse himself) is at the
    // Breach's camp, from 127 on (`Breche_PG_Sc10b`): G-49, the author's ruling.
    let rematch = reverse_in_gym(story(&ctx).await);
    ctx.set_flag(FLAG_SIL_HIDE_GYM07_REVERSE, battu && !rematch).await;
    ctx.set_flag(FLAG_SIL_HIDE_GYM07_BENOU, true).await;
    let pixels = story(&ctx).await == 74;
    ctx.set_flag(FLAG_SIL_HIDE_GYM07_PIXELS, !pixels).await;
    // fallback: the ice (`MB_ICE`, §4 room 1, §9): the source's own fallback, plain floor, the
    // room a maze of pillars (the map's 525 is `MB_THIN_ICE`, which the engine walks normally).
    fallback(&ctx, "ice").await;
}

// ---- room 2, the lying doors ----

/// A wrong mirror door: "the mirror sends you back… to the rink's entrance", room 1's S mat.
/// The warp ends the script.
async fn porte_piege(ctx: &Ctx) {
    ctx.lock_all().await;
    say(ctx, Gym07_Porte_Piege).await;
    let (x, y) = TAPIS_S;
    warp(ctx, MAP_REVERSE_CITY_GYM, x, y).await;
}

/// The left door (2–3, 14): the first statue's lie.
pub async fn ReverseCity_Gym_EventScript_PorteGauche(ctx: Ctx) {
    porte_piege(&ctx).await;
}

/// The right door (10–11, 14).
pub async fn ReverseCity_Gym_EventScript_PorteDroite(ctx: Ctx) {
    porte_piege(&ctx).await;
}

/// The middle door (6–7, 14), the right one ("Inversée, la vérité est"): to room 3.
pub async fn ReverseCity_Gym_EventScript_PorteMilieu(ctx: Ctx) {
    let (x, y) = SALLE_3;
    warp(&ctx, MAP_REVERSE_CITY_GYM, x, y).await;
}

/// The palace's rule, at room 2's entrance.
pub async fn ReverseCity_Gym_EventScript_SignRegle(ctx: Ctx) {
    msgbox(&ctx, Gym07_Sign_Regle, MsgBox::Sign).await;
}

/// The mime statue pointing left (its lie: the left door).
pub async fn ReverseCity_Gym_EventScript_Statue1(ctx: Ctx) {
    msgbox(&ctx, Gym07_Statue_1, MsgBox::Sign).await;
}

/// The mime statue hiding its eyes (its lie: the middle door is the trap).
pub async fn ReverseCity_Gym_EventScript_Statue2(ctx: Ctx) {
    msgbox(&ctx, Gym07_Statue_2, MsgBox::Sign).await;
}

// ---- the gym's trainers and guide ----

/// The guide by the hall's mat: the inverted battle explained.
pub async fn ReverseCity_Gym_EventScript_Guide(ctx: Ctx) {
    msgbox(&ctx, Gym07_Guide, MsgBox::Npc).await;
}

/// PATINEUSE ANNA, on room 2's landing past the rink (3, 19), sight 3 right.
pub async fn ReverseCity_Gym_EventScript_Anna(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM07_ANNA,
        TRAINER_SIL_PATINEUSE_ANNA,
        Gym07_Anna_Intro,
        Gym07_Anna_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym07_Anna_Defaite).await;
    }
}

/// MIME XELA, before the doors (8, 17), sight 2 left (the wall row's gap).
pub async fn ReverseCity_Gym_EventScript_Xela(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM07_XELA,
        TRAINER_SIL_MIME_XELA,
        Gym07_Xela_Intro,
        Gym07_Xela_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym07_Xela_Defaite).await;
    }
}

/// PSY NOMIS, room 3's niche (4, 8), sight 3 right (the corridor's width).
pub async fn ReverseCity_Gym_EventScript_Nomis(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM07_NOMIS,
        TRAINER_SIL_PSY_NOMIS,
        Gym07_Nomis_Intro,
        Gym07_Nomis_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym07_Nomis_Defaite).await;
    }
}

// ---- Reverse ----

/// Reverse_City_Gym_Sc01, Reverse the Dorsal Dancer: the coord row (5–7, 5) behind him (or
/// talking to him), `VAR_SIL_STORY` 73. He speaks without turning round; the inverse battle;
/// his flight; the black pixels (`FLAG_SIL_HIDE_GYM07_PIXELS` cleared); `FLAG_SIL_REVERSE_BATTU`
/// (the controls and the vault's lock fall with him). Sets 74, then Gym_Sc02 follows at once.
pub async fn ReverseCity_Gym_Sc01(ctx: Ctx) {
    if story(&ctx).await >= 123 {
        // Hidden outside [`reverse_in_gym`]'s range; the coord row behind him does nothing then.
        if !reverse_in_gym(story(&ctx).await) {
            return;
        }
        super::ch12_revanches::revanche(&ctx, &super::ch12_revanches::REVERSE).await;
        return;
    }
    if story(&ctx).await != 73 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Gym07_Sc01_T1).await;
    say(&ctx, Gym07_Sc01_T2).await;
    say(&ctx, Gym07_Sc01_T3).await;
    // set_inverse_battle(true) (R-2): Reverse's battles are inverse, open on
    // Gym07_Inverse_Debut and show him from the back (`silice::rules::INVERSE`; the back is
    // his front pic mirrored until the asset exists).
    if battle_trainer(&ctx, TRAINER_SIL_REVERSE_1, Gym07_Sc01_D).await != BattleOutcome::Won {
        ctx.release_all().await;
        return;
    }
    say(&ctx, Gym07_Sc01_T4).await;
    say(&ctx, Gym07_Sc01_T5).await;
    say(&ctx, Gym07_Sc01_T6).await;
    // The moonwalk (`WalkBackwardsFast(Up) x6`) has a wall right above him (6, 3), the top of
    // the hall of reflections: the source's §9 fallback for Reverse instead, the flash and he
    // is gone.
    screen_flash(&ctx, BLACK).await;
    hide(&ctx, LOCALID_GYM07_REVERSE).await;
    show(&ctx, LOCALID_GYM07_PIXELS_NOIRS, FLAG_SIL_HIDE_GYM07_PIXELS).await;
    say(&ctx, Gym07_Sc01_T7).await;
    ctx.set_flag(FLAG_SIL_REVERSE_BATTU, true).await;
    // The inversion falls with the Node (the source's `set_input_inverted(false)`).
    set_input_inverted(&ctx, false).await;
    set_story(&ctx, 74).await;
    ctx.release_all().await;
    ReverseCity_Gym_Sc02(ctx).await;
}

/// Benou comes out of the nook behind the mirror (9, 4): the source's `Walk(Left) x3` is two
/// cells, to (7, 4), since the black pixels lie on (6, 4).
const BENOU_IN: &[Step] = &[walk(Dir::Left), walk(Dir::Left), Step::FacePlayer];
/// He leans over the pixels, on his left.
const BENOU_PIXELS: &[Step] = &[face(Dir::Left)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
/// He goes back into his nook to hack the north gate. The source's `Walk(Down) x4` would walk
/// into a player standing on the coord row below him (5–7, 5); row 4 is the room's top.
const BENOU_OUT: &[Step] = &[walk(Dir::Right), walk(Dir::Right)];

/// Reverse_City_Gym_Sc02, Benou and the black pixels: chained from Gym_Sc01, and the map's
/// on-frame entry at `VAR_SIL_STORY` 74 (a replay if the story stopped there). The pixels
/// become the corrupted Mirror Badge (`FLAG_SIL_BADGE_07`, `_CORROMPU`), the Flux Turbine
/// (`FLAG_SIL_MODULE_CASCADE`), sync 70, Reverse's file as `LOG_07`. Sets 75.
pub async fn ReverseCity_Gym_Sc02(ctx: Ctx) {
    if story(&ctx).await != 74 {
        return;
    }
    ctx.lock_all().await;
    show(&ctx, LOCALID_GYM07_BENOU, FLAG_SIL_HIDE_GYM07_BENOU).await;
    move_wait(&ctx, LOCALID_GYM07_BENOU, BENOU_IN).await;
    say(&ctx, Gym07_Sc02_T1).await;
    move_wait(&ctx, LOCALID_GYM07_BENOU, BENOU_PIXELS).await;
    say(&ctx, Gym07_Sc02_T2).await;
    hide(&ctx, LOCALID_GYM07_PIXELS_NOIRS).await;
    play_fanfare(&ctx, "MUS_OBTAIN_BADGE").await;
    say(&ctx, Gym07_Sc02_T3).await;
    ctx.set_flag(FLAG_SIL_BADGE_07, true).await;
    ctx.set_flag(FLAG_SIL_BADGE_07_CORROMPU, true).await;
    move_wait(&ctx, LOCALID_GYM07_BENOU, FACE_PLAYER).await;
    say(&ctx, Gym07_Sc02_T4).await;
    ctx.fade(Fade::ToBlack).await;
    ctx.delay(60).await;
    ctx.fade(Fade::FromBlack).await;
    ctx.set_flag(FLAG_SIL_MODULE_CASCADE, true).await;
    play_fanfare(&ctx, "MUS_OBTAIN_TMHM").await;
    say(&ctx, Gym07_Sc02_T5).await;
    ctx.set_var(VAR_SIL_SYNC, 70).await;
    // fallback: show_sync (the sync gauge): nothing drawn; T6 gives the value.
    show_sync(&ctx, 70).await;
    say(&ctx, Gym07_Sc02_T6).await;
    say(&ctx, Gym07_Sc02_T7).await;
    ctx.set_flag(FLAG_SIL_LOG_07, true).await;
    // The log (`msgbox(…, Sign)`): the log as a message, as the Mallette's RAPPORTS reads it again.
    say(&ctx, Gym07_Sc02_Log07).await;
    say(&ctx, Gym07_Sc02_T8).await;
    say(&ctx, Gym07_Sc02_T9).await;
    move_wait(&ctx, LOCALID_GYM07_BENOU, BENOU_OUT).await;
    hide(&ctx, LOCALID_GYM07_BENOU).await;
    set_story(&ctx, 75).await;
    ctx.release_all().await;
}

// ---- hidden items ----

/// Room 3, at the foot of the great mirror where Reverse stood (6, 4): only once he has fled
/// and Benou has taken the pixels ("the reflection left something", §7).
pub async fn ReverseCity_Gym_EventScript_HiddenAlakazite(ctx: Ctx) {
    if story(&ctx).await < 75 {
        return;
    }
    // A sign running the hidden-item script: a Gen3 hidden item cannot wait for the story.
    hidden_item(&ctx, FLAG_SIL_ITEM_GYM07_ALAKAZITE, ITEM_SIL_ALAKAZITE).await;
}
