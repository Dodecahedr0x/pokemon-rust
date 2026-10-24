//! What Silice scripts (`src/silice/scripts/*.rs`) use: `use crate::silice::prelude::*;`.
//!
//! The Gen3 prelude (`crate::prelude`: the engine's script types, every generated constant,
//! [`msgbox`], `callstd`, `special`, `trainer_battle`, [`give_item`], `add_item`, `buffer`,
//! `multichoice`…) plus Silice's own helpers below, named after the pseudo-script primitives of
//! the chapters (`contenu-jeu/bible/conventions.md` §2). Every helper takes `&Ctx`; a script
//! receives its `Ctx` by value and passes `&ctx`.
//!
//! **Story and state:** [`story`], [`set_story`] (`VAR_SIL_STORY`), [`defeated`] (a trainer's
//! beaten flag), [`trainer`] (a route trainer; the registry lists its script for sight) and
//! [`post_battle`] (its line once beaten),
//! [`gift`] (an item given once, behind its `FLAG_SIL_ITEM_*`), [`hidden_item`] (a hidden item
//! as a sign script, a fallback).
//!
//! **Module obstacles:** [`cut_obstacle`] (the generic Lame-Frequence obstacle) and
//! [`repaint_if`] (its `on_load` repaint), with [`METATILE_RONCES_COUPEES`]; [`Obstacle`] with
//! [`module_obstacle`] and [`repaint_obstacle`] (any module, several cells, the obstacle's own
//! texts: Chapter 2's cables, server and crates).
//!
//! **Scenes:** [`say`] (a line inside a locked scene), [`ask`] (`msgbox(T, YesNo)` then
//! `if yes_no`), [`walk`] and the other step builders, [`move_wait`] (`move_obj` then
//! `wait_move`), [`show`] / [`hide`] (the source's `show_object` / `hide_object`, which keep
//! the hide flag in step), [`warp`], [`choose_starter`] (the Silice starters of `game.ron`),
//! [`battle_wild`], [`battle_trainer`] (a scripted trainer battle, the leader's or a boss's).
//!
//! **Buildings:** [`pokecenter_nurse`] (Gen3's nurse), [`mart`] (a fallback until M9's shop).
//!
//! **Screen effects:** [`screen_shake`], [`screen_flash`] (with [`WHITE`], [`YELLOW`],
//! [`BLACK`], [`NEGATIF`]), [`fade_white`], [`show_text_on_black`], [`camera_pan`].
//!
//! **Fallbacks:** [`fallback`] reports `Report::Degraded(name)` and returns, so the scene goes
//! on: [`play_se`] (nothing),
//! [`mallette_call`] and [`end_call`] (nothing: the call's lines are plain messages),
//! [`battle_two_trainers`] (two single battles in a row), [`show_sync`] (the sync gauge: the
//! scene's text says the value).
//!
//! The rest of the source's primitives are `Ctx` methods, called directly: `lock`, `lock_all`,
//! `release`, `release_all`, `face_player`, `set_flag(f, true)` (`set_flag`) and
//! `set_flag(f, false)` (`clear_flag`), `flag`, `set_var`, `var`, `delay`, `fade`, `turn`,
//! `heal_party`, `give_mon`, `door`, `set_metatile`, `set_respawn`. `call(Script)` is an
//! `.await` of the other script: `BricoBourg_Sc03(ctx.clone()).await`.

pub use crate::prelude::*;
/// Reverse-City's inverted controls (`Ctx::set_input_map`).
pub use engine::InputMap;

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

/// A route trainer, Gen3 `trainerbattle_single TRAINER, intro, defeat` (kind
/// `TRAINER_BATTLE_SINGLE`): seen (the engine's sight walks it up) or talked to, the intro, the
/// battle, the defeat text. Returns whether the trainer was already beaten when talked to (Gen3
/// `gotopostbattlescript`): the script then says its post-battle line.
///
/// **Registry contract** (`build/silice/registry.rs`): a script whose *first statement* calls
/// `trainer(&ctx, LOCALID_…, TRAINER_…, …)` is a trainer script, listed in `TRAINER_SCRIPTS`
/// so that sight knows its trainer and flag. `obj` is the trainer object's local id.
pub async fn trainer(
    ctx: &Ctx,
    obj: u8,
    trainer: TrainerId,
    intro: TextId,
    defeat: TextId,
) -> bool {
    let b = TrainerBattle {
        kind: kind::SINGLE,
        trainer: trainer.0,
        local_id: obj,
        intro: Some(intro),
        lose: defeat,
        not_enough: None,
        event_script: None,
        music: true,
    };
    trainer_battle(ctx, b).await
}

/// A beaten [`trainer`] talked to again: `text` as Gen3's `msgbox <PostBattle>, MSGBOX_AUTOCLOSE`
/// (the trainer is locked and faces the player already). The sources write no post-battle lines:
/// the scripts pass the defeat text.
pub async fn post_battle(ctx: &Ctx, text: TextId) {
    msgbox(ctx, text, MsgBox::Autoclose).await;
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

/// A hidden item as a sign script (the engine does not pick up `BgKind::HiddenItem` yet):
/// nothing once `flag` is set; else, locked, Gen3 `giveitem` (its "Obtained" and pocket
/// messages) and `flag` when it fit. A fallback (`Report::Degraded("hidden item")`).
pub async fn hidden_item(ctx: &Ctx, flag: FlagId, item: u16) {
    if ctx.flag(flag).await {
        return;
    }
    fallback(ctx, "hidden item").await;
    ctx.lock().await;
    gift(ctx, flag, item, 1, Obtain::Message).await;
    ctx.release().await;
}

// ---- module obstacles ----

/// `METATILE_RONCES_COUPEES`: synthetic brambles once cut, General's grass (metatile 1), walkable.
/// The same on every map whose brambles stand on the General primary (Brico-Bourg (4, 9),
/// Bois-Éther (8, 6); "SIL2 Ch1 maps as built").
pub const METATILE_RONCES_COUPEES: u16 = 1;

/// The generic Lame-Frequence obstacle (Ch1 `Bois_Ether_Sc08`, "the same script for every
/// bramble and cable; only the obstacle's flag changes"): nothing once `cut` is set; without
/// `FLAG_SIL_MODULE_COUPE`, `blocked` as a sign; else `Bois_Ether_Sc08_T2` (yes/no), on yes
/// `_T3`, the cell `(x, y)` repainted as `metatile` (walkable) and `cut` set. The map's
/// `on_load` repaints the cell from `cut` on every later load ([`repaint_if`]).
pub async fn cut_obstacle(
    ctx: &Ctx,
    (x, y): (i16, i16),
    metatile: u16,
    cut: FlagId,
    blocked: TextId,
) {
    if ctx.flag(cut).await {
        return;
    }
    if !ctx.flag(FLAG_SIL_MODULE_COUPE).await {
        msgbox(ctx, blocked, MsgBox::Sign).await;
        return;
    }
    ctx.lock().await;
    if ask(ctx, Bois_Ether_Sc08_T2).await {
        say(ctx, Bois_Ether_Sc08_T3).await;
        ctx.set_metatile(x, y, metatile, false).await;
        ctx.set_flag(cut, true).await;
    }
    ctx.release().await;
}

/// An `on_load` repaint: the cell `(x, y)` as `metatile` (walkable) when `flag` is set (Gen3
/// `call_if_set FLAG, …_EventScript_SetCutMetatile`; the map loads from its layout every time).
pub async fn repaint_if(ctx: &Ctx, flag: FlagId, (x, y): (i16, i16), metatile: u16) {
    if ctx.flag(flag).await {
        ctx.set_metatile(x, y, metatile, false).await;
    }
}

/// A module obstacle: cells that a Mallette module clears, once, for good (the source's
/// `field_move` obstacles: the Lame-Fréquence's cables, the Champ Magnétique's blocks). The
/// general form of [`cut_obstacle`]; see [`module_obstacle`] and [`repaint_obstacle`].
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
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
/// Returns whether it was cleared now.
pub async fn module_obstacle(ctx: &Ctx, o: &Obstacle) -> bool {
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
pub async fn repaint_obstacle(ctx: &Ctx, o: &Obstacle) {
    if ctx.flag(o.done).await {
        for &(x, y, m, blocked) in o.paint {
            ctx.set_metatile(x, y, m, blocked).await;
        }
    }
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
    // Gen3 sets it with the first Pokémon (Birch's lab): the start menu shows POKéMON from now
    // on (`BuildNormalStartMenu`).
    ctx.set_flag(FLAG_SYS_POKEMON_GET, true).await;
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

/// The source's colours for [`screen_flash`] (BGR555).
pub const WHITE: u16 = 0x7FFF;
/// `screen_flash(YELLOW)`.
pub const YELLOW: u16 = 0x03FF;
/// `screen_flash(PIXELS_NOIRS)`.
pub const BLACK: u16 = 0x0000;
/// `screen_flash(NEGATIF)`: no inverted palette (`Request::ScreenFlash` fills one colour); a
/// white flash stands in. Author question: keep it, or ask for a palette inversion.
pub const NEGATIF: u16 = WHITE;

/// How long a flash stays on screen when the source gives no duration (16 frames).
pub const FLASH_FRAMES: u16 = 16;
/// A shake's length when the source gives none (`screen_shake(m)`: 20 frames, Gen3
/// `ShakeCamera`'s usual `delay * num_shakes`).
pub const SHAKE_FRAMES: u16 = 20;

/// `screen_shake(magnitude, frames)` (`Request::ScreenShake`): the field shakes by `magnitude`
/// pixels for `frames` frames (0: [`SHAKE_FRAMES`]); the scene waits for its end.
pub async fn screen_shake(ctx: &Ctx, magnitude: u8, frames: u16) {
    let frames = if frames == 0 { SHAKE_FRAMES } else { frames };
    ctx.screen_shake(magnitude, frames).await;
}

/// `screen_flash(COLOR)` (`Request::ScreenFlash`): the screen fills with `color` for
/// [`FLASH_FRAMES`]; the scene waits for its end.
pub async fn screen_flash(ctx: &Ctx, color: u16) {
    ctx.screen_flash(color, FLASH_FRAMES).await;
}

/// `fade_white()` / `fade_from_white()`: Gen3's white fades.
pub async fn fade_white(ctx: &Ctx, out: bool) {
    ctx.fade(if out { Fade::ToWhite } else { Fade::FromWhite })
        .await;
}

/// `show_text_on_black(T)`: the field hidden behind black, `text` said, the field back.
pub async fn show_text_on_black(ctx: &Ctx, text: TextId) {
    ctx.text_on_black(true).await;
    say(ctx, text).await;
    ctx.close_message().await;
    ctx.text_on_black(false).await;
}

/// `camera_pan(dx, dy)` in cells, at 2 pixels a frame (`Request::CameraPan`); `(0, 0)` brings
/// the camera back on the player. The scene waits for the pan's end.
pub async fn camera_pan(ctx: &Ctx, cells_x: i16, cells_y: i16) {
    ctx.camera_pan(cells_x * 16, cells_y * 16, 2).await;
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

/// The source's `battle_two_trainers(A, B)` (Gen3 `trainerbattle_two_trainers`): no double
/// battle before Milestone 11, so the source's documented fallback, two single battles in a row
/// without a heal between them ([`battle_trainer`] each, with each trainer's defeat text).
/// Reported. Returns the second battle's outcome (a loss in the first whites out).
pub async fn battle_two_trainers(
    ctx: &Ctx,
    (a, a_defeat): (TrainerId, TextId),
    (b, b_defeat): (TrainerId, TextId),
) -> BattleOutcome {
    fallback(ctx, "battle_two_trainers").await;
    battle_trainer(ctx, a, a_defeat).await;
    battle_trainer(ctx, b, b_defeat).await
}

/// The source's `show_sync(n)`: the Mallette's animated synchronisation gauge (`VAR_SIL_SYNC`,
/// which the scene sets first). No gauge screen yet (the Mallette's panels are SIL2 content on
/// M9's kit): nothing is drawn; the scene's next text gives the value. Reported.
pub async fn show_sync(ctx: &Ctx, _percent: u16) {
    fallback(ctx, "show_sync").await;
}

/// The source's `mart(MART_*)` (Gen3 `pokemart`): no shop screen before Milestone 9 (its
/// Task J's `mart`), so the clerk greets and says goodbye on Gen3's own `gText_*` and nothing is
/// sold. Reported. `_items` is the stock the shop will sell (unused until then), kept at the call
/// site so that the swap to the real screen is one line.
pub async fn mart(ctx: &Ctx, _items: &'static [ItemId]) {
    ctx.lock().await;
    ctx.face_player().await;
    say(ctx, gText_HowMayIServeYou).await;
    fallback(ctx, "mart").await;
    say(ctx, gText_PleaseComeAgain).await;
    ctx.release().await;
}

// ---- battles and buildings ----

/// The source's `battle_trainer(TRAINER, Defeat)`: a scripted trainer battle (a leader, a boss
/// talked to; not a sight trainer, which is [`trainer`]). Unless the player lost, the trainer's
/// beaten flag is set, as Gen3 `CB2_EndTrainerBattle` does; a loss whites out and ends the
/// script.
pub async fn battle_trainer(ctx: &Ctx, trainer: TrainerId, defeat: TextId) -> BattleOutcome {
    let outcome = ctx.battle_trainer(trainer, defeat).await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(trainer_flag(trainer), true).await;
    }
    outcome
}

/// [`battle_trainer`] under `rules` (`silice::rules`) instead of the trainer's own; with
/// `no_whiteout`, a loss answers `Lost` and the script goes on (R-7, no whiteout).
pub async fn battle_trainer_with(
    ctx: &Ctx,
    trainer: TrainerId,
    defeat: TextId,
    no_whiteout: bool,
    rules: &'static engine::battle::BattleRules,
) -> BattleOutcome {
    let outcome = ctx
        .battle_trainer_rules(trainer, defeat, no_whiteout, rules)
        .await;
    if !matches!(outcome, BattleOutcome::Lost | BattleOutcome::Drew) {
        ctx.set_flag(trainer_flag(trainer), true).await;
    }
    outcome
}

/// Gen3 `Common_EventScript_PkmnCenterNurse` (`data/scripts/pkmn_center_nurse.inc`) without the
/// Gold Card, Pokérus and Union Room branches: rest the party (yes/no), heal, goodbye, on Gen3's
/// own `gText_*`. The heal machine's field effect (`FLDEFF_POKECENTER_HEAL`) is a pause.
pub async fn pokecenter_nurse(ctx: &Ctx) {
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

// ---- the post-game (Chapter 12) ----

/// `play_cry(SPECIES, …)`: no cries yet (the texts say them). Reported.
pub async fn play_cry(ctx: &Ctx) {
    fallback(ctx, "play_cry").await;
}

/// The source's `battle_wild_boss(SPECIES, level, aura: AllStats+1)` (§9): a wild battle under
/// [`crate::silice::rules::BOSS`]: the foe's stats +1 at the start, and no catching.
pub async fn battle_wild_boss(ctx: &Ctx, species: SpeciesId, level: u8) -> BattleOutcome {
    ctx.battle_wild_rules(species, level, &crate::silice::rules::BOSS)
        .await
}

/// The source's `battle_mirror()` (§9): `trainer` (its name and pic) battles with a healed copy
/// of the player's party ([`crate::silice::rules::MIRROR`]). Its beaten flag is set as
/// [`battle_trainer`]'s.
pub async fn battle_mirror(ctx: &Ctx, trainer: TrainerId, defeat: TextId) -> BattleOutcome {
    battle_trainer_with(ctx, trainer, defeat, false, &crate::silice::rules::MIRROR).await
}

/// The source's `give_egg(SPECIES, ivs)` (§9): the engine has no eggs (Milestone 13), so the
/// source's fallback, `give_mon(SPECIES, 5)`. Reported. Returns `give_mon`'s answer.
pub async fn give_egg(ctx: &Ctx, species: SpeciesId) -> u16 {
    fallback(ctx, "give_egg").await;
    ctx.give_mon(species, 5).await
}

/// The source's `enable_national_dex()`: Emerald's `special EnableNationalPokedex`.
pub async fn enable_national_dex(ctx: &Ctx) {
    special(ctx, SPECIAL_EnableNationalPokedex).await;
}

/// A rule of the Antennes de Combat (§4.3, `party_respects(RÈGLE)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartyRule {
    /// « SANS OBJETS »: no held item.
    NoItems,
    /// « MONO-ACIER »: Steel mons only.
    MonoSteel,
    /// « TRIO »: three mons at most.
    Trio,
}

/// The source's `party_respects(RÈGLE)` (§9): the party checked against `rule` (held items,
/// types, size). The rule is checked at the door only: inside the battle the bag stays open
/// (« ni du Sac » is not enforced: no battle rule for it yet).
pub async fn party_respects(ctx: &Ctx, rule: PartyRule) -> bool {
    let size = ctx.party_size().await as u8;
    match rule {
        PartyRule::Trio => size <= 3,
        PartyRule::NoItems => {
            for slot in 0..size {
                if ctx.query(Query::PartyHeldItem(slot)).await != 0 {
                    return false;
                }
            }
            true
        }
        PartyRule::MonoSteel => {
            for slot in 0..size {
                let s = SpeciesId(ctx.query(Query::PartySpecies(slot)).await);
                let types = crate::battle::TABLES.species_info(s).types;
                if !types.contains(&engine::mon::Type::Steel) {
                    return false;
                }
            }
            true
        }
    }
}
