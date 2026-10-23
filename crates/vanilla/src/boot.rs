//! Power-on to the player's first control in the moving truck: the intro and the title are
//! skipped with START, the main menu's NEW GAME and Birch's speech are advanced with A (the
//! gender menu's cursor starts on BOY), and the naming screen types [`PLAYER_NAME`]. Every
//! input is chosen from what RAM says the game is showing ([`Phase`]), never from frame
//! counts: a press is held one frame and released one frame, so each one is a new press.

use crate::state::{Mode, Vanilla, layout::*};
use anyhow::{Result, anyhow, ensure};
use engine::Buttons;

/// The player's name, as our engine's new game has it (`SACHA`).
pub const PLAYER_NAME: &str = "SACHA";

/// What the boot sees on a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The copyright screen, a fade or anything not listed: no input.
    Wait,
    Intro,
    Title,
    /// The main menu and Birch's speech (both under `CB2_MainMenu`).
    MainMenu,
    Naming,
    /// The overworld, not yet in the player's control (the truck's ride).
    Field,
    /// The player has control.
    Done,
}

/// The phases met and the frame (since power-on) each started on.
#[derive(Clone, Debug, Default)]
pub struct BootReport {
    pub phases: Vec<(Phase, u64)>,
}

/// Gen3's encoding of `A`..`Z` (`charmap.txt`: `A` = 0xBB).
pub fn gen3_char(c: char) -> u8 {
    assert!(c.is_ascii_uppercase(), "{c}: only A..Z");
    0xBB + (c as u8 - b'A')
}

/// The cell of an upper-case letter on the naming screen's `KEYBOARD_LETTERS_UPPER` page.
pub fn key_cell(c: char) -> (i16, i16) {
    const ROWS: [&str; 4] = ["ABCDEF .", "GHIJKL ,", "MNOPQRS ", "TUVWXYZ "];
    for (y, row) in ROWS.iter().enumerate() {
        if let Some(x) = row.find(c) {
            return (x as i16, y as i16);
        }
    }
    panic!("{c} is not on the upper-case page")
}

/// `KBPAGE_LETTERS_UPPER`, the page the screen opens on.
const PAGE_UPPER: u8 = 1;
/// `STATE_HANDLE_INPUT`.
const NS_HANDLE_INPUT: u8 = 2;
/// The OK button's cursor cell (`MoveCursorToOKButton`: column count, row 2).
const OK_BUTTON: (i16, i16) = (8, 2);

impl Vanilla {
    pub fn phase(&mut self) -> Phase {
        let s = self.sym.clone();
        let cb2 = self.callback2();
        if cb2 == s.cb2_overworld {
            return if self.mode() == Mode::FieldIdle {
                Phase::Done
            } else {
                Phase::Field
            };
        }
        let tasks = self.tasks();
        if tasks.contains(&s.task_naming_screen) {
            return Phase::Naming;
        }
        if cb2 == s.main_cb2_intro {
            return Phase::Intro;
        }
        if [
            s.task_title_phase1,
            s.task_title_phase2,
            s.task_title_phase3,
        ]
        .iter()
        .any(|t| tasks.contains(t))
        {
            return Phase::Title;
        }
        if cb2 == s.cb2_main_menu {
            return Phase::MainMenu;
        }
        Phase::Wait
    }

    /// The naming screen's typed text (Gen3 bytes up to EOS) and cursor cell, once it takes
    /// input on the upper-case page; `None` before.
    pub(crate) fn naming(&mut self) -> Option<(Vec<u8>, (i16, i16), u8)> {
        let ns = self.emu.read_u32(self.sym.naming_screen);
        if !EWRAM.contains(&ns) || self.emu.read_u8(ns + NS_STATE) != NS_HANDLE_INPUT {
            return None;
        }
        let text: Vec<u8> = (self.emu.read_bytes(ns + NS_TEXT_BUFFER, 16).into_iter())
            .take_while(|&b| b != 0xFF)
            .collect();
        let sprite =
            self.sym.sprites + SPRITE_SIZE * u32::from(self.emu.read_u8(ns + NS_CURSOR_SPRITE_ID));
        let x = self.emu.read_u16(sprite + SPRITE_DATA) as i16;
        let y = self.emu.read_u16(sprite + SPRITE_DATA + 2) as i16;
        Some((text, (x, y), self.emu.read_u8(ns + NS_CURRENT_PAGE)))
    }

    /// The button the naming screen needs next to spell `name`.
    fn naming_key(&mut self, name: &str) -> Buttons {
        let Some((text, (x, y), page)) = self.naming() else {
            return Buttons::NONE;
        };
        let target: Vec<u8> = name.chars().map(gen3_char).collect();
        if page != PAGE_UPPER {
            return Buttons::SELECT;
        }
        if !target.starts_with(&text) {
            return Buttons::B; // backspace (the screen opens with a random preset name)
        }
        let goal = match target.get(text.len()) {
            None => {
                if (x, y) != OK_BUTTON {
                    return Buttons::START; // to the OK button
                }
                OK_BUTTON
            }
            Some(_) => key_cell(name.chars().nth(text.len()).unwrap()),
        };
        match (x.cmp(&goal.0), y.cmp(&goal.1)) {
            (std::cmp::Ordering::Less, _) => Buttons::RIGHT,
            (std::cmp::Ordering::Greater, _) => Buttons::LEFT,
            (_, std::cmp::Ordering::Less) => Buttons::DOWN,
            (_, std::cmp::Ordering::Greater) => Buttons::UP,
            _ => Buttons::A,
        }
    }

    /// Runs from power-on until the player first has control (in the truck), within
    /// `max_frames`. Checks the result: a boy named [`PLAYER_NAME`].
    pub fn boot(&mut self, max_frames: u64) -> Result<BootReport> {
        let mut report = BootReport::default();
        let mut released = true;
        loop {
            let phase = self.phase();
            if report.phases.last().map(|p| p.0) != Some(phase) {
                report.phases.push((phase, self.frames));
            }
            if phase == Phase::Done {
                break;
            }
            if self.frames >= max_frames {
                return Err(anyhow!(
                    "boot: still in {phase:?} (mode {:?}) after {} frames; phases {:?}",
                    self.mode(),
                    self.frames,
                    report.phases
                ));
            }
            let want = match phase {
                Phase::Wait | Phase::Field | Phase::Done => Buttons::NONE,
                Phase::Intro | Phase::Title => Buttons::START,
                Phase::MainMenu => Buttons::A,
                Phase::Naming => self.naming_key(PLAYER_NAME),
            };
            // One frame pressed, one released: every press is new (`JOY_NEW`).
            let keys = if released { want } else { Buttons::NONE };
            released = keys == Buttons::NONE;
            self.step(keys)?;
        }
        let sb2 = self.save_block2()?;
        let name = self.emu.read_bytes(sb2 + SB2_NAME, PLAYER_NAME.len() + 1);
        let mut want: Vec<u8> = PLAYER_NAME.chars().map(gen3_char).collect();
        want.push(0xFF);
        ensure!(
            name == want,
            "boot: player name {name:02x?}, wanted {want:02x?}"
        );
        ensure!(
            self.emu.read_u8(sb2 + SB2_GENDER) == 0,
            "boot: the player is not a boy"
        );
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyboard_and_the_charmap() {
        assert_eq!(gen3_char('A'), 0xBB);
        assert_eq!(gen3_char('S'), 0xCD);
        assert_eq!(key_cell('A'), (0, 0));
        assert_eq!(key_cell('S'), (6, 2));
        assert_eq!(key_cell('H'), (1, 1));
        assert_eq!(key_cell('Z'), (6, 3));
    }
}
