//! Chapter 6, the Sentier des Pèlerins (`RouteS06`) and the Bois des Soupirs
//! (`RouteS06_BoisSoupirs`): `contenu-jeu/chapitres/06-manga-square.md` §2 (signs, ambient NPCs,
//! the cracked rocks), §3 Route_S06_Sc01 and Sc02 (the signal moves, the Princesse rescued) and
//! Bois_Soupirs_Sc01 (the Pierre aux Vers), §5 (the climb's trainers and the Poète) and §7 (the
//! items). `VAR_SIL_STORY` runs 55 (Chapter 5's end, off the Pilgrims' Elevator) to 61 (the
//! Princesse saved, the Monks' shortcut open). Manga-Square is `ch06_manga_square.rs`.
//!
//! **Trainer lines.** The source writes no intro or defeat line for Luna, Hoshi, Yumi, Gaston,
//! Kotaro and Armand (§5 gives their teams only), and `trainer()` needs both. Each borrows the
//! closest Ch6 line of its kind (an author question in the plan):
//! - Luna, cosplayeuse: the Dojo's Kimiko cosplayer; Kevin's "pas assez kawaii";
//! - Hoshi, moine: the Bonze; Musashi's "je retourne méditer";
//! - Yumi, mangaka: the old reader's changed ending; Tak's "Fin du chapitre";
//! - Gaston, pèlerin: the Coureur's ten climbs a day; Kenta's "OSU";
//! - Kotaro, ninja: the Shinobi-Gear seller; Kojiro's "J'ai cligné";
//! - Armand, poète (Bois des Soupirs): the young man's poem; Seiji's calligraphy.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["RouteS06", "RouteS06_BoisSoupirs"];

/// The bamboo curtain (23, 26), General 198 (a trunk) in the layout; open, the route's ground
/// (General 1, walkable): `METATILE_BAMBOU_OUVERT` ("SIL3 Ch6 maps as built").
const RIDEAU: (i16, i16) = (23, 26);
const METATILE_BAMBOU_OUVERT: u16 = 1;

// ---- map scripts ----

/// The curtain stays open once Route_S06_Sc02 has opened it (`FLAG_SIL_ROUTE_S06_RACCOURCI`).
pub async fn RouteS06_OnLoad(ctx: Ctx) {
    repaint_if(
        &ctx,
        FLAG_SIL_ROUTE_S06_RACCOURCI,
        RIDEAU,
        METATILE_BAMBOU_OUVERT,
    )
    .await;
}

/// The rescue's actors only at `VAR_SIL_STORY` 60 (Sc02 hides them for good), and at 55: the
/// player comes up the elevator at 55 and Sc01, the on-frame entry, sets 60 after the objects
/// have spawned. The randonneur by the cracked rocks from 61 (§2 "après VAR 61").
pub async fn RouteS06_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    for f in [
        FLAG_SIL_HIDE_ROUTE_S06_ADRIEN,
        FLAG_SIL_HIDE_ROUTE_S06_SBIRE_A,
        FLAG_SIL_HIDE_ROUTE_S06_SBIRE_B,
    ] {
        ctx.set_flag(f, s != 55 && s != 60).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_ROUTE_S06_RANDONNEUR, s < 61)
        .await;
}

// ---- scenes ----

/// Route_S06_Sc01, the signal moves to Manga-Square: the map's on-frame entry at
/// `VAR_SIL_STORY` 55 (Chapter 5's last value, `Owlys_Sc05`), so it plays as the player comes up
/// the Pilgrims' Elevator. Sets 60, so it plays once.
pub async fn RouteS06_Sc01(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Route_S06_Sc01_T1).await;
    set_story(&ctx, 60).await;
    ctx.release_all().await;
}

/// The player's walk from the stairs' top (x 12–15, row 28) to (12, 26), beside Adrien's cell
/// (13, 26) once he steps down (the source's `Walk(Up) x2`, re-derived: Adrien stands at (13, 25)
/// between the grunts (12, 25) and (14, 25)).
const UP_FROM_12: &[Step] = &[walk(Dir::Up), walk(Dir::Up)];
const UP_FROM_13: &[Step] = &[walk(Dir::Up), walk(Dir::Left), walk(Dir::Up)];
const UP_FROM_14: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Up),
];
const UP_FROM_15: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Left),
    walk(Dir::Up),
];
const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const FACE_UP: &[Step] = &[face(Dir::Up)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
/// Adrien steps down beside the player and faces the grunts.
const ADRIEN_BESIDE: &[Step] = &[walk(Dir::Down), face(Dir::Up)];
/// The grunts flee down the stairs: B straight down its column (14), A round the player by
/// x 11 (the stairs are x 12–15 from row 29).
const SBIRE_A_OUT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Right),
    walk(Dir::Down),
    walk(Dir::Down),
];
const SBIRE_B_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
];
/// Adrien east along row 26 to the curtain (`Walk(Right) x3, Face(Right)`), then through it.
const ADRIEN_TO_CURTAIN: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    face(Dir::Right),
];
const ADRIEN_THROUGH: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];

/// Route_S06_Sc02, the Princesse rescued: the plateau's stairs top (12–15, 28) at
/// `VAR_SIL_STORY` 60. Adrien and the two grunts, the multi battle (its fallback: two single
/// battles, Adrien's onigiri heal between them), the curtain opened. Sets 61.
pub async fn RouteS06_Sc02(ctx: Ctx) {
    if story(&ctx).await != 60 {
        return;
    }
    ctx.lock_all().await;
    // fallback: play_music(MUS_ENCOUNTER_AQUA) (audio, M12): the map's music goes on.
    fallback(&ctx, "play_music").await;
    say(&ctx, Route_S06_Sc02_T1).await;
    let (x, _) = ctx.player_pos().await;
    let path = match x {
        12 => UP_FROM_12,
        13 => UP_FROM_13,
        14 => UP_FROM_14,
        _ => UP_FROM_15,
    };
    move_wait(&ctx, PLAYER, path).await;
    move_wait(&ctx, PLAYER, FACE_UP).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_ADRIEN, FACE_DOWN).await;
    say(&ctx, Route_S06_Sc02_T2).await;
    say(&ctx, Route_S06_Sc02_T3).await;
    say(&ctx, Route_S06_Sc02_T4).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_ADRIEN, ADRIEN_BESIDE).await;
    say(&ctx, Route_S06_Sc02_T5).await;
    // fallback: battle_multi(SBIRE_S06_1, SBIRE_S06_2, partner ADRIEN_1) (M11's multi): the
    // source's own fallback, two single battles, Adrien's heal (T5b) between them.
    fallback(&ctx, "battle_multi").await;
    battle_trainer(&ctx, TRAINER_SIL_SBIRE_S06_1, Route_S06_Sc02_D1).await;
    say(&ctx, Route_S06_Sc02_T5b).await;
    ctx.heal_party().await;
    battle_trainer(&ctx, TRAINER_SIL_SBIRE_S06_2, Route_S06_Sc02_D2).await;
    say(&ctx, Route_S06_Sc02_T6).await;
    ctx.move_obj(LOCALID_ROUTE_S06_SBIRE_A, SBIRE_A_OUT).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_SBIRE_B, SBIRE_B_OUT).await;
    ctx.wait_move(LOCALID_ROUTE_S06_SBIRE_A).await;
    hide(&ctx, LOCALID_ROUTE_S06_SBIRE_A).await;
    hide(&ctx, LOCALID_ROUTE_S06_SBIRE_B).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Route_S06_Sc02_T7).await;
    say(&ctx, Route_S06_Sc02_T8).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_ADRIEN, ADRIEN_TO_CURTAIN).await;
    ctx.set_metatile(RIDEAU.0, RIDEAU.1, METATILE_BAMBOU_OUVERT, false)
        .await;
    ctx.redraw_map().await;
    say(&ctx, Route_S06_Sc02_T9).await;
    move_wait(&ctx, LOCALID_ROUTE_S06_ADRIEN, ADRIEN_THROUGH).await;
    // `hide` sets FLAG_SIL_HIDE_ROUTE_S06_ADRIEN.
    hide(&ctx, LOCALID_ROUTE_S06_ADRIEN).await;
    ctx.set_flag(FLAG_SIL_ROUTE_S06_RACCOURCI, true).await;
    set_story(&ctx, 61).await;
    ctx.release_all().await;
}

// ---- the cracked rocks R1–R3 ----

/// A cracked rock (R1 (5, 57), R2 (22, 41), R3 (20, 19)): without the Scanner de Faille
/// (`FLAG_SIL_MODULE_ECLATE_ROC`, Manga_Square_Sc02), the rock sign's line; with it, the rock
/// breaks (it comes back on the next load, as Emerald's).
///
/// fallback: the Rock Smash field move (its field effect and its `rock_smash` encounter table,
/// §6; M10): the rock is removed with a sound, and no wild Pokémon jumps out. Reported.
pub async fn RouteS06_EventScript_Rocher(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_MODULE_ECLATE_ROC).await {
        msgbox(&ctx, Route_S06_Sign_Rocher, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    fallback(&ctx, "rock_smash").await;
    play_se(&ctx, "SE_M_ROCK_THROW").await;
    let obj = ctx.var(VAR_LAST_TALKED).await;
    ctx.remove_object(obj as u8).await;
    ctx.release().await;
}

// ---- signs and NPCs ----

pub async fn RouteS06_EventScript_SignEntree(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Sign_Entree, MsgBox::Sign).await;
}

pub async fn RouteS06_EventScript_SignEncens(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Sign_Encens, MsgBox::Sign).await;
}

pub async fn RouteS06_EventScript_SignRocher(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Sign_Rocher, MsgBox::Sign).await;
}

/// The Dragon des Vœux's altar: one of the gym quiz's sources (Q3, "il dort au-dessus du ciel";
/// `FLAG_SIL_GYM06_LU_Q3`, the quiz's `multichoice` fallback in `ch06_gym.rs`).
pub async fn RouteS06_EventScript_SignAutel(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Sign_Autel, MsgBox::Sign).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q3, true).await;
}

pub async fn RouteS06_EventScript_Pelerine(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Npc_Pelerine, MsgBox::Npc).await;
}

pub async fn RouteS06_EventScript_Coureur(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Npc_Coureur, MsgBox::Npc).await;
}

pub async fn RouteS06_EventScript_Bonze(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Npc_Bonze, MsgBox::Npc).await;
}

/// The randonneur by R2, shown from `VAR_SIL_STORY` 61.
pub async fn RouteS06_EventScript_Rochers(ctx: Ctx) {
    msgbox(&ctx, Route_S06_Npc_Rochers, MsgBox::Npc).await;
}

// ---- trainers (§5; sight from the map's trainer objects) ----

/// COSPLAYEUSE LUNA (6, 9), in the Lacets hauts, facing east.
pub async fn RouteS06_EventScript_Luna(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S06_LUNA,
        TRAINER_SIL_COSPLAYEUSE_LUNA,
        Dojo_Npc_Cosplay_1,
        Manga_Square_Dojo_Sc01_D1,
    )
    .await
    {
        post_battle(&ctx, Manga_Square_Dojo_Sc01_D1).await;
    }
}

/// MOINE HOSHI (20, 13), in the Lacets hauts, facing west.
pub async fn RouteS06_EventScript_Hoshi(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S06_HOSHI,
        TRAINER_SIL_MOINE_HOSHI,
        Route_S06_Npc_Bonze,
        Gym_Sc01_D_MUSASHI,
    )
    .await
    {
        post_battle(&ctx, Gym_Sc01_D_MUSASHI).await;
    }
}

/// MANGAKA YUMI (8, 36), in the middle zone, facing south.
pub async fn RouteS06_EventScript_Yumi(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S06_YUMI,
        TRAINER_SIL_MANGAKA_YUMI,
        Manga_Square_Npc_Puriste,
        Gym_Sc03_D,
    )
    .await
    {
        post_battle(&ctx, Gym_Sc03_D).await;
    }
}

/// PÈLERIN GASTON (12, 49), in the Lacets bas, facing west.
pub async fn RouteS06_EventScript_Gaston(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S06_GASTON,
        TRAINER_SIL_PELERIN_GASTON,
        Route_S06_Npc_Coureur,
        Dojo_Kenta_Defaite,
    )
    .await
    {
        post_battle(&ctx, Dojo_Kenta_Defaite).await;
    }
}

/// NINJA KOTARO (20, 57), in the Lacets bas, facing west.
pub async fn RouteS06_EventScript_Kotaro(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ROUTE_S06_KOTARO,
        TRAINER_SIL_NINJA_KOTARO,
        Dojo_ShinobiGear,
        Gym_Sc01_D_KOJIRO,
    )
    .await
    {
        post_battle(&ctx, Gym_Sc01_D_KOJIRO).await;
    }
}

// ---- items (§7) ----

pub async fn RouteS06_EventScript_ItemHyperPotion(ctx: Ctx) {
    find_item(&ctx, ITEM_HYPER_POTION, 1).await;
}

pub async fn RouteS06_EventScript_ItemUltraBall(ctx: Ctx) {
    find_item(&ctx, ITEM_ULTRA_BALL, 1).await;
}

/// Behind R1.
pub async fn RouteS06_EventScript_ItemPpUp(ctx: Ctx) {
    find_item(&ctx, ITEM_PP_UP, 1).await;
}

/// Behind R2.
pub async fn RouteS06_EventScript_ItemMaxEther(ctx: Ctx) {
    find_item(&ctx, ITEM_MAX_ETHER, 1).await;
}

/// Behind R3, by the altar.
pub async fn RouteS06_EventScript_ItemDragonScale(ctx: Ctx) {
    find_item(&ctx, ITEM_DRAGON_SCALE, 1).await;
}

/// The Lax Incense behind the third burner (17, 24).
pub async fn RouteS06_EventScript_HiddenEncens(ctx: Ctx) {
    // fallback: hidden items (the engine ignores `BgKind::HiddenItem`; the SIL1 Task F
    // fallback): a sign on the cell (`06_ch6_scripts.ron`) runs `hidden_item`.
    hidden_item(&ctx, FLAG_SIL_ITEM_ROUTE_S06_ENCENS, ITEM_LAX_INCENSE).await;
}

// ---- the Bois des Soupirs ----

/// Bois_Soupirs_Sc01, the Pierre aux Vers: the old verse before the chapter's end
/// (`VAR_SIL_STORY` < 68), Adrien's fresh one after.
pub async fn RouteS06_BoisSoupirs_Sc01(ctx: Ctx) {
    let t = if story(&ctx).await < 68 {
        Bois_Soupirs_Sign_Pierre_Avant
    } else {
        Bois_Soupirs_Sign_Pierre_Apres
    };
    msgbox(&ctx, t, MsgBox::Sign).await;
}

pub async fn RouteS06_BoisSoupirs_EventScript_Amoureux(ctx: Ctx) {
    msgbox(&ctx, Bois_Soupirs_Npc_Amoureux, MsgBox::Npc).await;
}

/// POÈTE ARMAND on the pond's bridge (11, 8), facing west.
pub async fn RouteS06_BoisSoupirs_EventScript_Armand(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_BOIS_SOUPIRS_ARMAND,
        TRAINER_SIL_POETE_ARMAND,
        Bois_Soupirs_Npc_Amoureux,
        Gym_Seiji_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym_Seiji_Defaite).await;
    }
}

/// The Heart Scale on the pond's notch (Surf only).
pub async fn RouteS06_BoisSoupirs_EventScript_ItemHeartScale(ctx: Ctx) {
    find_item(&ctx, ITEM_HEART_SCALE, 1).await;
}

/// The Miracle Seed at the foot of the biggest cherry tree (6, 8).
pub async fn RouteS06_BoisSoupirs_EventScript_HiddenMiracleSeed(ctx: Ctx) {
    // fallback: hidden items (as above).
    hidden_item(
        &ctx,
        FLAG_SIL_ITEM_BOIS_SOUPIRS_MIRACLE_SEED,
        ITEM_MIRACLE_SEED,
    )
    .await;
}
