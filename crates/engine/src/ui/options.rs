//! The options menu (Gen3 `option_menu.c`).
//!
//! One framed window of seven rows: text speed, battle scene, battle style, sound, button
//! mode, frame, and CANCEL. Up and down move (wrapping). Left and right change the row's
//! value and write it into [`crate::options::Options`] immediately, the way leaving the menu
//! stores it (B, START, and A on CANCEL all leave). The frame row is Gen3's window type
//! (shown as 1..=20); the kit reads it the next time a window opens.
//!
//! What each value does in play:
//! - text speed is the printer's frames per glyph (`TextBox::set_speed`);
//! - battle scene off skips the move lunge, the hit blink, and the stat tint;
//! - battle style SHIFT asks, in a singles battle, before the opponent sends its next
//!   Pokémon, when the player has another one that can fight; SET does not ask;
//! - sound is stored and applied as the mixer's mode (playback is the `audio` gap);
//! - button mode L=A makes L press A in the field and in menus. LR is Gen3's help shortcut
//!   and is stored only.
//!
//! Deviations: no separate header window (there is no title string in `UiText`); YES on the
//! shift prompt sends the next usable Pokémon rather than opening the party menu (the same
//! simplification as the forced switch); doubles do not ask.

use super::kit::frame_budget;
use super::kit::widgets::{CHAR_CURSOR, CURSOR_W, EOS, ROW_H, push, push_number};
use super::kit::window::{MapBuf, Panel, TileAlloc, frame_kind};
use super::{Screen, ScreenCx, ScreenInput, ScreenOut, ScreenResult};
use crate::data::{GameData, UiText};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::options::{BattleStyle, ButtonMode, Options, Sound, TextSpeed};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// The list window (Gen3's option list is 26×14 at (2, 5); this one sits a row higher so the
/// seven rows fit under a single frame with no header).
pub const WINDOW: (u8, u8, u8, u8) = (2, 3, 26, 14);
/// Rows, including CANCEL.
pub const ROWS: u8 = 7;
/// The CANCEL row.
pub const CANCEL: u8 = 6;
/// Gen3 window frames (`optionsWindowFrameType` 0..20, shown plus one).
pub const FRAME_COUNT: u8 = 20;
/// Where a row's value is drawn.
const VALUE_X: i16 = 136;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OptionsMenu {
    cursor: u8,
    gfx: super::kit::Gfx<Panel>,
}

impl Default for OptionsMenu {
    fn default() -> Self {
        OptionsMenu {
            cursor: 0,
            gfx: super::kit::Gfx::none(),
        }
    }
}

impl OptionsMenu {
    pub fn new() -> Self {
        OptionsMenu::default()
    }

    /// The highlighted row, once the menu has been drawn.
    pub fn cursor(&self) -> u8 {
        self.cursor
    }
}

fn cycle(n: u8, len: u8, right: bool) -> u8 {
    if right {
        (n + 1) % len
    } else {
        (n + len - 1) % len
    }
}

fn nudge(opt: &mut Options, row: u8, right: bool) {
    match row {
        0 => {
            opt.text_speed = match (opt.text_speed, right) {
                (TextSpeed::Slow, true) | (TextSpeed::Fast, false) => TextSpeed::Mid,
                (TextSpeed::Mid, true) | (TextSpeed::Slow, false) => TextSpeed::Fast,
                (TextSpeed::Fast, true) | (TextSpeed::Mid, false) => TextSpeed::Slow,
            };
        }
        1 => opt.battle_scene = !opt.battle_scene,
        2 => {
            opt.battle_style = match opt.battle_style {
                BattleStyle::Shift => BattleStyle::Set,
                BattleStyle::Set => BattleStyle::Shift,
            };
        }
        3 => {
            opt.sound = match opt.sound {
                Sound::Mono => Sound::Stereo,
                Sound::Stereo => Sound::Mono,
            };
        }
        4 => {
            opt.button_mode = match (opt.button_mode, right) {
                (ButtonMode::Normal, true) | (ButtonMode::LEqualsA, false) => ButtonMode::Lr,
                (ButtonMode::Lr, true) | (ButtonMode::Normal, false) => ButtonMode::LEqualsA,
                (ButtonMode::LEqualsA, true) | (ButtonMode::Lr, false) => ButtonMode::Normal,
            };
        }
        5 => opt.frame = cycle(opt.frame, FRAME_COUNT, right),
        _ => {}
    }
}

fn label(row: u8) -> UiText {
    match row {
        0 => UiText::OptionTextSpeed,
        1 => UiText::OptionBattleScene,
        2 => UiText::OptionBattleStyle,
        3 => UiText::OptionSound,
        4 => UiText::OptionButtonMode,
        5 => UiText::OptionFrame,
        _ => UiText::OptionCancel,
    }
}

fn value(game: &impl GameData, opt: &Options, row: u8) -> Vec<u8> {
    let id = match row {
        0 => match opt.text_speed {
            TextSpeed::Slow => UiText::OptionSlow,
            TextSpeed::Mid => UiText::OptionMid,
            TextSpeed::Fast => UiText::OptionFast,
        },
        1 => {
            if opt.battle_scene {
                UiText::OptionOn
            } else {
                UiText::OptionOff
            }
        }
        2 => match opt.battle_style {
            BattleStyle::Shift => UiText::OptionShift,
            BattleStyle::Set => UiText::OptionSet,
        },
        3 => match opt.sound {
            Sound::Mono => UiText::OptionMono,
            Sound::Stereo => UiText::OptionStereo,
        },
        4 => match opt.button_mode {
            ButtonMode::Normal => UiText::OptionNormal,
            ButtonMode::Lr => UiText::OptionLr,
            ButtonMode::LEqualsA => UiText::OptionLEqualsA,
        },
        5 => {
            let mut v = Vec::with_capacity(4);
            push_number(&mut v, u32::from(opt.frame) + 1, 2, false);
            v.push(EOS);
            return v;
        }
        _ => return alloc::vec![EOS],
    };
    let mut v = Vec::new();
    push(&mut v, game.ui_text(id));
    v.push(EOS);
    v
}

fn paint(game: &impl GameData, p: &mut Panel, opt: &Options, cursor: u8) {
    p.clear(game);
    for row in 0..ROWS {
        let y = i16::from(row) * ROW_H + 1;
        if row == cursor {
            p.text(game, &[CHAR_CURSOR, EOS], 0, y);
        }
        let mut name = Vec::new();
        push(&mut name, game.ui_text(label(row)));
        name.push(EOS);
        p.text(game, &name, CURSOR_W, y);
        if row != CANCEL {
            p.text(game, &value(game, opt, row), VALUE_X, y);
        }
    }
}

impl Screen for OptionsMenu {
    fn step<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, input: ScreenInput) -> ScreenOut {
        let first = self.gfx.0.is_none();
        if first {
            self.draw(cx, true);
        }
        let pressed = input.pressed;
        let leave = pressed.contains(Buttons::B)
            || pressed.contains(Buttons::START)
            || (pressed.contains(Buttons::A) && self.cursor == CANCEL);
        if leave {
            return ScreenOut::Done(ScreenResult::Cancel);
        }
        let mut moved = false;
        if pressed.contains(Buttons::UP) {
            self.cursor = cycle(self.cursor, ROWS, false);
            moved = true;
        } else if pressed.contains(Buttons::DOWN) {
            self.cursor = cycle(self.cursor, ROWS, true);
            moved = true;
        } else if self.cursor != CANCEL
            && (pressed.contains(Buttons::LEFT) || pressed.contains(Buttons::RIGHT))
        {
            let before = *cx.options;
            nudge(cx.options, self.cursor, pressed.contains(Buttons::RIGHT));
            moved = *cx.options != before;
        }
        if moved {
            let frame = self.cursor == 5;
            self.draw(cx, first || frame);
        } else if let Some(p) = self.gfx.0.as_deref_mut() {
            p.flush_within(cx.f, &mut frame_budget(cx.f));
        }
        ScreenOut::Running
    }

    fn hash(&self, h: &mut Fnv64) {
        h.write(&[self.cursor]);
    }
}

impl OptionsMenu {
    fn draw<G: GameData>(&mut self, cx: &mut ScreenCx<'_, G>, upload_map: bool) {
        let game = cx.game;
        let mut tiles = TileAlloc::full();
        let (l, t, w, h) = WINDOW;
        let mut p = tiles.panel(game, l, t, w, h, true);
        paint(game, &mut p, cx.options, self.cursor);
        p.flush_within(cx.f, &mut frame_budget(cx.f));
        if upload_map {
            let mut map = MapBuf::full(frame_kind(game, cx.options.frame));
            map.put(&p);
            map.flush(cx.f);
        }
        self.gfx.0 = Some(Box::new(p));
    }
}
