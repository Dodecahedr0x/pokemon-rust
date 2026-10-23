//! What Silice scripts (`src/silice/scripts/*.rs`) use: `use crate::silice::prelude::*;`.
//!
//! The Gen3 prelude (`crate::prelude`: the engine's script types, every generated constant,
//! [`msgbox`], `callstd`, `special`, `trainer_battle`, [`give_item`], `add_item`, `buffer`,
//! `multichoice`…) plus Silice's own helpers below, named after the pseudo-script primitives of
//! the chapters (`contenu-jeu/bible/conventions.md` §2). Every helper takes `&Ctx`; a script
//! receives its `Ctx` by value and passes `&ctx`.
//!
//! **Story and state:** [`story`], [`set_story`] (`VAR_SIL_STORY`), [`defeated`] (a trainer's
//! beaten flag), [`gift`] (an item given once, behind its `FLAG_SIL_ITEM_*`).
//!
//! **Scenes:** [`say`] (a line inside a locked scene), [`ask`] (`msgbox(T, YesNo)` then
//! `if yes_no`), [`walk`] and the other step builders, [`move_wait`] (`move_obj` then
//! `wait_move`), [`show`] / [`hide`] (the source's `show_object` / `hide_object`, which keep
//! the hide flag in step), [`warp`], [`choose_starter`] (the Silice starters of `game.ron`),
//! [`battle_wild`].
//!
//! **Fallbacks:** [`fallback`] reports `Report::Degraded(name)` and returns, so the scene goes
//! on: [`screen_shake`] (a fade out and in), [`play_se`] (nothing), [`mallette_call`] and
//! [`end_call`] (nothing: the call's lines are plain messages).
//!
//! The rest of the source's primitives are `Ctx` methods, called directly: `lock`, `lock_all`,
//! `release`, `release_all`, `face_player`, `set_flag(f, true)` (`set_flag`) and
//! `set_flag(f, false)` (`clear_flag`), `flag`, `set_var`, `var`, `delay`, `fade`, `turn`,
//! `heal_party`, `give_mon`, `door`, `set_metatile`, `set_respawn`. `call(Script)` is an
//! `.await` of the other script: `BricoBourg_Sc03(ctx.clone()).await`.

pub use crate::prelude::*;

// ---- story and state ----

/// The story step, `VAR_SIL_STORY` (`bible/progression.md`; Ch0 runs 0 to 9).
pub async fn story(ctx: &Ctx) -> u16 {
    ctx.var(VAR_SIL_STORY).await
}

/// Sets the story step, `VAR_SIL_STORY`.
pub async fn set_story(ctx: &Ctx, step: u16) {
    ctx.set_var(VAR_SIL_STORY, step).await
}

/// Whether `trainer` has been beaten (Gen3 `TRAINER_FLAG(id)`, the flag `0x500 + id`).
pub async fn defeated(ctx: &Ctx, trainer: TrainerId) -> bool {
    ctx.flag(trainer_flag(trainer)).await
}

/// How [`gift`] tells the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Obtain {
    /// Gen3 `giveitem`'s "Obtained the …" and pocket messages ([`give_item`]).
    Message,
    /// No message: the scene's own text says it (`add_item`).
    Quiet,
}

/// An item given once (a §7 "Donné" row): nothing when `flag` is set; else gives `qty` of
/// `item` and, when it fit in the bag, sets `flag`. Returns whether it was given now.
pub async fn gift(ctx: &Ctx, flag: FlagId, item: u16, qty: u16, how: Obtain) -> bool {
    if ctx.flag(flag).await {
        return false;
    }
    let fit = match how {
        Obtain::Message => give_item(ctx, item, qty).await,
        Obtain::Quiet => add_item(ctx, item, qty).await,
    };
    if fit {
        ctx.set_flag(flag, true).await;
    }
    fit
}

// ---- scenes ----

/// One line of a locked scene: `msgbox(T, Default)`, and `msgbox(T, Npc)` inside a `lock_all`
/// scene. (Gen3's `MSGBOX_NPC`, [`msgbox`] with [`MsgBox::Npc`], locks, turns the talked-to
/// object and *releases*: it is for a lone NPC line only.)
pub async fn say(ctx: &Ctx, text: TextId) {
    ctx.say(text).await
}

/// `msgbox(T, YesNo)` then `if yes_no`: the answer (also in `VAR_RESULT`).
pub async fn ask(ctx: &Ctx, text: TextId) -> bool {
    msgbox(ctx, text, MsgBox::YesNo).await;
    ctx.var(VAR_RESULT).await == 1
}

/// `Walk(dir)` at the normal speed. The step builders are `const fn`s, for `const` paths:
/// `const PATH: &[Step] = &[walk(Dir::Up), walk(Dir::Up), face(Dir::Left)];`.
pub const fn walk(dir: Dir) -> Step {
    Step::Walk(dir, Speed::Normal)
}

/// `WalkInPlace` (facing `dir`) at the normal speed.
pub const fn walk_in_place(dir: Dir) -> Step {
    Step::WalkInPlace(dir, Speed::Normal)
}

/// `Face(dir)`.
pub const fn face(dir: Dir) -> Step {
    Step::Face(dir)
}

/// `move_obj(obj, steps) ; wait_move(obj)`. Several objects moving together: `ctx.move_obj`
/// each, then one `ctx.wait_move` on the last.
pub async fn move_wait(ctx: &Ctx, obj: u8, steps: &'static [Step]) {
    ctx.move_obj(obj, steps).await;
    ctx.wait_move(obj).await;
}

/// The source's `show_object(OBJ)`: clears the object's hide flag and spawns it (Gen3
/// `clearflag` + `addobject`), so it stays on the map. `hide_flag` is the object's `flag` in the
/// map file.
pub async fn show(ctx: &Ctx, obj: u8, hide_flag: FlagId) {
    ctx.set_flag(hide_flag, false).await;
    ctx.add_object(obj).await;
}

/// The source's `hide_object(OBJ)`: despawns the object and sets its hide flag (Gen3
/// `removeobject`), so it stays gone.
pub async fn hide(ctx: &Ctx, obj: u8) {
    ctx.remove_object(obj).await;
}

/// The source's `warp(MAP, x, y)`: to cell (x, y) of `map`. The map load ends the script (as
/// Gen3's `warp`): nothing after it runs.
pub async fn warp(ctx: &Ctx, map: MapId, x: i16, y: i16) {
    ctx.warp(map, None, x, y).await
}

/// The source's `let i = choose_starter([...])` and `set_var(VAR_SIL_STARTER, i)`: the starter
/// screen on `game.ron`'s starters (`SIL_STARTERS`), then the choice in `VAR_SIL_STARTER` and
/// the starter at `SIL_STARTER_LEVEL` in the party. No battle follows (Emerald's
/// `ChooseStarter` special runs its first battle; the Prologue's is a later scene). Returns the
/// index (0, 1 or 2).
pub async fn choose_starter(ctx: &Ctx) -> u8 {
    let i = ctx.choose_starter(SIL_STARTERS).await;
    // As Gen3's `GetStarterPokemon`: an out-of-range choice is the first starter.
    let i = if usize::from(i) < SIL_STARTERS.len() {
        i
    } else {
        0
    };
    ctx.set_var(VAR_SIL_STARTER, i.into()).await;
    ctx.give_mon(SIL_STARTERS[usize::from(i)], SIL_STARTER_LEVEL)
        .await;
    i
}

/// The source's `battle_wild(SPECIES, level)`: an ordinary wild battle (the player may run or
/// catch).
pub async fn battle_wild(ctx: &Ctx, species: SpeciesId, level: u8) -> BattleOutcome {
    ctx.battle_wild(species, level, false).await
}

// ---- fallbacks ----

/// A primitive the engine does not have yet (`bible/besoins-moteur.md`): reports
/// `Report::Degraded(name)` (logged, and a `World` event) and returns, so the scene continues.
/// `name` is the source's primitive, e.g. `"screen_shake"`.
pub async fn fallback(ctx: &Ctx, name: &'static str) {
    ctx.report(Report::Degraded(name)).await;
}

/// `screen_shake(magnitude, frames)`: no screen shake yet (SIL2's `Request::ScreenShake`); a fade
/// to black and back stands in (the Ch0 §9 fallback). Reported.
pub async fn screen_shake(ctx: &Ctx, _magnitude: u8, _frames: u16) {
    fallback(ctx, "screen_shake").await;
    ctx.fade(Fade::ToBlack).await;
    ctx.fade(Fade::FromBlack).await;
}

/// `play_se(SE_*)`: no sound yet; nothing plays (the texts carry the onomatopoeia). Reported,
/// and `se` (the decomp or `SE_SIL_*` name, which `cargo xtask silice check` checks in the
/// scripts; the build has no sound constants yet) logged.
pub async fn play_se(ctx: &Ctx, se: &'static str) {
    fallback(ctx, "play_se").await;
    ctx.log(se).await;
}

/// `mallette_call(PERSON)`: no call window yet; the call's lines are plain messages. Reported,
/// and `who` logged.
pub async fn mallette_call(ctx: &Ctx, who: &'static str) {
    fallback(ctx, "mallette_call").await;
    ctx.log(who).await;
}

/// `end_call()`: closes no call window (see [`mallette_call`]). Reported.
pub async fn end_call(ctx: &Ctx) {
    fallback(ctx, "end_call").await;
}
