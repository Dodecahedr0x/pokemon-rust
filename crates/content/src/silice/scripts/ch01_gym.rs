//! Chapter 1, the Grass-Hopper gym (`BoisEther_Gym`): `contenu-jeu/chapitres/01-bois-ether.md`
//! §3 Gym_Sc01 and Gym_Sc10 to Sc12, and §4 (the four zones: the spore airlock, the workshop's
//! three-piece valve, the pressure airlock, the greenhouse). `VAR_SIL_STORY` runs 12 (Julien's
//! call) to 15 (`LOG_01` read).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BoisEther_Gym"];

/// `METATILE_PORTE_SAS_OUVERTE`: an open airlock door's lower half, SilBoisEtherGym's
/// `porte_sas_ouverte` (519), walkable; its upper half is [`PORTE_OUVERTE_HAUT`] (518).
pub const METATILE_PORTE_SAS_OUVERTE: u16 = 519;
const PORTE_OUVERTE_HAUT: u16 = 518;
/// A closed airlock door: its top (SilBoisEtherGym's `porte_sas`, 516) and bottom (517) halves,
/// solid. Door D (8, 22) is a lower half.
const PORTE_HAUT: u16 = 516;
const PORTE_BAS: u16 = 517;

/// Zone 1's airlock door D (one cell).
const PORTE_D: (i16, i16) = (8, 22);
/// The doors are two cells tall: zone 2's (the valve's) is (7, 11–12), zone 3's front (7, 8–9).
/// Each is opened and closed as a whole ("SIL2 Ch1 maps as built").
const PORTE_VALVE: i16 = 11;
const PORTE_SERRE: i16 = 8;

/// Opens (walkable floor) or closes the two-cell airlock door whose top is (7, `top`).
async fn airlock(ctx: &Ctx, top: i16, open: bool) {
    let (haut, bas) = if open {
        (PORTE_OUVERTE_HAUT, METATILE_PORTE_SAS_OUVERTE)
    } else {
        (PORTE_HAUT, PORTE_BAS)
    };
    ctx.set_metatile(7, top, haut, !open).await;
    ctx.set_metatile(7, top + 1, bas, !open).await;
}

// ---- map scripts ----

/// Once the valve is in (`FLAG_SIL_GYM1_VALVE`), both inner airlocks are open on every load:
/// otherwise a player loading the map in the greenhouse (a whiteout's return, a later visit)
/// would be shut in. Door D opens again from its panel.
pub async fn BoisEther_Gym_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_GYM1_VALVE).await {
        airlock(&ctx, PORTE_VALVE, true).await;
        airlock(&ctx, PORTE_SERRE, true).await;
    }
}

/// Loïc is in his private greenhouse once beaten, back on his dais from 121 (Chapter 12's
/// rematches).
pub async fn BoisEther_Gym_OnTransition(ctx: Ctx) {
    let badge = ctx.flag(FLAG_SIL_BADGE_01).await;
    let rematch = super::ch12_revanches::ouvertes(&ctx).await;
    ctx.set_flag(FLAG_SIL_HIDE_GYM1_LOIC, badge && !rematch).await;
}

// ---- the hall ----

/// Gym_Sc01, inside the tree: one step in from the mat (6–7, 38) at `VAR_SIL_STORY` 12. Sets 13.
pub async fn BoisEther_Gym_Sc01(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Gym_Sc01_T1).await;
    set_story(&ctx, 13).await;
    ctx.release().await;
}

/// The guide (§4 Hall): his advice and the Fresh Water once (`FLAG_SIL_ITEM_GYM1_FRESH_WATER`,
/// with Gen3's obtain messages: his text offers it, it does not say it is obtained); after the
/// badge, his congratulations.
pub async fn BoisEther_Gym_EventScript_Conseiller(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_BADGE_01).await {
        msgbox(&ctx, Gym_Conseiller_Apres, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Gym_Conseiller).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_GYM1_FRESH_WATER,
        ITEM_FRESH_WATER,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}

// ---- zone 1, the spore airlock ----

/// The shower (7, 30) and the cell above it: clean again (`FLAG_SIL_GYM1_CONTAMINE` cleared).
/// Fires on every step (`VAR_TEMP_1` stays 0); nothing when clean.
pub async fn BoisEther_Gym_EventScript_Douche(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_GYM1_CONTAMINE).await {
        return;
    }
    ctx.lock().await;
    ctx.set_flag(FLAG_SIL_GYM1_CONTAMINE, false).await;
    say(&ctx, Gym_Z1_Douche).await;
    ctx.release().await;
}

/// An active grate (the 20 `v` cells): covered in pollen (`FLAG_SIL_GYM1_CONTAMINE`). Fires on
/// every step; nothing when already contaminated.
pub async fn BoisEther_Gym_EventScript_Spores(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_GYM1_CONTAMINE).await {
        return;
    }
    ctx.lock().await;
    ctx.set_flag(FLAG_SIL_GYM1_CONTAMINE, true).await;
    say(&ctx, Gym_Z1_Spores).await;
    ctx.release().await;
}

/// Airlock door D's panel (8, 22), from below or above: contaminated, back to the shower
/// (clean); else the door opens.
pub async fn BoisEther_Gym_EventScript_PorteSas1(ctx: Ctx) {
    ctx.lock().await;
    if ctx.flag(FLAG_SIL_GYM1_CONTAMINE).await {
        back_to_shower(&ctx).await;
        return;
    }
    ctx.door(PORTE_D.0, PORTE_D.1, true).await;
    ctx.wait_door().await;
    ctx.set_metatile(PORTE_D.0, PORTE_D.1, METATILE_PORTE_SAS_OUVERTE, false)
        .await;
    say(&ctx, Gym_Z1_Porte_Ok).await;
    ctx.release().await;
}

/// Door D's cell (8, 22), once its panel opened it: a contaminated player who steps onto it
/// (back from a grate) is sent to the shower, as by the panel. Fires on every step (`VAR_TEMP_1`
/// stays 0); nothing when clean. Not in the source, whose door is only a panel: without it, an
/// open door let a contaminated player through.
pub async fn BoisEther_Gym_EventScript_PorteSas1Passage(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_GYM1_CONTAMINE).await {
        return;
    }
    ctx.lock().await;
    back_to_shower(&ctx).await;
}

/// The refusal: the loudspeaker, clean again, and the warp to the shower (7, 30). The warp ends
/// the script (the map loads, door D closed again): the flag is cleared first.
async fn back_to_shower(ctx: &Ctx) {
    say(ctx, Gym_Z1_Porte_Refus).await;
    ctx.set_flag(FLAG_SIL_GYM1_CONTAMINE, false).await;
    warp(ctx, MAP_BOIS_ETHER_GYM, 7, 30).await;
}

/// The poster (4, 22).
pub async fn BoisEther_Gym_EventScript_Affiche(ctx: Ctx) {
    msgbox(&ctx, Gym_Z1_Affiche, MsgBox::Sign).await;
}

/// BOTANISTE IRIS (6, 23), facing right, sight 3.
pub async fn BoisEther_Gym_EventScript_Iris(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM1_IRIS,
        TRAINER_SIL_BOTANISTE_IRIS,
        Gym_Iris_Intro,
        Gym_Iris_Defeat,
    )
    .await
    {
        post_battle(&ctx, Gym_Iris_Defeat).await;
    }
}

/// APPRENTI JULES (11, 26), facing left, sight 3.
pub async fn BoisEther_Gym_EventScript_Jules(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM1_JULES,
        TRAINER_SIL_APPRENTI_JULES,
        Gym_Jules_Intro,
        Gym_Jules_Defeat,
    )
    .await
    {
        post_battle(&ctx, Gym_Jules_Defeat).await;
    }
}


// ---- zone 2, the workshop ----

/// APPRENTI MILO (8, 17), facing left, sight 4: beaten, he hands over the airlock seal
/// (`FLAG_SIL_GYM1_JOINT`; his defeat text gives it).
pub async fn BoisEther_Gym_EventScript_Milo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM1_MILO,
        TRAINER_SIL_APPRENTI_MILO,
        Gym_Milo_Intro,
        Gym_Milo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Gym_Milo_Defeat).await;
    }
    valve_piece(&ctx, TRAINER_SIL_APPRENTI_MILO, FLAG_SIL_GYM1_JOINT).await;
}

/// BOTANISTE ROSE (11, 13), facing down, sight 4: beaten, she hands over the pressure gauge
/// (`FLAG_SIL_GYM1_MANO`; her defeat text gives it).
pub async fn BoisEther_Gym_EventScript_Rose(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM1_ROSE,
        TRAINER_SIL_BOTANISTE_ROSE,
        Gym_Rose_Intro,
        Gym_Rose_Defeat,
    )
    .await
    {
        post_battle(&ctx, Gym_Rose_Defeat).await;
    }
    valve_piece(&ctx, TRAINER_SIL_BOTANISTE_ROSE, FLAG_SIL_GYM1_MANO).await;
}

/// The source's `TrainerBattle::continue_script`: once `trainer` is beaten, its valve piece's
/// flag. The pieces are flags, not items (§7). The source's "{PLAYER} obtient le JOINT / le
/// MANOMÈTRE!" has no text id: the defeat text, which hands the piece over, stands in.
async fn valve_piece(ctx: &Ctx, trainer: TrainerId, piece: FlagId) {
    if defeated(ctx, trainer).await && !ctx.flag(piece).await {
        ctx.set_flag(piece, true).await;
    }
}

/// APPRENTI PAUL (11, 18): the valve's three pieces.
pub async fn BoisEther_Gym_EventScript_Paul(ctx: Ctx) {
    msgbox(&ctx, Gym_Z2_Paul, MsgBox::Npc).await;
}

/// The workbench: with the three pieces, assemble the valve (yes/no): `FLAG_SIL_GYM1_VALVE`,
/// and the airlock door (7, 11–12) opens. Without them, the plan on the wall. Once the valve is
/// in, nothing more.
pub async fn BoisEther_Gym_EventScript_Etabli(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_GYM1_VALVE).await {
        return;
    }
    let pieces = ctx.flag(FLAG_SIL_GYM1_JOINT).await
        && ctx.flag(FLAG_SIL_GYM1_MANO).await
        && ctx.flag(FLAG_SIL_GYM1_FUSIBLE).await;
    if !pieces {
        msgbox(&ctx, Gym_Z2_Etabli_Manque, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if ask(&ctx, Gym_Z2_Etabli_Ok).await {
        ctx.set_flag(FLAG_SIL_GYM1_VALVE, true).await;
        airlock(&ctx, PORTE_VALVE, true).await;
        say(&ctx, Gym_Z2_Valve).await;
    }
    ctx.release().await;
}

/// Crate c1, the blue one (2, 15): its Super Potion, an item ball
/// (`FLAG_SIL_ITEM_GYM1_SUPER_POTION`).
pub async fn BoisEther_Gym_EventScript_CaissePotion(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Gym_Z2_Caisse_Potion).await;
    ctx.release().await;
    find_item(&ctx, ITEM_SUPER_POTION, 1).await;
}

/// Crate c2, the red one (4, 15): the fuse, once (`FLAG_SIL_GYM1_FUSIBLE`). Empty afterwards.
pub async fn BoisEther_Gym_EventScript_CaisseFusible(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_GYM1_FUSIBLE).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Gym_Z2_Caisse_Fusible).await;
    ctx.set_flag(FLAG_SIL_GYM1_FUSIBLE, true).await;
    ctx.release().await;
}

/// Crate c3 (6, 15): the spring.
pub async fn BoisEther_Gym_EventScript_CaisseRessort(ctx: Ctx) {
    msgbox(&ctx, Gym_Z2_Caisse_Ressort, MsgBox::Sign).await;
}

// ---- zone 3, the pressure airlock ----

/// The chamber's centre (7, 10), once per visit (`VAR_TEMP_2`): the door behind closes, the
/// pressure settles, the door ahead opens. Then the door behind opens again (not in the source):
/// the scene plays once per visit, so a closed door behind would shut the player in the
/// greenhouse until the next map load.
pub async fn BoisEther_Gym_EventScript_Sas2(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_2, 1).await;
    ctx.lock_all().await;
    airlock(&ctx, PORTE_VALVE, false).await;
    say(&ctx, Gym_Z3_T1).await;
    ctx.delay(60).await;
    airlock(&ctx, PORTE_SERRE, true).await;
    say(&ctx, Gym_Z3_T2).await;
    airlock(&ctx, PORTE_VALVE, true).await;
    ctx.release_all().await;
}

// ---- zone 4, the greenhouse ----

/// The private greenhouse's door (12, 1).
pub async fn BoisEther_Gym_EventScript_SerrePrivee(ctx: Ctx) {
    msgbox(&ctx, Gym_Serre_Privee, MsgBox::Sign).await;
}

/// Loïc's way to his private greenhouse's door (12, 1), re-derived (the source's `Walk(Up) x2,
/// Walk(Right) x3` from (7, 2) walks into the back wall): round the dais's east pillar (9, 2–3)
/// along y 4, up to (12, 2); or round its west pillar when the player stands east of him (8, 2).
const LOIC_OUT: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
    face(Dir::Up),
];
const LOIC_OUT_WEST: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
    face(Dir::Up),
];

/// Gym_Sc10, Loïc (obj 8 on the dais, until `FLAG_SIL_BADGE_01`): the battle, the Badge Éther,
/// Benou's module (Sc11), the TM, Loïc off to his private greenhouse, then `LOG_01` (Sc12). Sets
/// 14. A loss whites out (the script ends there).
pub async fn BoisEther_Gym_Sc10(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_BADGE_01).await {
        if super::ch12_revanches::ouvertes(&ctx).await {
            super::ch12_revanches::revanche_loic(&ctx).await;
        }
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Gym_Sc10_T1).await;
    battle_trainer(&ctx, TRAINER_SIL_LOIC_1, Gym_Sc10_Defeat).await;
    say(&ctx, Gym_Sc10_T2).await;
    ctx.set_flag(FLAG_SIL_BADGE_01, true).await;
    say(&ctx, Gym_Sc10_T3).await;
    say(&ctx, Gym_Sc10_T4).await;
    // fallback: play_se (audio, M12): a no-op that reports. CLIC.
    play_se(&ctx, "SE_CLICK").await;
    BoisEther_Gym_Sc11(ctx.clone()).await;
    // T5 says the TM is obtained.
    gift(&ctx, FLAG_SIL_ITEM_LOIC_TM, ITEM_TM09, 1, Obtain::Quiet).await;
    say(&ctx, Gym_Sc10_T5).await;
    say(&ctx, Gym_Sc10_T6).await;
    set_story(&ctx, 14).await;
    let out = if ctx.player_pos().await == (8, 2) {
        LOIC_OUT_WEST
    } else {
        LOIC_OUT
    };
    move_wait(&ctx, LOCALID_GYM1_LOIC, out).await;
    hide(&ctx, LOCALID_GYM1_LOIC).await;
    BoisEther_Gym_Sc12(ctx.clone()).await;
    ctx.release().await;
}

/// Gym_Sc11, Benou installs the Lame-Frequence (called from Sc10): `FLAG_SIL_MODULE_COUPE`, and
/// the synchronisation at 10 % (`VAR_SIL_SYNC`).
pub async fn BoisEther_Gym_Sc11(ctx: Ctx) {
    // fallback: play_se, mallette_call, end_call (audio, the call window): as Bois_Ether_Sc03.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    mallette_call(&ctx, "BENOU").await;
    say(&ctx, Gym_Sc11_T1).await;
    end_call(&ctx).await;
    ctx.set_flag(FLAG_SIL_MODULE_COUPE, true).await;
    say(&ctx, Gym_Sc11_T2).await;
    ctx.set_var(VAR_SIL_SYNC, 10).await;
    // fallback: show_sync (the Mallette's gauge screen, SIL2 on M9's kit): T3 gives the value.
    show_sync(&ctx, 10).await;
    say(&ctx, Gym_Sc11_T3).await;
}

/// Gym_Sc12, `LOG_01` "SILENCE", read by Gwada from afar (called from Sc10, `VAR_SIL_STORY` 14).
/// Sets `FLAG_SIL_LOG_01` and 15.
pub async fn BoisEther_Gym_Sc12(ctx: Ctx) {
    if story(&ctx).await != 14 {
        return;
    }
    ctx.lock_all().await;
    ctx.delay(30).await;
    say(&ctx, Gym_Sc12_T1).await;
    play_se(&ctx, "SE_POKENAV_CALL").await;
    mallette_call(&ctx, "GWADA").await;
    say(&ctx, Gym_Sc12_T2).await;
    // The log itself (the log as a message, as the Mallette's RAPPORTS reads it again, once `FLAG_SIL_LOG_01` is set).
    say(&ctx, LOG_01_Texte).await;
    say(&ctx, Gym_Sc12_T3).await;
    end_call(&ctx).await;
    say(&ctx, Gym_Sc12_T4).await;
    ctx.set_flag(FLAG_SIL_LOG_01, true).await;
    set_story(&ctx, 15).await;
    ctx.release_all().await;
}
