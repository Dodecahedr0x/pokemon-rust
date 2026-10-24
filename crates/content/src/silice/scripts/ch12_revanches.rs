//! Chapter 12, the post-game, the gym leaders' rematches (`contenu-jeu/chapitres/12-post-game.md`
//! §4.1 `PG_Revanche`, and the liberations it carries: `PG_Lib_TH`, `PG_Lib_Evalon`,
//! `PG_Lib_Loic`).
//!
//! From 121 each leader waits in their gym in place of their first battle (each gym's leader
//! script calls [`revanche`]): one rematch a day (`FLAG_SIL_DAILY_<LEADER>`, reset by the World
//! clock's day), a Mega Stone the first win (`FLAG_SIL_ITEM_PG_GEM_<LEADER>`). Tauber & Hocseat's
//! first win frees Raikou (their team becomes `_10`), Loulpix & Doris's frees Registeel and Latias;
//! Loïc tells of Celebi before his rematch from 130 once his gem is won.
//!
//! The source's line after the battle is passed as its defeat text (the module's convention).

use super::ch12_reveil::battle_lost;
use crate::silice::prelude::*;

/// A leader's rematch.
pub(super) struct Champion {
    trainer: TrainerId,
    intro: TextId,
    defaite: TextId,
    gemme: u16,
    flag_gemme: FlagId,
    daily: FlagId,
}

pub(super) const LOIC: Champion = Champion {
    trainer: TRAINER_SIL_LOIC_9,
    intro: Loic_PG_Revanche_Intro,
    defaite: Loic_PG_Revanche_Defaite,
    gemme: ITEM_SIL_VENUSAURITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_LOIC,
    daily: FLAG_SIL_DAILY_LOIC,
};

pub(super) const YOHAN: Champion = Champion {
    trainer: TRAINER_SIL_YOHAN_9,
    intro: Yohan_PG_Revanche_Intro,
    defaite: Yohan_PG_Revanche_Defaite,
    gemme: ITEM_SIL_ALAKAZITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_YOHAN,
    daily: FLAG_SIL_DAILY_YOHAN,
};

pub(super) const TAUBER_HOCSEAT: Champion = Champion {
    trainer: TRAINER_SIL_TAUBER_HOCSEAT_9,
    intro: Tauber_Hocseat_PG_Revanche_Intro,
    defaite: Tauber_Hocseat_PG_Revanche_Defaite,
    gemme: ITEM_SIL_GENGARITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_TAUBER_HOCSEAT,
    daily: FLAG_SIL_DAILY_TAUBER_HOCSEAT,
};

/// Tauber & Hocseat once Raikou is free (`PG_Lib_TH`): Electrike's line, team `_10`.
const TAUBER_HOCSEAT_10: Champion = Champion {
    trainer: TRAINER_SIL_TAUBER_HOCSEAT_10,
    intro: Tauber_Hocseat_PG_Revanche_Intro2,
    ..TAUBER_HOCSEAT
};

pub(super) const SAMUEL: Champion = Champion {
    trainer: TRAINER_SIL_SAMUEL_9,
    intro: Samuel_PG_Revanche_Intro,
    defaite: Samuel_PG_Revanche_Defaite,
    gemme: ITEM_SIL_BLAZIKENITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_SAMUEL,
    daily: FLAG_SIL_DAILY_SAMUEL,
};

pub(super) const SKYCKER: Champion = Champion {
    trainer: TRAINER_SIL_SKYCKER_9,
    intro: Skycker_PG_Revanche_Intro,
    defaite: Skycker_PG_Revanche_Defaite,
    gemme: ITEM_SIL_ABSOLITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_SKYCKER,
    daily: FLAG_SIL_DAILY_SKYCKER,
};

pub(super) const TAK: Champion = Champion {
    trainer: TRAINER_SIL_TAK_9,
    intro: Tak_PG_Revanche_Intro,
    defaite: Tak_PG_Revanche_Defaite,
    gemme: ITEM_SIL_CHARIZARDITE_X,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_TAK,
    daily: FLAG_SIL_DAILY_TAK,
};

pub(super) const REVERSE: Champion = Champion {
    trainer: TRAINER_SIL_REVERSE_9,
    intro: Reverse_PG_Revanche_Intro,
    defaite: Reverse_PG_Revanche_Defaite,
    gemme: ITEM_SIL_BANETTITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_REVERSE,
    daily: FLAG_SIL_DAILY_REVERSE,
};

pub(super) const LOULPIX_DORIS: Champion = Champion {
    trainer: TRAINER_SIL_LOULPIX_DORIS_9,
    intro: Loulpix_Doris_PG_Revanche_Intro,
    defaite: Loulpix_Doris_PG_Revanche_Defaite,
    gemme: ITEM_SIL_METAGROSSITE,
    flag_gemme: FLAG_SIL_ITEM_PG_GEM_LOULPIX_DORIS,
    daily: FLAG_SIL_DAILY_LOULPIX_DORIS,
};

/// Whether the rematches are open (`VAR_SIL_STORY` 121, the Council's call answered).
pub(super) async fn ouvertes(ctx: &Ctx) -> bool {
    story(ctx).await >= 121
}

/// `PG_Revanche` for `c`; whether the battle was won (false: refused, already done today, or a
/// whiteout). The caller adds its liberation after a first win ([`gemme_gagnee`] tells it).
async fn revanche_inner(ctx: &Ctx, c: &Champion) -> bool {
    ctx.lock().await;
    ctx.face_player().await;
    if ctx.flag(c.daily).await {
        say(ctx, PG_Revanche_Deja).await;
        ctx.release().await;
        return false;
    }
    if !ask(ctx, c.intro).await {
        ctx.release().await;
        return false;
    }
    if battle_lost(ctx, c.trainer, c.defaite).await {
        return false;
    }
    true
}

/// The end of a won rematch: the gem the first time, the daily flag. Returns whether this was the
/// first win.
async fn fin(ctx: &Ctx, c: &Champion) -> bool {
    let premiere = !ctx.flag(c.flag_gemme).await;
    if premiere {
        give_item(ctx, c.gemme, 1).await;
        ctx.set_flag(c.flag_gemme, true).await;
    }
    ctx.set_flag(c.daily, true).await;
    premiere
}

/// `PG_Revanche` for a leader without a liberation.
pub(super) async fn revanche(ctx: &Ctx, c: &Champion) {
    if revanche_inner(ctx, c).await {
        fin(ctx, c).await;
        ctx.release().await;
    }
}

/// Loïc: from 130 with his gem won, Celebi's dream first (`PG_Lib_Loic`: Celebi's clear point),
/// then his rematch.
pub(super) async fn revanche_loic(ctx: &Ctx) {
    if story(ctx).await >= 130
        && ctx.flag(FLAG_SIL_ITEM_PG_GEM_LOIC).await
        && !ctx.flag(FLAG_SIL_LIBERATION_CELEBI).await
    {
        ctx.lock().await;
        ctx.face_player().await;
        say(ctx, Loic_PG_Lib_T1).await;
        ctx.set_flag(FLAG_SIL_LIBERATION_CELEBI, true).await;
        ctx.set_flag(FLAG_SIL_HIDE_CELEBI, false).await;
        ctx.release().await;
    }
    revanche(ctx, &LOIC).await;
}

/// Tauber & Hocseat: the first win frees Raikou (`PG_Lib_TH`: its clear point); after, team `_10`.
pub(super) async fn revanche_tauber_hocseat(ctx: &Ctx) {
    let c = if ctx.flag(FLAG_SIL_LIBERATION_RAIKOU).await {
        &TAUBER_HOCSEAT_10
    } else {
        &TAUBER_HOCSEAT
    };
    if revanche_inner(ctx, c).await {
        if fin(ctx, c).await {
            say(ctx, Tauber_Hocseat_PG_Lib_T1).await;
            ctx.set_flag(FLAG_SIL_LIBERATION_RAIKOU, true).await;
            ctx.set_flag(FLAG_SIL_HIDE_RAIKOU, false).await;
        }
        ctx.release().await;
    }
}

/// Loulpix & Doris: the first win frees Registeel and Latias (`PG_Lib_Evalon`: their clear point).
pub(super) async fn revanche_loulpix_doris(ctx: &Ctx) {
    if revanche_inner(ctx, &LOULPIX_DORIS).await {
        if fin(ctx, &LOULPIX_DORIS).await {
            say(ctx, Evalon_PG_Lib_T1).await;
            ctx.set_flag(FLAG_SIL_LIBERATION_EVALON, true).await;
            ctx.set_flag(FLAG_SIL_HIDE_REGISTEEL, false).await;
            ctx.set_flag(FLAG_SIL_HIDE_LATIAS, false).await;
        }
        ctx.release().await;
    }
}
