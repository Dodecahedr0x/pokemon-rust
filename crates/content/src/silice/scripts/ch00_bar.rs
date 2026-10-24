//! Chapter 0, the Shaker (`BricoBourg_Bar`): `contenu-jeu/chapitres/00-prologue.md` §2 (the bar,
//! its ambient NPCs) and §3 Bar_Sc01 to Bar_Sc04. `VAR_SIL_STORY` runs 0 (the evening) to 1 (the
//! Mallette to take).

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BricoBourg_Bar"];

// ---- map scripts ----

/// Object visibility by story step (§8 "Visibilité"): the friends while `VAR_SIL_STORY` ≤ 1,
/// Julien at 0, the Mallette at 1 until taken; the sbires only inside Bar_Sc03.
pub async fn BricoBourg_Bar_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    for f in [
        FLAG_SIL_HIDE_BAR_MATHIEU,
        FLAG_SIL_HIDE_BAR_GWADA,
        FLAG_SIL_HIDE_BAR_ADRIEN,
    ] {
        ctx.set_flag(f, s > 1).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_BAR_JULIEN, s != 0).await;
    let mallette = s == 1 && !ctx.flag(FLAG_SIL_HAS_MALLETTE).await;
    ctx.set_flag(FLAG_SIL_HIDE_BAR_MALLETTE, !mallette).await;
    ctx.set_flag(FLAG_SIL_HIDE_BAR_SBIRE_A, true).await;
    ctx.set_flag(FLAG_SIL_HIDE_BAR_SBIRE_B, true).await;
}

// ---- scenes ----

const FACE_DOWN: &[Step] = &[face(Dir::Down)];
const FACE_RIGHT: &[Step] = &[face(Dir::Right)];

/// Bar_Sc01, the opening: on the bar's first frame (`on_frame` while `VAR_TEMP_1` is 0), at
/// `VAR_SIL_STORY` 0 before `FLAG_SIL_INTRO_DONE`. The new game has already faded in and put
/// the player on the stool facing the counter (`game.ron`), so the source's `fade(In)` and
/// turn are left out.
pub async fn BricoBourg_Bar_Sc01(ctx: Ctx) {
    // The on-frame entry re-runs while its var matches: leave it on every path.
    ctx.set_var(VAR_TEMP_1, 1).await;
    if story(&ctx).await != 0 || ctx.flag(FLAG_SIL_INTRO_DONE).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Bar_Sc01_T1).await;
    move_wait(&ctx, LOCALID_BAR_CROW, FACE_DOWN).await;
    say(&ctx, Bar_Sc01_T2).await;
    say(&ctx, Bar_Sc01_T3).await;
    move_wait(&ctx, LOCALID_BAR_ADRIEN, FACE_RIGHT).await;
    say(&ctx, Bar_Sc01_T4).await;
    ctx.set_flag(FLAG_SIL_INTRO_DONE, true).await;
    ctx.release_all().await;
}

/// Bar_Sc02, the friends (talk to MATHIEU, GWADA or ADRIEN): one line each at `VAR_SIL_STORY`
/// 0. At 1 (not in the source) they repeat Bar_Sc03_T6 ("take it") until the Mallette is
/// taken, then Bar_Sc04_T4 ("run").
pub async fn BricoBourg_Bar_Sc02(ctx: Ctx) {
    let text = if story(&ctx).await == 0 {
        match ctx.var(VAR_LAST_TALKED).await as u8 {
            LOCALID_BAR_MATHIEU => Bar_Sc02_T1,
            LOCALID_BAR_GWADA => Bar_Sc02_T2,
            _ => Bar_Sc02_T3,
        }
    } else if ctx.flag(FLAG_SIL_HAS_MALLETTE).await {
        Bar_Sc04_T4
    } else {
        Bar_Sc03_T6
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// Julien's way to the back door (12, 1): one step up beside it, facing it.
const JULIEN_TO_DOOR: &[Step] = &[walk(Dir::Up), face(Dir::Up)];
/// The player out of Julien's way when talking to him from (12, 2).
const PLAYER_ASIDE: &[Step] = &[walk(Dir::Left), face(Dir::Right)];
/// The sbires in from the blown-open entrance (6–7, 9).
const SBIRE_IN: &[Step] = &[walk(Dir::Up), walk(Dir::Up), walk(Dir::Up)];
/// The chase toward the back door, re-derived for the built bar (the source's `Walk(Up) x3/x4,
/// Walk(Right) x6`): along y 5, then up x 10, B ahead of A, clear of every cell the player can
/// talk to Julien from ((11, 3), (12, 2), (12, 4)).
const SBIRE_A_CHASE: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
];
const SBIRE_B_CHASE: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
];

/// Bar_Sc03, the stranger and the explosion (talk to JULIEN, `VAR_SIL_STORY` 0): the sbires
/// burst in, Julien leaves by the back door, the Mallette stays. Sets `VAR_SIL_STORY` 1.
pub async fn BricoBourg_Bar_Sc03(ctx: Ctx) {
    if story(&ctx).await != 0 {
        return;
    }
    ctx.lock_all().await;
    ctx.face_player().await;
    say(&ctx, Bar_Sc03_T1).await;
    ctx.delay(30).await;
    screen_shake(&ctx, 8, 20).await;
    // fallback: play_se (audio, M12): a no-op that reports; the source's SE_EXPLOSION is the
    // decomp's SE_M_EXPLOSION. BOUM is in T2.
    play_se(&ctx, "SE_M_EXPLOSION").await;
    say(&ctx, Bar_Sc03_T2).await;
    show(&ctx, LOCALID_BAR_SBIRE_A, FLAG_SIL_HIDE_BAR_SBIRE_A).await;
    show(&ctx, LOCALID_BAR_SBIRE_B, FLAG_SIL_HIDE_BAR_SBIRE_B).await;
    ctx.move_obj(LOCALID_BAR_SBIRE_A, SBIRE_IN).await;
    move_wait(&ctx, LOCALID_BAR_SBIRE_B, SBIRE_IN).await;
    say(&ctx, Bar_Sc03_T3).await;
    if ctx.player_pos().await == (12, 2) {
        move_wait(&ctx, PLAYER, PLAYER_ASIDE).await;
    }
    move_wait(&ctx, LOCALID_BAR_JULIEN, JULIEN_TO_DOOR).await;
    say(&ctx, Bar_Sc03_T4).await;
    hide(&ctx, LOCALID_BAR_JULIEN).await;
    ctx.move_obj(LOCALID_BAR_SBIRE_A, SBIRE_A_CHASE).await;
    move_wait(&ctx, LOCALID_BAR_SBIRE_B, SBIRE_B_CHASE).await;
    ctx.wait_move(LOCALID_BAR_SBIRE_A).await;
    hide(&ctx, LOCALID_BAR_SBIRE_A).await;
    hide(&ctx, LOCALID_BAR_SBIRE_B).await;
    ctx.delay(40).await;
    show(&ctx, LOCALID_BAR_MALLETTE, FLAG_SIL_HIDE_BAR_MALLETTE).await;
    say(&ctx, Bar_Sc03_T5).await;
    say(&ctx, Bar_Sc03_T6).await;
    set_story(&ctx, 1).await;
    ctx.release_all().await;
}

/// Bar_Sc04, picking up the Mallette (talk to the MALLETTE, `VAR_SIL_STORY` 1, not taken).
pub async fn BricoBourg_Bar_Sc04(ctx: Ctx) {
    if story(&ctx).await != 1 || ctx.flag(FLAG_SIL_HAS_MALLETTE).await {
        return;
    }
    ctx.lock_all().await;
    if !ask(&ctx, Bar_Sc04_T1).await {
        say(&ctx, Bar_Sc04_T2).await;
        ctx.release_all().await;
        return;
    }
    // The key item first, then the object: T3 says it, so quietly. Behind its flag, once.
    if !gift(
        &ctx,
        FLAG_SIL_HAS_MALLETTE,
        ITEM_SIL_MALLETTE,
        1,
        Obtain::Quiet,
    )
    .await
    {
        ctx.release_all().await;
        return;
    }
    hide(&ctx, LOCALID_BAR_MALLETTE).await;
    say(&ctx, Bar_Sc04_T3).await;
    say(&ctx, Bar_Sc04_T4).await;
    say(&ctx, Bar_Sc04_T5).await;
    ctx.release_all().await;
}

// ---- ambient NPCs ----

/// CROW behind the counter (not in the source): his Bar_Sc01 line at `VAR_SIL_STORY` 0, his
/// Bar_Sc04 advice after; from 121, Chapter 12's `Brico_Bourg_Bar_PG_Sc22` (the Lounge).
pub async fn BricoBourg_Bar_EventScript_Crow(ctx: Ctx) {
    if story(&ctx).await >= 121 {
        super::ch12_extras::crow_comptoir(&ctx).await;
        return;
    }
    let text = match story(&ctx).await {
        0 => Bar_Sc01_T2,
        _ => Bar_Sc04_T5,
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

/// The woman on the banquette.
pub async fn BricoBourg_Bar_EventScript_Cliente(ctx: Ctx) {
    msgbox(&ctx, Bar_Npc_Cliente, MsgBox::Npc).await;
}

/// The man in the armchair: his evening line, then Bar_Npc_Client_Apres once the door is gone.
pub async fn BricoBourg_Bar_EventScript_Client(ctx: Ctx) {
    let text = match story(&ctx).await {
        0 => Bar_Npc_Client,
        _ => Bar_Npc_Client_Apres,
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}
