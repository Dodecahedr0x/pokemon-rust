//! Chapter 3, the Nocturne, Tauber & Hocseat's gym (`ViberCity_Gym`):
//! `contenu-jeu/chapitres/03-viber-city.md` §4 (the "Trois pistes" puzzle, the trainers, the
//! manager, the strobe gimmick) and §3/§4 Nocturne_Sc01 and Sc02. `VAR_SIL_STORY` runs 34 (the
//! city woken, the club open) to 35 (the Tempo badge, `LOG_03`).
//!
//! **The tracks.** `VAR_SIL_NOCTURNE_STATE` is the track playing: 0 blue, 1 yellow, 2 violet.
//! The track's colour of laser barrier is open, the two others closed. Each console moves to the
//! next track (blue, yellow, violet, blue…); the map load plays blue again.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["ViberCity_Gym"];

/// A barrier's cells: `(x0..=x1, y)` and its track.
const BARRIERES: [(i16, i16, i16, u16); 4] = [
    // Violet, across the stage (row 4).
    (1, 11, 4, 2),
    // Yellow, zone C's gate (5–7, 7).
    (5, 7, 7, 1),
    // Blue, zone B's left half (2–3, 11): the false track.
    (2, 3, 11, 0),
    // Violet, zone B's right half (9–10, 11).
    (9, 10, 11, 2),
];

/// `METATILE_LASER_*`: SilViberCityGym's laser barriers (games/silice/gfx/SilViberCityGym.ron),
/// closed (solid) in the order blue, yellow, violet, then open (walkable) in the same order.
const METATILE_LASER_FERME: u16 = 534;
const METATILE_LASER_OUVERT: u16 = 537;

/// A barrier cell of `colour` (0 blue, 1 yellow, 2 violet), closed or open, as `01_terrain.ron`
/// draws them.
fn barrier_metatile(colour: u16, open: bool) -> u16 {
    if open {
        METATILE_LASER_OUVERT + colour
    } else {
        METATILE_LASER_FERME + colour
    }
}

/// Draws every barrier for `track`.
async fn draw_track(ctx: &Ctx, track: u16) {
    for (x0, x1, y, colour) in BARRIERES {
        let open = colour == track;
        for x in x0..=x1 {
            ctx.set_metatile(x, y, barrier_metatile(colour, open), !open)
                .await;
        }
    }
}

// ---- map scripts ----

/// The club plays the blue track on every load.
pub async fn ViberCity_Gym_OnLoad(ctx: Ctx) {
    ctx.set_var(VAR_SIL_NOCTURNE_STATE, 0).await;
    draw_track(&ctx, 0).await;
}

// ---- the consoles (Nocturne_Sc01) ----

/// Nocturne_Sc01, a DJ console: the next track, its barriers, its line.
async fn console(ctx: &Ctx) {
    ctx.lock_all().await;
    let track = (ctx.var(VAR_SIL_NOCTURNE_STATE).await + 1) % 3;
    ctx.set_var(VAR_SIL_NOCTURNE_STATE, track).await;
    draw_track(ctx, track).await;
    // fallback: play_music (each track its own variant of the gym theme, §4.2; audio, M12).
    fallback(ctx, "play_music").await;
    let text = match track {
        0 => Nocturne_Sc01_T0,
        1 => Nocturne_Sc01_T1,
        _ => Nocturne_Sc01_T2,
    };
    say(ctx, text).await;
    ctx.release_all().await;
}

/// Console A, zone A (2, 13).
pub async fn ViberCity_Gym_EventScript_ConsoleA(ctx: Ctx) {
    console(&ctx).await;
}

/// Console B, zone B's right half (9, 9).
pub async fn ViberCity_Gym_EventScript_ConsoleB(ctx: Ctx) {
    console(&ctx).await;
}

/// Console C, zone C (1, 6).
pub async fn ViberCity_Gym_EventScript_ConsoleC(ctx: Ctx) {
    console(&ctx).await;
}

// ---- the leaders (Nocturne_Sc02) ----

/// Nocturne_Sc02, Tauber & Hocseat: the cell before them (6, 3) at `VAR_SIL_STORY` 34, or
/// either of them talked to. Two Pokémon are needed; the battle, the Tempo badge, the Surcharge
/// Optique, TM34 (Shock Wave), the 30 % sync, `LOG_03`, the Link Stone. Sets 35. Afterwards,
/// each one's line.
pub async fn ViberCity_Gym_Sc02(ctx: Ctx) {
    let s = story(&ctx).await;
    if s >= 35 {
        let text = if ctx.var(VAR_LAST_TALKED).await == u16::from(LOCALID_NOCTURNE_HOCSEAT) {
            Nocturne_Hocseat_T1
        } else {
            Nocturne_Tauber_T1
        };
        msgbox(&ctx, text, MsgBox::Npc).await;
        return;
    }
    if s != 34 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Nocturne_Sc02_T1).await;
    say(&ctx, Nocturne_Sc02_T2).await;
    // fallback: count the Pokémon able to fight (§9): the party's size alone.
    fallback(&ctx, "party_valid_count").await;
    if ctx.party_size().await < 2 {
        say(&ctx, Nocturne_Sc02_T_PasAssez).await;
        ctx.release_all().await;
        return;
    }
    say(&ctx, Nocturne_Sc02_T3).await;
    // fallback: the strobe (Blanche, UV, Néon each turn, §4.5): no battle field effect; the
    // manager's rules and T3 say it. The engine runs the double battle as singles against the
    // same six (it reports "double battle" itself).
    fallback(&ctx, "strobe").await;
    battle_trainer(&ctx, TRAINER_SIL_TAUBER_HOCSEAT_1, Nocturne_Sc02_Defaite).await;
    say(&ctx, Nocturne_Sc02_T4).await;
    say(&ctx, Nocturne_Sc02_T5).await;
    ctx.set_flag(FLAG_SIL_BADGE_03, true).await;
    say(&ctx, Nocturne_Sc02_T6).await;
    say(&ctx, Nocturne_Sc02_T7).await;
    ctx.set_flag(FLAG_SIL_MODULE_FLASH, true).await;
    say(&ctx, Nocturne_Sc02_T8).await;
    say(&ctx, Nocturne_Sc02_T9).await;
    // §7 "Récompense", no flag: the scene runs once. T9 offers it ("Et prends ça").
    give_item(&ctx, ITEM_TM34, 1).await;
    ctx.set_var(VAR_SIL_SYNC, 30).await;
    // fallback: show_sync (the gauge screen): T10 says the value.
    show_sync(&ctx, 30).await;
    say(&ctx, Nocturne_Sc02_T10).await;
    ctx.set_flag(FLAG_SIL_LOG_03, true).await;
    // fallback: log_menu (the Mallette's log screen): the log's text as a message (T11).
    fallback(&ctx, "log_menu").await;
    say(&ctx, Nocturne_Sc02_T11).await;
    say(&ctx, Nocturne_Sc02_T12).await;
    say(&ctx, Nocturne_Sc02_T13).await;
    // T14 says the Link Stone is obtained.
    gift(
        &ctx,
        FLAG_SIL_ITEM_NOCTURNE_PIERRE_LIEN,
        ITEM_SIL_PIERRE_LIEN,
        1,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Nocturne_Sc02_T14).await;
    set_story(&ctx, 35).await;
    ctx.release_all().await;
}

// ---- the manager ----

/// The manager at the entry (4, 14): the rules (`Nocturne_Gerant_T1`); once after the badge,
/// his `_T2` and the Fresh Water (`FLAG_SIL_ITEM_NOCTURNE_FRESH_WATER`, with Gen3's obtain
/// messages: his line offers it); afterwards the rules again (T2 offers the water, so it is not
/// repeated; an author question).
pub async fn ViberCity_Gym_EventScript_Gerant(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_BADGE_03).await || ctx.flag(FLAG_SIL_ITEM_NOCTURNE_FRESH_WATER).await {
        msgbox(&ctx, Nocturne_Gerant_T1, MsgBox::Npc).await;
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    say(&ctx, Nocturne_Gerant_T2).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_NOCTURNE_FRESH_WATER,
        ITEM_FRESH_WATER,
        1,
        Obtain::Message,
    )
    .await;
    ctx.release().await;
}

// ---- trainers (§4.3) ----

/// VJ LÉNA (6, 13), facing down.
pub async fn ViberCity_Gym_EventScript_Lena(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NOCTURNE_LENA,
        TRAINER_SIL_NOCTURNE_LENA_1,
        Nocturne_Lena_Intro,
        Nocturne_Lena_Defaite,
    )
    .await
    {
        post_battle(&ctx, Nocturne_Lena_Apres).await;
    }
}

/// DANSEUR HUGO (10, 14), facing left.
pub async fn ViberCity_Gym_EventScript_Hugo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NOCTURNE_HUGO,
        TRAINER_SIL_NOCTURNE_HUGO_1,
        Nocturne_Hugo_Intro,
        Nocturne_Hugo_Defaite,
    )
    .await
    {
        post_battle(&ctx, Nocturne_Hugo_Apres).await;
    }
}

/// MYSTIMANIAC NINA (2, 9), on the false track, facing right.
pub async fn ViberCity_Gym_EventScript_Nina(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NOCTURNE_NINA,
        TRAINER_SIL_NOCTURNE_NINA_1,
        Nocturne_Nina_Intro,
        Nocturne_Nina_Defaite,
    )
    .await
    {
        post_battle(&ctx, Nocturne_Nina_Apres).await;
    }
}

/// DJ ÉLIOTT (6, 6), facing down onto the yellow gate.
pub async fn ViberCity_Gym_EventScript_Eliott(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_NOCTURNE_ELIOTT,
        TRAINER_SIL_NOCTURNE_ELIOTT_1,
        Nocturne_Eliott_Intro,
        Nocturne_Eliott_Defaite,
    )
    .await
    {
        post_battle(&ctx, Nocturne_Eliott_Apres).await;
    }
}

/// The Magnet on the false track (1, 10).
pub async fn ViberCity_Gym_EventScript_ItemMagnet(ctx: Ctx) {
    find_item(&ctx, ITEM_MAGNET, 1).await;
}
