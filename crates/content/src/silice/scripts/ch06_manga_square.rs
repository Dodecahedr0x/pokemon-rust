//! Chapter 6, Manga-Square (`MangaSquare`) and its small buildings (`MangaSquare_PokemonCenter`,
//! `MangaSquare_Mart`, the Konbini, and `MangaSquare_Toiletteur`): `contenu-jeu/chapitres/
//! 06-manga-square.md` §2 (signs, ambient NPCs, the Grand Torii's controleur), §3
//! Manga_Square_Sc01 to Sc04 (the arrival, Benou at the Grand Torii, Adrien's promise, the
//! groomer) and §7 (the Revive, the Konbini). `VAR_SIL_STORY` runs 61 (the Princesse saved) to
//! 62 here, then 66 (Tak beaten) to 68 (the chapter's end, the way to Route S7 open). The Dojo
//! and the Bibliothèque are `ch06_dojo.rs`, the gym `ch06_gym.rs`.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &[
    "MangaSquare",
    "MangaSquare_PokemonCenter",
    "MangaSquare_Mart",
    "MangaSquare_Toiletteur",
];

// ---- map scripts ----

/// The scenes' actors: Benou and Adrien (the chapter's end) only inside their scenes; the
/// controleur bars the Grand Torii until Manga_Square_Sc02 (67); the hard-drive courier from 64
/// (§2 "après VAR 64"). The square's palette is the dawn's ([`AUBE`]).
pub async fn MangaSquare_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    ctx.set_flag(FLAG_SIL_HIDE_MANGA_SQUARE_BENOU, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_MANGA_SQUARE_ADRIEN_FIN, true)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_MANGA_SQUARE_CONTROLEUR, s >= 67)
        .await;
    ctx.set_flag(FLAG_SIL_HIDE_MANGA_SQUARE_LIVREUR, s < 64)
        .await;
    // Chapter 12: Tak before the lit statue from 130 (`Manga_Square_PG_Sc32a`).
    ctx.set_flag(FLAG_SIL_HIDE_MANGA_TAK, s < 130).await;
    // The dawn tint (§2, the Secteur Zêta's sky set to dawn).
    set_palette(&ctx, AUBE, 0, false).await;
}

// ---- scenes ----

/// Manga_Square_Sc01, under the toriis: the way up to the Torii Sud (20–24, 33) or the Monks' shortcut's
/// first step (2, 25), at `VAR_SIL_STORY` 61. Kimiko's stream on every screen. Sets 62.
pub async fn MangaSquare_Sc01(ctx: Ctx) {
    if story(&ctx).await != 61 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Manga_Square_Sc01_T1).await;
    say(&ctx, Manga_Square_Sc01_T2).await;
    set_story(&ctx, 62).await;
    ctx.release_all().await;
}

const FACE_PLAYER: &[Step] = &[Step::FacePlayer];
/// The controleur steps aside along the Grand Torii's beam (the source's `Walk(Left) x6`; the
/// beam is x 35–39 on row 1).
const CONTROLEUR_OUT: &[Step] = &[walk(Dir::Left), walk(Dir::Left)];
/// Benou leaves north through the Grand Torii (37, 3) to its exit (37, 0) (`Walk(Up) x4`).
const BENOU_OUT: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up)];

/// The Mallette in Benou's hands: the source's `fade(Out) ; delay(60) ; fade(In)`.
async fn mallette_work(ctx: &Ctx) {
    ctx.fade(Fade::ToBlack).await;
    ctx.delay(60).await;
    ctx.fade(Fade::FromBlack).await;
}

/// Manga_Square_Sc02, Benou at the Grand Torii: the three cells in front of it (36–38, 6) at
/// `VAR_SIL_STORY` 66 (Tak beaten). The Scanner de Faille (`FLAG_SIL_MODULE_ECLATE_ROC`) and the
/// Méga-Liaison (`FLAG_SIL_MODULE_MEGA`); the controleur lets the player through. Sets 67.
pub async fn MangaSquare_Sc02(ctx: Ctx) {
    if story(&ctx).await != 66 {
        return;
    }
    ctx.lock_all().await;
    show(
        &ctx,
        LOCALID_MANGA_SQUARE_BENOU,
        FLAG_SIL_HIDE_MANGA_SQUARE_BENOU,
    )
    .await;
    say(&ctx, Manga_Square_Sc02_T1).await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_BENOU, FACE_PLAYER).await;
    say(&ctx, Manga_Square_Sc02_T2).await;
    say(&ctx, Manga_Square_Sc02_T3).await;
    say(&ctx, Manga_Square_Sc02_T4).await;
    mallette_work(&ctx).await;
    ctx.set_flag(FLAG_SIL_MODULE_ECLATE_ROC, true).await;
    // fallback: play_fanfare(MUS_OBTAIN_TMHM) (audio, M12): nothing.
    fallback(&ctx, "play_fanfare").await;
    say(&ctx, Manga_Square_Sc02_T5).await;
    say(&ctx, Manga_Square_Sc02_T6).await;
    say(&ctx, Manga_Square_Sc02_T6b).await;
    mallette_work(&ctx).await;
    ctx.set_flag(FLAG_SIL_MODULE_MEGA, true).await;
    fallback(&ctx, "play_fanfare").await;
    say(&ctx, Manga_Square_Sc02_T6c).await;
    say(&ctx, Manga_Square_Sc02_T6d).await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_CONTROLEUR, FACE_PLAYER).await;
    say(&ctx, Manga_Square_Sc02_T7).await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_CONTROLEUR, CONTROLEUR_OUT).await;
    // `hide` sets FLAG_SIL_HIDE_MANGA_SQUARE_CONTROLEUR (and OnTransition keeps it from 67).
    hide(&ctx, LOCALID_MANGA_SQUARE_CONTROLEUR).await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_BENOU, BENOU_OUT).await;
    hide(&ctx, LOCALID_MANGA_SQUARE_BENOU).await;
    set_story(&ctx, 67).await;
    ctx.release_all().await;
}

/// Adrien runs up from the square (37, 8) to the player under the Grand Torii (37, 1)
/// (`Walk(Up) x5, FacePlayer`), and back.
const ADRIEN_IN: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    Step::FacePlayer,
];
const ADRIEN_OUT: &[Step] = &[
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
];

/// Manga_Square_Sc03, Adrien's promise: the Grand Torii's exit cell (37, 1) at `VAR_SIL_STORY`
/// 67. Sets 68, the chapter's end (Route S7 through the exit (37, 0)).
pub async fn MangaSquare_Sc03(ctx: Ctx) {
    if story(&ctx).await != 67 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Manga_Square_Sc03_T1).await;
    show(
        &ctx,
        LOCALID_MANGA_SQUARE_ADRIEN,
        FLAG_SIL_HIDE_MANGA_SQUARE_ADRIEN_FIN,
    )
    .await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_ADRIEN, ADRIEN_IN).await;
    ctx.turn(PLAYER, Dir::Down).await;
    say(&ctx, Manga_Square_Sc03_T2).await;
    say(&ctx, Manga_Square_Sc03_T3).await;
    say(&ctx, Manga_Square_Sc03_T4).await;
    say(&ctx, Manga_Square_Sc03_T5).await;
    move_wait(&ctx, LOCALID_MANGA_SQUARE_ADRIEN, ADRIEN_OUT).await;
    hide(&ctx, LOCALID_MANGA_SQUARE_ADRIEN).await;
    set_story(&ctx, 68).await;
    ctx.release_all().await;
}

/// The Peigne de Luxe after the first time (§9).
const PEIGNE_PRICE: u32 = 500;

/// Manga_Square_Sc04, the groomer (the Toiletteur's WOMAN_5): a Peigne de Luxe?
///
/// +30 friendship to the lead (`add_friendship`), free the first time
/// (`FLAG_SIL_MANGA_SQUARE_PEIGNE_GRATUIT`), then 500 ₽. Author question: the source has no
/// line for a player short of 500 ₽; the refusal's (`Toiletteur_T3`) stands in, nothing taken.
pub async fn MangaSquare_Sc04(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ask(&ctx, Toiletteur_T1).await {
        let paid = ctx.flag(FLAG_SIL_MANGA_SQUARE_PEIGNE_GRATUIT).await;
        if paid && !ctx.money_check(PEIGNE_PRICE).await {
            say(&ctx, Toiletteur_T3).await;
            ctx.release().await;
            return;
        }
        if paid {
            ctx.money_remove(PEIGNE_PRICE).await;
        }
        ctx.fade(Fade::ToBlack).await;
        ctx.delay(40).await;
        ctx.fade(Fade::FromBlack).await;
        ctx.add_friendship(0, 30).await;
        say(&ctx, Toiletteur_T2).await;
        ctx.set_flag(FLAG_SIL_MANGA_SQUARE_PEIGNE_GRATUIT, true).await;
    } else {
        say(&ctx, Toiletteur_T3).await;
    }
    ctx.release().await;
}

// ---- signs and NPCs ----

pub async fn MangaSquare_EventScript_SignVille(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Sign_Ville, MsgBox::Sign).await;
}

pub async fn MangaSquare_EventScript_SignStatue(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Sign_Statue, MsgBox::Sign).await;
}

pub async fn MangaSquare_EventScript_SignGrandTorii(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Sign_GrandTorii, MsgBox::Sign).await;
}

/// The square's two holographic screens: Kimiko's stream.
pub async fn MangaSquare_EventScript_SignEcran(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Sign_Ecran, MsgBox::Sign).await;
}

pub async fn MangaSquare_EventScript_Cosplayeur(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Cosplayeur, MsgBox::Npc).await;
}

pub async fn MangaSquare_EventScript_Puriste(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Puriste, MsgBox::Npc).await;
}

pub async fn MangaSquare_EventScript_Mere(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Mere, MsgBox::Npc).await;
}

/// The elder by the statue: Kura-Shinobi and the Dome's closing twenty years ago, the gym
/// quiz's other source for Q1 and Q2 (`FLAG_SIL_GYM06_LU_Q1`, `_Q2`; `ch06_gym.rs`).
pub async fn MangaSquare_EventScript_Ancien(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Ancien, MsgBox::Npc).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q1, true).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q2, true).await;
}

/// The hard-drive courier, from `VAR_SIL_STORY` 64.
pub async fn MangaSquare_EventScript_Livreur(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Livreur, MsgBox::Npc).await;
}

pub async fn MangaSquare_EventScript_Gamin(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Gamin, MsgBox::Npc).await;
}

/// The Cocket controleur on the Grand Torii's exit (37, 1), until Manga_Square_Sc02.
pub async fn MangaSquare_EventScript_Controleur(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Npc_Controleur, MsgBox::Npc).await;
}

// ---- the Pokémon Center, the Konbini, the Toiletteur ----

/// The respawn at Manga-Square's Center on entering it, as Ch1–Ch3's Centers.
pub async fn MangaSquare_PokemonCenter_OnTransition(ctx: Ctx) {
    ctx.set_respawn(HEAL_LOCATION_SIL_MANGA_SQUARE).await;
}

pub async fn MangaSquare_PokemonCenter_EventScript_Nurse(ctx: Ctx) {
    pokecenter_nurse(&ctx).await;
}

pub async fn MangaSquare_PokemonCenter_EventScript_Fan(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_PC_Npc_Fan, MsgBox::Npc).await;
}

/// The Konbini's stock (§7 "Konbini de Manga-Square").
const KONBINI: &[ItemId] = &[
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_GREAT_BALL),
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_SUPER_POTION),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_SUPER_REPEL),
    ItemId(ITEM_ESCAPE_ROPE),
];

/// The Konbini's clerk.
pub async fn MangaSquare_Mart_EventScript_Clerk(ctx: Ctx) {
    // fallback: mart (M9's shop screen): Ch1's frame, the stock kept here.
    mart(&ctx, KONBINI).await;
}

/// The salon's Skitty: the source gives it no line; it turns to the player.
pub async fn MangaSquare_Toiletteur_EventScript_Skitty(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    ctx.release().await;
}
