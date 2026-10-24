//! Chapter 10, 1F: la Cave, Mathias's pillar (`TourNebula_1F`; `10-tour-piliers.md` §3
//! `Tour_1F_Sc01`–`Sc05`, §4.1, the elites Gaston and Coralie).
//!
//! The way: one of the two iron-bound barrels (7, 10) / (17, 10) pushed north off its pontoon
//! (Chapter 4's Champ Magnétique, `ch04_port::push_object`), the tasting (barrel C opens the
//! grille (10–14, 5)), the Cœur d'Évalon's pulse past it, Mathias at the counter, then the
//! elevator door (16, 1) behind it.
//!
//! Temp vars: `VAR_TEMP_1` (the intro's coords), `VAR_TEMP_2` (the pulse's coords), `VAR_TEMP_3`
//! and `VAR_TEMP_5` (the barrels' cells), `VAR_TEMP_4` (`push_object`'s first-push line).

use super::ch04_port::{Pushable, push_object};
use super::ch08_evalon::lost;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TourNebula_1F"];

/// The elevator door behind the counter (16, 1), open from story 101 (the map's metatile).
const PORTE: (i16, i16, u16) = (16, 1, 694);
/// The grille (10–14, 5), open: the cellar floor.
const GRILLE_Y: i16 = 5;
const GRILLE_OUVERTE: u16 = 568;

async fn open_grille(ctx: &Ctx) {
    for x in 10..=14 {
        ctx.set_metatile(x, GRILLE_Y, GRILLE_OUVERTE, false).await;
    }
}

/// The door from story 101, the grille from `FLAG_SIL_CAVE_GRILLE_OUVERTE`.
pub async fn TourNebula_1F_OnLoad(ctx: Ctx) {
    if story(&ctx).await >= 101 {
        ctx.set_metatile(PORTE.0, PORTE.1, PORTE.2, false).await;
    }
    if ctx.flag(FLAG_SIL_CAVE_GRILLE_OUVERTE).await {
        open_grille(&ctx).await;
    }
}

const GARDE_DOWN: &[Step] = &[walk(Dir::Down), walk(Dir::Down), Step::FacePlayer];
const GARDE_BACK: &[Step] = &[walk(Dir::Up), walk(Dir::Up), face(Dir::Left)];

/// `Tour_1F_Sc01`, the cellar's smell: the guard lets the player through (once,
/// `FLAG_SIL_TOUR_1F_INTRO`). Coords round the landing on `VAR_TEMP_1` 0.
pub async fn TourNebula_1F_Sc01(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_TOUR_1F_INTRO).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Tour_1F_Sc01_T1).await;
    move_wait(&ctx, LOCALID_TOUR_1F_GARDE, GARDE_DOWN).await;
    say(&ctx, Tour_1F_Sc01_T2).await;
    // The guard goes back to his post (13, 19), facing west as the map has him (the source:
    // Face(Down)).
    move_wait(&ctx, LOCALID_TOUR_1F_GARDE, GARDE_BACK).await;
    ctx.set_flag(FLAG_SIL_TOUR_1F_INTRO, true).await;
    ctx.release_all().await;
}

/// `Tour_1F_Sc02`, the maître de chai: the rule of the grille.
pub async fn TourNebula_1F_Sc02(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Sc02_T1, MsgBox::Npc).await;
}

/// A wrong barrel (A or B): its label, the tasting, the joke.
async fn faux_fut(ctx: &Ctx, label: TextId) {
    ctx.lock().await;
    say(ctx, label).await;
    if !ctx.flag(FLAG_SIL_CAVE_GRILLE_OUVERTE).await && ask(ctx, Tour_1F_Sc02_T4).await {
        say(ctx, Tour_1F_Sc02_T5).await;
    }
    ctx.release().await;
}

/// Barrel A (7, 7): synthetic rum.
pub async fn TourNebula_1F_Sc02_A(ctx: Ctx) {
    faux_fut(&ctx, Tour_1F_Sc02_T2).await;
}

/// Barrel B (10, 7): rum flavour n°7.
pub async fn TourNebula_1F_Sc02_B(ctx: Ctx) {
    faux_fut(&ctx, Tour_1F_Sc02_T3).await;
}

/// Barrel C (13, 7), the boss's rum: tasted, the maître opens the grille
/// (`FLAG_SIL_CAVE_GRILLE_OUVERTE`). Once open, only its label.
pub async fn TourNebula_1F_Sc02_C(ctx: Ctx) {
    ctx.lock().await;
    say(&ctx, Tour_1F_Sc02_T6).await;
    if !ctx.flag(FLAG_SIL_CAVE_GRILLE_OUVERTE).await && ask(&ctx, Tour_1F_Sc02_T4).await {
        say(&ctx, Tour_1F_Sc02_T7).await;
        move_wait(&ctx, LOCALID_TOUR_1F_MAITRE_CHAI, &[Step::FacePlayer]).await;
        say(&ctx, Tour_1F_Sc02_T8).await;
        open_grille(&ctx).await;
        ctx.redraw_map().await;
        ctx.set_flag(FLAG_SIL_CAVE_GRILLE_OUVERTE, true).await;
    }
    ctx.release().await;
}

/// `Tour_1F_Sc03`, the Cœur d'Évalon pulses (the row past the grille, `VAR_TEMP_2` 0): once
/// (`FLAG_SIL_TOUR_COEUR_PULSE`); with the Cœur, three Persim Berries.
pub async fn TourNebula_1F_Sc03(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_2, 1).await;
    if !ctx.flag(FLAG_SIL_CAVE_GRILLE_OUVERTE).await
        || ctx.flag(FLAG_SIL_TOUR_COEUR_PULSE).await
    {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Tour_1F_Sc03_T1).await;
    if ctx.flag(FLAG_SIL_COEUR_EVALON).await {
        say(&ctx, Tour_1F_Sc03_T2).await;
        // The Cœur's "Isolant" is the 1F battles' own rule (`silice::rules::HEART`); the three
        // berries the source gave as its fallback stay, since the text gives them.
        add_item(&ctx, ITEM_PERSIM_BERRY, 3).await;
    }
    ctx.set_flag(FLAG_SIL_TOUR_COEUR_PULSE, true).await;
    ctx.release_all().await;
}

/// `Tour_1F_Sc04`, Mathias the Distillateur (story 100): the battle, the ACCÈS ALPHA, 85 %, the
/// door behind the counter (→ 101). `Tour_1F_Sc05` afterwards.
pub async fn TourNebula_1F_Sc04(ctx: Ctx) {
    if story(&ctx).await != 100 {
        TourNebula_1F_Sc05(&ctx).await;
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Tour_1F_Sc04_T1).await;
    say(&ctx, Tour_1F_Sc04_T2).await;
    // fallback: Spinda's Teeter Dance is the battle's own; the Cœur's effect is not (see Sc03).
    if lost(battle_trainer(&ctx, TRAINER_SIL_MATHIAS_1, Tour_1F_Sc04_Defeat).await) {
        return;
    }
    say(&ctx, Tour_1F_Sc04_T3).await;
    say(&ctx, Tour_1F_Sc04_T4).await;
    add_item(&ctx, ITEM_SIL_ACCES_ALPHA, 1).await;
    ctx.set_flag(FLAG_SIL_KEY_ACCES_ALPHA, true).await;
    ctx.set_var(VAR_SIL_SYNC, 85).await;
    show_sync(&ctx, 85).await;
    say(&ctx, Tour_1F_Sc04_T5).await;
    say(&ctx, Tour_1F_Sc04_T6).await;
    ctx.set_metatile(PORTE.0, PORTE.1, PORTE.2, false).await;
    ctx.redraw_map().await;
    set_story(&ctx, 101).await;
    ctx.release_all().await;
}

/// `Tour_1F_Sc05`, Mathias after the battle.
async fn TourNebula_1F_Sc05(ctx: &Ctx) {
    msgbox(ctx, Tour_1F_Sc05_T1, MsgBox::Npc).await;
}

/// Gaston (3, 14), sight 4 east.
pub async fn TourNebula_1F_EventScript_Gaston(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_1F_GASTON,
        TRAINER_SIL_TOUR_GASTON,
        Tour_1F_Gaston_Intro,
        Tour_1F_Gaston_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_1F_Gaston_After).await;
    }
}

/// Coralie (3, 9), sight 4 east: she guards the barrel zone.
pub async fn TourNebula_1F_EventScript_Coralie(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_1F_CORALIE,
        TRAINER_SIL_TOUR_CORALIE,
        Tour_1F_Coralie_Intro,
        Tour_1F_Coralie_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_1F_Coralie_After).await;
    }
}

/// The steward's stock (§2: the last counter before the floors).
const INTENDANT: &[ItemId] = &[
    ItemId(ITEM_HYPER_POTION),
    ItemId(ITEM_MAX_POTION),
    ItemId(ITEM_FULL_RESTORE),
    ItemId(ITEM_FULL_HEAL),
    ItemId(ITEM_REVIVE),
    ItemId(ITEM_ULTRA_BALL),
    ItemId(ITEM_PERSIM_BERRY),
];

/// The steward (15, 14): his line, then the shop.
pub async fn TourNebula_1F_EventScript_Intendant(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Npc_Intendant, MsgBox::Npc).await;
    // fallback: mart (the shop screen): a greeting, nothing sold.
    mart(&ctx, INTENDANT).await;
}

pub async fn TourNebula_1F_EventScript_Garde(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Npc_Garde, MsgBox::Npc).await;
}

pub async fn TourNebula_1F_EventScript_Serveuse(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Npc_Serveuse, MsgBox::Npc).await;
}

/// The Full Restore ball (23, 13).
pub async fn TourNebula_1F_EventScript_ItemFullRestore(ctx: Ctx) {
    find_item(&ctx, ITEM_FULL_RESTORE, 1).await;
}

/// The two iron-bound barrels on the pontoons' north ends, each pushed north off its pontoon
/// (one is enough). Temp vars `VAR_TEMP_3` / `VAR_TEMP_5`.
const FUTS: &[Pushable] = &[
    (LOCALID_TOUR_1F_FUT_1, VAR_TEMP_3, (7, 10)),
    (LOCALID_TOUR_1F_FUT_2, VAR_TEMP_5, (17, 10)),
];

/// The barrel zone's floor (rows 6–10, x 5–20) without the stacks, the tasting barrels and the
/// maître de chai's cell.
fn fut_free((x, y): (i16, i16)) -> bool {
    const SOLID: &[(i16, i16)] = &[
        (16, 7),
        (7, 7),
        (10, 7),
        (13, 7),
        (9, 8),
        (10, 8),
        (14, 8),
        (15, 8),
        (9, 9),
        (10, 9),
        (14, 9),
        (15, 9),
    ];
    (5..=20).contains(&x) && (6..=10).contains(&y) && !SOLID.contains(&(x, y))
}

/// An iron-bound barrel (Champ Magnétique).
pub async fn TourNebula_1F_EventScript_FutCercle(ctx: Ctx) {
    push_object(&ctx, FUTS, fut_free).await;
}

pub async fn TourNebula_1F_EventScript_Entree(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Sign_Entree, MsgBox::Sign).await;
}

pub async fn TourNebula_1F_EventScript_Ponton(ctx: Ctx) {
    msgbox(&ctx, Tour_1F_Sign_Ponton, MsgBox::Sign).await;
}

/// The elevator door the player came in by (11, 22), closed: the source gives it no text (the
/// cabin only goes up); nothing happens.
pub async fn TourNebula_1F_EventScript_PorteVerrouillee(_ctx: Ctx) {
    // fallback: no text in the source for the locked elevator door.
}
