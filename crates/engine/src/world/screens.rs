//! Screen hosting (Milestone 9 Task K owns the file): opening a screen from START or a
//! request, pushing and popping, the fades, handing the screen over and restoring the field,
//! and answering the waiting request with the screen's result.
//!
//! - **Overlays** (`ui::kit::is_overlay`: the start menu, the save dialog) draw their windows
//!   on BG0 over the field, which keeps being drawn (frozen: no object moves while a screen
//!   is open, Gen3 `FreezeObjectEvents`). They open and close at once.
//! - **Full-screen screens** fade the screen to black (Gen3 `BeginNormalPaletteFade`, 16
//!   frames), take BG0 alone (`kit::window::enter_full`), draw on that black frame, then fade
//!   in. Between two full screens (party → summary) the same fade runs, and the screen below
//!   redraws when it gets the result back. Back to the field (or to an overlay over it), the
//!   field reloads spread under black (`Field::restore_spread`, as after a battle), then fades
//!   in; a field text box open before the screen is closed (Gen3 reloads the field and its
//!   windows are gone).
//! - Input reaches the top screen only while [`Phase::Running`]: Gen3 menus ignore input while
//!   `gPaletteFade.active`.
//! - A script request (`host/screens.rs`) marks the stack `waiting`; the bottom screen's
//!   result goes to `State::answer` when the last screen closes. A map script's request
//!   (immediate) never opens a screen.

use super::mode::Mode;
use super::{Event, State, World, draw_field};
use crate::data::{GameData, ItemId};
use crate::script::Fade;
use crate::ui::kit::{self, FADE_FRAMES, Pending, Phase, window};
use crate::ui::{
    PartyMode, ScreenCx, ScreenInput, ScreenKind, ScreenNote, ScreenOut, ScreenResult, ScreenStack,
    ScreenState,
};
use crate::video::Frame;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// `State::answer` for a screen closed with `Cancel` (the request maps it to its default).
pub(in crate::world) const ANSWER_CANCEL: u16 = u16::MAX;

/// START pressed while the field is idle and the player is not moving (Gen3
/// `ProcessPlayerFieldInput`'s `pressedStartButton` → `ShowStartMenu`). Returns whether a
/// screen opened (the frame's field control stops there). A game without a start menu
/// (`GameData::start_menu` empty) opens nothing.
pub(super) fn start_pressed(s: &mut State, game: &impl GameData, f: &mut Frame) -> bool {
    if game.start_menu().is_empty() {
        return false;
    }
    // The start menu draws its window on its first screen frame (the next one).
    open(s, game, ScreenKind::StartMenu, false, f);
    true
}

/// Opens `kind` over the field; `waiting`: a script request waits for its result.
pub(in crate::world) fn open(
    s: &mut State,
    game: &impl GameData,
    kind: ScreenKind,
    waiting: bool,
    f: &mut Frame,
) {
    let mut stack = Box::new(ScreenStack::new());
    stack.push(kind);
    stack.host.waiting = waiting;
    s.event(Event::ScreenOpened(kind.tag()));
    if kit::is_overlay(kind) {
        s.text.close(f);
        window::enter_overlay(game, s.options.frame, f);
    } else {
        stack.host.phase = Phase::Out;
        stack.host.pending = Pending::Enter;
        stack.host.level = 0;
    }
    s.mode = Mode::Screens(stack);
}

/// Encodes a screen result for `State::answer`.
const fn encode(r: ScreenResult) -> u16 {
    match r {
        ScreenResult::Cancel => ANSWER_CANCEL,
        ScreenResult::Value(v) => v,
        ScreenResult::Bool(b) => b as u16,
    }
}

/// Reloads the field under black ([`super::battle`]'s spread restore): tilesets, palettes and
/// BG1–BG3 controls over the next frames, object sprites re-uploaded, BG0 off.
fn restore_field(s: &mut State, f: &mut Frame) {
    f.bulk = true;
    s.field.restore_spread();
    s.objects.invalidate_vram();
    s.weather.resume(); // Gen3 `ResumeMap` → `ResumePausedWeather`
    s.text.close(f);
}

impl World {
    /// The bag's USE on `item` while its screen is missing (Milestone 9 Task B): opens what
    /// using it opens, for tools and the explorer. An item with a party use (a TM or HM, a
    /// medicine, an evolution item) opens the party menu on it (Gen3 `ItemUseOutOfBattle_TMHM`
    /// and the others → `PARTY_ACTION_USE_ITEM`, without the bag's "Booted up a TM" messages). `false` (nothing opened) unless the field is idle, the bag holds the
    /// item and the item has a party use.
    pub fn use_item(&mut self, game: &impl GameData, item: ItemId) -> bool {
        let idle = matches!(self.s.mode, Mode::Field)
            && !self.runner.is_running()
            && !self.s.transition.is_active()
            && self.s.objects.player().is_settled();
        let party_use = crate::item_use::field_use(game, item) == crate::item_use::FieldUse::Party;
        if !idle || self.s.bag.count(item) == 0 || !party_use {
            return false;
        }
        // A full-screen screen sets the screen up once faded out: nothing to draw now.
        let mut f = Frame::default();
        open(
            &mut self.s,
            game,
            ScreenKind::Party(PartyMode::UseItem(item)),
            false,
            &mut f,
        );
        true
    }

    /// Steps the top screen with `input`, giving it the waiting results.
    fn step_top(
        &mut self,
        game: &impl GameData,
        stack: &mut ScreenStack,
        f: &mut Frame,
        input: ScreenInput,
    ) -> ScreenOut {
        let frame = self.frame;
        let s = &mut self.s;
        let mut reports = Vec::new();
        let mut notes = Vec::new();
        let (saved, child) = (stack.saved.take(), stack.child.take());
        let out = match stack.top_mut() {
            None => ScreenOut::Done(ScreenResult::Cancel),
            Some(top) => {
                let mut cx = ScreenCx {
                    game,
                    f,
                    rng: &mut s.rng,
                    party: &mut s.party,
                    bag: &mut s.bag,
                    money: &mut s.money,
                    pc: &mut s.pc,
                    options: &mut s.options,
                    coins: &mut s.coins,
                    pokedex: &mut s.pokedex,
                    registered_item: &mut s.registered_item,
                    vars: &mut s.vars,
                    player_name: &mut s.player_name,
                    rival_name: &mut s.rival_name,
                    map: s.field.map(),
                    frame,
                    saved,
                    save_question: s.save.question(),
                    child,
                    reports: &mut reports,
                    notes: &mut notes,
                };
                top.step(&mut cx, input)
            }
        };
        for r in reports {
            super::battle::report(s, r);
        }
        for n in notes {
            match n {
                ScreenNote::Cry(species) => s.event(Event::Cry(species)),
                ScreenNote::Evolved(species) => s.event(Event::Evolved(species)),
            }
        }
        out
    }

    /// One frame while screens own the frame (`Mode::Screens`).
    pub(super) fn step_screens(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        f: &mut Frame,
    ) {
        match stack.host.phase {
            Phase::Running => self.screens_running(game, stack, f),
            Phase::Out => self.screens_out(game, stack, f),
            Phase::Restoring => self.screens_restoring(game, stack, f),
            Phase::In => {
                stack.host.level = stack.host.level.saturating_sub(1);
                if stack.host.level == 0 {
                    stack.host.phase = Phase::Running;
                }
                self.show(game, stack, f, ScreenInput::default());
            }
        }
    }

    /// Steps the top screen and draws the field under an overlay; the fade level last.
    /// Anything but `Running` from the screen is kept for the next running frame.
    fn show(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        f: &mut Frame,
        input: ScreenInput,
    ) {
        if !stack.host.full {
            draw_field(game, &mut self.s, f);
        }
        let out = self.step_top(game, &mut stack, f, input);
        if out != ScreenOut::Running {
            // Without input a screen only draws; a screen that answers at once (a stub)
            // closes on the next running frame.
            self.s.log("screen answered while fading");
        }
        f.brightness = stack.host.level;
        f.lighten = false;
        self.s.mode = Mode::Screens(stack);
    }

    fn screens_running(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        f: &mut Frame,
    ) {
        if !stack.host.full {
            draw_field(game, &mut self.s, f);
            f.brightness = 0;
            f.lighten = false;
        }
        let input = ScreenInput {
            pressed: self.s.input.pressed,
            held: self.s.input.held,
        };
        let out = self.step_top(game, &mut stack, f, input);
        let full = stack.host.full;
        match out {
            ScreenOut::Running => {}
            ScreenOut::Push(kind) => {
                if !stack.push(kind) {
                    self.s.log("screen stack full");
                } else {
                    self.s.event(Event::ScreenOpened(kind.tag()));
                    if kit::is_overlay(kind) && !full {
                        window::enter_overlay(game, self.s.options.frame, f);
                    } else {
                        stack.host.phase = Phase::Out;
                        stack.host.pending = Pending::Enter;
                    }
                }
            }
            ScreenOut::Done(result) => {
                let below = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|i| stack.kinds().nth(i));
                match below {
                    None if !full => {
                        // An overlay closes at once.
                        self.s.text.close(f);
                        self.finish(game, result, stack.host.waiting, false, f);
                        return;
                    }
                    Some(b) if !full && kit::is_overlay(b) => {
                        stack.pop();
                        stack.child = Some(result);
                        window::enter_overlay(game, self.s.options.frame, f);
                    }
                    _ => {
                        stack.host.phase = Phase::Out;
                        stack.host.pending = Pending::Pop(result);
                    }
                }
            }
            ScreenOut::RunScript(id) => {
                if full {
                    stack.host.phase = Phase::Out;
                    stack.host.pending = Pending::Script(id);
                } else {
                    self.s.text.close(f);
                    self.finish(game, ScreenResult::Cancel, stack.host.waiting, false, f);
                    self.run_after(game, Pending::Script(id), f);
                    return;
                }
            }
            ScreenOut::UseOnField(item) => {
                if full {
                    stack.host.phase = Phase::Out;
                    stack.host.pending = Pending::Item(item);
                } else {
                    self.s.text.close(f);
                    self.finish(game, ScreenResult::Cancel, stack.host.waiting, false, f);
                    self.run_after(game, Pending::Item(item), f);
                    return;
                }
            }
            ScreenOut::Fly(i) => {
                // The fly map is a full screen: the field reloads under black first.
                stack.host.phase = Phase::Out;
                stack.host.pending = Pending::Fly(i);
            }
            ScreenOut::Save => {
                let ok = super::save::dialog_save(self, game);
                stack.saved = Some(ok);
                self.s.event(Event::Saved { ok });
            }
        }
        self.s.mode = Mode::Screens(stack);
    }

    fn screens_out(&mut self, game: &impl GameData, mut stack: Box<ScreenStack>, f: &mut Frame) {
        let h = &mut stack.host;
        h.level = (h.level + 1).min(FADE_FRAMES);
        if !h.full {
            draw_field(game, &mut self.s, f);
        }
        if stack.host.level < FADE_FRAMES {
            f.brightness = stack.host.level;
            f.lighten = false;
            self.s.mode = Mode::Screens(stack);
            return;
        }
        // Black: what was waiting for it.
        match stack.host.pending {
            Pending::Enter => self.enter_top(game, stack, f),
            Pending::Pop(r) if self.boot_screen_swap(&stack, r) => {
                self.swap_boot_screen(game, stack, r, f);
            }
            Pending::Pop(r) if stack.len() > 1 => {
                stack.pop();
                stack.child = Some(r);
                stack.host.pending = Pending::None;
                match stack.kinds().last() {
                    Some(k) if !kit::is_overlay(k) => self.enter_top(game, stack, f),
                    _ => self.begin_restore(stack, f),
                }
            }
            Pending::Pop(_) | Pending::Script(_) | Pending::Item(_) | Pending::Fly(_) => {
                self.begin_restore(stack, f)
            }
            Pending::None => {
                stack.host.phase = Phase::In;
                self.show(game, stack, f, ScreenInput::default());
            }
        }
    }

    /// Title Start opens the main menu, and B on that menu returns to the title, without
    /// leaving for the field (`ui/title.rs`).
    fn boot_screen_swap(&self, stack: &ScreenStack, r: ScreenResult) -> bool {
        if !self.s.save.main_menu || stack.len() != 1 {
            return false;
        }
        match (stack.kinds().next(), r) {
            (Some(ScreenKind::Title), ScreenResult::Value(v)) if v == crate::ui::title::TO_MENU => {
                true
            }
            (Some(ScreenKind::MainMenu), ScreenResult::Cancel) => true,
            _ => false,
        }
    }

    fn swap_boot_screen(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        r: ScreenResult,
        f: &mut Frame,
    ) {
        let kind = match r {
            ScreenResult::Cancel => ScreenKind::Title,
            _ => ScreenKind::MainMenu,
        };
        stack.pop();
        stack.child = None;
        stack.push(kind);
        if let (Some(ScreenState::MainMenu(m)), Some(meta)) =
            (stack.top_mut(), self.s.save.boot_meta)
        {
            m.set_meta(meta);
        }
        self.s.event(Event::ScreenOpened(kind.tag()));
        self.enter_top(game, stack, f);
    }

    /// On a black frame: the top (full-screen) screen takes BG0 and draws.
    fn enter_top(&mut self, game: &impl GameData, mut stack: Box<ScreenStack>, f: &mut Frame) {
        window::enter_full(game, self.s.options.frame, f);
        f.objs.clear();
        stack.host.full = true;
        stack.host.pending = Pending::None;
        stack.host.phase = Phase::In;
        self.show(game, stack, f, ScreenInput::default());
    }

    /// On a black frame: the field starts reloading.
    fn begin_restore(&mut self, mut stack: Box<ScreenStack>, f: &mut Frame) {
        restore_field(&mut self.s, f);
        stack.host.full = false;
        stack.host.phase = Phase::Restoring;
        f.brightness = FADE_FRAMES;
        self.s.mode = Mode::Screens(stack);
    }

    fn screens_restoring(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        f: &mut Frame,
    ) {
        draw_field(game, &mut self.s, f);
        f.brightness = FADE_FRAMES;
        f.lighten = false;
        if self.s.field.uploading() {
            self.s.mode = Mode::Screens(stack);
            return;
        }
        match stack.host.pending {
            Pending::None => {
                // Back to the overlay below (party → start menu).
                window::enter_overlay(game, self.s.options.frame, f);
                stack.host.phase = Phase::In;
                self.show(game, stack, f, ScreenInput::default());
            }
            p => {
                let result = match p {
                    Pending::Pop(r) => r,
                    _ => ScreenResult::Cancel,
                };
                self.finish(game, result, stack.host.waiting, true, f);
                self.run_after(game, p, f);
            }
        }
    }

    /// Every screen is closed: the field owns the frame again (fading in from black if it was
    /// reloaded), and a waiting request gets `result`.
    fn finish(
        &mut self,
        game: &impl GameData,
        result: ScreenResult,
        waiting: bool,
        fade_in: bool,
        f: &mut Frame,
    ) {
        let s = &mut self.s;
        s.mode = Mode::Field;
        s.event(Event::ScreenClosed);
        if waiting {
            s.answer = Some(encode(result));
        }
        if fade_in {
            s.fader.start(Fade::FromBlack);
        }
        draw_field(game, s, f);
    }

    /// The script or item use a closing screen asked for.
    fn run_after(&mut self, game: &impl GameData, p: Pending, f: &mut Frame) {
        match p {
            Pending::Script(id) => {
                // Gen3 `ScriptContext_SetupScript` after the menu, with the field locked.
                self.s.objects.freeze_all();
                self.start_script(game, id, None);
            }
            Pending::Item(item) => {
                if let Some(id) = crate::item_use::use_on_field(&mut self.s, game, item) {
                    self.start_script(game, id, None);
                }
            }
            // Gen3 `FieldCallback_UseFly` → `FldEff_UseFly` (the bird and the take-off: not
            // drawn, degraded), then the warp `SetFlyDestination` set
            // (`SetWarpDestinationToHealLocation`).
            Pending::Fly(i) => {
                if let Some(d) = game.fly_destination(i)
                    && game.has_map(d.map)
                {
                    self.s.transition.start(game, d.map, None, d.x, d.y);
                    self.s.objects.freeze_all();
                }
            }
            _ => {}
        }
        let _ = f;
    }
}
