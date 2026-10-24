//! Chapter 5, the Plume-Noire silo (optional): the warehouse (`Owlys_Entrepot`) and the dark
//! silo below it (`Owlys_Entrepot_B1F`): `contenu-jeu/chapitres/05-owlys.md` §2, §3
//! Owlys_Entrepot_Sc01 (the Umbreon guide at three junctions) and Owlys_Entrepot_Sc02 (the Silo
//! Chief, Gogo and Romain's reserve, the choice), §5 (the grunts) and §7 (the items).
//!
//! **Temp-var junctions** ("SIL3 Ch5 maps as built"): the three junctions are coord events on
//! `VAR_TEMP_1` 0, which no script on the silo's B1F writes, so they always fire and each
//! returns at once once its `FLAG_SIL_OWLYS_SILO_NOCTALI_n` is set.

use crate::silice::prelude::*;

/// The maps whose scripts this file owns: strict (every script they name must exist).
pub const MAPS: &[&str] = &["Owlys_Entrepot", "Owlys_Entrepot_B1F"];

// ---- the warehouse (1F) ----

pub async fn Owlys_Entrepot_EventScript_Sbire1(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ENTREPOT_SBIRE_1,
        TRAINER_SIL_OWLYS_SILO_SBIRE_1,
        Owlys_Silo_1_Intro,
        Owlys_Silo_1_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Silo_1_Defeat).await;
    }
}

pub async fn Owlys_Entrepot_EventScript_Sbire2(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ENTREPOT_SBIRE_2,
        TRAINER_SIL_OWLYS_SILO_SBIRE_2,
        Owlys_Silo_2_Intro,
        Owlys_Silo_2_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Silo_2_Defeat).await;
    }
}

/// The third grunt (the source puts him "in the dark of B1F"; the map keeps him on 1F).
pub async fn Owlys_Entrepot_EventScript_Sbire3(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_ENTREPOT_SBIRE_3,
        TRAINER_SIL_OWLYS_SILO_SBIRE_3,
        Owlys_Silo_3_Intro,
        Owlys_Silo_3_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Silo_3_Defeat).await;
    }
}

/// The Escape Rope in aisle 2.
pub async fn Owlys_Entrepot_EventScript_ItemCorde(ctx: Ctx) {
    find_item(&ctx, ITEM_ESCAPE_ROPE, 1).await;
}

// ---- the silo (B1F) ----

/// The three Umbreon only appear in their junction scenes.
pub async fn Owlys_Entrepot_B1F_OnTransition(ctx: Ctx) {
    for f in [
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_1,
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_2,
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_3,
    ] {
        ctx.set_flag(f, true).await;
    }
}

/// Umbreon 1 trots left between two tanks; 2 goes down the aisle; 3 walks to the iron door
/// (the source's `Walk(Dir) x3` with each junction's direction, from its text).
const NOCTALI_1_PATH: &[Step] = &[walk(Dir::Left), walk(Dir::Left), walk(Dir::Left)];
const NOCTALI_2_PATH: &[Step] = &[walk(Dir::Down), walk(Dir::Down), walk(Dir::Down)];
const NOCTALI_3_PATH: &[Step] = &[walk(Dir::Right), walk(Dir::Right), walk(Dir::Right)];

/// Owlys_Entrepot_Sc01 at junction `n`: once (`done`), Umbreon `obj` shows, its line, it walks
/// `path` and vanishes. The source's moving light (a 2-cell radius around the Umbreon) is a
/// fallback: nothing is lit (the engine draws no darkness yet); the text gives the direction.
async fn noctali(
    ctx: &Ctx,
    obj: u8,
    hide_flag: FlagId,
    done: FlagId,
    text: TextId,
    path: &'static [Step],
) {
    if ctx.flag(done).await {
        return;
    }
    ctx.lock_all().await;
    // fallback: a moving light source (§9; "source de lumière mobile"): no light radius.
    fallback(ctx, "moving light").await;
    show(ctx, obj, hide_flag).await;
    say(ctx, text).await;
    move_wait(ctx, obj, path).await;
    hide(ctx, obj).await;
    ctx.set_flag(done, true).await;
    ctx.release_all().await;
}

/// Junction 1, at the foot of the freight lift.
pub async fn Owlys_Entrepot_B1F_Sc01_1(ctx: Ctx) {
    noctali(
        &ctx,
        LOCALID_SILO_NOCTALI_1,
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_1,
        FLAG_SIL_OWLYS_SILO_NOCTALI_1,
        Owlys_Entrepot_Sc01_T1,
        NOCTALI_1_PATH,
    )
    .await;
}

/// Junction 2, down the west aisle.
pub async fn Owlys_Entrepot_B1F_Sc01_2(ctx: Ctx) {
    noctali(
        &ctx,
        LOCALID_SILO_NOCTALI_2,
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_2,
        FLAG_SIL_OWLYS_SILO_NOCTALI_2,
        Owlys_Entrepot_Sc01_T2,
        NOCTALI_2_PATH,
    )
    .await;
}

/// Junction 3, the south corridor to the reserve.
pub async fn Owlys_Entrepot_B1F_Sc01_3(ctx: Ctx) {
    noctali(
        &ctx,
        LOCALID_SILO_NOCTALI_3,
        FLAG_SIL_HIDE_OWLYS_SILO_NOCTALI_3,
        FLAG_SIL_OWLYS_SILO_NOCTALI_3,
        Owlys_Entrepot_Sc01_T3,
        NOCTALI_3_PATH,
    )
    .await;
}

/// Gogo and Romain's Poudre Claire sacks (29–31, 25–26), "B" in the layout; emptied into the
/// canal they become the reserve's floor (742).
const SACS: &[(i16, i16)] = &[(29, 25), (30, 25), (31, 25), (29, 26), (30, 26), (31, 26)];
const METATILE_SOL_SILO: u16 = 742;

/// Owlys_Entrepot_Sc02, the Silo Chief (27, 26), a sight trainer facing the reserve's only gap:
/// his intro and battle, then he leaves (for good: his hide flag is
/// `FLAG_SIL_HIDE_OWLYS_SILO_CHEF`, `edits/Owlys_Entrepot_B1F/05_ch5_scripts.ron`), the sacks and
/// Romain's dictaphone, and the choice: empty the sacks into the canal (`FLAG_SIL_OWLYS_SILO_VIDE`,
/// read by Chapter 7; the sacks' cells become floor) or leave them. `FLAG_SIL_OWLYS_SILO_DECIDE`
/// either way.
pub async fn Owlys_Entrepot_B1F_Sc02(ctx: Ctx) {
    if trainer(
        &ctx,
        LOCALID_SILO_CHEF,
        TRAINER_SIL_OWLYS_CHEF_SILO,
        Owlys_Entrepot_Sc02_T1,
        Owlys_Entrepot_Sc02_Defeat,
    )
    .await
    {
        post_battle(&ctx, Owlys_Entrepot_Sc02_Defeat).await;
    }
    if ctx.flag(FLAG_SIL_OWLYS_SILO_DECIDE).await || ctx.flag(FLAG_SIL_OWLYS_SILO_VIDE).await {
        return;
    }
    ctx.lock_all().await;
    say(&ctx, Owlys_Entrepot_Sc02_T2).await;
    ctx.fade(Fade::ToBlack).await;
    hide(&ctx, LOCALID_SILO_CHEF).await;
    ctx.fade(Fade::FromBlack).await;
    say(&ctx, Owlys_Entrepot_Sc02_T3).await;
    say(&ctx, Owlys_Entrepot_Sc02_T4).await;
    if ask(&ctx, Owlys_Entrepot_Sc02_T5).await {
        ctx.set_flag(FLAG_SIL_OWLYS_SILO_VIDE, true).await;
        empty_sacks(&ctx).await;
        ctx.redraw_map().await;
        // The source's troubled water in the canal has no tile here.
        say(&ctx, Owlys_Entrepot_Sc02_T6).await;
    } else {
        say(&ctx, Owlys_Entrepot_Sc02_T7).await;
    }
    ctx.set_flag(FLAG_SIL_OWLYS_SILO_DECIDE, true).await;
    ctx.release_all().await;
}

/// The sacks gone once emptied (`FLAG_SIL_OWLYS_SILO_VIDE`), on every load. Added by
/// `edits/Owlys_Entrepot_B1F/05_ch5_scripts.ron`.
pub async fn Owlys_Entrepot_B1F_OnLoad(ctx: Ctx) {
    if ctx.flag(FLAG_SIL_OWLYS_SILO_VIDE).await {
        empty_sacks(&ctx).await;
    }
}

/// The sacks' cells as the reserve's floor (walkable).
async fn empty_sacks(ctx: &Ctx) {
    for &(x, y) in SACS {
        ctx.set_metatile(x, y, METATILE_SOL_SILO, false).await;
    }
}


