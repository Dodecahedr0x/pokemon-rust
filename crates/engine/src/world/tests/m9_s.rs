//! Milestone 9 Task S: the in-game save (`save_now`'s normalisation), the save dialog
//! (`special SaveGame` and the start menu's SAVE), the main menu, and the Hall of Fame with the
//! credits, on the synthetic game.

use super::*;
use crate::checkpoint::{Checkpoint, MAX_ENCODED, NotCapturable, SaveError, SaveMeta, encoded_len};
use crate::data::{StartAction, StartItem};
use crate::script::Speed;
use crate::ui::hall_of_fame::{CREDITS_FRAMES, Stage};
use crate::ui::save::{Prompt, SAVE_TIMER};
use crate::ui::{ScreenKind, ScreenState};
use crate::world::MainMenuChoice;

static START: [StartItem; 2] = [
    StartItem {
        label: TextId(0),
        action: StartAction::Save,
        shown_if: None,
        description: None,
    },
    StartItem {
        label: TextId(0),
        action: StartAction::Exit,
        shown_if: None,
        description: None,
    },
];

/// [`Game`] with a start menu: SAVE, EXIT.
struct SaveMenuGame;

impl GameData for SaveMenuGame {
    fn map(&self, id: MapId) -> &'static MapData {
        Game.map(id)
    }
    fn special_ids(&self) -> SpecialIds {
        Game.special_ids()
    }
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData> {
        Game.sprite(gfx)
    }
    fn player_sprite(&self) -> &'static SpriteData {
        Game.player_sprite()
    }
    fn font(&self) -> &'static Font {
        Game.font()
    }
    fn window(&self) -> &'static WindowGfx {
        Game.window()
    }
    fn text(&self, id: TextId) -> &'static [u8] {
        Game.text(id)
    }
    fn ui_text(&self, t: UiText) -> &'static [u8] {
        Game.ui_text(t)
    }
    fn behavior(&self, raw: u8) -> Behavior {
        Game.behavior(raw)
    }
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script {
        Game.script(id, ctx)
    }
    fn battle(&self) -> &'static crate::mon::BattleTables {
        Game.battle()
    }
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        Game.heal_location(id)
    }
    fn new_game(&self) -> NewGame {
        Game.new_game()
    }
    fn start_menu(&self) -> &'static [StartItem] {
        &START
    }
}

fn dialog(w: &World) -> Option<&crate::ui::save::SaveDialog> {
    match w.screens().and_then(|s| s.top()) {
        Some((_, ScreenState::SaveDialog(d))) => Some(d),
        _ => None,
    }
}

fn prompt(w: &World) -> Option<Prompt> {
    dialog(w).map(|d| d.prompt())
}

/// Steps idle frames until `done` (at most `max`).
fn until_g(w: &mut World, game: &impl GameData, max: u32, done: impl Fn(&World) -> bool) {
    for _ in 0..max {
        if done(w) {
            return;
        }
        w.step(game, Buttons::NONE).validate().unwrap();
    }
    panic!("never reached");
}

fn tap_g(w: &mut World, game: &impl GameData, keys: Buttons) {
    w.step(game, keys).validate().unwrap();
    w.step(game, Buttons::NONE).validate().unwrap();
}

/// Steps one frame, returning whether it raised `Event::Saved { ok }`.
fn saved_event(w: &mut World, game: &impl GameData, keys: Buttons) -> Option<bool> {
    w.step(game, keys).validate().unwrap();
    w.events().iter().find_map(|e| match e {
        Event::Saved { ok } => Some(*ok),
        _ => None,
    })
}

/// A restore of `cp` is capturable at once and equals its header's hash.
fn check_restore(cp: &Checkpoint) -> World {
    let back = World::restore(&Game, cp).expect("the save restores");
    assert_eq!(back.capturable(), Ok(()));
    assert_eq!(back.hash(), cp.header.world_hash);
    back
}

#[test]
fn a_capturable_world_saves_as_its_checkpoint() {
    let w = started();
    let cp = w.save_snapshot(&Game).unwrap();
    assert_eq!(cp, w.checkpoint(&Game).unwrap());
    check_restore(&cp);
}

static WALK: [crate::script::Step; 2] = [
    crate::script::Step::Walk(Dir::Left, Speed::Normal),
    crate::script::Step::Walk(Dir::Left, Speed::Normal),
];

#[test]
fn a_mid_script_save_ends_moves_closes_the_box_and_keeps_vars() {
    let mut w = started();
    const VAR_TEMP: VarId = VarId(0x4000);
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.lock_all().await;
            ctx.set_var(VAR_TEMP, 7).await;
            ctx.move_obj(1, &WALK).await;
            ctx.message(PLAYER_TEXT).await;
            ctx.wait_message().await;
            ctx.wait_button().await;
            ctx.release_all().await;
        })
    });
    // Mid-step for NPC 1, the message printing, the field locked.
    run(&mut w, Buttons::NONE, 5);
    assert!(w.capturable().is_err());
    let npc_at = w.objects().get(1).unwrap().pos();
    let cp = w.save_snapshot(&Game).expect("a mid-script save");
    let back = check_restore(&cp);
    assert!(!back.script_running());
    assert_eq!(back.var(VAR_TEMP), 7, "temp vars are saved");
    let npc = back.objects().get(1).unwrap();
    assert_eq!(npc.pos(), npc_at, "the step's destination cell");
    assert!(npc.is_settled());
    assert!(back.objects().iter().all(|o| !o.frozen), "unfrozen");
    assert!(back.text().is_pristine());
    // The live world goes on unchanged by the save.
    let h = w.hash();
    let _ = w.save_snapshot(&Game).unwrap();
    assert_eq!(w.hash(), h);
}

#[test]
fn battles_and_warps_cannot_be_saved() {
    let mut w = started();
    w.s.transition.start(&Game, HOUSE, Some(0), 3, 3);
    assert!(matches!(
        w.save_snapshot(&Game),
        Err(SaveError::NotCapturable(NotCapturable::Transition))
    ));
}

/// `special SaveGame` with A through every message: (frames to the save, the answer).
fn script_save(w: &mut World, keys: impl Fn(&World) -> Buttons) -> (u32, bool) {
    start(w, |ctx| {
        Box::pin(async move {
            let saved = ctx.save_dialog().await;
            ctx.set_var(VAR_RESULT, saved as u16 + 10).await;
        })
    });
    let mut saved_at = 0;
    for n in 0..2_000 {
        if !w.script_running() {
            return (saved_at, w.var(VAR_RESULT) == 11);
        }
        let k = keys(w);
        if saved_event(w, &Game, k).is_some() {
            saved_at = n;
        }
        run(w, Buttons::NONE, 1);
    }
    panic!("the save dialog never closed");
}

fn press_a_on_messages(w: &World) -> Buttons {
    match prompt(w) {
        Some(Prompt::Message | Prompt::YesNo { cursor: 0, .. }) => Buttons::A,
        Some(Prompt::YesNo { .. }) => Buttons::UP,
        _ => Buttons::NONE,
    }
}

#[test]
fn special_save_game_asks_saves_and_answers() {
    let mut w = started();
    let (at, ok) = script_save(&mut w, press_a_on_messages);
    assert!(ok && at > 0, "saved (at {at})");
    // The next save asks to overwrite (`gDifferentSaveFile` cleared), default yes: no.
    let (_, ok) = script_save(&mut w, |w| match prompt(w) {
        Some(Prompt::YesNo {
            cursor,
            overwrite: true,
        }) => {
            assert_eq!(cursor, 0, "default yes");
            Buttons::B
        }
        _ => press_a_on_messages(w),
    });
    assert!(!ok, "no to the overwrite question");
}

#[test]
fn no_and_b_cancel_without_saving() {
    let mut w = started();
    let (at, ok) = script_save(&mut w, |w| match prompt(w) {
        Some(Prompt::Message) => Buttons::A,
        Some(Prompt::YesNo { .. }) => Buttons::B,
        _ => Buttons::NONE,
    });
    assert_eq!((at, ok), (0, false));
}

#[test]
fn a_different_file_in_flash_warns_with_default_no() {
    let mut w = started();
    w.set_save_present(true);
    let (_, ok) = script_save(&mut w, |w| match prompt(w) {
        Some(Prompt::YesNo {
            cursor,
            overwrite: true,
        }) => {
            assert_eq!(cursor, 1, "default no");
            Buttons::A
        }
        Some(Prompt::YesNo { .. } | Prompt::Message) => Buttons::A,
        _ => Buttons::NONE,
    });
    assert!(!ok, "A on the warning's default no");
}

#[test]
fn the_success_timer_counts_save_start_timer_frames() {
    assert_eq!(SAVE_TIMER, 60);
    if let Some(root) = std::env::var_os("PKR_DECOMP") {
        let src = std::fs::read_to_string(std::path::Path::new(&root).join("src/start_menu.c"))
            .expect("start_menu.c");
        assert!(src.contains("sSaveDialogTimer = 60;"), "SaveStartTimer");
    }
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.save_dialog().await;
        })
    });
    // Through the questions to the result's message.
    for _ in 0..600 {
        if matches!(prompt(&w), Some(Prompt::Done { .. })) {
            break;
        }
        let k = press_a_on_messages(&w);
        run(&mut w, k, 1);
        run(&mut w, Buttons::NONE, 1);
    }
    assert_eq!(prompt(&w), Some(Prompt::Done { ok: true }));
    let mut n = 0;
    while w.screens_open() > 0 {
        run(&mut w, Buttons::NONE, 1);
        n += 1;
        assert!(n <= u32::from(SAVE_TIMER) + 2);
    }
    assert!(n + 2 >= u32::from(SAVE_TIMER), "{n} frames");
}

#[test]
fn a_failed_write_answers_not_saved() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            let saved = ctx.save_dialog().await;
            ctx.set_var(VAR_RESULT, saved as u16 + 10).await;
        })
    });
    let mut failed = false;
    for _ in 0..3_000 {
        if !w.script_running() {
            break;
        }
        let k = match prompt(&w) {
            // `SaveErrorTimer` waits for A held once its timer ran out.
            Some(Prompt::Done { ok: false }) => Buttons::A,
            _ => press_a_on_messages(&w),
        };
        if saved_event(&mut w, &Game, k) == Some(true) {
            w.save_failed();
            failed = true;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    assert!(failed);
    assert_eq!(w.var(VAR_RESULT), 10, "not saved");
}

#[test]
fn start_menu_save_closes_the_menu_and_continue_restores_the_same_world() {
    let g = SaveMenuGame;
    let mut w = World::new(&g);
    for _ in 0..16 {
        w.step(&g, Buttons::NONE);
    }
    tap_g(&mut w, &g, Buttons::START);
    until_g(&mut w, &g, 10, |w| w.screens_open() == 1);
    // SAVE is the first entry.
    tap_g(&mut w, &g, Buttons::A);
    assert_eq!(
        w.screens().unwrap().kinds().last(),
        Some(ScreenKind::SaveDialog)
    );
    let mut save = None;
    for _ in 0..1_000 {
        if w.screens_open() == 0 {
            break;
        }
        let k = press_a_on_messages(&w);
        if saved_event(&mut w, &g, k) == Some(true) {
            save = Some(w.save_snapshot(&g).unwrap());
        }
        w.step(&g, Buttons::NONE);
    }
    assert_eq!(w.screens_open(), 0, "saving closes the start menu");
    let cp = save.expect("saved");
    let back = World::restore(&g, &cp).unwrap();
    assert_eq!(back.capturable(), Ok(()));
    assert_eq!(back.map(), w.map());
    assert_eq!(back.objects().player().pos(), w.objects().player().pos());
    assert!(encoded_len(&cp) <= MAX_ENCODED);
}

#[test]
fn start_menu_save_cancelled_shows_the_menu_again() {
    let g = SaveMenuGame;
    let mut w = World::new(&g);
    for _ in 0..16 {
        w.step(&g, Buttons::NONE);
    }
    tap_g(&mut w, &g, Buttons::START);
    tap_g(&mut w, &g, Buttons::A);
    until_g(&mut w, &g, 600, |w| {
        matches!(prompt(w), Some(Prompt::YesNo { .. }))
    });
    tap_g(&mut w, &g, Buttons::B);
    tap_g(&mut w, &g, Buttons::NONE);
    assert_eq!(w.screens_open(), 1);
    assert!(matches!(
        w.screens().unwrap().top(),
        Some((ScreenKind::StartMenu, _))
    ));
}

fn meta() -> SaveMeta {
    let w = started();
    let cp = w.checkpoint(&Game).unwrap();
    cp.save_meta(&Game.special_ids())
}

fn menu_choice(keys: &[Buttons]) -> MainMenuChoice {
    let mut w = World::main_menu(&Game, meta());
    let _ = w.initial_frame();
    run(&mut w, Buttons::NONE, 20);
    for &k in keys {
        tap(&mut w, k, 2);
    }
    for _ in 0..200 {
        if let Some(c) = w.main_menu_choice() {
            return c;
        }
        run(&mut w, Buttons::NONE, 1);
    }
    panic!("no choice");
}

#[test]
fn the_main_menu_continues_or_starts_over() {
    assert_eq!(menu_choice(&[Buttons::A]), MainMenuChoice::Continue);
    assert_eq!(
        menu_choice(&[Buttons::DOWN, Buttons::A]),
        MainMenuChoice::NewGame
    );
    // No wrap: UP on CONTINUE stays there.
    assert_eq!(
        menu_choice(&[Buttons::UP, Buttons::A]),
        MainMenuChoice::Continue
    );
}

fn hof_stage(w: &World) -> Option<Stage> {
    match w.screens().and_then(|s| s.top()) {
        Some((_, ScreenState::HallOfFame(h))) => Some(h.stage()),
        _ => None,
    }
}

#[test]
fn the_hall_of_fame_shows_the_record_then_the_credits_then_saves_in_the_room() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.hall_of_fame().await;
            ctx.warp(HOUSE, None, 3, 3).await;
        })
    });
    until_g(&mut w, &Game, 100, |w| hof_stage(w).is_some());
    until_g(&mut w, &Game, 2_000, |w| {
        hof_stage(w) == Some(Stage::Champion)
    });
    run(&mut w, Buttons::NONE, 30);
    assert_eq!(hof_stage(&w), Some(Stage::Champion), "waits for A");
    tap(&mut w, Buttons::A, 2);
    assert_eq!(hof_stage(&w), Some(Stage::Credits));
    // START skips the credits' pages (they would last `CREDITS_FRAMES`).
    const { assert!(CREDITS_FRAMES > 8_000) };
    tap(&mut w, Buttons::START, 2);
    assert_eq!(hof_stage(&w), Some(Stage::TheEnd));
    tap(&mut w, Buttons::B, 2);
    let mut saved = None;
    for _ in 0..600 {
        if let Some(ok) = saved_event(&mut w, &Game, Buttons::NONE) {
            saved = Some(ok);
            break;
        }
    }
    assert_eq!(saved, Some(true), "saved once idle in the room");
    assert_eq!(w.map(), HOUSE);
    let cp = w.save_snapshot(&Game).unwrap();
    check_restore(&cp);
}

/// Silice `save_game` (R-6): no dialog, `Event::Saved` raised, `Bool` answered, and the save
/// restores as CONTINUE would (the script gone, the vars kept).
#[test]
fn a_scripted_save_saves_without_the_dialog() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.set_var(VAR_RESULT, 7).await;
            let saved = ctx.save_game().await;
            ctx.set_var(VAR_RESULT, saved as u16 + 10).await;
        })
    });
    let mut saved = None;
    let mut cp = None;
    for _ in 0..20 {
        if let Some(ok) = saved_event(&mut w, &Game, Buttons::NONE) {
            saved = Some(ok);
            cp = Some(w.save_snapshot(&Game).unwrap());
        }
        assert!(w.screens().is_none(), "no dialog");
        if !w.script_running() {
            break;
        }
    }
    assert_eq!(saved, Some(true));
    assert!(!w.script_running());
    assert_eq!(w.var(VAR_RESULT), 11, "answered true");
    let back = check_restore(&cp.unwrap());
    assert_eq!(back.var(VAR_RESULT), 7, "saved before the answer");
    assert!(!back.script_running());
}

/// The credits alone: from the credits' pages, no record and no save afterwards.
#[test]
fn the_credits_alone_end_without_saving() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.credits().await;
            ctx.set_var(VAR_RESULT, 3).await;
        })
    });
    until_g(&mut w, &Game, 100, |w| hof_stage(w).is_some());
    assert_eq!(hof_stage(&w), Some(Stage::Credits));
    // The field's fade-out and the screen's fade-in first.
    run(&mut w, Buttons::NONE, 60);
    tap(&mut w, Buttons::START, 2);
    assert_eq!(hof_stage(&w), Some(Stage::TheEnd));
    tap(&mut w, Buttons::B, 2);
    for _ in 0..600 {
        assert_eq!(saved_event(&mut w, &Game, Buttons::NONE), None, "no save");
    }
    assert_eq!(w.var(VAR_RESULT), 3);
    assert!(!w.script_running());
}

/// `reload_last_save` raises `Event::ReloadLastSave` once and is never answered.
#[test]
fn reload_last_save_raises_its_event_once_and_waits() {
    let mut w = started();
    start(&mut w, |ctx| {
        Box::pin(async move {
            ctx.reload_last_save().await;
            ctx.set_var(VAR_RESULT, 9).await;
        })
    });
    let mut raised = 0;
    for _ in 0..300 {
        run(&mut w, Buttons::NONE, 1);
        raised += (w.events().iter())
            .filter(|e| matches!(e, Event::ReloadLastSave))
            .count();
    }
    assert_eq!(raised, 1);
    assert!(w.script_running(), "never answered");
    assert_ne!(w.var(VAR_RESULT), 9);
}
