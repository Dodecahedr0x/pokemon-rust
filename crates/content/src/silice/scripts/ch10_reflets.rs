//! Chapter 10, 4F: les Reflets Infinis, Revenge's pillar (`TourNebula_4F`; `10-tour-piliers.md`
//! §3 `Tour_4F_Sc01`–`Sc06`, §4.4, the elites Écho and Mira).
//!
//! The way: the mirror maze, the three prisms turned to their mirrored arrows (p1 WEST 3, p2
//! NORTH 0, p3 SOUTH 2; `VAR_SIL_TOUR_PRISME_1`–`_3`, 0 NORTH … 3 WEST), the prism door (11, 4)
//! cracks, Revenge on its cell (`Sc03`, story 106 → 108), then the threshold: the hall below
//! (`Sc05`, R-6) or the vault elevator up (`Sc06`: the scripted save, the point of no return).

use super::ch08_evalon::lost;
use super::ch10_ascenseur::CABIN_FROM_HALL;
use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["TourNebula_4F"];

/// The prism door (11, 4), cracked: the maze floor.
const PORTE_PRISME: (i16, i16, u16) = (11, 4, 652);
/// The hall's elevator (Chapter 9's `Nebula_TourHall`): the cell in front of its door (12, 1).
const HALL_ASCENSEUR: (i16, i16) = (12, 2);

/// The prism door from `FLAG_SIL_REFLETS_PORTE_OUVERTE`.
///
/// The vault door (11, 0) opened by `Sc03` has no tile (`METATILE_PORTE_COFFRE_OUVERTE`): it
/// stays drawn shut, and stays a sign (`Sc06`).
pub async fn TourNebula_4F_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_REFLETS_PORTE_OUVERTE).await {
        ctx.set_metatile(PORTE_PRISME.0, PORTE_PRISME.1, PORTE_PRISME.2, false)
            .await;
    }
}

/// From story 107: Revenge gone, his glass statue in his place.
pub async fn TourNebula_4F_OnTransition(ctx: Ctx) {
    if story(&ctx).await >= 107 {
        ctx.set_flag(FLAG_SIL_HIDE_TOUR_REVENGE, true).await;
        ctx.set_flag(FLAG_SIL_HIDE_REVENGE_STATUE, false).await;
    }
}

/// `Tour_4F_Sc01`, the cold: two seconds of silence, then the reflections (once,
/// `FLAG_SIL_TOUR_4F_INTRO`). Coords on the hall on `VAR_TEMP_1` 0.
pub async fn TourNebula_4F_Sc01(ctx: Ctx) {
    ctx.set_var(VAR_TEMP_1, 1).await;
    if ctx.flag(FLAG_SIL_TOUR_4F_INTRO).await {
        return;
    }
    ctx.lock_all().await;
    ctx.delay(60).await;
    say(&ctx, Tour_4F_Sc01_T1).await;
    ctx.set_flag(FLAG_SIL_TOUR_4F_INTRO, true).await;
    ctx.release_all().await;
}

/// The prisms' solution (§4.4): p1 WEST, p2 NORTH, p3 SOUTH.
const SOLUTION: [u16; 3] = [3, 0, 2];
const PRISMES: [VarId; 3] = [
    VAR_SIL_TOUR_PRISME_1,
    VAR_SIL_TOUR_PRISME_2,
    VAR_SIL_TOUR_PRISME_3,
];

/// `Tour_4F_Sc02`, prism `n` (0-based): a quarter turn; the three right, the door cracks
/// (`FLAG_SIL_REFLETS_PORTE_OUVERTE`).
///
/// The source paints each prism's mirrored arrow on the floor in front of it (`Tour_4F_Indice_N`,
/// bg events the map does not have): the prism shows its arrow first. The prism's own orientation
/// tiles (`METATILE_PRISME_ORIENT`) do not exist: the prism is not redrawn.
async fn prisme(ctx: &Ctx, n: usize, indice: TextId) {
    ctx.lock().await;
    say(ctx, indice).await;
    if !ctx.flag(FLAG_SIL_REFLETS_PORTE_OUVERTE).await {
        say(ctx, Tour_4F_Sc02_T1).await;
        let v = (ctx.var(PRISMES[n]).await + 1) % 4;
        ctx.set_var(PRISMES[n], v).await;
        let mut solved = true;
        for (i, &want) in SOLUTION.iter().enumerate() {
            solved &= ctx.var(PRISMES[i]).await == want;
        }
        if solved {
            say(ctx, Tour_4F_Sc02_T2).await;
            ctx.set_metatile(PORTE_PRISME.0, PORTE_PRISME.1, PORTE_PRISME.2, false)
                .await;
            ctx.redraw_map().await;
            ctx.set_flag(FLAG_SIL_REFLETS_PORTE_OUVERTE, true).await;
        }
    }
    ctx.release().await;
}

/// p1 (2, 19): the arrow says EAST.
pub async fn TourNebula_4F_Sc02_1(ctx: Ctx) {
    prisme(&ctx, 0, Tour_4F_Indice_1).await;
}

/// p2 (12, 13): the arrow says SOUTH.
pub async fn TourNebula_4F_Sc02_2(ctx: Ctx) {
    prisme(&ctx, 1, Tour_4F_Indice_2).await;
}

/// p3 (5, 12), behind its false mirror: the arrow says NORTH.
pub async fn TourNebula_4F_Sc02_3(ctx: Ctx) {
    prisme(&ctx, 2, Tour_4F_Indice_3).await;
}

/// `Tour_4F_Sc03`, Revenge the Mirror (the prism door's cell, story 106): the battle, the Badge
/// Miroir restored, 99 %, the vault, Revenge turned to glass (→ 107), Julien's message (→ 108).
pub async fn TourNebula_4F_Sc03(ctx: Ctx) {
    if story(&ctx).await != 106 {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Tour_4F_Sc03_T1).await;
    ctx.turn(LOCALID_TOUR_4F_REVENGE, Dir::Down).await;
    say(&ctx, Tour_4F_Sc03_T2).await;
    say(&ctx, Tour_4F_Sc03_T3).await;
    // The Prisme de l'Ombre (the first three turns: half damage taken, 25 % recoil) is the
    // battle's own rule (`silice::rules::PRISM`, `trainer_rules`); Revenge says it before.
    if lost(battle_trainer(&ctx, TRAINER_SIL_REVENGE_1, Tour_4F_Sc03_Defeat).await) {
        return;
    }
    say(&ctx, Tour_4F_Sc03_T4).await;
    say(&ctx, Tour_4F_Sc03_T5).await;
    // fallback: the restored badge's sprite (the trainer card) does not exist; the flag does.
    say(&ctx, Tour_4F_Sc03_T6).await;
    ctx.set_flag(FLAG_SIL_BADGE_MIROIR_RESTORED, true).await;
    ctx.set_var(VAR_SIL_SYNC, 99).await;
    show_sync(&ctx, 99).await;
    say(&ctx, Tour_4F_Sc03_T7).await;
    say(&ctx, Tour_4F_Sc03_T8).await;
    say(&ctx, Tour_4F_Sc03_T9).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_TOUR_4F_REVENGE).await;
    show(&ctx, LOCALID_TOUR_4F_REVENGE_STATUE, FLAG_SIL_HIDE_REVENGE_STATUE).await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Tour_4F_Sc03_T10).await;
    set_story(&ctx, 107).await;
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Tour_4F_Sc03_T11).await;
    end_call(&ctx).await;
    set_story(&ctx, 108).await;
    ctx.release_all().await;
}

/// Revenge talked to: his scene at 106 (the door's coord plays it first).
pub async fn TourNebula_4F_EventScript_Revenge(ctx: Ctx) {
    TourNebula_4F_Sc03(ctx).await;
}

/// `Tour_4F_Sc04`, the glass statue.
pub async fn TourNebula_4F_Sc04(ctx: Ctx) {
    msgbox(&ctx, Tour_4F_Sc04_T1, MsgBox::Sign).await;
}

/// `Tour_4F_Sc05`, the elevator door (10, 24) from story 108: down to the hall (R-6, to heal and
/// shop before the point of no return; the hall's elevator brings the player back here).
/// Before, the door is shut (no text in the source).
pub async fn TourNebula_4F_Sc05(ctx: Ctx) {
    if story(&ctx).await < 108 {
        return;
    }
    ctx.lock_all().await;
    if ask(&ctx, Tour_4F_Sc05_T1).await {
        play_se(&ctx, "SE_ELEVATOR").await;
        warp(&ctx, MAP_NEBULA_TOUR_HALL, HALL_ASCENSEUR.0, HALL_ASCENSEUR.1).await;
        return;
    }
    ctx.release_all().await;
}

const STEP_LEFT: &[Step] = &[walk(Dir::Left)];
const STEP_DOWN: &[Step] = &[walk(Dir::Down)];

/// `Tour_4F_Sc06`, the threshold (the vault door (11, 0), or the coords (10–12, 1), story 108):
/// Gwada's warning once (`FLAG_SIL_TOUR_SEUIL_VU`), the save (R-6: the game's only offered save,
/// `save_game`, no dialog), then the point of no return (→ 109, the cabin). No: a step back.
pub async fn TourNebula_4F_Sc06(ctx: Ctx) {
    if story(&ctx).await != 108 {
        return;
    }
    ctx.lock_all().await;
    if !ctx.flag(FLAG_SIL_TOUR_SEUIL_VU).await {
        play_se(&ctx, "SE_PIN").await;
        mallette_call(&ctx, "GWADA").await;
        say(&ctx, Tour_4F_Sc06_T1).await;
        end_call(&ctx).await;
        ctx.set_flag(FLAG_SIL_TOUR_SEUIL_VU, true).await;
    } else {
        say(&ctx, Tour_4F_Sc06_T2).await;
    }
    if ask(&ctx, Tour_4F_Sc06_T3).await && ctx.save_game().await {
        say(&ctx, Tour_4F_Sc06_T3y).await;
    }
    if !ask(&ctx, Tour_4F_Sc06_T4).await {
        say(&ctx, Tour_4F_Sc06_T4n).await;
        // Off the coord row: down, except from (11, 1), whose south cell is the statue's.
        let back = if ctx.player_pos().await == (11, 1) {
            STEP_LEFT
        } else {
            STEP_DOWN
        };
        move_wait(&ctx, PLAYER, back).await;
        ctx.release_all().await;
        return;
    }
    set_story(&ctx, 109).await;
    play_se(&ctx, "SE_ELEVATOR").await;
    // The source's cabin cell (6, 6) is no landing; the hall's (3, 6) is: `Sc00` dispatches.
    warp(&ctx, MAP_TOUR_NEBULA_ASCENSEUR, CABIN_FROM_HALL.0, CABIN_FROM_HALL.1).await;
}

/// Écho (13, 9), sight 4 west.
pub async fn TourNebula_4F_EventScript_Echo(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_4F_ECHO,
        TRAINER_SIL_TOUR_ECHO,
        Tour_4F_Echo_Intro,
        Tour_4F_Echo_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_4F_Echo_After).await;
    }
}

/// Mira (3, 7), sight 4 east: afterwards, the arrows' rule.
pub async fn TourNebula_4F_EventScript_Mira(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_TOUR_4F_MIRA,
        TRAINER_SIL_TOUR_MIRA,
        Tour_4F_Mira_Intro,
        Tour_4F_Mira_Defeat,
    )
    .await
    {
        post_battle(&ctx, Tour_4F_Mira_After).await;
    }
}

/// The Full Restore ball (9, 21).
pub async fn TourNebula_4F_EventScript_ItemFullRestore(ctx: Ctx) {
    find_item(&ctx, ITEM_FULL_RESTORE, 1).await;
}

/// The hidden Max Revive (17, 19), behind its false mirror: a sign while the engine ignores
/// hidden items.
pub async fn TourNebula_4F_EventScript_HiddenMaxRevive(ctx: Ctx) {
    hidden_item(&ctx, FLAG_SIL_ITEM_TOUR_4F_MAX_REVIVE, ITEM_MAX_REVIVE).await;
}

pub async fn TourNebula_4F_EventScript_Entree(ctx: Ctx) {
    msgbox(&ctx, Tour_4F_Sign_Entree, MsgBox::Sign).await;
}

pub async fn TourNebula_4F_EventScript_Miroir(ctx: Ctx) {
    msgbox(&ctx, Tour_4F_Sign_Miroir, MsgBox::Sign).await;
}

pub async fn TourNebula_4F_EventScript_FauxMiroir(ctx: Ctx) {
    msgbox(&ctx, Tour_4F_Sign_FauxMiroir, MsgBox::Sign).await;
}
