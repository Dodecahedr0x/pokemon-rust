//! Chapter 1, the village (`BoisEther`): `contenu-jeu/chapitres/01-bois-ether.md` §2 (signs,
//! ambient NPCs, items) and §3 Bois_Ether_Sc01 to Sc04 and Sc06 to Sc08. `VAR_SIL_STORY` runs 9
//! (in sight of the village, from Route S1) to 17 (the Secteur Bêta barrier open: Chapter 2 takes
//! over at Route S2). The gym is `ch01_gym.rs`, the shops and the Center `ch01_buildings.rs`.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BoisEther"];

/// `METATILE_BARRIERE_OUVERTE`: the open barrier, General's grass (1), walkable. The barrier
/// (General's fence, 6) lies in Route S2's connection strip, which stays primary-only.
pub const METATILE_BARRIERE_OUVERTE: u16 = 1;

/// The Secteur Bêta barrier, two cells between the trees (33, 12) and (33, 15).
const BARRIER: [(i16, i16); 2] = [(33, 13), (33, 14)];

/// The brambles in front of the secret grove.
const RONCES: (i16, i16) = (8, 6);

/// The meeting point's front cell, in front of Adrien (17, 12).
const RDV: (i16, i16) = (17, 13);

// ---- map scripts ----

/// The cut brambles (`FLAG_SIL_BOIS_ETHER_RONCES_CUT`) and, from `VAR_SIL_STORY` 17, the open
/// barrier: the map loads from its layout every time.
pub async fn BoisEther_OnLoad(ctx: Ctx) {
    repaint_if(
        &ctx,
        FLAG_SIL_BOIS_ETHER_RONCES_CUT,
        RONCES,
        METATILE_RONCES_COUPEES,
    )
    .await;
    if story(&ctx).await >= 17 {
        for (x, y) in BARRIER {
            ctx.set_metatile(x, y, METATILE_BARRIERE_OUVERTE, false).await;
        }
    }
}

/// Object visibility by story step (§2 "PNJ d'ambiance"): the technician until the alert
/// (`VAR_SIL_STORY` 16), the barrier guard until it opens (17), Adrien at the meeting point at 10
/// and 11 only; Sc06's five actors only inside the scene. During the alert, the red sky.
pub async fn BoisEther_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    ctx.set_flag(FLAG_SIL_HIDE_BOIS_ETHER_TECHNICIEN, s >= 16)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_BOIS_ETHER_GARDE, s >= 17).await;
    ctx.set_flag(FLAG_SIL_HIDE_BOIS_ETHER_ADRIEN, !(10..=11).contains(&s))
        .await;
    for f in [
        FLAG_SIL_HIDE_BOIS_ETHER_GOGO,
        FLAG_SIL_HIDE_BOIS_ETHER_ROMAIN,
        FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_1,
        FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_2,
        FLAG_SIL_HIDE_BOIS_ETHER_MATHIEU,
    ] {
        ctx.set_flag(f, true).await;
    }
    if ctx.flag(FLAG_SIL_BOIS_ETHER_RED_SKY).await {
        // set_sky_alert(RED) on every load while the alert lasts; fallback: MUS_ENCOUNTER_MAGMA
        // (audio, M12): the map's own music.
        set_palette(&ctx, SKY_ALERT_RED, 0, false).await;
    }
}

// ---- scenes ----

/// Bois_Ether_Sc01, arrival: the south entrance row (x 12–15, y 24) at `VAR_SIL_STORY` 9.
/// Sets 10.
pub async fn BoisEther_Sc01(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Bois_Ether_Sc01_T1).await;
    set_story(&ctx, 10).await;
    ctx.release().await;
}

/// Up to 15 steps right or left along the meeting row y 13 (x 2–31).
static RIGHTS: [Step; 15] = [walk(Dir::Right); 15];
static LEFTS: [Step; 15] = [walk(Dir::Left); 15];
const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// Bois_Ether_Sc02, the meeting: Adrien, and nobody else. The whole row y 13 at
/// `VAR_SIL_STORY` 10 (so the clearing cannot be walked around): the player first walks along it
/// to the meeting point (17, 13), facing Adrien. Sets 11, then Sc03.
pub async fn BoisEther_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    let (x, _) = ctx.player_pos().await;
    let to_rdv: &'static [Step] = if x < RDV.0 {
        &RIGHTS[..(RDV.0 - x).min(15) as usize]
    } else {
        &LEFTS[..(x - RDV.0).min(15) as usize]
    };
    if !to_rdv.is_empty() {
        move_wait(&ctx, PLAYER, to_rdv).await;
    }
    move_wait(&ctx, PLAYER, FACE_UP).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Bois_Ether_Sc02_T1).await;
    ctx.delay(60).await;
    say(&ctx, Bois_Ether_Sc02_T2).await;
    say(&ctx, Bois_Ether_Sc02_T3).await;
    set_story(&ctx, 11).await;
    BoisEther_Sc03(ctx.clone()).await;
}

/// Adrien's way out, re-derived for the built village (the source's `Walk(Right) x6, Walk(Down)
/// x2` from (17, 12) crosses the guardian at (19, 12)): up in front of the oak, east along y 11,
/// down to (23, 13).
const ADRIEN_OUT: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Down),
    walk(Dir::Down),
];

/// Bois_Ether_Sc03, Julien's call (chained after Sc02, `VAR_SIL_STORY` 11): the signal moves to
/// Gris-Échec; Adrien leaves along the coast. Sets 12.
pub async fn BoisEther_Sc03(ctx: Ctx) {
    if story(&ctx).await != 11 {
        // Chained inside Sc02's `lock_all`: never leave the field locked.
        ctx.release_all().await;
        return;
    }
    // fallback: play_se (audio, M12): a no-op that reports; SE_RING is the decomp's
    // SE_POKENAV_CALL.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    // fallback: mallette_call (the call window with a portrait, plan §1.1 P1): plain messages.
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Bois_Ether_Sc03_T1).await;
    // fallback: end_call (as mallette_call): nothing to close.
    end_call(&ctx).await;
    say(&ctx, Bois_Ether_Sc03_T2).await;
    say(&ctx, Bois_Ether_Sc03_T3).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_ADRIEN, ADRIEN_OUT).await;
    hide(&ctx, LOCALID_BOIS_ETHER_ADRIEN).await;
    set_story(&ctx, 12).await;
    ctx.release_all().await;
}

/// Bois_Ether_Sc04, the oak's guardian (obj 8): the Miracle Seed, once
/// (`FLAG_SIL_ITEM_BOIS_ETHER_MIRACLE_SEED`; T1 says it is obtained). Afterwards (not in the
/// source), his advice, T2.
pub async fn BoisEther_Sc04(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_ITEM_BOIS_ETHER_MIRACLE_SEED).await {
        say(&ctx, Bois_Ether_Sc04_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_BOIS_ETHER_MIRACLE_SEED,
            ITEM_MIRACLE_SEED,
            1,
            Obtain::Quiet,
        )
        .await;
    }
    say(&ctx, Bois_Ether_Sc04_T2).await;
    ctx.release().await;
}

/// Sc06's walks, re-derived for the built village (the actors start at (15, 10), (19, 10),
/// (15, 11), (19, 11) and (22, 11); the player stands on row y 6, x 13–20).
const UP_3: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up)];
const UP_2: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];
const MATHIEU_IN: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    Step::FacePlayer,
];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];
/// Gogo flees down x 15; Romain down to y 11, then east of the guardian (19, 12), to (22, 13).
const GOGO_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
];
const ROMAIN_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Down),
    walk(Dir::Down),
];
const MATHIEU_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
];

/// Bois_Ether_Sc06, the red sky: Gogo & Romain, and the Grand Praugect. The row every step off
/// the gym's deck crosses (x 13–21, y 6) at `VAR_SIL_STORY` 15 (`FLAG_SIL_BADGE_01` set and
/// `FLAG_SIL_BOIS_ETHER_AMBUSH` not). Sets the alert, Mathieu's Super Potions, the ambush and the
/// Labo hint, and 16.
pub async fn BoisEther_Sc06(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_BADGE_01).await || ctx.flag(FLAG_SIL_BOIS_ETHER_AMBUSH).await {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, PLAYER, FACE_DOWN).await;
    // set_sky_alert(RED): the sky turns red; the flag keeps it on the next loads.
    set_palette(&ctx, SKY_ALERT_RED, PALETTE_FRAMES, false).await;
    ctx.set_flag(FLAG_SIL_BOIS_ETHER_RED_SKY, true).await;
    say(&ctx, Bois_Ether_Sc06_T1).await;
    show(&ctx, LOCALID_BOIS_ETHER_GOGO, FLAG_SIL_HIDE_BOIS_ETHER_GOGO).await;
    show(&ctx, LOCALID_BOIS_ETHER_ROMAIN, FLAG_SIL_HIDE_BOIS_ETHER_ROMAIN).await;
    show(&ctx, LOCALID_BOIS_ETHER_SBIRE_1, FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_1).await;
    show(&ctx, LOCALID_BOIS_ETHER_SBIRE_2, FLAG_SIL_HIDE_BOIS_ETHER_SBIRE_2).await;
    ctx.move_obj(LOCALID_BOIS_ETHER_GOGO, UP_3).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_ROMAIN, UP_3).await;
    ctx.wait_move(LOCALID_BOIS_ETHER_GOGO).await;
    say(&ctx, Bois_Ether_Sc06_T2).await;
    say(&ctx, Bois_Ether_Sc06_T3).await;
    ctx.move_obj(LOCALID_BOIS_ETHER_SBIRE_1, UP_2).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_SBIRE_2, UP_2).await;
    ctx.wait_move(LOCALID_BOIS_ETHER_SBIRE_1).await;
    say(&ctx, Bois_Ether_Sc06_T4).await;
    show(&ctx, LOCALID_BOIS_ETHER_MATHIEU, FLAG_SIL_HIDE_BOIS_ETHER_MATHIEU).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_MATHIEU, MATHIEU_IN).await;
    say(&ctx, Bois_Ether_Sc06_T5).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_MATHIEU, FACE_DOWN).await;
    say(&ctx, Bois_Ether_Sc06_T6).await;
    // fallback: battle_two_trainers (a double battle against two trainers, Milestone 11): the
    // source's own fallback, Gogo then Romain, without a heal between them.
    battle_two_trainers(
        &ctx,
        (TRAINER_SIL_GOGO_1, Bois_Ether_Gogo_Defeat),
        (TRAINER_SIL_ROMAIN_1, Bois_Ether_Romain_Defeat),
    )
    .await;
    // Praugect won off screen.
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_BOIS_ETHER_SBIRE_1).await;
    hide(&ctx, LOCALID_BOIS_ETHER_SBIRE_2).await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Bois_Ether_Sc06_T7).await;
    ctx.move_obj(LOCALID_BOIS_ETHER_GOGO, GOGO_OUT).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_ROMAIN, ROMAIN_OUT).await;
    ctx.wait_move(LOCALID_BOIS_ETHER_GOGO).await;
    hide(&ctx, LOCALID_BOIS_ETHER_GOGO).await;
    hide(&ctx, LOCALID_BOIS_ETHER_ROMAIN).await;
    say(&ctx, Bois_Ether_Sc06_T8).await;
    // T8 says the two Super Potions are obtained.
    gift(
        &ctx,
        FLAG_SIL_ITEM_MATHIEU_CH1,
        ITEM_SUPER_POTION,
        2,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Bois_Ether_Sc06_T9).await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_MATHIEU, MATHIEU_OUT).await;
    hide(&ctx, LOCALID_BOIS_ETHER_MATHIEU).await;
    ctx.set_flag(FLAG_SIL_BOIS_ETHER_AMBUSH, true).await;
    ctx.set_flag(FLAG_SIL_LABO_HINT, true).await;
    set_story(&ctx, 16).await;
    ctx.release_all().await;
}

/// The guard steps aside: up, or down when the player stands above him.
const GARDE_UP: &[Step] = &[walk(Dir::Up), face(Dir::Down)];
const GARDE_DOWN: &[Step] = &[walk(Dir::Down), face(Dir::Up)];

/// Bois_Ether_Sc07, the Secteur Bêta barrier falls: the guard (obj 9) talked to, or the cells in
/// front of the barrier (32, 12–15), at `VAR_SIL_STORY` 16. Benou's call opens the barrier; the
/// alert ends. Sets 17: the chapter's end, Route S2 is open. Before the alert, his line.
pub async fn BoisEther_Sc07(ctx: Ctx) {
    let s = story(&ctx).await;
    if s < 16 {
        msgbox(&ctx, Bois_Ether_Npc_Garde, MsgBox::Npc).await;
        return;
    }
    if s > 16 {
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_BOIS_ETHER_GARDE, FACE_PLAYER).await;
    say(&ctx, Bois_Ether_Sc07_T1).await;
    // fallback: play_se (audio, M12) and mallette_call (the call window): as in Sc03.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    mallette_call(&ctx, "BENOU").await;
    say(&ctx, Bois_Ether_Sc07_T2).await;
    end_call(&ctx).await;
    for (x, y) in BARRIER {
        ctx.set_metatile(x, y, METATILE_BARRIERE_OUVERTE, false).await;
    }
    say(&ctx, Bois_Ether_Sc07_T3).await;
    let aside = if ctx.player_pos().await == (32, 12) {
        GARDE_DOWN
    } else {
        GARDE_UP
    };
    move_wait(&ctx, LOCALID_BOIS_ETHER_GARDE, aside).await;
    // clear_sky_alert(): the map's own palette back; the alert's flag ends (§8: cleared at 17).
    clear_palette(&ctx, PALETTE_FRAMES).await;
    ctx.set_flag(FLAG_SIL_BOIS_ETHER_RED_SKY, false).await;
    set_story(&ctx, 17).await;
    ctx.release_all().await;
}

/// Bois_Ether_Sc08, the secret grove's brambles (8, 6): the generic Lame-Frequence obstacle
/// (`FLAG_SIL_BOIS_ETHER_RONCES_CUT`; [`BoisEther_OnLoad`] keeps them cut).
pub async fn BoisEther_Sc08(ctx: Ctx) {
    cut_obstacle(
        &ctx,
        RONCES,
        METATILE_RONCES_COUPEES,
        FLAG_SIL_BOIS_ETHER_RONCES_CUT,
        Bois_Ether_Sc08_T1,
    )
    .await;
}

// ---- items (§7) ----

/// The X Defend behind the Center (28, 8).
pub async fn BoisEther_EventScript_ItemXDefend(ctx: Ctx) {
    find_item(&ctx, ITEM_X_DEFEND, 1).await;
}

// ---- ambient NPCs (§2; after the alert, `VAR_SIL_STORY` ≥ 16, their second lines) ----

pub async fn BoisEther_EventScript_Jardiniere(ctx: Ctx) {
    let text = if story(&ctx).await >= 16 {
        Bois_Ether_Npc_Jardiniere_2
    } else {
        Bois_Ether_Npc_Jardiniere
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

pub async fn BoisEther_EventScript_Vieux(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Npc_Vieux, MsgBox::Npc).await;
}

pub async fn BoisEther_EventScript_Fillette(ctx: Ctx) {
    let text = if story(&ctx).await >= 16 {
        Bois_Ether_Npc_Fillette_2
    } else {
        Bois_Ether_Npc_Fillette
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

pub async fn BoisEther_EventScript_Marchand(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Npc_Marchand, MsgBox::Npc).await;
}

pub async fn BoisEther_EventScript_Fan(ctx: Ctx) {
    let text = if story(&ctx).await >= 16 {
        Bois_Ether_Npc_Fan_2
    } else {
        Bois_Ether_Npc_Fan
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// The Cocket technician by his cable (hidden from the alert on).
pub async fn BoisEther_EventScript_Technicien(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Npc_Technicien, MsgBox::Npc).await;
}

pub async fn BoisEther_EventScript_Toux(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Npc_Toux, MsgBox::Npc).await;
}

// ---- signs ----

pub async fn BoisEther_EventScript_SignVille(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Sign_Ville, MsgBox::Sign).await;
}

pub async fn BoisEther_EventScript_SignArene(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Sign_Arene, MsgBox::Sign).await;
}

pub async fn BoisEther_EventScript_SignChene(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Sign_Chene, MsgBox::Sign).await;
}

pub async fn BoisEther_EventScript_SignBarriere(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Sign_Barriere, MsgBox::Sign).await;
}

pub async fn BoisEther_EventScript_SignCable(ctx: Ctx) {
    msgbox(&ctx, Bois_Ether_Sign_Cable, MsgBox::Sign).await;
}
