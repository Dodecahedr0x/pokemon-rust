//! The player's options (Gen3 `gSaveBlock2Ptr->options*`, `option_menu.c`; Milestone 9 Wave 0).
//!
//! The defaults are Gen3's `SetDefaultOptions` (text MID, battle scene on, style SHIFT, sound
//! MONO, frame 1) and button mode NORMAL (a zeroed save block), which reproduce the engine's
//! behaviour before options existed: options at their defaults are route-neutral. Task SM's
//! screen edits them; the text speed reaches the text box through `TextBox::set_speed`.

use crate::hash::Fnv64;
use serde::{Deserialize, Serialize};

/// Gen3 `OPTIONS_TEXT_SPEED_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TextSpeed {
    Slow,
    #[default]
    Mid,
    Fast,
}

impl TextSpeed {
    /// Frames per drawn glyph (Gen3 `sTextSpeedFrameDelays`: 8, 4, 1). Gen3's printer draws a
    /// glyph and then waits that many frames, so a glyph takes one frame more there (and
    /// control codes cost a frame); the engine's printer has charged the delay itself per drawn
    /// glyph since Milestone 3 (`text.rs`), and keeps that model for the three speeds.
    pub const fn frames_per_char(self) -> u8 {
        match self {
            TextSpeed::Slow => 8,
            TextSpeed::Mid => 4,
            TextSpeed::Fast => 1,
        }
    }
}

/// Gen3 `OPTIONS_BATTLE_STYLE_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum BattleStyle {
    /// Asks whether to switch when the opponent sends out its next mon.
    #[default]
    Shift,
    Set,
}

/// Gen3 `OPTIONS_SOUND_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Sound {
    #[default]
    Mono,
    Stereo,
}

/// Gen3 `OPTIONS_BUTTON_MODE_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ButtonMode {
    #[default]
    Normal,
    /// L and R open the help (not modelled; stored only).
    Lr,
    /// L acts as A.
    LEqualsA,
}

/// The six Gen3 options.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Options {
    pub text_speed: TextSpeed,
    /// Battle animations on (Gen3 `!optionsBattleSceneOff`).
    pub battle_scene: bool,
    pub battle_style: BattleStyle,
    pub sound: Sound,
    pub button_mode: ButtonMode,
    /// Window frame 0..20 (Gen3 `optionsWindowFrameType`; the menu shows it plus one).
    pub frame: u8,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            text_speed: TextSpeed::Mid,
            battle_scene: true,
            battle_style: BattleStyle::Shift,
            sound: Sound::Mono,
            button_mode: ButtonMode::Normal,
            frame: 0,
        }
    }
}

impl Options {
    /// Feeds the options into `h`, only when they differ from the defaults (so a world that
    /// never changed them keeps its hashes).
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut Fnv64) {
        if *self != Options::default() {
            h.write(&[
                0xA2,
                self.text_speed as u8,
                self.battle_scene as u8,
                self.battle_style as u8,
                self.sound as u8,
                self.button_mode as u8,
                self.frame,
            ]);
        }
    }
}
