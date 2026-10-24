//! Sound on the GBA (Milestone 12): the MP2K engine of the `sound` crate fed each frame's
//! commands, its PSG channels on the hardware registers, its DirectSound channels on agb's
//! mixer.
//!
//! Gen3 mixes its own DirectSound channels (`SoundMainRAM`, 13,379 Hz, in IWRAM); here each of
//! the engine's five channels plays on an agb mixer channel at 10,512 Hz, its pitch, volume and
//! panning set once a frame from the engine's envelope ([`sound::ChanView`]). Differences, all
//! small: the volume steps are agb's sixteenths, a reversed sample (a few cry modes) plays
//! forwards, and there is no reverb.

use agb::fixnum::Num;
use agb::sound::mixer::{ChannelId, Mixer, SoundChannel, SoundData};
use alloc::boxed::Box;
use engine::audio::Cmd;
use sound::{Blob, CHANS, ChanView, Engine, Psg, SAMPLE_RATE, SAMPLES_PER_FRAME};

/// The mixer's rate (`Frequency::Hz10512`).
pub const MIXER_HZ: u64 = 10512;
const SOUNDCNT_H: *mut u16 = 0x0400_0082 as *mut u16;

/// The engine's writes go to the sound registers (`0x04000060` on).
struct HwPsg;

impl Psg for HwPsg {
    fn write(&mut self, reg: u8, value: u8) {
        // SAFETY: `reg` is one of the engine's PSG register offsets, all in the sound block.
        unsafe { ((0x0400_0060 + reg as usize) as *mut u8).write_volatile(value) }
    }
}

/// The mixer channel playing an engine channel's note.
struct Voice {
    id: ChannelId,
    generation: u32,
}

pub struct Audio<'gba> {
    mixer: Mixer<'gba>,
    engine: Box<Engine<'static>>,
    voices: [Option<Voice>; CHANS],
}

impl<'gba> Audio<'gba> {
    /// The engine on `blob` (the content's sound blob), `None` without one (the stub).
    pub fn new(mixer: Mixer<'gba>, blob: &'static [u8]) -> Option<Self> {
        let blob = Blob::new(blob)?;
        let engine = Box::new(Engine::new(blob, &mut HwPsg));
        // The PSG at 100% next to agb's DirectSound setup, as Gen3's `SOUND_ALL_MIX_FULL`.
        // SAFETY: SOUNDCNT_H is a plain sound register.
        unsafe { SOUNDCNT_H.write_volatile((SOUNDCNT_H.read_volatile() & !3) | 2) };
        Some(Audio {
            mixer,
            engine,
            voices: Default::default(),
        })
    }

    /// Hands the game's commands to the engine, in order.
    pub fn apply(&mut self, cmds: &[Cmd]) {
        for &c in cmds {
            self.engine.apply(c, &mut HwPsg);
        }
    }

    /// One frame: `cmds`, then `SoundMain` (the players, the PSG, the envelopes), then the
    /// mixer's channels and its buffer.
    pub fn frame(&mut self, cmds: &[Cmd]) {
        self.apply(cmds);
        self.engine.frame(&mut HwPsg);
        let chans = self.engine.ds_chans();
        self.engine.advance(SAMPLES_PER_FRAME as u32);
        let mono = self.engine.mono();
        let blob = self.engine.blob();
        for (v, slot) in chans.iter().zip(self.voices.iter_mut()) {
            let same = slot
                .as_ref()
                .is_some_and(|o| v.on && o.generation == v.generation);
            if !same {
                if let Some(old) = slot.take()
                    && let Some(ch) = self.mixer.channel(&old.id)
                {
                    ch.stop();
                }
                if v.on {
                    *slot = start(blob, v).and_then(|ch| {
                        let id = self.mixer.play_sound(ch)?;
                        Some(Voice {
                            id,
                            generation: v.generation,
                        })
                    });
                }
            }
            if let Some(o) = slot
                && let Some(ch) = self.mixer.channel(&o.id)
            {
                set(ch, v, mono);
            }
        }
        self.mixer.frame();
    }
}

/// A mixer channel for `v`'s note, from its position; `None` for a wave agb cannot play.
fn start(blob: Blob<'static>, v: &ChanView) -> Option<SoundChannel> {
    let size = blob.u32(v.wav + 12) as usize;
    let data = blob.samples(v.wav);
    if size == 0 || data.len() < size || !(data.as_ptr() as usize).is_multiple_of(4) {
        return None;
    }
    // SAFETY: 4-aligned (checked above), `'static`.
    let mut ch = SoundChannel::new_high_priority(unsafe { SoundData::new(&data[..size]) });
    if v.looped {
        let at = blob.u32(v.wav + 8).min(size as u32 - 1);
        ch.should_loop().restart_point(Num::<u32, 8>::new(at));
    }
    ch.set_pos(Num::<u32, 8>::new(v.start.min(size as u32 - 1)));
    Some(ch)
}

/// The note's pitch and its envelope's volumes this frame.
fn set(ch: &mut SoundChannel, v: &ChanView, mono: bool) {
    // `step` is source samples per 13,379 Hz output sample with 23 fraction bits.
    let speed = (v.step as u64 * SAMPLE_RATE as u64 / MIXER_HZ) >> 15;
    ch.playback(Num::<u32, 8>::from_raw(speed.min(u32::MAX as u64) as u32));
    // agb gives each side `(1 ∓ panning) / 2 × volume`; Gen3 `sample × envelope / 256`.
    // In mono both sides get both FIFOs at half: `(l + r) / 2` each, which panning 0 gives.
    let (l, r) = (v.left as i32, v.right as i32);
    let sum = l + r;
    let pan = if mono || sum == 0 {
        0
    } else {
        (r - l) * 256 / sum
    };
    ch.volume(Num::<i16, 8>::from_raw(sum as i16))
        .panning(Num::<i16, 8>::from_raw(pan as i16));
}
