//! The PSG channels: `CgbSound`, `CgbModVol`, `CgbOscOff` and `MidiKeyToCgbFreq`
//! (`src/m4a.c`). The registers go out through [`Psg`]; reads come from the engine's copy of
//! what it wrote (the bits Gen3 reads back are the written ones).

use crate::engine::{
    CHANS, Engine, SF_ENV, SF_ENV_ATTACK, SF_ENV_DECAY, SF_ENV_SUSTAIN, SF_IEC, SF_ON, SF_START,
    SF_STOP, TYPE_FIX,
};
use crate::psg::{Psg, reg};
use crate::tables::{CGB_FREQ, CGB_SCALE, CGB3_VOL, NOISE};

pub(crate) const MO_PIT: u8 = 0x02;
pub(crate) const MO_VOL: u8 = 0x01;
const ENV_DIR_INC: u8 = 0x08;
const ENV_DIR_DEC: u8 = 0x00;

/// `MidiKeyToCgbFreq` for PSG channel `chan` (1..=4).
pub(crate) fn midi_key_to_cgb_freq(chan: u8, key: u8, fine: u8) -> u32 {
    if chan == 4 {
        let k = if key <= 20 { 0 } else { (key - 21).min(59) };
        return NOISE[k as usize] as u32;
    }
    let (k, fine) = if key <= 35 {
        (0usize, 0i32)
    } else {
        let k = key - 36;
        if k > 130 {
            (130, 255)
        } else {
            (k as usize, fine as i32)
        }
    };
    let v = |k: usize| {
        let s = CGB_SCALE[k];
        (CGB_FREQ[(s & 0xF) as usize] as i32) >> (s >> 4)
    };
    let (a, b) = (v(k), v(k + 1));
    (a + ((fine * (b - a)) >> 8) + 2048) as u32
}

/// The registers of PSG channel `ch` (1..=4): NRx0..NRx4.
fn regs(ch: usize) -> [u8; 5] {
    match ch {
        1 => [reg::NR10, reg::NR11, reg::NR12, reg::NR13, reg::NR14],
        2 => [reg::NR10 + 1, reg::NR21, reg::NR22, reg::NR23, reg::NR24],
        3 => [reg::NR30, reg::NR31, reg::NR32, reg::NR33, reg::NR34],
        _ => [reg::NR30 + 1, reg::NR41, reg::NR42, reg::NR43, reg::NR44],
    }
}

impl Engine<'_> {
    fn reg(&self, r: u8) -> u8 {
        self.regs[r as usize]
    }

    /// `CgbOscOff`.
    pub(crate) fn cgb_osc_off(&mut self, psg: &mut impl Psg, ch: usize) {
        match ch {
            1 => {
                self.write(psg, reg::NR12, 8);
                self.write(psg, reg::NR14, 0x80);
            }
            2 => {
                self.write(psg, reg::NR22, 8);
                self.write(psg, reg::NR24, 0x80);
            }
            3 => self.write(psg, reg::NR30, 0),
            _ => {
                self.write(psg, reg::NR42, 8);
                self.write(psg, reg::NR44, 0x80);
            }
        }
    }

    /// `CgbModVol`.
    fn cgb_mod_vol(&mut self, c: usize) {
        let mono = self.mono;
        let ch = &mut self.chans[c];
        let (r, l) = (ch.right_volume as u32, ch.left_volume as u32);
        // `CgbPan`: hard left or right when one side is at most half the other.
        let hard = if r >= l {
            (r / 2 >= l).then_some(0x0F)
        } else {
            (l / 2 >= r).then_some(0xF0)
        };
        let goal = (l + r) / 16;
        match (mono, hard) {
            (false, Some(pan)) => {
                ch.pan = pan;
                ch.env_right_or_goal = goal.min(15) as u8;
            }
            _ => {
                ch.pan = 0xFF;
                ch.env_right_or_goal = goal as u8;
            }
        }
        ch.sustain_goal = ((ch.env_right_or_goal as u32 * ch.sustain as u32 + 15) >> 4) as u8;
        ch.pan &= ch.pan_mask;
    }

    /// `CgbSound`.
    pub(crate) fn cgb_sound(&mut self, psg: &mut impl Psg) {
        if self.c15 != 0 {
            self.c15 -= 1;
        } else {
            self.c15 = 14;
        }
        for ch in 1..=4usize {
            let c = CHANS + ch - 1;
            if self.chans[c].status & SF_ON == 0 {
                continue;
            }
            let [nrx0, nrx1, nrx2, nrx3, nrx4] = regs(ch);
            let mut prev_c15 = self.c15 as i32;
            let mut step_dir = self.reg(nrx2);
            // The envelope (labels as in m4a.c).
            #[derive(Clone, Copy, PartialEq)]
            enum At {
                StepRepeat,
                StepComplete,
                Complete,
                OscOff,
                DecayStart,
                PseudoEchoStart,
                SustainStart,
                Sustain,
            }
            let status = self.chans[c].status;
            let mut at = if status & SF_START != 0 {
                if status & SF_STOP == 0 {
                    self.chans[c].status = SF_ENV_ATTACK;
                    self.chans[c].modify = MO_PIT | MO_VOL;
                    self.cgb_mod_vol(c);
                    let x = self.chans[c];
                    match ch {
                        1 | 2 => {
                            if ch == 1 {
                                self.write(psg, nrx0, x.sweep);
                            }
                            self.write(psg, nrx1, ((x.wav << 6) as u8).wrapping_add(x.length));
                            step_dir = x.attack.wrapping_add(ENV_DIR_INC);
                            self.chans[c].n4 = if x.length != 0 { 0x40 } else { 0 };
                        }
                        3 => {
                            if x.wav != x.current_wave {
                                self.write(psg, nrx0, 0x40);
                                for i in 0..16 {
                                    let b = self.blob().u8(x.wav + i);
                                    self.write(psg, reg::WAVE + i as u8, b);
                                }
                                self.chans[c].current_wave = x.wav;
                            }
                            self.write(psg, nrx0, 0);
                            self.write(psg, nrx1, x.length);
                            self.chans[c].n4 = if x.length != 0 { 0xC0 } else { 0x80 };
                        }
                        _ => {
                            self.write(psg, nrx1, x.length);
                            self.write(psg, nrx3, (x.wav << 3) as u8);
                            step_dir = x.attack.wrapping_add(ENV_DIR_INC);
                            self.chans[c].n4 = if x.length != 0 { 0x40 } else { 0 };
                        }
                    }
                    self.chans[c].env_left_or_counter = x.attack;
                    if x.attack as i8 != 0 {
                        self.chans[c].envelope_volume = 0;
                        At::StepComplete
                    } else {
                        At::DecayStart
                    }
                } else {
                    At::OscOff
                }
            } else if status & SF_IEC != 0 {
                let ch_ = &mut self.chans[c];
                ch_.echo_length = ch_.echo_length.wrapping_sub(1);
                if (ch_.echo_length as i8) <= 0 {
                    At::OscOff
                } else {
                    At::Complete
                }
            } else if status & SF_STOP != 0 && status & SF_ENV != 0 {
                let ch_ = &mut self.chans[c];
                ch_.status &= !SF_ENV;
                ch_.env_left_or_counter = ch_.release;
                if ch_.release as i8 != 0 {
                    ch_.modify |= MO_VOL;
                    if ch != 3 {
                        step_dir = ch_.release | ENV_DIR_DEC;
                    }
                    At::StepComplete
                } else {
                    At::PseudoEchoStart
                }
            } else {
                At::StepRepeat
            };
            loop {
                match at {
                    At::StepRepeat => {
                        if self.chans[c].env_left_or_counter == 0 {
                            if ch == 3 {
                                self.chans[c].modify |= MO_VOL;
                            }
                            self.cgb_mod_vol(c);
                            let x = &mut self.chans[c];
                            match x.status & SF_ENV {
                                0 => {
                                    // release
                                    x.envelope_volume = x.envelope_volume.wrapping_sub(1);
                                    if (x.envelope_volume as i8) <= 0 {
                                        at = At::PseudoEchoStart;
                                        continue;
                                    }
                                    x.env_left_or_counter = x.release;
                                }
                                SF_ENV_SUSTAIN => {
                                    at = At::Sustain;
                                    continue;
                                }
                                SF_ENV_DECAY => {
                                    x.envelope_volume = x.envelope_volume.wrapping_sub(1);
                                    if (x.envelope_volume as i8) <= (x.sustain_goal as i8) {
                                        at = At::SustainStart;
                                        continue;
                                    }
                                    x.env_left_or_counter = x.decay;
                                }
                                _ => {
                                    x.envelope_volume = x.envelope_volume.wrapping_add(1);
                                    if x.envelope_volume >= x.env_right_or_goal {
                                        at = At::DecayStart;
                                        continue;
                                    }
                                    x.env_left_or_counter = x.attack;
                                }
                            }
                        }
                        at = At::StepComplete;
                    }
                    At::PseudoEchoStart => {
                        let x = &mut self.chans[c];
                        x.envelope_volume =
                            ((x.env_right_or_goal as u32 * x.echo_volume as u32 + 0xFF) >> 8) as u8;
                        if x.envelope_volume != 0 {
                            x.status |= SF_IEC;
                            x.modify |= MO_VOL;
                            if ch != 3 {
                                step_dir = ENV_DIR_INC;
                            }
                            at = At::Complete;
                        } else {
                            at = At::OscOff;
                        }
                    }
                    At::SustainStart => {
                        let x = &mut self.chans[c];
                        if x.sustain == 0 {
                            x.status &= !SF_ENV;
                            at = At::PseudoEchoStart;
                        } else {
                            x.status = x.status.wrapping_sub(1);
                            x.modify |= MO_VOL;
                            if ch != 3 {
                                step_dir = ENV_DIR_INC;
                            }
                            at = At::Sustain;
                        }
                    }
                    At::Sustain => {
                        let x = &mut self.chans[c];
                        x.envelope_volume = x.sustain_goal;
                        x.env_left_or_counter = 7;
                        at = At::StepComplete;
                    }
                    At::DecayStart => {
                        let x = &mut self.chans[c];
                        x.status = x.status.wrapping_sub(1);
                        x.env_left_or_counter = x.decay;
                        if x.env_left_or_counter != 0 {
                            x.modify |= MO_VOL;
                            x.envelope_volume = x.env_right_or_goal;
                            if ch != 3 {
                                step_dir = x.decay | ENV_DIR_DEC;
                            }
                            at = At::StepComplete;
                        } else {
                            at = At::SustainStart;
                        }
                    }
                    At::StepComplete => {
                        let x = &mut self.chans[c];
                        x.env_left_or_counter = x.env_left_or_counter.wrapping_sub(1);
                        if prev_c15 == 0 {
                            prev_c15 -= 1;
                            at = At::StepRepeat;
                        } else {
                            at = At::Complete;
                        }
                    }
                    At::OscOff => {
                        self.cgb_osc_off(psg, ch);
                        self.chans[c].status = 0;
                        break;
                    }
                    At::Complete => {
                        let x = self.chans[c];
                        if x.modify & MO_PIT != 0 {
                            let mut f = x.frequency;
                            if ch < 4 && x.ty & TYPE_FIX != 0 {
                                // SOUNDBIAS at 65,536 Hz PWM (`SOUND_MODE_DA_BIT_8`).
                                f = (f + 1) & 0x7FE;
                                self.chans[c].frequency = f;
                            }
                            if ch != 4 {
                                self.write(psg, nrx3, f as u8);
                            } else {
                                let v = (self.reg(nrx3) & 0x08) | f as u8;
                                self.write(psg, nrx3, v);
                            }
                            let n4 = (x.n4 & 0xC0).wrapping_add((f >> 8) as u8);
                            self.chans[c].n4 = n4;
                            self.write(psg, nrx4, n4);
                        }
                        if x.modify & MO_VOL != 0 {
                            let x = self.chans[c];
                            let nr51 = (self.reg(reg::NR51) & !x.pan_mask) | x.pan;
                            self.write(psg, reg::NR51, nr51);
                            if ch == 3 {
                                let v = CGB3_VOL[(x.envelope_volume & 15) as usize];
                                self.write(psg, nrx2, v);
                                if x.n4 & 0x80 != 0 {
                                    self.write(psg, nrx0, 0x80);
                                    self.write(psg, nrx4, x.n4);
                                    self.chans[c].n4 = x.n4 & 0x7F;
                                }
                            } else {
                                let v = (step_dir & 0xF).wrapping_add(x.envelope_volume << 4);
                                self.write(psg, nrx2, v);
                                self.write(psg, nrx4, x.n4 | 0x80);
                                if ch == 1 && self.reg(nrx0) & 0x08 == 0 {
                                    self.write(psg, nrx4, x.n4 | 0x80);
                                }
                            }
                        }
                        break;
                    }
                }
            }
            self.chans[c].modify = 0;
        }
    }
}
