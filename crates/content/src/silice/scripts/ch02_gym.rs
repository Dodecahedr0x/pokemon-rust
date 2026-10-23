//! Chapter 2, Yohan's chessboard arena (`GrisEchec_Gym`) and its Chambre Sourde
//! (`GrisEchec_Gym_Sourde`): `contenu-jeu/chapitres/02-gris-echec.md` §4 (the hall, Salle 1's
//! binary tiles, Salle 2's invisible maze, the deaf room, Salle 3's chessboard) and §3 Gym2_Sc10
//! to Sc12 (Yohan, Gwada's module and Lainergie, LOG_02). `VAR_SIL_STORY` 25 → 27.
//!
//! **Temp-var triggers** ("SIL2 Ch2–Ch3 maps as built"): every Salle 1 tile and Salle 3 watched
//! square is a coord event on `VAR_TEMP_3` 0, which no script on this map writes, so they always
//! fire and the scripts decide. Salle 1 counts the right bits in `VAR_TEMP_2` (cleared on every
//! map load, as Gen3's `ClearTempFieldEventData`).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["GrisEchec_Gym", "GrisEchec_Gym_Sourde"];

// ---- map scripts ----

/// The robot and Gym2_Sc11's actors (Gwada, Wattouat, Lainergie) are hidden on every load:
/// their scenes show them.
pub async fn GrisEchec_Gym_OnTransition(ctx: Ctx) {
    for f in [
        FLAG_SIL_HIDE_GYM2_ROBOT,
        FLAG_SIL_HIDE_GYM2_GWADA,
        FLAG_SIL_HIDE_GYM2_WATTOUAT,
        FLAG_SIL_HIDE_GYM2_LAINERGIE,
    ] {
        ctx.set_flag(f, true).await;
    }
}

// ---- the hall ----

pub async fn GrisEchec_Gym_EventScript_Conseiller(ctx: Ctx) {
    msgbox(&ctx, Gym2_Conseiller, MsgBox::Npc).await;
}

pub async fn GrisEchec_Gym_EventScript_EcranHall(ctx: Ctx) {
    msgbox(&ctx, Gym2_Ecran_Hall, MsgBox::Sign).await;
}

/// Salle 1's screen: "enter your key". With the Carte Perforée, its code too.
pub async fn GrisEchec_Gym_EventScript_EcranS1(ctx: Ctx) {
    msgbox(&ctx, Gym2_S1_Ecran, MsgBox::Sign).await;
    if ctx.flag(FLAG_SIL_CARTE_PERFOREE).await {
        // fallback: reading the Carte Perforée from the bag (`Carte_Perforee_Lire`, M9 item
        // use): the screen shows it (the source's own §9 fallback).
        fallback(&ctx, "key item use").await;
        msgbox(&ctx, Carte_Perforee_Lire, MsgBox::Sign).await;
    }
}

// ---- Salle 1, the binary tiles (y 30–37) ----

/// The robot comes from the right of the player's tile: placed three cells right, it walks
/// left twice (the source's `Walk(Left) x2`), so it ends beside the player.
const ROBOT_IN: &[Step] = &[walk(Dir::Left), walk(Dir::Left), Step::FacePlayer];

/// A parity error (a wrong tile, or the doorway without the 8 bits): the screen, the robot's
/// battle while it is undefeated, then back to the hall's top (7, 38). The warp ends the script
/// and clears the count.
async fn s1_error(ctx: &Ctx) {
    ctx.lock_all().await;
    say(ctx, Gym2_S1_Erreur).await;
    if !defeated(ctx, TRAINER_SIL_ROBOT_BINAIRE).await {
        let (x, y) = ctx.player_pos().await;
        ctx.set_object_template_pos(LOCALID_GYM2_ROBOT, x + 3, y)
            .await;
        show(ctx, LOCALID_GYM2_ROBOT, FLAG_SIL_HIDE_GYM2_ROBOT).await;
        move_wait(ctx, LOCALID_GYM2_ROBOT, ROBOT_IN).await;
        say(ctx, Gym2_Robot_Intro).await;
        battle_trainer(ctx, TRAINER_SIL_ROBOT_BINAIRE, Gym2_Robot_Defeat).await;
        hide(ctx, LOCALID_GYM2_ROBOT).await;
    }
    warp(ctx, MAP_GRIS_ECHEC_GYM, 7, 38).await;
}

/// A wrong tile.
pub async fn GrisEchec_Gym_EventScript_S1Erreur(ctx: Ctx) {
    s1_error(&ctx).await;
}

/// A right tile: one more bit, only on the row the count expects (37 minus the count), so the
/// same tile twice does not count twice.
pub async fn GrisEchec_Gym_EventScript_S1Bit(ctx: Ctx) {
    let n = ctx.var(VAR_TEMP_2).await;
    let (_, y) = ctx.player_pos().await;
    if i32::from(y) == 37 - i32::from(n) {
        ctx.set_var(VAR_TEMP_2, n + 1).await;
    }
}

/// The doorway (7, 29): the key must be 8 bits. Coming back down (facing down: from Salle 2,
/// or from the Chambre Sourde, whose warp cleared the count) is no error.
pub async fn GrisEchec_Gym_EventScript_S1Fin(ctx: Ctx) {
    if ctx.player_facing().await == Dir::Down {
        return;
    }
    if ctx.var(VAR_TEMP_2).await < 8 {
        s1_error(&ctx).await;
    }
}

// ---- Salle 2, the invisible maze (y 22–29) ----

/// The maze's path, from the terminal's entry (7, 28) to the exit (7, 22) (the source's local
/// path plus (3, 22)).
const MAZE_PATH: &[(i16, i16)] = &[
    (7, 28),
    (7, 27),
    (8, 27),
    (9, 27),
    (10, 27),
    (10, 26),
    (10, 25),
    (9, 25),
    (8, 25),
    (7, 25),
    (6, 25),
    (5, 25),
    (4, 25),
    (4, 24),
    (4, 23),
    (5, 23),
    (6, 23),
    (7, 23),
    (7, 22),
];
/// The maze floor (MossdeepGym 513) and the lit path: the binary tiles' green panel (612).
const METATILE_SOL_NORMAL: u16 = 513;
const METATILE_SOL_ALLUME: u16 = 612;

/// Gym2_S2's terminal (6, 28): on yes, the path lights up for 3 seconds (180 frames), then
/// goes out. Reusable at will.
pub async fn GrisEchec_Gym_EventScript_Terminal(ctx: Ctx) {
    ctx.lock().await;
    if ask(&ctx, Gym2_S2_Terminal).await {
        for &(x, y) in MAZE_PATH {
            ctx.set_metatile(x, y, METATILE_SOL_ALLUME, false).await;
        }
        ctx.redraw_map().await;
        ctx.delay(180).await;
        for &(x, y) in MAZE_PATH {
            ctx.set_metatile(x, y, METATILE_SOL_NORMAL, false).await;
        }
        ctx.redraw_map().await;
        say(&ctx, Gym2_S2_Sature).await;
    }
    ctx.release().await;
}

// ---- Salle 3, the chessboard (y 12–21) ----

/// A living chess piece: its object, trainer and lines.
#[derive(Clone, Copy)]
struct Piece {
    obj: u8,
    trainer: TrainerId,
    intro: TextId,
    defeat: TextId,
}

const PION_1: Piece = Piece {
    obj: LOCALID_GYM2_PION_1,
    trainer: TRAINER_SIL_ECHEC_PION_1,
    intro: Gym2_Pion1_Intro,
    defeat: Gym2_Pion1_Defeat,
};
const PION_2: Piece = Piece {
    obj: LOCALID_GYM2_PION_2,
    trainer: TRAINER_SIL_ECHEC_PION_2,
    intro: Gym2_Pion2_Intro,
    defeat: Gym2_Pion2_Defeat,
};
const CAVALIER: Piece = Piece {
    obj: LOCALID_GYM2_CAVALIER,
    trainer: TRAINER_SIL_ECHEC_CAVALIER,
    intro: Gym2_Cavalier_Intro,
    defeat: Gym2_Cavalier_Defeat,
};
const FOU: Piece = Piece {
    obj: LOCALID_GYM2_FOU,
    trainer: TRAINER_SIL_ECHEC_FOU,
    intro: Gym2_Fou_Intro,
    defeat: Gym2_Fou_Defeat,
};
const TOUR: Piece = Piece {
    obj: LOCALID_GYM2_TOUR,
    trainer: TRAINER_SIL_ECHEC_TOUR,
    intro: Gym2_Tour_Intro,
    defeat: Gym2_Tour_Defeat,
};

/// The piece turns to the player with a "!" (the source's §4 script; the engine has emotes).
const PIECE_SEES: &[Step] = &[Step::FacePlayer, Step::Emote(Emote::Exclamation)];

/// A piece's battle (seen on a watched square, or talked to).
async fn piece_battle(ctx: &Ctx, p: Piece) {
    ctx.lock_all().await;
    move_wait(ctx, p.obj, PIECE_SEES).await;
    say(ctx, p.intro).await;
    battle_trainer(ctx, p.trainer, p.defeat).await;
    ctx.release_all().await;
}

/// A watched square: the first of `watchers` not beaten battles (the source's priority order);
/// a beaten piece watches nothing. A square next to the King then starts Gym2_Sc10.
async fn watched(ctx: Ctx, watchers: &[Piece], king: bool) {
    for &p in watchers {
        if !defeated(&ctx, p.trainer).await {
            piece_battle(&ctx, p).await;
            return;
        }
    }
    if king {
        yohan(ctx, false).await;
    }
}

/// A piece talked to: its battle while unbeaten, then its defeat line.
async fn piece_talk(ctx: Ctx, p: Piece) {
    if defeated(&ctx, p.trainer).await {
        msgbox(&ctx, p.defeat, MsgBox::Npc).await;
    } else {
        piece_battle(&ctx, p).await;
    }
}

pub async fn GrisEchec_Gym_EventScript_VueP1(ctx: Ctx) {
    watched(ctx, &[PION_1], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueP2(ctx: Ctx) {
    watched(ctx, &[PION_2], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueC(ctx: Ctx) {
    watched(ctx, &[CAVALIER], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueF(ctx: Ctx) {
    watched(ctx, &[FOU], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueT(ctx: Ctx) {
    watched(ctx, &[TOUR], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueCF(ctx: Ctx) {
    watched(ctx, &[CAVALIER, FOU], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueFT(ctx: Ctx) {
    watched(ctx, &[FOU, TOUR], false).await;
}

pub async fn GrisEchec_Gym_EventScript_VueRoi(ctx: Ctx) {
    watched(ctx, &[], true).await;
}

pub async fn GrisEchec_Gym_EventScript_VueCFRoi(ctx: Ctx) {
    watched(ctx, &[CAVALIER, FOU], true).await;
}

pub async fn GrisEchec_Gym_EventScript_Pion1(ctx: Ctx) {
    piece_talk(ctx, PION_1).await;
}

pub async fn GrisEchec_Gym_EventScript_Pion2(ctx: Ctx) {
    piece_talk(ctx, PION_2).await;
}

pub async fn GrisEchec_Gym_EventScript_Cavalier(ctx: Ctx) {
    piece_talk(ctx, CAVALIER).await;
}

pub async fn GrisEchec_Gym_EventScript_Fou(ctx: Ctx) {
    piece_talk(ctx, FOU).await;
}

pub async fn GrisEchec_Gym_EventScript_Tour(ctx: Ctx) {
    piece_talk(ctx, TOUR).await;
}

/// The rules screen (4, 12), at the far end of the board.
pub async fn GrisEchec_Gym_EventScript_EcranEchiquier(ctx: Ctx) {
    msgbox(&ctx, Gym2_Echiquier_Ecran, MsgBox::Sign).await;
}

// ---- Yohan ----

const YOHAN_SEES: &[Step] = &[Step::FacePlayer];

/// Gym2_Sc10, Yohan, who only speaks through screens: a square next to the King (d7, e7, f7,
/// d8, f8) once its watchers are beaten, or talking to him, while `FLAG_SIL_BADGE_02` is clear.
/// The battle, the Badge Échiquier, the TM Iron Tail (`FLAG_SIL_ITEM_YOHAN_TM`), the Champ
/// Magnétique module's speech; sets `VAR_SIL_STORY` 26, then Gym2_Sc11 and Sc12. After the
/// badge, talked to: his defeat screen (the source gives him no other line; a question for the
/// author); the squares then do nothing.
pub async fn GrisEchec_Gym_Sc10(ctx: Ctx) {
    yohan(ctx, true).await;
}

/// [`GrisEchec_Gym_Sc10`], talked to (`talked`) or from a square next to the King.
async fn yohan(ctx: Ctx, talked: bool) {
    if ctx.flag(FLAG_SIL_BADGE_02).await {
        if talked {
            msgbox(&ctx, Gym2_Sc10_Defeat, MsgBox::Sign).await;
        }
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_GYM2_YOHAN, YOHAN_SEES).await;
    say(&ctx, Gym2_Sc10_T1).await;
    say(&ctx, Gym2_Sc10_T2).await;
    // fallback: Mega Evolution (SIL3, plan §3.4: no form change is built yet). The source's own
    // fallback (§9): Alakazam battles unevolved, a normal battle, and its Gym2_Sc10_Mega screen
    // is not shown.
    fallback(&ctx, "mega_evolution").await;
    battle_trainer(&ctx, TRAINER_SIL_YOHAN_1, Gym2_Sc10_Defeat).await;
    say(&ctx, Gym2_Sc10_T3).await;
    ctx.set_flag(FLAG_SIL_BADGE_02, true).await;
    say(&ctx, Gym2_Sc10_T4).await;
    // T5 announces the TM.
    gift(
        &ctx,
        FLAG_SIL_ITEM_YOHAN_TM,
        ITEM_TM23,
        1,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Gym2_Sc10_T5).await;
    say(&ctx, Gym2_Sc10_T6).await;
    set_story(&ctx, 26).await;
    gym2_sc11(&ctx).await;
    ctx.release_all().await;
}

/// Gwada and Wattouat up the e column from under e1: Gwada to e7 (7, 14), or e6 when the player
/// stands on e7 (the source's `Walk(Up) x7` would end on the player); Wattouat one cell behind.
const GWADA_UP_7: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    Step::FacePlayer,
];
const GWADA_UP_6: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    Step::FacePlayer,
];

/// Gym2_Sc11, Gwada installs the module and Wattouat evolves (chained after Sc10).
async fn gym2_sc11(ctx: &Ctx) {
    let on_e7 = ctx.player_pos().await == (7, 14);
    let (gwada_path, lainergie_at) = if on_e7 {
        (GWADA_UP_6, (7, 16))
    } else {
        (GWADA_UP_7, (7, 15))
    };
    show(ctx, LOCALID_GYM2_GWADA, FLAG_SIL_HIDE_GYM2_GWADA).await;
    show(ctx, LOCALID_GYM2_WATTOUAT, FLAG_SIL_HIDE_GYM2_WATTOUAT).await;
    // fallback: the follower Pokémon (§9): two objects moved together; Wattouat starts one cell
    // behind Gwada and takes the same steps.
    fallback(ctx, "follower").await;
    ctx.move_obj(LOCALID_GYM2_GWADA, gwada_path).await;
    move_wait(ctx, LOCALID_GYM2_WATTOUAT, gwada_path).await;
    ctx.wait_move(LOCALID_GYM2_GWADA).await;
    say(ctx, Gym2_Sc11_T1).await;
    say(ctx, Gym2_Sc11_T2).await;
    say(ctx, Gym2_Sc11_T3).await;
    ctx.set_flag(FLAG_SIL_MODULE_FORCE, true).await;
    say(ctx, Gym2_Sc11_T4).await;
    // fallback: screen_flash(WHITE) (`Request::ScreenFlash`): a fade out and in.
    screen_flash(ctx).await;
    // fallback: play_se (audio, M12): SE_THUNDER is the decomp's.
    play_se(ctx, "SE_THUNDER").await;
    say(ctx, Gym2_Sc11_T5).await;
    // fallback: play_evolution(SPECIES_MAREEP, SPECIES_FLAAFFY) on an NPC (M9's evolution
    // scene, `Request::Evolve`, is for party mons): the source's own fallback, a fade, Wattouat
    // swapped for Lainergie on its cell, the fade back, and the message (T6).
    fallback(ctx, "play_evolution").await;
    ctx.fade(Fade::ToBlack).await;
    hide(ctx, LOCALID_GYM2_WATTOUAT).await;
    let (x, y) = lainergie_at;
    ctx.set_object_template_pos(LOCALID_GYM2_LAINERGIE, x, y)
        .await;
    show(ctx, LOCALID_GYM2_LAINERGIE, FLAG_SIL_HIDE_GYM2_LAINERGIE).await;
    ctx.fade(Fade::FromBlack).await;
    say(ctx, Gym2_Sc11_T6).await;
    ctx.set_flag(FLAG_SIL_GWADA_LAINERGIE, true).await;
    say(ctx, Gym2_Sc11_T7).await;
    ctx.set_var(VAR_SIL_SYNC, 20).await;
    // fallback: show_sync(20) (the Mallette's sync gauge, a SIL2 Mallette panel): T8 says it.
    show_sync(ctx, 20).await;
    say(ctx, Gym2_Sc11_T8).await;
    gym2_sc12(ctx).await;
}

/// Gym2_Sc12, LOG_02 "FIDÉLITÉ" (chained after Sc11): the Mallette hacks the arena's server,
/// Gwada reads it, Julien's short call. Sets `FLAG_SIL_LOG_02` and `VAR_SIL_STORY` 27; Gwada and
/// Lainergie go back outside (hidden here; Gris-Échec shows them at 27).
async fn gym2_sc12(ctx: &Ctx) {
    say(ctx, Gym2_Sc12_T1).await;
    say(ctx, LOG_02_Texte).await;
    ctx.set_flag(FLAG_SIL_LOG_02, true).await;
    say(ctx, Gym2_Sc12_T2).await;
    say(ctx, Gym2_Sc12_T3).await;
    // fallback: mallette_call / end_call (the call window, plan §1.1 P1): plain messages.
    mallette_call(ctx, "JULIEN").await;
    say(ctx, Gym2_Sc12_T4).await;
    end_call(ctx).await;
    say(ctx, Gym2_Sc12_T5).await;
    set_story(ctx, 27).await;
    ctx.fade(Fade::ToBlack).await;
    hide(ctx, LOCALID_GYM2_GWADA).await;
    hide(ctx, LOCALID_GYM2_LAINERGIE).await;
    ctx.fade(Fade::FromBlack).await;
}

// ---- the Chambre Sourde (`GrisEchec_Gym_Sourde`) ----

/// The MÉLOMANE, whose team knows only sound moves.
pub async fn GrisEchec_Gym_Sourde_EventScript_Melomane(ctx: Ctx) {
    let beaten = trainer(
        &ctx,
        LOCALID_GYM2_MELOMANE,
        TRAINER_SIL_MELOMANE_SOURDE,
        Gym2_Melomane_Intro,
        Gym2_Melomane_Defeat,
    )
    .await;
    // fallback: the deaf room's rule, sound moves fail for both sides (`sSoundMovesTable`,
    // §9): no battle rule yet (SIL3's battle-rule table). The source's own fallback: the
    // Mélomane keeps his sound team, and the texts make the joke.
    fallback(&ctx, "sound rule").await;
    if beaten {
        msgbox(&ctx, Gym2_Melomane_Defeat, MsgBox::Autoclose).await;
    }
}

/// The chest at the back: the Soothe Bell, once (`FLAG_SIL_ITEM_GYM2_SOOTHE_BELL`, the item
/// ball's flag); its line announces it.
pub async fn GrisEchec_Gym_Sourde_EventScript_Coffre(ctx: Ctx) {
    ctx.lock().await;
    if gift(
        &ctx,
        FLAG_SIL_ITEM_GYM2_SOOTHE_BELL,
        ITEM_SOOTHE_BELL,
        1,
        Obtain::Quiet,
    )
    .await
    {
        hide(&ctx, LOCALID_GYM2_COFFRE).await;
        say(&ctx, Gym2_Sourde_Coffre).await;
    }
    ctx.release().await;
}

pub async fn GrisEchec_Gym_Sourde_EventScript_Panneau(ctx: Ctx) {
    msgbox(&ctx, Gym2_Sourde_Panneau, MsgBox::Sign).await;
}
