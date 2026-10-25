//! Chapter 6, the Sanctuaire du Bushido (`MangaSquare_Gym`), behind the Dojo:
//! `contenu-jeu/chapitres/06-manga-square.md` §3 Manga_Square_Gym_Sc01 (the samurai's staring
//! duel, ×2), Sc02 (the scroll quiz, ×5) and Sc03 (Tak), §4 (the trainers). `VAR_SIL_STORY` 64
//! (the gym open) → 65 (the fifth door) → 66 (Tak beaten).

use crate::silice::prelude::*;
use crate::silice::rules::WITH_PETALS_BLINK;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["MangaSquare_Gym"];

/// `METATILE_SHOJI_OUVERT`: an opened shoji door, SilMangaSquareGym `shoji_ouverte`, walkable.
const METATILE_SHOJI_OUVERT: u16 = 522;

/// Door `n` (1 to 5, south to north) is on (10, 24 − 4n); its desk on (9, 25 − 4n), read from
/// the alley (10, 25 − 4n) or from below.
const fn door_y(n: u16) -> i16 {
    24 - 4 * n as i16
}

const PORTES: [FlagId; 5] = [
    FLAG_SIL_GYM06_PORTE_1,
    FLAG_SIL_GYM06_PORTE_2,
    FLAG_SIL_GYM06_PORTE_3,
    FLAG_SIL_GYM06_PORTE_4,
    FLAG_SIL_GYM06_PORTE_5,
];

/// The open doors stay open (the source's `on_load`).
pub async fn MangaSquare_Gym_OnLoad(ctx: Ctx) {
    for (i, flag) in PORTES.iter().enumerate() {
        if ctx.flag(*flag).await {
            let n = i as u16 + 1;
            ctx.set_metatile(10, door_y(n), METATILE_SHOJI_OUVERT, false)
                .await;
        }
    }
}

// ---- Sc01, the staring duel ----

/// One samurai of the Allée des Regards.
struct Samurai {
    obj: u8,
    trainer: TrainerId,
    intro: TextId,
    defeat: TextId,
    /// The way the player faces to talk to him from the alley.
    side: Dir,
}

/// Musashi's niche is east of (10, 25), Kojiro's west of (10, 22).
fn samurai_at(y: i16) -> Option<Samurai> {
    match y {
        25 => Some(Samurai {
            obj: LOCALID_GYM06_MUSASHI,
            trainer: TRAINER_SIL_SAMOURAI_MUSASHI,
            intro: Gym_Sc01_T1_MUSASHI,
            defeat: Gym_Sc01_D_MUSASHI,
            side: Dir::Right,
        }),
        22 => Some(Samurai {
            obj: LOCALID_GYM06_KOJIRO,
            trainer: TRAINER_SIL_SAMOURAI_KOJIRO,
            intro: Gym_Sc01_T1_KOJIRO,
            defeat: Gym_Sc01_D_KOJIRO,
            side: Dir::Left,
        }),
        _ => None,
    }
}

/// Manga_Square_Gym_Sc01, the staring duel: the cell in front of each samurai on the alley,
/// (10, 25) for Musashi and (10, 22) for Kojiro (coord events, `VAR_TEMP_1` 0, every step), or
/// the samurai talked to (from that same cell). Once beaten, the coord does nothing and the
/// samurai says his defeat line.
///
/// `wait_input_or_timeout(180)`: a button before the gong is a blink (T3), else T4.
/// A blink is the source's `set_battle_malus(PLAYER_LEAD, SPEED, -1)`: the same samurai under
/// `rules::PETALS_BLINK` (the petals and the lead's Speed −1); no blink is his normal battle.
/// `TRAINER_SIL_SAMOURAI_<NOM>_HATIF` (+2 levels), the source's fallback for the malus, is no
/// longer fought (author question: drop it, or keep it as well?). Either win marks the samurai beaten (his normal
/// trainer flag). (`Gym_Sc01_Repli_Q`, the yes/no of the fallback without the timed wait, is
/// no longer said.)
pub async fn MangaSquare_Gym_Sc01(ctx: Ctx) {
    let (_, y) = ctx.player_pos().await;
    let Some(s) = samurai_at(y) else {
        return;
    };
    let talked = ctx.player_facing().await == s.side;
    if defeated(&ctx, s.trainer).await {
        if talked {
            msgbox(&ctx, s.defeat, MsgBox::Npc).await;
        }
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, s.obj, FACE_PLAYER).await;
    ctx.turn(PLAYER, s.side).await;
    say(&ctx, s.intro).await;
    say(&ctx, Gym_Sc01_T2).await;
    let blinked = ctx.wait_input_or_timeout(180).await;
    // The gong (the source's SE_GONG, not in Gen3): the Battle Arena's judge.
    play_se(&ctx, "SE_ARENA_TIMEUP1").await;
    let outcome = if blinked {
        say(&ctx, Gym_Sc01_T3).await;
        battle_trainer_with(&ctx, s.trainer, s.defeat, false, &WITH_PETALS_BLINK).await
    } else {
        say(&ctx, Gym_Sc01_T4).await;
        battle_trainer(&ctx, s.trainer, s.defeat).await
    };
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(trainer_flag(s.trainer), true).await;
    }
    ctx.release_all().await;
}

const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

// ---- Sc02, the scroll quiz ----

/// Whether the player has read the answer to question `n` in town (the quiz's sources, §3
/// Manga_Square_Gym_Sc02's table).
async fn knows(ctx: &Ctx, n: u16) -> bool {
    let flag = match n {
        1 => FLAG_SIL_GYM06_LU_Q1,
        2 => FLAG_SIL_GYM06_LU_Q2,
        3 => FLAG_SIL_GYM06_LU_Q3,
        4 => FLAG_SIL_CONTE_COQ_LU,
        _ => FLAG_SIL_GYM06_LU_Q5,
    };
    ctx.flag(flag).await
}

const QUESTIONS: [TextId; 5] = [
    Gym_Sc02_Q1,
    Gym_Sc02_Q2,
    Gym_Sc02_Q3,
    Gym_Sc02_Q4,
    Gym_Sc02_Q5,
];

/// The entrance cell a wrong answer drops the player on (one row above the arrow warp).
const ENTREE: (i16, i16) = (10, 28);

/// Manga_Square_Gym_Sc02, the scrolls of history: the five desks (9, 21), (9, 17), (9, 13),
/// (9, 9) and (9, 5), each before its closed shoji door. The desk is the cell the player faces.
/// A right answer opens the door (`FLAG_SIL_GYM06_PORTE_<n>`; the fifth sets `VAR_SIL_STORY`
/// 65); a wrong one drops the player back at the entrance. An open door's desk is silent.
///
/// fallback: `multichoice(Gym_Sc02_Q<n>_Choix)` (§9): the menu and its option texts do not exist
/// (the source's own fallback, three yes/no on the options, has no texts either). The answer is
/// the player's knowledge instead: right iff the question's source was read in town
/// (`FLAG_SIL_GYM06_LU_Q1`, `_Q2`, `_Q3`, `FLAG_SIL_CONTE_COQ_LU`, `FLAG_SIL_GYM06_LU_Q5`).
pub async fn MangaSquare_Gym_Sc02(ctx: Ctx) {
    let (x, y) = ctx.player_pos().await;
    let (dx, dy) = match ctx.player_facing().await {
        Dir::Up => (0, -1),
        Dir::Down => (0, 1),
        Dir::Left => (-1, 0),
        Dir::Right => (1, 0),
    };
    let desk_y = y + dy;
    if x + dx != 9 || !(5..=21).contains(&desk_y) || (25 - desk_y) % 4 != 0 {
        return;
    }
    let n = ((25 - desk_y) / 4) as u16;
    let porte = PORTES[usize::from(n - 1)];
    if ctx.flag(porte).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, QUESTIONS[usize::from(n - 1)]).await;
    fallback(&ctx, "multichoice").await;
    if knows(&ctx, n).await {
        say(&ctx, Gym_Sc02_Juste).await;
        // The source's SE_SHOJI (not in Gen3): Emerald's sliding door.
        play_se(&ctx, "SE_SLIDING_DOOR").await;
        ctx.set_metatile(10, door_y(n), METATILE_SHOJI_OUVERT, false)
            .await;
        ctx.redraw_map().await;
        ctx.set_flag(porte, true).await;
        if n == 5 && story(&ctx).await < 65 {
            set_story(&ctx, 65).await;
        }
        ctx.release().await;
    } else {
        say(&ctx, Gym_Sc02_Faux).await;
        // The warp ends the script (and its lock).
        warp(&ctx, MAP_MANGA_SQUARE_GYM, ENTREE.0, ENTREE.1).await;
    }
}

// ---- Sc03, Tak ----

/// Manga_Square_Gym_Sc03, Tak, the Keeper of the Code: talking to him at `VAR_SIL_STORY` 65
/// (the five doors open). The badge, TM40 (Aerial Ace), the Charizardite Y, the Mallette at 60%
/// and LOG_06. Sets 66. Afterwards, his reaction to the log.
pub async fn MangaSquare_Gym_Sc03(ctx: Ctx) {
    let s = story(&ctx).await;
    if s >= 121 {
        super::ch12_revanches::revanche(&ctx, &super::ch12_revanches::TAK).await;
        return;
    }
    if s != 65 {
        if s > 65 {
            msgbox(&ctx, Gym_Sc03_T11, MsgBox::Npc).await;
        }
        return;
    }
    ctx.lock_all().await;
    move_wait(&ctx, LOCALID_GYM06_TAK, FACE_PLAYER).await;
    play_music(&ctx, "MUS_RG_ENCOUNTER_GYM_LEADER").await;
    say(&ctx, Gym_Sc03_T1).await;
    say(&ctx, Gym_Sc03_T2).await;
    say(&ctx, Gym_Sc03_T3).await;
    // The petals (`WEATHER_PETALS`, §4) are every sanctuary battle's field rule
    // (`silice::rules::PETALS`, not a weather). Charizard Mega Evolves (the Charizardite X):
    // Gym_Mega_T1 is Tak's line before it; T2 and T3 are the engine's own Mega messages.
    battle_trainer(&ctx, TRAINER_SIL_TAK_1, Gym_Sc03_D).await;
    say(&ctx, Gym_Sc03_T4).await;
    say(&ctx, Gym_Sc03_T5).await;
    say(&ctx, Gym_Sc03_T6).await;
    play_fanfare(&ctx, "MUS_OBTAIN_BADGE").await;
    say(&ctx, Gym_Sc03_T7).await;
    ctx.set_flag(FLAG_SIL_BADGE_06, true).await;
    say(&ctx, Gym_Sc03_T8).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_GYM06_TM_AEROPIQUE,
        ITEM_TM40,
        1,
        Obtain::Message,
    )
    .await;
    say(&ctx, Gym_Sc03_T9).await;
    say(&ctx, Gym_Sc03_T9b).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_GYM06_CHARIZARDITE_Y,
        ITEM_SIL_CHARIZARDITE_Y,
        1,
        Obtain::Message,
    )
    .await;
    say(&ctx, Gym_Sc03_T9c).await;
    say(&ctx, Gym_Sc03_T10).await;
    ctx.set_var(VAR_SIL_SYNC, 60).await;
    // fallback: show_sync (the Mallette's gauge screen): T10 gives the value.
    show_sync(&ctx, 60).await;
    ctx.set_flag(FLAG_SIL_LOG_06, true).await;
    // The log (`msgbox(Gym_Sc03_Log06, Sign)`, read at once): the log as a message, as the Mallette's RAPPORTS reads it again, inside the
    // scene's lock.
    say(&ctx, Gym_Sc03_Log06).await;
    say(&ctx, Gym_Sc03_T11).await;
    set_story(&ctx, 66).await;
    ctx.release_all().await;
}

// ---- trainers and the guide ----

/// The Kunoichi on Seiji's gravel (12, 10), facing left, sight 4.
pub async fn MangaSquare_Gym_EventScript_Ayame(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM06_AYAME,
        TRAINER_SIL_KUNOICHI_AYAME,
        Gym_Ayame_Intro,
        Gym_Ayame_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym_Ayame_Defaite).await;
    }
}

/// The scribe monk (7, 6), sight 3.
pub async fn MangaSquare_Gym_EventScript_Seiji(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_GYM06_SEIJI,
        TRAINER_SIL_MOINE_SEIJI,
        Gym_Seiji_Intro,
        Gym_Seiji_Defaite,
    )
    .await
    {
        post_battle(&ctx, Gym_Seiji_Defaite).await;
    }
}

pub async fn MangaSquare_Gym_EventScript_Guide(ctx: Ctx) {
    msgbox(&ctx, Gym_Guide, MsgBox::Npc).await;
}
