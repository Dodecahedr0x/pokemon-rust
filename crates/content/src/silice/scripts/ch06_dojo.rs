//! Chapter 6, the Grand Dojo (`MangaSquare_Dojo`) and the Bibliothèque Ancestrale
//! (`MangaSquare_Bibliotheque`): `contenu-jeu/chapitres/06-manga-square.md` §2 (the stands, the
//! monk, the code scroll, the lore books), §3 Manga_Square_Dojo_Sc01 to Sc04 and
//! Manga_Square_Bibliotheque_Sc01 to Sc02, and §7 (the shops). `VAR_SIL_STORY` runs 62 (the
//! signing queue) to 64 (the tale read, the gym open).
//!
//! The gym quiz's sources (`ch06_gym.rs`): reading a book sets its question's read flag
//! (Livre 1 → `FLAG_SIL_GYM06_LU_Q1`, Livre 2 → `_Q2`, Livre 3 → `_Q3`, the code scroll →
//! `_Q5`; Q4 is the tale, `FLAG_SIL_CONTE_COQ_LU`).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["MangaSquare_Dojo", "MangaSquare_Bibliotheque"];

// ---- the Dojo ----

/// The back door's monk stands aside from `VAR_SIL_STORY` 64 (the tale read): to (28, 5),
/// facing left, off the alcove's only way in (27, 4) and (27, 5). Gen3 `setobjectxyperm`
/// before the objects spawn.
pub async fn MangaSquare_Dojo_OnTransition(ctx: Ctx) {
    if story(&ctx).await >= 64 {
        ctx.set_object_template_pos(LOCALID_DOJO_MOINE, 28, 5).await;
        ctx.set_movement_type(LOCALID_DOJO_MOINE, MovementType::Face(Dir::Left))
            .await;
    }
}

/// The cell the player stands on for the signing scene: above the queue, between Adrien
/// (13, 10) and Kevin (15, 10). Each fan steps in below it (14, 10), facing up.
const QUEUE_X: i16 = 14;
const QUEUE_Y: i16 = 9;

/// The player joins (14, 9) along row 9 (free: the fans and Adrien are on row 10, the
/// ribbon on row 11, the stage from row 8 up and x 18). From row 10 (beside Adrien or a fan),
/// one step up first.
async fn join_queue(ctx: &Ctx) {
    let (x, y) = ctx.player_pos().await;
    if y > QUEUE_Y {
        move_wait(ctx, PLAYER, UP).await;
    }
    for _ in x..QUEUE_X {
        move_wait(ctx, PLAYER, RIGHT).await;
    }
    for _ in QUEUE_X..x {
        move_wait(ctx, PLAYER, LEFT).await;
    }
    ctx.turn(PLAYER, Dir::Down).await;
}

const UP: &[Step] = &[walk(Dir::Up)];
const LEFT: &[Step] = &[walk(Dir::Left)];
const RIGHT: &[Step] = &[walk(Dir::Right)];
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// One fan of the queue: steps in below the player (the source's `Walk(Left)`, from its cell to
/// (14, 10)), faces up, fights, then leaves the queue for row 8 (the source's `Walk(Up) x2`, by
/// the cells right of the player). A fan already beaten (a whiteout on a later one: the scene
/// restarts at the next) only leaves.
struct Fan {
    obj: u8,
    trainer: TrainerId,
    intro: TextId,
    defeat: TextId,
    step_in: &'static [Step],
    step_out: &'static [Step],
    /// From its own cell, for a fan already beaten.
    leave: &'static [Step],
}

const FANS: [Fan; 3] = [
    Fan {
        obj: LOCALID_DOJO_FAN_KEVIN,
        trainer: TRAINER_SIL_FAN_KEVIN,
        intro: Manga_Square_Dojo_Sc01_T3,
        defeat: Manga_Square_Dojo_Sc01_D1,
        step_in: &[walk(Dir::Left), face(Dir::Up)],
        step_out: &[walk(Dir::Right), walk(Dir::Up), walk(Dir::Up)],
        leave: &[walk(Dir::Up), walk(Dir::Up)],
    },
    Fan {
        obj: LOCALID_DOJO_FAN_BRUNO,
        trainer: TRAINER_SIL_FAN_BRUNO,
        intro: Manga_Square_Dojo_Sc01_T4,
        defeat: Manga_Square_Dojo_Sc01_D2,
        step_in: &[walk(Dir::Left), walk(Dir::Left), face(Dir::Up)],
        step_out: &[
            walk(Dir::Right),
            walk(Dir::Right),
            walk(Dir::Up),
            walk(Dir::Up),
        ],
        leave: &[walk(Dir::Up), walk(Dir::Up)],
    },
    Fan {
        obj: LOCALID_DOJO_FAN_DIANE,
        trainer: TRAINER_SIL_FAN_DIANE,
        intro: Manga_Square_Dojo_Sc01_T5,
        defeat: Manga_Square_Dojo_Sc01_D3,
        step_in: &[
            walk(Dir::Left),
            walk(Dir::Left),
            walk(Dir::Left),
            face(Dir::Up),
        ],
        step_out: &[
            walk(Dir::Right),
            walk(Dir::Right),
            walk(Dir::Right),
            walk(Dir::Up),
            walk(Dir::Up),
        ],
        leave: &[walk(Dir::Up), walk(Dir::Up)],
    },
];

/// The queue is free: Adrien walks to its head (13, 10) → (17, 10), the player follows on row 9
/// (14, 9) → (18, 9), by the stage.
const ADRIEN_TO_STAGE: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    face(Dir::Right),
];
const PLAYER_TO_STAGE: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
];
/// Adrien leaves for the Bibliothèque: past the ribbon's east end (19, 11) and down towards the
/// exit (the source's `Walk(Down) x6`).
const ADRIEN_OUT: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
    walk(Dir::Down),
];

/// Manga_Square_Dojo_Sc01, the poet and the waifu cushion: talking to Adrien in the queue (or to
/// one of its fans) at `VAR_SIL_STORY` 62. The three fans in a row (no heal between them; each
/// beaten fan stays beaten), Kimiko on the stage, TM08 (Bulk Up). Adrien leaves for the
/// Bibliothèque (`FLAG_SIL_HIDE_DOJO_ADRIEN` set, `_HIDE_BIBLIO_ADRIEN` cleared). Sets 63.
pub async fn MangaSquare_Dojo_Sc01(ctx: Ctx) {
    if story(&ctx).await != 62 {
        // After the scene Adrien is gone; before it (61) the square's scene runs first.
        return;
    }
    ctx.lock_all().await;
    join_queue(&ctx).await;
    move_wait(&ctx, LOCALID_DOJO_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T1).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T2).await;
    for fan in &FANS {
        if defeated(&ctx, fan.trainer).await {
            move_wait(&ctx, fan.obj, fan.leave).await;
            continue;
        }
        move_wait(&ctx, fan.obj, fan.step_in).await;
        say(&ctx, fan.intro).await;
        // A loss whites out: the script ends here and the scene restarts at this fan.
        battle_trainer(&ctx, fan.trainer, fan.defeat).await;
        if fan.trainer == TRAINER_SIL_FAN_DIANE {
            say(&ctx, Manga_Square_Dojo_Sc01_T6).await;
        }
        move_wait(&ctx, fan.obj, fan.step_out).await;
    }
    ctx.move_obj(LOCALID_DOJO_ADRIEN, ADRIEN_TO_STAGE).await;
    move_wait(&ctx, PLAYER, PLAYER_TO_STAGE).await;
    ctx.wait_move(LOCALID_DOJO_ADRIEN).await;
    move_wait(&ctx, LOCALID_DOJO_KIMIKO, FACE_PLAYER).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T7).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T8).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T9).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T10).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T11).await;
    // The source's `Walk(Left) x2, FacePlayer`: Adrien is already beside the player (17, 10).
    move_wait(&ctx, LOCALID_DOJO_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Manga_Square_Dojo_Sc01_T12).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_DOJO_TM_GONFLETTE,
        ITEM_TM08,
        1,
        Obtain::Message,
    )
    .await;
    say(&ctx, Manga_Square_Dojo_Sc01_T13).await;
    move_wait(&ctx, LOCALID_DOJO_ADRIEN, ADRIEN_OUT).await;
    // `hide` sets FLAG_SIL_HIDE_DOJO_ADRIEN.
    hide(&ctx, LOCALID_DOJO_ADRIEN).await;
    ctx.set_flag(FLAG_SIL_HIDE_BIBLIO_ADRIEN, false).await;
    set_story(&ctx, 63).await;
    ctx.release_all().await;
}

/// A fan of the queue: the signing scene at 62 (the source's trigger is Adrien; talking to the
/// barrier is the same request), then only its defeat line.
async fn queue_fan(ctx: Ctx, line: TextId) {
    if story(&ctx).await == 62 {
        MangaSquare_Dojo_Sc01(ctx).await;
    } else {
        msgbox(&ctx, line, MsgBox::Npc).await;
    }
}

pub async fn MangaSquare_Dojo_EventScript_Kevin(ctx: Ctx) {
    queue_fan(ctx, Manga_Square_Dojo_Sc01_D1).await;
}

pub async fn MangaSquare_Dojo_EventScript_Bruno(ctx: Ctx) {
    queue_fan(ctx, Manga_Square_Dojo_Sc01_D2).await;
}

pub async fn MangaSquare_Dojo_EventScript_Diane(ctx: Ctx) {
    queue_fan(ctx, Manga_Square_Dojo_Sc01_D3).await;
}

/// Kimiko on the stage, before the signing: her stream's line (the source gives her no
/// line outside Manga_Square_Dojo_Sc01).
pub async fn MangaSquare_Dojo_EventScript_Kimiko(ctx: Ctx) {
    msgbox(&ctx, Manga_Square_Sc01_T2, MsgBox::Npc).await;
}

/// Manga_Square_Dojo_Sc02, the fan who missed the signing: TM21 (Frustration) once, from
/// `VAR_SIL_STORY` 63. Before it, he only turns to the player (the source gives no line).
pub async fn MangaSquare_Dojo_Sc02(ctx: Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if story(&ctx).await >= 63 {
        if !ctx.flag(FLAG_SIL_ITEM_DOJO_TM_FRUSTRATION).await {
            say(&ctx, Dojo_Sc02_T1).await;
            gift(
                &ctx,
                FLAG_SIL_ITEM_DOJO_TM_FRUSTRATION,
                ITEM_TM21,
                1,
                Obtain::Message,
            )
            .await;
        }
        say(&ctx, Dojo_Sc02_T2).await;
    }
    ctx.release().await;
}

/// Manga_Square_Dojo_Sc03, Kenta on the training mat (sight 3): then the Black Belt once.
pub async fn MangaSquare_Dojo_Sc03(ctx: Ctx) {
    if !trainer(
        &ctx,
        LOCALID_DOJO_KENTA,
        TRAINER_SIL_CEINTURE_KENTA,
        Dojo_Kenta_Intro,
        Dojo_Kenta_Defaite,
    )
    .await
    {
        return;
    }
    ctx.lock().await;
    ctx.face_player().await;
    if !ctx.flag(FLAG_SIL_ITEM_DOJO_BLACK_BELT).await {
        say(&ctx, Dojo_Sc03_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_DOJO_BLACK_BELT,
            ITEM_BLACK_BELT,
            1,
            Obtain::Message,
        )
        .await;
    } else {
        say(&ctx, Dojo_Sc03_T2).await;
    }
    ctx.release().await;
}

/// The food stand's drinks (§7 "Stand Bouffe").
const STAND_BOUFFE: &[ItemId] = &[
    ItemId(ITEM_FRESH_WATER),
    ItemId(ITEM_SODA_POP),
    ItemId(ITEM_LEMONADE),
];

/// Manga_Square_Dojo_Sc04, the food stand: a Nanab Berry once, then the drinks.
pub async fn MangaSquare_Dojo_Sc04(ctx: Ctx) {
    if !ctx.flag(FLAG_SIL_ITEM_DOJO_NANAB).await {
        ctx.lock().await;
        ctx.face_player().await;
        say(&ctx, Dojo_Sc04_T1).await;
        gift(
            &ctx,
            FLAG_SIL_ITEM_DOJO_NANAB,
            ITEM_NANAB_BERRY,
            1,
            Obtain::Message,
        )
        .await;
        ctx.release().await;
    } else {
        msgbox(&ctx, Dojo_Sc04_T2, MsgBox::Npc).await;
        // fallback: open_shop(SHOP_DOJO_BOUFFE) (M9's shop screen): `mart`'s frame.
        mart(&ctx, STAND_BOUFFE).await;
    }
}

/// Shinobi-Gear's stock (§7).
const SHINOBI_GEAR: &[ItemId] = &[
    ItemId(ITEM_FOCUS_BAND),
    ItemId(ITEM_SMOKE_BALL),
    ItemId(ITEM_QUICK_CLAW),
];

/// The Shinobi-Gear vendor: his line, then the shop.
pub async fn MangaSquare_Dojo_EventScript_ShinobiGear(ctx: Ctx) {
    msgbox(&ctx, Dojo_ShinobiGear, MsgBox::Npc).await;
    // fallback: mart (M9's shop screen).
    mart(&ctx, SHINOBI_GEAR).await;
}

pub async fn MangaSquare_Dojo_EventScript_Restauratrice(ctx: Ctx) {
    msgbox(&ctx, Dojo_Npc_Restauratrice, MsgBox::Npc).await;
}

pub async fn MangaSquare_Dojo_EventScript_Cosplay1(ctx: Ctx) {
    msgbox(&ctx, Dojo_Npc_Cosplay_1, MsgBox::Npc).await;
}

pub async fn MangaSquare_Dojo_EventScript_Cosplay2(ctx: Ctx) {
    msgbox(&ctx, Dojo_Npc_Cosplay_2, MsgBox::Npc).await;
}

/// The back door's monk: the gym is open from `VAR_SIL_STORY` 64 (OnTransition moves him).
pub async fn MangaSquare_Dojo_EventScript_Moine(ctx: Ctx) {
    let text = if story(&ctx).await < 64 {
        Dojo_Npc_Moine_Bloque
    } else {
        Dojo_Npc_Moine_Ouvert
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// The code scroll: the quiz's Q5 (the third virtue).
pub async fn MangaSquare_Dojo_EventScript_SignCode(ctx: Ctx) {
    msgbox(&ctx, Dojo_Sign_Code, MsgBox::Sign).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q5, true).await;
}

// ---- the Bibliothèque ----

/// Manga_Square_Bibliotheque_Sc01, the tale of the Dawn Rooster: the four cells two rows in
/// front of Adrien's desk (12–15, 7), or Adrien himself, at `VAR_SIL_STORY` 63. Both editions of
/// the tale; Kimiko and her drones leave the Dojo. Sets 64 (the gym's monk lets the player
/// through). Afterwards Adrien repeats his last line.
pub async fn MangaSquare_Bibliotheque_Sc01(ctx: Ctx) {
    let s = story(&ctx).await;
    if s > 63 {
        msgbox(&ctx, Biblio_Sc01_T7, MsgBox::Npc).await;
        return;
    }
    if s != 63 {
        return;
    }
    ctx.lock_all().await;
    ctx.turn(LOCALID_BIBLIO_ADRIEN, Dir::Down).await;
    say(&ctx, Biblio_Sc01_T1).await;
    say(&ctx, Biblio_Sc01_T2).await;
    // fallback: show_book (§9, a page on a parchment background): the source's own fallback,
    // the tale in the message box.
    fallback(&ctx, "show_book").await;
    say(&ctx, Biblio_Conte_Ecran).await;
    say(&ctx, Biblio_Sc01_T3).await;
    say(&ctx, Biblio_Conte_Papier).await;
    say(&ctx, Biblio_Sc01_T4).await;
    say(&ctx, Biblio_Sc01_T5).await;
    // The servers' hum (the source's SE_SERVEURS, not in Gen3): the PC's start-up sound.
    play_se(&ctx, "SE_PC_ON").await;
    say(&ctx, Biblio_Sc01_T6).await;
    say(&ctx, Biblio_Sc01_T7).await;
    ctx.set_flag(FLAG_SIL_CONTE_COQ_LU, true).await;
    // The signing is over: Kimiko and her drones leave the Dojo together (§8).
    ctx.set_flag(FLAG_SIL_HIDE_DOJO_KIMIKO, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_DOJO_DRONE_1, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_DOJO_DRONE_2, true).await;
    set_story(&ctx, 64).await;
    ctx.release_all().await;
}

/// History: the Pact of Closure, twenty years ago (the quiz's Q1).
pub async fn MangaSquare_Bibliotheque_EventScript_Livre1(ctx: Ctx) {
    msgbox(&ctx, Biblio_Livre_1, MsgBox::Sign).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q1, true).await;
}

/// Geography: the Dome's sectors and the town's old name (Q2).
pub async fn MangaSquare_Bibliotheque_EventScript_Livre2(ctx: Ctx) {
    msgbox(&ctx, Biblio_Livre_2, MsgBox::Sign).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q2, true).await;
}

/// Legends: the Wishing Dragon (Q3).
pub async fn MangaSquare_Bibliotheque_EventScript_Livre3(ctx: Ctx) {
    msgbox(&ctx, Biblio_Livre_3, MsgBox::Sign).await;
    ctx.set_flag(FLAG_SIL_GYM06_LU_Q3, true).await;
}

/// Pokémon: the trainers of old (no question).
pub async fn MangaSquare_Bibliotheque_EventScript_Livre4(ctx: Ctx) {
    msgbox(&ctx, Biblio_Livre_4, MsgBox::Sign).await;
}

pub async fn MangaSquare_Bibliotheque_EventScript_Borne(ctx: Ctx) {
    msgbox(&ctx, Biblio_Borne, MsgBox::Sign).await;
}

pub async fn MangaSquare_Bibliotheque_EventScript_Archiviste(ctx: Ctx) {
    msgbox(&ctx, Biblio_Npc_Archiviste, MsgBox::Npc).await;
}

pub async fn MangaSquare_Bibliotheque_EventScript_Prestataire(ctx: Ctx) {
    msgbox(&ctx, Biblio_Npc_Prestataire, MsgBox::Npc).await;
}

pub async fn MangaSquare_Bibliotheque_EventScript_Etudiant(ctx: Ctx) {
    msgbox(&ctx, Biblio_Npc_Etudiant, MsgBox::Npc).await;
}

