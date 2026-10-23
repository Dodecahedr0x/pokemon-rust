//! Screen hosting (Milestone 9 Task K owns the file): opening a screen from START or a
//! request, pushing and popping, handing the VRAM over and restoring the field, and answering
//! the waiting request with the screen's result.
//!
//! Wave 0: the calls are in place and the bodies are minimal: [`start_pressed`] opens nothing
//! (so START does nothing, as before), and [`World::step_screens`] steps the top screen and
//! closes it on `Done` without any fade or VRAM handover (no screen opens before Task K).

use super::mode::Mode;
use super::{Event, State, World, draw_field};
use crate::data::GameData;
use crate::ui::{ScreenCx, ScreenInput, ScreenKind, ScreenOut, ScreenResult, ScreenStack};
use crate::video::Frame;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// START pressed while the field is idle and the player is not moving (Gen3
/// `ProcessPlayerFieldInput`'s `pressedStartButton` → `ShowStartMenu`). Returns whether a
/// screen opened (the frame's field control stops there). Wave 0: never opens.
pub(super) fn start_pressed(s: &mut State, game: &impl GameData) -> bool {
    let _ = (s, game);
    false
}

/// Opens `kind` over the field (Task K: the fade and the VRAM handover).
#[allow(dead_code, reason = "Milestone 9 Wave 0: Task K opens screens")]
pub(super) fn open(s: &mut State, kind: ScreenKind) {
    let mut stack = match core::mem::replace(&mut s.mode, Mode::Field) {
        Mode::Screens(stack) => stack,
        other => {
            s.mode = other;
            Box::new(ScreenStack::new())
        }
    };
    if stack.push(kind) {
        s.event(Event::ScreenOpened(kind.tag()));
    }
    s.mode = if stack.is_empty() {
        Mode::Field
    } else {
        Mode::Screens(stack)
    };
}

/// Closes every screen (Task K: restores the field).
pub(super) fn close(s: &mut State) {
    if matches!(s.mode, Mode::Screens(_)) {
        s.mode = Mode::Field;
        s.event(Event::ScreenClosed);
    }
}

impl World {
    /// One frame while screens own the frame (`Mode::Screens`).
    pub(super) fn step_screens(
        &mut self,
        game: &impl GameData,
        mut stack: Box<ScreenStack>,
        f: &mut Frame,
    ) {
        let s = &mut self.s;
        let mut reports = Vec::new();
        let input = ScreenInput {
            pressed: s.input.pressed,
            held: s.input.held,
        };
        let out = match stack.top_mut() {
            None => ScreenOut::Done(ScreenResult::Cancel),
            Some(top) => {
                let mut cx = ScreenCx {
                    game,
                    f,
                    party: &mut s.party,
                    bag: &mut s.bag,
                    money: &mut s.money,
                    pc: &mut s.pc,
                    options: &mut s.options,
                    coins: &mut s.coins,
                    pokedex: &mut s.pokedex,
                    registered_item: &mut s.registered_item,
                    vars: &s.vars,
                    player_name: &s.player_name,
                    saved: None,
                    child: None,
                    reports: &mut reports,
                };
                top.step(&mut cx, input)
            }
        };
        for r in reports {
            super::battle::report(s, r);
        }
        match out {
            ScreenOut::Running => {}
            ScreenOut::Push(kind) => {
                if stack.push(kind) {
                    s.event(Event::ScreenOpened(kind.tag()));
                }
            }
            ScreenOut::Done(result) => {
                stack.pop();
                // Task K hands `result` to the parent screen (`ScreenCx::child`) or, for the
                // last screen, to the waiting request (`host/screens.rs`).
                let _ = result;
            }
            ScreenOut::RunScript(id) => {
                close(s);
                s.objects.freeze_all();
                self.start_script(game, id, None);
                return;
            }
            ScreenOut::Save => {
                // Task K hands the result to the screen next frame; Wave 0 has no screen
                // that saves.
                let _ = super::save::save_now(self, game);
            }
        }
        let s = &mut self.s;
        if stack.is_empty() {
            s.mode = Mode::Field;
            s.event(Event::ScreenClosed);
            draw_field(game, s, f);
        } else {
            s.mode = Mode::Screens(stack);
        }
    }
}
