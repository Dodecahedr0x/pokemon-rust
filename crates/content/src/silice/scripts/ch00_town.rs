//! Chapter 0, the village (`BricoBourg`): `contenu-jeu/chapitres/00-prologue.md` §2 (signs,
//! ambient NPCs, the north guard) and §3 Brico_Bourg_Sc01 to Sc04. `VAR_SIL_STORY` runs 1 (the
//! night chase) to 5 (the starter chosen); `VAR_SIL_CH0_GATE` opens the north exit.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["BricoBourg"];

// ---- map scripts ----

/// The Labo's brambles (4, 8), cut in Chapter 1 ([`BricoBourg_EventScript_Ronces`]): repainted
/// on every load once `FLAG_SIL_BRICO_LABO_RONCES_CUT` is set ("SIL2 Ch1 maps as built").
pub async fn BricoBourg_OnLoad(ctx: Ctx) {
    repaint_if(
        &ctx,
        FLAG_SIL_BRICO_LABO_RONCES_CUT,
        RONCES,
        METATILE_RONCES_COUPEES,
    )
    .await;
}

/// Object visibility by story step (§8 "Visibilité"): the day NPCs (LINGE, GAMIN, BRICOLEUSE)
/// from `VAR_SIL_STORY` 3, the friends in front of the house at 3 and 4, the sbire only inside
/// Brico_Bourg_Sc01. The old man and the fisherman stay. At night (≤ 2) the source darkens the
/// palette.
pub async fn BricoBourg_OnTransition(ctx: Ctx) {
    let s = story(&ctx).await;
    for f in [
        FLAG_SIL_HIDE_BRICO_LINGE,
        FLAG_SIL_HIDE_BRICO_GAMIN,
        FLAG_SIL_HIDE_BRICO_BRICOLEUSE,
    ] {
        ctx.set_flag(f, s < 3).await;
    }
    for f in [
        FLAG_SIL_HIDE_BRICO_MATHIEU,
        FLAG_SIL_HIDE_BRICO_GWADA,
        FLAG_SIL_HIDE_BRICO_ADRIEN,
    ] {
        ctx.set_flag(f, !(3..=4).contains(&s)).await;
    }
    ctx.set_flag(FLAG_SIL_HIDE_BRICO_SBIRE, true).await;
    if s <= 2 {
        // fallback: set_time_tint(NUIT) (`Request::SetTint`, SIL2 Wave 0 in the M9 plan): no
        // tint, and no flag-selected night map either (the maps leave it out); the village
        // keeps its day palette.
        fallback(&ctx, "set_time_tint").await;
    }
}

// ---- scenes ----

/// The sbire's steps: in from (18, 14), facing the player, then off north along x 16.
const SBIRE_IN: &[Step] = &[walk(Dir::Left), walk(Dir::Left), walk_in_place(Dir::Left)];
const SBIRE_OUT: &[Step] = &[
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];

/// Brico_Bourg_Sc01, the chase (night): the cells around the Shaker's exit (14, 14) at
/// `VAR_SIL_STORY` 1, once (`FLAG_SIL_BRICO_CHASE`).
///
/// Every way out of the Shaker crosses these cells, so they also keep the Mallette in play (not
/// in the source): without it, MATHIEU's Bar_Sc04_T2 calls the player back to the door. Else
/// the player could reach the house at story 1 empty-handed, and Player_House_Sc01 would move
/// the story past the Mallette for good.
pub async fn BricoBourg_Sc01(ctx: Ctx) {
    if story(&ctx).await != 1 {
        return;
    }
    if !ctx.flag(FLAG_SIL_HAS_MALLETTE).await {
        ctx.lock_all().await;
        say(&ctx, Bar_Sc04_T2).await;
        const FROM_WEST: &[Step] = &[walk(Dir::Right), face(Dir::Up)];
        const FROM_EAST: &[Step] = &[walk(Dir::Left), face(Dir::Up)];
        const FROM_SOUTH: &[Step] = &[walk(Dir::Up)];
        let back = match ctx.player_pos().await {
            (13, _) => FROM_WEST,
            (15, _) => FROM_EAST,
            _ => FROM_SOUTH,
        };
        move_wait(&ctx, PLAYER, back).await;
        ctx.release_all().await;
        return;
    }
    if ctx.flag(FLAG_SIL_BRICO_CHASE).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Brico_Bourg_Sc01_T1).await;
    show(&ctx, LOCALID_BRICO_SBIRE, FLAG_SIL_HIDE_BRICO_SBIRE).await;
    move_wait(&ctx, LOCALID_BRICO_SBIRE, SBIRE_IN).await;
    say(&ctx, Brico_Bourg_Sc01_T2).await;
    move_wait(&ctx, LOCALID_BRICO_SBIRE, SBIRE_OUT).await;
    hide(&ctx, LOCALID_BRICO_SBIRE).await;
    ctx.set_flag(FLAG_SIL_BRICO_CHASE, true).await;
    say(&ctx, Brico_Bourg_Sc01_T3).await;
    ctx.release_all().await;
}

/// The friends turn to the player.
const FACE_PLAYER: &[Step] = &[Step::FacePlayer];

/// Brico_Bourg_Sc02, click (the morning): on the village's first frame at `VAR_SIL_STORY` 3
/// (the map's on-frame entry: the player steps out of the house door at (5, 14), the friends
/// stand below at (4–6, 16)). The Mallette opens; sets 4, then Brico_Bourg_Sc03.
pub async fn BricoBourg_Sc02(ctx: Ctx) {
    ctx.lock_all().await;
    ctx.move_obj(LOCALID_BRICO_MATHIEU, FACE_PLAYER).await;
    ctx.move_obj(LOCALID_BRICO_GWADA, FACE_PLAYER).await;
    move_wait(&ctx, LOCALID_BRICO_ADRIEN, FACE_PLAYER).await;
    say(&ctx, Brico_Bourg_Sc02_T1).await;
    say(&ctx, Brico_Bourg_Sc02_T2).await;
    ctx.delay(30).await;
    // fallback: screen_shake (`Request::ScreenShake`, SIL2 Wave 0 in the M9 plan): a fade out
    // and in.
    screen_shake(&ctx, 2, 10).await;
    // fallback: play_se (audio, M12): a no-op that reports. CLIC is in T3.
    play_se(&ctx, "SE_CLICK").await;
    say(&ctx, Brico_Bourg_Sc02_T3).await;
    say(&ctx, Brico_Bourg_Sc02_T4).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_OPEN, true).await;
    set_story(&ctx, 4).await;
    BricoBourg_Sc03(ctx.clone()).await;
}

/// The friends' ways out of the village, re-derived for the built town (the source's
/// `Walk(Up) x8` with side steps): MATHIEU up x 3, GWADA up x 8 behind ADRIEN, ADRIEN along
/// y 16 and up x 16 (the column the night sbire took).
const MATHIEU_OUT: &[Step] = &[
    walk(Dir::Left),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];
const GWADA_OUT: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];
const ADRIEN_OUT: &[Step] = &[
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Right),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
    walk(Dir::Up),
];

/// Brico_Bourg_Sc03, Julien's call and the secret compartment (chained after Sc02,
/// `VAR_SIL_STORY` 4): the starter, the friends' gifts, everyone off to Bois-Éther. Opens the
/// north exit (`VAR_SIL_CH0_GATE` 1, with the starter), sets the Mallette's flags and 5.
pub async fn BricoBourg_Sc03(ctx: Ctx) {
    if story(&ctx).await != 4 {
        return;
    }
    // fallback: play_se (audio, M12): a no-op that reports; the source's SE_RING is the
    // decomp's SE_POKENAV_CALL. T1 says it rings.
    play_se(&ctx, "SE_POKENAV_CALL").await;
    say(&ctx, Brico_Bourg_Sc03_T1).await;
    // fallback: mallette_call (the call window with a portrait, plan §1.1 P1: the M9 start-menu
    // entry, then SIL2/SIL3 screens): the call's lines are plain message boxes.
    mallette_call(&ctx, "JULIEN").await;
    say(&ctx, Brico_Bourg_Sc03_T2).await;
    say(&ctx, Brico_Bourg_Sc03_T3).await;
    say(&ctx, Brico_Bourg_Sc03_T4).await;
    // fallback: end_call (the call window, as mallette_call): nothing to close.
    end_call(&ctx).await;
    say(&ctx, Brico_Bourg_Sc03_T5).await;
    say(&ctx, Brico_Bourg_Sc03_T6).await;
    choose_starter(&ctx).await;
    // The north guard (BricoBourg_EventScript_BlockNorth) lets the player through from now on.
    ctx.set_var(VAR_SIL_CH0_GATE, 1).await;
    say(&ctx, Brico_Bourg_Sc03_T7).await;
    say(&ctx, Brico_Bourg_Sc03_T8).await;
    say(&ctx, Brico_Bourg_Sc03_T9).await;
    say(&ctx, Brico_Bourg_Sc03_T10).await;
    // The friends' gifts, once each, before they leave; T11 and T12 announce them.
    gift(
        &ctx,
        FLAG_SIL_ITEM_PROLOGUE_POTIONS,
        ITEM_POTION,
        2,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Brico_Bourg_Sc03_T11).await;
    gift(
        &ctx,
        FLAG_SIL_ITEM_PROLOGUE_BALLS,
        ITEM_POKE_BALL,
        5,
        Obtain::Quiet,
    )
    .await;
    say(&ctx, Brico_Bourg_Sc03_T12).await;
    ctx.move_obj(LOCALID_BRICO_MATHIEU, MATHIEU_OUT).await;
    ctx.move_obj(LOCALID_BRICO_GWADA, GWADA_OUT).await;
    move_wait(&ctx, LOCALID_BRICO_ADRIEN, ADRIEN_OUT).await;
    ctx.wait_move(LOCALID_BRICO_MATHIEU).await;
    ctx.wait_move(LOCALID_BRICO_GWADA).await;
    hide(&ctx, LOCALID_BRICO_MATHIEU).await;
    hide(&ctx, LOCALID_BRICO_GWADA).await;
    hide(&ctx, LOCALID_BRICO_ADRIEN).await;
    say(&ctx, Brico_Bourg_Sc03_T13).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_MAP, true).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_DEX, true).await;
    ctx.set_flag(FLAG_SIL_MALLETTE_CALLS, true).await;
    set_story(&ctx, 5).await;
    ctx.release_all().await;
}

/// Brico_Bourg_Sc04, the signal on the map: the north exit row (x 10–13, y 1) at
/// `VAR_SIL_STORY` 5, once (`FLAG_SIL_S01_SIGNAL_SEEN`).
pub async fn BricoBourg_Sc04(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_S01_SIGNAL_SEEN).await {
        return;
    }
    ctx.lock().await;
    say(&ctx, Brico_Bourg_Sc04_T1).await;
    ctx.set_flag(FLAG_SIL_S01_SIGNAL_SEEN, true).await;
    ctx.release().await;
}

/// The north guard (x 10–13, y 2, `VAR_SIL_CH0_GATE` 0: before the starter): Brico_Bourg_Block_T1,
/// then one step back south.
pub async fn BricoBourg_EventScript_BlockNorth(ctx: Ctx) {
    ctx.lock_all().await;
    say(&ctx, Brico_Bourg_Block_T1).await;
    const STEP_BACK: &[Step] = &[walk(Dir::Down)];
    move_wait(&ctx, PLAYER, STEP_BACK).await;
    ctx.release_all().await;
}

// ---- signs ----

pub async fn BricoBourg_EventScript_SignVillage(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sign_Village, MsgBox::Sign).await;
}

pub async fn BricoBourg_EventScript_SignShaker(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sign_Shaker, MsgBox::Sign).await;
}

pub async fn BricoBourg_EventScript_SignLabo(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sign_Labo, MsgBox::Sign).await;
}

/// The synthetic brambles in front of the Labo door.
const RONCES: (i16, i16) = (4, 8);

/// The synthetic brambles in front of the Labo (4, 8): Ch0's sign until Chapter 1's
/// Lame-Frequence (`FLAG_SIL_MODULE_COUPE`) cuts them, the generic obstacle
/// (`FLAG_SIL_BRICO_LABO_RONCES_CUT`; [`BricoBourg_OnLoad`] keeps them cut).
pub async fn BricoBourg_EventScript_Ronces(ctx: Ctx) {
    cut_obstacle(
        &ctx,
        RONCES,
        METATILE_RONCES_COUPEES,
        FLAG_SIL_BRICO_LABO_RONCES_CUT,
        Brico_Bourg_Sign_Ronces,
    )
    .await;
}

/// The hidden Potion behind the fountain (10, 8), once (`FLAG_SIL_ITEM_BRICO_POTION`).
pub async fn BricoBourg_EventScript_HiddenPotion(ctx: Ctx) {
    // fallback: hidden items (the engine has ignored `BgKind::HiddenItem` since M3, and no
    // milestone owns it yet): the map has a sign here instead, and this script gives the item
    // with Gen3's obtain messages, once, behind the item's flag.
    hidden_item(&ctx, FLAG_SIL_ITEM_BRICO_POTION, ITEM_POTION).await;
}

// ---- ambient NPCs ----

/// The old man by the fountain: his night line at `VAR_SIL_STORY` ≤ 2 (the source says 1; the
/// village is dark until the morning, 3).
pub async fn BricoBourg_EventScript_Vieux(ctx: Ctx) {
    let text = if story(&ctx).await <= 2 {
        Brico_Bourg_Npc_Vieux_Nuit
    } else {
        Brico_Bourg_Npc_Vieux
    };
    msgbox(&ctx, text, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Linge(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Npc_Linge, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Gamin(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Npc_Gamin, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Pecheur(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Npc_Pecheur, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Bricoleuse(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Npc_Bricoleuse, MsgBox::Npc).await;
}

/// The friends in front of the house (story 3–4). Brico_Bourg_Sc02 starts as the player steps
/// out, so they are only met inside it; talked to (not in the source), their Sc02 greeting.
pub async fn BricoBourg_EventScript_Mathieu(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sc02_T1, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Gwada(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sc02_T1, MsgBox::Npc).await;
}

pub async fn BricoBourg_EventScript_Adrien(ctx: Ctx) {
    msgbox(&ctx, Brico_Bourg_Sc02_T1, MsgBox::Npc).await;
}
