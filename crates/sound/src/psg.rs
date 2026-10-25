//! The GB sound registers the engine writes (`CgbSound`, `ply_port`) and a software emulation
//! of the four PSG channels for the native back end.

/// Register offsets from `REG_SOUND1CNT_L` (`0x04000060`).
pub mod reg {
    pub const NR10: u8 = 0x00;
    pub const NR11: u8 = 0x02;
    pub const NR12: u8 = 0x03;
    pub const NR13: u8 = 0x04;
    pub const NR14: u8 = 0x05;
    pub const NR21: u8 = 0x08;
    pub const NR22: u8 = 0x09;
    pub const NR23: u8 = 0x0C;
    pub const NR24: u8 = 0x0D;
    pub const NR30: u8 = 0x10;
    pub const NR31: u8 = 0x12;
    pub const NR32: u8 = 0x13;
    pub const NR33: u8 = 0x14;
    pub const NR34: u8 = 0x15;
    pub const NR41: u8 = 0x18;
    pub const NR42: u8 = 0x19;
    pub const NR43: u8 = 0x1C;
    pub const NR44: u8 = 0x1D;
    pub const NR50: u8 = 0x20;
    pub const NR51: u8 = 0x21;
    pub const SOUNDCNT_X: u8 = 0x24;
    /// `REG_WAVE_RAM0`: 16 bytes.
    pub const WAVE: u8 = 0x30;
}

/// Where the engine's sound register writes go: the GBA's registers, or [`SoftPsg`].
pub trait Psg {
    /// Writes `value` to the register at `reg` bytes past `0x04000060`.
    fn write(&mut self, reg: u8, value: u8);
}

/// Drops every write (the dry runs).
pub struct NullPsg;

impl Psg for NullPsg {
    fn write(&mut self, _: u8, _: u8) {}
}

/// The GB clock the PSG timers count (4,194,304 Hz on the GBA's sound unit).
const GB_CLOCK: u64 = 4_194_304;

#[derive(Clone, Copy, Default)]
struct Square {
    on: bool,
    duty: u8,
    length: u16,
    length_on: bool,
    volume: u8,
    env_up: bool,
    env_period: u8,
    env_timer: u8,
    freq: u16,
    /// Phase in 1/8 of a period, 16.16 fixed point.
    phase: u32,
    sweep_period: u8,
    sweep_down: bool,
    sweep_shift: u8,
    sweep_timer: u8,
}

#[derive(Clone, Copy, Default)]
struct Wave {
    on: bool,
    dac: bool,
    length: u16,
    length_on: bool,
    /// Output shift: 0 mute, else the sample is shifted right by `shift - 1`; 75% separately.
    level: u8,
    freq: u16,
    phase: u32,
}

#[derive(Clone, Copy, Default)]
struct Noise {
    on: bool,
    length: u16,
    length_on: bool,
    volume: u8,
    env_up: bool,
    env_period: u8,
    env_timer: u8,
    nr43: u8,
    lfsr: u16,
    phase: u32,
}

/// A software model of the four PSG channels, rendered to signed samples: each channel at
/// ±(volume × (NR50 level + 1)) / 2 around zero, as the GBA mixes its PSG at 100%
/// (`SOUND_CGB_MIX_FULL`), next to DirectSound's ±256.
pub struct SoftPsg {
    regs: [u8; 0x50],
    sq: [Square; 2],
    wave: Wave,
    noise: Noise,
    wave_ram: [u8; 16],
    /// The frame sequencer (512 Hz), in output samples, 16.16.
    seq_phase: u32,
    seq_step: u8,
    rate: u32,
}

impl SoftPsg {
    /// A PSG rendered at `rate` Hz.
    pub fn new(rate: u32) -> Self {
        SoftPsg {
            regs: [0; 0x50],
            sq: [Square::default(); 2],
            wave: Wave::default(),
            noise: Noise {
                lfsr: 0x7FFF,
                ..Noise::default()
            },
            wave_ram: [0; 16],
            seq_phase: 0,
            seq_step: 0,
            rate,
        }
    }

    fn square_trigger(&mut self, i: usize) {
        let (nrx1, nrx2) = if i == 0 {
            (reg::NR11, reg::NR12)
        } else {
            (reg::NR21, reg::NR22)
        };
        let s = &mut self.sq[i];
        let r2 = self.regs[nrx2 as usize];
        s.on = r2 & 0xF8 != 0;
        if s.length == 0 {
            s.length = 64 - (self.regs[nrx1 as usize] & 0x3F) as u16;
        }
        s.volume = r2 >> 4;
        s.env_up = r2 & 8 != 0;
        s.env_period = r2 & 7;
        s.env_timer = s.env_period;
        if i == 0 {
            let r0 = self.regs[reg::NR10 as usize];
            s.sweep_period = (r0 >> 4) & 7;
            s.sweep_down = r0 & 8 != 0;
            s.sweep_shift = r0 & 7;
            s.sweep_timer = s.sweep_period;
        }
    }

    fn square_freq(&self, i: usize) -> u16 {
        let (lo, hi) = if i == 0 {
            (reg::NR13, reg::NR14)
        } else {
            (reg::NR23, reg::NR24)
        };
        self.regs[lo as usize] as u16 | ((self.regs[hi as usize] as u16 & 7) << 8)
    }

    /// One step of the 512 Hz frame sequencer: lengths at 256 Hz, sweep at 128 Hz, envelopes
    /// at 64 Hz.
    fn sequencer(&mut self) {
        let step = self.seq_step;
        self.seq_step = (step + 1) & 7;
        if step & 1 == 0 {
            for s in &mut self.sq {
                if s.length_on && s.length > 0 {
                    s.length -= 1;
                    if s.length == 0 {
                        s.on = false;
                    }
                }
            }
            let w = &mut self.wave;
            if w.length_on && w.length > 0 {
                w.length -= 1;
                if w.length == 0 {
                    w.on = false;
                }
            }
            let n = &mut self.noise;
            if n.length_on && n.length > 0 {
                n.length -= 1;
                if n.length == 0 {
                    n.on = false;
                }
            }
        }
        if step == 2 || step == 6 {
            let s = &mut self.sq[0];
            if s.sweep_period != 0 {
                if s.sweep_timer > 0 {
                    s.sweep_timer -= 1;
                }
                if s.sweep_timer == 0 {
                    s.sweep_timer = s.sweep_period;
                    if s.sweep_shift != 0 {
                        let d = s.freq >> s.sweep_shift;
                        let f = if s.sweep_down {
                            s.freq.wrapping_sub(d)
                        } else {
                            s.freq + d
                        };
                        if f > 2047 {
                            s.on = false;
                        } else {
                            s.freq = f;
                        }
                    }
                }
            }
        }
        if step == 7 {
            let env = |on: &mut bool, vol: &mut u8, up: bool, period: u8, timer: &mut u8| {
                let _ = on;
                if period == 0 {
                    return;
                }
                if *timer > 0 {
                    *timer -= 1;
                }
                if *timer == 0 {
                    *timer = period;
                    if up && *vol < 15 {
                        *vol += 1;
                    } else if !up && *vol > 0 {
                        *vol -= 1;
                    }
                }
            };
            for s in &mut self.sq {
                env(
                    &mut s.on,
                    &mut s.volume,
                    s.env_up,
                    s.env_period,
                    &mut s.env_timer,
                );
            }
            let n = &mut self.noise;
            env(
                &mut n.on,
                &mut n.volume,
                n.env_up,
                n.env_period,
                &mut n.env_timer,
            );
        }
    }

    /// Adds `out.len()` stereo samples of the PSG to `out` (left, right).
    pub fn render(&mut self, out: &mut [(i32, i32)]) {
        const DUTY: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];
        let rate = self.rate as u64;
        let seq_step = ((512u64 << 16) / rate) as u32;
        let nr50 = self.regs[reg::NR50 as usize];
        let nr51 = self.regs[reg::NR51 as usize];
        let (vol_r, vol_l) = ((nr50 & 7) as i32 + 1, ((nr50 >> 4) & 7) as i32 + 1);
        let enabled = self.regs[reg::SOUNDCNT_X as usize] & 0x80 != 0;
        for o in out.iter_mut() {
            self.seq_phase += seq_step;
            while self.seq_phase >= 1 << 16 {
                self.seq_phase -= 1 << 16;
                self.sequencer();
            }
            if !enabled {
                continue;
            }
            let mut v = [0i32; 4];
            for (i, s) in self.sq.iter_mut().enumerate() {
                if !s.on {
                    continue;
                }
                // 8 duty steps per period of (2048 - f) * 4 GB cycles each.
                let period = (2048 - s.freq.min(2047) as u64) * 4;
                s.phase = s
                    .phase
                    .wrapping_add(((GB_CLOCK << 16) / (period * rate)) as u32);
                let step = (s.phase >> 16) & 7;
                let high = DUTY[s.duty as usize & 3] >> step & 1 != 0;
                let a = s.volume as i32;
                v[i] = if high { a } else { -a };
            }
            let w = &mut self.wave;
            if w.on && w.dac {
                let period = (2048 - w.freq.min(2047) as u64) * 2;
                w.phase = w
                    .phase
                    .wrapping_add(((GB_CLOCK << 16) / (period * rate)) as u32);
                let n = ((w.phase >> 16) & 31) as usize;
                let b = self.wave_ram[n / 2];
                let s = if n & 1 == 0 { b >> 4 } else { b & 15 } as i32;
                let s = s * 2 - 15;
                v[2] = match w.level {
                    1 => s,
                    2 => s / 2,
                    3 => s / 4,
                    4 => s * 3 / 4,
                    _ => 0,
                };
            }
            let n = &mut self.noise;
            if n.on {
                let r = (n.nr43 & 7) as u64;
                let shift = (n.nr43 >> 4) as u64;
                let div = if r == 0 { 8 } else { r * 16 };
                let hz_x16 = (GB_CLOCK << 16) / (div << shift);
                n.phase = n.phase.wrapping_add((hz_x16 / rate) as u32);
                while n.phase >= 1 << 16 {
                    n.phase -= 1 << 16;
                    let bit = (n.lfsr ^ (n.lfsr >> 1)) & 1;
                    n.lfsr = (n.lfsr >> 1) | (bit << 14);
                    if n.nr43 & 8 != 0 {
                        n.lfsr = (n.lfsr & !(1 << 6)) | (bit << 6);
                    }
                }
                let a = n.volume as i32;
                v[3] = if n.lfsr & 1 == 0 { a } else { -a };
            }
            let (mut l, mut r) = (0, 0);
            for (i, x) in v.iter().enumerate() {
                if nr51 & (1 << i) != 0 {
                    r += x;
                }
                if nr51 & (0x10 << i) != 0 {
                    l += x;
                }
            }
            o.0 += l * vol_l / 2;
            o.1 += r * vol_r / 2;
        }
    }
}

impl Psg for SoftPsg {
    fn write(&mut self, r: u8, value: u8) {
        if let Some(x) = self.regs.get_mut(r as usize) {
            *x = value;
        }
        match r {
            reg::NR11 | reg::NR21 => {
                let i = (r == reg::NR21) as usize;
                self.sq[i].duty = value >> 6;
                self.sq[i].length = 64 - (value & 0x3F) as u16;
            }
            reg::NR12 | reg::NR22 if value & 0xF8 == 0 => {
                self.sq[(r == reg::NR22) as usize].on = false;
            }
            reg::NR13 | reg::NR23 => {
                let i = (r == reg::NR23) as usize;
                self.sq[i].freq = self.square_freq(i);
            }
            reg::NR14 | reg::NR24 => {
                let i = (r == reg::NR24) as usize;
                self.sq[i].freq = self.square_freq(i);
                self.sq[i].length_on = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.square_trigger(i);
                }
            }
            reg::NR30 => {
                self.wave.dac = value & 0x80 != 0;
                if !self.wave.dac {
                    self.wave.on = false;
                }
            }
            reg::NR31 => self.wave.length = 256 - value as u16,
            reg::NR32 => {
                self.wave.level = if value & 0x80 != 0 {
                    4
                } else {
                    match (value >> 5) & 3 {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        _ => 3,
                    }
                };
            }
            reg::NR33 => {
                self.wave.freq = (self.wave.freq & 0x700) | value as u16;
            }
            reg::NR34 => {
                self.wave.freq = (self.wave.freq & 0xFF) | ((value as u16 & 7) << 8);
                self.wave.length_on = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.wave.on = self.wave.dac;
                    if self.wave.length == 0 {
                        self.wave.length = 256;
                    }
                    self.wave.phase = 0;
                }
            }
            reg::NR41 => self.noise.length = 64 - (value & 0x3F) as u16,
            reg::NR42 if value & 0xF8 == 0 => self.noise.on = false,
            reg::NR43 => self.noise.nr43 = value,
            reg::NR44 => {
                self.noise.length_on = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    let r2 = self.regs[reg::NR42 as usize];
                    let n = &mut self.noise;
                    n.on = r2 & 0xF8 != 0;
                    if n.length == 0 {
                        n.length = 64;
                    }
                    n.volume = r2 >> 4;
                    n.env_up = r2 & 8 != 0;
                    n.env_period = r2 & 7;
                    n.env_timer = n.env_period;
                    n.lfsr = 0x7FFF;
                }
            }
            0x30..=0x3F => self.wave_ram[(r - 0x30) as usize] = value,
            _ => {}
        }
    }
}
