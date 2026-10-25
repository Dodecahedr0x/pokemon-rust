//! The World's sound hooks (Milestone 12): map music on map loads and crossings
//! (`overworld.c`), the battle music, and the frame's tick ([`crate::audio`]).

use super::State;
use crate::audio::{Audio, MUS_NONE};
use crate::data::{GameData, MapId, MapType};
use crate::video::Frame;

impl State {
    /// Gen3 `GetCurrLocationDefaultMusic` (without Route 111's sandstorm).
    pub(super) fn default_music(&self, game: &impl GameData) -> u16 {
        game.map_music(self.field.map(), self.objects.player().pos().0)
    }

    /// Gen3 `Overworld_PlaySpecialMapMusic`: after a warp's load, a battle, a screen.
    pub(super) fn play_special_map_music(&mut self, game: &impl GameData) {
        let music = self.special_map_music(game);
        if music != self.audio.current {
            self.audio.play_new_map_music(music);
        }
    }

    /// The music [`State::play_special_map_music`] picks.
    pub(super) fn special_map_music(&self, game: &impl GameData) -> u16 {
        self.special_music_with(game, self.audio.saved)
    }

    fn special_music_with(&self, game: &impl GameData, saved: u16) -> u16 {
        let ids = &game.audio().ids;
        let mut music = self.default_music(game);
        if music != ids.abnormal_weather && music != MUS_NONE {
            if saved != 0 {
                music = saved;
            } else if game.map_type(self.field.map()) == MapType::Underwater {
                music = ids.underwater;
            } else if self.objects.surfing() {
                music = ids.surf;
            }
        }
        music
    }

    /// Gen3 `TransitionMapMusic`: crossing into a connected map.
    pub(super) fn transition_map_music(&mut self, game: &impl GameData, f: &mut Frame) {
        let ids = &game.audio().ids;
        let mut new = self.default_music(game);
        let current = self.audio.current;
        if new != ids.abnormal_weather && new != MUS_NONE {
            if current == ids.underwater || current == ids.surf {
                return;
            }
            if self.objects.surfing() {
                new = ids.surf;
            }
        }
        if new != current {
            self.audio.fade_out_and_play(&mut f.sound, new, 8);
        }
    }

    /// Gen3 `TryFadeOutOldMapMusic`: a warp to `dest` starts (no hold for the fade: the load
    /// does not wait for the music to stop).
    pub(super) fn warp_fade_out_music(&mut self, game: &impl GameData, dest: MapId, f: &mut Frame) {
        if game.map_music(dest, -1) != self.audio.current {
            let speed = if game.map_indoors(dest) { 2 } else { 4 };
            self.audio.fade_out_map_music(&mut f.sound, speed);
        }
    }

    /// The audio state a restore of a field on this map rebuilds when the snapshot has none:
    /// the map's music playing, no `savedMusic`.
    pub(super) fn settled_audio_unsaved(&self, game: &impl GameData) -> Audio {
        let music = self.special_music_with(game, 0);
        Audio::settled(game.audio(), music, 0)
    }
}
