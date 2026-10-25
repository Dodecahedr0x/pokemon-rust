//! The software back end: the engine, the DirectSound mixer of `SoundMainRAM` (13,379 Hz,
//! linear interpolation, `FIX` voices one sample per output sample, reverb) and [`SoftPsg`],
//! to 16-bit stereo at any host rate.
//!
//! Differences from the GBA, all inaudible or kinder: the DirectSound sum is clamped where Gen3
//! wraps its 8-bit lanes, the output keeps 16 bits where the GBA's DAC has 9, and a reversed
//! cry (`TONEDATA_TYPE_REV`) is read backwards from decoded PCM.

use crate::engine::{CHANS, Engine, SF_ON, TYPE_FIX, TYPE_REV};
use crate::psg::SoftPsg;
use crate::{Blob, Cmd, SAMPLE_RATE, SAMPLES_PER_FRAME};

/// Frames of DirectSound kept for the reverb (`pcmDmaPeriod`: 1,584 / 224).
const REVERB_FRAMES: usize = 7;
/// The GBA mixes PSG and DirectSound to ±512; this scales that to 16 bits.
const OUTPUT_GAIN: i32 = 60;

pub struct Synth<'a> {
    pub engine: Engine<'a>,
    psg: SoftPsg,
    /// The DirectSound output of the last frames (left, right), for the reverb.
    history: [[(i8, i8); SAMPLES_PER_FRAME]; REVERB_FRAMES],
    frame_index: usize,
    /// The current frame's output at 13,379 Hz.
    frame: [(i16, i16); SAMPLES_PER_FRAME],
    /// Position in `frame` of the next host sample, 16.16 in source samples.
    pos: u64,
    /// Source samples per host sample, 16.16.
    ratio: u64,
}

impl<'a> Synth<'a> {
    /// A synth that renders at `host_rate` Hz.
    pub fn new(blob: Blob<'a>, host_rate: u32) -> Self {
        let mut psg = SoftPsg::new(SAMPLE_RATE);
        let engine = Engine::new(blob, &mut psg);
        Synth {
            engine,
            psg,
            history: [[(0, 0); SAMPLES_PER_FRAME]; REVERB_FRAMES],
            frame_index: 0,
            frame: [(0, 0); SAMPLES_PER_FRAME],
            pos: (SAMPLES_PER_FRAME as u64) << 16,
            ratio: ((SAMPLE_RATE as u64) << 16) / host_rate.max(1) as u64,
        }
    }

    pub fn apply(&mut self, cmd: Cmd) {
        self.engine.apply(cmd, &mut self.psg);
    }

    /// Fills `out` with host-rate samples, running engine frames as needed.
    pub fn render(&mut self, out: &mut [(i16, i16)]) {
        for o in out.iter_mut() {
            while (self.pos >> 16) as usize >= SAMPLES_PER_FRAME {
                self.pos -= (SAMPLES_PER_FRAME as u64) << 16;
                self.next_frame();
            }
            let i = (self.pos >> 16) as usize;
            let f = (self.pos & 0xFFFF) as i32;
            let a = self.frame[i];
            let b = if i + 1 < SAMPLES_PER_FRAME {
                self.frame[i + 1]
            } else {
                a
            };
            let lerp = |x: i16, y: i16| (x as i32 + (((y as i32 - x as i32) * f) >> 16)) as i16;
            *o = (lerp(a.0, b.0), lerp(a.1, b.1));
            self.pos += self.ratio;
        }
    }

    /// One engine frame mixed into `self.frame` (`SoundMain`).
    pub fn next_frame(&mut self) {
        self.engine.frame(&mut self.psg);
        let mut l = [0i32; SAMPLES_PER_FRAME];
        let mut r = [0i32; SAMPLES_PER_FRAME];
        // The reverb: the frames 7 and 6 back, both sides, scaled by `reverb / 512`.
        let reverb = self.engine.reverb as i32;
        if reverb != 0 {
            let old = &self.history[self.frame_index];
            let next = &self.history[(self.frame_index + 1) % REVERB_FRAMES];
            for i in 0..SAMPLES_PER_FRAME {
                let s = old[i].0 as i32 + old[i].1 as i32 + next[i].0 as i32 + next[i].1 as i32;
                let mut v = (s * reverb) >> 9;
                if v & 0x80 != 0 {
                    v += 1;
                }
                l[i] = v;
                r[i] = v;
            }
        }
        for c in 0..CHANS {
            self.mix(c, &mut l, &mut r);
        }
        let mut psg = [(0i32, 0i32); SAMPLES_PER_FRAME];
        self.psg.render(&mut psg);
        let mono = self.engine.mono;
        let h = &mut self.history[self.frame_index];
        for i in 0..SAMPLES_PER_FRAME {
            let (dl, dr) = (l[i].clamp(-128, 127), r[i].clamp(-128, 127));
            h[i] = (dl as i8, dr as i8);
            // DirectSound at 100% (×2); in mono both FIFOs go to both sides at 50%.
            let (ol, or) = if mono {
                (dl + dr, dl + dr)
            } else {
                (dl * 2, dr * 2)
            };
            let out = |x: i32| (x * OUTPUT_GAIN).clamp(-32768, 32767) as i16;
            self.frame[i] = (out(ol + psg[i].0), out(or + psg[i].1));
        }
        self.frame_index = (self.frame_index + 1) % REVERB_FRAMES;
    }

    /// `SoundMainRAM`'s mixing of DirectSound channel `c` for one frame.
    fn mix(&mut self, c: usize, l: &mut [i32], r: &mut [i32]) {
        let e = &self.engine;
        let ch = e.chans[c];
        if ch.status & SF_ON == 0 {
            return;
        }
        let blob = e.blob();
        let data = blob.samples(ch.wav);
        let size = e.wave_size(ch.wav) as usize;
        if data.is_empty() || size == 0 {
            self.engine.chans[c].status = 0;
            return;
        }
        let (vr, vl) = (ch.env_right_or_goal as i32, ch.env_left_or_counter as i32);
        let fix = ch.ty & TYPE_FIX != 0;
        let rev = ch.ty & TYPE_REV != 0;
        let step = e.step_of(c);
        let sample = |i: u32| -> i32 {
            let i = (i as usize).min(size);
            if rev {
                data[size.saturating_sub(1 + i).min(size)] as i8 as i32
            } else {
                data[i] as i8 as i32
            }
        };
        let mut fw = ch.fw;
        for i in 0..SAMPLES_PER_FRAME {
            let pos = self.engine.chans[c].pos;
            let v = if fix {
                sample(pos)
            } else {
                let (s0, s1) = (sample(pos), sample(pos + 1));
                s0 + (((fw >> 7) as i32 * (s1 - s0)) >> 16)
            };
            // `(sample * volume) >> 8`, floored, into each side.
            r[i] += (v * vr) >> 8;
            l[i] += (v * vl) >> 8;
            fw += step;
            let n = fw >> 23;
            if n != 0 {
                fw &= 0x7F_FFFF;
                self.engine.consume(c, n as i64);
                if self.engine.chans[c].status == 0 {
                    break;
                }
            }
        }
        self.engine.chans[c].fw = fw;
    }
}
