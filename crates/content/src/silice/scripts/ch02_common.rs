//! Chapter 2's shared helpers (no scripts, no maps): scripted battles, module obstacles, the
//! Centre's nurse, the Mart's clerk and the screen flash, used by the other `ch02_*` files with
//! explicit imports (`use super::ch02_common::{…}`).
//!
//! **Merge note.** The Chapter 1 scripts, written in parallel, add `battle_trainer`,
//! `pokecenter_nurse`, `mart` and `show_sync` to `silice/prelude.rs` with these same signatures
//! and behaviour. These copies stay here, private to Chapter 2, so that the two branches never
//! touch the same prelude lines; the explicit imports shadow the prelude's glob, so both compile
//! side by side. Once both have landed, the copies of `battle_trainer`, `pokecenter_nurse` and
//! `mart` can go (one commit, no behaviour change), and the generic [`Obstacle`] can move to the
//! prelude next to Chapter 1's `cut_obstacle`.

use crate::silice::prelude::*;

// ---- battles ----

/// The source's `battle_trainer(TRAINER, Defeat)`: a scripted trainer battle (a leader, a boss
/// talked to, a scene's rival; not a sight trainer, which is `trainer`). Unless the player lost,
/// the trainer's beaten flag is set, as Gen3 `CB2_EndTrainerBattle` does; a loss whites out and
/// ends the script. (Chapter 1's prelude helper, see the merge note.)
pub(crate) async fn battle_trainer(ctx: &Ctx, trainer: TrainerId, defeat: TextId) -> BattleOutcome {
    let outcome = ctx.battle_trainer(trainer, defeat).await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(trainer_flag(trainer), true).await;
    }
    outcome
}

/// [`battle_trainer`] inside a scene that goes on after it: whether the player won.
pub(crate) async fn scene_battle(ctx: &Ctx, trainer: TrainerId, defeat: TextId) -> bool {
    let outcome = battle_trainer(ctx, trainer, defeat).await;
    !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew)
}

// ---- module obstacles ----

/// A module obstacle: cells that a Mallette module clears, once, for good (the source's
/// `field_move` obstacles: the Lame-Frequence's cables, the Champ Magnétique's blocks). See
/// [`module_obstacle`] and [`repaint_obstacle`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Obstacle {
    /// The module the obstacle needs (`FLAG_SIL_MODULE_*`).
    pub module: FlagId,
    /// Set once cleared; the map's `on_load` repaints from it.
    pub done: FlagId,
    /// The cells once cleared: (x, y, metatile, blocked).
    pub paint: &'static [(i16, i16, u16, bool)],
    /// Shown as a sign without the module.
    pub blocked: TextId,
    /// The question with the module (yes/no), or `None` when the module acts at once.
    pub ask: Option<TextId>,
    /// Said once the obstacle is cleared.
    pub cleared: TextId,
}

/// [`Obstacle`] interacted with: nothing once `done`; without the module, `blocked` as a sign;
/// else (locked) `ask` (on no, nothing), `cleared`, the cells repainted and redrawn, `done` set.
/// Chapter 1's `cut_obstacle` with the obstacle's own texts and cells. Returns whether it was
/// cleared now.
pub(crate) async fn module_obstacle(ctx: &Ctx, o: &Obstacle) -> bool {
    if ctx.flag(o.done).await {
        return false;
    }
    if !ctx.flag(o.module).await {
        msgbox(ctx, o.blocked, MsgBox::Sign).await;
        return false;
    }
    ctx.lock().await;
    if let Some(q) = o.ask
        && !ask(ctx, q).await
    {
        ctx.release().await;
        return false;
    }
    say(ctx, o.cleared).await;
    for &(x, y, m, blocked) in o.paint {
        ctx.set_metatile(x, y, m, blocked).await;
    }
    ctx.redraw_map().await;
    ctx.set_flag(o.done, true).await;
    ctx.release().await;
    true
}

/// [`Obstacle`]'s `on_load` half: its cells repainted once `done` is set.
pub(crate) async fn repaint_obstacle(ctx: &Ctx, o: &Obstacle) {
    if ctx.flag(o.done).await {
        for &(x, y, m, blocked) in o.paint {
            ctx.set_metatile(x, y, m, blocked).await;
        }
    }
}

// ---- buildings ----

/// Gen3 `Common_EventScript_PkmnCenterNurse` (`data/scripts/pkmn_center_nurse.inc`) without the
/// Gold Card, Pokérus and Union Room branches: rest the party (yes/no), heal, goodbye, on Gen3's
/// own `gText_*`. The heal machine's field effect (`FLDEFF_POKECENTER_HEAL`) is a pause.
/// (Chapter 1's prelude helper, see the merge note.)
pub(crate) async fn pokecenter_nurse(ctx: &Ctx) {
    ctx.lock().await;
    ctx.face_player().await;
    if ask(ctx, gText_WouldYouLikeToRestYourPkmn).await {
        say(ctx, gText_IllTakeYourPkmn).await;
        ctx.delay(60).await;
        ctx.heal_party().await;
        say(ctx, gText_RestoredPkmnToFullHealth).await;
    }
    say(ctx, gText_WeHopeToSeeYouAgain).await;
    ctx.release().await;
}

/// The source's `mart(MART_*)` (Gen3 `pokemart`): no shop screen before Milestone 9, so the
/// clerk greets and says goodbye on Gen3's own `gText_*` and nothing is sold. Reported (`"mart"`).
/// `items` is the stock the shop will sell. (Chapter 1's prelude helper, see the merge note.)
pub(crate) async fn mart(ctx: &Ctx, items: &'static [ItemId]) {
    ctx.lock().await;
    ctx.face_player().await;
    say(ctx, gText_HowMayIServeYou).await;
    fallback(ctx, "mart").await;
    ctx.log(if items.is_empty() {
        "mart: empty stock"
    } else {
        "mart: stock not sold yet"
    })
    .await;
    say(ctx, gText_PleaseComeAgain).await;
    ctx.release().await;
}

// ---- fallbacks ----

/// `screen_flash(COLOR)`: no screen flash yet (`Request::ScreenFlash`, SIL2 Wave 0 in the M9
/// plan); a fade to black and back stands in (the chapter's §9 fallback). Reported.
pub(crate) async fn screen_flash(ctx: &Ctx) {
    fallback(ctx, "screen_flash").await;
    ctx.fade(Fade::ToBlack).await;
    ctx.fade(Fade::FromBlack).await;
}
