//! Keyboard input for a focused play pane (plan: Task G "keys.rs", in the core so it is tested
//! without the `ui` feature). Pure: the UI feeds it
//! `KeyboardEvent::code()` names and turns its [`Outcome`]s into pane commands.
//!
//! The layout is the `play` bin's: arrows, X = A, Z = B, A = L, S = R, Enter = Start,
//! Backspace = Select. F5 saves, F8 rewinds, F9 reloads the parent node, Space pauses or
//! resumes, `.` steps one frame and Tab fast-forwards while held.

use crate::native::{Inspector, NativeCmd};
use crate::pane::RunMode;
use crate::rom::RomCmd;
use engine::Buttons;
use game_native::trace::Parent;

/// The speed of a held Tab.
pub const HOLD_FAST: u8 = 8;

/// The key codes that are not buttons but act on the pane.
const ACTION_KEYS: [&str; 7] = ["F5", "F8", "F9", "Space", "Period", "NumpadDecimal", "Tab"];

/// A pane command a key asks for (besides the buttons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Save,
    Rewind,
    /// Restart from the node the current trace starts from (or a new game).
    ReloadParent,
    Mode(RunMode),
    Step(u32),
}

/// What one key event does. `handled` keys get `prevent_default` (Tab must not move the focus,
/// F5 must not reload the webview, Space and Backspace must not scroll).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub handled: bool,
    /// The held buttons, when they changed.
    pub buttons: Option<Buttons>,
    pub action: Option<Action>,
}

/// The game button a key code holds.
pub fn button(code: &str) -> Option<Buttons> {
    Some(match code {
        "ArrowUp" => Buttons::UP,
        "ArrowDown" => Buttons::DOWN,
        "ArrowLeft" => Buttons::LEFT,
        "ArrowRight" => Buttons::RIGHT,
        "KeyX" => Buttons::A,
        "KeyZ" => Buttons::B,
        "KeyA" => Buttons::L,
        "KeyS" => Buttons::R,
        "Enter" | "NumpadEnter" => Buttons::START,
        "Backspace" => Buttons::SELECT,
        _ => return None,
    })
}

/// The modifier keys held with a key event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub meta: bool,
    pub ctrl: bool,
}

/// A focused pane's keyboard: the buttons held, the run mode a held Tab interrupted, and the
/// last mode it sent (the pane's status lags behind by up to one poll).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keyboard {
    held: Buttons,
    /// While Tab is held: the mode to go back to.
    fast_from: Option<RunMode>,
    /// The last mode sent through this keyboard, until the pane's status shows it.
    sent: Option<RunMode>,
}

impl Keyboard {
    pub fn held(&self) -> Buttons {
        self.held
    }

    /// The pane's mode: `run` (its last status), unless a mode sent since is not shown yet.
    fn current(&mut self, run: RunMode) -> RunMode {
        if self.sent == Some(run) {
            self.sent = None;
        }
        self.sent.unwrap_or(run)
    }

    /// A mode asked for by the UI (a button, or Space): while Tab is held it becomes the mode
    /// to go back to and nothing is sent (`None`); else it is sent.
    pub fn mode(&mut self, m: RunMode) -> Option<RunMode> {
        if self.fast_from.is_some() {
            self.fast_from = Some(m);
            return None;
        }
        self.sent = Some(m);
        Some(m)
    }

    /// A mode this keyboard sends.
    fn send(&mut self, m: RunMode) -> Option<Action> {
        self.sent = Some(m);
        Some(Action::Mode(m))
    }

    /// A key went down; `repeat` is the OS auto-repeat; `run` is the pane's mode in its last
    /// status. With Cmd or Ctrl held the key is the webview's (a shortcut): macOS sends no
    /// key-up for keys released while Cmd is held, so they are never pressed here.
    pub fn down(&mut self, code: &str, repeat: bool, run: RunMode, mods: Mods) -> Outcome {
        if mods.meta || mods.ctrl {
            return Outcome::default();
        }
        if let Some(b) = button(code) {
            let now = self.held | b;
            let changed = now != self.held;
            self.held = now;
            return Outcome {
                handled: true,
                buttons: changed.then_some(now),
                action: None,
            };
        }
        if !ACTION_KEYS.contains(&code) {
            return Outcome::default();
        }
        let action = match code {
            // A held `.` keeps stepping; the other keys act once per press.
            "Period" | "NumpadDecimal" => {
                // A step pauses the pane (both panes' `Step` paths do).
                self.sent = Some(RunMode::Pause);
                Some(Action::Step(1))
            }
            _ if repeat => None,
            "F5" => Some(Action::Save),
            "F8" => Some(Action::Rewind),
            "F9" => Some(Action::ReloadParent),
            "Space" => {
                // While Tab is held, the toggle applies to the mode Tab goes back to.
                let now = self.fast_from.unwrap_or_else(|| self.current(run));
                let m = match now {
                    RunMode::Pause => RunMode::Play,
                    _ => RunMode::Pause,
                };
                self.mode(m).map(Action::Mode)
            }
            "Tab" => match self.fast_from {
                Some(_) => None,
                None => {
                    self.fast_from = Some(self.current(run));
                    self.send(RunMode::Fast(HOLD_FAST))
                }
            },
            _ => None,
        };
        Outcome {
            handled: true,
            buttons: None,
            action,
        }
    }

    pub fn up(&mut self, code: &str) -> Outcome {
        if let Some(b) = button(code) {
            let now = Buttons(self.held.0 & !b.0);
            let changed = now != self.held;
            self.held = now;
            return Outcome {
                handled: true,
                buttons: changed.then_some(now),
                action: None,
            };
        }
        if matches!(code, "MetaLeft" | "MetaRight") {
            // Keys released while Cmd was held sent no key-up: release them all.
            return self.blur();
        }
        let action = match code {
            "Tab" => self.fast_from.take().and_then(|m| self.send(m)),
            _ => None,
        };
        Outcome {
            handled: ACTION_KEYS.contains(&code),
            buttons: None,
            action,
        }
    }

    /// The pane lost the focus (or the window did): every button is released and a held Tab
    /// ends.
    pub fn blur(&mut self) -> Outcome {
        let buttons = (self.held != Buttons::NONE).then_some(Buttons::NONE);
        self.held = Buttons::NONE;
        let action = self.fast_from.take().and_then(|m| self.send(m));
        Outcome {
            handled: false,
            buttons,
            action,
        }
    }
}

/// The native pane's commands for an outcome; `status` is its last inspector (for the parent).
pub fn native_cmds(o: Outcome, status: &Inspector) -> Vec<NativeCmd> {
    let mut out = Vec::new();
    if let Some(b) = o.buttons {
        out.push(NativeCmd::Keys(b));
    }
    out.extend(o.action.map(|a| match a {
        Action::Save => NativeCmd::Save,
        Action::Rewind => NativeCmd::Rewind,
        Action::ReloadParent => match status.parent {
            Parent::Checkpoint(id) => NativeCmd::Load(id),
            Parent::NewGame => NativeCmd::NewGame,
        },
        Action::Mode(m) => NativeCmd::Mode(m),
        Action::Step(n) => NativeCmd::Step(n),
    }));
    out
}

/// The ROM pane's commands for an outcome: no save or rewind there (decision 10); F9 re-boots
/// the last source.
pub fn rom_cmds(o: Outcome) -> Vec<RomCmd> {
    let mut out = Vec::new();
    if let Some(b) = o.buttons {
        out.push(RomCmd::Keys(b));
    }
    match o.action {
        None | Some(Action::Save | Action::Rewind) => {}
        Some(Action::ReloadParent) => out.push(RomCmd::Reload),
        Some(Action::Mode(m)) => out.push(RomCmd::Mode(m)),
        // As the Step button: pause, then run the pending frames (the native pane's `Step`
        // pauses by itself).
        Some(Action::Step(n)) => out.extend([RomCmd::Mode(RunMode::Pause), RomCmd::Step(n)]),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_native::trace::CheckpointId;

    const NO: Mods = Mods {
        meta: false,
        ctrl: false,
    };
    const CMD: Mods = Mods {
        meta: true,
        ctrl: false,
    };
    const CTRL: Mods = Mods {
        meta: false,
        ctrl: true,
    };

    #[test]
    fn cmd_or_ctrl_chords_are_the_webviews() {
        let mut k = Keyboard::default();
        for mods in [CMD, CTRL] {
            for code in ["KeyX", "KeyA", "Tab", "F5", "Space", "Period"] {
                assert_eq!(
                    k.down(code, false, RunMode::Play, mods),
                    Outcome::default(),
                    "{code} with {mods:?}"
                );
            }
        }
        assert_eq!(k.held(), Buttons::NONE, "nothing was pressed");
        assert_eq!(k.blur(), Outcome::default(), "and Tab was not held");
    }

    #[test]
    fn a_cmd_key_up_releases_everything() {
        // macOS: X down, Cmd down, X up (no key-up sent), Cmd up.
        let mut k = Keyboard::default();
        k.down("KeyX", false, RunMode::Play, NO);
        k.down("Tab", false, RunMode::Play, NO);
        assert_eq!(
            k.down("MetaLeft", false, RunMode::Play, NO),
            Outcome::default()
        );
        let o = k.up("MetaLeft");
        assert!(!o.handled);
        assert_eq!(o.buttons, Some(Buttons::NONE));
        assert_eq!(o.action, Some(Action::Mode(RunMode::Play)));
        assert_eq!(k.held(), Buttons::NONE);
        assert_eq!(
            k.up("MetaRight"),
            Outcome::default(),
            "nothing left to release"
        );
    }

    #[test]
    fn modes_while_tab_is_held_become_the_mode_to_restore() {
        let mut k = Keyboard::default();
        k.down("Tab", false, RunMode::Play, NO);
        // A button (or Space) while fast-forwarding: nothing is sent now...
        assert_eq!(k.mode(RunMode::Pause), None);
        assert_eq!(
            k.down("Space", false, RunMode::Fast(HOLD_FAST), NO).action,
            None
        );
        assert_eq!(k.mode(RunMode::Fast(2)), None);
        // ...and releasing Tab goes to the last one asked for.
        assert_eq!(k.up("Tab").action, Some(Action::Mode(RunMode::Fast(2))));
        assert_eq!(k.mode(RunMode::Pause), Some(RunMode::Pause));
    }

    #[test]
    fn the_last_sent_mode_wins_over_a_stale_status() {
        let mut k = Keyboard::default();
        // Two Space presses before the status catches up: pause, then play again.
        let a = k.down("Space", false, RunMode::Play, NO).action;
        let b = k.down("Space", false, RunMode::Play, NO).action;
        assert_eq!(
            (a, b),
            (
                Some(Action::Mode(RunMode::Pause)),
                Some(Action::Mode(RunMode::Play))
            )
        );
        // Tab held right after: it restores the mode just sent, not the stale status.
        k.down("Space", false, RunMode::Play, NO); // Pause sent, status still Play
        k.down("Tab", false, RunMode::Play, NO);
        assert_eq!(k.up("Tab").action, Some(Action::Mode(RunMode::Pause)));
        // Once the status shows the sent mode, the status is the reference again.
        assert_eq!(
            k.down("Space", false, RunMode::Pause, NO).action,
            Some(Action::Mode(RunMode::Play))
        );
        assert_eq!(
            k.down("Space", false, RunMode::Play, NO).action,
            Some(Action::Mode(RunMode::Pause))
        );
        k.mode(RunMode::Play);
        assert_eq!(
            k.down("Space", false, RunMode::Play, NO).action,
            Some(Action::Mode(RunMode::Pause)),
            "Play shown"
        );
        assert_eq!(
            k.down("Space", false, RunMode::Pause, NO).action,
            Some(Action::Mode(RunMode::Play)),
            "Pause shown, as sent"
        );
    }

    #[test]
    fn the_play_bin_layout() {
        let layout = [
            ("ArrowUp", Buttons::UP),
            ("ArrowDown", Buttons::DOWN),
            ("ArrowLeft", Buttons::LEFT),
            ("ArrowRight", Buttons::RIGHT),
            ("KeyX", Buttons::A),
            ("KeyZ", Buttons::B),
            ("KeyA", Buttons::L),
            ("KeyS", Buttons::R),
            ("Enter", Buttons::START),
            ("Backspace", Buttons::SELECT),
        ];
        for (code, b) in layout {
            assert_eq!(button(code), Some(b), "{code}");
        }
        assert_eq!(button("KeyQ"), None);
        assert_eq!(button("F5"), None);
    }

    #[test]
    fn buttons_are_held_until_released() {
        let mut k = Keyboard::default();
        let o = k.down("KeyX", false, RunMode::Play, NO);
        assert!(o.handled);
        assert_eq!(o.buttons, Some(Buttons::A));
        let o = k.down("ArrowUp", false, RunMode::Play, NO);
        assert_eq!(o.buttons, Some(Buttons::A | Buttons::UP));
        // An auto-repeat changes nothing but is still handled (no scrolling).
        let o = k.down("ArrowUp", true, RunMode::Play, NO);
        assert_eq!((o.handled, o.buttons), (true, None));
        assert_eq!(k.up("KeyX").buttons, Some(Buttons::UP));
        assert_eq!(k.up("KeyX").buttons, None);
        assert_eq!(k.held(), Buttons::UP);
        // Unknown keys are left to the webview.
        assert_eq!(k.down("KeyQ", false, RunMode::Play, NO), Outcome::default());
        assert_eq!(k.up("KeyQ"), Outcome::default());
    }

    #[test]
    fn function_keys_space_and_period() {
        let mut k = Keyboard::default();
        let act = |k: &mut Keyboard, code, run| k.down(code, false, run, NO).action;
        assert_eq!(act(&mut k, "F5", RunMode::Play), Some(Action::Save));
        assert_eq!(act(&mut k, "F8", RunMode::Play), Some(Action::Rewind));
        assert_eq!(act(&mut k, "F9", RunMode::Play), Some(Action::ReloadParent));
        assert_eq!(
            act(&mut k, "Space", RunMode::Play),
            Some(Action::Mode(RunMode::Pause))
        );
        assert_eq!(
            act(&mut k, "Space", RunMode::Pause),
            Some(Action::Mode(RunMode::Play))
        );
        assert_eq!(
            act(&mut k, "Space", RunMode::Fast(4)),
            Some(Action::Mode(RunMode::Pause))
        );
        assert_eq!(act(&mut k, "Period", RunMode::Pause), Some(Action::Step(1)));
        // A held F5 saves once; a held `.` keeps stepping.
        assert_eq!(k.down("F5", true, RunMode::Play, NO).action, None);
        assert!(k.down("F5", true, RunMode::Play, NO).handled);
        assert_eq!(
            k.down("Period", true, RunMode::Pause, NO).action,
            Some(Action::Step(1))
        );
        let up = k.up("F5");
        assert!(up.handled && up.action.is_none());
    }

    #[test]
    fn tab_fast_forwards_while_held_then_restores_the_mode() {
        let mut k = Keyboard::default();
        let o = k.down("Tab", false, RunMode::Pause, NO);
        assert!(o.handled);
        assert_eq!(o.action, Some(Action::Mode(RunMode::Fast(HOLD_FAST))));
        // A second down (repeat or not) while held keeps the first mode to restore.
        assert_eq!(
            k.down("Tab", false, RunMode::Fast(HOLD_FAST), NO).action,
            None
        );
        assert_eq!(
            k.down("Tab", true, RunMode::Fast(HOLD_FAST), NO).action,
            None
        );
        assert_eq!(k.up("Tab").action, Some(Action::Mode(RunMode::Pause)));
        assert_eq!(k.up("Tab").action, None);
    }

    #[test]
    fn blur_releases_everything() {
        let mut k = Keyboard::default();
        k.down("KeyZ", false, RunMode::Play, NO);
        k.down("Tab", false, RunMode::Play, NO);
        let o = k.blur();
        assert!(!o.handled);
        assert_eq!(o.buttons, Some(Buttons::NONE));
        assert_eq!(o.action, Some(Action::Mode(RunMode::Play)));
        assert_eq!(k.held(), Buttons::NONE);
        assert_eq!(k.blur(), Outcome::default());
    }

    #[test]
    fn outcomes_become_pane_commands() {
        let mut status = Inspector::default();
        let o = Outcome {
            handled: true,
            buttons: Some(Buttons::A),
            action: Some(Action::ReloadParent),
        };
        let native = native_cmds(o, &status);
        assert!(matches!(
            native[..],
            [NativeCmd::Keys(Buttons::A), NativeCmd::NewGame]
        ));
        status.parent = Parent::Checkpoint(CheckpointId(3));
        assert!(matches!(
            native_cmds(o, &status)[..],
            [NativeCmd::Keys(_), NativeCmd::Load(CheckpointId(3))]
        ));
        assert_eq!(rom_cmds(o), [RomCmd::Keys(Buttons::A), RomCmd::Reload]);
        let save = Outcome {
            handled: true,
            buttons: None,
            action: Some(Action::Save),
        };
        assert!(matches!(native_cmds(save, &status)[..], [NativeCmd::Save]));
        assert_eq!(rom_cmds(save), []);
        let step = Outcome {
            action: Some(Action::Step(1)),
            ..save
        };
        assert!(matches!(
            native_cmds(step, &status)[..],
            [NativeCmd::Step(1)]
        ));
        assert_eq!(
            rom_cmds(step),
            [RomCmd::Mode(RunMode::Pause), RomCmd::Step(1)]
        );
    }
}
