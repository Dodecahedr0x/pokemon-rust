//! Music and sound effects as the World sees them (Milestone 12): Gen3's `sound.c` and the
//! state of the m4a music players that game logic reads (`IsSEPlaying`, `IsBGMStopped`,
//! `IsFanfareTaskInactive`, `IsCryFinished`), with each `m4a*` call emitted as a
//! [`sound::Cmd`] in [`crate::video::Frame::sound`] for the shells' sound engine.
//!
//! The World does not run the sequencer. It keeps, per music player, the song, its priority
//! and how many `SoundMain` calls are left until its tracks end, from the content build's dry
//! runs of the real engine ([`AudioData::songs`], [`AudioData::cry_frames`]). That is all the
//! waits need, and it is small enough to hash and checkpoint. Timers are in `SoundMain` calls
//! (one per frame, at VBlank, after the frame's script and tasks): [`Audio::tick`] runs at the
//! end of each step in Gen3's order (tasks, `MapMusicMain`, then `SoundMain`).
//!
//! Deviations (`docs/plans/2026-10-24-milestone-12-audio.md`): a warp does not hold until the
//! old music has faded out; Route 118's and Route 111's sandstorm music special cases are left
//! out; trainers' encounter music (when they spot the player) is not played.

use crate::hash::Fnv64;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
pub use sound::{Cmd, Cry, Player};

/// Gen3 `MUS_NONE`: no music change (map headers, `GetCurrLocationDefaultMusic`).
pub const MUS_NONE: u16 = 0xFFFF;
/// Gen3 `MUS_DUMMY`: the song with no tracks (`savedMusic` cleared).
pub const MUS_DUMMY: u16 = 0;
/// A song that never ends by itself (it loops): [`SongInfo::frames`].
pub const FOREVER: u16 = u16::MAX;
/// Gen3 `CRY_VOLUME`, `CRY_PRIORITY_NORMAL`.
pub const CRY_VOLUME: u8 = 120;
pub const CRY_PRIORITY_NORMAL: u8 = 10;
/// Cry modes (`CRY_MODE_*`) [`AudioData::cry_frames`] has.
pub const CRY_MODES: usize = 13;

/// One `gSongTable` entry, as the World needs it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SongInfo {
    /// The m4a player (`ms`: 0 BGM, 1-3 SE1-SE3).
    pub player: u8,
    /// `SongHeader.priority`.
    pub priority: u8,
    /// `SoundMain` calls until the song's tracks all end when it starts alone ([`FOREVER`]
    /// when it loops; 0 for a song without tracks).
    pub frames: u16,
}

/// Song numbers the engine plays on its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AudioIds {
    /// `MUS_VS_WILD`, `MUS_VICTORY_WILD`.
    pub vs_wild: u16,
    pub victory_wild: u16,
    /// `SE_LOW_HEALTH`.
    pub low_health: u16,
    /// `MUS_SURF`, `MUS_UNDERWATER`, `MUS_ABNORMAL_WEATHER`.
    pub surf: u16,
    pub underwater: u16,
    pub abnormal_weather: u16,
}

/// A game's sound data (`GameData::audio`). The default ([`EMPTY_AUDIO`]) has no songs:
/// every song then has no tracks and nothing waits.
pub struct AudioData {
    /// The sound blob the shells play (`sound::blob`); empty without sound.
    pub blob: &'static [u8],
    pub songs: &'static [SongInfo],
    /// `sFanfares`: (song, duration in frames).
    pub fanfares: &'static [(u16, u16)],
    /// The cry id of each species number (`SpeciesToCryId(species - 1)`).
    pub species_cry: &'static [u16],
    /// `SoundMain` calls until each cry ends (`IsPokemonCryPlaying`), by cry id and mode.
    pub cry_frames: &'static [[u16; CRY_MODES]],
    pub ids: AudioIds,
}

pub const EMPTY_AUDIO: AudioData = AudioData {
    blob: &[],
    songs: &[],
    fanfares: &[],
    species_cry: &[],
    cry_frames: &[],
    ids: AudioIds {
        vs_wild: MUS_DUMMY,
        victory_wild: MUS_DUMMY,
        low_health: MUS_DUMMY,
        surf: MUS_DUMMY,
        underwater: MUS_DUMMY,
        abnormal_weather: MUS_NONE,
    },
};

impl AudioData {
    pub fn song(&self, n: u16) -> SongInfo {
        self.songs.get(n as usize).copied().unwrap_or(SongInfo {
            player: 0,
            priority: 0,
            frames: 0,
        })
    }
}

/// One m4a music player (`struct MusicPlayerInfo`), as far as the game reads it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PlayerState {
    pub song: u16,
    /// `SoundMain` calls until its tracks end (0: none left, `MUSICPLAYER_STATUS_TRACK`
    /// clear; [`FOREVER`]: it loops).
    pub left: u16,
    /// `MUSICPLAYER_STATUS_PAUSE`.
    pub paused: bool,
    pub priority: u8,
}

impl PlayerState {
    fn active(&self) -> bool {
        self.left > 0 && !self.paused
    }
}

/// Gen3 `sMapMusicState` (`MapMusicMain`).
pub mod map_state {
    pub const IDLE: u8 = 0;
    pub const START: u8 = 1;
    pub const PLAYING: u8 = 2;
    pub const FADING_OUT: u8 = 5;
    pub const FADE_THEN_PLAY: u8 = 6;
    pub const FADE_THEN_FADE_IN: u8 = 7;
}

/// The sound state the game reads. Hashed whole; checkpointed when it differs from what a
/// restore rebuilds ([`Audio::settled`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Audio {
    /// BGM, SE1, SE2, SE3.
    pub players: [PlayerState; 4],
    /// `SoundMain` calls left of the BGM's fade-out (0: none), and whether it pauses
    /// (`m4aMPlayFadeOutTemporarily`) rather than stops.
    pub fade: u16,
    pub fade_temporary: bool,
    /// `sCurrentMapMusic`, `sNextMapMusic`, `sMapMusicState`, `sMapMusicFadeInSpeed`.
    pub current: u16,
    pub next: u16,
    pub state: u8,
    pub fade_in: u8,
    /// Gen3 `gSaveBlock1Ptr->savedMusic`.
    pub saved: u16,
    /// `Task_Fanfare`'s `sFanfareCounter` while the task runs.
    pub fanfare: Option<u16>,
    /// `Task_DuckBGMForPokemonCry`'s `gPokemonCryBGMDuckingCounter` while the task runs.
    pub duck: Option<u8>,
    /// `SoundMain` calls until the last cry ends.
    pub cry: u16,
    /// The low-HP beep plays (`lowHpSong`).
    pub low_hp: bool,
}

impl Audio {
    /// The state a restore rebuilds: `music` playing as the map's (and `saved`).
    pub fn settled(data: &AudioData, music: u16, saved: u16) -> Audio {
        let mut a = Audio {
            saved,
            ..Audio::default()
        };
        if music != MUS_NONE {
            let info = data.song(music);
            if info.player == 0 {
                a.players[0] = PlayerState {
                    song: music,
                    left: info.frames,
                    paused: info.frames == 0,
                    priority: info.priority,
                };
            }
            a.current = music;
            a.state = map_state::PLAYING;
        }
        a
    }

    #[allow(clippy::should_implement_trait)] // the engine-wide `hash(&self, &mut Fnv64)` shape
    pub fn hash(&self, h: &mut Fnv64) {
        for p in &self.players {
            h.write_u16(p.song);
            h.write_u16(p.left);
            h.write(&[p.paused as u8, p.priority]);
        }
        h.write_u16(self.fade);
        h.write_u16(self.current);
        h.write_u16(self.next);
        h.write_u16(self.saved);
        h.write_u16(self.fanfare.unwrap_or(0xFFFF));
        h.write_u16(self.cry);
        h.write(&[
            self.fade_temporary as u8,
            self.state,
            self.fade_in,
            self.duck.unwrap_or(0xFF),
            self.low_hp as u8,
        ]);
    }

    // ---- m4a ----

    /// `m4aSongNumStart`: the song on its own player, unless SE1 or SE2 plays a song of higher
    /// priority (`MPlayStart`'s `unk_B` players).
    pub fn start(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        out.push(Cmd::Start(song));
        let info = data.song(song);
        let p = &mut self.players[(info.player & 3) as usize];
        let guarded = matches!(info.player, 1 | 2);
        if guarded && p.active() && p.priority > info.priority {
            return;
        }
        *p = PlayerState {
            song,
            left: info.frames,
            paused: info.frames == 0,
            priority: info.priority,
        };
        if info.player == 0 {
            self.fade = 0;
        }
    }

    /// `m4aSongNumStop`: stops the song's player if it plays `song`.
    fn stop_song(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        out.push(Cmd::StopSong(song));
        let p = &mut self.players[(data.song(song).player & 3) as usize];
        if p.song == song {
            p.paused = true;
        }
    }

    /// `m4aMPlayAllStop`.
    fn stop_all(&mut self, out: &mut Vec<Cmd>) {
        out.push(Cmd::StopAll);
        for p in &mut self.players {
            p.paused = true;
        }
        self.cry = 0;
    }

    // ---- sound.c ----

    /// `PlayBGM`.
    pub fn play_bgm(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        let song = if song == MUS_NONE { MUS_DUMMY } else { song };
        self.start(data, out, song);
    }

    /// `PlaySE`.
    pub fn play_se(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        self.start(data, out, song);
    }

    /// `IsSEPlaying`: SE1 or SE2 has tracks, unless both are paused.
    pub fn is_se_playing(&self) -> bool {
        let [_, a, b, _] = &self.players;
        !((a.paused && b.paused) || (a.left == 0 && b.left == 0))
    }

    /// `PlayFanfare`: the BGM stops (`m4aMPlayStop`) for the fanfare (`sFanfares`; a song not
    /// in it plays the first), and `Task_Fanfare` counts its duration.
    pub fn play_fanfare(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        let Some(&(song, frames)) = (data.fanfares.iter())
            .find(|f| f.0 == song)
            .or(data.fanfares.first())
        else {
            return;
        };
        out.push(Cmd::Stop(Player::Bgm));
        self.players[0].paused = true;
        self.start(data, out, song);
        self.fanfare = Some(frames);
    }

    /// `IsFanfareTaskInactive`.
    pub fn fanfare_done(&self) -> bool {
        self.fanfare.is_none()
    }

    /// `FadeOutBGMTemporarily` (`fadeoutbgm`): `speed` is m4a's (4 per script unit).
    pub fn fade_out_bgm(&mut self, out: &mut Vec<Cmd>, speed: u16, temporary: bool) {
        out.push(if temporary {
            Cmd::FadeOutTemporarily(Player::Bgm, speed)
        } else {
            Cmd::FadeOut(Player::Bgm, speed)
        });
        // `FadeOutBody` steps the volume down 16 times, every `speed` frames.
        self.fade = 16 * speed.max(1);
        self.fade_temporary = temporary;
    }

    /// `IsBGMPausedOrStopped`.
    pub fn bgm_paused_or_stopped(&self) -> bool {
        self.players[0].paused || self.players[0].left == 0
    }

    /// `FadeInBGM`: un-pauses the BGM, which fades in.
    pub fn fade_in_bgm(&mut self, out: &mut Vec<Cmd>, speed: u16) {
        out.push(Cmd::FadeIn(Player::Bgm, speed));
        self.players[0].paused = false;
        self.fade = 0;
    }

    /// `FadeInNewBGM`.
    fn fade_in_new_bgm(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16, speed: u16) {
        let song = if song == MUS_NONE { MUS_DUMMY } else { song };
        self.start(data, out, song);
        out.push(Cmd::ImmInit(Player::Bgm));
        out.push(Cmd::Volume(Player::Bgm, 0));
        self.stop_song(data, out, song);
        self.fade_in_bgm(out, speed);
    }

    /// `PlayCry_Script` (and `PlayCry_ByMode` outside doubles): the BGM ducks to 85 while
    /// the cry plays.
    pub fn play_cry(&mut self, data: &AudioData, out: &mut Vec<Cmd>, species: u16, mode: u8) {
        out.push(Cmd::Volume(Player::Bgm, 85));
        let id = species
            .checked_sub(1)
            .and_then(|_| data.species_cry.get(species as usize).copied());
        if let Some(id) = id {
            out.push(Cmd::Cry(Cry::for_mode(
                id,
                mode,
                0,
                CRY_VOLUME,
                CRY_PRIORITY_NORMAL,
            )));
            self.cry = (data.cry_frames.get(id as usize))
                .map_or(0, |c| c[(mode as usize).min(CRY_MODES - 1)]);
        }
        self.duck = Some(2);
    }

    /// `IsCryFinished`: the ducking task is over.
    pub fn cry_done(&self) -> bool {
        self.duck.is_none()
    }

    // ---- map music (sound.c, overworld.c) ----

    /// `PlayNewMapMusic`.
    pub fn play_new_map_music(&mut self, song: u16) {
        self.current = song;
        self.next = 0;
        self.state = map_state::START;
    }

    /// `FadeOutMapMusic`.
    pub fn fade_out_map_music(&mut self, out: &mut Vec<Cmd>, speed: u16) {
        if self.not_waiting_for_bgm_stop() {
            self.fade_out_bgm(out, speed, false);
        }
        self.current = 0;
        self.next = 0;
        self.state = map_state::FADING_OUT;
    }

    /// `FadeOutAndPlayNewMapMusic`.
    pub fn fade_out_and_play(&mut self, out: &mut Vec<Cmd>, song: u16, speed: u16) {
        self.fade_out_map_music(out, speed);
        self.next = song;
        self.state = map_state::FADE_THEN_PLAY;
    }

    /// `FadeOutAndFadeInNewMapMusic`.
    pub fn fade_out_and_fade_in(
        &mut self,
        out: &mut Vec<Cmd>,
        song: u16,
        out_speed: u16,
        in_speed: u8,
    ) {
        self.fade_out_map_music(out, out_speed);
        self.next = song;
        self.state = map_state::FADE_THEN_FADE_IN;
        self.fade_in = in_speed;
    }

    /// `IsNotWaitingForBGMStop`.
    fn not_waiting_for_bgm_stop(&self) -> bool {
        !matches!(
            self.state,
            map_state::FADING_OUT | map_state::FADE_THEN_PLAY | map_state::FADE_THEN_FADE_IN
        )
    }

    /// `ResetMapMusic`.
    pub fn reset_map_music(&mut self) {
        self.current = 0;
        self.next = 0;
        self.state = map_state::IDLE;
        self.fade_in = 0;
    }

    /// `Overworld_ChangeMusicTo` (`fadenewbgm`).
    pub fn change_music_to(&mut self, out: &mut Vec<Cmd>, data: &AudioData, song: u16) {
        if self.current != song && self.current != data.ids.abnormal_weather {
            self.fade_out_and_play(out, song, 8);
        }
    }

    /// `PlayBattleBGM`: map music reset, every player stopped, `song` on the BGM player.
    pub fn play_battle_bgm(&mut self, data: &AudioData, out: &mut Vec<Cmd>, song: u16) {
        self.reset_map_music();
        self.stop_all(out);
        self.fanfare = None;
        self.low_hp = false;
        self.play_bgm(data, out, song);
    }

    /// The low-HP beep on or off (`SE_LOW_HEALTH`, `BattleStopLowHpSound`).
    pub fn set_low_hp(&mut self, data: &AudioData, out: &mut Vec<Cmd>, on: bool) {
        if on == self.low_hp {
            return;
        }
        self.low_hp = on;
        if on {
            self.play_se(data, out, data.ids.low_health);
        } else {
            self.stop_song(data, out, data.ids.low_health);
        }
    }

    // ---- one frame ----

    /// The end of a frame: the tasks (`Task_Fanfare`, `Task_DuckBGMForPokemonCry`), then
    /// `MapMusicMain`, then the VBlank's `SoundMain`.
    pub fn tick(&mut self, data: &AudioData, out: &mut Vec<Cmd>) {
        match self.fanfare {
            Some(0) => {
                out.push(Cmd::Continue(Player::Bgm));
                self.players[0].paused = false;
                self.fanfare = None;
            }
            Some(n) => self.fanfare = Some(n - 1),
            None => {}
        }
        match self.duck {
            Some(0) if self.cry == 0 => {
                out.push(Cmd::Volume(Player::Bgm, 256));
                self.duck = None;
            }
            Some(0) | None => {}
            Some(n) => self.duck = Some(n - 1),
        }
        self.map_music_main(data, out);
        // `SoundMain`.
        let bgm = &mut self.players[0];
        if self.fade > 0 && bgm.active() {
            self.fade -= 1;
            if self.fade == 0 {
                bgm.paused = true;
                if !self.fade_temporary {
                    bgm.left = 0;
                }
            }
        }
        for p in &mut self.players {
            if p.active() && p.left != FOREVER {
                p.left -= 1;
                if p.left == 0 {
                    p.paused = true;
                }
            }
        }
        self.cry = self.cry.saturating_sub(1);
        // An SE player without tracks is idle whatever it played last (its priority only
        // guards a playing song): normalised, so that it hashes and snapshots as idle.
        for p in &mut self.players[1..] {
            if p.left == 0 {
                *p = PlayerState::default();
            }
        }
    }

    /// `MapMusicMain`.
    fn map_music_main(&mut self, data: &AudioData, out: &mut Vec<Cmd>) {
        let stopped = self.players[0].left == 0;
        match self.state {
            map_state::START => {
                self.state = map_state::PLAYING;
                self.play_bgm(data, out, self.current);
            }
            map_state::FADING_OUT if stopped => {
                self.next = 0;
                self.state = map_state::IDLE;
            }
            map_state::FADE_THEN_PLAY if stopped && self.fanfare.is_none() => {
                self.current = self.next;
                self.next = 0;
                self.state = map_state::PLAYING;
                self.play_bgm(data, out, self.current);
            }
            map_state::FADE_THEN_FADE_IN if stopped && self.fanfare.is_none() => {
                let speed = self.fade_in as u16;
                self.fade_in_new_bgm(data, out, self.next, speed);
                self.current = self.next;
                self.next = 0;
                self.state = map_state::PLAYING;
                self.fade_in = 0;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
