//! The m4a engine: players, tracks, the sequencer and the channels' envelopes. Field names and
//! the order of every step follow `src/m4a.c` / `src/m4a_1.s`; the comments name the Gen3
//! routine each function ports.
//!
//! Addresses: a `u32` below [`CRY_RAM`] is an offset into the [`Blob`]; from `CRY_RAM` on it is
//! a byte of a cry player's song (`gPokemonCrySongs[i]`, built in RAM by `SetPokemonCryTone`).
//! Pointers to tracks and channels are indices; [`NONE`] is null. Gen3 compares track pointers
//! when two notes of the same priority compete for a channel: here the track's index stands for
//! its address (BGM's tracks, then SE1's, SE2's, SE3's and the cry players', `.bss` order).

use crate::blob::{Blob, TONE};
use crate::cmd::{Cmd, Cry, Player};
use crate::psg::{Psg, reg};
use crate::tables::{CLOCK, FREQ, SCALE};

/// Null track or channel.
pub const NONE: u8 = 0xFF;
/// `gMPlayTable`'s four players, then the two cry players.
pub const PLAYERS: usize = 6;
/// DirectSound channels (`m4aSoundMode(... | 5 << SOUND_MODE_MAXCHN_SHIFT)`).
pub const CHANS: usize = 5;
/// Channel ids: DirectSound `0..CHANS`, then the PSG channels 1..=4.
const CGB0: usize = CHANS;
const ALL_CHANS: usize = CHANS + 4;
/// Tracks per player: `NUM_TRACKS_BGM` 10, SE1 3, SE2 9, SE3 1, and 2 per cry player.
const TRACK_COUNTS: [u8; PLAYERS] = [10, 3, 9, 1, 2, 2];
const TRACKS: usize = 27;
/// The first byte address of the cry players' songs.
pub(crate) const CRY_RAM: u32 = 0x8000_0000;
const CRY_SONG: usize = 0x34;

pub(crate) const MPT_FLG_VOLSET: u8 = 0x01;
pub(crate) const MPT_FLG_VOLCHG: u8 = 0x03;
pub(crate) const MPT_FLG_PITSET: u8 = 0x04;
pub(crate) const MPT_FLG_PITCHG: u8 = 0x0C;
pub(crate) const MPT_FLG_START: u8 = 0x40;
pub(crate) const MPT_FLG_EXIST: u8 = 0x80;

pub(crate) const SF_START: u8 = 0x80;
pub(crate) const SF_STOP: u8 = 0x40;
pub(crate) const SF_LOOP: u8 = 0x10;
pub(crate) const SF_IEC: u8 = 0x04;
pub(crate) const SF_ENV: u8 = 0x03;
pub(crate) const SF_ENV_ATTACK: u8 = 0x03;
pub(crate) const SF_ENV_DECAY: u8 = 0x02;
pub(crate) const SF_ENV_SUSTAIN: u8 = 0x01;
pub(crate) const SF_ON: u8 = SF_START | SF_STOP | SF_IEC | SF_ENV;

pub(crate) const TYPE_CGB: u8 = 0x07;
pub(crate) const TYPE_FIX: u8 = 0x08;
pub(crate) const TYPE_REV: u8 = 0x10;
pub(crate) const TYPE_SPL: u8 = 0x40;
pub(crate) const TYPE_RHY: u8 = 0x80;

const STATUS_TRACK: u32 = 0x0000_FFFF;
const STATUS_PAUSE: u32 = 0x8000_0000;
const TEMPORARY_FADE: u16 = 1;
const FADE_IN: u16 = 2;
const FADE_VOL_SHIFT: u16 = 2;
/// `m4aSoundMode`'s master volume (12) and its `divFreq` at 13,379 Hz:
/// `(16777216 / 13379 + 1) >> 1`.
const MASTER_VOLUME: u32 = 12;
pub(crate) const DIV_FREQ: u32 = 627;

const C_V: u8 = 0x40;

/// `struct ToneData`.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Tone {
    pub ty: u8,
    pub key: u8,
    pub length: u8,
    pub pan_sweep: u8,
    pub wav: u32,
    /// attack, decay, sustain, release; a keysplit's table offset.
    pub adsr: [u8; 4],
}

/// `struct MusicPlayerTrack`.
#[derive(Clone, Copy, Default)]
struct Track {
    flags: u8,
    wait: u8,
    pattern_level: u8,
    rep_n: u8,
    gate_time: u8,
    key: u8,
    velocity: u8,
    running_status: u8,
    key_m: i8,
    pit_m: u8,
    key_shift: i8,
    key_shift_x: i8,
    tune: i8,
    pit_x: u8,
    bend: i8,
    bend_range: u8,
    vol_mr: u8,
    vol_ml: u8,
    vol: u8,
    vol_x: u8,
    pan: i8,
    pan_x: i8,
    mod_m: i8,
    mod_: u8,
    mod_t: u8,
    lfo_speed: u8,
    lfo_speed_c: u8,
    lfo_delay: u8,
    lfo_delay_c: u8,
    priority: u8,
    echo_volume: u8,
    echo_length: u8,
    chan: u8,
    tone: Tone,
    timer: u16,
    unk_3c: u32,
    cmd_ptr: u32,
    pattern_stack: [u32; 3],
}

impl Track {
    /// `Clear64byte`: every field before `cmdPtr`.
    fn clear64(&mut self) {
        *self = Track {
            cmd_ptr: self.cmd_ptr,
            pattern_stack: self.pattern_stack,
            chan: NONE,
            ..Track::default()
        };
    }
}

/// `struct MusicPlayerInfo`.
#[derive(Clone, Copy, Default)]
struct Info {
    /// The SongHeader (0: none).
    song: u32,
    status: u32,
    track_count: u8,
    priority: u8,
    /// `unk_B`: a new song must have at least this one's priority (SE1, SE2).
    unk_b: u8,
    clock: u32,
    tempo_d: u16,
    tempo_u: u16,
    tempo_i: u16,
    tempo_c: u16,
    fade_oi: u16,
    fade_oc: u16,
    fade_ov: u16,
    first_track: u8,
    tone: u32,
}

/// `struct SoundChannel` and `struct CgbChannel` in one.
#[derive(Clone, Copy, Default)]
pub(crate) struct Chan {
    pub status: u8,
    pub ty: u8,
    pub right_volume: u8,
    pub left_volume: u8,
    pub attack: u8,
    pub decay: u8,
    pub sustain: u8,
    pub release: u8,
    pub key: u8,
    pub envelope_volume: u8,
    /// DirectSound: `envelopeVolumeRight`; PSG: `envelopeGoal`.
    pub env_right_or_goal: u8,
    /// DirectSound: `envelopeVolumeLeft`; PSG: `envelopeCounter`.
    pub env_left_or_counter: u8,
    pub echo_volume: u8,
    pub echo_length: u8,
    pub gate_time: u8,
    pub midi_key: u8,
    pub velocity: u8,
    pub priority: u8,
    pub rhythm_pan: i8,
    // DirectSound
    pub count: i32,
    pub fw: u32,
    pub frequency: u32,
    pub wav: u32,
    /// The current sample's index in the wave's data.
    pub pos: u32,
    pub track: u8,
    pub prev: u8,
    pub next: u8,
    /// Bumped on every note start (the GBA back end restarts its channel on a change).
    pub generation: u32,
    // PSG
    pub sustain_goal: u8,
    pub n4: u8,
    pub pan: u8,
    pub pan_mask: u8,
    pub modify: u8,
    pub length: u8,
    pub sweep: u8,
    pub current_wave: u32,
}

/// A DirectSound channel as a mixer back end needs it this frame.
#[derive(Clone, Copy, Debug)]
pub struct ChanView {
    /// Mixing this frame (`SOUND_CHANNEL_SF_ON`).
    pub on: bool,
    /// Bumped on every note start.
    pub generation: u32,
    /// The WaveData's offset in the blob.
    pub wav: u32,
    /// The position the note started at (a cry's `SetPokemonCryProgress`).
    pub start: u32,
    /// Source samples per output sample at [`crate::SAMPLE_RATE`], 23-bit fraction.
    pub step: u32,
    /// `envelopeVolumeRight` / `Left` (0..=255).
    pub right: u8,
    pub left: u8,
    pub looped: bool,
    pub reverse: bool,
}

/// The sound engine (`gSoundInfo`, the players and their tracks, the cry songs).
pub struct Engine<'a> {
    blob: Blob<'a>,
    players: [Info; PLAYERS],
    tracks: [Track; TRACKS],
    pub(crate) chans: [Chan; ALL_CHANS],
    mem_acc: [u8; 16],
    /// `soundInfo->reverb` (0..127).
    pub(crate) reverb: u8,
    pub(crate) c15: u8,
    /// `soundInfo->mode` bit 0: mono (`SetPokemonCryStereo(FALSE)`, Emerald's default).
    pub(crate) mono: bool,
    /// `gPokemonCrySong`: the template `SetPokemonCry*` edit.
    cry_template: [u8; CRY_SONG],
    cry_songs: [[u8; CRY_SONG]; 2],
    /// `gMPlay_PokemonCry`: the cry player of the last cry.
    last_cry: usize,
    /// The PSG registers as last written, from `REG_SOUND1CNT_L` (`0x04000060`).
    pub(crate) regs: [u8; 0x50],
}

impl<'a> Engine<'a> {
    /// `m4aSoundInit`: every player open and paused, the PSG on (`MPlayExtender`).
    pub fn new(blob: Blob<'a>, psg: &mut impl Psg) -> Self {
        let mut e = Engine {
            blob,
            players: [Info::default(); PLAYERS],
            tracks: [Track {
                chan: NONE,
                ..Track::default()
            }; TRACKS],
            chans: [Chan {
                track: NONE,
                prev: NONE,
                next: NONE,
                ..Chan::default()
            }; ALL_CHANS],
            mem_acc: [0; 16],
            reverb: 0,
            c15: 0,
            mono: true,
            cry_template: [0; CRY_SONG],
            cry_songs: [[0; CRY_SONG]; 2],
            last_cry: 4,
            regs: [0; 0x50],
        };
        let mut first = 0u8;
        for (i, p) in e.players.iter_mut().enumerate() {
            *p = Info {
                status: STATUS_PAUSE,
                track_count: TRACK_COUNTS[i],
                unk_b: (i == 1 || i == 2) as u8,
                first_track: first,
                ..Info::default()
            };
            first += TRACK_COUNTS[i];
        }
        for (c, mask) in [0x11u8, 0x22, 0x44, 0x88].into_iter().enumerate() {
            e.chans[CGB0 + c].ty = c as u8 + 1;
            e.chans[CGB0 + c].pan_mask = mask;
        }
        // MPlayExtender's register setup.
        e.write(psg, reg::SOUNDCNT_X, 0x8F);
        e.write(psg, reg::NR12, 0x08);
        e.write(psg, reg::NR22, 0x08);
        e.write(psg, reg::NR42, 0x08);
        e.write(psg, reg::NR14, 0x80);
        e.write(psg, reg::NR24, 0x80);
        e.write(psg, reg::NR44, 0x80);
        e.write(psg, reg::NR30, 0);
        e.write(psg, reg::NR50, 0x77);
        e.cry_template = cry_song_template(blob.dummy_tone());
        e
    }

    pub fn blob(&self) -> Blob<'a> {
        self.blob
    }

    pub(crate) fn write(&mut self, psg: &mut impl Psg, r: u8, v: u8) {
        if let Some(s) = self.regs.get_mut(r as usize) {
            *s = v;
        }
        psg.write(r, v);
    }

    // ---- memory ----

    fn byte(&self, at: u32) -> u8 {
        if at >= CRY_RAM {
            let (i, o) = (
                ((at - CRY_RAM) >> 8) as usize,
                ((at - CRY_RAM) & 0xFF) as usize,
            );
            self.cry_songs
                .get(i)
                .and_then(|s| s.get(o))
                .copied()
                .unwrap_or(0)
        } else {
            self.blob.u8(at)
        }
    }

    fn word(&self, at: u32) -> u32 {
        u32::from_le_bytes([
            self.byte(at),
            self.byte(at + 1),
            self.byte(at + 2),
            self.byte(at + 3),
        ])
    }

    fn tone(&self, at: u32) -> Tone {
        Tone {
            ty: self.byte(at),
            key: self.byte(at + 1),
            length: self.byte(at + 2),
            pan_sweep: self.byte(at + 3),
            wav: self.word(at + 4),
            adsr: [
                self.byte(at + 8),
                self.byte(at + 9),
                self.byte(at + 10),
                self.byte(at + 11),
            ],
        }
    }

    fn tracks_of(&self, p: usize) -> core::ops::Range<usize> {
        let i = &self.players[p];
        i.first_track as usize..i.first_track as usize + i.track_count as usize
    }

    // ---- commands ----

    /// Runs one command (call before [`Engine::frame`]).
    pub fn apply(&mut self, cmd: Cmd, psg: &mut impl Psg) {
        match cmd {
            Cmd::Start(n) => {
                if let Some((p, h)) = self.song(n) {
                    self.start(psg, p, h);
                }
            }
            Cmd::StartOrChange(n) => {
                if let Some((p, h)) = self.song(n) {
                    let i = &self.players[p];
                    if i.song != h || i.status & STATUS_TRACK == 0 || i.status & STATUS_PAUSE != 0 {
                        self.start(psg, p, h);
                    }
                }
            }
            Cmd::StopSong(n) => {
                if let Some((p, h)) = self.song(n)
                    && self.players[p].song == h
                {
                    self.stop(psg, p);
                }
            }
            Cmd::Stop(p) => self.stop(psg, p as usize),
            Cmd::Continue(p) => self.players[p as usize].status &= !STATUS_PAUSE,
            Cmd::StopAll => {
                for p in 0..PLAYERS {
                    self.stop(psg, p);
                }
            }
            Cmd::FadeOut(p, speed) => self.fade(p as usize, speed, 64 << FADE_VOL_SHIFT),
            Cmd::FadeOutTemporarily(p, speed) => {
                self.fade(p as usize, speed, (64 << FADE_VOL_SHIFT) | TEMPORARY_FADE)
            }
            Cmd::FadeIn(p, speed) => {
                self.fade(p as usize, speed, FADE_IN);
                self.players[p as usize].status &= !STATUS_PAUSE;
            }
            Cmd::ImmInit(p) => {
                for t in self.tracks_of(p as usize) {
                    let t = &mut self.tracks[t];
                    if t.flags & MPT_FLG_EXIST != 0 && t.flags & MPT_FLG_START != 0 {
                        t.clear64();
                        t.flags = MPT_FLG_EXIST;
                        t.bend_range = 2;
                        t.vol_x = 64;
                        t.lfo_speed = 22;
                        t.tone.ty = 1;
                    }
                }
            }
            Cmd::Volume(p, volume) => {
                for t in self.tracks_of(p as usize) {
                    let t = &mut self.tracks[t];
                    if t.flags & MPT_FLG_EXIST != 0 {
                        t.vol_x = (volume / 4) as u8;
                        t.flags |= MPT_FLG_VOLCHG;
                    }
                }
            }
            Cmd::Pan(p, pan) => {
                for t in self.tracks_of(p as usize) {
                    let t = &mut self.tracks[t];
                    if t.flags & MPT_FLG_EXIST != 0 {
                        t.pan_x = pan;
                        t.flags |= MPT_FLG_VOLCHG;
                    }
                }
            }
            Cmd::Cry(c) => self.cry(psg, c),
            Cmd::StopCry => self.stop(psg, self.last_cry),
            Cmd::Stereo(on) => self.mono = !on,
        }
    }

    /// `gSongTable[n]`: its player and SongHeader.
    fn song(&self, n: u16) -> Option<(usize, u32)> {
        let s = self.blob.song(n)?;
        (s.header != 0 && (s.player as usize) < 4).then_some((s.player as usize, s.header))
    }

    fn fade(&mut self, p: usize, speed: u16, ov: u16) {
        let i = &mut self.players[p];
        i.fade_oc = speed;
        i.fade_oi = speed;
        i.fade_ov = ov;
    }

    /// `MPlayStart`.
    fn start(&mut self, psg: &mut impl Psg, p: usize, header: u32) {
        let i = self.players[p];
        let new_priority = self.byte(header + 2);
        let first = &self.tracks[i.first_track as usize];
        let idle = (i.song == 0 || first.flags & MPT_FLG_START == 0)
            && (i.status & STATUS_TRACK == 0 || i.status & STATUS_PAUSE != 0);
        if i.unk_b != 0 && !idle && i.priority > new_priority {
            return;
        }
        let count = self.byte(header);
        let tone = self.word(header + 4);
        let info = &mut self.players[p];
        info.status = 0;
        info.song = header;
        info.tone = tone;
        info.priority = new_priority;
        info.clock = 0;
        info.tempo_d = 150;
        info.tempo_i = 150;
        info.tempo_u = 0x100;
        info.tempo_c = 0;
        info.fade_oi = 0;
        let range = self.tracks_of(p);
        for (k, t) in range.enumerate() {
            self.track_stop(psg, t);
            if k < count as usize {
                let ptr = self.word(header + 8 + 4 * k as u32);
                let tr = &mut self.tracks[t];
                tr.flags = MPT_FLG_EXIST | MPT_FLG_START;
                tr.chan = NONE;
                tr.cmd_ptr = ptr;
            } else {
                self.tracks[t].flags = 0;
            }
        }
        let reverb = self.byte(header + 3);
        if reverb & 0x80 != 0 {
            self.reverb = reverb & 0x7F;
        }
    }

    /// `m4aMPlayStop`.
    fn stop(&mut self, psg: &mut impl Psg, p: usize) {
        self.players[p].status |= STATUS_PAUSE;
        for t in self.tracks_of(p) {
            self.track_stop(psg, t);
        }
    }

    /// `TrackStop`.
    fn track_stop(&mut self, psg: &mut impl Psg, t: usize) {
        if self.tracks[t].flags & MPT_FLG_EXIST == 0 {
            return;
        }
        let mut c = self.tracks[t].chan;
        while c != NONE {
            let ch = c as usize;
            if self.chans[ch].status != 0 {
                if self.chans[ch].ty & TYPE_CGB != 0 {
                    self.cgb_osc_off(psg, ch - CGB0 + 1);
                }
                self.chans[ch].status = 0;
            }
            self.chans[ch].track = NONE;
            c = self.chans[ch].next;
        }
        self.tracks[t].chan = NONE;
    }

    /// `RealClearChain`.
    fn clear_chain(&mut self, c: usize) {
        let t = self.chans[c].track;
        if t == NONE {
            return;
        }
        let (next, prev) = (self.chans[c].next, self.chans[c].prev);
        if prev != NONE {
            self.chans[prev as usize].next = next;
        } else {
            self.tracks[t as usize].chan = next;
        }
        if next != NONE {
            self.chans[next as usize].prev = prev;
        }
        self.chans[c].track = NONE;
    }

    // ---- cries (`SetPokemonCry*`, `SetPokemonCryTone`) ----

    fn cry(&mut self, psg: &mut impl Psg, c: Cry) {
        let s = &mut self.cry_template;
        s[0x1D] = c.volume & 0x7F;
        s[0x28] = (c.pan as u8).wrapping_add(C_V) & 0x7F;
        // SetPokemonCryPitch
        let b = (c.pitch as i32 + 0x80) as i16;
        let a = s[0x19].wrapping_sub(s[0x12]);
        s[0x2A] = ((b >> 8) & 0x7F) as u8;
        s[0x12] = ((b >> 1) & 0x7F) as u8;
        s[0x19] = (a.wrapping_add(((b >> 1) & 0x7F) as u8)) & 0x7F;
        s[0x2E..0x30].copy_from_slice(&c.length.to_le_bytes());
        s[0x20..0x24].copy_from_slice(&0u32.to_le_bytes());
        s[0x26] = c.release;
        // SetPokemonCryChorus
        if c.chorus != 0 {
            s[0] = 2;
            s[0x19] = (c.chorus as u8).wrapping_add(s[0x12]) & 0x7F;
        } else {
            s[0] = 1;
        }
        s[2] = c.priority;
        let tone = self.blob.cry_tone(c.index, c.reverse);
        // SetPokemonCryTone: a free cry player, else the one that has played longest.
        let mut pick = None;
        let (mut max_clock, mut max_i) = (0, 0);
        for i in 0..2 {
            let t = &self.tracks[self.players[4 + i].first_track as usize];
            if t.flags == 0
                && (t.chan == NONE
                    || self.chans[t.chan as usize].track as usize
                        != self.players[4 + i].first_track as usize)
            {
                pick = Some(i);
                break;
            }
            if max_clock < self.players[4 + i].clock {
                max_clock = self.players[4 + i].clock;
                max_i = i;
            }
        }
        let i = pick.unwrap_or(max_i);
        let base = CRY_RAM + ((i as u32) << 8);
        let mut song = self.cry_template;
        song[4..8].copy_from_slice(&tone.to_le_bytes());
        song[8..12].copy_from_slice(&(base + 0x11).to_le_bytes());
        song[12..16].copy_from_slice(&(base + 0x18).to_le_bytes());
        song[0x14..0x18].copy_from_slice(&(base + 0x1A).to_le_bytes());
        self.cry_songs[i] = song;
        self.last_cry = 4 + i;
        self.start(psg, 4 + i, base);
    }

    // ---- queries (`src/sound.c`) ----

    /// `IsBGMStopped`... per player: no track of `p` exists.
    pub fn is_stopped(&self, p: Player) -> bool {
        self.players[p as usize].status & STATUS_TRACK == 0
    }

    /// `IsBGMPlaying` / `IsSpecialSEPlaying`: not paused and a track exists.
    pub fn is_playing(&self, p: Player) -> bool {
        let s = self.players[p as usize].status;
        s & STATUS_PAUSE == 0 && s & STATUS_TRACK != 0
    }

    /// `IsSEPlaying`.
    pub fn is_se_playing(&self) -> bool {
        let (a, b) = (self.players[1].status, self.players[2].status);
        let paused = a & STATUS_PAUSE != 0 && b & STATUS_PAUSE != 0;
        let empty = a & STATUS_TRACK == 0 && b & STATUS_TRACK == 0;
        !paused && !empty
    }

    /// `IsPokemonCryPlaying(gMPlay_PokemonCry)`.
    pub fn is_cry_playing(&self) -> bool {
        let t = self.players[self.last_cry].first_track as usize;
        let c = self.tracks[t].chan;
        c != NONE && self.chans[c as usize].track as usize == t
    }

    /// Whether the last cry's channel is still sounding (its sample not ended).
    pub fn is_cry_sounding(&self) -> bool {
        let t = self.players[self.last_cry].first_track as usize;
        let c = self.tracks[t].chan;
        c != NONE
            && self.chans[c as usize].track as usize == t
            && self.chans[c as usize].status & SF_ON != 0
    }

    /// The song header `p` plays (0: none).
    pub fn song_of(&self, p: Player) -> u32 {
        self.players[p as usize].song
    }

    // ---- one frame (`SoundMain`) ----

    /// `SoundMain` up to the mixing: every player's `MPlayMain` (BGM first), `CgbSound`, and
    /// each DirectSound channel's envelope step. The DirectSound mixing itself is the back
    /// end's: [`crate::synth`], or [`Engine::advance`] when nothing is mixed.
    pub fn frame(&mut self, psg: &mut impl Psg) {
        for p in 0..PLAYERS {
            self.mplay_main(psg, p);
        }
        self.cgb_sound(psg);
        for c in 0..CHANS {
            self.ds_envelope(c);
        }
    }

    /// The mixing's bookkeeping without the mixing: each sounding DirectSound channel moves
    /// `n` output samples through its wave (stopping at a non-looping end).
    pub fn advance(&mut self, n: u32) {
        for c in 0..CHANS {
            if self.chans[c].status & SF_ON != 0 {
                self.advance_chan(c, n);
            }
        }
    }

    pub(crate) fn step_of(&self, c: usize) -> u32 {
        if self.chans[c].ty & TYPE_FIX != 0 {
            0x80_0000
        } else {
            self.chans[c].frequency.wrapping_mul(DIV_FREQ)
        }
    }

    pub(crate) fn wave_size(&self, wav: u32) -> u32 {
        self.blob.u32(wav + 12)
    }

    fn advance_chan(&mut self, c: usize, n: u32) {
        let step = self.step_of(c) as u64;
        let total = self.chans[c].fw as u64 + step * n as u64;
        let used = (total >> 23) as i64;
        self.chans[c].fw = (total & 0x7F_FFFF) as u32;
        self.consume(c, used);
    }

    /// Moves channel `c` `used` samples on, wrapping at the loop or stopping at the end.
    pub(crate) fn consume(&mut self, c: usize, used: i64) {
        let ch = &mut self.chans[c];
        let mut count = ch.count as i64 - used;
        if count <= 0 {
            if ch.status & SF_LOOP != 0 {
                let size = self.blob.u32(ch.wav + 12) as i64;
                let lp = self.blob.u32(ch.wav + 8) as i64;
                let len = (size - lp).max(1);
                while count <= 0 {
                    count += len;
                }
                ch.pos = (size - count) as u32;
                ch.count = count as i32;
            } else {
                ch.status = 0;
                ch.count = 0;
            }
        } else {
            ch.pos = ch.pos.wrapping_add(used as u32);
            ch.count = count as i32;
        }
    }

    /// Mono output (`SetPokemonCryStereo(FALSE)`, Emerald's default): both sides get both
    /// FIFOs.
    pub fn mono(&self) -> bool {
        self.mono
    }

    /// The DirectSound channels for a mixer back end.
    pub fn ds_chans(&self) -> [ChanView; CHANS] {
        core::array::from_fn(|c| {
            let ch = &self.chans[c];
            ChanView {
                on: ch.status & SF_ON != 0,
                generation: ch.generation,
                wav: ch.wav,
                start: ch.pos,
                step: self.step_of(c),
                right: ch.env_right_or_goal,
                left: ch.env_left_or_counter,
                looped: ch.status & SF_LOOP != 0,
                reverse: ch.ty & TYPE_REV != 0,
            }
        })
    }

    /// `SoundMainRAM`'s per-channel envelope, before the channel is mixed.
    fn ds_envelope(&mut self, c: usize) {
        let ch = &mut self.chans[c];
        let mut status = ch.status;
        if status & SF_ON == 0 {
            return;
        }
        let mut env: u32;
        if status & SF_START != 0 {
            if status & SF_STOP != 0 {
                ch.status = 0;
                return;
            }
            status = SF_ENV_ATTACK;
            let size = self.blob.u32(ch.wav + 12);
            let start = ch.count.max(0) as u32;
            ch.pos = start;
            ch.count = size as i32 - start as i32;
            ch.fw = 0;
            if self.blob.u16(ch.wav + 2) & 0x4000 != 0 {
                status |= SF_LOOP;
            }
            ch.generation = ch.generation.wrapping_add(1);
            env = ch.attack as u32;
            if env >= 0xFF {
                env = 0xFF;
                status -= 1;
            }
            if ch.count <= 0 {
                // An empty wave: Gen3 would read past it; stop instead.
                status = 0;
            }
        } else {
            env = ch.envelope_volume as u32;
            if status & SF_IEC != 0 {
                // `subs; bhi`: on while the length was at least 2.
                let old = ch.echo_length;
                ch.echo_length = old.wrapping_sub(1);
                if old <= 1 {
                    ch.status = 0;
                    return;
                }
            } else if status & SF_STOP != 0 {
                env = (env * ch.release as u32) >> 8;
                if env <= ch.echo_volume as u32 {
                    env = ch.echo_volume as u32;
                    if env == 0 {
                        ch.status = 0;
                        return;
                    }
                    status |= SF_IEC;
                }
            } else {
                match status & SF_ENV {
                    SF_ENV_DECAY => {
                        env = (env * ch.decay as u32) >> 8;
                        if env <= ch.sustain as u32 {
                            env = ch.sustain as u32;
                            if env == 0 {
                                env = ch.echo_volume as u32;
                                if env == 0 {
                                    ch.status = 0;
                                    return;
                                }
                                status |= SF_IEC;
                            } else {
                                status -= 1;
                            }
                        }
                    }
                    SF_ENV_ATTACK => {
                        env += ch.attack as u32;
                        if env >= 0xFF {
                            env = 0xFF;
                            status -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        ch.status = status;
        ch.envelope_volume = env as u8;
        let v = ((MASTER_VOLUME + 1) * env) >> 4;
        ch.env_right_or_goal = ((ch.right_volume as u32 * v) >> 8) as u8;
        ch.env_left_or_counter = ((ch.left_volume as u32 * v) >> 8) as u8;
    }

    // ---- the sequencer ----

    /// `MPlayMain`.
    fn mplay_main(&mut self, psg: &mut impl Psg, p: usize) {
        if self.players[p].status & STATUS_PAUSE != 0 {
            return;
        }
        self.fade_out_body(psg, p);
        if self.players[p].status & STATUS_PAUSE != 0 {
            return;
        }
        let range = self.tracks_of(p);
        let mut tempo_c = self.players[p]
            .tempo_c
            .wrapping_add(self.players[p].tempo_i);
        while tempo_c >= 150 {
            self.players[p].tempo_c = tempo_c;
            let mut bits = 0u32;
            for (k, t) in range.clone().enumerate() {
                if self.tracks[t].flags & MPT_FLG_EXIST == 0 {
                    continue;
                }
                bits |= 1 << k;
                // The gate times of the track's channels.
                let mut c = self.tracks[t].chan;
                while c != NONE {
                    let ch = c as usize;
                    let next = self.chans[ch].next;
                    if self.chans[ch].status & SF_ON != 0 {
                        if self.chans[ch].gate_time != 0 {
                            self.chans[ch].gate_time -= 1;
                            if self.chans[ch].gate_time == 0 {
                                self.chans[ch].status |= SF_STOP;
                            }
                        }
                    } else {
                        self.clear_chain(ch);
                    }
                    c = next;
                }
                if self.tracks[t].flags & MPT_FLG_START != 0 {
                    let tr = &mut self.tracks[t];
                    tr.clear64();
                    tr.flags = MPT_FLG_EXIST;
                    tr.bend_range = 2;
                    tr.vol_x = 64;
                    tr.lfo_speed = 22;
                    tr.tone.ty = 1;
                }
                let mut ended = false;
                while self.tracks[t].wait == 0 {
                    let at = self.tracks[t].cmd_ptr;
                    let mut cmd = self.byte(at);
                    if cmd < 0x80 {
                        cmd = self.tracks[t].running_status;
                    } else {
                        self.tracks[t].cmd_ptr = at + 1;
                        if cmd >= 0xBD {
                            self.tracks[t].running_status = cmd;
                        }
                    }
                    if cmd >= 0xCF {
                        self.ply_note(psg, cmd - 0xCF, p, t);
                    } else if cmd > 0xB0 {
                        self.ply(psg, cmd, p, t);
                        if self.tracks[t].flags == 0 {
                            ended = true;
                            break;
                        }
                    } else {
                        self.tracks[t].wait = CLOCK[(cmd - 0x80) as usize];
                    }
                }
                if ended {
                    continue;
                }
                let tr = &mut self.tracks[t];
                tr.wait -= 1;
                // The LFO.
                if tr.lfo_speed != 0 && tr.mod_ != 0 {
                    if tr.lfo_delay_c != 0 {
                        tr.lfo_delay_c -= 1;
                    } else {
                        tr.lfo_speed_c = tr.lfo_speed_c.wrapping_add(tr.lfo_speed);
                        let x = tr.lfo_speed_c;
                        let wave: i32 = if (x.wrapping_sub(0x40) as i8) < 0 {
                            x as i8 as i32
                        } else {
                            0x80 - x as i32
                        };
                        let m = (tr.mod_ as i32 * wave) >> 6;
                        if (m as u8) != tr.mod_m as u8 {
                            tr.mod_m = m as i8;
                            tr.flags |= if tr.mod_t == 0 {
                                MPT_FLG_PITCHG
                            } else {
                                MPT_FLG_VOLCHG
                            };
                        }
                    }
                }
            }
            self.players[p].clock = self.players[p].clock.wrapping_add(1);
            if bits == 0 {
                self.players[p].status = STATUS_PAUSE;
                return;
            }
            self.players[p].status = bits;
            tempo_c = self.players[p].tempo_c - 150;
        }
        self.players[p].tempo_c = tempo_c;
        // Volume and pitch.
        for t in range {
            let f = self.tracks[t].flags;
            if f & MPT_FLG_EXIST == 0 || f & (MPT_FLG_VOLCHG | MPT_FLG_PITCHG) == 0 {
                continue;
            }
            self.trk_vol_pit_set(t);
            let mut c = self.tracks[t].chan;
            while c != NONE {
                let ch = c as usize;
                let next = self.chans[ch].next;
                if self.chans[ch].status & SF_ON == 0 {
                    self.clear_chain(ch);
                } else {
                    let cgb = self.chans[ch].ty & TYPE_CGB;
                    let flags = self.tracks[t].flags;
                    if flags & MPT_FLG_VOLCHG != 0 {
                        self.chn_vol_set(ch, t);
                        if cgb != 0 {
                            self.chans[ch].modify |= crate::cgb::MO_VOL;
                        }
                    }
                    if flags & MPT_FLG_PITCHG != 0 {
                        let key = (self.chans[ch].key as i32 + self.tracks[t].key_m as i32).max(0);
                        let pit = self.tracks[t].pit_m;
                        if cgb != 0 {
                            self.chans[ch].frequency =
                                crate::cgb::midi_key_to_cgb_freq(cgb, key as u8, pit);
                            self.chans[ch].modify |= crate::cgb::MO_PIT;
                        } else {
                            self.chans[ch].frequency =
                                self.midi_key_to_freq(self.chans[ch].wav, key as u8, pit);
                        }
                    }
                }
                c = next;
            }
            self.tracks[t].flags &= 0xF0;
        }
    }

    /// `FadeOutBody`.
    fn fade_out_body(&mut self, psg: &mut impl Psg, p: usize) {
        let i = &mut self.players[p];
        if i.fade_oi == 0 {
            return;
        }
        i.fade_oc = i.fade_oc.wrapping_sub(1);
        if i.fade_oc != 0 {
            return;
        }
        i.fade_oc = i.fade_oi;
        if i.fade_ov & FADE_IN != 0 {
            i.fade_ov = i.fade_ov.wrapping_add(4 << FADE_VOL_SHIFT);
            if i.fade_ov >= 64 << FADE_VOL_SHIFT {
                i.fade_ov = 64 << FADE_VOL_SHIFT;
                i.fade_oi = 0;
            }
        } else {
            i.fade_ov = i.fade_ov.wrapping_sub(4 << FADE_VOL_SHIFT);
            if (i.fade_ov as i16) <= 0 {
                let temporary = i.fade_ov & TEMPORARY_FADE != 0;
                for t in self.tracks_of(p) {
                    self.track_stop(psg, t);
                    if !temporary {
                        self.tracks[t].flags = 0;
                    }
                }
                let i = &mut self.players[p];
                if temporary {
                    i.status |= STATUS_PAUSE;
                } else {
                    i.status = STATUS_PAUSE;
                }
                i.fade_oi = 0;
                return;
            }
        }
        let vol_x = (self.players[p].fade_ov >> FADE_VOL_SHIFT) as u8;
        for t in self.tracks_of(p) {
            let tr = &mut self.tracks[t];
            if tr.flags & MPT_FLG_EXIST != 0 {
                tr.vol_x = vol_x;
                tr.flags |= MPT_FLG_VOLCHG;
            }
        }
    }

    /// `TrkVolPitSet`.
    fn trk_vol_pit_set(&mut self, t: usize) {
        let tr = &mut self.tracks[t];
        if tr.flags & MPT_FLG_VOLSET != 0 {
            let mut x = (tr.vol as u32 * tr.vol_x as u32) >> 5;
            if tr.mod_t == 1 {
                x = (x * (tr.mod_m as i32 + 128) as u32) >> 7;
            }
            let mut y = 2 * tr.pan as i32 + tr.pan_x as i32;
            if tr.mod_t == 2 {
                y += tr.mod_m as i32;
            }
            y = y.clamp(-128, 127);
            tr.vol_mr = (((y + 128) as u32 * x) >> 8) as u8;
            tr.vol_ml = (((127 - y) as u32 * x) >> 8) as u8;
        }
        if tr.flags & MPT_FLG_PITSET != 0 {
            let bend = tr.bend as i32 * tr.bend_range as i32;
            let mut x = (tr.tune as i32 + bend) * 4
                + ((tr.key_shift as i32) << 8)
                + ((tr.key_shift_x as i32) << 8)
                + tr.pit_x as i32;
            if tr.mod_t == 0 {
                x += 16 * tr.mod_m as i32;
            }
            tr.key_m = (x >> 8) as i8;
            tr.pit_m = x as u8;
        }
        tr.flags &= !(MPT_FLG_PITSET | MPT_FLG_VOLSET);
    }

    /// `ChnVolSetAsm`.
    fn chn_vol_set(&mut self, c: usize, t: usize) {
        let ch = &mut self.chans[c];
        let tr = &self.tracks[t];
        let pan = ch.rhythm_pan as i32;
        let r = ((0x80 + pan) * ch.velocity as i32 * tr.vol_mr as i32) >> 14;
        let l = ((0x7F - pan) * ch.velocity as i32 * tr.vol_ml as i32) >> 14;
        ch.right_volume = r.clamp(0, 0xFF) as u8;
        ch.left_volume = l.clamp(0, 0xFF) as u8;
    }

    /// `MidiKeyToFreq`.
    pub(crate) fn midi_key_to_freq(&self, wav: u32, key: u8, fine: u8) -> u32 {
        let (mut key, mut fine) = (key as usize, (fine as u32) << 24);
        if key > 178 {
            key = 178;
            fine = 255 << 24;
        }
        let v = |k: usize| {
            let s = SCALE[k];
            FREQ[(s & 0xF) as usize] >> (s >> 4)
        };
        let (a, b) = (v(key), v(key + 1));
        let x = a.wrapping_add(umul3232h32(b.wrapping_sub(a), fine));
        umul3232h32(self.blob.u32(wav + 4), x)
    }

    /// The jump table (`gMPlayJumpTable`) for commands 0xB1..=0xCE.
    fn ply(&mut self, psg: &mut impl Psg, cmd: u8, p: usize, t: usize) {
        match cmd {
            0xB2 => self.ply_goto(t),
            0xB3 => {
                // PATT
                let tr = &mut self.tracks[t];
                if tr.pattern_level < 3 {
                    tr.pattern_stack[tr.pattern_level as usize] = tr.cmd_ptr + 4;
                    tr.pattern_level += 1;
                    self.ply_goto(t);
                } else {
                    self.ply_fine(t);
                }
            }
            0xB4 => {
                // PEND
                let tr = &mut self.tracks[t];
                if tr.pattern_level != 0 {
                    tr.pattern_level -= 1;
                    tr.cmd_ptr = tr.pattern_stack[tr.pattern_level as usize];
                }
            }
            0xB5 => {
                // REPT
                let at = self.tracks[t].cmd_ptr;
                let n = self.byte(at);
                if n == 0 {
                    self.tracks[t].cmd_ptr = at + 1;
                    self.ply_goto(t);
                } else {
                    let tr = &mut self.tracks[t];
                    tr.rep_n = tr.rep_n.wrapping_add(1);
                    tr.cmd_ptr = at + 1;
                    if tr.rep_n < n {
                        self.ply_goto(t);
                    } else {
                        tr.rep_n = 0;
                        tr.cmd_ptr = at + 5;
                    }
                }
            }
            0xB9 => self.ply_memacc(t),
            0xBA => self.tracks[t].priority = self.next_byte(t),
            0xBB => {
                let b = self.next_byte(t) as u16;
                let i = &mut self.players[p];
                i.tempo_d = b * 2;
                i.tempo_i = ((i.tempo_d as u32 * i.tempo_u as u32) >> 8) as u16;
            }
            0xBC => {
                self.tracks[t].key_shift = self.next_byte(t) as i8;
                self.tracks[t].flags |= MPT_FLG_PITCHG;
            }
            0xBD => {
                let n = self.next_byte(t) as u32;
                let tone = self.tone(self.players[p].tone + n * TONE);
                self.tracks[t].tone = tone;
            }
            0xBE => {
                self.tracks[t].vol = self.next_byte(t);
                self.tracks[t].flags |= MPT_FLG_VOLCHG;
            }
            0xBF => {
                self.tracks[t].pan = self.next_byte(t).wrapping_sub(C_V) as i8;
                self.tracks[t].flags |= MPT_FLG_VOLCHG;
            }
            0xC0 => {
                self.tracks[t].bend = self.next_byte(t).wrapping_sub(C_V) as i8;
                self.tracks[t].flags |= MPT_FLG_PITCHG;
            }
            0xC1 => {
                self.tracks[t].bend_range = self.next_byte(t);
                self.tracks[t].flags |= MPT_FLG_PITCHG;
            }
            0xC2 => {
                let b = self.next_byte(t);
                self.tracks[t].lfo_speed = b;
                if b == 0 {
                    self.clear_mod_m(t);
                }
            }
            0xC3 => self.tracks[t].lfo_delay = self.next_byte(t),
            0xC4 => {
                let b = self.next_byte(t);
                self.tracks[t].mod_ = b;
                if b == 0 {
                    self.clear_mod_m(t);
                }
            }
            0xC5 => {
                let b = self.next_byte(t);
                if self.tracks[t].mod_t != b {
                    self.tracks[t].mod_t = b;
                    self.tracks[t].flags |= MPT_FLG_VOLCHG | MPT_FLG_PITCHG;
                }
            }
            0xC8 => {
                self.tracks[t].tune = self.next_byte(t).wrapping_sub(C_V) as i8;
                self.tracks[t].flags |= MPT_FLG_PITCHG;
            }
            0xCC => {
                // PORT: a sound register write.
                let r = self.next_byte(t);
                let v = self.next_byte(t);
                self.write(psg, r, v);
            }
            0xCD => self.ply_xcmd(t),
            0xCE => self.ply_endtie(t),
            // FINE, and the unused slots that run it.
            _ => self.ply_fine(t),
        }
    }

    fn next_byte(&mut self, t: usize) -> u8 {
        let at = self.tracks[t].cmd_ptr;
        self.tracks[t].cmd_ptr = at + 1;
        self.byte(at)
    }

    fn ply_goto(&mut self, t: usize) {
        let to = self.word(self.tracks[t].cmd_ptr);
        self.tracks[t].cmd_ptr = to;
    }

    /// `ply_fine`.
    fn ply_fine(&mut self, t: usize) {
        let mut c = self.tracks[t].chan;
        while c != NONE {
            let ch = c as usize;
            if self.chans[ch].status & SF_ON != 0 {
                self.chans[ch].status |= SF_STOP;
            }
            self.clear_chain(ch);
            c = self.chans[ch].next;
        }
        self.tracks[t].flags = 0;
    }

    /// `clear_modM`.
    fn clear_mod_m(&mut self, t: usize) {
        let tr = &mut self.tracks[t];
        tr.mod_m = 0;
        tr.lfo_speed_c = 0;
        tr.flags |= if tr.mod_t == 0 {
            MPT_FLG_PITCHG
        } else {
            MPT_FLG_VOLCHG
        };
    }

    /// `ply_memacc`.
    fn ply_memacc(&mut self, t: usize) {
        let op = self.next_byte(t);
        let a = self.next_byte(t) as usize & 15;
        let data = self.next_byte(t);
        let m = |e: &Self, i: u8| e.mem_acc[i as usize & 15];
        let x = self.mem_acc[a];
        let cond = match op {
            0 => {
                self.mem_acc[a] = data;
                return;
            }
            1 => {
                self.mem_acc[a] = x.wrapping_add(data);
                return;
            }
            2 => {
                self.mem_acc[a] = x.wrapping_sub(data);
                return;
            }
            3 => {
                self.mem_acc[a] = m(self, data);
                return;
            }
            4 => {
                self.mem_acc[a] = x.wrapping_add(m(self, data));
                return;
            }
            5 => {
                self.mem_acc[a] = x.wrapping_sub(m(self, data));
                return;
            }
            6 => x == data,
            7 => x != data,
            8 => x > data,
            9 => x >= data,
            10 => x <= data,
            11 => x < data,
            12 => x == m(self, data),
            13 => x != m(self, data),
            14 => x > m(self, data),
            15 => x >= m(self, data),
            16 => x <= m(self, data),
            17 => x < m(self, data),
            _ => return,
        };
        if cond {
            self.ply_goto(t);
        } else {
            self.tracks[t].cmd_ptr += 4;
        }
    }

    /// `ply_xcmd` and `gXcmdTable`.
    fn ply_xcmd(&mut self, t: usize) {
        let n = self.next_byte(t);
        let at = self.tracks[t].cmd_ptr;
        match n {
            1 => {
                // xWAVE: a ROM pointer, never in the extracted songs.
                self.tracks[t].cmd_ptr = at + 4;
            }
            2 => self.tracks[t].tone.ty = self.next_byte(t),
            4 => self.tracks[t].tone.adsr[0] = self.next_byte(t),
            5 => self.tracks[t].tone.adsr[1] = self.next_byte(t),
            6 => self.tracks[t].tone.adsr[2] = self.next_byte(t),
            7 => self.tracks[t].tone.adsr[3] = self.next_byte(t),
            8 => self.tracks[t].echo_volume = self.next_byte(t),
            9 => self.tracks[t].echo_length = self.next_byte(t),
            10 => self.tracks[t].tone.length = self.next_byte(t),
            11 => self.tracks[t].tone.pan_sweep = self.next_byte(t),
            12 => {
                // xWAIT
                let len = u16::from_le_bytes([self.byte(at), self.byte(at + 1)]);
                let tr = &mut self.tracks[t];
                if tr.timer < len {
                    tr.timer += 1;
                    tr.cmd_ptr = at - 2;
                    tr.wait = 1;
                } else {
                    tr.timer = 0;
                    tr.cmd_ptr = at + 2;
                }
            }
            13 => {
                self.tracks[t].unk_3c = self.word(at);
                self.tracks[t].cmd_ptr = at + 4;
            }
            // ply_xxx: FINE.
            _ => self.ply_fine(t),
        }
    }

    /// `ply_endtie`.
    fn ply_endtie(&mut self, t: usize) {
        let at = self.tracks[t].cmd_ptr;
        let b = self.byte(at);
        let key = if b < 0x80 {
            self.tracks[t].key = b;
            self.tracks[t].cmd_ptr = at + 1;
            b
        } else {
            self.tracks[t].key
        };
        let mut c = self.tracks[t].chan;
        while c != NONE {
            let ch = &mut self.chans[c as usize];
            if ch.status & (SF_START | SF_ENV) != 0
                && ch.status & SF_STOP == 0
                && ch.midi_key == key
            {
                ch.status |= SF_STOP;
                return;
            }
            c = ch.next;
        }
    }

    /// `ply_note`: a note of `gClockTable[clock]` ticks.
    fn ply_note(&mut self, _psg: &mut impl Psg, clock: u8, p: usize, t: usize) {
        {
            let tr = &mut self.tracks[t];
            tr.gate_time = CLOCK[clock as usize];
        }
        let at = self.tracks[t].cmd_ptr;
        let b0 = self.byte(at);
        if b0 < 0x80 {
            self.tracks[t].key = b0;
            let mut ptr = at + 1;
            let b1 = self.byte(ptr);
            if b1 < 0x80 {
                self.tracks[t].velocity = b1;
                ptr += 1;
                let b2 = self.byte(ptr);
                if b2 < 0x80 {
                    let tr = &mut self.tracks[t];
                    tr.gate_time = tr.gate_time.wrapping_add(b2);
                    ptr += 1;
                }
            }
            self.tracks[t].cmd_ptr = ptr;
        }
        let mut rhythm_pan: i32 = 0;
        let tt = self.tracks[t].tone.ty;
        let (tone, key) = if tt & (TYPE_RHY | TYPE_SPL) != 0 {
            let k = self.tracks[t].key;
            let idx = if tt & TYPE_SPL != 0 {
                self.blob.u8(self.word_of_adsr(t).wrapping_add(k as u32))
            } else {
                k
            };
            let sub = self.tone(self.tracks[t].tone.wav + idx as u32 * TONE);
            if sub.ty & (TYPE_SPL | TYPE_RHY) != 0 {
                return;
            }
            if tt & TYPE_RHY != 0 {
                if sub.pan_sweep & 0x80 != 0 {
                    rhythm_pan = (sub.pan_sweep as i32 - 0xC0) * 2;
                }
                (sub, sub.key)
            } else {
                (sub, k)
            }
        } else {
            (self.tracks[t].tone, self.tracks[t].key)
        };
        let prio =
            (self.players[p].priority as u32 + self.tracks[t].priority as u32).min(0xFF) as u8;
        let cgb = tone.ty & TYPE_CGB;
        // Track "addresses" for the tie-breaks: index + 1, 0 for none.
        let addr = |tr: u8| if tr == NONE { 0 } else { tr as u32 + 1 };
        let me = t as u32 + 1;
        let c = if cgb != 0 {
            let c = CGB0 + cgb as usize - 1;
            let ch = &self.chans[c];
            if ch.status & SF_ON != 0
                && ch.status & SF_STOP == 0
                && (ch.priority > prio || (ch.priority == prio && addr(ch.track) < me))
            {
                return;
            }
            c
        } else {
            let (mut pr, mut tr) = (prio, me);
            let mut stopped = false;
            let mut best = None;
            let mut free = None;
            for c in 0..CHANS {
                let ch = &self.chans[c];
                if ch.status & SF_ON == 0 {
                    free = Some(c);
                    break;
                }
                if ch.status & SF_STOP != 0 {
                    if !stopped {
                        stopped = true;
                        pr = ch.priority;
                        tr = addr(ch.track);
                        best = Some(c);
                        continue;
                    }
                } else if stopped {
                    continue;
                }
                if ch.priority < pr {
                    pr = ch.priority;
                    tr = addr(ch.track);
                    best = Some(c);
                } else if ch.priority == pr {
                    let a = addr(ch.track);
                    if a > tr {
                        tr = a;
                        best = Some(c);
                    } else if a == tr {
                        best = Some(c);
                    }
                }
            }
            match free.or(best) {
                Some(c) => c,
                None => return,
            }
        };
        self.clear_chain(c);
        let head = self.tracks[t].chan;
        self.chans[c].prev = NONE;
        self.chans[c].next = head;
        if head != NONE {
            self.chans[head as usize].prev = c as u8;
        }
        self.tracks[t].chan = c as u8;
        self.chans[c].track = t as u8;
        let delay = self.tracks[t].lfo_delay;
        self.tracks[t].lfo_delay_c = delay;
        if delay != 0 {
            self.clear_mod_m(t);
        }
        self.trk_vol_pit_set(t);
        let tr = self.tracks[t];
        {
            let ch = &mut self.chans[c];
            ch.gate_time = tr.gate_time;
            ch.midi_key = tr.key;
            ch.velocity = tr.velocity;
            ch.priority = prio;
            ch.key = key;
            ch.rhythm_pan = rhythm_pan as i8;
            ch.ty = tone.ty;
            ch.wav = tone.wav;
            ch.attack = tone.adsr[0];
            ch.decay = tone.adsr[1];
            ch.sustain = tone.adsr[2];
            ch.release = tone.adsr[3];
            ch.echo_volume = tr.echo_volume;
            ch.echo_length = tr.echo_length;
        }
        self.chn_vol_set(c, t);
        let k = (key as i32 + tr.key_m as i32).max(0) as u8;
        if cgb != 0 {
            let ch = &mut self.chans[c];
            ch.length = tone.length;
            let ps = tone.pan_sweep;
            ch.sweep = if ps & 0x80 != 0 || ps & 0x70 == 0 {
                8
            } else {
                ps
            };
            ch.frequency = crate::cgb::midi_key_to_cgb_freq(cgb, k, tr.pit_m);
        } else {
            self.chans[c].count = tr.unk_3c as i32;
            self.chans[c].frequency = self.midi_key_to_freq(tone.wav, k, tr.pit_m);
        }
        self.chans[c].status = SF_START;
        self.tracks[t].flags &= 0xF0;
    }

    /// A keysplit tone's table offset (its last four bytes).
    fn word_of_adsr(&self, t: usize) -> u32 {
        u32::from_le_bytes(self.tracks[t].tone.adsr)
    }
}

fn umul3232h32(a: u32, b: u32) -> u32 {
    ((a as u64 * b as u64) >> 32) as u32
}

/// `gPokemonCrySongTemplate` (`src/m4a_tables.c`).
fn cry_song_template(dummy_tone: u32) -> [u8; CRY_SONG] {
    const TUNE: u8 = 0xC8;
    const GOTO: u8 = 0xB2;
    const VOICE: u8 = 0xBD;
    const VOL: u8 = 0xBE;
    const XCMD: u8 = 0xCD;
    const PAN: u8 = 0xBF;
    const TIE: u8 = 0xCF;
    const EOT: u8 = 0xCE;
    const FINE: u8 = 0xB1;
    let mut s = [0u8; CRY_SONG];
    s[0] = 1; // trackCount
    s[2] = 255; // priority
    s[4..8].copy_from_slice(&dummy_tone.to_le_bytes());
    s[0x11] = TUNE;
    s[0x12] = C_V;
    s[0x13] = GOTO;
    s[0x18] = TUNE;
    s[0x19] = C_V + 16;
    s[0x1A] = VOICE;
    s[0x1B] = 0;
    s[0x1C] = VOL;
    s[0x1D] = 127;
    s[0x1E] = XCMD;
    s[0x1F] = 0x0D;
    s[0x24] = XCMD;
    s[0x25] = 0x07; // xRELE
    s[0x26] = 0;
    s[0x27] = PAN;
    s[0x28] = C_V;
    s[0x29] = TIE;
    s[0x2A] = 60; // Cn3
    s[0x2B] = 127;
    s[0x2C] = XCMD;
    s[0x2D] = 0x0C; // xWAIT
    s[0x2E..0x30].copy_from_slice(&60u16.to_le_bytes());
    s[0x30] = EOT;
    s[0x31] = FINE;
    s
}
