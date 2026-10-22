//! Keyboard input for a focused play pane (plan: Task G "keys.rs"). Pure: the UI feeds it
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

/// A focused pane's keyboard: the buttons held, and the run mode a held Tab interrupted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keyboard {
    held: Buttons,
    /// While Tab is held: the mode to go back to.
    fast_from: Option<RunMode>,
}

impl Keyboard {
    pub fn held(&self) -> Buttons {
        self.held
    }

    /// A key went down; `repeat` is the OS auto-repeat; `run` is the pane's mode now.
    pub fn down(&mut self, code: &str, repeat: bool, run: RunMode) -> Outcome {
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
            "Period" | "NumpadDecimal" => Some(Action::Step(1)),
            _ if repeat => None,
            "F5" => Some(Action::Save),
            "F8" => Some(Action::Rewind),
            "F9" => Some(Action::ReloadParent),
            "Space" => Some(Action::Mode(match run {
                RunMode::Pause => RunMode::Play,
                _ => RunMode::Pause,
            })),
            "Tab" => match self.fast_from {
                Some(_) => None,
                None => {
                    self.fast_from = Some(run);
                    Some(Action::Mode(RunMode::Fast(HOLD_FAST)))
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
        Outcome {
            handled: ACTION_KEYS.contains(&code),
            buttons: None,
            action: match code {
                "Tab" => self.fast_from.take().map(Action::Mode),
                _ => None,
            },
        }
    }

    /// The pane lost the focus: every button is released and a held Tab ends.
    pub fn blur(&mut self) -> Outcome {
        let buttons = (self.held != Buttons::NONE).then_some(Buttons::NONE);
        self.held = Buttons::NONE;
        Outcome {
            handled: false,
            buttons,
            action: self.fast_from.take().map(Action::Mode),
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
    out.extend(o.action.and_then(|a| match a {
        Action::Save | Action::Rewind => None,
        Action::ReloadParent => Some(RomCmd::Reload),
        Action::Mode(m) => Some(RomCmd::Mode(m)),
        // A paused ROM pane runs pending steps; a running one would just run one more.
        Action::Step(n) => Some(RomCmd::Step(n)),
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_native::trace::CheckpointId;

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
        let o = k.down("KeyX", false, RunMode::Play);
        assert!(o.handled);
        assert_eq!(o.buttons, Some(Buttons::A));
        let o = k.down("ArrowUp", false, RunMode::Play);
        assert_eq!(o.buttons, Some(Buttons::A | Buttons::UP));
        // An auto-repeat changes nothing but is still handled (no scrolling).
        let o = k.down("ArrowUp", true, RunMode::Play);
        assert_eq!((o.handled, o.buttons), (true, None));
        assert_eq!(k.up("KeyX").buttons, Some(Buttons::UP));
        assert_eq!(k.up("KeyX").buttons, None);
        assert_eq!(k.held(), Buttons::UP);
        // Unknown keys are left to the webview.
        assert_eq!(k.down("KeyQ", false, RunMode::Play), Outcome::default());
        assert_eq!(k.up("KeyQ"), Outcome::default());
    }

    #[test]
    fn function_keys_space_and_period() {
        let mut k = Keyboard::default();
        let act = |k: &mut Keyboard, code, run| k.down(code, false, run).action;
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
        assert_eq!(k.down("F5", true, RunMode::Play).action, None);
        assert!(k.down("F5", true, RunMode::Play).handled);
        assert_eq!(
            k.down("Period", true, RunMode::Pause).action,
            Some(Action::Step(1))
        );
        let up = k.up("F5");
        assert!(up.handled && up.action.is_none());
    }

    #[test]
    fn tab_fast_forwards_while_held_then_restores_the_mode() {
        let mut k = Keyboard::default();
        let o = k.down("Tab", false, RunMode::Pause);
        assert!(o.handled);
        assert_eq!(o.action, Some(Action::Mode(RunMode::Fast(HOLD_FAST))));
        // A second down (repeat or not) while held keeps the first mode to restore.
        assert_eq!(k.down("Tab", false, RunMode::Fast(HOLD_FAST)).action, None);
        assert_eq!(k.down("Tab", true, RunMode::Fast(HOLD_FAST)).action, None);
        assert_eq!(k.up("Tab").action, Some(Action::Mode(RunMode::Pause)));
        assert_eq!(k.up("Tab").action, None);
    }

    #[test]
    fn blur_releases_everything() {
        let mut k = Keyboard::default();
        k.down("KeyZ", false, RunMode::Play);
        k.down("Tab", false, RunMode::Play);
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
        assert_eq!(rom_cmds(step), [RomCmd::Step(1)]);
    }
}
